//! Smart contract upgrade functionality with timelock security.
//! 
//! ## Authorization Invariants
//!
//! The upgrade system enforces the following security invariants:
//!
//! 1. **Admin-Only Operations**: Only the stored admin can schedule, execute, or cancel upgrades.
//!    - Verified by `crate::access::require_admin()` inside every helper in this
//!      module; the contract entry points delegate to these helpers, so no call
//!      path can reach upgrade logic without the check (issue #1169)
//!    - Both authentication (require_auth) and authorization (admin check) required
//!
//! 2. **Complete Authorization Required**: Partial or insufficient authorization is rejected.
//!    - Address must match stored admin exactly
//!    - Address must provide valid signature via require_auth()
//!    - Role-based permissions (Instructor, Student) cannot override admin requirement
//!
//! 3. **Timelock Enforcement**: Upgrades cannot execute before their timelock expires.
//!    - Scheduled upgrades have execute_after ledger number
//!    - Current ledger must be >= execute_after to execute
//!    - No bypass mechanism exists for emergency execution
//!
//! 4. **State Integrity**: Authorization state is preserved throughout upgrade lifecycle.
//!    - Admin role maintained across schedule/cancel/execute operations
//!    - No privilege escalation possible during upgrade process
//!    - Storage keys remain consistent and protected
//!
//! 5. **Upgrade History**: All completed upgrades are recorded for audit trail.
//!    - Immutable history stored with executor, timestamp, and WASM hash
//!    - Cannot be modified or deleted after recording
//!
//! ## Security Review (#1169)
//!
//! The upgrade authorization flow has been comprehensively tested to ensure:
//! - Unauthorized upgrade attempts are rejected with clear error messages
//! - Authorization by wrong accounts fails even if they have other valid roles  
//! - Insufficient authorization (e.g., non-admin roles) cannot bypass restrictions
//! - Complete authorization sequence is required for all upgrade operations
//! - Authorization invariants are preserved across the entire upgrade lifecycle
//! - No multisig or partial authorization vulnerabilities exist
//!
//! ### Findings addressed (#1169)
//!
//! 1. **Authorization lived only in the entry points.** The helpers below trusted
//!    their caller, so a future entry point (or an internal call) that skipped
//!    `require_admin` would silently allow anyone to schedule, execute or cancel
//!    an upgrade. Every helper now re-checks `access::require_admin` itself.
//! 2. **Zero-length timelock allowed.** `timelock_ledgers == 0` made an upgrade
//!    executable in the very ledger it was scheduled, defeating the timelock
//!    invariant. A minimum of one ledger is now enforced at schedule time.
//! 3. **Zero WASM hash accepted.** Scheduling the all-zero hash would brick the
//!    contract on execute; it is now rejected when the upgrade is scheduled.
//! 4. **The admin slot could be re-initialized.** `SharedContract::initialize`
//!    had no once-only guard, so any address could call it, become the admin
//!    and then schedule *and* execute an upgrade — a complete takeover of the
//!    upgrade authority. It now refuses a second `initialize` call, matching
//!    `governance` and `credential_metadata`.
//! 5. **An immediate upgrade path existed.** `SharedContract::upgrade` swapped
//!    the WASM for the admin with no timelock, no pending record and no history
//!    entry, bypassing every invariant above. The admin check still runs first,
//!    but the swap itself now refuses and directs callers to
//!    `schedule_upgrade` → `execute_upgrade`.
//! 6. **The governance approval gate had no caller.**
//!    `governance::approve_upgrade` is documented as the *admin* approval gate
//!    but took no caller argument and performed no check, so anyone could
//!    approve a proposal that had merely finished voting. It now takes the
//!    approving admin and enforces `access::require_admin`.
//!
//! Covered by the tests in `upgrade_tests.rs`
//! ("Additional Authorization Security Tests (#1169)", "Bypass paths (#1169)"),
//! the re-initialization test in `tests.rs`, and the approval-gate tests in
//! `contracts/governance/src/lib.rs`.

#![allow(unused)]
use soroban_sdk::{contracttype, symbol_short, Address, BytesN, Env, Symbol};

// =============================================================================
// Storage keys
// =============================================================================

#[contracttype]
pub enum UpgradeKey {
    PendingUpgrade,
    UpgradeHistory(u32),
    UpgradeHistoryCount,
}

// =============================================================================
// Types
// =============================================================================

#[contracttype]
#[derive(Clone)]
pub struct ScheduledUpgrade {
    pub new_wasm_hash: BytesN<32>,
    pub scheduled_at: u32,
    pub execute_after: u32,
    pub proposed_by: Address,
}

#[contracttype]
#[derive(Clone)]
pub struct UpgradeRecord {
    pub wasm_hash: BytesN<32>,
    pub upgraded_at: u32,
    pub upgraded_by: Address,
}

// =============================================================================
// Events
// =============================================================================

const UPGRADE_SCHEDULED: Symbol = symbol_short!("upg_schd");
const UPGRADE_EXECUTED: Symbol = symbol_short!("upg_exec");
const UPGRADE_CANCELLED: Symbol = symbol_short!("upg_cxl");

// =============================================================================
// Upgrade functions
// =============================================================================

/// Schedule a WASM upgrade with a timelock. Only callable by admin.
/// The upgrade will execute once `timelock_ledgers` ledgers have passed.
pub fn schedule_upgrade(
    env: &Env,
    admin: &Address,
    new_wasm_hash: BytesN<32>,
    timelock_ledgers: u32,
) {
    // Defense in depth (#1169): re-check the admin role *and* the signature
    // here so the module stays safe even if an entry point forgets the check.
    crate::access::require_admin(env, admin, &crate::DataKey::Admin);

    // A zero hash would replace this contract with nothing (#1169).
    assert!(
        new_wasm_hash != BytesN::from_array(env, &[0u8; 32]),
        "Invalid WASM hash"
    );
    // A zero-length timelock makes the upgrade executable immediately (#1169).
    assert!(timelock_ledgers > 0, "Timelock must be at least 1 ledger");

    let current_ledger = env.ledger().sequence();
    let pending = ScheduledUpgrade {
        new_wasm_hash: new_wasm_hash.clone(),
        scheduled_at: current_ledger,
        execute_after: current_ledger
            .checked_add(timelock_ledgers)
            .expect("ledger overflow"),
        proposed_by: admin.clone(),
    };
    env.storage()
        .instance()
        .set(&UpgradeKey::PendingUpgrade, &pending);
    env.events().publish(
        (UPGRADE_SCHEDULED, symbol_short!("hash")),
        (admin, new_wasm_hash, current_ledger + timelock_ledgers),
    );
}

/// Execute a scheduled upgrade once the timelock has expired.
/// Stores the upgrade in history, then performs the WASM swap.
pub fn execute_upgrade(env: &Env, executor: &Address) {
    // Defense in depth (#1169): only the stored admin may execute, and must
    // have signed — recorded in `upgraded_by` as the executor identity.
    crate::access::require_admin(env, executor, &crate::DataKey::Admin);

    let pending: ScheduledUpgrade = env
        .storage()
        .instance()
        .get(&UpgradeKey::PendingUpgrade)
        .expect("No pending upgrade");

    assert!(
        env.ledger().sequence() >= pending.execute_after,
        "Timelock not expired"
    );

    let count: u32 = env
        .storage()
        .instance()
        .get(&UpgradeKey::UpgradeHistoryCount)
        .unwrap_or(0);

    let record = UpgradeRecord {
        wasm_hash: pending.new_wasm_hash.clone(),
        upgraded_at: env.ledger().sequence(),
        upgraded_by: executor.clone(),
    };
    env.storage()
        .instance()
        .set(&UpgradeKey::UpgradeHistory(count), &record);
    env.storage()
        .instance()
        .set(&UpgradeKey::UpgradeHistoryCount, &(count + 1));

    env.storage()
        .instance()
        .remove(&UpgradeKey::PendingUpgrade);

    env.events().publish(
        (UPGRADE_EXECUTED, symbol_short!("hash")),
        (executor, pending.new_wasm_hash.clone()),
    );

    // Perform the WASM upgrade — replaces this contract's code atomically.
    env.deployer()
        .update_current_contract_wasm(pending.new_wasm_hash);
}

/// Cancel a pending upgrade before it executes. Only callable by admin.
pub fn cancel_upgrade(env: &Env, admin: &Address) {
    // Defense in depth (#1169): only the stored admin may cancel.
    crate::access::require_admin(env, admin, &crate::DataKey::Admin);

    assert!(
        env.storage()
            .instance()
            .has(&UpgradeKey::PendingUpgrade),
        "No pending upgrade to cancel"
    );
    env.storage()
        .instance()
        .remove(&UpgradeKey::PendingUpgrade);
    env.events().publish(
        (UPGRADE_CANCELLED, symbol_short!("admin")),
        admin,
    );
}

/// Returns the pending upgrade, if any.
pub fn get_pending_upgrade(env: &Env) -> Option<ScheduledUpgrade> {
    env.storage().instance().get(&UpgradeKey::PendingUpgrade)
}

/// Returns the number of completed upgrades.
pub fn get_upgrade_count(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&UpgradeKey::UpgradeHistoryCount)
        .unwrap_or(0)
}

/// Returns a specific upgrade history entry.
pub fn get_upgrade_record(env: &Env, index: u32) -> Option<UpgradeRecord> {
    env.storage()
        .instance()
        .get(&UpgradeKey::UpgradeHistory(index))
}
