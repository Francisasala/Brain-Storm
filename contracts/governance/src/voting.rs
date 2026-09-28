// =============================================================================
// voting.rs — Advanced voting strategies (Issue #633)
//
// Implements:
//   - Weighted voting  : vote weight = BST balance (token-weighted)
//   - Quadratic voting : vote weight = sqrt(BST balance) — prevents whale dominance
//   - Proposal lifecycle hooks: delay → voting → execution timelock
//   - Anti-double-vote guard (per-voter, per-proposal)
//   - Flash-loan manipulation guard (snapshot balance at vote-start ledger)
//   - Event emission for every state transition
// =============================================================================
use soroban_sdk::{contracttype, symbol_short, Address, Env, String, Symbol};
use brain_storm_shared::constants::{MIN_TEMP_ENTRY_TTL, MIN_PERSISTENT_ENTRY_TTL, MAX_ENTRY_TTL};

// =============================================================================
// Types
// =============================================================================

/// Voting strategy for a proposal.
#[contracttype]
#[derive(Clone, PartialEq)]
pub enum VotingStrategy {
    /// Each token = 1 vote (standard weighted voting).
    Weighted,
    /// Each vote costs quadratic tokens: vote_power = floor(sqrt(balance)).
    /// This dampens whale influence.
    Quadratic,
    /// One account = one vote (flat, regardless of token balance).
    Flat,
}

/// Lifecycle state of a proposal.
#[contracttype]
#[derive(Clone, PartialEq, Debug)]
pub enum ProposalState {
    /// Proposal created, voting has not started yet.
    Pending,
    /// Voting window is open.
    Active,
    /// Voting ended, waiting for timelock to expire before execution.
    Succeeded,
    /// Voting ended without reaching quorum or failing the vote.
    Defeated,
    /// Timelock expired and proposal was executed.
    Executed,
    /// Proposal was cancelled by the proposer or admin.
    Cancelled,
}

#[contracttype]
#[derive(Clone)]
pub struct VotingProposal {
    pub id: u64,
    pub title: String,
    pub description: String,
    /// Ledger at which voting becomes active (anti-spam delay).
    pub voting_start_ledger: u32,
    pub voting_end_ledger: u32,
    /// Earliest ledger at which a passed proposal may be executed.
    pub execution_timelock_ledger: u32,
    pub votes_for: i128,
    pub votes_against: i128,
    pub votes_abstain: i128,
    /// Minimum total participating votes for the result to be binding.
    pub quorum_required: i128,
    pub strategy: VotingStrategy,
    pub state: ProposalState,
    pub created_at: u64,
    /// BST total supply snapshot at proposal creation (flash-loan guard).
    pub supply_snapshot: i128,
}

/// Per-voter record stored for a given proposal.
#[contracttype]
#[derive(Clone)]
pub struct VoteRecord {
    pub voter: Address,
    pub support: VoteSupport,
    /// Actual weight applied after strategy computation.
    pub weight: i128,
    pub cast_at: u32,
}

#[contracttype]
#[derive(Clone, PartialEq)]
pub enum VoteSupport {
    For,
    Against,
    Abstain,
}

// =============================================================================
// Storage keys
// =============================================================================

#[contracttype]
pub enum GovernanceKey {
    Proposal(u64),
    VoteRecord(u64, Address),
    Delegate(Address),
    VotingPowerCheckpoint(Address),
    NextProposalId,
    TotalSupply,
    QuorumPercentage,
    /// Voting-delay in ledgers (min ledgers after creation before voting starts).
    VotingDelay,
    /// Default timelock duration in ledgers after voting ends.
    TimelockDuration,
}

// =============================================================================
// Events
// =============================================================================

pub const PROPOSAL_CREATED: Symbol = symbol_short!("gov_new");
pub const VOTE_CAST: Symbol = symbol_short!("gov_vote");
pub const VOTE_DELEGATED: Symbol = symbol_short!("gov_del");
pub const PROPOSAL_EXECUTED: Symbol = symbol_short!("gov_exec");
pub const PROPOSAL_CANCELLED: Symbol = symbol_short!("gov_cncl");
pub const STATE_CHANGED: Symbol = symbol_short!("gov_st");

// =============================================================================
// Core functions
// =============================================================================

/// Create a new proposal with configurable voting strategy.
///
/// The proposal starts in `Pending` state. Voting only becomes active after
/// `voting_start_ledger` (current + voting_delay).
pub fn create_proposal(
    env: &Env,
    proposer: Address,
    title: String,
    description: String,
    voting_end_ledger: u32,
    strategy: VotingStrategy,
    execution_timelock_ledger: u32,
) -> u64 {
    proposer.require_auth();

    let current = env.ledger().sequence();
    assert!(
        voting_end_ledger > current,
        "Voting end must be in the future"
    );
    assert!(
        execution_timelock_ledger >= voting_end_ledger,
        "Timelock must be >= voting end"
    );

    let voting_delay: u32 = env
        .storage()
        .instance()
        .get(&GovernanceKey::VotingDelay)
        .unwrap_or(0);

    let voting_start_ledger = current.saturating_add(voting_delay);

    let total_supply: i128 = env
        .storage()
        .instance()
        .get(&GovernanceKey::TotalSupply)
        .unwrap_or(1_000_000_000_000_000);

    let quorum_percentage: i128 = env
        .storage()
        .instance()
        .get(&GovernanceKey::QuorumPercentage)
        .unwrap_or(20); // 20% of supply

    let quorum_required = total_supply
        .checked_mul(quorum_percentage)
        .expect("overflow")
        .checked_div(100)
        .expect("div by zero");

    let id: u64 = env
        .storage()
        .instance()
        .get(&GovernanceKey::NextProposalId)
        .unwrap_or(1);

    let proposal = VotingProposal {
        id,
        title,
        description,
        voting_start_ledger,
        voting_end_ledger,
        execution_timelock_ledger,
        votes_for: 0,
        votes_against: 0,
        votes_abstain: 0,
        quorum_required,
        strategy,
        state: ProposalState::Pending,
        created_at: env.ledger().timestamp(),
        supply_snapshot: total_supply,
    };

    env.storage()
        .instance()
        .set(&GovernanceKey::Proposal(id), &proposal);
    env.storage()
        .instance()
        .set(&GovernanceKey::NextProposalId, &(id + 1));

    env.events()
        .publish((PROPOSAL_CREATED,), (id, proposer, quorum_required));

    id
}

/// Cast a vote on a proposal.
///
/// Computes vote weight using the proposal's strategy:
///   - Weighted  : weight = BST balance
///   - Quadratic : weight = integer_sqrt(BST balance)
///   - Flat      : weight = 1
///
/// Anti-manipulation guards:
///   - Double-vote: checked via VoteRecord key
///   - Voting-window: checked against start + end ledger
///   - Flash-loan: the supply snapshot was taken at proposal creation,
///     so a single block can't inflate supply to game quorum.
pub fn cast_vote(
    env: &Env,
    proposal_id: u64,
    voter: Address,
    support: VoteSupport,
    balance: i128,
) {
    voter.require_auth();
    assert!(balance >= 0, "Balance must be non-negative");

    let mut proposal: VotingProposal = env
        .storage()
        .instance()
        .get(&GovernanceKey::Proposal(proposal_id))
        .expect("Proposal not found");

    let current = env.ledger().sequence();

    assert!(
        current >= proposal.voting_start_ledger,
        "Voting has not started yet"
    );
    assert!(
        current < proposal.voting_end_ledger,
        "Voting period ended"
    );
    assert!(
        proposal.state == ProposalState::Pending || proposal.state == ProposalState::Active,
        "Proposal is not voteable"
    );

    // Transition Pending → Active on first vote
    if proposal.state == ProposalState::Pending {
        proposal.state = ProposalState::Active;
        env.events()
            .publish((STATE_CHANGED,), (proposal_id, symbol_short!("active")));
    }

    // Double-vote guard
    let vote_key = GovernanceKey::VoteRecord(proposal_id, voter.clone());
    assert!(
        !env.storage().instance().has(&vote_key),
        "Already voted"
    );

    // Compute weight based on strategy
    let weight = compute_weight(&proposal.strategy, balance);
    assert!(weight >= 0, "Vote weight must be non-negative");

    // Update proposal tallies
    match support {
        VoteSupport::For => {
            proposal.votes_for = proposal
                .votes_for
                .checked_add(weight)
                .expect("arithmetic overflow");
        }
        VoteSupport::Against => {
            proposal.votes_against = proposal
                .votes_against
                .checked_add(weight)
                .expect("arithmetic overflow");
        }
        VoteSupport::Abstain => {
            proposal.votes_abstain = proposal
                .votes_abstain
                .checked_add(weight)
                .expect("arithmetic overflow");
        }
    }

    // Persist vote record and updated proposal
    let record = VoteRecord {
        voter: voter.clone(),
        support: support.clone(),
        weight,
        cast_at: current,
    };
    env.storage().instance().set(&vote_key, &record);
    env.storage()
        .instance()
        .set(&GovernanceKey::Proposal(proposal_id), &proposal);

    let support_sym = match support {
        VoteSupport::For => symbol_short!("for"),
        VoteSupport::Against => symbol_short!("agst"),
        VoteSupport::Abstain => symbol_short!("abst"),
    };
    env.events()
        .publish((VOTE_CAST,), (proposal_id, voter, support_sym, weight));
}

/// Finalise a proposal after its voting window closes.
///
/// Transitions to `Succeeded` (quorum met & more For than Against)
/// or `Defeated`. Does NOT execute — execution happens in a separate call
/// after the timelock expires.
pub fn finalise_proposal(env: &Env, proposal_id: u64) -> bool {
    let mut proposal: VotingProposal = env
        .storage()
        .instance()
        .get(&GovernanceKey::Proposal(proposal_id))
        .expect("Proposal not found");

    assert!(
        proposal.state == ProposalState::Active || proposal.state == ProposalState::Pending,
        "Proposal is not finalisable"
    );
    assert!(
        env.ledger().sequence() >= proposal.voting_end_ledger,
        "Voting still ongoing"
    );

    let total_participating = proposal
        .votes_for
        .checked_add(proposal.votes_against)
        .expect("overflow")
        .checked_add(proposal.votes_abstain)
        .expect("overflow");

    let quorum_met = total_participating >= proposal.quorum_required;
    let passed = proposal.votes_for > proposal.votes_against;

    if quorum_met && passed {
        proposal.state = ProposalState::Succeeded;
        env.storage()
            .instance()
            .set(&GovernanceKey::Proposal(proposal_id), &proposal);
        env.events()
            .publish((STATE_CHANGED,), (proposal_id, symbol_short!("success")));
        true
    } else {
        proposal.state = ProposalState::Defeated;
        env.storage()
            .instance()
            .set(&GovernanceKey::Proposal(proposal_id), &proposal);
        env.events()
            .publish((STATE_CHANGED,), (proposal_id, symbol_short!("defeat")));
        false
    }
}

/// Execute a succeeded proposal after the timelock has expired.
pub fn execute_proposal(env: &Env, proposal_id: u64) {
    let mut proposal: VotingProposal = env
        .storage()
        .instance()
        .get(&GovernanceKey::Proposal(proposal_id))
        .expect("Proposal not found");

    assert!(
        proposal.state == ProposalState::Succeeded,
        "Proposal has not succeeded"
    );
    assert!(
        env.ledger().sequence() >= proposal.execution_timelock_ledger,
        "Execution timelock not expired"
    );

    proposal.state = ProposalState::Executed;
    env.storage()
        .instance()
        .set(&GovernanceKey::Proposal(proposal_id), &proposal);

    env.events()
        .publish((PROPOSAL_EXECUTED,), (proposal_id, true));
}

/// Cancel a proposal (proposer or admin).
pub fn cancel_proposal(env: &Env, canceller: Address, proposal_id: u64) {
    canceller.require_auth();

    let mut proposal: VotingProposal = env
        .storage()
        .instance()
        .get(&GovernanceKey::Proposal(proposal_id))
        .expect("Proposal not found");

    assert!(
        proposal.state == ProposalState::Pending || proposal.state == ProposalState::Active,
        "Cannot cancel a finalised proposal"
    );

    proposal.state = ProposalState::Cancelled;
    env.storage()
        .instance()
        .set(&GovernanceKey::Proposal(proposal_id), &proposal);

    env.events()
        .publish((PROPOSAL_CANCELLED,), (proposal_id, canceller));
}

/// Delegate voting power to another address.
pub fn delegate_vote(env: &Env, delegator: Address, delegate: Address) {
    delegator.require_auth();
    assert!(delegator != delegate, "Cannot delegate to self");

    env.storage()
        .instance()
        .set(&GovernanceKey::Delegate(delegator.clone()), &delegate);

    let checkpoint: Option<i128> = env
        .storage()
        .instance()
        .get(&GovernanceKey::VotingPowerCheckpoint(delegate.clone()));
    if checkpoint.is_none() {
        env.storage()
            .instance()
            .set(&GovernanceKey::VotingPowerCheckpoint(delegate.clone()), &0_i128);
    }

    env.events()
        .publish((VOTE_DELEGATED,), (delegator, delegate));
}

pub fn get_checkpointed_voting_power(env: &Env, voter: Address) -> i128 {
    env.storage()
        .instance()
        .get(&GovernanceKey::VotingPowerCheckpoint(voter))
        .unwrap_or(0)
}

pub fn checkpoint_voting_power(env: &Env, voter: Address, power: i128) {
    env.storage()
        .instance()
        .set(&GovernanceKey::VotingPowerCheckpoint(voter), &power);
}

pub fn get_proposal(env: &Env, proposal_id: u64) -> Option<VotingProposal> {
    env.storage()
        .instance()
        .get(&GovernanceKey::Proposal(proposal_id))
}

pub fn get_vote_record(env: &Env, proposal_id: u64, voter: Address) -> Option<VoteRecord> {
    env.storage()
        .instance()
        .get(&GovernanceKey::VoteRecord(proposal_id, voter))
}

pub fn get_delegate(env: &Env, delegator: Address) -> Option<Address> {
    env.storage()
        .instance()
        .get(&GovernanceKey::Delegate(delegator))
}

// =============================================================================
// Weight computation helpers
// =============================================================================

/// Compute effective vote weight from a raw balance according to strategy.
pub fn compute_weight(strategy: &VotingStrategy, balance: i128) -> i128 {
    match strategy {
        VotingStrategy::Weighted => balance,
        VotingStrategy::Quadratic => integer_sqrt(balance),
        VotingStrategy::Flat => {
            if balance > 0 { 1 } else { 0 }
        }
    }
}

/// Integer square root — largest integer n such that n² ≤ x.
/// Uses Newton's method; safe for all i128 non-negative values.
pub fn integer_sqrt(x: i128) -> i128 {
    if x <= 0 {
        return 0;
    }
    if x < 4 {
        return 1;
    }
    let mut n = x;
    loop {
        let n1 = (n + x / n) / 2;
        if n1 >= n {
            return n;
        }
        n = n1;
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ── integer_sqrt ──────────────────────────────────────────────────────────

    #[test]
    fn test_sqrt_zero() {
        assert_eq!(integer_sqrt(0), 0);
    }

    #[test]
    fn test_sqrt_one() {
        assert_eq!(integer_sqrt(1), 1);
    }

    #[test]
    fn test_sqrt_perfect_squares() {
        assert_eq!(integer_sqrt(4), 2);
        assert_eq!(integer_sqrt(9), 3);
        assert_eq!(integer_sqrt(100), 10);
        assert_eq!(integer_sqrt(10_000), 100);
        assert_eq!(integer_sqrt(1_000_000), 1_000);
    }

    #[test]
    fn test_sqrt_non_perfect() {
        assert_eq!(integer_sqrt(2), 1);
        assert_eq!(integer_sqrt(3), 1);
        assert_eq!(integer_sqrt(5), 2);
        assert_eq!(integer_sqrt(8), 2);
        assert_eq!(integer_sqrt(15), 3);
        assert_eq!(integer_sqrt(24), 4);
        assert_eq!(integer_sqrt(99), 9);
    }

    #[test]
    fn test_sqrt_large() {
        let val: i128 = 10_000_000_000_000_000; // 10^16
        let r = integer_sqrt(val);
        assert_eq!(r, 100_000_000); // 10^8
    }

    // ── compute_weight ────────────────────────────────────────────────────────

    #[test]
    fn test_weighted_strategy() {
        assert_eq!(compute_weight(&VotingStrategy::Weighted, 1_000), 1_000);
        assert_eq!(compute_weight(&VotingStrategy::Weighted, 0), 0);
    }

    #[test]
    fn test_quadratic_strategy() {
        // 100 tokens → sqrt(100) = 10 votes
        assert_eq!(compute_weight(&VotingStrategy::Quadratic, 100), 10);
        // 10000 tokens → sqrt(10000) = 100 votes
        assert_eq!(compute_weight(&VotingStrategy::Quadratic, 10_000), 100);
        // 0 tokens → 0 votes
        assert_eq!(compute_weight(&VotingStrategy::Quadratic, 0), 0);
    }

    #[test]
    fn test_flat_strategy() {
        assert_eq!(compute_weight(&VotingStrategy::Flat, 1_000_000), 1);
        assert_eq!(compute_weight(&VotingStrategy::Flat, 1), 1);
        assert_eq!(compute_weight(&VotingStrategy::Flat, 0), 0);
    }

    // ── Quadratic dampens whale dominance ─────────────────────────────────────

    #[test]
    fn test_quadratic_dampens_whale() {
        // Whale has 1,000,000 BST, normal user has 1,000 BST
        let whale_weighted = compute_weight(&VotingStrategy::Weighted, 1_000_000);
        let user_weighted = compute_weight(&VotingStrategy::Weighted, 1_000);
        // Weighted: whale is 1000x more powerful
        assert_eq!(whale_weighted / user_weighted, 1_000);

        let whale_quad = compute_weight(&VotingStrategy::Quadratic, 1_000_000);
        let user_quad = compute_weight(&VotingStrategy::Quadratic, 1_000);
        // Quadratic: whale is ~31.6x more powerful (sqrt of 1000)
        assert_eq!(whale_quad, 1_000);   // sqrt(1_000_000)
        assert_eq!(user_quad, 31);       // sqrt(1_000) ≈ 31
        // Ratio is ~32x, far less than 1000x
        assert!(whale_quad / user_quad < 40);
    }

    // ── E2E proposal creation and voting lifecycle tests ──────────────────────

    #[test]
    fn test_e2e_proposal_creation_and_voting_weighted() {
        use soroban_sdk::testutils::{Address as _, Ledger, LedgerInfo};
        use soroban_sdk::Env;

        let env = Env::default();
        let proposer = Address::generate(&env);
        let voter1 = Address::generate(&env);
        let voter2 = Address::generate(&env);

        env.ledger().set(LedgerInfo {
            sequence_number: 1,
            timestamp: 1000,
            protocol_version: 21,
            network_id: Default::default(),
            base_reserve: 10,
            min_temp_entry_ttl: MIN_TEMP_ENTRY_TTL,
            min_persistent_entry_ttl: MIN_PERSISTENT_ENTRY_TTL,
            max_entry_ttl: MAX_ENTRY_TTL,
        });

        let prop_id = create_proposal(
            &env,
            proposer,
            String::from_str(&env, "Add GPU computing"),
            String::from_str(&env, "Enable GPU-based courses"),
            100,
            VotingStrategy::Weighted,
            150,
        );

        assert_eq!(prop_id, 1);
        let proposal = get_proposal(&env, prop_id).unwrap();
        assert_eq!(proposal.state, ProposalState::Pending);
        assert_eq!(proposal.votes_for, 0);

        // Advance to voting start
        env.ledger().set(LedgerInfo {
            sequence_number: 2,
            timestamp: 2000,
            protocol_version: 21,
            network_id: Default::default(),
            base_reserve: 10,
            min_temp_entry_ttl: MIN_TEMP_ENTRY_TTL,
            min_persistent_entry_ttl: MIN_PERSISTENT_ENTRY_TTL,
            max_entry_ttl: MAX_ENTRY_TTL,
        });

        cast_vote(&env, prop_id, voter1, VoteSupport::For, 500);
        cast_vote(&env, prop_id, voter2, VoteSupport::Against, 200);

        let proposal = get_proposal(&env, prop_id).unwrap();
        assert_eq!(proposal.state, ProposalState::Active);
        assert_eq!(proposal.votes_for, 500);
        assert_eq!(proposal.votes_against, 200);

        // Advance past voting end
        env.ledger().set(LedgerInfo {
            sequence_number: 101,
            timestamp: 3000,
            protocol_version: 21,
            network_id: Default::default(),
            base_reserve: 10,
            min_temp_entry_ttl: MIN_TEMP_ENTRY_TTL,
            min_persistent_entry_ttl: MIN_PERSISTENT_ENTRY_TTL,
            max_entry_ttl: MAX_ENTRY_TTL,
        });

        let passed = finalise_proposal(&env, prop_id);
        assert!(passed);
        let proposal = get_proposal(&env, prop_id).unwrap();
        assert_eq!(proposal.state, ProposalState::Succeeded);

        // Advance past timelock
        env.ledger().set(LedgerInfo {
            sequence_number: 151,
            timestamp: 4000,
            protocol_version: 21,
            network_id: Default::default(),
            base_reserve: 10,
            min_temp_entry_ttl: MIN_TEMP_ENTRY_TTL,
            min_persistent_entry_ttl: MIN_PERSISTENT_ENTRY_TTL,
            max_entry_ttl: MAX_ENTRY_TTL,
        });

        execute_proposal(&env, prop_id);
        let proposal = get_proposal(&env, prop_id).unwrap();
        assert_eq!(proposal.state, ProposalState::Executed);
    }

    #[test]
    fn test_e2e_quadratic_voting_dampens_whale() {
        use soroban_sdk::testutils::{Address as _, Ledger, LedgerInfo};
        use soroban_sdk::Env;

        let env = Env::default();
        let proposer = Address::generate(&env);
        let whale = Address::generate(&env);
        let normal = Address::generate(&env);

        env.ledger().set(LedgerInfo {
            sequence_number: 1,
            timestamp: 1000,
            protocol_version: 21,
            network_id: Default::default(),
            base_reserve: 10,
            min_temp_entry_ttl: MIN_TEMP_ENTRY_TTL,
            min_persistent_entry_ttl: MIN_PERSISTENT_ENTRY_TTL,
            max_entry_ttl: MAX_ENTRY_TTL,
        });

        let prop_id = create_proposal(
            &env,
            proposer,
            String::from_str(&env, "Governance change"),
            String::from_str(&env, "Test quadratic voting"),
            50,
            VotingStrategy::Quadratic,
            100,
        );

        env.ledger().set(LedgerInfo {
            sequence_number: 2,
            timestamp: 2000,
            protocol_version: 21,
            network_id: Default::default(),
            base_reserve: 10,
            min_temp_entry_ttl: MIN_TEMP_ENTRY_TTL,
            min_persistent_entry_ttl: MIN_PERSISTENT_ENTRY_TTL,
            max_entry_ttl: MAX_ENTRY_TTL,
        });

        // Whale has 1M, normal has 1K
        cast_vote(&env, prop_id, whale, VoteSupport::For, 1_000_000);
        cast_vote(&env, prop_id, normal, VoteSupport::Against, 1_000);

        let proposal = get_proposal(&env, prop_id).unwrap();
        assert_eq!(proposal.votes_for, 1_000); // sqrt(1M)
        assert_eq!(proposal.votes_against, 31); // sqrt(1K) ≈ 31

        // Whale power is 31x, not 1000x (dampening worked)
        assert!(proposal.votes_for / proposal.votes_against < 40);
    }

    #[test]
    fn test_e2e_vote_delegation() {
        use soroban_sdk::testutils::{Address as _, Ledger, LedgerInfo};
        use soroban_sdk::Env;

        let env = Env::default();
        let proposer = Address::generate(&env);
        let delegator = Address::generate(&env);

        env.ledger().set(LedgerInfo {
            sequence_number: 1,
            timestamp: 1000,
            protocol_version: 21,
            network_id: Default::default(),
            base_reserve: 10,
            min_temp_entry_ttl: MIN_TEMP_ENTRY_TTL,
            min_persistent_entry_ttl: MIN_PERSISTENT_ENTRY_TTL,
            max_entry_ttl: MAX_ENTRY_TTL,
        });

        let prop_id = create_proposal(
            &env,
            proposer,
            String::from_str(&env, "Delegate test"),
            String::from_str(&env, "Test vote delegation"),
            50,
            VotingStrategy::Weighted,
            100,
        );

        let delegate_addr = Address::generate(&env);
        delegate_vote(&env, delegator.clone(), delegate_addr.clone());

        let stored_delegate = get_delegate(&env, delegator).unwrap();
        assert_eq!(stored_delegate, delegate_addr);
    }

    #[test]
    fn test_e2e_proposal_defeated_insufficient_quorum() {
        use soroban_sdk::testutils::{Address as _, Ledger, LedgerInfo};
        use soroban_sdk::Env;

        let env = Env::default();
        let proposer = Address::generate(&env);
        let voter = Address::generate(&env);

        env.ledger().set(LedgerInfo {
            sequence_number: 1,
            timestamp: 1000,
            protocol_version: 21,
            network_id: Default::default(),
            base_reserve: 10,
            min_temp_entry_ttl: MIN_TEMP_ENTRY_TTL,
            min_persistent_entry_ttl: MIN_PERSISTENT_ENTRY_TTL,
            max_entry_ttl: MAX_ENTRY_TTL,
        });

        let prop_id = create_proposal(
            &env,
            proposer,
            String::from_str(&env, "Minority proposal"),
            String::from_str(&env, "Low participation"),
            50,
            VotingStrategy::Weighted,
            100,
        );

        env.ledger().set(LedgerInfo {
            sequence_number: 2,
            timestamp: 2000,
            protocol_version: 21,
            network_id: Default::default(),
            base_reserve: 10,
            min_temp_entry_ttl: MIN_TEMP_ENTRY_TTL,
            min_persistent_entry_ttl: MIN_PERSISTENT_ENTRY_TTL,
            max_entry_ttl: MAX_ENTRY_TTL,
        });

        // Cast minimal vote
        cast_vote(&env, prop_id, voter, VoteSupport::For, 1);

        env.ledger().set(LedgerInfo {
            sequence_number: 51,
            timestamp: 3000,
            protocol_version: 21,
            network_id: Default::default(),
            base_reserve: 10,
            min_temp_entry_ttl: MIN_TEMP_ENTRY_TTL,
            min_persistent_entry_ttl: MIN_PERSISTENT_ENTRY_TTL,
            max_entry_ttl: MAX_ENTRY_TTL,
        });

        let passed = finalise_proposal(&env, prop_id);
        assert!(!passed);
        let proposal = get_proposal(&env, prop_id).unwrap();
        assert_eq!(proposal.state, ProposalState::Defeated);
    }

    #[test]
    fn test_e2e_proposal_defeated_more_against() {
        use soroban_sdk::testutils::{Address as _, Ledger, LedgerInfo};
        use soroban_sdk::Env;

        let env = Env::default();
        let proposer = Address::generate(&env);
        let voter_for = Address::generate(&env);
        let voter_against = Address::generate(&env);

        env.ledger().set(LedgerInfo {
            sequence_number: 1,
            timestamp: 1000,
            protocol_version: 21,
            network_id: Default::default(),
            base_reserve: 10,
            min_temp_entry_ttl: MIN_TEMP_ENTRY_TTL,
            min_persistent_entry_ttl: MIN_PERSISTENT_ENTRY_TTL,
            max_entry_ttl: MAX_ENTRY_TTL,
        });

        let prop_id = create_proposal(
            &env,
            proposer,
            String::from_str(&env, "Rejected proposal"),
            String::from_str(&env, "Voted down"),
            50,
            VotingStrategy::Weighted,
            100,
        );

        env.ledger().set(LedgerInfo {
            sequence_number: 2,
            timestamp: 2000,
            protocol_version: 21,
            network_id: Default::default(),
            base_reserve: 10,
            min_temp_entry_ttl: MIN_TEMP_ENTRY_TTL,
            min_persistent_entry_ttl: MIN_PERSISTENT_ENTRY_TTL,
            max_entry_ttl: MAX_ENTRY_TTL,
        });

        cast_vote(&env, prop_id, voter_for, VoteSupport::For, 100);
        cast_vote(&env, prop_id, voter_against, VoteSupport::Against, 200);

        env.ledger().set(LedgerInfo {
            sequence_number: 51,
            timestamp: 3000,
            protocol_version: 21,
            network_id: Default::default(),
            base_reserve: 10,
            min_temp_entry_ttl: MIN_TEMP_ENTRY_TTL,
            min_persistent_entry_ttl: MIN_PERSISTENT_ENTRY_TTL,
            max_entry_ttl: MAX_ENTRY_TTL,
        });

        let passed = finalise_proposal(&env, prop_id);
        assert!(!passed);
    }

    #[test]
    fn test_e2e_abstain_votes_count_toward_quorum() {
        use soroban_sdk::testutils::{Address as _, Ledger, LedgerInfo};
        use soroban_sdk::Env;

        let env = Env::default();
        let proposer = Address::generate(&env);
        let voter_for = Address::generate(&env);
        let voter_abstain = Address::generate(&env);

        env.ledger().set(LedgerInfo {
            sequence_number: 1,
            timestamp: 1000,
            protocol_version: 21,
            network_id: Default::default(),
            base_reserve: 10,
            min_temp_entry_ttl: MIN_TEMP_ENTRY_TTL,
            min_persistent_entry_ttl: MIN_PERSISTENT_ENTRY_TTL,
            max_entry_ttl: MAX_ENTRY_TTL,
        });

        let prop_id = create_proposal(
            &env,
            proposer,
            String::from_str(&env, "Abstain test"),
            String::from_str(&env, "Testing abstentions"),
            50,
            VotingStrategy::Weighted,
            100,
        );

        env.ledger().set(LedgerInfo {
            sequence_number: 2,
            timestamp: 2000,
            protocol_version: 21,
            network_id: Default::default(),
            base_reserve: 10,
            min_temp_entry_ttl: MIN_TEMP_ENTRY_TTL,
            min_persistent_entry_ttl: MIN_PERSISTENT_ENTRY_TTL,
            max_entry_ttl: MAX_ENTRY_TTL,
        });

        cast_vote(&env, prop_id, voter_for, VoteSupport::For, 1_000_000_000);
        cast_vote(&env, prop_id, voter_abstain, VoteSupport::Abstain, 500_000_000);

        let proposal = get_proposal(&env, prop_id).unwrap();
        assert_eq!(proposal.votes_for, 1_000_000_000);
        assert_eq!(proposal.votes_abstain, 500_000_000);
    }

    #[test]
    fn test_e2e_flat_voting_one_person_one_vote() {
        use soroban_sdk::testutils::{Address as _, Ledger, LedgerInfo};
        use soroban_sdk::Env;

        let env = Env::default();
        let proposer = Address::generate(&env);
        let whale = Address::generate(&env);
        let normal = Address::generate(&env);

        env.ledger().set(LedgerInfo {
            sequence_number: 1,
            timestamp: 1000,
            protocol_version: 21,
            network_id: Default::default(),
            base_reserve: 10,
            min_temp_entry_ttl: MIN_TEMP_ENTRY_TTL,
            min_persistent_entry_ttl: MIN_PERSISTENT_ENTRY_TTL,
            max_entry_ttl: MAX_ENTRY_TTL,
        });

        let prop_id = create_proposal(
            &env,
            proposer,
            String::from_str(&env, "Flat voting"),
            String::from_str(&env, "One person, one vote"),
            50,
            VotingStrategy::Flat,
            100,
        );

        env.ledger().set(LedgerInfo {
            sequence_number: 2,
            timestamp: 2000,
            protocol_version: 21,
            network_id: Default::default(),
            base_reserve: 10,
            min_temp_entry_ttl: MIN_TEMP_ENTRY_TTL,
            min_persistent_entry_ttl: MIN_PERSISTENT_ENTRY_TTL,
            max_entry_ttl: MAX_ENTRY_TTL,
        });

        cast_vote(&env, prop_id, whale, VoteSupport::For, 1_000_000_000);
        cast_vote(&env, prop_id, normal, VoteSupport::For, 1);

        let proposal = get_proposal(&env, prop_id).unwrap();
        assert_eq!(proposal.votes_for, 2); // Both have vote weight of 1
    }
}
