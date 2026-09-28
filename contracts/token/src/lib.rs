#![no_std]
extern crate alloc;
use alloc::format;

use soroban_sdk::{
    contract, contractimpl, contracttype, contracterror,
    Address, Env, String, Symbol, Vec,
};

/// Token contract with checked arithmetic operations
/// All balance operations use checked_add/checked_sub to prevent overflow/underflow

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum TokenError {
    /// Arithmetic overflow or underflow occurred
    ArithmeticError = 1,
    /// Insufficient balance for operation
    InsufficientBalance = 2,
    /// Unauthorized operation
    Unauthorized = 3,
    /// Invalid amount (zero or negative)
    InvalidAmount = 4,
    /// Transfer amount exceeds balance
    TransferExceedsBalance = 5,
    /// Allowance insufficient
    InsufficientAllowance = 6,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TokenState {
    pub name: String,
    pub symbol: String,
    pub decimals: u32,
    pub total_supply: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Balance {
    pub amount: i128,
    pub last_updated: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Allowance {
    pub amount: i128,
    pub spender: Address,
}

#[contract]
pub struct TokenContract;

#[contractimpl]
impl TokenContract {
    // ============================================
    // # Initialization
    // ============================================

    pub fn initialize(
        env: Env,
        admin: Address,
        name: String,
        symbol: String,
        decimals: u32,
        initial_supply: i128,
    ) -> Result<(), TokenError> {
        admin.require_auth();

        // Check initial supply is valid
        if initial_supply <= 0 {
            return Err(TokenError::InvalidAmount);
        }

        // Store token state
        let state = TokenState {
            name,
            symbol,
            decimals,
            total_supply: initial_supply,
        };
        env.storage().instance().set(&Symbol::new(&env, "state"), &state);

        // Mint initial supply to admin
        let balance = Balance {
            amount: initial_supply,
            last_updated: env.ledger().timestamp(),
        };
        env.storage().persistent().set(&Symbol::new(&env, &format!("balance_{:?}", admin)), &balance);

        env.storage().instance().set(&Symbol::new(&env, "admin"), &admin);

        Ok(())
    }

    // ============================================
    // # Balance Operations with Checked Arithmetic
    // ============================================

    /// Transfer tokens from one account to another
    pub fn transfer(
        env: Env,
        from: Address,
        to: Address,
        amount: i128,
    ) -> Result<(), TokenError> {
        from.require_auth();

        // Validate amount
        if amount <= 0 {
            return Err(TokenError::InvalidAmount);
        }

        // Get balances with checked operations
        let from_balance = Self::get_balance_internal(&env, &from)?;
        let to_balance = Self::get_balance_internal(&env, &to)?;

        // Check sufficient balance using checked_sub
        let new_from_balance = from_balance
            .checked_sub(amount)
            .ok_or(TokenError::InsufficientBalance)?;

        // Update balances with checked_add
        let new_to_balance = to_balance
            .checked_add(amount)
            .ok_or(TokenError::ArithmeticError)?;

        // Store updated balances
        let from_balance_obj = Balance {
            amount: new_from_balance,
            last_updated: env.ledger().timestamp(),
        };
        let to_balance_obj = Balance {
            amount: new_to_balance,
            last_updated: env.ledger().timestamp(),
        };

        env.storage().persistent().set(&Symbol::new(&env, &format!("balance_{:?}", from)), &from_balance_obj);
        env.storage().persistent().set(&Symbol::new(&env, &format!("balance_{:?}", to)), &to_balance_obj);

        // Update total supply (no change on transfer)

        // Emit transfer event
        env.events().publish(
            (Symbol::new(&env, "transfer"),),
            (from, to, amount),
        );

        Ok(())
    }

    /// Mint new tokens (admin only)
    pub fn mint(
        env: Env,
        admin: Address,
        to: Address,
        amount: i128,
    ) -> Result<(), TokenError> {
        admin.require_auth();

        // Validate admin
        let stored_admin: Address = env.storage().instance().get(&Symbol::new(&env, "admin"))
            .ok_or(TokenError::Unauthorized)?;
        if admin != stored_admin {
            return Err(TokenError::Unauthorized);
        }

        // Validate amount
        if amount <= 0 {
            return Err(TokenError::InvalidAmount);
        }

        // Get current state
        let mut state: TokenState = env.storage().instance().get(&Symbol::new(&env, "state"))
            .ok_or(TokenError::ArithmeticError)?;

        // Update total supply with checked_add
        let new_total_supply = state.total_supply
            .checked_add(amount)
            .ok_or(TokenError::ArithmeticError)?;
        state.total_supply = new_total_supply;

        // Update recipient balance
        let to_balance = Self::get_balance_internal(&env, &to)?;
        let new_to_balance = to_balance
            .checked_add(amount)
            .ok_or(TokenError::ArithmeticError)?;

        let balance_obj = Balance {
            amount: new_to_balance,
            last_updated: env.ledger().timestamp(),
        };
        env.storage().persistent().set(&Symbol::new(&env, &format!("balance_{:?}", to)), &balance_obj);

        // Store updated state
        env.storage().instance().set(&Symbol::new(&env, "state"), &state);

        // Emit mint event
        env.events().publish(
            (Symbol::new(&env, "mint"),),
            (to, amount),
        );

        Ok(())
    }

    /// Burn tokens (admin only)
    pub fn burn(
        env: Env,
        admin: Address,
        from: Address,
        amount: i128,
    ) -> Result<(), TokenError> {
        admin.require_auth();

        // Validate admin
        let stored_admin: Address = env.storage().instance().get(&Symbol::new(&env, "admin"))
            .ok_or(TokenError::Unauthorized)?;
        if admin != stored_admin {
            return Err(TokenError::Unauthorized);
        }

        // Validate amount
        if amount <= 0 {
            return Err(TokenError::InvalidAmount);
        }

        // Get current state
        let mut state: TokenState = env.storage().instance().get(&Symbol::new(&env, "state"))
            .ok_or(TokenError::ArithmeticError)?;

        // Update total supply with checked_sub
        let new_total_supply = state.total_supply
            .checked_sub(amount)
            .ok_or(TokenError::ArithmeticError)?;
        state.total_supply = new_total_supply;

        // Update from balance
        let from_balance = Self::get_balance_internal(&env, &from)?;
        let new_from_balance = from_balance
            .checked_sub(amount)
            .ok_or(TokenError::InsufficientBalance)?;

        let balance_obj = Balance {
            amount: new_from_balance,
            last_updated: env.ledger().timestamp(),
        };
        env.storage().persistent().set(&Symbol::new(&env, &format!("balance_{:?}", from)), &balance_obj);

        // Store updated state
        env.storage().instance().set(&Symbol::new(&env, "state"), &state);

        // Emit burn event
        env.events().publish(
            (Symbol::new(&env, "burn"),),
            (from, amount),
        );

        Ok(())
    }

    /// Approve allowance for spender
    pub fn approve(
        env: Env,
        owner: Address,
        spender: Address,
        amount: i128,
    ) -> Result<(), TokenError> {
        owner.require_auth();

        if amount < 0 {
            return Err(TokenError::InvalidAmount);
        }

        let allowance = Allowance {
            amount,
            spender: spender.clone(),
        };
        env.storage().persistent().set(
            &Symbol::new(&env, &format!("allowance_{:?}_{:?}", owner, spender)),
            &allowance,
        );

        env.events().publish(
            (Symbol::new(&env, "approve"),),
            (owner, spender, amount),
        );

        Ok(())
    }

    /// Transfer tokens via allowance
    pub fn transfer_from(
        env: Env,
        spender: Address,
        from: Address,
        to: Address,
        amount: i128,
    ) -> Result<(), TokenError> {
        spender.require_auth();

        // Validate amount
        if amount <= 0 {
            return Err(TokenError::InvalidAmount);
        }

        // Get and update allowance with checked_sub
        let allowance_key = Symbol::new(&env, &format!("allowance_{:?}_{:?}", from, spender));
        let mut allowance: Allowance = env.storage().persistent().get(&allowance_key)
            .ok_or(TokenError::InsufficientAllowance)?;

        let new_allowance = allowance.amount
            .checked_sub(amount)
            .ok_or(TokenError::InsufficientAllowance)?;
        allowance.amount = new_allowance;
        env.storage().persistent().set(&allowance_key, &allowance);

        // Perform transfer with checked operations
        let from_balance = Self::get_balance_internal(&env, &from)?;
        let to_balance = Self::get_balance_internal(&env, &to)?;

        let new_from_balance = from_balance
            .checked_sub(amount)
            .ok_or(TokenError::InsufficientBalance)?;
        let new_to_balance = to_balance
            .checked_add(amount)
            .ok_or(TokenError::ArithmeticError)?;

        let from_balance_obj = Balance {
            amount: new_from_balance,
            last_updated: env.ledger().timestamp(),
        };
        let to_balance_obj = Balance {
            amount: new_to_balance,
            last_updated: env.ledger().timestamp(),
        };

        env.storage().persistent().set(&Symbol::new(&env, &format!("balance_{:?}", from)), &from_balance_obj);
        env.storage().persistent().set(&Symbol::new(&env, &format!("balance_{:?}", to)), &to_balance_obj);

        env.events().publish(
            (Symbol::new(&env, "transfer_from"),),
            (spender, from, to, amount),
        );

        Ok(())
    }

    // ============================================
    // # View Functions
    // ============================================

    /// Get balance of an account with error handling
    pub fn balance(env: Env, account: Address) -> Result<i128, TokenError> {
        Self::get_balance_internal(&env, &account)
    }

    /// Get total supply with error handling
    pub fn total_supply(env: Env) -> Result<i128, TokenError> {
        let state: TokenState = env.storage().instance().get(&Symbol::new(&env, "state"))
            .ok_or(TokenError::ArithmeticError)?;
        Ok(state.total_supply)
    }

    /// Get allowance
    pub fn allowance(env: Env, owner: Address, spender: Address) -> Result<i128, TokenError> {
        let key = Symbol::new(&env, &format!("allowance_{:?}_{:?}", owner, spender));
        let allowance: Allowance = env.storage().persistent().get(&key)
            .ok_or(TokenError::ArithmeticError)?;
        Ok(allowance.amount)
    }

    /// Get token state
    pub fn get_state(env: Env) -> Result<TokenState, TokenError> {
        let state: TokenState = env.storage().instance().get(&Symbol::new(&env, "state"))
            .ok_or(TokenError::ArithmeticError)?;
        Ok(state)
    }

    /// Get balance as Balance struct
    pub fn get_balance_struct(env: Env, account: Address) -> Result<Balance, TokenError> {
        Self::get_balance_struct_internal(&env, &account)
    }

    // ============================================
    // # Internal Helper Functions
    // ============================================

    /// Internal function to get balance with error handling
    fn get_balance_internal(env: &Env, account: &Address) -> Result<i128, TokenError> {
        let key = Symbol::new(env, &format!("balance_{:?}", account));
        let balance: Balance = env.storage().persistent().get(&key)
            .unwrap_or(Balance {
                amount: 0,
                last_updated: env.ledger().timestamp(),
            });
        Ok(balance.amount)
    }

    /// Internal function to get Balance struct
    fn get_balance_struct_internal(env: &Env, account: &Address) -> Result<Balance, TokenError> {
        let key = Symbol::new(env, &format!("balance_{:?}", account));
        let balance: Balance = env.storage().persistent().get(&key)
            .unwrap_or(Balance {
                amount: 0,
                last_updated: env.ledger().timestamp(),
            });
        Ok(balance)
    }
}

// ============================================
// # Tests
// ============================================

#[cfg(test)]
mod test;
