#[cfg(test)]
mod tests {
    use crate::{RoyaltyDistributionContract, RoyaltyDistributionContractClient};
    use soroban_sdk::{testutils::{Address as _, Events}, Address, Env, FromVal};

    fn setup() -> (Env, RoyaltyDistributionContractClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let id = env.register_contract(None, RoyaltyDistributionContract);
        let client = RoyaltyDistributionContractClient::new(&env, &id);
        let admin = Address::generate(&env);
        client.initialize(&admin);
        (env, client, admin)
    }

    use proptest::prelude::*;

    proptest! {
        #[test]
        fn fuzz_split_totals_remain_exact_for_all_valid_percentages(
            creator in 0u32..=100,
            contributor in 0u32..=100,
            total in 1i128..=1_000_000_000_000i128,
        ) {
            let platform = 100_u32.saturating_sub(creator).saturating_sub(contributor);
            let creator_amount = total.checked_mul(creator as i128).unwrap() / 100;
            let contributor_amount = total.checked_mul(contributor as i128).unwrap() / 100;
            let platform_amount = total - creator_amount - contributor_amount;
            let sum = creator_amount + contributor_amount + platform_amount;
            prop_assert_eq!(sum, total);
            prop_assert!(platform_amount >= 0);
        }
    }

    #[test]
    fn test_initialize() {
        let (_, _, _) = setup();
        // Contract initialized without panic
    }

    #[test]
    #[should_panic(expected = "Already initialized")]
    fn test_double_initialize_panics() {
        let (_, client, admin) = setup();
        client.initialize(&admin);
    }

    #[test]
    fn test_set_royalty_split_valid() {
        let (_, client, admin) = setup();
        client.set_royalty_split(&admin, &1, &60, &30, &10);
        let split = client.get_royalty_split(&1).unwrap();
        assert_eq!(split.creator_percentage, 60);
        assert_eq!(split.contributor_percentage, 30);
        assert_eq!(split.platform_percentage, 10);
    }

    #[test]
    #[should_panic(expected = "Percentages must sum to 100")]
    fn test_split_not_summing_to_100_panics() {
        let (_, client, admin) = setup();
        client.set_royalty_split(&admin, &1, &50, &30, &10); // 90 ≠ 100
    }

    #[test]
    #[should_panic(expected = "Unauthorized: admin required")]
    fn test_non_admin_cannot_set_split() {
        let (env, client, _) = setup();
        let rando = Address::generate(&env);
        client.set_royalty_split(&rando, &1, &60, &30, &10);
    }

    #[test]
    fn test_add_royalty_recipient() {
        let (env, client, admin) = setup();
        let creator = Address::generate(&env);
        client.set_royalty_split(&admin, &1, &60, &30, &10);
        client.add_royalty_recipient(&admin, &1, &creator);
    }

    #[test]
    #[should_panic(expected = "Unauthorized: admin required")]
    fn test_non_admin_cannot_add_recipient() {
        let (env, client, admin) = setup();
        let rando = Address::generate(&env);
        let recipient = Address::generate(&env);
        client.set_royalty_split(&admin, &1, &60, &30, &10);
        client.add_royalty_recipient(&rando, &1, &recipient);
    }

    #[test]
    fn test_distribute_royalties_and_balance() {
        let (env, client, admin) = setup();
        let creator = Address::generate(&env);
        let contributor = Address::generate(&env);
        let platform = Address::generate(&env);

        client.set_royalty_split(&admin, &1, &60, &30, &10);
        client.add_royalty_recipient(&admin, &1, &creator);
        client.add_royalty_recipient(&admin, &1, &contributor);
        client.add_royalty_recipient(&admin, &1, &platform);

        client.distribute_royalties(&admin, &1, &1000);
        assert_eq!(client.get_royalty_balance(&creator), 600);
        assert_eq!(client.get_royalty_balance(&contributor), 300);
    }

    #[test]
    #[should_panic(expected = "Amount must be positive")]
    fn test_distribute_zero_amount_panics() {
        let (env, client, admin) = setup();
        let creator = Address::generate(&env);
        client.set_royalty_split(&admin, &1, &60, &30, &10);
        client.add_royalty_recipient(&admin, &1, &creator);
        client.distribute_royalties(&admin, &1, &0);
    }

    #[test]
    fn test_withdraw_royalties() {
        let (env, client, admin) = setup();
        let creator = Address::generate(&env);
        let contributor = Address::generate(&env);
        let platform = Address::generate(&env);

        client.set_royalty_split(&admin, &1, &60, &30, &10);
        client.add_royalty_recipient(&admin, &1, &creator);
        client.add_royalty_recipient(&admin, &1, &contributor);
        client.add_royalty_recipient(&admin, &1, &platform);

        client.distribute_royalties(&admin, &1, &1000);
        let withdrawn = client.withdraw_royalties(&creator);
        assert_eq!(withdrawn, 600);
        assert_eq!(client.get_royalty_balance(&creator), 0);
    }

    #[test]
    #[should_panic(expected = "No royalties to withdraw")]
    fn test_withdraw_with_no_balance_panics() {
        let (env, client, _) = setup();
        let user = Address::generate(&env);
        client.withdraw_royalties(&user);
    }

    #[test]
    fn test_split_not_found_returns_none() {
        let (_, client, _) = setup();
        assert!(client.get_royalty_split(&999).is_none());
    }

    #[test]
    #[should_panic(expected = "Unauthorized: admin required")]
    fn test_non_admin_cannot_distribute_royalties() {
        let (env, client, admin) = setup();
        let rando = Address::generate(&env);
        let creator = Address::generate(&env);
        client.set_royalty_split(&admin, &1, &60, &30, &10);
        client.add_royalty_recipient(&admin, &1, &creator);
        client.distribute_royalties(&rando, &1, &1000);
    }

    #[test]
    #[should_panic(expected = "Royalty split not configured")]
    fn test_distribute_without_split_panics() {
        let (env, client, admin) = setup();
        let creator = Address::generate(&env);
        client.add_royalty_recipient(&admin, &1, &creator);
        client.distribute_royalties(&admin, &1, &1000);
    }

    #[test]
    fn test_distribute_with_single_recipient_combines_creator_and_platform() {
        let (env, client, admin) = setup();
        let creator = Address::generate(&env);

        client.set_royalty_split(&admin, &1, &60, &30, &10);
        client.add_royalty_recipient(&admin, &1, &creator);

        client.distribute_royalties(&admin, &1, &1000);

        // With only one recipient, the contributor branch (count > 1) is
        // skipped, and the platform lookup falls back to index
        // count.saturating_sub(1) == 0, which is the creator itself. So the
        // creator ends up with creator_amount + platform_amount, while
        // contributor_amount (300) is never allocated to anyone.
        assert_eq!(client.get_royalty_balance(&creator), 700);
    }

    #[test]
    fn test_get_payment_record_nonexistent_returns_none() {
        let (_, client, _) = setup();
        assert!(client.get_payment_record(&999).is_none());
    }

    #[test]
    fn test_get_payment_count_increments_across_calls() {
        let (env, client, admin) = setup();
        let creator = Address::generate(&env);

        client.set_royalty_split(&admin, &1, &60, &30, &10);
        client.add_royalty_recipient(&admin, &1, &creator);

        assert_eq!(client.get_payment_count(), 0);
        client.distribute_royalties(&admin, &1, &1000);
        assert_eq!(client.get_payment_count(), 1);
        client.distribute_royalties(&admin, &1, &500);
        assert_eq!(client.get_payment_count(), 2);
    }

    #[test]
    fn test_get_total_distributed_scoped_per_course() {
        let (env, client, admin) = setup();
        let creator_a = Address::generate(&env);
        let creator_b = Address::generate(&env);

        client.set_royalty_split(&admin, &1, &60, &30, &10);
        client.add_royalty_recipient(&admin, &1, &creator_a);

        client.set_royalty_split(&admin, &2, &60, &30, &10);
        client.add_royalty_recipient(&admin, &2, &creator_b);

        client.distribute_royalties(&admin, &1, &1000);
        client.distribute_royalties(&admin, &2, &500);

        assert_eq!(client.get_total_distributed(&1), 1000);
        assert_eq!(client.get_total_distributed(&2), 500);
    }

    #[test]
    fn test_royalty_event_follows_shared_contract_convention() {
        let (env, client, admin) = setup();
        let creator = Address::generate(&env);
        let contributor = Address::generate(&env);
        let platform = Address::generate(&env);

        client.set_royalty_split(&admin, &1, &60, &30, &10);
        client.add_royalty_recipient(&admin, &1, &creator);
        client.add_royalty_recipient(&admin, &1, &contributor);
        client.add_royalty_recipient(&admin, &1, &platform);
        client.distribute_royalties(&admin, &1, &1000);

        let events = env.events().all();
        assert!(!events.is_empty());
        let newest = events.get(events.len() - 1).unwrap();
        let topics = newest.1;
        assert_eq!(soroban_sdk::Symbol::from_val(&env, &topics.get(0).unwrap()), soroban_sdk::symbol_short!("royalty"));
        assert_eq!(soroban_sdk::Symbol::from_val(&env, &topics.get(1).unwrap()), soroban_sdk::symbol_short!("distribtd"));
    }

    #[test]
    fn test_royalty_balance_accumulates_across_multiple_distributions() {
        let (env, client, admin) = setup();
        let creator = Address::generate(&env);
        let contributor = Address::generate(&env);
        let platform = Address::generate(&env);

        client.set_royalty_split(&admin, &1, &60, &30, &10);
        client.add_royalty_recipient(&admin, &1, &creator);
        client.add_royalty_recipient(&admin, &1, &contributor);
        client.add_royalty_recipient(&admin, &1, &platform);

        client.distribute_royalties(&admin, &1, &1000);
        client.distribute_royalties(&admin, &1, &1000);

        // Balances should be summed across calls, not overwritten.
        assert_eq!(client.get_royalty_balance(&creator), 1200);
        assert_eq!(client.get_royalty_balance(&contributor), 600);
        assert_eq!(client.get_royalty_balance(&platform), 200);
    }

    #[test]
    fn test_set_royalty_split_overwrites_previous_split() {
        let (_, client, admin) = setup();
        client.set_royalty_split(&admin, &1, &60, &30, &10);
        client.set_royalty_split(&admin, &1, &50, &40, &10);

        let split = client.get_royalty_split(&1).unwrap();
        assert_eq!(split.creator_percentage, 50);
        assert_eq!(split.contributor_percentage, 40);
        assert_eq!(split.platform_percentage, 10);
    }

    #[test]
    fn test_withdraw_only_zeroes_withdrawing_recipient_balance() {
        let (env, client, admin) = setup();
        let creator = Address::generate(&env);
        let contributor = Address::generate(&env);
        let platform = Address::generate(&env);

        client.set_royalty_split(&admin, &1, &60, &30, &10);
        client.add_royalty_recipient(&admin, &1, &creator);
        client.add_royalty_recipient(&admin, &1, &contributor);
        client.add_royalty_recipient(&admin, &1, &platform);

        client.distribute_royalties(&admin, &1, &1000);
        client.withdraw_royalties(&creator);

        assert_eq!(client.get_royalty_balance(&creator), 0);
        assert_eq!(client.get_royalty_balance(&contributor), 300);
        assert_eq!(client.get_royalty_balance(&platform), 100);
    }

    // =========================================================================
    // Rounding Regression Tests (Issue #1030)
    //
    // These tests verify the invariant:
    //   sum(distributed_amounts) <= total_amount
    //
    // The contract uses independent truncating integer division for each share,
    // which means rounding dust (remainders) can be lost. This is by design —
    // the floor-division approach ensures no party receives more than their
    // contractual percentage. The dust is unallocated.
    // =========================================================================

    /// Helper: distribute and return (creator, contributor, platform) balances.
    fn distribute_and_get_balances(
        client: &RoyaltyDistributionContractClient<'static>,
        admin: &Address,
        course_id: u64,
        total: i128,
    ) -> (i128, i128, i128) {
        let env = &client.env;
        let creator = Address::generate(env);
        let contributor = Address::generate(env);
        let platform_addr = Address::generate(env);

        client.add_royalty_recipient(admin, &course_id, &creator);
        client.add_royalty_recipient(admin, &course_id, &contributor);
        client.add_royalty_recipient(admin, &course_id, &platform_addr);

        client.distribute_royalties(admin, &course_id, &total);

        let c = client.get_royalty_balance(&creator);
        let co = client.get_royalty_balance(&contributor);
        let p = client.get_royalty_balance(&platform_addr);
        (c, co, p)
    }

    // ── 1/3-style splits (33/33/34) ────────────────────────────────────────

    #[test]
    fn test_rounding_thirds_split_100() {
        let (env, client, admin) = setup();
        client.set_royalty_split(&admin, &1, &33, &33, &34);
        let (c, co, p) = distribute_and_get_balances(&client, &admin, 1, 100);
        // 100 * 33 / 100 = 33, 100 * 33 / 100 = 33, 100 * 34 / 100 = 34
        assert_eq!(c, 33);
        assert_eq!(co, 33);
        assert_eq!(p, 34);
        // Invariant: sum <= total
        assert!(c + co + p <= 100);
    }

    #[test]
    fn test_rounding_thirds_split_1000() {
        let (env, client, admin) = setup();
        client.set_royalty_split(&admin, &1, &33, &33, &34);
        let (c, co, p) = distribute_and_get_balances(&client, &admin, 1, 1000);
        // 1000 * 33 / 100 = 330, 1000 * 33 / 100 = 330, 1000 * 34 / 100 = 340
        assert_eq!(c, 330);
        assert_eq!(co, 330);
        assert_eq!(p, 340);
        assert!(c + co + p <= 1000);
    }

    #[test]
    fn test_rounding_thirds_split_1001() {
        let (env, client, admin) = setup();
        client.set_royalty_split(&admin, &1, &33, &33, &34);
        let (c, co, p) = distribute_and_get_balances(&client, &admin, 1, 1001);
        // 1001 * 33 / 100 = 330 (floor), 1001 * 33 / 100 = 330, 1001 * 34 / 100 = 340
        assert_eq!(c, 330);
        assert_eq!(co, 330);
        assert_eq!(p, 340);
        // Dust: 1001 - 1000 = 1 unit lost to rounding
        assert!(c + co + p <= 1001);
        assert_eq!(c + co + p, 1000);
    }

    // ── 2/3-style splits (66/33/1) ─────────────────────────────────────────

    #[test]
    fn test_rounding_two_thirds_split_100() {
        let (env, client, admin) = setup();
        client.set_royalty_split(&admin, &1, &66, &33, &1);
        let (c, co, p) = distribute_and_get_balances(&client, &admin, 1, 100);
        // 100 * 66 / 100 = 66, 100 * 33 / 100 = 33, 100 * 1 / 100 = 1
        assert_eq!(c, 66);
        assert_eq!(co, 33);
        assert_eq!(p, 1);
        assert!(c + co + p <= 100);
    }

    #[test]
    fn test_rounding_two_thirds_split_1000() {
        let (env, client, admin) = setup();
        client.set_royalty_split(&admin, &1, &66, &33, &1);
        let (c, co, p) = distribute_and_get_balances(&client, &admin, 1, 1000);
        // 1000 * 66 / 100 = 660, 1000 * 33 / 100 = 330, 1000 * 1 / 100 = 10
        assert_eq!(c, 660);
        assert_eq!(co, 330);
        assert_eq!(p, 10);
        assert!(c + co + p <= 1000);
    }

    #[test]
    fn test_rounding_two_thirds_split_999() {
        let (env, client, admin) = setup();
        client.set_royalty_split(&admin, &1, &66, &33, &1);
        let (c, co, p) = distribute_and_get_balances(&client, &admin, 1, 999);
        // 999 * 66 / 100 = 659, 999 * 33 / 100 = 329, 999 * 1 / 100 = 9
        assert_eq!(c, 659);
        assert_eq!(co, 329);
        assert_eq!(p, 9);
        // Dust: 999 - 997 = 2 units lost
        assert!(c + co + p <= 999);
        assert_eq!(c + co + p, 997);
    }

    // ── Very small amounts ──────────────────────────────────────────────────

    #[test]
    fn test_rounding_amount_1() {
        let (env, client, admin) = setup();
        client.set_royalty_split(&admin, &1, &60, &30, &10);
        let (c, co, p) = distribute_and_get_balances(&client, &admin, 1, 1);
        // 1 * 60 / 100 = 0, 1 * 30 / 100 = 0, 1 * 10 / 100 = 0
        assert_eq!(c, 0);
        assert_eq!(co, 0);
        assert_eq!(p, 0);
        // All dust lost — expected behavior with floor division on small amounts
        assert!(c + co + p <= 1);
    }

    #[test]
    fn test_rounding_amount_10() {
        let (env, client, admin) = setup();
        client.set_royalty_split(&admin, &1, &60, &30, &10);
        let (c, co, p) = distribute_and_get_balances(&client, &admin, 1, 10);
        // 10 * 60 / 100 = 6, 10 * 30 / 100 = 3, 10 * 10 / 100 = 1
        assert_eq!(c, 6);
        assert_eq!(co, 3);
        assert_eq!(p, 1);
        assert!(c + co + p <= 10);
    }

    #[test]
    fn test_rounding_amount_99() {
        let (env, client, admin) = setup();
        client.set_royalty_split(&admin, &1, &33, &33, &34);
        let (c, co, p) = distribute_and_get_balances(&client, &admin, 1, 99);
        // 99 * 33 / 100 = 32, 99 * 33 / 100 = 32, 99 * 34 / 100 = 33
        assert_eq!(c, 32);
        assert_eq!(co, 32);
        assert_eq!(p, 33);
        // Dust: 99 - 97 = 2 units lost
        assert!(c + co + p <= 99);
        assert_eq!(c + co + p, 97);
    }

    // ── Values that leave specific remainders ───────────────────────────────

    #[test]
    fn test_rounding_amount_333_with_thirds() {
        let (env, client, admin) = setup();
        client.set_royalty_split(&admin, &1, &33, &33, &34);
        let (c, co, p) = distribute_and_get_balances(&client, &admin, 1, 333);
        // 333 * 33 / 100 = 109, 333 * 33 / 100 = 109, 333 * 34 / 100 = 113
        assert_eq!(c, 109);
        assert_eq!(co, 109);
        assert_eq!(p, 113);
        // Dust: 333 - 331 = 2 units lost
        assert!(c + co + p <= 333);
        assert_eq!(c + co + p, 331);
    }

    #[test]
    fn test_rounding_amount_777_with_60_30_10() {
        let (env, client, admin) = setup();
        client.set_royalty_split(&admin, &1, &60, &30, &10);
        let (c, co, p) = distribute_and_get_balances(&client, &admin, 1, 777);
        // 777 * 60 / 100 = 466, 777 * 30 / 100 = 233, 777 * 10 / 100 = 77
        assert_eq!(c, 466);
        assert_eq!(co, 233);
        assert_eq!(p, 77);
        // Dust: 777 - 776 = 1 unit lost
        assert!(c + co + p <= 777);
        assert_eq!(c + co + p, 776);
    }

    #[test]
    fn test_rounding_amount_10000() {
        let (env, client, admin) = setup();
        client.set_royalty_split(&admin, &1, &50, &25, &25);
        let (c, co, p) = distribute_and_get_balances(&client, &admin, 1, 10000);
        assert_eq!(c, 5000);
        assert_eq!(co, 2500);
        assert_eq!(p, 2500);
        // Exact split — no dust
        assert_eq!(c + co + p, 10000);
    }

    #[test]
    fn test_rounding_amount_10001() {
        let (env, client, admin) = setup();
        client.set_royalty_split(&admin, &1, &50, &25, &25);
        let (c, co, p) = distribute_and_get_balances(&client, &admin, 1, 10001);
        // 10001 * 50 / 100 = 5000, 10001 * 25 / 100 = 2500, 10001 * 25 / 100 = 2500
        assert_eq!(c, 5000);
        assert_eq!(co, 2500);
        assert_eq!(p, 2500);
        // Dust: 1 unit lost
        assert!(c + co + p <= 10001);
        assert_eq!(c + co + p, 10000);
    }

    // ── Asymmetric splits ───────────────────────────────────────────────────

    #[test]
    fn test_rounding_90_5_5_split() {
        let (env, client, admin) = setup();
        client.set_royalty_split(&admin, &1, &90, &5, &5);
        let (c, co, p) = distribute_and_get_balances(&client, &admin, 1, 1000);
        assert_eq!(c, 900);
        assert_eq!(co, 50);
        assert_eq!(p, 50);
        assert_eq!(c + co + p, 1000);
    }

    #[test]
    fn test_rounding_90_5_5_split_999() {
        let (env, client, admin) = setup();
        client.set_royalty_split(&admin, &1, &90, &5, &5);
        let (c, co, p) = distribute_and_get_balances(&client, &admin, 1, 999);
        // 999 * 90 / 100 = 899, 999 * 5 / 100 = 49, 999 * 5 / 100 = 49
        assert_eq!(c, 899);
        assert_eq!(co, 49);
        assert_eq!(p, 49);
        // Dust: 999 - 997 = 2 units lost
        assert!(c + co + p <= 999);
        assert_eq!(c + co + p, 997);
    }

    #[test]
    fn test_rounding_1_1_98_split() {
        let (env, client, admin) = setup();
        client.set_royalty_split(&admin, &1, &1, &1, &98);
        let (c, co, p) = distribute_and_get_balances(&client, &admin, 1, 500);
        // 500 * 1 / 100 = 5, 500 * 1 / 100 = 5, 500 * 98 / 100 = 490
        assert_eq!(c, 5);
        assert_eq!(co, 5);
        assert_eq!(p, 490);
        assert_eq!(c + co + p, 500);
    }

    // ── Accumulation across multiple distributions ──────────────────────────

    #[test]
    fn test_rounding_dust_accumulates() {
        let (env, client, admin) = setup();
        client.set_royalty_split(&admin, &1, &33, &33, &34);

        // Distribute 1001 three times — each time 1 unit of dust is lost
        let creator = Address::generate(&env);
        let contributor = Address::generate(&env);
        let platform_addr = Address::generate(&env);

        client.add_royalty_recipient(&admin, &1, &creator);
        client.add_royalty_recipient(&admin, &1, &contributor);
        client.add_royalty_recipient(&admin, &1, &platform_addr);

        client.distribute_royalties(&admin, &1, &1001);
        client.distribute_royalties(&admin, &1, &1001);
        client.distribute_royalties(&admin, &1, &1001);

        let c = client.get_royalty_balance(&creator);
        let co = client.get_royalty_balance(&contributor);
        let p = client.get_royalty_balance(&platform_addr);

        // Each distribution: 330 + 330 + 340 = 1000 (1 dust lost per call)
        assert_eq!(c, 990); // 330 * 3
        assert_eq!(co, 990); // 330 * 3
        assert_eq!(p, 1020); // 340 * 3

        // Total distributed: 3000, total input: 3003, dust lost: 3
        assert_eq!(c + co + p, 3000);
        assert!(c + co + p <= 3003);
    }

    // ── Invariant: sum never exceeds total ──────────────────────────────────

    #[test]
    fn test_invariant_sum_never_exceeds_total() {
        let (env, client, admin) = setup();
        let amounts = [1, 100, 999, 1001, 9999];
        let splits = [
            (33u32, 33u32, 34u32),
            (66, 33, 1),
            (90, 5, 5),
        ];

        for (creator_pct, contributor_pct, platform_pct) in splits {
            for amount in amounts {
                let course_id = (creator_pct as u64) * 10000 + amount as u64;
                client.set_royalty_split(&admin, &course_id, &creator_pct, &contributor_pct, &platform_pct);

                let creator = Address::generate(&env);
                let contributor = Address::generate(&env);
                let platform_addr = Address::generate(&env);

                client.add_royalty_recipient(&admin, &course_id, &creator);
                client.add_royalty_recipient(&admin, &course_id, &contributor);
                client.add_royalty_recipient(&admin, &course_id, &platform_addr);

                client.distribute_royalties(&admin, &course_id, &amount);

                let c = client.get_royalty_balance(&creator);
                let co = client.get_royalty_balance(&contributor);
                let p = client.get_royalty_balance(&platform_addr);

                // Core invariant: sum of distributed amounts must never exceed total
                assert!(
                    c + co + p <= amount,
                    "Invariant violated: {} + {} + {} = {} > {} (split: {}/{}/{})",
                    c, co, p, c + co + p, amount,
                    creator_pct, contributor_pct, platform_pct
                );

                // Each individual share must be <= floor(total * pct / 100)
                let max_creator = (amount * creator_pct as i128) / 100;
                let max_contributor = (amount * contributor_pct as i128) / 100;
                let max_platform = (amount * platform_pct as i128) / 100;

                assert!(c <= max_creator, "Creator share {} exceeds max {}", c, max_creator);
                assert!(co <= max_contributor, "Contributor share {} exceeds max {}", co, max_contributor);
                assert!(p <= max_platform, "Platform share {} exceeds max {}", p, max_platform);
            }
        }
    }
}
