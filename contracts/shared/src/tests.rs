#![cfg(test)]
use soroban_sdk::{testutils::Address as _, Address, Env};

use crate::{Permission, Role, SharedContract, SharedContractClient};

fn setup() -> (Env, Address, SharedContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, SharedContract);
    let client = SharedContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    (env, admin, client)
}

// ── has_role ─────────────────────────────────────────────────────────────────

#[test]
fn test_admin_has_admin_role() {
    let (_, admin, client) = setup();
    assert!(client.has_role(&admin, &Role::Admin));
}

#[test]
fn test_assign_instructor_role() {
    let (env, admin, client) = setup();
    let instructor = Address::generate(&env);
    client.assign_role(&admin, &instructor, &Role::Instructor);
    assert!(client.has_role(&instructor, &Role::Instructor));
}

#[test]
fn test_assign_student_role() {
    let (env, admin, client) = setup();
    let student = Address::generate(&env);
    client.assign_role(&admin, &student, &Role::Student);
    assert!(client.has_role(&student, &Role::Student));
}

#[test]
#[should_panic(expected = "Unauthorized: admin required")]
fn test_non_admin_cannot_assign_role() {
    let (env, _, client) = setup();
    let rando = Address::generate(&env);
    let target = Address::generate(&env);
    client.assign_role(&rando, &target, &Role::Instructor);
}

// ── has_permission — Admin ────────────────────────────────────────────────────

#[test]
fn test_admin_has_all_permissions() {
    let (_, admin, client) = setup();
    assert!(client.has_permission(&admin, &Permission::CreateCourse));
    assert!(client.has_permission(&admin, &Permission::EnrollStudent));
    assert!(client.has_permission(&admin, &Permission::IssueCredential));
    assert!(client.has_permission(&admin, &Permission::MintToken));
    assert!(client.has_permission(&admin, &Permission::ManageUsers));
}

// ── has_permission — Instructor ───────────────────────────────────────────────

#[test]
fn test_instructor_permissions() {
    let (env, admin, client) = setup();
    let instructor = Address::generate(&env);
    client.assign_role(&admin, &instructor, &Role::Instructor);

    assert!(client.has_permission(&instructor, &Permission::CreateCourse));
    assert!(client.has_permission(&instructor, &Permission::EnrollStudent));
    assert!(!client.has_permission(&instructor, &Permission::IssueCredential));
    assert!(!client.has_permission(&instructor, &Permission::MintToken));
    assert!(!client.has_permission(&instructor, &Permission::ManageUsers));
}

// ── has_permission — Student ──────────────────────────────────────────────────

#[test]
fn test_student_has_no_permissions() {
    let (env, admin, client) = setup();
    let student = Address::generate(&env);
    client.assign_role(&admin, &student, &Role::Student);

    assert!(!client.has_permission(&student, &Permission::CreateCourse));
    assert!(!client.has_permission(&student, &Permission::EnrollStudent));
    assert!(!client.has_permission(&student, &Permission::IssueCredential));
    assert!(!client.has_permission(&student, &Permission::MintToken));
    assert!(!client.has_permission(&student, &Permission::ManageUsers));
}

// ── has_permission — unassigned address ──────────────────────────────────────

#[test]
fn test_unassigned_address_has_no_permissions() {
    let (env, _, client) = setup();
    let stranger = Address::generate(&env);
    assert!(!client.has_permission(&stranger, &Permission::CreateCourse));
}

// ── upgrade (Issue 4) ─────────────────────────────────────────────────────────

#[test]
#[should_panic(expected = "Unauthorized: admin required")]
fn test_non_admin_cannot_upgrade() {
    use soroban_sdk::BytesN;
    let (env, _, client) = setup();
    let rando = Address::generate(&env);
    let fake_hash = BytesN::from_array(&env, &[0u8; 32]);
    client.upgrade(&rando, &fake_hash);
}

// =============================================================================
// #696 — Auth, boundary, and cross-contract tests
// =============================================================================

// ── double-initialize guard ───────────────────────────────────────────────────

#[test]
#[should_panic]
fn test_double_initialize_rejected() {
    let (_, admin, client) = setup();
    // second initialize should panic
    client.initialize(&admin);
}

#[test]
#[should_panic(expected = "Already initialized")]
fn test_reinitialization_cannot_replace_admin() {
    // Security (#1169): the admin slot gates every upgrade, so a later
    // `initialize` call — by anyone — must not be able to take it over.
    let (env, _, client) = setup();
    let attacker = Address::generate(&env);
    client.initialize(&attacker);
}

// ── assign_role: only admin ───────────────────────────────────────────────────

#[test]
#[should_panic(expected = "Unauthorized: admin required")]
fn test_non_admin_cannot_assign_admin_role() {
    let (env, _, client) = setup();
    let rando = Address::generate(&env);
    let target = Address::generate(&env);
    client.assign_role(&rando, &target, &Role::Admin);
}

#[test]
#[should_panic(expected = "Unauthorized: admin required")]
fn test_non_admin_cannot_assign_instructor_role() {
    let (env, _, client) = setup();
    let rando = Address::generate(&env);
    let target = Address::generate(&env);
    client.assign_role(&rando, &target, &Role::Instructor);
}

// ── has_role: unassigned and wrong-role checks ───────────────────────────────

#[test]
fn test_newly_created_address_has_no_role() {
    let (env, _, client) = setup();
    let stranger = Address::generate(&env);
    assert!(!client.has_role(&stranger, &Role::Admin));
    assert!(!client.has_role(&stranger, &Role::Instructor));
    assert!(!client.has_role(&stranger, &Role::Student));
}

#[test]
fn test_instructor_does_not_have_admin_role() {
    let (env, admin, client) = setup();
    let instructor = Address::generate(&env);
    client.assign_role(&admin, &instructor, &Role::Instructor);
    assert!(!client.has_role(&instructor, &Role::Admin));
}

// ── permission boundary: each role ───────────────────────────────────────────

#[test]
fn test_student_lacks_all_gated_permissions() {
    let (env, admin, client) = setup();
    let student = Address::generate(&env);
    client.assign_role(&admin, &student, &Role::Student);
    assert!(!client.has_permission(&student, &Permission::MintToken));
    assert!(!client.has_permission(&student, &Permission::IssueCredential));
    assert!(!client.has_permission(&student, &Permission::ManageUsers));
    assert!(!client.has_permission(&student, &Permission::CreateCourse));
    assert!(!client.has_permission(&student, &Permission::EnrollStudent));
}

#[test]
fn test_instructor_lacks_privileged_permissions() {
    let (env, admin, client) = setup();
    let instructor = Address::generate(&env);
    client.assign_role(&admin, &instructor, &Role::Instructor);
    assert!(!client.has_permission(&instructor, &Permission::IssueCredential));
    assert!(!client.has_permission(&instructor, &Permission::MintToken));
    assert!(!client.has_permission(&instructor, &Permission::ManageUsers));
}

// ── authorize_caller: auth guard ─────────────────────────────────────────────

#[test]
#[should_panic(expected = "Unauthorized: admin required")]
fn test_non_admin_cannot_authorize_caller() {
    let (env, _, client) = setup();
    let rando = Address::generate(&env);
    let target_contract = Address::generate(&env);
    let caller = Address::generate(&env);
    client.authorize_caller(&rando, &target_contract, &caller);
}

#[test]
fn test_authorized_caller_is_recognized() {
    let (env, admin, client) = setup();
    let target_contract = Address::generate(&env);
    let caller = Address::generate(&env);
    client.authorize_caller(&admin, &target_contract, &caller);
    assert!(client.is_caller_authorized(&target_contract, &caller));
}

#[test]
fn test_unauthorized_caller_is_not_recognized() {
    let (env, _, client) = setup();
    let target_contract = Address::generate(&env);
    let stranger = Address::generate(&env);
    assert!(!client.is_caller_authorized(&target_contract, &stranger));
}

// ── call_contract: unauthorized caller rejected ───────────────────────────────

#[test]
#[should_panic(expected = "Caller not authorized")]
fn test_call_contract_rejects_unauthorized_caller() {
    let (env, _, client) = setup();
    let caller = Address::generate(&env);
    let target_contract = Address::generate(&env);
    client.call_contract(&caller, &target_contract, &soroban_sdk::symbol_short!("fn"));
}

// ── upgrade: auth guard ───────────────────────────────────────────────────────

#[test]
#[should_panic(expected = "Unauthorized: admin required")]
fn test_non_admin_cannot_upgrade_contract() {
    use soroban_sdk::BytesN;
    let (env, _, client) = setup();
    let rando = Address::generate(&env);
    let hash = BytesN::from_array(&env, &[0u8; 32]);
    client.upgrade(&rando, &hash);
}

// ── schedule_upgrade: auth guard ─────────────────────────────────────────────

#[test]
#[should_panic(expected = "Unauthorized: admin required")]
fn test_non_admin_cannot_schedule_upgrade() {
    use soroban_sdk::BytesN;
    let (env, _, client) = setup();
    let rando = Address::generate(&env);
    let hash = BytesN::from_array(&env, &[0u8; 32]);
    client.schedule_upgrade(&rando, &hash, &100);
}

#[test]
#[should_panic(expected = "Unauthorized: admin required")]
fn test_non_admin_cannot_cancel_upgrade() {
    use soroban_sdk::BytesN;
    let (env, admin, client) = setup();
    // Schedule a real upgrade so the cancel path is reachable
    // (non-zero hash: schedule_upgrade rejects the all-zero hash, issue #1169)
    let hash = BytesN::from_array(&env, &[0xAAu8; 32]);
    client.schedule_upgrade(&admin, &hash, &100);
    // Now try to cancel as a non-admin — should panic
    let rando = Address::generate(&env);
    client.cancel_upgrade(&rando);
}
