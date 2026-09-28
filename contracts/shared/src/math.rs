//! Overflow-safe arithmetic helpers shared across contracts.
//!
//! Contract arithmetic must never silently wrap or rely on the build
//! profile's `overflow-checks` setting to catch bad math. These helpers
//! make the check explicit and produce a clear panic message instead of
//! an opaque VM trap.

use crate::constants::BASIS_POINTS_DENOMINATOR;
use crate::errors::SharedError;

// ---------------------------------------------------------------------------
// Checked helpers (panic with an explicit message on overflow / zero divisor)
// ---------------------------------------------------------------------------

pub fn checked_add_i128(a: i128, b: i128) -> i128 {
    a.checked_add(b).expect("math: i128 addition overflow")
}

pub fn checked_sub_i128(a: i128, b: i128) -> i128 {
    a.checked_sub(b).expect("math: i128 subtraction overflow")
}

pub fn checked_mul_i128(a: i128, b: i128) -> i128 {
    a.checked_mul(b).expect("math: i128 multiplication overflow")
}

/// Integer division that reports a zero divisor explicitly.
///
/// A bare `a / 0` traps in the VM with no indication of which computation
/// failed; this names it.
pub fn checked_div_i128(a: i128, b: i128) -> i128 {
    assert!(b != 0, "math: i128 division by zero");
    a.checked_div(b).expect("math: i128 division overflow")
}

/// Computes `(a * b) / denominator` — the proportional-share formula that
/// AMM and fee math is built from.
///
/// The intermediate product is a checked `i128`: if `a * b` exceeds `i128`
/// this panics rather than wrapping. It does *not* promote to a wider type,
/// so a mathematically-representable result can still be rejected when the
/// intermediate does not fit. That fails closed, which is the right default
/// for value-moving code.
pub fn checked_mul_div_i128(a: i128, b: i128, denominator: i128) -> i128 {
    checked_div_i128(checked_mul_i128(a, b), denominator)
}

pub fn checked_add_u32(a: u32, b: u32) -> u32 {
    a.checked_add(b).expect("math: u32 addition overflow")
}

pub fn checked_sub_u32(a: u32, b: u32) -> u32 {
    a.checked_sub(b).expect("math: u32 subtraction overflow")
}

// ---------------------------------------------------------------------------
// `Result`-returning helpers (for callers that prefer propagating SharedError)
// ---------------------------------------------------------------------------

/// Calculate percentage with basis points (bps)
/// where 100 bps = 1%
pub fn calculate_bps(amount: i128, bps: i128) -> Result<i128, SharedError> {
    if amount < 0 {
        return Err(SharedError::InvalidAmount);
    }
    if bps < 0 || bps > BASIS_POINTS_DENOMINATOR {
        return Err(SharedError::InvalidInput);
    }
    if bps == 0 {
        return Ok(0);
    }

    let result = amount
        .checked_mul(bps)
        .ok_or(SharedError::ArithmeticOverflow)?
        .checked_div(BASIS_POINTS_DENOMINATOR)
        .ok_or(SharedError::OperationFailed)?;

    Ok(result)
}

/// Check if a value is within bounds
pub fn is_within_bounds(value: i128, min: i128, max: i128) -> bool {
    value >= min && value <= max
}

/// Clamp a value between min and max
pub fn clamp(value: i128, min: i128, max: i128) -> i128 {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

/// Safe addition with overflow check
pub fn safe_add(a: i128, b: i128) -> Result<i128, SharedError> {
    a.checked_add(b).ok_or(SharedError::ArithmeticOverflow)
}

/// Safe subtraction with overflow check
pub fn safe_sub(a: i128, b: i128) -> Result<i128, SharedError> {
    a.checked_sub(b).ok_or(SharedError::ArithmeticOverflow)
}

/// Safe multiplication with overflow check
pub fn safe_mul(a: i128, b: i128) -> Result<i128, SharedError> {
    a.checked_mul(b).ok_or(SharedError::ArithmeticOverflow)
}

/// Safe division with overflow check
pub fn safe_div(a: i128, b: i128) -> Result<i128, SharedError> {
    if b == 0 {
        return Err(SharedError::InvalidInput);
    }
    a.checked_div(b).ok_or(SharedError::OperationFailed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_i128_ok() {
        assert_eq!(checked_add_i128(1, 2), 3);
    }

    #[test]
    #[should_panic(expected = "math: i128 addition overflow")]
    fn add_i128_overflow() {
        checked_add_i128(i128::MAX, 1);
    }

    #[test]
    #[should_panic(expected = "math: i128 subtraction overflow")]
    fn sub_i128_overflow() {
        checked_sub_i128(i128::MIN, 1);
    }

    #[test]
    #[should_panic(expected = "math: u32 subtraction overflow")]
    fn sub_u32_underflow() {
        checked_sub_u32(0, 1);
    }

    #[test]
    #[should_panic(expected = "math: i128 division by zero")]
    fn div_i128_by_zero() {
        checked_div_i128(1, 0);
    }

    #[test]
    fn mul_div_i128_ok() {
        assert_eq!(checked_mul_div_i128(10, 3, 4), 7); // truncates toward zero
        assert_eq!(checked_mul_div_i128(0, 5, 7), 0);
    }

    #[test]
    #[should_panic(expected = "math: i128 division by zero")]
    fn mul_div_i128_zero_denominator() {
        checked_mul_div_i128(1, 1, 0);
    }

    #[test]
    #[should_panic(expected = "math: i128 multiplication overflow")]
    fn mul_div_i128_intermediate_overflow() {
        // The result would fit in i128, but the a*b intermediate does not.
        checked_mul_div_i128(i128::MAX, 2, 2);
    }

    #[test]
    fn calculate_bps_ok() {
        assert_eq!(calculate_bps(1_000, 500).unwrap(), 50);
        assert_eq!(calculate_bps(1_000, 0).unwrap(), 0);
    }

    #[test]
    fn calculate_bps_rejects_bad_input() {
        assert_eq!(calculate_bps(-1, 100), Err(SharedError::InvalidAmount));
        assert_eq!(
            calculate_bps(1_000, BASIS_POINTS_DENOMINATOR + 1),
            Err(SharedError::InvalidInput)
        );
    }

    #[test]
    fn bounds_and_clamp() {
        assert!(is_within_bounds(5, 0, 10));
        assert!(!is_within_bounds(15, 0, 10));
        assert_eq!(clamp(15, 0, 10), 10);
        assert_eq!(clamp(-5, 0, 10), 0);
        assert_eq!(clamp(5, 0, 10), 5);
    }

    #[test]
    fn safe_ops_ok_and_err() {
        assert_eq!(safe_add(1, 2).unwrap(), 3);
        assert_eq!(safe_sub(5, 3).unwrap(), 2);
        assert_eq!(safe_mul(3, 4).unwrap(), 12);
        assert_eq!(safe_div(10, 2).unwrap(), 5);
        assert_eq!(safe_div(1, 0), Err(SharedError::InvalidInput));
        assert_eq!(
            safe_mul(i128::MAX, 2),
            Err(SharedError::ArithmeticOverflow)
        );
    }
}
