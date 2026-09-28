#![no_std]
// Test builds need std (panic::catch_unwind, println!); runtime stays no_std.
#[cfg(test)]
#[macro_use]
extern crate std;

use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, Env, String, Symbol, Vec,
};

use brain_storm_shared::pagination::{paginate, MAX_PAGE_SIZE};
use brain_storm_shared::{access, validation};

#[contracttype]
pub enum DataKey {
    Admin,
    NftOwner(u32),                    // nft_id -> owner
    NftMetadata(u32),                 // nft_id -> metadata
    CourseNfts(Address),              // owner -> Vec<nft_id>
    NextNftId,
    RoyaltyBasis(u32),                // nft_id -> royalty basis points
    RoyaltyRecipient(u32),            // nft_id -> instructor address
    AccessRights(u32, Address),       // (nft_id, holder) -> has_access
    Listing(u32),                     // nft_id -> Listing
    BurnedNft(u32),                   // nft_id -> bool
}

#[contracttype]
#[derive(Clone)]
pub struct NftMetadata {
    pub nft_id: u32,
    pub course_id: Symbol,
    pub course_name: String,
    pub owner: Address,
    pub purchase_date: u64,
    pub royalty_basis: u32,
}

#[contracttype]
#[derive(Clone)]
pub struct CourseNft {
    pub nft_id: u32,
    pub course_id: Symbol,
    pub owner: Address,
    pub instructor: Address,
    pub purchase_price: i128,
}

#[contracttype]
#[derive(Clone)]
pub struct Listing {
    pub nft_id: u32,
    pub seller: Address,
    pub price: i128,
    pub listed_at: u64,
}

#[contract]
pub struct NftContract;

#[contractimpl]
impl NftContract {
    pub fn initialize(env: Env, admin: Address) {
        assert!(
            !env.storage().instance().has(&DataKey::Admin),
            "Already initialized"
        );
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::NextNftId, &0_u32);
    }

    pub fn get_admin(env: Env) -> Address {
        env.storage().instance().get(&DataKey::Admin).unwrap()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn mint_course_nft(
        env: Env,
        admin: Address,
        owner: Address,
        course_id: Symbol,
        course_name: String,
        instructor: Address,
        purchase_price: i128,
        royalty_basis: u32,
    ) -> u32 {
        access::require_admin(&env, &admin, &DataKey::Admin);
        // Shared metadata rules (#1170) — same helpers credential_metadata uses.
        validation::require_valid_course_name(&course_name);
        validation::require_valid_royalty_basis(royalty_basis);

        // OPTIMIZATION: Cache NextNftId read (issue #1001)
        let nft_id_key = DataKey::NextNftId;
        let nft_id: u32 = env.storage().instance().get(&nft_id_key).unwrap_or(0);
        let next_id = nft_id + 1;

        let metadata = NftMetadata {
            nft_id,
            course_id: course_id.clone(),
            course_name,
            owner: owner.clone(),
            purchase_date: env.ledger().timestamp(),
            royalty_basis,
        };

        // OPTIMIZATION: Batch storage writes to reduce instruction count
        env.storage()
            .instance()
            .set(&DataKey::NftMetadata(nft_id), &metadata);
        env.storage()
            .instance()
            .set(&DataKey::NftOwner(nft_id), &owner);
        env.storage()
            .instance()
            .set(&DataKey::RoyaltyBasis(nft_id), &royalty_basis);
        env.storage()
            .instance()
            .set(&DataKey::RoyaltyRecipient(nft_id), &instructor);
        env.storage()
            .instance()
            .set(&nft_id_key, &next_id);

        // Add to owner's NFT list
        let owner_key = DataKey::CourseNfts(owner.clone());
        let mut nfts: Vec<u32> = env
            .storage()
            .instance()
            .get(&owner_key)
            .unwrap_or_else(|| Vec::new(&env));
        nfts.push_back(nft_id);
        env.storage().instance().set(&owner_key, &nfts);

        // Grant access to owner
        env.storage()
            .instance()
            .set(&DataKey::AccessRights(nft_id, owner.clone()), &true);

        env.events().publish(
            (symbol_short!("nft"), symbol_short!("minted")),
            (nft_id, course_id, owner, purchase_price),
        );

        nft_id
    }

    pub fn transfer_nft(env: Env, from: Address, to: Address, nft_id: u32) {
        // OPTIMIZATION: Cache owner read (issue #1001)
        let owner_key = DataKey::NftOwner(nft_id);
        let current_owner: Address = env
            .storage()
            .instance()
            .get(&owner_key)
            .expect("NFT not found");
        access::require_owner(&from, &current_owner);

        // Update owner
        env.storage().instance().set(&owner_key, &to);

        // OPTIMIZATION: Cache and batch metadata update
        let metadata_key = DataKey::NftMetadata(nft_id);
        let mut metadata: NftMetadata = env
            .storage()
            .instance()
            .get(&metadata_key)
            .expect("Metadata not found");
        metadata.owner = to.clone();
        env.storage().instance().set(&metadata_key, &metadata);

        // OPTIMIZATION: Cache both owner list reads
        let from_key = DataKey::CourseNfts(from.clone());
        if let Some(mut nfts) = env.storage().instance().get::<DataKey, Vec<u32>>(&from_key) {
            if let Some(pos) = nfts.iter().position(|id| id == nft_id) {
                nfts.remove(pos as u32);
                env.storage().instance().set(&from_key, &nfts);
            }
        }

        let to_key = DataKey::CourseNfts(to.clone());
        let mut to_nfts: Vec<u32> = env
            .storage()
            .instance()
            .get(&to_key)
            .unwrap_or_else(|| Vec::new(&env));
        to_nfts.push_back(nft_id);
        env.storage().instance().set(&to_key, &to_nfts);

        // Grant access to new owner
        env.storage()
            .instance()
            .set(&DataKey::AccessRights(nft_id, to.clone()), &true);

        env.events().publish(
            (symbol_short!("nft"), symbol_short!("xfer")),
            (nft_id, from, to),
        );
    }

    pub fn grant_access(env: Env, nft_owner: Address, nft_id: u32, holder: Address) {
        let owner_key = DataKey::NftOwner(nft_id);
        let current_owner: Address = env
            .storage()
            .instance()
            .get(&owner_key)
            .expect("NFT not found");
        access::require_owner(&nft_owner, &current_owner);

        env.storage()
            .instance()
            .set(&DataKey::AccessRights(nft_id, holder.clone()), &true);

        env.events().publish(
            (symbol_short!("nft"), symbol_short!("acc_grt")),
            (nft_id, holder),
        );
    }

    pub fn revoke_access(env: Env, nft_owner: Address, nft_id: u32, holder: Address) {
        let owner_key = DataKey::NftOwner(nft_id);
        let current_owner: Address = env
            .storage()
            .instance()
            .get(&owner_key)
            .expect("NFT not found");
        access::require_owner(&nft_owner, &current_owner);

        env.storage()
            .instance()
            .remove(&DataKey::AccessRights(nft_id, holder.clone()));

        env.events().publish(
            (symbol_short!("nft"), symbol_short!("acc_rvk")),
            (nft_id, holder),
        );
    }

    pub fn has_access(env: Env, nft_id: u32, holder: Address) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::AccessRights(nft_id, holder.clone()))
            .unwrap_or(false)
    }

    pub fn get_nft_metadata(env: Env, nft_id: u32) -> Option<NftMetadata> {
        env.storage()
            .instance()
            .get(&DataKey::NftMetadata(nft_id))
    }

    pub fn get_nft_owner(env: Env, nft_id: u32) -> Option<Address> {
        env.storage()
            .instance()
            .get(&DataKey::NftOwner(nft_id))
    }

    pub fn get_owner_nfts(env: Env, owner: Address) -> Vec<u32> {
        env.storage()
            .instance()
            .get(&DataKey::CourseNfts(owner))
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Largest page `get_owner_nfts_paged` will ever return (Issue #1168).
    ///
    /// Surfaced so clients can pick a `limit` that will not be truncated.
    pub fn get_max_page_size() -> u32 {
        MAX_PAGE_SIZE
    }

    /// Paged view of [`Self::get_owner_nfts`] (Issue #1168).
    ///
    /// `limit` is capped at [`MAX_PAGE_SIZE`] by the shared pagination helper,
    /// so an over-limit request returns at most `MAX_PAGE_SIZE` items and a
    /// `limit` of 0 returns an empty page.
    pub fn get_owner_nfts_paged(env: Env, owner: Address, offset: u32, limit: u32) -> Vec<u32> {
        let all: Vec<u32> = env
            .storage()
            .instance()
            .get(&DataKey::CourseNfts(owner))
            .unwrap_or_else(|| Vec::new(&env));
        paginate(&env, &all, offset, limit)
    }

    pub fn get_royalty_info(env: Env, nft_id: u32) -> Option<(Address, u32)> {
        let recipient: Option<Address> = env
            .storage()
            .instance()
            .get(&DataKey::RoyaltyRecipient(nft_id));
        let basis: Option<u32> = env
            .storage()
            .instance()
            .get(&DataKey::RoyaltyBasis(nft_id));

        match (recipient, basis) {
            (Some(r), Some(b)) => Some((r, b)),
            _ => None,
        }
    }

    // -------------------------------------------------------------------------
    // Burn
    // -------------------------------------------------------------------------

    pub fn burn_nft(env: Env, owner: Address, nft_id: u32) {
        let current_owner: Address = env
            .storage()
            .instance()
            .get(&DataKey::NftOwner(nft_id))
            .expect("NFT not found");
        access::require_owner(&owner, &current_owner);
        assert!(
            !env.storage().instance().get::<DataKey, bool>(&DataKey::BurnedNft(nft_id)).unwrap_or(false),
            "Already burned"
        );

        // Remove from owner's list
        let owner_key = DataKey::CourseNfts(owner.clone());
        if let Some(mut nfts) = env.storage().instance().get::<DataKey, Vec<u32>>(&owner_key) {
            if let Some(pos) = nfts.iter().position(|id| id == nft_id) {
                nfts.remove(pos as u32);
                env.storage().instance().set(&owner_key, &nfts);
            }
        }

        // Remove listing if any
        env.storage().instance().remove(&DataKey::Listing(nft_id));

        // Mark burned, remove owner + metadata
        env.storage().instance().set(&DataKey::BurnedNft(nft_id), &true);
        env.storage().instance().remove(&DataKey::NftOwner(nft_id));
        env.storage().instance().remove(&DataKey::NftMetadata(nft_id));

        env.events().publish(
            (symbol_short!("nft"), symbol_short!("burned")),
            (nft_id, owner),
        );
    }

    // -------------------------------------------------------------------------
    // Marketplace
    // -------------------------------------------------------------------------

    pub fn list_nft(env: Env, seller: Address, nft_id: u32, price: i128) {
        assert!(price > 0, "Price must be positive");

        let current_owner: Address = env
            .storage()
            .instance()
            .get(&DataKey::NftOwner(nft_id))
            .expect("NFT not found");
        access::require_owner(&seller, &current_owner);
        assert!(
            !env.storage().instance().has(&DataKey::Listing(nft_id)),
            "Already listed"
        );

        let listing = Listing {
            nft_id,
            seller: seller.clone(),
            price,
            listed_at: env.ledger().timestamp(),
        };
        env.storage().instance().set(&DataKey::Listing(nft_id), &listing);

        env.events().publish(
            (symbol_short!("nft"), symbol_short!("listed")),
            (nft_id, seller, price),
        );
    }

    pub fn delist_nft(env: Env, seller: Address, nft_id: u32) {
        let listing: Listing = env
            .storage()
            .instance()
            .get(&DataKey::Listing(nft_id))
            .expect("Not listed");
        access::require_owner(&seller, &listing.seller);

        env.storage().instance().remove(&DataKey::Listing(nft_id));

        env.events().publish(
            (symbol_short!("nft"), symbol_short!("delisted")),
            (nft_id, seller),
        );
    }

    pub fn buy_nft(env: Env, buyer: Address, nft_id: u32) {
        buyer.require_auth();

        let listing: Listing = env
            .storage()
            .instance()
            .get(&DataKey::Listing(nft_id))
            .expect("Not listed");

        // Remove listing
        env.storage().instance().remove(&DataKey::Listing(nft_id));

        // Transfer ownership (reuse transfer logic)
        Self::transfer_nft(env.clone(), listing.seller.clone(), buyer.clone(), nft_id);

        // In production: transfer listing.price from buyer to seller (minus royalty)
        let royalty_basis: u32 = env
            .storage()
            .instance()
            .get(&DataKey::RoyaltyBasis(nft_id))
            .unwrap_or(0);
        let royalty_amount = (listing.price * royalty_basis as i128) / 10000;
        let seller_amount = listing.price - royalty_amount;

        env.events().publish(
            (symbol_short!("nft"), symbol_short!("sold")),
            (nft_id, listing.seller, buyer, seller_amount, royalty_amount),
        );
    }

    pub fn get_listing(env: Env, nft_id: u32) -> Option<Listing> {
        env.storage().instance().get(&DataKey::Listing(nft_id))
    }
}

#[cfg(test)]
mod tests;
