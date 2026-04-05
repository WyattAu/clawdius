//! Strategy Interface
//!
//! Trait for implementing trading strategies.
//!
//! Includes a production `MovingAverageCrossover` implementation that uses
//! interior mutability (Mutex) for price history, maintaining the
//! `Strategy: Send + Sync` contract.

use super::signal::{MarketData, Signal, SignalDirection};
use std::sync::Mutex;

/// Strategy trait for implementing trading algorithms.
///
/// Strategies are thread-safe (Send + Sync). Stateful strategies
/// use interior mutability (e.g., Mutex) for internal buffers.
pub trait Strategy: Send + Sync {
    /// Evaluates market data and optionally returns a signal.
    fn evaluate(&self, market_data: &MarketData) -> Option<Signal>;

    /// Returns the strategy name.
    fn name(&self) -> &str;
}

/// Moving Average Crossover Strategy.
///
/// Generates buy signals when the short-period SMA crosses above the
/// long-period SMA, and sell signals when it crosses below.
///
/// Uses a ring-buffer approach: maintains up to `max(short, long)` prices
/// per symbol, computing SMAs on each tick.
///
/// State is protected by `Mutex` for thread safety.
pub struct MovingAverageCrossover {
    /// Short moving average period (e.g., 5).
    short_period: usize,
    /// Long moving average period (e.g., 20).
    long_period: usize,
    /// Per-symbol price history.
    prices: Mutex<std::collections::HashMap<String, Vec<f64>>>,
    /// Per-symbol previous relationship: true = short was above long.
    prev_state: Mutex<std::collections::HashMap<String, bool>>,
    /// Minimum confidence threshold for signal generation.
    min_confidence: f64,
}

impl MovingAverageCrossover {
    /// Create a new MA crossover strategy.
    ///
    /// # Panics
    /// Panics if `short_period == 0`, `long_period == 0`, or `short_period >= long_period`.
    pub fn new(short_period: usize, long_period: usize) -> Self {
        assert!(short_period > 0, "short_period must be > 0");
        assert!(long_period > 0, "long_period must be > 0");
        assert!(
            short_period < long_period,
            "short_period ({short_period}) must be < long_period ({long_period})"
        );
        Self {
            short_period,
            long_period,
            prices: Mutex::new(std::collections::HashMap::new()),
            prev_state: Mutex::new(std::collections::HashMap::new()),
            min_confidence: 0.0,
        }
    }

    /// Create with custom confidence threshold.
    pub fn with_confidence(short_period: usize, long_period: usize, min_confidence: f64) -> Self {
        let mut s = Self::new(short_period, long_period);
        s.min_confidence = min_confidence.max(0.0).min(1.0);
        s
    }

    /// Compute simple moving average over the last `period` prices.
    fn sma(prices: &[f64], period: usize) -> Option<f64> {
        if prices.len() < period {
            return None;
        }
        let start = prices.len() - period;
        let sum: f64 = prices[start..].iter().sum();
        Some(sum / period as f64)
    }

    /// Compute confidence based on crossover magnitude.
    ///
    /// Confidence = |short_ma - long_ma| / long_ma, clamped to [0, 1].
    fn compute_confidence(short_ma: f64, long_ma: f64) -> f64 {
        if long_ma.abs() < 1e-10 {
            return 0.5;
        }
        let ratio = (short_ma - long_ma).abs() / long_ma.abs();
        ratio.min(1.0).max(0.0)
    }
}

impl Strategy for MovingAverageCrossover {
    fn evaluate(&self, market_data: &MarketData) -> Option<Signal> {
        let price: f64 = market_data.price.to_string().parse().unwrap_or(0.0);
        if price <= 0.0 {
            return None;
        }

        let symbol = &market_data.symbol;

        let (short_ma, long_ma, was_above) = {
            let mut prices = self.prices.lock().expect("prices lock poisoned");
            let entry = prices.entry(symbol.clone()).or_default();
            entry.push(price);
            if entry.len() > self.long_period {
                let drain_from = entry.len() - self.long_period;
                entry.drain(0..drain_from);
            }

            let short_ma = Self::sma(entry, self.short_period);
            let long_ma = Self::sma(entry, self.long_period);

            let prev = self.prev_state.lock().expect("prev_state lock poisoned");
            let was_above = prev.get(symbol).copied().unwrap_or(false);

            (short_ma, long_ma, was_above)
        };

        let short_ma = short_ma?;
        let long_ma = long_ma?;

        let is_above = short_ma > long_ma;
        let crossed = is_above != was_above;

        self.prev_state
            .lock()
            .expect("prev_state lock poisoned")
            .insert(symbol.clone(), is_above);

        if !crossed {
            return None;
        }

        let confidence = Self::compute_confidence(short_ma, long_ma);
        if confidence < self.min_confidence {
            return None;
        }

        let direction = if is_above {
            SignalDirection::Buy
        } else {
            SignalDirection::Sell
        };

        Some(Signal {
            symbol: symbol.clone(),
            direction,
            confidence,
            strategy: self.name().to_string(),
            timestamp: market_data.timestamp,
        })
    }

    fn name(&self) -> &'static str {
        "MovingAverageCrossover"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    fn make_market_data(symbol: &str, price: f64, timestamp: u64) -> MarketData {
        MarketData {
            symbol: symbol.to_string(),
            price: Decimal::from_f64_retain(price).unwrap_or(Decimal::ZERO),
            volume: Decimal::from(1000),
            timestamp,
        }
    }

    fn feed_prices(
        strategy: &MovingAverageCrossover,
        symbol: &str,
        prices: &[f64],
        start_ts: u64,
    ) -> Vec<Signal> {
        let mut signals = Vec::new();
        for (i, &price) in prices.iter().enumerate() {
            let md = make_market_data(symbol, price, start_ts + i as u64);
            if let Some(sig) = strategy.evaluate(&md) {
                signals.push(sig);
            }
        }
        signals
    }

    #[test]
    fn test_ma_crossover_name() {
        let strategy = MovingAverageCrossover::new(3, 5);
        assert_eq!(strategy.name(), "MovingAverageCrossover");
    }

    #[test]
    fn test_ma_crossover_insufficient_data() {
        let strategy = MovingAverageCrossover::new(5, 10);
        let md = make_market_data("AAPL", 100.0, 0);
        assert!(strategy.evaluate(&md).is_none());
    }

    #[test]
    fn test_ma_crossover_buy_signal() {
        let strategy = MovingAverageCrossover::new(3, 5);
        let signals = feed_prices(&strategy, "AAPL", &[100.0, 100.0, 100.0, 100.0, 110.0], 0);
        assert!(!signals.is_empty());
        assert_eq!(signals.last().unwrap().direction, SignalDirection::Buy);
    }

    #[test]
    fn test_ma_crossover_sell_signal() {
        let strategy = MovingAverageCrossover::new(3, 5);

        // Uptrend establishes was_above=true
        let buy_sigs = feed_prices(&strategy, "AAPL", &[100.0, 100.0, 100.0, 100.0, 110.0], 0);
        assert!(!buy_sigs.is_empty());
        assert_eq!(buy_sigs.last().unwrap().direction, SignalDirection::Buy);

        // Downtrend: first tick where short_ma <= long_ma triggers Sell crossover
        let sell_sigs = feed_prices(&strategy, "AAPL", &[90.0, 80.0, 70.0], 5);
        assert!(!sell_sigs.is_empty());
        assert_eq!(sell_sigs[0].direction, SignalDirection::Sell);
    }

    #[test]
    fn test_ma_crossover_multi_symbol_independent() {
        let strategy = MovingAverageCrossover::new(2, 3);

        // A: uptrend → Buy
        let sig_a = feed_prices(&strategy, "A", &[100.0, 100.0, 110.0], 0);
        assert!(!sig_a.is_empty());
        assert_eq!(sig_a.last().unwrap().direction, SignalDirection::Buy);

        // B: establish uptrend then cross down → Sell
        let sig_b = feed_prices(&strategy, "B", &[200.0, 200.0, 210.0, 190.0], 0);
        assert!(sig_b.len() >= 2);
        assert_eq!(sig_b.last().unwrap().direction, SignalDirection::Sell);
    }

    #[test]
    fn test_ma_crossover_confidence_threshold() {
        let strategy = MovingAverageCrossover::with_confidence(3, 5, 0.99);
        let signals = feed_prices(&strategy, "AAPL", &[100.0, 100.0, 100.0, 100.0, 100.5], 0);
        assert!(signals.is_empty());
    }

    #[test]
    fn test_ma_crossover_zero_price_ignored() {
        let strategy = MovingAverageCrossover::new(2, 3);
        let md = make_market_data("AAPL", 0.0, 0);
        assert!(strategy.evaluate(&md).is_none());
    }

    #[test]
    #[should_panic(expected = "short_period must be > 0")]
    fn test_ma_crossover_zero_short_period() {
        let _ = MovingAverageCrossover::new(0, 5);
    }

    #[test]
    #[should_panic(expected = "long_period must be > 0")]
    fn test_ma_crossover_zero_long_period() {
        let _ = MovingAverageCrossover::new(3, 0);
    }

    #[test]
    #[should_panic(expected = "short_period")]
    fn test_ma_crossover_short_ge_long() {
        let _ = MovingAverageCrossover::new(5, 5);
    }

    #[test]
    fn test_sma_computation() {
        assert_eq!(
            MovingAverageCrossover::sma(&[10.0, 20.0, 30.0], 3),
            Some(20.0)
        );
        assert_eq!(
            MovingAverageCrossover::sma(&[10.0, 20.0, 30.0, 40.0], 3),
            Some(30.0)
        );
        assert_eq!(MovingAverageCrossover::sma(&[10.0], 3), None);
        assert_eq!(MovingAverageCrossover::sma(&[], 3), None);
    }

    #[test]
    fn test_confidence_computation() {
        let c = MovingAverageCrossover::compute_confidence(110.0, 100.0);
        assert!((c - 0.1).abs() < 1e-10);
        let c = MovingAverageCrossover::compute_confidence(100.0, 100.0);
        assert!(c.abs() < 1e-10);
        let c = MovingAverageCrossover::compute_confidence(200.0, 100.0);
        assert!((c - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_ma_crossover_trend_reversal() {
        let strategy = MovingAverageCrossover::new(3, 5);

        // Uptrend → Buy
        let up = feed_prices(&strategy, "X", &[100.0, 101.0, 102.0, 103.0, 104.0], 0);
        assert!(!up.is_empty());
        assert_eq!(up.last().unwrap().direction, SignalDirection::Buy);

        // Downtrend → Sell
        let down = feed_prices(&strategy, "X", &[90.0, 80.0, 70.0], 5);
        assert!(!down.is_empty());
        assert_eq!(down[0].direction, SignalDirection::Sell);
    }
}
