//! Strategy Interface
//!
//! Trait for implementing trading strategies.

use super::signal::{MarketData, Signal};

/// Strategy trait for implementing trading algorithms.
///
/// Strategies are stateless and thread-safe.
pub trait Strategy: Send + Sync {
    /// Evaluates market data and optionally returns a signal.
    fn evaluate(&self, market_data: &MarketData) -> Option<Signal>;

    /// Returns the strategy name.
    fn name(&self) -> &str;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MovingAverageCrossover {
        short_period: usize,
        long_period: usize,
    }

    impl Strategy for MovingAverageCrossover {
        fn evaluate(&self, _market_data: &MarketData) -> Option<Signal> {
            None
        }

        fn name(&self) -> &str {
            "MovingAverageCrossover"
        }
    }

    #[test]
    fn test_strategy_trait() {
        let strategy = MovingAverageCrossover {
            short_period: 10,
            long_period: 20,
        };
        assert_eq!(strategy.name(), "MovingAverageCrossover");
    }
}
