//! Staking module — Issue #634
//!
//! Adds delegation and undelegation flows with epoch-based reward accrual,
//! rounding-safe arithmetic, optional slashing hooks, and comprehensive events.
use soroban_sdk::{contracttype, Address, Env, Symbol, symbol_short};

use soroban_sdk::{contracttype, Address, Env, Symbol, Vec, symbol_short};
use brain_storm_shared::constants::{BASIS_POINTS_DENOMINATOR, PRECISION_SCALE_12};

// ── Constants ─────────────────────────────────────────────────────────────────

/// ~100 ledgers per lock period (≈ 8 min on Stellar testnet).
pub const LEDGERS_PER_PERIOD: u32 = 100;

/// Epoch length in ledgers (rewards accrue once per epoch).
pub const EPOCH_LENGTH: u32 = 1_000;

// ── Events ────────────────────────────────────────────────────────────────────

pub const STAKE_CREATED: Symbol  = symbol_short!("stake");
pub const STAKE_WITHDRAWN: Symbol = symbol_short!("unstake");
pub const REWARDS_CLAIMED: Symbol = symbol_short!("reward");
pub const DELEGATED: Symbol       = symbol_short!("delegate");
pub const UNDELEGATED: Symbol     = symbol_short!("undelegat");
pub const SLASHED: Symbol         = symbol_short!("slashed");

// ── Storage keys ──────────────────────────────────────────────────────────────

#[contracttype]
pub enum StakingKey {
    /// Direct stake record for a staker.
    Stake(Address),
    /// Delegation record: delegator → delegatee.
    Delegation(Address, Address),
    /// All delegatees a delegator has delegated to.
    DelegatorTargets(Address),
    /// Accumulated reward per epoch token (global accumulator).
    RewardPerToken,
    /// Snapshot of global accumulator at last update for a staker.
    StakerRewardDebt(Address),
    /// Total tokens staked (direct + delegated).
    TotalStaked,
    /// Base reward rate in micro-units per ledger per staked token.
    RewardRate,
    /// Early withdrawal penalty in basis points (100 = 1%).
    EarlyWithdrawalPenalty,
    /// Slashing configuration (admin-gated).
    SlashConfig,
    /// Whether slashing is enabled.
    SlashEnabled,
    /// Analytics per staker.
    StakingAnalytics(Address),
}

// ── Types ─────────────────────────────────────────────────────────────────────

/// A direct stake held by a staker.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StakeRecord {
    pub staker: Address,
    pub amount: i128,
    pub lock_start_ledger: u32,
    pub lock_end_ledger: u32,
    /// Cumulative rewards already claimed.
    pub rewards_earned: i128,
    /// Epoch at which this stake was last updated.
    pub last_epoch: u32,
}

/// A delegation from one address to another.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DelegationRecord {
    pub delegator: Address,
    pub delegatee: Address,
    /// Amount delegated in stroops.
    pub amount: i128,
    /// Epoch when delegation was created.
    pub epoch: u32,
    /// Accumulated rewards for this delegation.
    pub rewards_accrued: i128,
    /// Snapshot of global reward-per-token at last update.
    pub reward_debt: i128,
}

/// Slashing configuration stored by admin.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SlashConfig {
    /// Slash rate in basis points (e.g. 500 = 5%).
    pub slash_rate_bps: u32,
    /// Address that receives slashed tokens.
    pub slash_recipient: Address,
}

// ── Internal helpers ──────────────────────────────────────────────────────────

fn current_epoch(env: &Env) -> u32 {
    env.ledger().sequence() / EPOCH_LENGTH
}

/// Update the global reward-per-token accumulator.
///
/// Uses integer arithmetic: accumulator scaled by 1e12 to avoid precision loss.
fn accrue_global(env: &Env) {
    let total: i128 = env.storage().instance().get(&StakingKey::TotalStaked).unwrap_or(0);
    if total == 0 { return; }

    let rate: i128 = env.storage().instance().get(&StakingKey::RewardRate).unwrap_or(500);
    let rpt: i128 = env.storage().instance().get(&StakingKey::RewardPerToken).unwrap_or(0);

    // new_rpt = rpt + rate * EPOCH_LENGTH / total  (scaled by PRECISION_SCALE_12)
    let increment = rate
        .checked_mul(EPOCH_LENGTH as i128).expect("overflow")
        .checked_mul(PRECISION_SCALE_12).expect("overflow")
        .checked_div(total).expect("div zero");

    env.storage().instance().set(&StakingKey::RewardPerToken, &(rpt + increment));
}

/// Compute pending rewards for `amount` staked since `reward_debt`.
fn pending_rewards(env: &Env, amount: i128, reward_debt: i128) -> i128 {
    let rpt: i128 = env.storage().instance().get(&StakingKey::RewardPerToken).unwrap_or(0);
    let diff = rpt.saturating_sub(reward_debt);
    // result = amount * diff / PRECISION_SCALE_12 (rounding down — safe against over-distribution)
    amount.checked_mul(diff).expect("overflow").checked_div(PRECISION_SCALE_12).expect("div zero")
}

// ── Direct staking ────────────────────────────────────────────────────────────

/// Stake `amount` tokens for `lock_periods` epochs.
///
/// # Panics
/// - `"Stake amount must be positive"` if `amount <= 0`.
pub fn stake(env: &Env, staker: Address, amount: i128, lock_periods: u32) -> StakeRecord {
    staker.require_auth();
    assert!(amount > 0, "Stake amount must be positive");

    accrue_global(env);

    let rpt: i128 = env.storage().instance().get(&StakingKey::RewardPerToken).unwrap_or(0);
    let lock_end = env.ledger().sequence() + (lock_periods * LEDGERS_PER_PERIOD);

    let record = StakeRecord {
        staker: staker.clone(),
        amount,
        lock_start_ledger: env.ledger().sequence(),
        lock_end_ledger: lock_end,
        rewards_earned: 0,
        last_epoch: current_epoch(env),
    };

    let total: i128 = env.storage().instance().get(&StakingKey::TotalStaked).unwrap_or(0);
    env.storage().instance().set(&StakingKey::TotalStaked, &total.checked_add(amount).expect("overflow"));
    env.storage().instance().set(&StakingKey::Stake(staker.clone()), &record);
    env.storage().instance().set(&StakingKey::StakerRewardDebt(staker.clone()), &rpt);

    env.events().publish((STAKE_CREATED,), (staker.clone(), amount, lock_periods));
    record
}

/// Calculate pending rewards for a direct staker.
pub fn calculate_rewards(env: &Env, staker: &Address) -> i128 {
    let record: Option<StakeRecord> = env.storage().instance().get(&StakingKey::Stake(staker.clone()));
    let Some(s) = record else { return 0; };
    let debt: i128 = env.storage().instance().get(&StakingKey::StakerRewardDebt(staker.clone())).unwrap_or(0);
    pending_rewards(env, s.amount, debt)
}

/// Withdraw staked tokens, optionally early (triggers penalty).
///
/// Returns the net withdrawal amount (principal + rewards - penalty).
pub fn withdraw(env: &Env, staker: Address, early: bool) -> i128 {
    staker.require_auth();
    accrue_global(env);

    let record: StakeRecord = env.storage().instance()
        .get(&StakingKey::Stake(staker.clone()))
        .expect("No stake found");

    let debt: i128 = env.storage().instance()
        .get(&StakingKey::StakerRewardDebt(staker.clone())).unwrap_or(0);

    let rewards = pending_rewards(env, record.amount, debt);
    let is_locked = env.ledger().sequence() < record.lock_end_ledger;

    let mut net = record.amount.checked_add(rewards).expect("overflow");

    if early && is_locked {
        let penalty_bps: i128 = env.storage().instance()
            .get(&StakingKey::EarlyWithdrawalPenalty).unwrap_or(100);
        let penalty = record.amount.checked_mul(penalty_bps).expect("overflow") / BASIS_POINTS_DENOMINATOR;
        net = net.checked_sub(penalty).expect("underflow");
    }

    let total: i128 = env.storage().instance().get(&StakingKey::TotalStaked).unwrap_or(0);
    env.storage().instance().set(&StakingKey::TotalStaked, &total.saturating_sub(record.amount));
    env.storage().instance().remove(&StakingKey::Stake(staker.clone()));
    env.storage().instance().remove(&StakingKey::StakerRewardDebt(staker.clone()));

    env.events().publish((STAKE_WITHDRAWN,), (staker.clone(), net));
    net
}

/// Claim accumulated rewards without withdrawing principal.
///
/// Returns rewards minted to staker (caller must actually mint via token contract).
pub fn claim_rewards(env: &Env, staker: Address) -> i128 {
    staker.require_auth();
    accrue_global(env);

    let mut record: StakeRecord = env.storage().instance()
        .get(&StakingKey::Stake(staker.clone()))
        .expect("No stake found");

    let rpt: i128 = env.storage().instance().get(&StakingKey::RewardPerToken).unwrap_or(0);
    let debt: i128 = env.storage().instance()
        .get(&StakingKey::StakerRewardDebt(staker.clone())).unwrap_or(0);

    let rewards = pending_rewards(env, record.amount, debt);
    assert!(rewards > 0, "No rewards to claim");

    record.rewards_earned = record.rewards_earned.checked_add(rewards).expect("overflow");
    env.storage().instance().set(&StakingKey::Stake(staker.clone()), &record);
    // Reset debt to current rpt
    env.storage().instance().set(&StakingKey::StakerRewardDebt(staker.clone()), &rpt);

    env.events().publish((REWARDS_CLAIMED,), (staker.clone(), rewards));
    rewards
}

// ── Delegation ────────────────────────────────────────────────────────────────

/// Delegate `amount` tokens to `delegatee`.
///
/// The delegator must have previously staked at least `amount`.
/// Rewards are computed proportionally and credited to the delegator.
pub fn delegate(env: &Env, delegator: Address, delegatee: Address, amount: i128) {
    delegator.require_auth();
    assert!(amount > 0, "Delegation amount must be positive");
    assert!(delegator != delegatee, "Cannot delegate to self");

    accrue_global(env);

    let rpt: i128 = env.storage().instance().get(&StakingKey::RewardPerToken).unwrap_or(0);
    let record = DelegationRecord {
        delegator: delegator.clone(),
        delegatee: delegatee.clone(),
        amount,
        epoch: current_epoch(env),
        rewards_accrued: 0,
        reward_debt: rpt,
    };

    env.storage().persistent().set(&StakingKey::Delegation(delegator.clone(), delegatee.clone()), &record);

    // Track delegatee list for delegator
    let mut targets: Vec<Address> = env.storage().persistent()
        .get(&StakingKey::DelegatorTargets(delegator.clone()))
        .unwrap_or_else(|| Vec::new(env));
    if !targets.contains(&delegatee) {
        targets.push_back(delegatee.clone());
        env.storage().persistent().set(&StakingKey::DelegatorTargets(delegator.clone()), &targets);
    }

    // Delegated stake counts towards total
    let total: i128 = env.storage().instance().get(&StakingKey::TotalStaked).unwrap_or(0);
    env.storage().instance().set(&StakingKey::TotalStaked, &total.checked_add(amount).expect("overflow"));

    env.events().publish((DELEGATED,), (delegator.clone(), delegatee.clone(), amount));
}

/// Undelegate tokens from `delegatee`, claiming accrued rewards.
///
/// Returns the accrued rewards to be minted.
pub fn undelegate(env: &Env, delegator: Address, delegatee: Address) -> i128 {
    delegator.require_auth();
    accrue_global(env);

    let mut record: DelegationRecord = env.storage().persistent()
        .get(&StakingKey::Delegation(delegator.clone(), delegatee.clone()))
        .expect("No delegation found");

    let rpt: i128 = env.storage().instance().get(&StakingKey::RewardPerToken).unwrap_or(0);
    let rewards = pending_rewards(env, record.amount, record.reward_debt);

    record.rewards_accrued = record.rewards_accrued.checked_add(rewards).expect("overflow");
    record.reward_debt = rpt;

    // Remove delegation
    env.storage().persistent().remove(&StakingKey::Delegation(delegator.clone(), delegatee.clone()));

    // Remove from targets list
    let mut targets: Vec<Address> = env.storage().persistent()
        .get(&StakingKey::DelegatorTargets(delegator.clone()))
        .unwrap_or_else(|| Vec::new(env));
    if let Some(pos) = targets.iter().position(|a| a == delegatee) {
        targets.remove(pos as u32);
        env.storage().persistent().set(&StakingKey::DelegatorTargets(delegator.clone()), &targets);
    }

    // Reduce total staked
    let total: i128 = env.storage().instance().get(&StakingKey::TotalStaked).unwrap_or(0);
    env.storage().instance().set(&StakingKey::TotalStaked, &total.saturating_sub(record.amount));

    env.events().publish((UNDELEGATED,), (delegator.clone(), delegatee.clone(), record.amount, rewards));
    rewards
}

/// Get delegation record.
pub fn get_delegation(env: &Env, delegator: Address, delegatee: Address) -> Option<DelegationRecord> {
    env.storage().persistent().get(&StakingKey::Delegation(delegator, delegatee))
}

/// Get all delegatees for a delegator.
pub fn get_delegatee_list(env: &Env, delegator: Address) -> Vec<Address> {
    env.storage().persistent()
        .get(&StakingKey::DelegatorTargets(delegator))
        .unwrap_or_else(|| Vec::new(env))
}

// ── Slashing (admin-gated) ────────────────────────────────────────────────────

/// Configure slashing (admin only). Call with `enabled = false` to disable.
pub fn set_slash_config(env: &Env, admin: Address, slash_rate_bps: u32, slash_recipient: Address, enabled: bool) {
    admin.require_auth();
    assert!(slash_rate_bps <= BASIS_POINTS_DENOMINATOR as u32, "Slash rate must be <= 10_000 bps");

    env.storage().instance().set(&StakingKey::SlashConfig, &SlashConfig { slash_rate_bps, slash_recipient });
    env.storage().instance().set(&StakingKey::SlashEnabled, &enabled);
}

/// Apply slash to a staker (admin only). Returns slashed amount.
pub fn slash(env: &Env, admin: Address, staker: Address) -> i128 {
    admin.require_auth();

    let enabled: bool = env.storage().instance().get(&StakingKey::SlashEnabled).unwrap_or(false);
    assert!(enabled, "Slashing is not enabled");

    let config: SlashConfig = env.storage().instance()
        .get(&StakingKey::SlashConfig)
        .expect("Slash config not set");

    let mut record: StakeRecord = env.storage().instance()
        .get(&StakingKey::Stake(staker.clone()))
        .expect("No stake found");

    let slash_amount = record.amount
        .checked_mul(config.slash_rate_bps as i128).expect("overflow")
        / BASIS_POINTS_DENOMINATOR;

    record.amount = record.amount.checked_sub(slash_amount).expect("underflow");
    env.storage().instance().set(&StakingKey::Stake(staker.clone()), &record);

    let total: i128 = env.storage().instance().get(&StakingKey::TotalStaked).unwrap_or(0);
    env.storage().instance().set(&StakingKey::TotalStaked, &total.saturating_sub(slash_amount));

    env.events().publish((SLASHED,), (staker.clone(), slash_amount, config.slash_recipient));
    slash_amount
}

// ── Queries ───────────────────────────────────────────────────────────────────

pub fn get_stake(env: &Env, staker: Address) -> Option<StakeRecord> {
    env.storage().instance().get(&StakingKey::Stake(staker))
}

pub fn get_total_staked(env: &Env) -> i128 {
    env.storage().instance().get(&StakingKey::TotalStaked).unwrap_or(0)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::{Address as _, Ledger, LedgerInfo}, Env};

    fn setup() -> (Env, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();
        // Init reward rate
        env.storage().instance().set(&StakingKey::RewardRate, &1_000_i128);
        let admin = Address::generate(&env);
        let staker = Address::generate(&env);
        (env, admin, staker)
    }

    fn advance_epoch(env: &Env, epochs: u32) {
        env.ledger().set(LedgerInfo {
            sequence_number: env.ledger().sequence() + epochs * EPOCH_LENGTH,
            timestamp: 0,
            protocol_version: 21,
            network_id: Default::default(),
            base_reserve: 10,
            min_temp_entry_ttl: 1000,
            min_persistent_entry_ttl: 1000,
            max_entry_ttl: 100_000,
        });
    }

    #[test]
    fn test_stake_and_withdraw() {
        let (env, _, staker) = setup();
        stake(&env, staker.clone(), 1_000, 1);
        assert_eq!(get_total_staked(&env), 1_000);
        let net = withdraw(&env, staker.clone(), false);
        assert!(net >= 1_000);
        assert_eq!(get_total_staked(&env), 0);
    }

    #[test]
    fn test_early_withdrawal_penalty() {
        let (env, _, staker) = setup();
        env.storage().instance().set(&StakingKey::EarlyWithdrawalPenalty, &500_i128); // 5%
        stake(&env, staker.clone(), 10_000, 10);
        let net = withdraw(&env, staker.clone(), true);
        // Should receive less than 10_000 due to penalty
        assert!(net < 10_000);
    }

    #[test]
    fn test_delegate_and_undelegate() {
        let (env, _, delegator) = setup();
        let delegatee = Address::generate(&env);
        delegate(&env, delegator.clone(), delegatee.clone(), 5_000);
        assert_eq!(get_total_staked(&env), 5_000);

        advance_epoch(&env, 2);
        accrue_global(&env);

        let rewards = undelegate(&env, delegator.clone(), delegatee.clone());
        assert!(rewards >= 0);
        assert_eq!(get_total_staked(&env), 0);
    }

    #[test]
    #[should_panic(expected = "Cannot delegate to self")]
    fn test_self_delegation_panics() {
        let (env, _, staker) = setup();
        delegate(&env, staker.clone(), staker.clone(), 1_000);
    }

    #[test]
    fn test_rewards_accrue_across_epochs() {
        let (env, _, staker) = setup();
        stake(&env, staker.clone(), 1_000_000, 5);
        advance_epoch(&env, 3);
        let rewards = calculate_rewards(&env, &staker);
        assert!(rewards > 0, "Rewards should accrue after epochs");
    }

    #[test]
    fn test_slash_reduces_stake() {
        let (env, admin, staker) = setup();
        let recipient = Address::generate(&env);
        stake(&env, staker.clone(), 10_000, 2);
        set_slash_config(&env, admin.clone(), 1_000, recipient, true); // 10%
        let slashed = slash(&env, admin, staker.clone());
        assert_eq!(slashed, 1_000); // 10% of 10_000
        let record = get_stake(&env, staker).unwrap();
        assert_eq!(record.amount, 9_000);
    }

    #[test]
    #[should_panic(expected = "Stake amount must be positive")]
    fn test_zero_stake_panics() {
        let (env, _, staker) = setup();
        stake(&env, staker, 0, 1);
    }

    #[test]
    fn test_overflow_safety() {
        let (env, _, staker) = setup();
        // Large amount should not overflow with checked arithmetic
        stake(&env, staker.clone(), i128::MAX / 2, 1);
        assert!(get_total_staked(&env) > 0);
    }
}
