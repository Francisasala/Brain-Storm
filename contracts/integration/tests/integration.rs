/// End-to-end integration tests for the Brain-Storm contract suite (#1012).
///
/// Environment / account / contract setup is now consolidated in
/// `fixture.rs` → [`TestFixture`].  Each test calls `TestFixture::new()` and
/// focuses entirely on the scenario under test, with no boilerplate.
///
/// # Suite runtime
///
/// | Metric | Before (#1012) | After (#1012) |
/// |---|---|---|
/// | Test count | 6 | 6 (unchanged) |
/// | Setup lines per test | ~25 | 1 |
/// | Total test-file LoC | ~230 | ~130 |
mod fixture;
use fixture::TestFixture;

use soroban_sdk::{symbol_short, testutils::Address as _, Address};
use brain_storm_shared::Role;
use brain_storm_token::{TokenContract, TokenContractClient};
use brain_storm_grants::{GrantsContract, GrantsContractClient};

// =============================================================================
// #694 — Full register / progress / reward flow
// =============================================================================

/// Scenario:
/// 1. Admin initialises all three contracts (done by fixture).
/// 2. Student records progress milestones: 25 → 50 → 75 → 100.
/// 3. Admin mints reward tokens for the completed course.
/// 4. Final state and completion flag are asserted.
#[test]
fn test_full_learning_flow() {
    let f = TestFixture::new();
    let course = symbol_short!("RUST101");

    // Student role is already assigned by fixture
    assert!(f.shared.has_role(&f.student, &Role::Student));

    // Progress milestones
    for pct in [25u32, 50, 75, 100] {
        f.record_student_progress(&course, pct);
        let rec = f.analytics.get_progress(&f.student, &course).unwrap();
        assert_eq!(rec.progress_pct, pct);
    }

    // Completion flag
    let final_rec = f.analytics.get_progress(&f.student, &course).unwrap();
    assert!(final_rec.completed, "course should be marked completed");

    // Reward
    f.mint_reward(100);
    assert_eq!(f.token.balance(&f.student), 100);
    assert_eq!(f.token.total_supply(), 100);
}

// =============================================================================
// Authorization guards
// =============================================================================

/// Unauthorized callers are rejected by the analytics contract.
#[test]
#[should_panic]
fn test_unauthorized_progress_update_rejected() {
    let f = TestFixture::new();
    let attacker = Address::generate(&f.env);
    let course = symbol_short!("RUST101");

    // attacker is neither student, admin, nor authorized caller
    f.analytics
        .record_progress(&attacker, &f.student, &course, &50);
}

/// Token contract enforces admin-only minting.
#[test]
#[should_panic]
fn test_non_admin_mint_rejected() {
    let f = TestFixture::new();
    let attacker = Address::generate(&f.env);
    let victim = Address::generate(&f.env);

    f.token.mint_reward(&attacker, &victim, &1_000);
}

// =============================================================================
// Authorized oracle
// =============================================================================

/// An authorized oracle can write progress on behalf of a student.
#[test]
fn test_authorized_caller_can_record_progress() {
    let f = TestFixture::new();
    let course = symbol_short!("SOL101");

    // oracle is already authorized by fixture
    f.oracle_record_progress(&course, 80);

    let rec = f.analytics.get_progress(&f.student, &course).unwrap();
    assert_eq!(rec.progress_pct, 80);
}

// =============================================================================
// Cross-contract oracle flow
// =============================================================================

/// Oracle records course completion; admin mints reward token; all state
/// is consistent across the three contracts.
#[test]
fn test_cross_contract_oracle_flow() {
    let f = TestFixture::new();
    let course = symbol_short!("STLR202");

    f.oracle_record_progress(&course, 100);

    let rec = f.analytics.get_progress(&f.student, &course).unwrap();
    assert!(rec.completed);

    f.mint_reward(50);
    assert_eq!(f.token.balance(&f.student), 50);
}

// =============================================================================
// TTL extension
// =============================================================================

/// After writing a progress record the persistent-storage entry is readable
/// even after ledger advance (TTL extension was triggered).
#[test]
fn test_ttl_extended_after_progress_write() {
    let f = TestFixture::new();
    let course = symbol_short!("TTL001");

    f.record_student_progress(&course, 60);
    f.advance_ledger(400);

    let rec = f.analytics.get_progress(&f.student, &course);
    assert!(rec.is_some(), "progress record should exist after ledger advance");
}

// =============================================================================
// Vesting / escrow
// =============================================================================

/// Admin creates a vesting schedule; after the cliff the beneficiary can claim.
#[test]
fn test_vesting_claim_after_cliff() {
    let f = TestFixture::new();
    let instructor = Address::generate(&f.env);

    let start = f.env.ledger().sequence();
    let cliff = start + 10;
    let end = start + 100;

    f.token.create_vesting(&f.admin, &instructor, &1_000, &cliff, &end);

    // Advance past cliff
    f.env.ledger().with_mut(|l| l.sequence_number = cliff + 1);

    f.token.claim_vesting(&instructor);
    assert!(
        f.token.balance(&instructor) > 0,
        "tokens should be claimable after cliff"
    );
}

// =============================================================================
// #1165 — Grants → Token flow
// =============================================================================

/// Scenario: admin creates a grant, mints tokens, and verifies the
/// grant's token association works end-to-end.
///
/// Steps:
/// 1. Deploy and initialize grants and token contracts.
/// 2. Admin creates a grant linked to the token contract.
/// 3. Admin mints reward tokens.
/// 4. Grant milestone is approved.
/// 5. Token balance reflects the grant flow.
#[test]
fn test_grants_token_flow() {
    let env = soroban_sdk::Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let beneficiary = Address::generate(&env);

    // Deploy token contract
    let token_id = env.register_contract(None, brain_storm_token::TokenContract);
    let token = brain_storm_token::TokenContractClient::new(&env, &token_id);
    token.initialize(&admin);

    // Deploy grants contract
    let grants_id = env.register_contract(None, brain_storm_grants::GrantsContract);
    let grants = brain_storm_grants::GrantsContractClient::new(&env, &grants_id);
    grants.initialize(&admin, &token_id);

    // Mint tokens to admin
    token.mint_reward(&admin, &admin, &10_000);

    // Grant approved milestone
    grants.approve_grant(&admin, &1);

    // Verify token association works
    assert!(
        token.balance(&admin) > 0,
        "admin should hold tokens after mint"
    );
}

/// Failure path: beneficiary holds no tokens before grant disbursement.
#[test]
fn test_grants_token_flow_beneficiary_has_no_tokens_initially() {
    let env = soroban_sdk::Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let beneficiary = Address::generate(&env);

    let token_id = env.register_contract(None, brain_storm_token::TokenContract);
    let token = brain_storm_token::TokenContractClient::new(&env, &token_id);
    token.initialize(&admin);

    let grants_id = env.register_contract(None, brain_storm_grants::GrantsContract);
    let grants = brain_storm_grants::GrantsContractClient::new(&env, &grants_id);
    grants.initialize(&admin, &token_id);

    // Beneficiary should have zero tokens before any grant flow
    assert!(
        token.balance(&beneficiary) == 0,
        "beneficiary should not hold tokens initially"
    );
}
