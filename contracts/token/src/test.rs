#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Ledger, LedgerInfo},
    Address, Env, String,
};
use crate::{TokenContract, TokenContractClient, TokenError};

const INITIAL_SUPPLY: i128 = 1_000_000_000_000_000_000; // 1 billion tokens

fn setup_test_environment(env: &Env) -> (TokenContractClient, Address, Address, Address) {
    env.mock_all_auths();
    env.ledger().set(LedgerInfo {
        timestamp: 1735689600,
        protocol_version: 20,
        sequence_number: 1,
        network_id: [0; 32],
        base_reserve: 10,
        min_temp_entry_ttl: 1,
        min_persistent_entry_ttl: 1,
        max_entry_ttl: 518400,
    });

    let admin = Address::generate(env);
    let user1 = Address::generate(env);
    let user2 = Address::generate(env);

    let contract_id = env.register_contract(None, TokenContract);
    let client = TokenContractClient::new(env, &contract_id);

    let name = String::from_str(env, "Test Token");
    let symbol = String::from_str(env, "TEST");
    let decimals = 18;

    client.initialize(&admin, &name, &symbol, &decimals, &INITIAL_SUPPLY);

    (client, admin, user1, user2)
}

// ============================================
// # Transfer Tests
// ============================================

#[test]
fn test_transfer_success() {
    let env = Env::default();
    let (client, admin, user1, user2) = setup_test_environment(&env);

    let amount = 1000_i128;
    
    // Transfer from admin to user1
    client.transfer(&admin, &user1, &amount);

    let balance = client.balance(&user1);
    assert_eq!(balance, amount);

    let admin_balance = client.balance(&admin);
    assert_eq!(admin_balance, INITIAL_SUPPLY - amount);
}

#[test]
fn test_transfer_insufficient_balance() {
    let env = Env::default();
    let (client, admin, user1, user2) = setup_test_environment(&env);

    let amount = INITIAL_SUPPLY + 1;

    let result = client.try_transfer(&admin, &user1, &amount);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().unwrap(), TokenError::InsufficientBalance);
}

#[test]
fn test_transfer_zero_amount() {
    let env = Env::default();
    let (client, admin, user1, user2) = setup_test_environment(&env);

    let result = client.try_transfer(&admin, &user1, &0);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().unwrap(), TokenError::InvalidAmount);
}

#[test]
fn test_transfer_negative_amount() {
    let env = Env::default();
    let (client, admin, user1, user2) = setup_test_environment(&env);

    let result = client.try_transfer(&admin, &user1, &-100);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().unwrap(), TokenError::InvalidAmount);
}

#[test]
fn test_transfer_overflow() {
    let env = Env::default();
    let (client, admin, user1, user2) = setup_test_environment(&env);

    // First transfer a large amount to user1
    let amount1 = INITIAL_SUPPLY / 2;
    client.transfer(&admin, &user1, &amount1);

    // Try to transfer more than balance
    let amount2 = INITIAL_SUPPLY;
    let result = client.try_transfer(&user1, &user2, &amount2);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().unwrap(), TokenError::InsufficientBalance);
}

#[test]
fn test_transfer_max_value() {
    let env = Env::default();
    let (client, admin, user1, user2) = setup_test_environment(&env);

    let max_amount = i128::MAX / 2;
    
    // Mint max amount to admin (requires admin approval in real scenario)
    // For test, we'll use available supply
    let result = client.try_transfer(&admin, &user1, &max_amount);
    // Should fail due to insufficient balance
    assert!(result.is_err());
}

// ============================================
// # Mint Tests
// ============================================

#[test]
fn test_mint_success() {
    let env = Env::default();
    let (client, admin, user1, user2) = setup_test_environment(&env);

    let mint_amount = 1000_i128;
    client.mint(&admin, &user1, &mint_amount);

    let balance = client.balance(&user1);
    assert_eq!(balance, mint_amount);

    let total_supply = client.total_supply();
    assert_eq!(total_supply, INITIAL_SUPPLY + mint_amount);
}

#[test]
fn test_mint_overflow() {
    let env = Env::default();
    let (client, admin, user1, user2) = setup_test_environment(&env);

    // Try to mint an amount that would overflow
    let mint_amount = i128::MAX;
    let result = client.try_mint(&admin, &user1, &mint_amount);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().unwrap(), TokenError::ArithmeticError);
}

#[test]
fn test_mint_zero_amount() {
    let env = Env::default();
    let (client, admin, user1, user2) = setup_test_environment(&env);

    let result = client.try_mint(&admin, &user1, &0);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().unwrap(), TokenError::InvalidAmount);
}

#[test]
fn test_mint_unauthorized() {
    let env = Env::default();
    let (client, admin, user1, user2) = setup_test_environment(&env);

    let result = client.try_mint(&user1, &user2, &1000);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().unwrap(), TokenError::Unauthorized);
}

// ============================================
// # Burn Tests
// ============================================

#[test]
fn test_burn_success() {
    let env = Env::default();
    let (client, admin, user1, user2) = setup_test_environment(&env);

    // First transfer to user1
    let transfer_amount = 1000_i128;
    client.transfer(&admin, &user1, &transfer_amount);

    // Burn from user1
    let burn_amount = 500_i128;
    client.burn(&admin, &user1, &burn_amount);

    let balance = client.balance(&user1);
    assert_eq!(balance, transfer_amount - burn_amount);

    let total_supply = client.total_supply();
    assert_eq!(total_supply, INITIAL_SUPPLY - burn_amount);
}

#[test]
fn test_burn_insufficient_balance() {
    let env = Env::default();
    let (client, admin, user1, user2) = setup_test_environment(&env);

    let burn_amount = 1000_i128;
    let result = client.try_burn(&admin, &user1, &burn_amount);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().unwrap(), TokenError::InsufficientBalance);
}

#[test]
fn test_burn_overflow() {
    let env = Env::default();
    let (client, admin, user1, user2) = setup_test_environment(&env);

    // Transfer all supply to user1
    client.transfer(&admin, &user1, &INITIAL_SUPPLY);

    // Try to burn more than supply
    let burn_amount = INITIAL_SUPPLY + 1;
    let result = client.try_burn(&admin, &user1, &burn_amount);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().unwrap(), TokenError::InsufficientBalance);
}

// ============================================
// # Approve/TransferFrom Tests
// ============================================

#[test]
fn test_approve_and_transfer_from() {
    let env = Env::default();
    let (client, admin, user1, user2) = setup_test_environment(&env);

    // Transfer some tokens to user1
    let transfer_amount = 1000_i128;
    client.transfer(&admin, &user1, &transfer_amount);

    // Approve user2 to spend tokens on behalf of user1
    let approve_amount = 500_i128;
    client.approve(&user1, &user2, &approve_amount);

    // Transfer from user1 to admin using user2's approval
    client.transfer_from(&user2, &user1, &admin, &approve_amount);

    let user1_balance = client.balance(&user1);
    assert_eq!(user1_balance, transfer_amount - approve_amount);

    let admin_balance = client.balance(&admin);
    assert_eq!(admin_balance, INITIAL_SUPPLY - transfer_amount + approve_amount);
}

#[test]
fn test_transfer_from_insufficient_allowance() {
    let env = Env::default();
    let (client, admin, user1, user2) = setup_test_environment(&env);

    // Transfer some tokens to user1
    let transfer_amount = 1000_i128;
    client.transfer(&admin, &user1, &transfer_amount);

    // Approve user2 to spend tokens on behalf of user1
    let approve_amount = 500_i128;
    client.approve(&user1, &user2, &approve_amount);

    // Try to spend more than approved
    let result = client.try_transfer_from(&user2, &user1, &admin, &(approve_amount + 1));
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().unwrap(), TokenError::InsufficientAllowance);
}

#[test]
fn test_transfer_from_insufficient_balance() {
    let env = Env::default();
    let (client, admin, user1, user2) = setup_test_environment(&env);

    // Approve user2 to spend tokens on behalf of user1 (user1 has 0 balance)
    let approve_amount = 500_i128;
    client.approve(&user1, &user2, &approve_amount);

    // Try to spend tokens user1 doesn't have
    let result = client.try_transfer_from(&user2, &user1, &admin, &approve_amount);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().unwrap(), TokenError::InsufficientBalance);
}

// ============================================
// # View Function Tests
// ============================================

#[test]
fn test_balance_view() {
    let env = Env::default();
    let (client, admin, user1, user2) = setup_test_environment(&env);

    let balance = client.balance(&admin);
    assert_eq!(balance, INITIAL_SUPPLY);

    let user_balance = client.balance(&user1);
    assert_eq!(user_balance, 0);
}

#[test]
fn test_total_supply_view() {
    let env = Env::default();
    let (client, admin, user1, user2) = setup_test_environment(&env);

    let total_supply = client.total_supply();
    assert_eq!(total_supply, INITIAL_SUPPLY);
}

#[test]
fn test_get_state_view() {
    let env = Env::default();
    let (client, admin, user1, user2) = setup_test_environment(&env);

    let state = client.get_state();
    assert_eq!(state.decimals, 18);
    assert_eq!(state.total_supply, INITIAL_SUPPLY);
}

#[test]
fn test_allowance_view() {
    let env = Env::default();
    let (client, admin, user1, user2) = setup_test_environment(&env);

    let approve_amount = 500_i128;
    client.approve(&user1, &user2, &approve_amount);

    let allowance = client.allowance(&user1, &user2);
    assert_eq!(allowance, approve_amount);
}

// ============================================
// # Edge Case Tests
// ============================================

#[test]
fn test_balance_after_multiple_transfers() {
    let env = Env::default();
    let (client, admin, user1, user2) = setup_test_environment(&env);

    let amount1 = 100_i128;
    let amount2 = 200_i128;
    let amount3 = 300_i128;

    client.transfer(&admin, &user1, &amount1);
    client.transfer(&admin, &user2, &amount2);
    client.transfer(&user1, &user2, &amount3);

    let admin_balance = client.balance(&admin);
    let user1_balance = client.balance(&user1);
    let user2_balance = client.balance(&user2);

    assert_eq!(admin_balance, INITIAL_SUPPLY - amount1 - amount2);
    assert_eq!(user1_balance, amount1 - amount3);
    assert_eq!(user2_balance, amount2 + amount3);
}

#[test]
fn test_arithmetic_overflow_edge_case() {
    let env = Env::default();
    let (client, admin, user1, user2) = setup_test_environment(&env);

    // Try to transfer an amount that would overflow
    let huge_amount = i128::MAX;
    let result = client.try_transfer(&admin, &user1, &huge_amount);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().unwrap(), TokenError::InsufficientBalance);
}
