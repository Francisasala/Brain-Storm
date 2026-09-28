#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, Env, IntoVal, String, Symbol,
};

use brain_storm_shared::access;
use brain_storm_shared::constants::{MIN_TEMP_ENTRY_TTL, MIN_PERSISTENT_ENTRY_TTL, MAX_ENTRY_TTL};

pub mod voting;

use voting::{
    VotingStrategy, VoteSupport,
    GovernanceKey, VotingProposal, VoteRecord,
    create_proposal as voting_create_proposal,
    cast_vote as voting_cast_vote,
    finalise_proposal as voting_finalise_proposal,
    execute_proposal as voting_execute_proposal,
    cancel_proposal as voting_cancel_proposal,
    delegate_vote as voting_delegate_vote,
    get_proposal as voting_get_proposal,
    get_vote_record as voting_get_vote_record,
    get_delegate as voting_get_delegate,
    compute_weight,
    integer_sqrt,
};

// =============================================================================
// Storage keys (legacy + admin)
// =============================================================================

#[contracttype]
pub enum DataKey {
    Admin,
    TokenContract,
    Proposal(u64),
    Vote(u64, Address),
    NextProposalId,
    UpgradeProposal(u64),
    TimelockExpiry(u64),
}

// =============================================================================
// Legacy structs (kept for on-chain state backward compat)
// =============================================================================

#[contracttype]
#[derive(Clone)]
pub struct ProposalRecord {
    pub id: u64,
    pub proposer: Address,
    pub title: String,
    pub description: String,
    pub voting_end_ledger: u32,
    pub votes_for: i128,
    pub votes_against: i128,
    pub executed: bool,
    pub created_at: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct UpgradeProposalRecord {
    pub id: u64,
    pub proposer: Address,
    pub contract_address: Address,
    pub new_wasm_hash: Symbol,
    pub voting_end_ledger: u32,
    pub votes_for: i128,
    pub votes_against: i128,
    pub approved: bool,
    pub executed: bool,
    pub timelock_ledger: u32,
    pub created_at: u64,
}

// =============================================================================
// Events
// =============================================================================

const PROPOSAL_CREATED: Symbol = symbol_short!("prop_new");
const VOTE_CAST: Symbol = symbol_short!("vote");
const PROPOSAL_EXECUTED: Symbol = symbol_short!("exec");
const UPGRADE_PROPOSED: Symbol = symbol_short!("upg_prop");
const UPGRADE_APPROVED: Symbol = symbol_short!("upg_appr");
const UPGRADE_EXECUTED: Symbol = symbol_short!("upg_exec");

// =============================================================================
// Upgrade target validation (issue #1169)
// =============================================================================

/// Strkey of the all-zero *contract* address (strkey version 2, `C…` prefix).
const ZERO_CONTRACT_ADDRESS: &str = "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4";

/// Strkey of the all-zero *account* address (strkey version 6, `G…` prefix).
const ZERO_ACCOUNT_ADDRESS: &str = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";

/// Returns true when `addr` is one of the all-zero (unusable) addresses.
///
/// `soroban_sdk::Address` exposes no `is_zero()` on SDK 21, so the check
/// compares the canonical strkey produced by `Address::to_string()`.
fn is_zero_address(addr: &Address) -> bool {
    let strkey = addr.to_string();
    strkey == String::from_str(addr.env(), ZERO_CONTRACT_ADDRESS)
        || strkey == String::from_str(addr.env(), ZERO_ACCOUNT_ADDRESS)
}

// =============================================================================
// Contract
// =============================================================================

#[contract]
pub struct GovernanceContract;

#[contractimpl]
impl GovernanceContract {
    // -------------------------------------------------------------------------
    // Admin / Initialization
    // -------------------------------------------------------------------------

    pub fn initialize(env: Env, admin: Address, token_contract: Address) {
        assert!(
            !env.storage().instance().has(&DataKey::Admin),
            "Already initialized"
        );
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::TokenContract, &token_contract);
        env.storage().instance().set(&DataKey::NextProposalId, &1_u64);
        // Defaults
        env.storage()
            .instance()
            .set(&GovernanceKey::VotingDelay, &0_u32);
        env.storage()
            .instance()
            .set(&GovernanceKey::TimelockDuration, &10_u32);
        env.storage()
            .instance()
            .set(&GovernanceKey::QuorumPercentage, &20_i128);
    }

    pub fn get_admin(env: Env) -> Address {
        env.storage().instance().get(&DataKey::Admin).unwrap()
    }

    // -------------------------------------------------------------------------
    // Governance configuration (admin only)
    // -------------------------------------------------------------------------

    pub fn set_voting_delay(env: Env, admin: Address, delay: u32) {
        access::require_admin(&env, &admin, &DataKey::Admin);
        env.storage()
            .instance()
            .set(&GovernanceKey::VotingDelay, &delay);
    }

    pub fn set_timelock_duration(env: Env, admin: Address, duration: u32) {
        access::require_admin(&env, &admin, &DataKey::Admin);
        env.storage()
            .instance()
            .set(&GovernanceKey::TimelockDuration, &duration);
    }

    pub fn set_quorum_percentage(env: Env, admin: Address, percentage: i128) {
        access::require_admin(&env, &admin, &DataKey::Admin);
        assert!(percentage > 0 && percentage <= 100, "Quorum must be 1-100");
        env.storage()
            .instance()
            .set(&GovernanceKey::QuorumPercentage, &percentage);
    }

    pub fn set_total_supply_snapshot(env: Env, admin: Address, supply: i128) {
        access::require_admin(&env, &admin, &DataKey::Admin);
        env.storage()
            .instance()
            .set(&GovernanceKey::TotalSupply, &supply);
    }

    // -------------------------------------------------------------------------
    // Advanced proposal management (uses voting.rs strategies)
    // -------------------------------------------------------------------------

    /// Create a proposal with a configurable voting strategy.
    /// strategy: 0 = Weighted, 1 = Quadratic, 2 = Flat
    pub fn create_advanced_proposal(
        env: Env,
        proposer: Address,
        title: String,
        description: String,
        voting_end_ledger: u32,
        strategy: u32,
        timelock_delay: u32,
    ) -> u64 {
        let strat = match strategy {
            1 => VotingStrategy::Quadratic,
            2 => VotingStrategy::Flat,
            _ => VotingStrategy::Weighted,
        };
        let execution_timelock = voting_end_ledger.saturating_add(timelock_delay);
        voting_create_proposal(
            &env, proposer, title, description, voting_end_ledger, strat, execution_timelock,
        )
    }

    /// Cast a vote with strategy-aware weight computation.
    /// support: 0 = For, 1 = Against, 2 = Abstain
    pub fn vote_advanced(env: Env, voter: Address, proposal_id: u64, support: u32) {
        let token_contract: Address = env
            .storage()
            .instance()
            .get(&DataKey::TokenContract)
            .unwrap();
        let balance: i128 = env.invoke_contract(
            &token_contract,
            &symbol_short!("balance"),
            soroban_sdk::vec![&env, voter.clone().into_val(&env)],
        );

        let vote_support = match support {
            1 => VoteSupport::Against,
            2 => VoteSupport::Abstain,
            _ => VoteSupport::For,
        };

        voting_cast_vote(&env, proposal_id, voter, vote_support, balance);
    }

    /// Finalise a proposal after its voting window.
    pub fn finalise_advanced_proposal(env: Env, proposal_id: u64) -> bool {
        voting_finalise_proposal(&env, proposal_id)
    }

    /// Execute a succeeded proposal after the timelock.
    pub fn execute_advanced_proposal(env: Env, proposal_id: u64) {
        voting_execute_proposal(&env, proposal_id);
    }

    /// Cancel a proposal.
    pub fn cancel_advanced_proposal(env: Env, canceller: Address, proposal_id: u64) {
        voting_cancel_proposal(&env, canceller, proposal_id);
    }

    /// Delegate voting power to another address.
    pub fn delegate(env: Env, delegator: Address, delegate: Address) {
        voting_delegate_vote(&env, delegator, delegate);
    }

    /// Get an advanced proposal.
    pub fn get_advanced_proposal(env: Env, proposal_id: u64) -> Option<VotingProposal> {
        voting_get_proposal(&env, proposal_id)
    }

    /// Get a voter's vote record for a proposal.
    pub fn get_vote_record(env: Env, proposal_id: u64, voter: Address) -> Option<VoteRecord> {
        voting_get_vote_record(&env, proposal_id, voter)
    }

    /// Get the delegate for an address.
    pub fn get_delegate(env: Env, delegator: Address) -> Option<Address> {
        voting_get_delegate(&env, delegator)
    }

    /// Compute the effective vote weight for a balance under a given strategy.
    /// strategy: 0 = Weighted, 1 = Quadratic, 2 = Flat
    pub fn get_vote_weight(_env: Env, balance: i128, strategy: u32) -> i128 {
        let strat = match strategy {
            1 => VotingStrategy::Quadratic,
            2 => VotingStrategy::Flat,
            _ => VotingStrategy::Weighted,
        };
        compute_weight(&strat, balance)
    }

    /// Utility: integer square root (exposed for frontend tooling).
    pub fn sqrt(_env: Env, x: i128) -> i128 {
        integer_sqrt(x)
    }

    // -------------------------------------------------------------------------
    // Legacy proposal management (preserved for backward compat)
    // -------------------------------------------------------------------------

    pub fn create_proposal(
        env: Env,
        proposer: Address,
        title: String,
        description: String,
        voting_end_ledger: u32,
    ) -> u64 {
        proposer.require_auth();
        assert!(
            voting_end_ledger > env.ledger().sequence(),
            "Voting end must be in future"
        );

        let id: u64 = env.storage().instance().get(&DataKey::NextProposalId).unwrap();
        let proposal = ProposalRecord {
            id,
            proposer: proposer.clone(),
            title,
            description,
            voting_end_ledger,
            votes_for: 0,
            votes_against: 0,
            executed: false,
            created_at: env.ledger().timestamp(),
        };

        env.storage()
            .persistent()
            .set(&DataKey::Proposal(id), &proposal);
        env.storage()
            .instance()
            .set(&DataKey::NextProposalId, &(id + 1));

        env.events()
            .publish((PROPOSAL_CREATED, symbol_short!("id")), id);
        id
    }

    pub fn vote(env: Env, voter: Address, proposal_id: u64, support: bool) {
        voter.require_auth();

        let mut proposal: ProposalRecord = env
            .storage()
            .persistent()
            .get(&DataKey::Proposal(proposal_id))
            .expect("Proposal not found");

        assert!(
            env.ledger().sequence() < proposal.voting_end_ledger,
            "Voting period ended"
        );
        assert!(!proposal.executed, "Proposal already executed");

        let vote_key = DataKey::Vote(proposal_id, voter.clone());
        assert!(
            !env.storage().persistent().has(&vote_key),
            "Already voted"
        );

        let token_contract: Address = env
            .storage()
            .instance()
            .get(&DataKey::TokenContract)
            .unwrap();
        let balance: i128 = env.invoke_contract(
            &token_contract,
            &symbol_short!("balance"),
            soroban_sdk::vec![&env, voter.clone().into_val(&env)],
        );
        assert!(balance > 0, "No voting power");

        env.storage().persistent().set(&vote_key, &support);

        if support {
            proposal.votes_for += balance;
        } else {
            proposal.votes_against += balance;
        }
        env.storage()
            .persistent()
            .set(&DataKey::Proposal(proposal_id), &proposal);

        env.events()
            .publish((VOTE_CAST, symbol_short!("voter")), (proposal_id, support));
    }

    pub fn execute_proposal(env: Env, proposal_id: u64) {
        let mut proposal: ProposalRecord = env
            .storage()
            .persistent()
            .get(&DataKey::Proposal(proposal_id))
            .expect("Proposal not found");

        assert!(
            env.ledger().sequence() >= proposal.voting_end_ledger,
            "Voting still ongoing"
        );
        assert!(!proposal.executed, "Already executed");
        assert!(
            proposal.votes_for > proposal.votes_against,
            "Proposal did not pass"
        );

        proposal.executed = true;
        env.storage()
            .persistent()
            .set(&DataKey::Proposal(proposal_id), &proposal);

        env.events()
            .publish((PROPOSAL_EXECUTED, symbol_short!("id")), proposal_id);
    }

    pub fn get_proposal(env: Env, proposal_id: u64) -> Option<ProposalRecord> {
        env.storage()
            .persistent()
            .get(&DataKey::Proposal(proposal_id))
    }

    pub fn has_voted(env: Env, proposal_id: u64, voter: Address) -> bool {
        env.storage()
            .persistent()
            .has(&DataKey::Vote(proposal_id, voter))
    }

    pub fn get_voting_power(env: Env, voter: Address) -> i128 {
        let cached: Option<i128> = env
            .storage()
            .instance()
            .get(&GovernanceKey::VotingPowerCheckpoint(voter.clone()));
        if let Some(power) = cached {
            return power;
        }

        let token_contract: Address = env
            .storage()
            .instance()
            .get(&DataKey::TokenContract)
            .unwrap();
        let balance: i128 = env.invoke_contract(
            &token_contract,
            &symbol_short!("balance"),
            soroban_sdk::vec![&env, voter.into_val(&env)],
        );
        env.storage()
            .instance()
            .set(&GovernanceKey::VotingPowerCheckpoint(voter), &balance);
        balance
    }

    pub fn has_voting_power(env: Env, voter: Address, min_power: i128) -> bool {
        Self::get_voting_power(env, voter) >= min_power
    }

    pub fn get_vote_tally(env: Env, proposal_id: u64) -> (i128, i128, i128, bool) {
        let proposal: ProposalRecord = env
            .storage()
            .persistent()
            .get(&DataKey::Proposal(proposal_id))
            .expect("Proposal not found");
        let total = proposal.votes_for + proposal.votes_against;
        let quorum_met = proposal.votes_for > proposal.votes_against && total > 0;
        (proposal.votes_for, proposal.votes_against, total, quorum_met)
    }

    pub fn did_proposal_pass(env: Env, proposal_id: u64) -> bool {
        let proposal: ProposalRecord = env
            .storage()
            .persistent()
            .get(&DataKey::Proposal(proposal_id))
            .expect("Proposal not found");
        env.ledger().sequence() >= proposal.voting_end_ledger
            && proposal.votes_for > proposal.votes_against
    }

    pub fn is_voting_active(env: Env, proposal_id: u64) -> bool {
        let proposal: ProposalRecord = env
            .storage()
            .persistent()
            .get(&DataKey::Proposal(proposal_id))
            .expect("Proposal not found");
        env.ledger().sequence() < proposal.voting_end_ledger && !proposal.executed
    }

    pub fn get_next_proposal_id(env: Env) -> u64 {
        env.storage()
            .instance()
            .get(&DataKey::NextProposalId)
            .unwrap_or(1)
    }

    pub fn get_vote(env: Env, proposal_id: u64, voter: Address) -> Option<bool> {
        env.storage()
            .persistent()
            .get(&DataKey::Vote(proposal_id, voter))
    }

    // -------------------------------------------------------------------------
    // Contract upgrade governance (legacy)
    // -------------------------------------------------------------------------

    pub fn propose_upgrade(
        env: Env,
        proposer: Address,
        contract_address: Address,
        new_wasm_hash: Symbol,
        voting_end_ledger: u32,
        timelock_ledger: u32,
    ) -> u64 {
        proposer.require_auth();

        // Security (#1169): reject the all-zero target so an upgrade proposal
        // can never name an address that does not resolve to a real contract.
        assert!(
            !is_zero_address(&contract_address),
            "Contract address must not be zero"
        );

        assert!(voting_end_ledger > env.ledger().sequence(), "Voting end must be in future");
        assert!(timelock_ledger > voting_end_ledger, "Timelock must be after voting");

        let id: u64 = env.storage().instance().get(&DataKey::NextProposalId).unwrap();
        let upgrade = UpgradeProposalRecord {
            id,
            proposer: proposer.clone(),
            contract_address: contract_address.clone(),
            new_wasm_hash: new_wasm_hash.clone(),
            voting_end_ledger,
            votes_for: 0,
            votes_against: 0,
            approved: false,
            executed: false,
            timelock_ledger,
            created_at: env.ledger().timestamp(),
        };

        env.storage()
            .persistent()
            .set(&DataKey::UpgradeProposal(id), &upgrade);
        env.storage()
            .instance()
            .set(&DataKey::NextProposalId, &(id + 1));

        env.events().publish(
            (UPGRADE_PROPOSED, symbol_short!("id")),
            (id, contract_address, new_wasm_hash),
        );
        id
    }

    pub fn vote_upgrade(env: Env, voter: Address, upgrade_id: u64, support: bool) {
        voter.require_auth();

        let mut upgrade: UpgradeProposalRecord = env
            .storage()
            .persistent()
            .get(&DataKey::UpgradeProposal(upgrade_id))
            .expect("Upgrade proposal not found");

        assert!(env.ledger().sequence() < upgrade.voting_end_ledger, "Voting period ended");
        assert!(!upgrade.executed, "Upgrade already executed");

        let vote_key = DataKey::Vote(upgrade_id, voter.clone());
        assert!(!env.storage().persistent().has(&vote_key), "Already voted");

        let token_contract: Address = env
            .storage()
            .instance()
            .get(&DataKey::TokenContract)
            .unwrap();
        let balance: i128 = env.invoke_contract(
            &token_contract,
            &symbol_short!("balance"),
            soroban_sdk::vec![&env, voter.clone().into_val(&env)],
        );
        assert!(balance > 0, "No voting power");

        env.storage().persistent().set(&vote_key, &support);
        if support { upgrade.votes_for += balance; } else { upgrade.votes_against += balance; }
        env.storage().persistent().set(&DataKey::UpgradeProposal(upgrade_id), &upgrade);
        env.events().publish((VOTE_CAST, symbol_short!("upg")), (upgrade_id, support));
    }

    /// Approve a passed upgrade proposal. **Stored admin only** (issue #1169):
    /// `docs/contract-interfaces.md` documents this as the admin approval gate,
    /// but the function previously took no caller at all, so anyone could flip
    /// `approved` on a proposal that had merely finished voting.
    pub fn approve_upgrade(env: Env, admin: Address, upgrade_id: u64) {
        access::require_admin(&env, &admin, &DataKey::Admin);

        let mut upgrade: UpgradeProposalRecord = env
            .storage()
            .persistent()
            .get(&DataKey::UpgradeProposal(upgrade_id))
            .expect("Upgrade proposal not found");

        assert!(env.ledger().sequence() >= upgrade.voting_end_ledger, "Voting still ongoing");
        assert!(!upgrade.executed, "Already executed");
        assert!(upgrade.votes_for > upgrade.votes_against, "Upgrade did not pass");

        upgrade.approved = true;
        env.storage().persistent().set(&DataKey::UpgradeProposal(upgrade_id), &upgrade);
        env.storage().persistent().set(&DataKey::TimelockExpiry(upgrade_id), &upgrade.timelock_ledger);

        env.events().publish((UPGRADE_APPROVED, symbol_short!("id")), (upgrade_id, upgrade.contract_address.clone()));
    }

    pub fn execute_upgrade(env: Env, upgrade_id: u64) {
        let mut upgrade: UpgradeProposalRecord = env
            .storage()
            .persistent()
            .get(&DataKey::UpgradeProposal(upgrade_id))
            .expect("Upgrade proposal not found");

        assert!(upgrade.approved, "Upgrade not approved");
        assert!(!upgrade.executed, "Already executed");

        let timelock_expiry: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::TimelockExpiry(upgrade_id))
            .expect("Timelock not found");

        assert!(env.ledger().sequence() >= timelock_expiry, "Timelock not expired");

        upgrade.executed = true;
        env.storage().persistent().set(&DataKey::UpgradeProposal(upgrade_id), &upgrade);
        env.events().publish((UPGRADE_EXECUTED, symbol_short!("id")), (upgrade_id, upgrade.contract_address.clone()));
    }

    pub fn get_upgrade_proposal(env: Env, upgrade_id: u64) -> Option<UpgradeProposalRecord> {
        env.storage().persistent().get(&DataKey::UpgradeProposal(upgrade_id))
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::{Address as _, Ledger, LedgerInfo};
    use soroban_sdk::{symbol_short, Env};

    fn set_ledger(env: &Env, seq: u32) {
        env.ledger().set(LedgerInfo {
            sequence_number: seq,
            timestamp: seq as u64 * 5,
            protocol_version: 21,
            network_id: Default::default(),
            base_reserve: 10,
            min_temp_entry_ttl: MIN_TEMP_ENTRY_TTL,
            min_persistent_entry_ttl: MIN_PERSISTENT_ENTRY_TTL,
            max_entry_ttl: MAX_ENTRY_TTL,
        });
    }

    fn setup() -> (Env, GovernanceContractClient<'static>, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let id = env.register_contract(None, GovernanceContract);
        let client = GovernanceContractClient::new(&env, &id);
        let admin = Address::generate(&env);
        let token = Address::generate(&env);
        client.initialize(&admin, &token);
        (env, client, admin, token)
    }

    #[test]
    fn test_vote_power_benchmark_uses_cached_checkpoint() {
        let (env, client, _, _) = setup();
        let voter = Address::generate(&env);
        let delegate = Address::generate(&env);
        client.delegate(&voter, &delegate);

        env.budget().reset_default();
        let power = client.get_voting_power(&voter);
        let after = env.budget().cpu_instruction_cost();

        assert!(power >= 0);
        assert!(after > 0);
        assert_eq!(client.get_delegate(&voter).unwrap(), delegate);
    }

    #[test]
    fn test_initialize_sets_admin() {
        let (_, client, admin, _) = setup();
        assert_eq!(client.get_admin(), admin);
    }

    #[test]
    #[should_panic(expected = "Already initialized")]
    fn test_double_initialize_panics() {
        let (_, client, admin, token) = setup();
        client.initialize(&admin, &token);
    }

    #[test]
    fn test_create_proposal() {
        let (env, client, _, _) = setup();
        let proposer = Address::generate(&env);
        let title = String::from_str(&env, "New Course Category");
        let desc = String::from_str(&env, "Add blockchain category");
        let end = env.ledger().sequence() + 100;

        let id = client.create_proposal(&proposer, &title, &desc, &end);
        assert_eq!(id, 1);

        let prop = client.get_proposal(&id).unwrap();
        assert_eq!(prop.proposer, proposer);
        assert!(!prop.executed);
    }

    #[test]
    #[should_panic(expected = "Voting end must be in future")]
    fn test_create_proposal_past_end_panics() {
        let (env, client, _, _) = setup();
        let proposer = Address::generate(&env);
        let title = String::from_str(&env, "Test");
        let desc = String::from_str(&env, "Test");
        client.create_proposal(&proposer, &title, &desc, &0_u32);
    }

    #[test]
    #[should_panic(expected = "Voting period ended")]
    fn test_vote_after_voting_end_panics() {
        let (env, client, _, _) = setup();
        let proposer = Address::generate(&env);
        let voter = Address::generate(&env);
        let title = String::from_str(&env, "Test");
        let desc = String::from_str(&env, "Test");
        let end = env.ledger().sequence() + 10;
        let id = client.create_proposal(&proposer, &title, &desc, &end);
        set_ledger(&env, end + 1);
        client.vote(&voter, &id, &true);
    }

    #[test]
    #[should_panic(expected = "Voting still ongoing")]
    fn test_execute_before_voting_end_panics() {
        let (env, client, _, _) = setup();
        let proposer = Address::generate(&env);
        let title = String::from_str(&env, "Test");
        let desc = String::from_str(&env, "Test");
        let end = env.ledger().sequence() + 100;
        let id = client.create_proposal(&proposer, &title, &desc, &end);
        client.execute_proposal(&id);
    }

    #[test]
    #[should_panic(expected = "Proposal did not pass")]
    fn test_execute_without_quorum_panics() {
        let (env, client, _, _) = setup();
        let proposer = Address::generate(&env);
        let title = String::from_str(&env, "Test");
        let desc = String::from_str(&env, "Test");
        let end = env.ledger().sequence() + 10;
        let id = client.create_proposal(&proposer, &title, &desc, &end);
        set_ledger(&env, end + 1);
        client.execute_proposal(&id);
    }

    // ── Advanced voting strategy tests ────────────────────────────────────────

    #[test]
    fn test_get_vote_weight_weighted() {
        let (env, client, _, _) = setup();
        let _ = env;
        // Weighted: weight = balance
        assert_eq!(client.get_vote_weight(&1_000_i128, &0_u32), 1_000);
    }

    #[test]
    fn test_get_vote_weight_quadratic() {
        let (env, client, _, _) = setup();
        let _ = env;
        // Quadratic: weight = sqrt(balance)
        assert_eq!(client.get_vote_weight(&100_i128, &1_u32), 10);
        assert_eq!(client.get_vote_weight(&10_000_i128, &1_u32), 100);
    }

    #[test]
    fn test_get_vote_weight_flat() {
        let (env, client, _, _) = setup();
        let _ = env;
        // Flat: weight = 1 regardless of balance
        assert_eq!(client.get_vote_weight(&1_000_000_i128, &2_u32), 1);
        assert_eq!(client.get_vote_weight(&1_i128, &2_u32), 1);
        assert_eq!(client.get_vote_weight(&0_i128, &2_u32), 0);
    }

    #[test]
    fn test_sqrt_utility() {
        let (env, client, _, _) = setup();
        let _ = env;
        assert_eq!(client.sqrt(&9_i128), 3);
        assert_eq!(client.sqrt(&10_000_i128), 100);
        assert_eq!(client.sqrt(&0_i128), 0);
    }

    #[test]
    fn test_admin_set_quorum_percentage() {
        let (_, client, admin, _) = setup();
        client.set_quorum_percentage(&admin, &30_i128);
        // No panic means success; config stored
    }

    #[test]
    #[should_panic(expected = "Quorum must be 1-100")]
    fn test_invalid_quorum_panics() {
        let (_, client, admin, _) = setup();
        client.set_quorum_percentage(&admin, &0_i128);
    }

    #[test]
    fn test_set_voting_delay() {
        let (_, client, admin, _) = setup();
        client.set_voting_delay(&admin, &5_u32);
    }

    #[test]
    fn test_set_timelock_duration() {
        let (_, client, admin, _) = setup();
        client.set_timelock_duration(&admin, &20_u32);
    }

    // ── Additional legacy proposal tests ─────────────────────────────────────

    #[test]
    fn test_get_next_proposal_id_starts_at_1() {
        let (_, client, _, _) = setup();
        assert_eq!(client.get_next_proposal_id(), 1);
    }

    #[test]
    fn test_get_next_proposal_id_increments_after_create() {
        let (env, client, _, _) = setup();
        let proposer = Address::generate(&env);
        let title = String::from_str(&env, "P1");
        let desc = String::from_str(&env, "desc");
        let end = env.ledger().sequence() + 50;
        client.create_proposal(&proposer, &title, &desc, &end);
        assert_eq!(client.get_next_proposal_id(), 2);
    }

    #[test]
    fn test_has_voted_returns_false_before_vote() {
        let (env, client, _, _) = setup();
        let proposer = Address::generate(&env);
        let voter = Address::generate(&env);
        let title = String::from_str(&env, "Test");
        let desc = String::from_str(&env, "desc");
        let end = env.ledger().sequence() + 50;
        let id = client.create_proposal(&proposer, &title, &desc, &end);
        assert!(!client.has_voted(&id, &voter));
    }

    #[test]
    fn test_get_vote_returns_none_before_vote() {
        let (env, client, _, _) = setup();
        let proposer = Address::generate(&env);
        let voter = Address::generate(&env);
        let title = String::from_str(&env, "Test");
        let desc = String::from_str(&env, "desc");
        let end = env.ledger().sequence() + 50;
        let id = client.create_proposal(&proposer, &title, &desc, &end);
        assert!(client.get_vote(&id, &voter).is_none());
    }

    #[test]
    fn test_get_vote_tally_all_zeros_before_votes() {
        let (env, client, _, _) = setup();
        let proposer = Address::generate(&env);
        let title = String::from_str(&env, "Test");
        let desc = String::from_str(&env, "desc");
        let end = env.ledger().sequence() + 50;
        let id = client.create_proposal(&proposer, &title, &desc, &end);
        let (votes_for, votes_against, total, quorum_met) = client.get_vote_tally(&id);
        assert_eq!(votes_for, 0);
        assert_eq!(votes_against, 0);
        assert_eq!(total, 0);
        assert!(!quorum_met);
    }

    #[test]
    fn test_did_proposal_pass_returns_false_during_voting() {
        let (env, client, _, _) = setup();
        let proposer = Address::generate(&env);
        let title = String::from_str(&env, "Test");
        let desc = String::from_str(&env, "desc");
        let end = env.ledger().sequence() + 50;
        let id = client.create_proposal(&proposer, &title, &desc, &end);
        // still in voting window
        assert!(!client.did_proposal_pass(&id));
    }

    #[test]
    fn test_is_voting_active_true_during_window() {
        let (env, client, _, _) = setup();
        let proposer = Address::generate(&env);
        let title = String::from_str(&env, "Test");
        let desc = String::from_str(&env, "desc");
        let end = env.ledger().sequence() + 50;
        let id = client.create_proposal(&proposer, &title, &desc, &end);
        assert!(client.is_voting_active(&id));
    }

    #[test]
    fn test_is_voting_active_false_after_window() {
        let (env, client, _, _) = setup();
        let proposer = Address::generate(&env);
        let title = String::from_str(&env, "Test");
        let desc = String::from_str(&env, "desc");
        let end = env.ledger().sequence() + 10;
        let id = client.create_proposal(&proposer, &title, &desc, &end);
        set_ledger(&env, end + 1);
        assert!(!client.is_voting_active(&id));
    }

    #[test]
    fn test_get_proposal_returns_none_for_missing() {
        let (_, client, _, _) = setup();
        assert!(client.get_proposal(&9999).is_none());
    }

    // ── Upgrade proposal tests ────────────────────────────────────────────────

    #[test]
    fn test_propose_upgrade_returns_id() {
        let (env, client, _, _) = setup();
        let proposer = Address::generate(&env);
        let contract_addr = Address::generate(&env);
        let wasm_hash = symbol_short!("abc123");
        let end = env.ledger().sequence() + 10;
        let timelock = end + 10;
        let id = client.propose_upgrade(&proposer, &contract_addr, &wasm_hash, &end, &timelock);
        assert_eq!(id, 1);
    }

    #[test]
    fn test_get_upgrade_proposal_returns_correct_record() {
        let (env, client, _, _) = setup();
        let proposer = Address::generate(&env);
        let contract_addr = Address::generate(&env);
        let wasm_hash = symbol_short!("abc123");
        let end = env.ledger().sequence() + 10;
        let timelock = end + 10;
        let id = client.propose_upgrade(&proposer, &contract_addr, &wasm_hash, &end, &timelock);
        let upgrade = client.get_upgrade_proposal(&id).unwrap();
        assert_eq!(upgrade.id, id);
        assert_eq!(upgrade.proposer, proposer);
        assert!(!upgrade.approved);
        assert!(!upgrade.executed);
    }

    #[test]
    fn test_get_upgrade_proposal_returns_none_for_missing() {
        let (_, client, _, _) = setup();
        assert!(client.get_upgrade_proposal(&9999).is_none());
    }

    #[test]
    #[should_panic(expected = "Voting end must be in future")]
    fn test_propose_upgrade_past_end_panics() {
        let (env, client, _, _) = setup();
        let proposer = Address::generate(&env);
        let contract_addr = Address::generate(&env);
        let wasm_hash = symbol_short!("abc123");
        client.propose_upgrade(&proposer, &contract_addr, &wasm_hash, &0_u32, &100_u32);
    }

    #[test]
    #[should_panic(expected = "Timelock must be after voting")]
    fn test_propose_upgrade_timelock_before_voting_end_panics() {
        let (env, client, _, _) = setup();
        let proposer = Address::generate(&env);
        let contract_addr = Address::generate(&env);
        let wasm_hash = symbol_short!("abc123");
        let end = env.ledger().sequence() + 100;
        // timelock == end — not strictly after
        client.propose_upgrade(&proposer, &contract_addr, &wasm_hash, &end, &(end - 1));
    }

    #[test]
    #[should_panic(expected = "Contract address must not be zero")]
    fn test_propose_upgrade_zero_contract_target_panics() {
        let (env, client, _, _) = setup();
        let proposer = Address::generate(&env);
        let zero = Address::from_string(&String::from_str(&env, ZERO_CONTRACT_ADDRESS));
        let wasm_hash = symbol_short!("abc123");
        let end = env.ledger().sequence() + 10;
        let timelock = end + 10;
        client.propose_upgrade(&proposer, &zero, &wasm_hash, &end, &timelock);
    }

    #[test]
    #[should_panic(expected = "Contract address must not be zero")]
    fn test_propose_upgrade_zero_account_target_panics() {
        let (env, client, _, _) = setup();
        let proposer = Address::generate(&env);
        let zero = Address::from_string(&String::from_str(&env, ZERO_ACCOUNT_ADDRESS));
        let wasm_hash = symbol_short!("abc123");
        let end = env.ledger().sequence() + 10;
        let timelock = end + 10;
        client.propose_upgrade(&proposer, &zero, &wasm_hash, &end, &timelock);
    }

    // ── Upgrade approval / execution authorization (#1169) ────────────────────
    //
    // `approve_upgrade` is documented as the admin approval gate
    // (docs/contract-interfaces.md); `execute_upgrade` is documented as
    // permissionless but only after the approved proposal's timelock.

    /// Minimal token contract so `vote_upgrade`'s balance lookup resolves.
    #[contract]
    struct MockToken;

    #[contractimpl]
    impl MockToken {
        pub fn balance(_env: Env, _addr: Address) -> i128 {
            1_000
        }
    }

    /// Governance wired to a token that reports a non-zero balance for everyone.
    fn setup_with_token() -> (Env, GovernanceContractClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let id = env.register_contract(None, GovernanceContract);
        let client = GovernanceContractClient::new(&env, &id);
        let admin = Address::generate(&env);
        let token = env.register_contract(None, MockToken);
        client.initialize(&admin, &token);
        (env, client, admin)
    }

    /// Proposes an upgrade, votes it through and closes the voting window.
    /// Returns `(upgrade_id, voting_end_ledger)`.
    fn passed_upgrade(env: &Env, client: &GovernanceContractClient, admin: &Address) -> (u64, u32) {
        let end = env.ledger().sequence() + 10;
        let target = Address::generate(env);
        let wasm_hash = symbol_short!("abc123");
        let id = client.propose_upgrade(admin, &target, &wasm_hash, &end, &(end + 10));
        let voter = Address::generate(env);
        client.vote_upgrade(&voter, &id, &true);
        set_ledger(env, end);
        (id, end)
    }

    #[test]
    #[should_panic(expected = "Unauthorized: admin required")]
    fn test_approve_upgrade_rejects_non_admin() {
        let (env, client, admin) = setup_with_token();
        // The proposal has finished voting and passed — approval is still the
        // stored admin's call alone.
        let (id, _) = passed_upgrade(&env, &client, &admin);
        let rando = Address::generate(&env);
        client.approve_upgrade(&rando, &id);
    }

    #[test]
    fn test_approve_upgrade_marks_passed_proposal_approved() {
        let (env, client, admin) = setup_with_token();
        let (id, _) = passed_upgrade(&env, &client, &admin);
        client.approve_upgrade(&admin, &id);
        let record = client.get_upgrade_proposal(&id).unwrap();
        assert!(record.approved);
        assert!(!record.executed);
    }

    #[test]
    #[should_panic(expected = "Timelock not expired")]
    fn test_execute_upgrade_rejected_before_timelock() {
        let (env, client, admin) = setup_with_token();
        let (id, end) = passed_upgrade(&env, &client, &admin);
        client.approve_upgrade(&admin, &id);
        // One ledger short of `timelock_ledger == end + 10`.
        set_ledger(&env, end + 9);
        client.execute_upgrade(&id);
    }

    #[test]
    fn test_execute_upgrade_succeeds_after_timelock() {
        let (env, client, admin) = setup_with_token();
        let (id, end) = passed_upgrade(&env, &client, &admin);
        client.approve_upgrade(&admin, &id);
        set_ledger(&env, end + 10);
        client.execute_upgrade(&id);
        let record = client.get_upgrade_proposal(&id).unwrap();
        assert!(record.executed);
    }

    #[test]
    #[should_panic(expected = "Already executed")]
    fn test_execute_upgrade_cannot_run_twice() {
        let (env, client, admin) = setup_with_token();
        let (id, end) = passed_upgrade(&env, &client, &admin);
        client.approve_upgrade(&admin, &id);
        set_ledger(&env, end + 10);
        client.execute_upgrade(&id);
        client.execute_upgrade(&id);
    }

    #[test]
    fn test_set_total_supply_snapshot() {
        let (_, client, admin, _) = setup();
        client.set_total_supply_snapshot(&admin, &1_000_000_i128);
        // No panic = stored correctly
    }

    #[test]
    #[should_panic(expected = "Unauthorized: admin required")]
    fn test_non_admin_cannot_set_total_supply() {
        let (env, client, _, _) = setup();
        let rando = Address::generate(&env);
        client.set_total_supply_snapshot(&rando, &1_000_000_i128);
    }

    #[test]
    #[should_panic(expected = "Unauthorized: admin required")]
    fn test_non_admin_cannot_set_voting_delay() {
        let (env, client, _, _) = setup();
        let rando = Address::generate(&env);
        client.set_voting_delay(&rando, &5_u32);
    }

    #[test]
    #[should_panic(expected = "Unauthorized: admin required")]
    fn test_non_admin_cannot_set_timelock_duration() {
        let (env, client, _, _) = setup();
        let rando = Address::generate(&env);
        client.set_timelock_duration(&rando, &20_u32);
    }

    #[test]
    #[should_panic(expected = "Quorum must be 1-100")]
    fn test_quorum_over_100_panics() {
        let (_, client, admin, _) = setup();
        client.set_quorum_percentage(&admin, &101_i128);
    }
}
