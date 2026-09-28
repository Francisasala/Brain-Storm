//! Shared oracle (price-feed) validation helpers.

use crate::errors::SharedError;
use soroban_sdk::String;

/// A price sample reported by an oracle feed.
pub struct Price {
    pub value: i128,
    pub timestamp: u64,
    pub source: String,
}

/// Validate oracle price freshness
pub fn validate_freshness(
    timestamp: u64,
    current_time: u64,
    max_age: u64,
) -> Result<(), SharedError> {
    if current_time < timestamp {
        return Err(SharedError::InvalidInput);
    }
    if current_time - timestamp > max_age {
        return Err(SharedError::Timeout);
    }
    Ok(())
}

/// Validate oracle price is reasonable
pub fn validate_price(
    price: i128,
    min_price: i128,
    max_price: i128,
) -> Result<(), SharedError> {
    if price < min_price || price > max_price {
        return Err(SharedError::InvalidInput);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::Env;

    #[test]
    fn freshness_accepts_recent_sample() {
        assert!(validate_freshness(1_000, 1_100, 60).is_ok());
        assert!(validate_freshness(1_000, 1_060, 60).is_ok());
    }

    #[test]
    fn freshness_rejects_future_sample() {
        assert_eq!(
            validate_freshness(2_000, 1_000, 60),
            Err(SharedError::InvalidInput)
        );
    }

    #[test]
    fn freshness_rejects_stale_sample() {
        assert_eq!(
            validate_freshness(1_000, 1_100, 30),
            Err(SharedError::Timeout)
        );
    }

    #[test]
    fn price_bounds() {
        assert!(validate_price(100, 1, 1_000).is_ok());
        assert_eq!(
            validate_price(0, 1, 1_000),
            Err(SharedError::InvalidInput)
        );
        assert_eq!(
            validate_price(1_001, 1, 1_000),
            Err(SharedError::InvalidInput)
        );
    }

    #[test]
    fn price_struct_is_constructible() {
        let env = Env::default();
        let p = Price {
            value: 42,
            timestamp: 7,
            source: String::from_str(&env, "feed"),
        };
        assert_eq!(p.value, 42);
        assert_eq!(p.timestamp, 7);
    }

    /// Verify an oracle price is still within its staleness window before use.
    ///
    /// Returns `Ok(price)` only when `current_time - timestamp <= max_age`;
    /// otherwise returns an error so the caller must not use the stale value.
    pub fn validate_oracle_price(
        price: &Price,
        current_time: u64,
        max_age: u64,
    ) -> Result<i128, SharedError> {
        validate_freshness(price.timestamp, current_time, max_age)?;
        validate_price(price.value, 0, i128::MAX)?;
        Ok(price.value)
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use soroban_sdk::Env;

        #[test]
        fn freshness_accepts_recent_sample() {
            assert!(validate_freshness(1_000, 1_000, 60).is_ok());
            assert!(validate_freshness(1_000, 1_059, 60).is_ok());
        }

        #[test]
        fn freshness_rejects_future_sample() {
            assert_eq!(
                validate_freshness(2_000, 1_000, 60),
                Err(SharedError::InvalidInput)
            );
        }

        #[test]
        fn freshness_rejects_stale_sample() {
            assert_eq!(
                validate_freshness(1_000, 1_100, 30),
                Err(SharedError::Timeout)
            );
        }

        #[test]
        fn price_bounds() {
            assert!(validate_price(100, 1, 1_000).is_ok());
            assert_eq!(
                validate_price(0, 1, 1_000),
                Err(SharedError::InvalidInput)
            );
            assert_eq!(
                validate_price(1_001, 1, 1_000),
                Err(SharedError::InvalidInput)
            );
        }

        #[test]
        fn validate_oracle_price_fresh() {
            let env = Env::default();
            let ts = env.ledger().timestamp();
            let price = Price {
                value: 500,
                timestamp: ts,
                source: soroban_sdk::String::from_str(&env, "feed"),
            };
            assert!(validate_oracle_price(&price, ts, 60).is_ok());
        }

        #[test]
        fn validate_oracle_price_stale() {
            let env = Env::default();
            let price = Price {
                value: 500,
                timestamp: 0,
                source: soroban_sdk::String::from_str(&env, "feed"),
            };
            assert_eq!(
                validate_oracle_price(&price, 1000, 60),
                Err(SharedError::Timeout)
            );
        }
    }
}