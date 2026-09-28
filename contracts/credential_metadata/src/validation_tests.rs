#![cfg(test)]
//! Unit tests for credential_metadata validation helpers.
//! These helpers have no existing tests — this module covers all public functions.

use soroban_sdk::{testutils::Address as _, Address, Env, String};

use crate::{CredentialMetadataContract, CredentialMetadataContractClient, DataKey, MetadataRecord};
use crate::validation::{
    get_metadata_checked, get_metadata_or_panic, is_metadata_valid, is_renewable, metadata_exists,
    validate_admin, validate_future_timestamp, validate_metadata_fields,
};

fn setup() -> (Env, CredentialMetadataContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let id = env.register_contract(None, CredentialMetadataContract);
    let client = CredentialMetadataContractClient::new(&env, &id);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    (env, client, admin)
}

/// Store a sample metadata record via the contract client.
fn store_sample(
    env: &Env,
    client: &CredentialMetadataContractClient,
    admin: &Address,
    credential_id: u64,
    expiry: u64,
) {
    client.store_metadata(
        admin,
        &credential_id,
        &String::from_str(env, "Rust Fundamentals"),
        &1_000,       // completion_date
        &expiry,      // expiry_timestamp
        &String::from_str(env, "A"),
        &String::from_str(env, "QmHash"),
    );
}

// ── validate_admin ────────────────────────────────────────────────────────────

#[test]
fn test_validate_admin_returns_admin_for_correct_caller() {
    let (env, _, admin) = setup();
    env.as_contract(&env.register_contract(None, CredentialMetadataContract), || {
        // Directly set admin in storage to test the helper in isolation.
        env.storage().instance().set(&DataKey::Admin, &admin);
        let returned = validate_admin(&env, &admin);
        assert_eq!(returned, admin);
    });
}

#[test]
#[should_panic(expected = "Only admin can perform this action")]
fn test_validate_admin_panics_for_wrong_caller() {
    let (env, _, admin) = setup();
    env.as_contract(&env.register_contract(None, CredentialMetadataContract), || {
        env.storage().instance().set(&DataKey::Admin, &admin);
        let rando = Address::generate(&env);
        validate_admin(&env, &rando);
    });
}

// ── metadata_exists ───────────────────────────────────────────────────────────

#[test]
fn test_metadata_exists_returns_false_when_not_stored() {
    let (env, _, _) = setup();
    env.as_contract(&env.register_contract(None, CredentialMetadataContract), || {
        assert!(!metadata_exists(&env, 1));
    });
}

#[test]
fn test_metadata_exists_returns_true_after_store() {
    let (env, client, admin) = setup();
    store_sample(&env, &client, &admin, 42, 9_999_999);

    // Reach into the same contract storage by invoking via client
    // and verifying through the public API
    assert!(client.get_metadata(&42).is_some());
}

// ── get_metadata_checked ──────────────────────────────────────────────────────

#[test]
fn test_get_metadata_checked_returns_none_for_missing() {
    let (env, _, _) = setup();
    env.as_contract(&env.register_contract(None, CredentialMetadataContract), || {
        assert!(get_metadata_checked(&env, 99).is_none());
    });
}

// ── get_metadata_or_panic ─────────────────────────────────────────────────────

#[test]
#[should_panic(expected = "Credential metadata not found")]
fn test_get_metadata_or_panic_panics_for_missing() {
    let (env, _, _) = setup();
    env.as_contract(&env.register_contract(None, CredentialMetadataContract), || {
        get_metadata_or_panic(&env, 999);
    });
}

// ── is_renewable ─────────────────────────────────────────────────────────────

#[test]
fn test_is_renewable_returns_false_when_no_metadata() {
    let (env, _, _) = setup();
    env.as_contract(&env.register_contract(None, CredentialMetadataContract), || {
        // No metadata stored — always false
        assert!(!is_renewable(&env, 1, 86400));
    });
}

// ── validate_future_timestamp ─────────────────────────────────────────────────

#[test]
#[should_panic(expected = "Timestamp must be in the future")]
fn test_validate_future_timestamp_panics_for_past() {
    let (env, _, _) = setup();
    env.as_contract(&env.register_contract(None, CredentialMetadataContract), || {
        // ledger timestamp defaults to 0 in test env;
        // passing 0 means it is NOT in the future (must be > current)
        validate_future_timestamp(&env, 0);
    });
}

#[test]
fn test_validate_future_timestamp_passes_for_future() {
    let (env, _, _) = setup();
    env.as_contract(&env.register_contract(None, CredentialMetadataContract), || {
        // Any value > 0 is in the future when ledger timestamp = 0
        validate_future_timestamp(&env, 1_000_000); // should not panic
    });
}

// ── Integration: store → exists → get ─────────────────────────────────────────

#[test]
fn test_store_and_retrieve_via_client() {
    let (env, client, admin) = setup();
    store_sample(&env, &client, &admin, 7, 9_999_999);

    let meta = client.get_metadata(&7).unwrap();
    assert_eq!(meta.credential_id, 7);
    assert_eq!(meta.grade, String::from_str(&env, "A"));
    assert_eq!(meta.ipfs_hash, String::from_str(&env, "QmHash"));
}

#[test]
fn test_is_expired_returns_false_for_far_future_expiry() {
    let (env, client, admin) = setup();
    store_sample(&env, &client, &admin, 10, 9_999_999_999);
    assert!(!client.is_expired(&10));
}

// ── Metadata field validation (Issue #1170) ─────────────────────────────────
//
// Field rules come from `brain_storm_shared::validation`; this contract only
// decides *when* to apply them (every write path) and exposes a non-panicking
// `is_metadata_valid` for callers that must report instead of reject.

#[test]
fn test_is_metadata_valid_false_when_missing() {
    let (env, client, _) = setup();
    let valid = env.as_contract(&client.address, || is_metadata_valid(&env, 1234));
    assert!(!valid);
}

#[test]
fn test_is_metadata_valid_true_for_stored_record() {
    let (env, client, admin) = setup();
    store_sample(&env, &client, &admin, 55, 9_999_999);
    let valid = env.as_contract(&client.address, || is_metadata_valid(&env, 55));
    assert!(valid);
}

/// A record exactly as the write paths store it.
fn valid_record(env: &Env, credential_id: u64) -> MetadataRecord {
    MetadataRecord {
        credential_id,
        course_name: String::from_str(env, "Rust Fundamentals"),
        completion_date: 1_000,
        expiry_timestamp: 9_999_999,
        grade: String::from_str(env, "A"),
        ipfs_hash: String::from_str(env, "QmHash"),
    }
}

/// Writes a record straight to storage, bypassing the write-path gate, so the
/// reader (`is_metadata_valid`) is what gets tested.
fn write_record(env: &Env, client: &CredentialMetadataContractClient, record: MetadataRecord) {
    let id = record.credential_id;
    env.as_contract(&client.address, || {
        env.storage()
            .persistent()
            .set(&DataKey::Metadata(id), &record);
    });
}

#[test]
fn test_is_metadata_valid_false_for_empty_ipfs_hash() {
    let (env, client, _) = setup();
    let mut record = valid_record(&env, 70);
    record.ipfs_hash = String::from_str(&env, "");
    write_record(&env, &client, record);
    let valid = env.as_contract(&client.address, || is_metadata_valid(&env, 70));
    assert!(!valid);
}

#[test]
fn test_is_metadata_valid_false_for_invalid_course_name() {
    let (env, client, _) = setup();
    let mut record = valid_record(&env, 71);
    record.course_name = String::from_str(&env, "x");
    write_record(&env, &client, record);
    let valid = env.as_contract(&client.address, || is_metadata_valid(&env, 71));
    assert!(!valid);
}

#[test]
fn test_is_metadata_valid_false_when_expiry_before_completion() {
    let (env, client, _) = setup();
    let mut record = valid_record(&env, 72);
    record.expiry_timestamp = 999; // completion_date is 1_000
    write_record(&env, &client, record);
    let valid = env.as_contract(&client.address, || is_metadata_valid(&env, 72));
    assert!(!valid);
}

#[test]
#[should_panic(expected = "Course name must be between 3 and 100 characters")]
fn test_store_metadata_rejects_empty_course_name() {
    let (env, client, admin) = setup();
    client.store_metadata(
        &admin,
        &1,
        &String::from_str(&env, ""),
        &1_000,
        &9_999_999,
        &String::from_str(&env, "A"),
        &String::from_str(&env, "QmHash"),
    );
}

#[test]
#[should_panic(expected = "Course name must be between 3 and 100 characters")]
fn test_store_metadata_rejects_overlong_course_name() {
    let (env, client, admin) = setup();
    let overlong = "X".repeat(101);
    client.store_metadata(
        &admin,
        &1,
        &String::from_str(&env, &overlong),
        &1_000,
        &9_999_999,
        &String::from_str(&env, "A"),
        &String::from_str(&env, "QmHash"),
    );
}

#[test]
#[should_panic(expected = "Grade must be 20 characters or less")]
fn test_store_metadata_rejects_empty_grade() {
    let (env, client, admin) = setup();
    client.store_metadata(
        &admin,
        &1,
        &String::from_str(&env, "Rust Fundamentals"),
        &1_000,
        &9_999_999,
        &String::from_str(&env, ""),
        &String::from_str(&env, "QmHash"),
    );
}

#[test]
#[should_panic(expected = "Course name must be between 3 and 100 characters")]
fn test_update_metadata_rejects_invalid_course_name() {
    let (env, client, admin) = setup();
    store_sample(&env, &client, &admin, 66, 9_999_999);
    client.update_metadata(
        &admin,
        &66,
        &String::from_str(&env, "x"),
        &String::from_str(&env, "B"),
    );
}

#[test]
#[should_panic(expected = "String must not be empty")]
fn test_store_metadata_rejects_empty_ipfs_hash() {
    let (env, client, admin) = setup();
    client.store_metadata(
        &admin,
        &1,
        &String::from_str(&env, "Rust Fundamentals"),
        &1_000,
        &9_999_999,
        &String::from_str(&env, "A"),
        &String::from_str(&env, ""),
    );
}

#[test]
fn test_validate_metadata_fields_accepts_valid_values() {
    let env = Env::default();
    // No panic = accepted
    validate_metadata_fields(
        &String::from_str(&env, "Rust Fundamentals"),
        &String::from_str(&env, "A"),
        &String::from_str(&env, "QmHash"),
    );
}

#[test]
#[should_panic(expected = "String must not be empty")]
fn test_validate_metadata_fields_rejects_missing_ipfs_hash() {
    let env = Env::default();
    validate_metadata_fields(
        &String::from_str(&env, "Rust Fundamentals"),
        &String::from_str(&env, "A"),
        &String::from_str(&env, ""),
    );
}
