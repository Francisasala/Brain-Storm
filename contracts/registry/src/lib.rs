#![no_std]
//! Registry contract — verification levels, certified skills, and specialisations.
//!
//! #656 Acceptance Criteria:
//! - Verification levels and skills stored and queryable.
//! - Admin/curator-gated mutations.
//! - Events emitted for UI/indexer consumption.
//! - Tests cover edge cases.
//!
//! #663: Pausable/emergency-stop mechanism.
//! #662: Batch operations & gas optimisation.

// Test builds need std (proptest property helpers); runtime stays no_std.
#[cfg(test)]
extern crate std;

use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, Env, Symbol, Vec,
};

use brain_storm_shared::access;
use brain_storm_shared::pagination::{paginate, MAX_PAGE_SIZE};

// ── Storage keys ──────────────────────────────────────────────────────────────

#[contracttype]
pub enum DataKey {
    Admin,
    Curator(Address),
    VerificationLevel(Address),   // persistent: VerificationLevel
    CertifiedSkills(Address),     // persistent: Vec<Symbol>
    Specialisation(Address),      // persistent: Vec<Symbol>
    SkillExpiry(Address, Symbol), // persistent: u64 timestamp (0 = never)
    UserList,                     // instance: Vec<Address> — ordered registration list
    Paused,                       // instance: bool (#663)
}

fn level_ord(level: &VerificationLevel) -> u32 {
    match level {
        VerificationLevel::Unverified => 0,
        VerificationLevel::Basic => 1,
        VerificationLevel::Advanced => 2,
        VerificationLevel::Expert => 3,
    }
}

// ── Types ─────────────────────────────────────────────────────────────────────

/// Numeric verification tier (0 = unverified … 3 = fully verified).
#[contracttype]
#[derive(Clone, PartialEq, Debug)]
pub enum VerificationLevel {
    Unverified,
    Basic,
    Advanced,
    Expert,
}

// ── Events ────────────────────────────────────────────────────────────────────
const EVT_VL_SET: Symbol = symbol_short!("vl_set");
const EVT_SKILL_ADD: Symbol = symbol_short!("sk_add");
const EVT_SKILL_RM: Symbol = symbol_short!("sk_rm");
const EVT_SPEC_SET: Symbol = symbol_short!("sp_set");
const EVT_CURATOR_ADD: Symbol = symbol_short!("cur_add");
const EVT_PAUSED: Symbol = symbol_short!("paused");
const EVT_UNPAUSED: Symbol = symbol_short!("unpaused");

#[contract]
pub struct RegistryContract;

#[contractimpl]
impl RegistryContract {
    // ── Initialisation ────────────────────────────────────────────────────────

    pub fn initialize(env: Env, admin: Address) {
        assert!(!env.storage().instance().has(&DataKey::Admin), "Already initialized");
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Paused, &false);
    }

    pub fn get_admin(env: Env) -> Address {
        env.storage().instance().get(&DataKey::Admin).unwrap()
    }

    // ── Pausable (#663) ───────────────────────────────────────────────────────

    /// Pause all mutating operations. Admin only.
    pub fn pause(env: Env, admin: Address) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);
        assert!(!Self::is_paused_internal(&env), "Already paused");
        env.storage().instance().set(&DataKey::Paused, &true);
        env.events().publish((EVT_PAUSED,), admin);
    }

    /// Resume all operations. Admin only.
    pub fn unpause(env: Env, admin: Address) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);
        assert!(Self::is_paused_internal(&env), "Not paused");
        env.storage().instance().set(&DataKey::Paused, &false);
        env.events().publish((EVT_UNPAUSED,), admin);
    }

    pub fn is_paused(env: Env) -> bool {
        Self::is_paused_internal(&env)
    }

    // ── Curator management (admin-only) ───────────────────────────────────────

    pub fn add_curator(env: Env, admin: Address, curator: Address) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);
        env.storage().instance().set(&DataKey::Curator(curator.clone()), &true);
        env.events().publish((EVT_CURATOR_ADD, symbol_short!("addr")), curator);
    }

    pub fn remove_curator(env: Env, admin: Address, curator: Address) {
        admin.require_auth();
        Self::assert_admin(&env, &admin);
        env.storage().instance().remove(&DataKey::Curator(curator));
    }

    pub fn is_curator(env: Env, addr: Address) -> bool {
        env.storage()
            .instance()
            .get::<DataKey, bool>(&DataKey::Curator(addr))
            .unwrap_or(false)
    }

    // ── Verification levels ───────────────────────────────────────────────────

    /// Set the verification level for `user` (admin or curator only). Blocked when paused.
    pub fn set_verification_level(
        env: Env,
        setter: Address,
        user: Address,
        level: VerificationLevel,
    ) {
        Self::require_not_paused(&env);
        setter.require_auth();
        Self::assert_admin_or_curator(&env, &setter);
        env.storage()
            .persistent()
            .set(&DataKey::VerificationLevel(user.clone()), &level);
        env.events()
            .publish((EVT_VL_SET, symbol_short!("user")), (user, level_ord(&level)));
    }

    pub fn get_verification_level(env: Env, user: Address) -> VerificationLevel {
        env.storage()
            .persistent()
            .get(&DataKey::VerificationLevel(user))
            .unwrap_or(VerificationLevel::Unverified)
    }

    // ── Certified skills ──────────────────────────────────────────────────────

    /// Add a certified skill to `user`. Blocked when paused.
    pub fn add_certified_skill(
        env: Env,
        setter: Address,
        user: Address,
        skill: Symbol,
        expiry_ts: u64,
    ) {
        Self::require_not_paused(&env);
        setter.require_auth();
        Self::assert_admin_or_curator(&env, &setter);

        let mut skills: Vec<Symbol> = env
            .storage()
            .persistent()
            .get(&DataKey::CertifiedSkills(user.clone()))
            .unwrap_or_else(|| Vec::new(&env));

        if !skills.iter().any(|s| s == skill) {
            skills.push_back(skill.clone());
            env.storage()
                .persistent()
                .set(&DataKey::CertifiedSkills(user.clone()), &skills);
        }

        env.storage()
            .persistent()
            .set(&DataKey::SkillExpiry(user.clone(), skill.clone()), &expiry_ts);

        env.events()
            .publish((EVT_SKILL_ADD, symbol_short!("user")), (user, skill, expiry_ts));
    }

    /// Remove a certified skill from `user`. Blocked when paused.
    pub fn remove_certified_skill(env: Env, setter: Address, user: Address, skill: Symbol) {
        Self::require_not_paused(&env);
        setter.require_auth();
        Self::assert_admin_or_curator(&env, &setter);

        let skills: Vec<Symbol> = env
            .storage()
            .persistent()
            .get(&DataKey::CertifiedSkills(user.clone()))
            .unwrap_or_else(|| Vec::new(&env));

        let mut updated = Vec::new(&env);
        for s in skills.iter() {
            if s != skill {
                updated.push_back(s);
            }
        }
        env.storage()
            .persistent()
            .set(&DataKey::CertifiedSkills(user.clone()), &updated);
        env.storage()
            .persistent()
            .remove(&DataKey::SkillExpiry(user.clone(), skill.clone()));

        env.events()
            .publish((EVT_SKILL_RM, symbol_short!("user")), (user, skill));
    }

    /// Returns skills that have not expired.
    pub fn get_certified_skills(env: Env, user: Address) -> Vec<Symbol> {
        let skills: Vec<Symbol> = env
            .storage()
            .persistent()
            .get(&DataKey::CertifiedSkills(user.clone()))
            .unwrap_or_else(|| Vec::new(&env));

        let now = env.ledger().timestamp();
        let mut valid = Vec::new(&env);
        for skill in skills.iter() {
            let expiry: u64 = env
                .storage()
                .persistent()
                .get(&DataKey::SkillExpiry(user.clone(), skill.clone()))
                .unwrap_or(0);
            if expiry == 0 || now < expiry {
                valid.push_back(skill);
            }
        }
        valid
    }

    pub fn has_certified_skill(env: Env, user: Address, skill: Symbol) -> bool {
        let skills = Self::get_certified_skills(env, user);
        skills.iter().any(|s| s == skill)
    }

    // ── Specialisations ───────────────────────────────────────────────────────

    /// Set specialisations for a user. Blocked when paused.
    pub fn set_specialisations(
        env: Env,
        setter: Address,
        user: Address,
        specs: Vec<Symbol>,
    ) {
        Self::require_not_paused(&env);
        setter.require_auth();
        Self::assert_admin_or_curator(&env, &setter);
        env.storage()
            .persistent()
            .set(&DataKey::Specialisation(user.clone()), &specs);
        env.events()
            .publish((EVT_SPEC_SET, symbol_short!("user")), user);
    }

    pub fn get_specialisations(env: Env, user: Address) -> Vec<Symbol> {
        env.storage()
            .persistent()
            .get(&DataKey::Specialisation(user))
            .unwrap_or_else(|| Vec::new(&env))
    }

    // ── Batch operations (#662) ───────────────────────────────────────────────

    /// Register multiple users in one transaction. Blocked when paused.
    pub fn batch_register_users(env: Env, users: Vec<Address>) {
        Self::require_not_paused(&env);
        // Read list once
        let mut list: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::UserList)
            .unwrap_or_else(|| Vec::new(&env));

        for user in users.iter() {
            user.require_auth();
            if !list.iter().any(|a| a == user) {
                list.push_back(user.clone());
            }
        }
        // Write list once — single storage write vs N writes
        env.storage().instance().set(&DataKey::UserList, &list);
    }

    /// Set verification levels for multiple users in one transaction (admin/curator). Blocked when paused.
    pub fn batch_set_verification_levels(
        env: Env,
        setter: Address,
        users: Vec<Address>,
        level: VerificationLevel,
    ) {
        Self::require_not_paused(&env);
        setter.require_auth();
        Self::assert_admin_or_curator(&env, &setter);

        let level_u32 = level_ord(&level);

        for user in users.iter() {
            env.storage()
                .persistent()
                .set(&DataKey::VerificationLevel(user.clone()), &level);
            env.events()
                .publish((EVT_VL_SET, symbol_short!("user")), (user, level_u32));
        }
    }

    // ── Pagination + filtering (#701) ─────────────────────────────────────────

    /// Register a user in the global listing (idempotent).
    pub fn register_user(env: Env, user: Address) {
        user.require_auth();
        let mut list: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::UserList)
            .unwrap_or_else(|| Vec::new(&env));
        if !list.iter().any(|a| a == user) {
            list.push_back(user.clone());
            env.storage().instance().set(&DataKey::UserList, &list);
        }
    }

    /// Return a page of registered users.
    ///
    /// `limit` is capped at [`MAX_PAGE_SIZE`] by the shared pagination helper,
    /// so over-limit requests are truncated predictably (Issue #1168).
    pub fn list_users(env: Env, offset: u32, limit: u32) -> Vec<Address> {
        let list: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::UserList)
            .unwrap_or_else(|| Vec::new(&env));
        paginate(&env, &list, offset, limit)
    }

    /// Return users filtered by minimum verification level.
    ///
    /// `limit` is capped at [`MAX_PAGE_SIZE`] by the shared pagination helper,
    /// so over-limit requests are truncated predictably (Issue #1168).
    pub fn list_users_by_level(
        env: Env,
        min_level: VerificationLevel,
        offset: u32,
        limit: u32,
    ) -> Vec<Address> {
        let list: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::UserList)
            .unwrap_or_else(|| Vec::new(&env));

        let min_ord = level_ord(&min_level);
        let mut filtered = Vec::new(&env);
        for addr in list.iter() {
            let level = env
                .storage()
                .persistent()
                .get(&DataKey::VerificationLevel(addr.clone()))
                .unwrap_or(VerificationLevel::Unverified);
            if level_ord(&level) >= min_ord {
                filtered.push_back(addr);
            }
        }

        paginate(&env, &filtered, offset, limit)
    }

    pub fn total_users(env: Env) -> u32 {
        let list: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::UserList)
            .unwrap_or_else(|| Vec::new(&env));
        list.len()
    }

    /// Largest page `list_users` / `list_users_by_level` will ever return
    /// (Issue #1168).
    ///
    /// Surfaced so clients can validate a `limit` before calling instead of
    /// discovering the cap from a truncated page.
    pub fn get_max_page_size() -> u32 {
        MAX_PAGE_SIZE
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    fn is_paused_internal(env: &Env) -> bool {
        env.storage().instance().get(&DataKey::Paused).unwrap_or(false)
    }

    fn require_not_paused(env: &Env) {
        assert!(!Self::is_paused_internal(env), "Contract is paused");
    }

    fn assert_admin(env: &Env, caller: &Address) {
        assert!(access::is_admin(env, caller, &DataKey::Admin), "Only admin");
    }

    fn assert_admin_or_curator(env: &Env, caller: &Address) {
        let is_curator = env
            .storage()
            .instance()
            .get::<DataKey, bool>(&DataKey::Curator(caller.clone()))
            .unwrap_or(false);
        assert!(
            is_curator || access::is_admin(env, caller, &DataKey::Admin),
            "Unauthorized: admin or curator required"
        );
    }
}

#[cfg(test)]
mod fuzz_tests;

#[cfg(test)]
mod tests;

// #862 — lookup-function unit tests
#[cfg(test)]
mod lookup_tests;
