//! #862 — Unit tests for registry contract **lookup** functions.
//!
//! These tests target the read-path functions that were not adequately
//! covered by the existing `test.rs`:
//!
//! * `get_certified_skills`  — expiry boundary conditions & multi-skill isolation
//! * `has_certified_skill`   — after add / remove / expiry
//! * `get_verification_level`— default + all explicit levels
//! * `get_specialisations`   — empty default, overwrite semantics
//! * `list_users`            — zero-limit, exact-fit, last-page edge cases
//! * `list_users_by_level`   — Unverified boundary, level downgrade visibility
//! * `total_users`           — after batch vs individual registration
//! * `get_admin`             — post-initialization read

#[cfg(test)]
mod lookup_tests {
    use crate::{RegistryContract, RegistryContractClient, VerificationLevel};
    use soroban_sdk::{
        symbol_short,
        testutils::{Address as _, Ledger},
        vec, Address, Env, Vec,
    };

    // ─────────────────────────────────────────────────────────────────────────
    // Helpers
    // ─────────────────────────────────────────────────────────────────────────

    fn setup() -> (Env, RegistryContractClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let id = env.register_contract(None, RegistryContract);
        let client = RegistryContractClient::new(&env, &id);
        let admin = Address::generate(&env);
        client.initialize(&admin);
        (env, client, admin)
    }

    // ─────────────────────────────────────────────────────────────────────────
    // get_admin
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn lookup_get_admin_returns_initialised_admin() {
        let (_, client, admin) = setup();
        assert_eq!(client.get_admin(), admin);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // get_verification_level — all variant returns
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn lookup_default_level_is_unverified_for_unknown_address() {
        let (env, client, _) = setup();
        let stranger = Address::generate(&env);
        assert_eq!(
            client.get_verification_level(&stranger),
            VerificationLevel::Unverified
        );
    }

    #[test]
    fn lookup_all_verification_levels_round_trip() {
        let (env, client, admin) = setup();

        let levels = [
            VerificationLevel::Unverified,
            VerificationLevel::Basic,
            VerificationLevel::Advanced,
            VerificationLevel::Expert,
        ];

        for level in &levels {
            let user = Address::generate(&env);
            client.set_verification_level(&admin, &user, level);
            assert_eq!(client.get_verification_level(&user), *level);
        }
    }

    #[test]
    fn lookup_verification_level_overwrite_returns_new_level() {
        let (env, client, admin) = setup();
        let user = Address::generate(&env);

        client.set_verification_level(&admin, &user, &VerificationLevel::Basic);
        assert_eq!(client.get_verification_level(&user), VerificationLevel::Basic);

        client.set_verification_level(&admin, &user, &VerificationLevel::Expert);
        assert_eq!(client.get_verification_level(&user), VerificationLevel::Expert);
    }

    #[test]
    fn lookup_level_independent_per_user() {
        let (env, client, admin) = setup();
        let u1 = Address::generate(&env);
        let u2 = Address::generate(&env);

        client.set_verification_level(&admin, &u1, &VerificationLevel::Basic);
        // u2 never touched — must still be Unverified
        assert_eq!(
            client.get_verification_level(&u2),
            VerificationLevel::Unverified
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // get_certified_skills — expiry boundary conditions
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn lookup_skills_empty_for_new_user() {
        let (env, client, _) = setup();
        let user = Address::generate(&env);
        assert_eq!(client.get_certified_skills(&user).len(), 0);
    }

    #[test]
    fn lookup_skill_with_zero_expiry_never_expires() {
        let (env, client, admin) = setup();
        let user = Address::generate(&env);
        let skill = symbol_short!("rust");
        // expiry=0 means permanent
        client.add_certified_skill(&admin, &user, &skill, &0);
        // Advance ledger to maximum sane timestamp
        env.ledger().with_mut(|l| l.timestamp = u64::MAX / 2);
        let skills = client.get_certified_skills(&user);
        assert_eq!(skills.len(), 1);
        assert_eq!(skills.get(0).unwrap(), skill);
    }

    #[test]
    fn lookup_skill_exactly_at_expiry_is_expired() {
        // When `now == expiry` the condition is `now < expiry` which is false → expired.
        let (env, client, admin) = setup();
        let user = Address::generate(&env);
        let skill = symbol_short!("sol");
        let expiry: u64 = 500;
        client.add_certified_skill(&admin, &user, &skill, &expiry);
        // Set ledger time exactly to expiry value
        env.ledger().with_mut(|l| l.timestamp = expiry);
        assert_eq!(client.get_certified_skills(&user).len(), 0);
    }

    #[test]
    fn lookup_skill_one_second_before_expiry_is_valid() {
        let (env, client, admin) = setup();
        let user = Address::generate(&env);
        let skill = symbol_short!("sol");
        let expiry: u64 = 500;
        client.add_certified_skill(&admin, &user, &skill, &expiry);
        env.ledger().with_mut(|l| l.timestamp = expiry - 1);
        let skills = client.get_certified_skills(&user);
        assert_eq!(skills.len(), 1);
    }

    #[test]
    fn lookup_mixed_expired_and_valid_skills_returns_only_valid() {
        let (env, client, admin) = setup();
        let user = Address::generate(&env);
        let expired_skill = symbol_short!("old");
        let valid_skill = symbol_short!("new");
        let permanent_skill = symbol_short!("perm");

        client.add_certified_skill(&admin, &user, &expired_skill, &100);
        client.add_certified_skill(&admin, &user, &valid_skill, &9_999_999);
        client.add_certified_skill(&admin, &user, &permanent_skill, &0);

        // Advance past `expired_skill` expiry but well before `valid_skill`
        env.ledger().with_mut(|l| l.timestamp = 200);

        let skills = client.get_certified_skills(&user);
        assert_eq!(skills.len(), 2, "expected valid + permanent, got {}", skills.len());
        assert!(skills.iter().any(|s| s == valid_skill));
        assert!(skills.iter().any(|s| s == permanent_skill));
    }

    #[test]
    fn lookup_all_skills_expired_returns_empty_list() {
        let (env, client, admin) = setup();
        let user = Address::generate(&env);
        client.add_certified_skill(&admin, &user, &symbol_short!("a"), &10);
        client.add_certified_skill(&admin, &user, &symbol_short!("b"), &20);
        env.ledger().with_mut(|l| l.timestamp = 100);
        assert_eq!(client.get_certified_skills(&user).len(), 0);
    }

    #[test]
    fn lookup_skills_isolated_between_users() {
        let (env, client, admin) = setup();
        let u1 = Address::generate(&env);
        let u2 = Address::generate(&env);
        let skill = symbol_short!("rust");
        client.add_certified_skill(&admin, &u1, &skill, &0);
        // u2 should not see u1's skill
        assert_eq!(client.get_certified_skills(&u2).len(), 0);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // has_certified_skill — post-add / post-remove / post-expiry
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn lookup_has_skill_true_after_add() {
        let (env, client, admin) = setup();
        let user = Address::generate(&env);
        let skill = symbol_short!("rust");
        client.add_certified_skill(&admin, &user, &skill, &0);
        assert!(client.has_certified_skill(&user, &skill));
    }

    #[test]
    fn lookup_has_skill_false_after_remove() {
        let (env, client, admin) = setup();
        let user = Address::generate(&env);
        let skill = symbol_short!("rust");
        client.add_certified_skill(&admin, &user, &skill, &0);
        client.remove_certified_skill(&admin, &user, &skill);
        assert!(!client.has_certified_skill(&user, &skill));
    }

    #[test]
    fn lookup_has_skill_false_after_expiry() {
        let (env, client, admin) = setup();
        let user = Address::generate(&env);
        let skill = symbol_short!("rust");
        client.add_certified_skill(&admin, &user, &skill, &50);
        env.ledger().with_mut(|l| l.timestamp = 100);
        assert!(!client.has_certified_skill(&user, &skill));
    }

    #[test]
    fn lookup_has_skill_false_for_unknown_skill() {
        let (env, client, _) = setup();
        let user = Address::generate(&env);
        let skill = symbol_short!("ghost");
        assert!(!client.has_certified_skill(&user, &skill));
    }

    #[test]
    fn lookup_has_skill_checks_per_skill_not_all_skills() {
        let (env, client, admin) = setup();
        let user = Address::generate(&env);
        let s1 = symbol_short!("rust");
        let s2 = symbol_short!("python");
        client.add_certified_skill(&admin, &user, &s1, &0);
        // s2 never added
        assert!(client.has_certified_skill(&user, &s1));
        assert!(!client.has_certified_skill(&user, &s2));
    }

    #[test]
    fn lookup_remove_nonexistent_skill_leaves_others_intact() {
        let (env, client, admin) = setup();
        let user = Address::generate(&env);
        let kept = symbol_short!("rust");
        let ghost = symbol_short!("ghost");
        client.add_certified_skill(&admin, &user, &kept, &0);
        // Removing a skill that doesn't exist should be a no-op
        client.remove_certified_skill(&admin, &user, &ghost);
        assert!(client.has_certified_skill(&user, &kept));
    }

    // ─────────────────────────────────────────────────────────────────────────
    // get_specialisations — empty default + overwrite semantics
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn lookup_specialisations_empty_for_new_user() {
        let (env, client, _) = setup();
        let user = Address::generate(&env);
        assert_eq!(client.get_specialisations(&user).len(), 0);
    }

    #[test]
    fn lookup_specialisations_returns_all_set_values() {
        let (env, client, admin) = setup();
        let user = Address::generate(&env);
        let mut specs = Vec::new(&env);
        specs.push_back(symbol_short!("defi"));
        specs.push_back(symbol_short!("nft"));
        specs.push_back(symbol_short!("dao"));
        client.set_specialisations(&admin, &user, &specs);
        let returned = client.get_specialisations(&user);
        assert_eq!(returned.len(), 3);
    }

    #[test]
    fn lookup_specialisations_overwrite_replaces_previous() {
        let (env, client, admin) = setup();
        let user = Address::generate(&env);

        let mut first = Vec::new(&env);
        first.push_back(symbol_short!("defi"));
        first.push_back(symbol_short!("nft"));
        client.set_specialisations(&admin, &user, &first);

        let mut second = Vec::new(&env);
        second.push_back(symbol_short!("dao"));
        client.set_specialisations(&admin, &user, &second);

        let result = client.get_specialisations(&user);
        assert_eq!(result.len(), 1);
        assert_eq!(result.get(0).unwrap(), symbol_short!("dao"));
    }

    #[test]
    fn lookup_specialisations_can_be_cleared_with_empty_vec() {
        let (env, client, admin) = setup();
        let user = Address::generate(&env);

        let mut specs = Vec::new(&env);
        specs.push_back(symbol_short!("defi"));
        client.set_specialisations(&admin, &user, &specs);
        assert_eq!(client.get_specialisations(&user).len(), 1);

        // Overwrite with empty
        client.set_specialisations(&admin, &user, &Vec::new(&env));
        assert_eq!(client.get_specialisations(&user).len(), 0);
    }

    #[test]
    fn lookup_specialisations_isolated_per_user() {
        let (env, client, admin) = setup();
        let u1 = Address::generate(&env);
        let u2 = Address::generate(&env);
        let mut specs = Vec::new(&env);
        specs.push_back(symbol_short!("defi"));
        client.set_specialisations(&admin, &u1, &specs);
        assert_eq!(client.get_specialisations(&u2).len(), 0);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // list_users — pagination edge cases
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn lookup_list_users_zero_limit_returns_empty() {
        let (env, client, _) = setup();
        let user = Address::generate(&env);
        client.register_user(&user);
        let page = client.list_users(&0, &0);
        assert_eq!(page.len(), 0);
    }

    // ── Page-size cap (#1168) ─────────────────────────────────────────────────

    #[test]
    fn lookup_get_max_page_size_returns_cap() {
        let (_, client, _) = setup();
        assert_eq!(client.get_max_page_size(), 100);
    }

    #[test]
    fn lookup_list_users_over_limit_is_truncated_to_max_page_size() {
        let (env, client, _) = setup();
        for _ in 0..150 {
            client.register_user(&Address::generate(&env));
        }
        assert_eq!(client.total_users(), 150);

        // Unbounded request → capped at the shared page-size limit.
        let page = client.list_users(&0, &u32::MAX);
        assert_eq!(page.len(), 100);

        // A request just above the cap is truncated to it, not rejected.
        let page = client.list_users(&0, &120);
        assert_eq!(page.len(), 100);

        // Walking on still returns the remainder, also capped.
        let tail = client.list_users(&100, &u32::MAX);
        assert_eq!(tail.len(), 50);
    }

    #[test]
    fn lookup_list_users_by_level_over_limit_is_truncated_to_max_page_size() {
        let (env, client, _) = setup();
        for _ in 0..150 {
            client.register_user(&Address::generate(&env));
        }
        let page = client.list_users_by_level(&VerificationLevel::Unverified, &0, &u32::MAX);
        assert_eq!(page.len(), 100);
    }

    #[test]
    fn lookup_list_users_by_level_zero_limit_returns_empty() {
        let (env, client, _) = setup();
        let user = Address::generate(&env);
        client.register_user(&user);
        let page = client.list_users_by_level(&VerificationLevel::Unverified, &0, &0);
        assert_eq!(page.len(), 0);
    }

    #[test]
    fn lookup_list_users_exact_fit_page() {
        let (env, client, _) = setup();
        for _ in 0..3 {
            client.register_user(&Address::generate(&env));
        }
        // limit exactly matches total
        let page = client.list_users(&0, &3);
        assert_eq!(page.len(), 3);
    }

    #[test]
    fn lookup_list_users_does_not_return_duplicates_on_repeated_register() {
        let (env, client, _) = setup();
        let u = Address::generate(&env);
        client.register_user(&u);
        client.register_user(&u);
        client.register_user(&u);
        assert_eq!(client.total_users(), 1);
        let page = client.list_users(&0, &10);
        assert_eq!(page.len(), 1);
    }

    #[test]
    fn lookup_list_users_preserves_insertion_order() {
        let (env, client, _) = setup();
        let u1 = Address::generate(&env);
        let u2 = Address::generate(&env);
        let u3 = Address::generate(&env);
        client.register_user(&u1);
        client.register_user(&u2);
        client.register_user(&u3);
        let page = client.list_users(&0, &3);
        assert_eq!(page.get(0).unwrap(), u1);
        assert_eq!(page.get(1).unwrap(), u2);
        assert_eq!(page.get(2).unwrap(), u3);
    }

    #[test]
    fn lookup_total_users_consistent_with_batch_register() {
        let (env, client, _) = setup();
        let users = vec![
            &env,
            Address::generate(&env),
            Address::generate(&env),
            Address::generate(&env),
        ];
        client.batch_register_users(&users);
        assert_eq!(client.total_users(), 3);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // list_users_by_level — Unverified boundary + level downgrade
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn lookup_list_by_level_unverified_includes_all_registered_users() {
        // min_level=Unverified (ord=0) — every user qualifies.
        let (env, client, admin) = setup();
        let u1 = Address::generate(&env);
        let u2 = Address::generate(&env);
        client.register_user(&u1);
        client.register_user(&u2);
        client.set_verification_level(&admin, &u1, &VerificationLevel::Expert);
        // u2 stays Unverified
        let result =
            client.list_users_by_level(&VerificationLevel::Unverified, &0, &10);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn lookup_list_by_level_expert_excludes_lower_levels() {
        let (env, client, admin) = setup();
        for _ in 0..3 {
            let u = Address::generate(&env);
            client.register_user(&u);
            client.set_verification_level(&admin, &u, &VerificationLevel::Basic);
        }
        let expert = Address::generate(&env);
        client.register_user(&expert);
        client.set_verification_level(&admin, &expert, &VerificationLevel::Expert);

        let result = client.list_users_by_level(&VerificationLevel::Expert, &0, &10);
        assert_eq!(result.len(), 1);
        assert_eq!(result.get(0).unwrap(), expert);
    }

    #[test]
    fn lookup_list_by_level_unregistered_user_excluded_even_if_leveled() {
        // A user with a level set but NOT registered via `register_user` must NOT
        // appear in list_users_by_level (the list is derived from UserList).
        let (env, client, admin) = setup();
        let unlisted = Address::generate(&env);
        // Give them a high level but never call register_user
        client.set_verification_level(&admin, &unlisted, &VerificationLevel::Expert);
        let result =
            client.list_users_by_level(&VerificationLevel::Expert, &0, &10);
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn lookup_list_by_level_after_level_upgrade_is_reflected() {
        let (env, client, admin) = setup();
        let user = Address::generate(&env);
        client.register_user(&user);
        client.set_verification_level(&admin, &user, &VerificationLevel::Basic);

        let before =
            client.list_users_by_level(&VerificationLevel::Advanced, &0, &10);
        assert_eq!(before.len(), 0);

        client.set_verification_level(&admin, &user, &VerificationLevel::Advanced);
        let after =
            client.list_users_by_level(&VerificationLevel::Advanced, &0, &10);
        assert_eq!(after.len(), 1);
    }

    #[test]
    fn lookup_list_by_level_pagination_across_filtered_set() {
        let (env, client, admin) = setup();
        for _ in 0..6 {
            let u = Address::generate(&env);
            client.register_user(&u);
            client.set_verification_level(&admin, &u, &VerificationLevel::Advanced);
        }
        let p1 = client.list_users_by_level(&VerificationLevel::Advanced, &0, &4);
        let p2 = client.list_users_by_level(&VerificationLevel::Advanced, &4, &4);
        assert_eq!(p1.len(), 4);
        assert_eq!(p2.len(), 2);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // total_users
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn lookup_total_users_is_zero_on_fresh_contract() {
        let (_, client, _) = setup();
        assert_eq!(client.total_users(), 0);
    }

    #[test]
    fn lookup_total_users_increments_with_each_unique_user() {
        let (env, client, _) = setup();
        assert_eq!(client.total_users(), 0);
        client.register_user(&Address::generate(&env));
        assert_eq!(client.total_users(), 1);
        client.register_user(&Address::generate(&env));
        assert_eq!(client.total_users(), 2);
    }

    #[test]
    fn lookup_total_users_not_affected_by_skill_or_level_changes() {
        let (env, client, admin) = setup();
        let user = Address::generate(&env);
        client.register_user(&user);
        assert_eq!(client.total_users(), 1);
        // Mutations to other data must not change the user count
        client.set_verification_level(&admin, &user, &VerificationLevel::Expert);
        client.add_certified_skill(&admin, &user, &symbol_short!("rust"), &0);
        assert_eq!(client.total_users(), 1);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // is_curator / is_paused — state reads after mutations
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn lookup_is_curator_false_before_add_true_after() {
        let (env, client, admin) = setup();
        let curator = Address::generate(&env);
        assert!(!client.is_curator(&curator));
        client.add_curator(&admin, &curator);
        assert!(client.is_curator(&curator));
    }

    #[test]
    fn lookup_is_curator_false_after_removal() {
        let (env, client, admin) = setup();
        let curator = Address::generate(&env);
        client.add_curator(&admin, &curator);
        client.remove_curator(&admin, &curator);
        assert!(!client.is_curator(&curator));
    }

    #[test]
    fn lookup_is_paused_reflects_pause_unpause_cycle() {
        let (_, client, admin) = setup();
        assert!(!client.is_paused());
        client.pause(&admin);
        assert!(client.is_paused());
        client.unpause(&admin);
        assert!(!client.is_paused());
    }
}
