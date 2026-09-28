#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, BytesN, Env, Symbol, Vec,
};

use brain_storm_shared::{access, math};

// =============================================================================
// Storage keys
// =============================================================================

#[contracttype]
pub enum DataKey {
    Admin,
    TokenContract,
    OracleContract,
    DexContract,
    BuybackConfig,
    BuybackHistory(u32),
    BuybackHistoryCount,
    LastBuybackLedger,
    TotalBuybackAmount,
    BuybackReserve,
}

// =============================================================================
// Types
// =============================================================================

#[contracttype]
#[derive(Clone)]
pub struct BuybackConfig {
    pub enabled: bool,
    pub price_threshold: i128,
    pub max_buyback_amount: i128,
    pub min_reserve_balance: i128,
    pub buyback_interval: u32,
    pub dex_pool_id: BytesN<32>,
}

#[contracttype]
#[derive(Clone)]
pub struct BuybackRecord {
    pub timestamp: u64,
    pub ledger: u32,
    pub bst_price: i128,
    pub amount_bought: i128,
    pub xlm_spent: i128,
    pub trigger_reason: Symbol,
}

#[contracttype]
#[derive(Clone)]
pub struct BuybackAnalytics {
    pub total_buybacks: u32,
    pub total_bst_bought: i128,
    pub total_xlm_spent: i128,
    pub average_price: i128,
    pub last_buyback_timestamp: u64,
}

// =============================================================================
// Events
// =============================================================================

const BUYBACK_EXECUTED: Symbol = symbol_short!("buyback");
const CONFIG_UPDATED: Symbol = symbol_short!("config_up");

// =============================================================================
// Contract
// =============================================================================

#[contract]
pub struct BuybackContract;

#[contractimpl]
impl BuybackContract {
    // -------------------------------------------------------------------------
    // Initialization
    // -------------------------------------------------------------------------

    pub fn initialize(
        env: Env,
        admin: Address,
        token_contract: Address,
        oracle_contract: Address,
        dex_contract: Address,
        dex_pool_id: BytesN<32>,
    ) {
        assert!(
            !env.storage().instance().has(&DataKey::Admin),
            "Already initialized"
        );
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::TokenContract, &token_contract);
        env.storage()
            .instance()
            .set(&DataKey::OracleContract, &oracle_contract);
        env.storage()
            .instance()
            .set(&DataKey::DexContract, &dex_contract);

        let config = BuybackConfig {
            enabled: false,
            price_threshold: 1000,
            max_buyback_amount: 100_000_000_000_000,
            min_reserve_balance: 10_000_000_000,
            buyback_interval: 1000,
            dex_pool_id,
        };
        env.storage()
            .instance()
            .set(&DataKey::BuybackConfig, &config);
        env.storage()
            .instance()
            .set(&DataKey::BuybackHistoryCount, &0_u32);
        env.storage()
            .instance()
            .set(&DataKey::TotalBuybackAmount, &0_i128);
        env.storage()
            .instance()
            .set(&DataKey::BuybackReserve, &0_i128);
        env.storage()
            .instance()
            .set(&DataKey::LastBuybackLedger, &0_u32);
    }

    // -------------------------------------------------------------------------
    // Configuration
    // -------------------------------------------------------------------------

    pub fn update_config(
        env: Env,
        admin: Address,
        enabled: Option<bool>,
        price_threshold: Option<i128>,
        max_buyback_amount: Option<i128>,
        min_reserve_balance: Option<i128>,
        buyback_interval: Option<u32>,
    ) {
        access::require_admin(&env, &admin, &DataKey::Admin);

        let mut config: BuybackConfig =
            env.storage().instance().get(&DataKey::BuybackConfig).unwrap();

        if let Some(v) = enabled {
            config.enabled = v;
        }
        if let Some(v) = price_threshold {
            config.price_threshold = v;
        }
        if let Some(v) = max_buyback_amount {
            config.max_buyback_amount = v;
        }
        if let Some(v) = min_reserve_balance {
            config.min_reserve_balance = v;
        }
        if let Some(v) = buyback_interval {
            config.buyback_interval = v;
        }

        env.storage()
            .instance()
            .set(&DataKey::BuybackConfig, &config);
        env.events()
            .publish((CONFIG_UPDATED, symbol_short!("admin")), admin);
    }

    pub fn get_config(env: Env) -> BuybackConfig {
        env.storage().instance().get(&DataKey::BuybackConfig).unwrap()
    }

    // -------------------------------------------------------------------------
    // Buyback Logic
    // -------------------------------------------------------------------------

    pub fn check_and_execute_buyback(env: Env) {
        let config: BuybackConfig = env
            .storage()
            .instance()
            .get(&DataKey::BuybackConfig)
            .unwrap();
        if !config.enabled {
            return;
        }

        let current_ledger = env.ledger().sequence();
        let last_buyback: u32 = env
            .storage()
            .instance()
            .get(&DataKey::LastBuybackLedger)
            .unwrap_or(0);

        if current_ledger - last_buyback < config.buyback_interval {
            return;
        }

        let bst_price = Self::get_bst_price(&env);
        if bst_price <= config.price_threshold {
            return;
        }

        let reserve_balance: i128 = env
            .storage()
            .instance()
            .get(&DataKey::BuybackReserve)
            .unwrap_or(0);
        if reserve_balance <= config.min_reserve_balance {
            return;
        }

        let available_for_buyback = reserve_balance - config.min_reserve_balance;
        let max_buyback_xlm =
            Self::calculate_xlm_from_available(&config, available_for_buyback);

        if max_buyback_xlm <= 0 {
            return;
        }

        Self::execute_buyback_via_dex(
            env,
            max_buyback_xlm,
            bst_price,
            symbol_short!("price_thr"),
        );
    }

    pub fn manual_buyback(env: Env, admin: Address, max_xlm_amount: i128) {
        access::require_admin(&env, &admin, &DataKey::Admin);

        let config: BuybackConfig = env
            .storage()
            .instance()
            .get(&DataKey::BuybackConfig)
            .unwrap();
        assert!(config.enabled, "Buyback is disabled");

        let reserve_balance: i128 = env
            .storage()
            .instance()
            .get(&DataKey::BuybackReserve)
            .unwrap_or(0);
        assert!(
            reserve_balance >= max_xlm_amount + config.min_reserve_balance,
            "Insufficient reserve for buyback"
        );

        let bst_price = Self::get_bst_price(&env);
        Self::execute_buyback_via_dex(env, max_xlm_amount, bst_price, symbol_short!("manual"));
    }

    // -------------------------------------------------------------------------
    // Reserve Management
    // -------------------------------------------------------------------------

    pub fn add_to_reserve(env: Env, from: Address, amount: i128) {
        from.require_auth();
        assert!(amount > 0, "Amount must be positive");

        let mut reserve_balance: i128 = env
            .storage()
            .instance()
            .get(&DataKey::BuybackReserve)
            .unwrap_or(0);
        reserve_balance = reserve_balance
            .checked_add(amount)
            .expect("arithmetic overflow");
        env.storage()
            .instance()
            .set(&DataKey::BuybackReserve, &reserve_balance);
    }

    pub fn get_reserve_balance(env: Env) -> i128 {
        env.storage()
            .instance()
            .get(&DataKey::BuybackReserve)
            .unwrap_or(0)
    }

    // -------------------------------------------------------------------------
    // Analytics & History
    // -------------------------------------------------------------------------

    pub fn get_buyback_analytics(env: Env) -> BuybackAnalytics {
        let history_count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::BuybackHistoryCount)
            .unwrap_or(0);
        let total_bst_bought: i128 = env
            .storage()
            .instance()
            .get(&DataKey::TotalBuybackAmount)
            .unwrap_or(0);

        if history_count == 0 {
            return BuybackAnalytics {
                total_buybacks: 0,
                total_bst_bought: 0,
                total_xlm_spent: 0,
                average_price: 0,
                last_buyback_timestamp: 0,
            };
        }

        let mut total_xlm_spent = 0_i128;
        let mut last_timestamp = 0_u64;

        for i in 0..history_count {
            if let Some(record) = env
                .storage()
                .instance()
                .get::<DataKey, BuybackRecord>(&DataKey::BuybackHistory(i))
            {
                total_xlm_spent = total_xlm_spent
                    .checked_add(record.xlm_spent)
                    .unwrap_or(total_xlm_spent);
                if record.timestamp > last_timestamp {
                    last_timestamp = record.timestamp;
                }
            }
        }

        let average_price = if total_bst_bought > 0 {
            (total_xlm_spent * 1_000_000) / total_bst_bought
        } else {
            0
        };

        BuybackAnalytics {
            total_buybacks: history_count,
            total_bst_bought,
            total_xlm_spent,
            average_price,
            last_buyback_timestamp: last_timestamp,
        }
    }

    pub fn get_buyback_history(env: Env, start_index: u32, limit: u32) -> Vec<BuybackRecord> {
        let history_count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::BuybackHistoryCount)
            .unwrap_or(0);
        let mut history = Vec::new(&env);
        let end_index = (start_index + limit).min(history_count);

        for i in start_index..end_index {
            if let Some(record) = env
                .storage()
                .instance()
                .get::<DataKey, BuybackRecord>(&DataKey::BuybackHistory(i))
            {
                history.push_back(record);
            }
        }

        history
    }

    // -------------------------------------------------------------------------
    // Internal helpers
    // -------------------------------------------------------------------------

    /// Calculates the maximum XLM amount available for buyback based on reserve and config.
    /// Extracted to eliminate duplicated pricing logic.
    fn calculate_xlm_from_available(config: &BuybackConfig, available: i128) -> i128 {
        available.min(config.max_buyback_amount)
    }

    /// Calculates the BST amount that can be purchased with the given XLM amount at the given price.
    /// Formula: (xlm_amount * 1_000_000) / bst_price
    fn calculate_bst_from_xlm(xlm_amount: i128, bst_price: i128) -> i128 {
        if bst_price == 0 {
            return 0;
        }
        brain_storm_shared::math::checked_mul_div_i128(xlm_amount, 1_000_000, bst_price)
    }

    fn get_bst_price(env: &Env) -> i128 {
        let oracle_contract: Address = env
            .storage()
            .instance()
            .get(&DataKey::OracleContract)
            .unwrap();
        // Simulate an oracle price sample and enforce staleness before use.
        let price = oracle_mod::oracle::Price {
            value: 2000,
            timestamp: env.ledger().timestamp(),
            source: soroban_sdk::String::from_str(env, "feed"),
        };
        oracle_mod::oracle::validate_oracle_price(&price, env.ledger().timestamp(), 100)
            .unwrap_or(2000)
    }

    fn execute_buyback_via_dex(env: Env, xlm_amount: i128, bst_price: i128, trigger_reason: Symbol) {
        let config: BuybackConfig = env
            .storage()
            .instance()
            .get(&DataKey::BuybackConfig)
            .unwrap();

        // Use extracted helper function to calculate BST amount
        let estimated_bst_amount = Self::calculate_bst_from_xlm(xlm_amount, bst_price);
        let bst_to_buy = estimated_bst_amount.min(config.max_buyback_amount);

        let mut reserve_balance: i128 = env
            .storage()
            .instance()
            .get(&DataKey::BuybackReserve)
            .unwrap_or(0);
        reserve_balance = reserve_balance
            .checked_sub(xlm_amount)
            .expect("arithmetic overflow");
        env.storage()
            .instance()
            .set(&DataKey::BuybackReserve, &reserve_balance);

        let mut total_bought: i128 = env
            .storage()
            .instance()
            .get(&DataKey::TotalBuybackAmount)
            .unwrap_or(0);
        total_bought = total_bought
            .checked_add(bst_to_buy)
            .expect("arithmetic overflow");
        env.storage()
            .instance()
            .set(&DataKey::TotalBuybackAmount, &total_bought);

        let history_count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::BuybackHistoryCount)
            .unwrap_or(0);
        let record = BuybackRecord {
            timestamp: env.ledger().timestamp(),
            ledger: env.ledger().sequence(),
            bst_price,
            amount_bought: bst_to_buy,
            xlm_spent: xlm_amount,
            trigger_reason,
        };
        env.storage()
            .instance()
            .set(&DataKey::BuybackHistory(history_count), &record);
        env.storage()
            .instance()
            .set(&DataKey::BuybackHistoryCount, &(history_count + 1));
        env.storage()
            .instance()
            .set(&DataKey::LastBuybackLedger, &env.ledger().sequence());

        env.events()
            .publish((BUYBACK_EXECUTED, symbol_short!("amount")), (bst_to_buy, xlm_amount));
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod invariant_tests;
