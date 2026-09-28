#![no_std]
// Test builds need std (panic::catch_unwind); runtime stays no_std.
#[cfg(test)]
extern crate std;

use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, Bytes, BytesN, Env, String, Symbol,
};

pub mod linkage;
pub mod validation;
pub use linkage::{
    set_nft_contract, get_credential_nft_link, get_nft_credential, is_linked,
    CredentialNftLink,
};

#[contracttype]
pub enum DataKey {
    Admin,
    Metadata(u64),
    MetadataHash(u64),
    MetadataHistory(u64, u32),
    HistoryCount(u64),
    /// Compact storage: content-addressed hash + off-chain URI pointer only.
    /// Replaces a full `MetadataRecord` blob when the contract is operating
    /// in reduced-footprint mode.  Both variants may exist for the same
    /// `credential_id` during migration; `get_metadata_compact` prefers this
    /// key and falls back to `Metadata`.
    CompactMeta(u64),
}

#[contracttype]
#[derive(Clone)]
pub struct MetadataRecord {
    pub credential_id: u64,
    pub course_name: String,
    pub completion_date: u64,
    pub expiry_timestamp: u64,
    pub grade: String,
    pub ipfs_hash: String,
}

/// Compact on-chain representation: stores only the content hash and a short
/// off-chain pointer.  All mutable human-readable fields live in IPFS (or
/// another content-addressed store) and are authenticated by `content_hash`.
///
/// Storage cost vs full `MetadataRecord`:
/// - Full record (Strings): ≈ 150–400 bytes depending on field lengths
/// - Compact record (fixed BytesN<32> + u64 timestamps): ≈ 56 bytes flat
#[contracttype]
#[derive(Clone)]
pub struct CompactMetadataRecord {
    pub credential_id: u64,
    /// SHA-256 / IPFS CIDv1 content hash (32 bytes, fixed size).
    pub content_hash: soroban_sdk::BytesN<32>,
    /// Timestamp at which the credential was issued.
    pub issued_at: u64,
    /// Timestamp at which the credential expires (0 = never).
    pub expiry_timestamp: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct MetadataHistoryEntry {
    pub credential_id: u64,
    pub course_name: String,
    pub grade: String,
    pub recorded_at: u64,
}

const STORE: Symbol = symbol_short!("store");
const UPDATE: Symbol = symbol_short!("update");
const EXPIRE: Symbol = symbol_short!("expire");
const RENEW: Symbol = symbol_short!("renew");
const GRACE_PERIOD_SECONDS: u64 = 30 * 24 * 60 * 60;

#[contract]
pub struct CredentialMetadataContract;

#[contractimpl]
impl CredentialMetadataContract {
    pub fn initialize(env: Env, admin: Address) {
        assert!(
            !env.storage().instance().has(&DataKey::Admin),
            "Already initialized"
        );
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
    }

    /// Initialise with NFT contract address for credential↔NFT linkage (Issue #635).
    pub fn initialize_with_nft(env: Env, admin: Address, nft_contract: Address) {
        assert!(
            !env.storage().instance().has(&DataKey::Admin),
            "Already initialized"
        );
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        linkage::set_nft_contract(&env, nft_contract);
    }

    /// Issue a credential AND atomically mint a linked NFT (Issue #635).
    ///
    /// Stores the credential metadata and calls the NFT contract cross-contract.
    /// If either operation fails the whole call is rolled back.
    ///
    /// # Returns
    /// The minted NFT ID.
    #[allow(clippy::too_many_arguments)]
    pub fn issue_with_nft(
        env: Env,
        admin: Address,
        credential_id: u64,
        course_name: String,
        completion_date: u64,
        expiry_timestamp: u64,
        grade: String,
        ipfs_hash: String,
        owner: Address,
        course_id: soroban_sdk::Symbol,
        instructor: Address,
        royalty_basis: u32,

    ) -> u32 {
        admin.require_auth();
        validation::validate_admin(&env, &admin);
        // Shared metadata rules (#1170) — same helpers the NFT contract uses.
        validation::validate_metadata_fields(&course_name, &grade, &ipfs_hash);

        // Store credential metadata first
        let metadata = MetadataRecord {
            credential_id,
            course_name: course_name.clone(),
            completion_date,
            expiry_timestamp,
            grade,
            ipfs_hash,
        };
        env.storage().persistent().set(&DataKey::Metadata(credential_id), &metadata);
        env.events().publish((STORE, symbol_short!("cred")), credential_id);

        // Atomically mint linked NFT (rolls back everything on failure)
        linkage::issue_and_mint_nft(
            &env,
            &admin,
            credential_id,
            owner,
            course_id,
            course_name,
            instructor,
            royalty_basis,
        )
    }

    /// Get the NFT link for a credential (Issue #635).
    pub fn get_credential_link(env: Env, credential_id: u64) -> Option<linkage::CredentialNftLink> {
        linkage::get_credential_nft_link(&env, credential_id)
    }

    /// Reverse lookup: get credential ID from NFT ID (Issue #635).
    pub fn get_nft_credential_id(env: Env, nft_id: u32) -> Option<u64> {
        linkage::get_nft_credential(&env, nft_id)
    }

    /// Check whether a credential has a linked NFT (Issue #635).
    pub fn credential_is_linked(env: Env, credential_id: u64) -> bool {
        linkage::is_linked(&env, credential_id)
    }

    pub fn store_metadata(
        env: Env,
        admin: Address,
        credential_id: u64,
        course_name: String,
        completion_date: u64,
        expiry_timestamp: u64,
        grade: String,
        ipfs_hash: String,
    ) {
        admin.require_auth();
        validation::validate_admin(&env, &admin);
        // Shared metadata rules (#1170) — reject invalid fields before storing.
        validation::validate_metadata_fields(&course_name, &grade, &ipfs_hash);

        let metadata = MetadataRecord {
            credential_id,
            course_name,
            completion_date,
            expiry_timestamp,
            grade,
            ipfs_hash,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Metadata(credential_id), &metadata);

        env.events()
            .publish((STORE, symbol_short!("cred")), credential_id);
    }

    pub fn update_metadata(
        env: Env,
        admin: Address,
        credential_id: u64,
        course_name: String,
        grade: String,
    ) {
        admin.require_auth();
        validation::validate_admin(&env, &admin);

        let mut metadata: MetadataRecord = validation::get_metadata_or_panic(&env, credential_id);
        // Shared metadata rules (#1170) — the new values must be valid too, and
        // the stored content pointer an update leaves untouched must still hold.
        validation::validate_metadata_fields(&course_name, &grade, &metadata.ipfs_hash);

        let history_count: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::HistoryCount(credential_id))
            .unwrap_or(0);

        let history_entry = MetadataHistoryEntry {
            credential_id,
            course_name: metadata.course_name.clone(),
            grade: metadata.grade.clone(),
            recorded_at: env.ledger().timestamp(),
        };

        env.storage().persistent().set(
            &DataKey::MetadataHistory(credential_id, history_count),
            &history_entry,
        );
        env.storage()
            .persistent()
            .set(&DataKey::HistoryCount(credential_id), &(history_count + 1));

        metadata.course_name = course_name;
        metadata.grade = grade;

        env.storage()
            .persistent()
            .set(&DataKey::Metadata(credential_id), &metadata);

        env.events()
            .publish((UPDATE, symbol_short!("cred")), credential_id);
    }

    pub fn get_metadata(env: Env, credential_id: u64) -> Option<MetadataRecord> {
        env.storage()
            .persistent()
            .get(&DataKey::Metadata(credential_id))
    }

    pub fn is_expired(env: Env, credential_id: u64) -> bool {
        let metadata = Self::get_metadata(env.clone(), credential_id);
        match metadata {
            Some(record) => env.ledger().timestamp() > record.expiry_timestamp,
            None => false,
        }
    }

    pub fn is_valid(env: Env, credential_id: u64) -> bool {
        let metadata = Self::get_metadata(env.clone(), credential_id);
        match metadata {
            Some(record) => env.ledger().timestamp() <= record.expiry_timestamp,
            None => false,
        }
    }

    pub fn can_renew(env: Env, credential_id: u64) -> bool {
        validation::is_renewable(&env, credential_id, GRACE_PERIOD_SECONDS)
    }

    pub fn renew_credential(
        env: Env,
        admin: Address,
        credential_id: u64,
        new_expiry_timestamp: u64,
    ) {
        admin.require_auth();
        validation::validate_admin(&env, &admin);

        let mut metadata: MetadataRecord = validation::get_metadata_or_panic(&env, credential_id);

        assert!(
            Self::can_renew(env.clone(), credential_id),
            "Credential not eligible for renewal"
        );

        validation::validate_future_timestamp(&env, new_expiry_timestamp);

        metadata.expiry_timestamp = new_expiry_timestamp;

        env.storage()
            .persistent()
            .set(&DataKey::Metadata(credential_id), &metadata);

        env.events()
            .publish((RENEW, symbol_short!("cred")), credential_id);
    }

    pub fn emit_expiry_event(env: Env, credential_id: u64) {
        env.events()
            .publish((EXPIRE, symbol_short!("cred")), credential_id);
    }

    pub fn store_metadata_hash(env: Env, admin: Address, credential_id: u64, hash: Bytes) {
        admin.require_auth();
        validation::validate_admin(&env, &admin);

        env.storage()
            .persistent()
            .set(&DataKey::MetadataHash(credential_id), &hash);
    }

    pub fn verify_metadata_hash(env: Env, credential_id: u64, hash: Bytes) -> bool {
        let stored_hash: Option<Bytes> = env
            .storage()
            .persistent()
            .get(&DataKey::MetadataHash(credential_id));
        match stored_hash {
            Some(h) => h == hash,
            None => false,
        }
    }

    // ── Compact / reduced-footprint storage (#1010) ───────────────────────────

    /// Store a compact metadata record: only a 32-byte content hash and two
    /// u64 timestamps.  All human-readable fields live off-chain (IPFS / CAS)
    /// and are authenticated by `content_hash`.
    ///
    /// Storage cost per entry is ≈ 56 bytes vs ≈ 150–400 bytes for the full
    /// `MetadataRecord`, giving a measurable reduction in Soroban rent.
    pub fn store_compact_metadata(
        env: Env,
        admin: Address,
        credential_id: u64,
        content_hash: soroban_sdk::BytesN<32>,
        issued_at: u64,
        expiry_timestamp: u64,
    ) {
        admin.require_auth();
        validation::validate_admin(&env, &admin);

        let record = CompactMetadataRecord {
            credential_id,
            content_hash,
            issued_at,
            expiry_timestamp,
        };
        env.storage()
            .persistent()
            .set(&DataKey::CompactMeta(credential_id), &record);
        env.events()
            .publish((STORE, symbol_short!("compact")), credential_id);
    }

    /// Retrieve the compact record for a credential, if present.
    pub fn get_compact_metadata(
        env: Env,
        credential_id: u64,
    ) -> Option<CompactMetadataRecord> {
        env.storage()
            .persistent()
            .get(&DataKey::CompactMeta(credential_id))
    }

    /// Verify a compact credential's content hash.
    /// Returns `true` when the supplied hash matches the stored hash.
    pub fn verify_compact_hash(
        env: Env,
        credential_id: u64,
        hash: soroban_sdk::BytesN<32>,
    ) -> bool {
        let stored: Option<CompactMetadataRecord> = env
            .storage()
            .persistent()
            .get(&DataKey::CompactMeta(credential_id));
        match stored {
            Some(r) => r.content_hash == hash,
            None => false,
        }
    }

    /// Check whether the compact credential has expired.
    pub fn is_compact_expired(env: Env, credential_id: u64) -> bool {
        let stored: Option<CompactMetadataRecord> = env
            .storage()
            .persistent()
            .get(&DataKey::CompactMeta(credential_id));
        match stored {
            Some(r) => r.expiry_timestamp != 0 && env.ledger().timestamp() > r.expiry_timestamp,
            None => false,
        }
    }

    /// Migration helper: read an existing full `MetadataRecord` and write it
    /// back as a `CompactMetadataRecord` (storing only the derived 32-byte hash
    /// of the ipfs_hash field and the timestamps).  The full record is **not**
    /// deleted so that callers that still depend on `get_metadata` continue to
    /// work unchanged.
    ///
    /// This lets the chain migrate entries incrementally; new writes should
    /// prefer `store_compact_metadata` directly.
    pub fn migrate_to_compact(
        env: Env,
        admin: Address,
        credential_id: u64,
        content_hash: soroban_sdk::BytesN<32>,
    ) {
        admin.require_auth();
        validation::validate_admin(&env, &admin);

        let full: MetadataRecord = validation::get_metadata_or_panic(&env, credential_id);

        let compact = CompactMetadataRecord {
            credential_id,
            content_hash,
            issued_at: full.completion_date,
            expiry_timestamp: full.expiry_timestamp,
        };
        env.storage()
            .persistent()
            .set(&DataKey::CompactMeta(credential_id), &compact);
        env.events()
            .publish((STORE, symbol_short!("migrated")), credential_id);
    }

    pub fn get_metadata_history(
        env: Env,
        credential_id: u64,
        index: u32,
    ) -> Option<MetadataHistoryEntry> {
        env.storage()
            .persistent()
            .get(&DataKey::MetadataHistory(credential_id, index))
    }

    pub fn get_history_count(env: Env, credential_id: u64) -> u32 {
        env.storage()
            .persistent()
            .get(&DataKey::HistoryCount(credential_id))
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod validation_tests;
