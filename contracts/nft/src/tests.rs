#[cfg(test)]
mod tests {
    use crate::{NftContract, NftContractClient};
    use soroban_sdk::{symbol_short, testutils::Address as _, Address, Env, String};

    fn setup() -> (Env, NftContractClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let id = env.register_contract(None, NftContract);
        let client = NftContractClient::new(&env, &id);
        let admin = Address::generate(&env);
        client.initialize(&admin);
        (env, client, admin)
    }

    fn mint_nft(
        env: &Env,
        client: &NftContractClient,
        admin: &Address,
        owner: &Address,
    ) -> u32 {
        let instructor = Address::generate(env);
        client.mint_course_nft(
            admin,
            owner,
            &symbol_short!("RUST101"),
            &String::from_str(env, "Rust Fundamentals"),
            &instructor,
            &1000,
            &500,
        )
    }

    #[test]
    fn test_initialize_sets_admin() {
        let (_, client, admin) = setup();
        assert_eq!(client.get_admin(), admin);
    }

    #[test]
    #[should_panic(expected = "Already initialized")]
    fn test_double_initialize_panics() {
        let (_, client, admin) = setup();
        client.initialize(&admin);
    }

    #[test]
    fn test_mint_course_nft_returns_id() {
        let (env, client, admin) = setup();
        let owner = Address::generate(&env);
        let nft_id = mint_nft(&env, &client, &admin, &owner);
        assert_eq!(nft_id, 0);
    }

    #[test]
    fn test_mint_increments_id() {
        let (env, client, admin) = setup();
        let owner = Address::generate(&env);
        let id1 = mint_nft(&env, &client, &admin, &owner);
        let id2 = mint_nft(&env, &client, &admin, &owner);
        assert_eq!(id1, 0);
        assert_eq!(id2, 1);
    }

    #[test]
    #[should_panic(expected = "Unauthorized: admin required")]
    fn test_non_admin_cannot_mint() {
        let (env, client, _) = setup();
        let rando = Address::generate(&env);
        let owner = Address::generate(&env);
        mint_nft(&env, &client, &rando, &owner);
    }

    #[test]
    #[should_panic(expected = "Royalty basis must be <= 10000")]
    fn test_royalty_basis_exceeds_max_panics() {
        let (env, client, admin) = setup();
        let owner = Address::generate(&env);
        let instructor = Address::generate(&env);
        client.mint_course_nft(
            &admin,
            &owner,
            &symbol_short!("RUST101"),
            &String::from_str(&env, "Rust"),
            &instructor,
            &1000,
            &10001, // exceeds 10000
        );
    }

    #[test]
    fn test_get_nft_metadata() {
        let (env, client, admin) = setup();
        let owner = Address::generate(&env);
        let nft_id = mint_nft(&env, &client, &admin, &owner);
        let metadata = client.get_nft_metadata(&nft_id).unwrap();
        assert_eq!(metadata.nft_id, nft_id);
        assert_eq!(metadata.owner, owner);
        assert_eq!(metadata.royalty_basis, 500);
    }

    #[test]
    fn test_get_nft_owner() {
        let (env, client, admin) = setup();
        let owner = Address::generate(&env);
        let nft_id = mint_nft(&env, &client, &admin, &owner);
        assert_eq!(client.get_nft_owner(&nft_id).unwrap(), owner);
    }

    #[test]
    fn test_get_owner_nfts() {
        let (env, client, admin) = setup();
        let owner = Address::generate(&env);
        mint_nft(&env, &client, &admin, &owner);
        mint_nft(&env, &client, &admin, &owner);
        let nfts = client.get_owner_nfts(&owner);
        assert_eq!(nfts.len(), 2);
    }

    #[test]
    fn test_transfer_nft_changes_owner() {
        let (env, client, admin) = setup();
        let owner = Address::generate(&env);
        let recipient = Address::generate(&env);
        let nft_id = mint_nft(&env, &client, &admin, &owner);
        client.transfer_nft(&owner, &recipient, &nft_id);
        assert_eq!(client.get_nft_owner(&nft_id).unwrap(), recipient);
    }

    #[test]
    #[should_panic(expected = "Unauthorized: owner required")]
    fn test_transfer_by_non_owner_panics() {
        let (env, client, admin) = setup();
        let owner = Address::generate(&env);
        let rando = Address::generate(&env);
        let recipient = Address::generate(&env);
        let nft_id = mint_nft(&env, &client, &admin, &owner);
        client.transfer_nft(&rando, &recipient, &nft_id);
    }

    #[test]
    fn test_owner_has_access_after_mint() {
        let (env, client, admin) = setup();
        let owner = Address::generate(&env);
        let nft_id = mint_nft(&env, &client, &admin, &owner);
        assert!(client.has_access(&nft_id, &owner));
    }

    #[test]
    fn test_grant_and_revoke_access() {
        let (env, client, admin) = setup();
        let owner = Address::generate(&env);
        let viewer = Address::generate(&env);
        let nft_id = mint_nft(&env, &client, &admin, &owner);

        assert!(!client.has_access(&nft_id, &viewer));
        client.grant_access(&owner, &nft_id, &viewer);
        assert!(client.has_access(&nft_id, &viewer));
        client.revoke_access(&owner, &nft_id, &viewer);
        assert!(!client.has_access(&nft_id, &viewer));
    }

    #[test]
    fn test_get_royalty_info() {
        let (env, client, admin) = setup();
        let owner = Address::generate(&env);
        let nft_id = mint_nft(&env, &client, &admin, &owner);
        let (_, basis) = client.get_royalty_info(&nft_id).unwrap();
        assert_eq!(basis, 500);
    }

    #[test]
    fn test_get_metadata_nonexistent_returns_none() {
        let (_, client, _) = setup();
        assert!(client.get_nft_metadata(&9999).is_none());
    }

    #[test]
    fn test_get_nft_owner_nonexistent_returns_none() {
        let (_, client, _) = setup();
        assert!(client.get_nft_owner(&9999).is_none());
    }

    #[test]
    fn test_get_owner_nfts_empty_for_unknown_owner() {
        let (env, client, _) = setup();
        let nobody = Address::generate(&env);
        assert_eq!(client.get_owner_nfts(&nobody).len(), 0);
    }

    // ── Pagination bounds (#1168) ─────────────────────────────────────────────

    #[test]
    fn test_get_max_page_size_returns_shared_cap() {
        let (_, client, _) = setup();
        assert_eq!(client.get_max_page_size(), 100);
    }

    #[test]
    fn test_get_owner_nfts_paged_zero_limit_returns_empty() {
        let (env, client, admin) = setup();
        let owner = Address::generate(&env);
        mint_nft(&env, &client, &admin, &owner);
        let page = client.get_owner_nfts_paged(&owner, &0, &0);
        assert_eq!(page.len(), 0);
    }

    #[test]
    fn test_get_owner_nfts_paged_over_limit_is_truncated_to_max_page_size() {
        let (env, client, admin) = setup();
        // 120 mints is well past the default test budget.
        env.budget().reset_unlimited();
        let owner = Address::generate(&env);
        for _ in 0..120 {
            mint_nft(&env, &client, &admin, &owner);
        }
        assert_eq!(client.get_owner_nfts(&owner).len(), 120);

        // Unbounded request → capped at the shared page-size limit.
        let page = client.get_owner_nfts_paged(&owner, &0, &u32::MAX);
        assert_eq!(page.len(), 100);

        // A mid-list over-limit request stays capped (70 remaining < 100).
        let tail = client.get_owner_nfts_paged(&owner, &50, &u32::MAX);
        assert_eq!(tail.len(), 70);

        // Exactly-at-the-limit requests are not truncated.
        let full = client.get_owner_nfts_paged(&owner, &0, &100);
        assert_eq!(full.len(), 100);
    }

    #[test]
    fn test_get_owner_nfts_paged_walks_the_whole_list() {
        let (env, client, admin) = setup();
        let owner = Address::generate(&env);
        for _ in 0..5 {
            mint_nft(&env, &client, &admin, &owner);
        }
        let mut seen = 0;
        let mut offset = 0;
        loop {
            let page = client.get_owner_nfts_paged(&owner, &offset, &2);
            if page.len() == 0 {
                break;
            }
            seen += page.len();
            offset += 2;
        }
        assert_eq!(seen, 5);
        assert_eq!(client.get_owner_nfts_paged(&owner, &0, &5).len(), 5);
    }

    #[test]
    fn test_get_royalty_info_nonexistent_returns_none() {
        let (_, client, _) = setup();
        assert!(client.get_royalty_info(&9999).is_none());
    }

    #[test]
    fn test_transfer_updates_nft_list_of_sender_and_receiver() {
        let (env, client, admin) = setup();
        let owner = Address::generate(&env);
        let recipient = Address::generate(&env);
        let nft_id = mint_nft(&env, &client, &admin, &owner);

        client.transfer_nft(&owner, &recipient, &nft_id);

        // sender's list should be empty
        assert_eq!(client.get_owner_nfts(&owner).len(), 0);
        // recipient's list should have the nft
        assert_eq!(client.get_owner_nfts(&recipient).len(), 1);
        assert_eq!(client.get_owner_nfts(&recipient).get(0).unwrap(), nft_id);
    }

    #[test]
    fn test_grant_access_by_non_owner_panics() {
        let (env, client, admin) = setup();
        let owner = Address::generate(&env);
        let rando = Address::generate(&env);
        let viewer = Address::generate(&env);
        let nft_id = mint_nft(&env, &client, &admin, &owner);
        // rando is not the owner
        // mock_all_auths still allows the call but require_owner will panic
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            client.grant_access(&rando, &nft_id, &viewer);
        }));
        assert!(result.is_err());
    }

    // ── Burn ────────────────────────────────────────────────────────────────

    #[test]
    fn test_burn_nft_removes_owner() {
        let (env, client, admin) = setup();
        let owner = Address::generate(&env);
        let nft_id = mint_nft(&env, &client, &admin, &owner);
        client.burn_nft(&owner, &nft_id);
        assert!(client.get_nft_owner(&nft_id).is_none());
    }

    #[test]
    fn test_burn_nft_removes_metadata() {
        let (env, client, admin) = setup();
        let owner = Address::generate(&env);
        let nft_id = mint_nft(&env, &client, &admin, &owner);
        client.burn_nft(&owner, &nft_id);
        assert!(client.get_nft_metadata(&nft_id).is_none());
    }

    #[test]
    fn test_burn_nft_removes_from_owner_list() {
        let (env, client, admin) = setup();
        let owner = Address::generate(&env);
        let nft_id = mint_nft(&env, &client, &admin, &owner);
        assert_eq!(client.get_owner_nfts(&owner).len(), 1);
        client.burn_nft(&owner, &nft_id);
        assert_eq!(client.get_owner_nfts(&owner).len(), 0);
    }

    #[test]
    #[should_panic]
    fn test_burn_nonexistent_nft_panics() {
        let (env, client, _) = setup();
        let owner = Address::generate(&env);
        client.burn_nft(&owner, &9999);
    }

    #[test]
    #[should_panic(expected = "Already burned")]
    fn test_burn_already_burned_nft_panics() {
        let (env, client, admin) = setup();
        let owner = Address::generate(&env);
        let nft_id = mint_nft(&env, &client, &admin, &owner);
        client.burn_nft(&owner, &nft_id);
        client.burn_nft(&owner, &nft_id);
    }

    // ── Marketplace ─────────────────────────────────────────────────────────

    #[test]
    fn test_list_nft() {
        let (env, client, admin) = setup();
        let owner = Address::generate(&env);
        let nft_id = mint_nft(&env, &client, &admin, &owner);
        client.list_nft(&owner, &nft_id, &500);
        let listing = client.get_listing(&nft_id).unwrap();
        assert_eq!(listing.nft_id, nft_id);
        assert_eq!(listing.seller, owner);
        assert_eq!(listing.price, 500);
    }

    #[test]
    #[should_panic(expected = "Price must be positive")]
    fn test_list_nft_zero_price_panics() {
        let (env, client, admin) = setup();
        let owner = Address::generate(&env);
        let nft_id = mint_nft(&env, &client, &admin, &owner);
        client.list_nft(&owner, &nft_id, &0);
    }

    #[test]
    #[should_panic(expected = "Already listed")]
    fn test_list_nft_already_listed_panics() {
        let (env, client, admin) = setup();
        let owner = Address::generate(&env);
        let nft_id = mint_nft(&env, &client, &admin, &owner);
        client.list_nft(&owner, &nft_id, &500);
        client.list_nft(&owner, &nft_id, &600);
    }

    #[test]
    fn test_delist_nft() {
        let (env, client, admin) = setup();
        let owner = Address::generate(&env);
        let nft_id = mint_nft(&env, &client, &admin, &owner);
        client.list_nft(&owner, &nft_id, &500);
        client.delist_nft(&owner, &nft_id);
        assert!(client.get_listing(&nft_id).is_none());
    }

    #[test]
    #[should_panic]
    fn test_delist_nft_not_listed_panics() {
        let (env, client, admin) = setup();
        let owner = Address::generate(&env);
        let nft_id = mint_nft(&env, &client, &admin, &owner);
        client.delist_nft(&owner, &nft_id);
    }

    #[test]
    fn test_buy_nft_transfers_ownership() {
        let (env, client, admin) = setup();
        let owner = Address::generate(&env);
        let buyer = Address::generate(&env);
        let nft_id = mint_nft(&env, &client, &admin, &owner);
        client.list_nft(&owner, &nft_id, &500);
        client.buy_nft(&buyer, &nft_id);
        assert_eq!(client.get_nft_owner(&nft_id).unwrap(), buyer);
    }

    #[test]
    fn test_buy_nft_removes_listing() {
        let (env, client, admin) = setup();
        let owner = Address::generate(&env);
        let buyer = Address::generate(&env);
        let nft_id = mint_nft(&env, &client, &admin, &owner);
        client.list_nft(&owner, &nft_id, &500);
        client.buy_nft(&buyer, &nft_id);
        assert!(client.get_listing(&nft_id).is_none());
    }

    #[test]
    #[should_panic]
    fn test_buy_nft_not_listed_panics() {
        let (env, client, admin) = setup();
        let owner = Address::generate(&env);
        let buyer = Address::generate(&env);
        let nft_id = mint_nft(&env, &client, &admin, &owner);
        client.buy_nft(&buyer, &nft_id);
    }

    #[test]
    fn test_get_listing_returns_none_for_nonexistent() {
        let (_, client, _) = setup();
        assert!(client.get_listing(&9999).is_none());
    }

    // ── Benchmarks (Issue #1001) ────────────────────────────────────────────
    
    #[test]
    fn test_benchmark_mint_instruction_count() {
        let (env, client, admin) = setup();
        let owner = Address::generate(&env);
        let instructor = Address::generate(&env);
        
        // Baseline: single mint instruction count check
        env.budget().reset_default();
        let nft_id = client.mint_course_nft(
            &admin,
            &owner,
            &symbol_short!("RUST101"),
            &String::from_str(&env, "Rust Fundamentals"),
            &instructor,
            &1000,
            &500,
        );
        
        // Assert instruction count within budget (optimized: <100k CPU instructions)
        let cpu_instructions = env.budget().cpu_instruction_cost();
        println!("Mint CPU instructions: {}", cpu_instructions);
        assert!(cpu_instructions < 150_000, "Mint exceeded budget: {} instructions", cpu_instructions);
        assert_eq!(nft_id, 0);
    }
    
    #[test]
    fn test_benchmark_transfer_instruction_count() {
        let (env, client, admin) = setup();
        let owner = Address::generate(&env);
        let new_owner = Address::generate(&env);
        
        let nft_id = mint_nft(&env, &client, &admin, &owner);
        
        env.budget().reset_default();
        client.transfer_nft(&owner, &new_owner, &nft_id);
        
        let cpu_instructions = env.budget().cpu_instruction_cost();
        println!("Transfer CPU instructions: {}", cpu_instructions);
        assert!(cpu_instructions < 150_000, "Transfer exceeded budget: {} instructions", cpu_instructions);
        assert_eq!(client.get_nft_owner(&nft_id).unwrap(), new_owner);
    }

    #[test]
    fn test_benchmark_multiple_mints() {
        let (env, client, admin) = setup();
        let owner = Address::generate(&env);
        
        env.budget().reset_default();
        // Mint 5 NFTs and measure total instruction cost
        for _ in 0..5 {
            mint_nft(&env, &client, &admin, &owner);
        }
        
        let cpu_instructions = env.budget().cpu_instruction_cost();
        println!("5-mint total CPU instructions: {}", cpu_instructions);
        // 5 mints should be reasonably efficient with caching
        assert!(cpu_instructions < 600_000, "5 mints exceeded budget: {} instructions", cpu_instructions);
        
        // Verify all 5 were minted
        assert_eq!(client.get_owner_nfts(&owner).len(), 5);
    }

    // ── Metadata validation (Issue #1170) ─────────────────────────────────────
    // The rules live in `brain_storm_shared::validation` and are shared with
    // `contracts/credential_metadata`.

    fn mint_with_course_name(env: &Env, client: &NftContractClient, admin: &Address, name: &str) {
        let owner = Address::generate(env);
        let instructor = Address::generate(env);
        client.mint_course_nft(
            admin,
            &owner,
            &symbol_short!("RUST101"),
            &String::from_str(env, name),
            &instructor,
            &1000,
            &500,
        );
    }

    #[test]
    #[should_panic(expected = "Course name must be between 3 and 100 characters")]
    fn test_mint_rejects_empty_course_name() {
        let (env, client, admin) = setup();
        mint_with_course_name(&env, &client, &admin, "");
    }

    #[test]
    #[should_panic(expected = "Course name must be between 3 and 100 characters")]
    fn test_mint_rejects_too_short_course_name() {
        let (env, client, admin) = setup();
        mint_with_course_name(&env, &client, &admin, "Ab");
    }

    #[test]
    #[should_panic(expected = "Course name must be between 3 and 100 characters")]
    fn test_mint_rejects_overlong_course_name() {
        let (env, client, admin) = setup();
        let overlong = "X".repeat(101);
        mint_with_course_name(&env, &client, &admin, &overlong);
    }

    #[test]
    fn test_mint_accepts_boundary_course_name_and_royalty() {
        let (env, client, admin) = setup();
        let owner = Address::generate(&env);
        let instructor = Address::generate(&env);
        // Exactly 3 characters and the maximum royalty basis both pass.
        let nft_id = client.mint_course_nft(
            &admin,
            &owner,
            &symbol_short!("RUST101"),
            &String::from_str(&env, "Rst"),
            &instructor,
            &1000,
            &10000,
        );
        assert_eq!(client.get_royalty_info(&nft_id).unwrap().1, 10000);
    }
}
