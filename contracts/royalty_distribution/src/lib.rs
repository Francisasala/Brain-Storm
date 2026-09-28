#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, Env, Symbol,
};

use brain_storm_shared::{access, events::emit_contract_event};

#[contracttype]
pub enum DataKey {
    Admin,
    RoyaltySplit(u64),
    RoyaltyRecipient(u64, u32),
    RecipientCount(u64),
    RoyaltyBalance(Address),
    PaymentRecord(u64),
    PaymentCount,
}

#[contracttype]
#[derive(Clone)]
pub struct RoyaltySplit {
    pub course_id: u64,
    pub creator_percentage: u32,
    pub contributor_percentage: u32,
    pub platform_percentage: u32,
}

#[contracttype]
#[derive(Clone)]
pub struct RoyaltyPayment {
    pub course_id: u64,
    pub amount: i128,
    pub timestamp: u64,
}

const EVT_DOMAIN: Symbol = symbol_short!("royalty");
const DISTRIBUTE: Symbol = symbol_short!("distribtd");
const WITHDRAW: Symbol = symbol_short!("withdrawn");
const SPLIT_SET: Symbol = symbol_short!("split_set");

#[contract]
pub struct RoyaltyDistributionContract;

#[contractimpl]
impl RoyaltyDistributionContract {
    pub fn initialize(env: Env, admin: Address) {
        assert!(
            !env.storage().instance().has(&DataKey::Admin),
            "Already initialized"
        );
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
    }

    pub fn set_royalty_split(
        env: Env,
        admin: Address,
        course_id: u64,
        creator_pct: u32,
        contributor_pct: u32,
        platform_pct: u32,
    ) {
        access::require_admin(&env, &admin, &DataKey::Admin);
        assert!(
            creator_pct + contributor_pct + platform_pct == 100,
            "Percentages must sum to 100"
        );

        let split = RoyaltySplit {
            course_id,
            creator_percentage: creator_pct,
            contributor_percentage: contributor_pct,
            platform_percentage: platform_pct,
        };

        env.storage()
            .persistent()
            .set(&DataKey::RoyaltySplit(course_id), &split);

        emit_contract_event(
            &env,
            EVT_DOMAIN,
            SPLIT_SET,
            (admin.clone(), course_id, (creator_pct, contributor_pct, platform_pct)),
        );
    }

    pub fn add_royalty_recipient(
        env: Env,
        admin: Address,
        course_id: u64,
        recipient: Address,
    ) {
        access::require_admin(&env, &admin, &DataKey::Admin);

        let count: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::RecipientCount(course_id))
            .unwrap_or(0);

        env.storage()
            .persistent()
            .set(&DataKey::RoyaltyRecipient(course_id, count), &recipient);

        env.storage()
            .persistent()
            .set(&DataKey::RecipientCount(course_id), &(count + 1));
    }

    pub fn distribute_royalties(
        env: Env,
        admin: Address,
        course_id: u64,
        total_amount: i128,
    ) {
        access::require_admin(&env, &admin, &DataKey::Admin);
        assert!(total_amount > 0, "Amount must be positive");

        let split: RoyaltySplit = env
            .storage()
            .persistent()
            .get(&DataKey::RoyaltySplit(course_id))
            .expect("Royalty split not configured");

        let creator_amount = compute_split_amount(total_amount, split.creator_percentage);
        let contributor_amount = compute_split_amount(total_amount, split.contributor_percentage);
        let platform_amount = total_amount
            .checked_sub(creator_amount)
            .and_then(|remaining| remaining.checked_sub(contributor_amount))
            .unwrap_or(0);

        let creator: Address = env
            .storage()
            .persistent()
            .get(&DataKey::RoyaltyRecipient(course_id, 0))
            .expect("Creator not set");

        let mut creator_balance: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::RoyaltyBalance(creator.clone()))
            .unwrap_or(0);
        creator_balance += creator_amount;
        env.storage()
            .persistent()
            .set(&DataKey::RoyaltyBalance(creator), &creator_balance);

        let count: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::RecipientCount(course_id))
            .unwrap_or(0);

        if count > 1 {
            let contributor: Address = env
                .storage()
                .persistent()
                .get(&DataKey::RoyaltyRecipient(course_id, 1))
                .expect("Contributor not set");

            let mut contrib_balance: i128 = env
                .storage()
                .persistent()
                .get(&DataKey::RoyaltyBalance(contributor.clone()))
                .unwrap_or(0);
            contrib_balance += contributor_amount;
            env.storage()
                .persistent()
                .set(&DataKey::RoyaltyBalance(contributor), &contrib_balance);
        }

        let platform: Address = env
            .storage()
            .persistent()
            .get(&DataKey::RoyaltyRecipient(course_id, count.saturating_sub(1)))
            .expect("Platform not set");

        let mut platform_balance: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::RoyaltyBalance(platform.clone()))
            .unwrap_or(0);
        platform_balance += platform_amount;
        env.storage()
            .persistent()
            .set(&DataKey::RoyaltyBalance(platform), &platform_balance);

        let payment_id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::PaymentCount)
            .unwrap_or(0);

        let record = RoyaltyPayment {
            course_id,
            amount: total_amount,
            timestamp: env.ledger().timestamp(),
        };

        env.storage()
            .persistent()
            .set(&DataKey::PaymentRecord(payment_id), &record);
        env.storage()
            .instance()
            .set(&DataKey::PaymentCount, &(payment_id + 1));

        emit_contract_event(
            &env,
            EVT_DOMAIN,
            DISTRIBUTE,
            (admin.clone(), course_id, total_amount),
        );
    }

    pub fn withdraw_royalties(env: Env, recipient: Address) -> i128 {
        recipient.require_auth();

        let balance: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::RoyaltyBalance(recipient.clone()))
            .unwrap_or(0);

        assert!(balance > 0, "No royalties to withdraw");

        env.storage()
            .persistent()
            .set(&DataKey::RoyaltyBalance(recipient.clone()), &0i128);

        emit_contract_event(&env, EVT_DOMAIN, WITHDRAW, (recipient.clone(), balance, 0i128));

        balance
    }

    pub fn get_royalty_balance(env: Env, recipient: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::RoyaltyBalance(recipient))
            .unwrap_or(0)
    }

    pub fn get_royalty_split(env: Env, course_id: u64) -> Option<RoyaltySplit> {
        env.storage()
            .persistent()
            .get(&DataKey::RoyaltySplit(course_id))
    }

    pub fn get_payment_record(env: Env, payment_id: u64) -> Option<RoyaltyPayment> {
        env.storage()
            .persistent()
            .get(&DataKey::PaymentRecord(payment_id))
    }

    pub fn get_payment_count(env: Env) -> u64 {
        env.storage()
            .instance()
            .get(&DataKey::PaymentCount)
            .unwrap_or(0)
    }

    pub fn get_total_distributed(env: Env, course_id: u64) -> i128 {
        let count: u64 = env
            .storage()
            .instance()
            .get(&DataKey::PaymentCount)
            .unwrap_or(0);

        let mut total: i128 = 0;
        let mut i: u64 = 0;
        while i < count {
            let record: Option<RoyaltyPayment> = env
                .storage()
                .persistent()
                .get(&DataKey::PaymentRecord(i));
            if let Some(r) = record {
                if r.course_id == course_id {
                    total += r.amount;
                }
            }
            i += 1;
        }
        total
    }
}

fn compute_split_amount(total_amount: i128, percentage: u32) -> i128 {
    let pct: i128 = percentage as i128;
    total_amount
        .checked_mul(pct)
        .expect("royalty split overflow")
        .checked_div(100)
        .expect("division by zero")
}

#[cfg(test)]
mod tests;
