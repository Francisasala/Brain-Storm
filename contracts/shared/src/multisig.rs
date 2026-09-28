use soroban_sdk::{contracttype, Address, Env, String, Symbol, symbol_short, Vec};

#[contracttype]
#[derive(Clone)]
pub struct MultiSigProposal {
    pub id: u64,
    pub operation: String,
    pub proposer: Address,
    pub approvals: Vec<Address>,
    pub threshold: u32,
    pub created_ledger: u32,
    pub expires_ledger: u32,
    pub executed: bool,
}

#[contracttype]
pub enum MultiSigKey {
    Proposal(u64),
    NextProposalId,
    Threshold,
    Admins,
    ProposalTimeout,
}

pub const MULTISIG_PROPOSAL_CREATED: Symbol = symbol_short!("ms_prop");
pub const MULTISIG_APPROVED: Symbol = symbol_short!("ms_appr");
pub const MULTISIG_EXECUTED: Symbol = symbol_short!("ms_exec");
pub const MULTISIG_EXPIRED: Symbol = symbol_short!("ms_exp");

pub fn create_proposal(
    env: &Env,
    operation: String,
    proposer: Address,
    threshold: u32,
    timeout_ledgers: u32,
) -> u64 {
    proposer.require_auth();

    let id: u64 = env
        .storage()
        .instance()
        .get(&MultiSigKey::NextProposalId)
        .unwrap_or(1);

    let proposal = MultiSigProposal {
        id,
        operation,
        proposer: proposer.clone(),
        approvals: Vec::new(env),
        threshold,
        created_ledger: env.ledger().sequence(),
        expires_ledger: env.ledger().sequence() + timeout_ledgers,
        executed: false,
    };

    env.storage()
        .instance()
        .set(&MultiSigKey::Proposal(id), &proposal);
    env.storage()
        .instance()
        .set(&MultiSigKey::NextProposalId, &(id + 1));

    env.events()
        .publish((MULTISIG_PROPOSAL_CREATED,), (id, proposer));

    id
}

pub fn approve_proposal(env: &Env, proposal_id: u64, approver: Address) {
    approver.require_auth();

    let mut proposal: MultiSigProposal = env
        .storage()
        .instance()
        .get(&MultiSigKey::Proposal(proposal_id))
        .expect("Proposal not found");

    assert!(!proposal.executed, "Proposal already executed");
    assert!(
        env.ledger().sequence() < proposal.expires_ledger,
        "Proposal expired"
    );

    let mut approvals = proposal.approvals.clone();
    assert!(
        !approvals.iter().any(|a| a == approver),
        "Already approved"
    );

    approvals.push_back(approver.clone());
    proposal.approvals = approvals;

    env.storage()
        .instance()
        .set(&MultiSigKey::Proposal(proposal_id), &proposal);

    env.events()
        .publish((MULTISIG_APPROVED,), (proposal_id, approver));
}

pub fn get_threshold(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&MultiSigKey::Threshold)
        .unwrap_or(1)
}

pub fn set_threshold(env: &Env, admin: Address, threshold: u32) {
    admin.require_auth();
    assert!(threshold > 0, "Threshold must be at least 1");
    env.storage().instance().set(&MultiSigKey::Threshold, &threshold);
}

pub fn execute_proposal(env: &Env, proposal_id: u64) -> bool {
    let mut proposal: MultiSigProposal = env
        .storage()
        .instance()
        .get(&MultiSigKey::Proposal(proposal_id))
        .expect("Proposal not found");

    assert!(!proposal.executed, "Already executed");

    if env.ledger().sequence() >= proposal.expires_ledger {
        env.events()
            .publish((MULTISIG_EXPIRED,), (proposal_id,));
        return false;
    }

    let effective_threshold = if proposal.threshold > 0 {
        proposal.threshold
    } else {
        get_threshold(env)
    };

    if proposal.approvals.len() >= effective_threshold {
        proposal.executed = true;
        env.storage()
            .instance()
            .set(&MultiSigKey::Proposal(proposal_id), &proposal);

        env.events()
            .publish((MULTISIG_EXECUTED,), (proposal_id,));
        return true;
    }

    false
}

pub fn get_proposal(env: &Env, proposal_id: u64) -> Option<MultiSigProposal> {
    env.storage()
        .instance()
        .get(&MultiSigKey::Proposal(proposal_id))
}

// =============================================================================
// Tests (#1167 — configurable threshold)
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{contract, contractimpl, testutils::Address as _};

    #[contracttype]
    enum TestKey {
        Admin,
    }

    #[contract]
    struct TestContract;

    #[contractimpl]
    impl TestContract {
        pub fn init(env: Env, admin: Address) {
            env.storage().instance().set(&TestKey::Admin, &admin);
        }
    }

    fn setup() -> (Env, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let id = env.register_contract(None, TestContract);
        env.as_contract(&id, || {
            env.storage().instance().set(&MultiSigKey::NextProposalId, &1u64);
            env.storage().instance().set(&MultiSigKey::Threshold, &1u32);
        });
        (env, admin, id)
    }

    #[test]
    fn test_get_threshold_default_is_one() {
        let (env, _, id) = setup();
        let result = env.as_contract(&id, || get_threshold(&env));
        assert_eq!(result, 1);
    }

    #[test]
    fn test_set_and_get_threshold() {
        let (env, admin, id) = setup();
        env.as_contract(&id, || set_threshold(&env, admin.clone(), 3));
        let result = env.as_contract(&id, || get_threshold(&env));
        assert_eq!(result, 3);
    }

    #[test]
    #[should_panic(expected = "Threshold must be at least 1")]
    fn test_set_threshold_zero_panics() {
        let (env, admin, id) = setup();
        env.as_contract(&id, || set_threshold(&env, admin, 0));
    }

    #[test]
    fn test_threshold_boundary_conditions() {
        let (env, admin, id) = setup();
        env.as_contract(&id, || set_threshold(&env, admin.clone(), 3));
        let proposer = Address::generate(&env);
        let id2 = env.as_contract(&id, || {
            create_proposal(&env, soroban_sdk::String::from_str(&env, "op"), proposer, 3, 100)
        });
        let a = Address::generate(&env);
        env.as_contract(&id, || approve_proposal(&env, id2, a.clone()));
        let b = Address::generate(&env);
        env.as_contract(&id, || approve_proposal(&env, id2, b.clone()));
        // Only 2 approvals, threshold is 3, so not executed
        assert!(!env.as_contract(&id, || execute_proposal(&env, id2)));
    }
}
