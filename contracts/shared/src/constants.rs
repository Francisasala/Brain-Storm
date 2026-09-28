//! Shared constants used across all contracts
//!
//! This module defines magic numbers extracted from contract code to improve
//! readability, auditability, and maintainability. All numeric constants should
//! be defined here and imported where needed.
//!
//! # Categories
//! - **Basis Points**: Calculations using basis points (1 bps = 0.01%)
//! - **Precision Scaling**: Fixed-point arithmetic scaling factors
//! - **Time Values**: Ledger-related timing constants (in ledgers, not seconds)

// ============================================================================
// Basis Points Constants
// ============================================================================

/// Basis points denominator for percentage calculations.
///
/// 100 basis points = 1%
/// 10,000 basis points = 100%
///
/// # Usage
/// To calculate X% of amount: `(amount * bps) / BASIS_POINTS_DENOMINATOR`
pub const BASIS_POINTS_DENOMINATOR: i128 = 10_000;

// ============================================================================
// Precision Scaling Constants
// ============================================================================

/// Precision scale for reward-per-token accounting (10^12).
///
/// Used in staking to avoid precision loss when dividing by total staked.
/// Stores accumulator values multiplied by this factor, then divides during
/// pending reward calculation.
///
/// # Example
/// ```ignore
/// let increment = rate
///     .checked_mul(EPOCH_LENGTH)
///     .checked_mul(PRECISION_SCALE_12)
///     .checked_div(total_staked);
/// ```
pub const PRECISION_SCALE_12: i128 = 1_000_000_000_000;

// ============================================================================
// Ledger Timing Constants
// ============================================================================

/// Default minimum TTL for temporary storage entries (in ledgers).
///
/// Used in ledger operation setup. Equivalent to ~5 seconds on Stellar testnet
/// (assuming ~20ms block time).
pub const MIN_TEMP_ENTRY_TTL: u32 = 1_000;

/// Default minimum TTL for persistent storage entries (in ledgers).
///
/// Used in ledger operation setup. Equivalent to ~20 seconds on Stellar testnet.
pub const MIN_PERSISTENT_ENTRY_TTL: u32 = 1_000;

/// Default maximum TTL for storage entries (in ledgers).
///
/// Used in ledger operation setup. Equivalent to ~27 minutes on Stellar testnet.
pub const MAX_ENTRY_TTL: u32 = 100_000;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basis_points_constants() {
        assert_eq!(BASIS_POINTS_DENOMINATOR, 10_000);
    }

    #[test]
    fn test_precision_scale_constants() {
        assert_eq!(PRECISION_SCALE_12, 1_000_000_000_000);
    }

    #[test]
    fn test_ledger_ttl_constants() {
        assert!(MIN_TEMP_ENTRY_TTL <= MAX_ENTRY_TTL);
        assert!(MIN_PERSISTENT_ENTRY_TTL <= MAX_ENTRY_TTL);
    }
}
