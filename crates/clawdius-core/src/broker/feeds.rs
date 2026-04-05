//! Market Data Feed
//!
//! Provides synthetic market data for testing and simulation.
//! Includes a `SimulatedFeed` with Geometric Brownian Motion (GBM) price dynamics.

use std::collections::HashMap;
use std::sync::Mutex;

// ─── Types ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Symbol(pub String);

#[derive(Debug, Clone)]
pub struct Quote {
    pub symbol: Symbol,
    pub bid: f64,
    pub ask: f64,
    pub timestamp: u64,
}

#[derive(Debug, Clone)]
pub enum MarketUpdate {
    Quote(Quote),
    Heartbeat,
}

pub trait MarketFeed: Send + Sync {
    fn subscribe(&mut self, symbols: &[Symbol]) -> Result<(), FeedError>;
    fn status(&self) -> FeedStatus;
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FeedStatus {
    Disconnected,
    Connected,
    Error,
}

#[derive(Debug, thiserror::Error)]
pub enum FeedError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Not connected")]
    NotConnected,
}

// ─── GBM Configuration ─────────────────────────────────────────────────────

/// Configuration for Geometric Brownian Motion price simulation.
///
/// Per YP-HFT-BROKER-001, the GBM model:
///   S(t+dt) = S(t) * exp((μ - σ²/2)*dt + σ*√dt*Z)
/// where Z ~ N(0,1).
#[derive(Debug, Clone)]
pub struct GbmConfig {
    /// Annualized drift (μ). Default: 0.05 (5% annual return).
    pub drift: f64,
    /// Annualized volatility (σ). Default: 0.20 (20%).
    pub volatility: f64,
    /// Time step in years (dt). Default: 1.0 / 252.0 (one trading day).
    pub dt: f64,
    /// Half-spread in basis points. Default: 5 (0.05%).
    pub spread_bps: f64,
    /// RNG seed for reproducibility. Default: 42.
    pub seed: u64,
}

impl Default for GbmConfig {
    fn default() -> Self {
        Self {
            drift: 0.05,
            volatility: 0.20,
            dt: 1.0 / 252.0,
            spread_bps: 5.0,
            seed: 42,
        }
    }
}

impl GbmConfig {
    /// Create a low-volatility config for stable testing.
    pub fn low_volatility() -> Self {
        Self {
            drift: 0.05,
            volatility: 0.05,
            dt: 1.0 / 252.0,
            spread_bps: 2.0,
            seed: 42,
        }
    }

    /// Create a high-volatility config for stress testing.
    pub fn high_volatility() -> Self {
        Self {
            drift: 0.0,
            volatility: 0.60,
            dt: 1.0 / 252.0,
            spread_bps: 10.0,
            seed: 123,
        }
    }
}

// ─── Xoshiro256++ PRNG ────────────────────────────────────────────────────

/// Fast xoshiro256++ PRNG. Zero external dependencies.
/// Period: 2²⁵⁶ - 1. Passes TestU01 BigCrush.
/// Reference: Sebastiano Vigna, 2022.
struct Xoshiro256 {
    s: [u64; 4],
}

impl Xoshiro256 {
    fn new(seed: u64) -> Self {
        // SplitMix64 to expand seed into 4 state words.
        let mut z = seed;
        let mut s = [0u64; 4];
        for word in &mut s {
            z = z.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut t = z;
            t = (t ^ (t >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            t = (t ^ (t >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            t ^= t >> 31;
            *word = t;
        }
        Self { s }
    }

    /// Advance state and return a u64 in [0, u64::MAX].
    #[inline]
    fn next_u64(&mut self) -> u64 {
        let result = Self::rotl(self.s[0].wrapping_add(self.s[3]), 23).wrapping_add(self.s[0]);

        let t = self.s[1] << 17;
        self.s[2] ^= self.s[0];
        self.s[3] ^= self.s[1];
        self.s[1] ^= self.s[2];
        self.s[0] ^= self.s[3];
        self.s[2] ^= t;
        self.s[3] = Self::rotl(self.s[3], 45);

        result
    }

    /// Return a f64 in [0, 1).
    #[inline]
    fn next_f64(&mut self) -> f64 {
        // Take the top 53 bits for full f64 precision.
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    #[inline]
    fn rotl(x: u64, k: u32) -> u64 {
        x.rotate_left(k)
    }
}

// ─── Box-Muller Transform ─────────────────────────────────────────────────

/// Box-Muller transform: two uniform [0,1) samples → two standard normal samples.
/// Uses the "polar form" for numerical stability (avoids sin/cos near zero).
struct BoxMuller {
    rng: Xoshiro256,
    /// Cached second sample from the previous pair.
    spare: Option<f64>,
}

impl BoxMuller {
    fn new(seed: u64) -> Self {
        Self {
            rng: Xoshiro256::new(seed),
            spare: None,
        }
    }

    /// Generate a standard normal sample N(0,1).
    fn next_normal(&mut self) -> f64 {
        if let Some(z) = self.spare.take() {
            return z;
        }

        // Polar form of Box-Muller.
        // Standard: z0 = sqrt(-2*ln(s)/s) * x1, z1 = sqrt(-2*ln(s)/s) * x2
        // where s = x1^2 + x2^2, x1 = 2*u1-1, x2 = 2*u2-1.
        loop {
            let u1 = self.rng.next_f64();
            let u2 = self.rng.next_f64();
            let x1 = 2.0 * u1 - 1.0;
            let x2 = 2.0 * u2 - 1.0;
            let s = x1 * x1 + x2 * x2;

            if s > 0.0 && s < 1.0 {
                let factor = (-2.0 * s.ln() / s).sqrt();
                let z0 = factor * x1;
                let z1 = factor * x2;
                self.spare = Some(z1);
                return z0;
            }
        }
    }
}

// ─── Simulated Feed ───────────────────────────────────────────────────────

/// Simulated market data feed with GBM price dynamics.
///
/// Generates synthetic quotes using Geometric Brownian Motion:
///   S(t+dt) = S(t) * exp((μ - σ²/2)*dt + σ*√dt*Z)
///
/// Thread-safe: uses `Mutex` for interior mutability of prices.
pub struct SimulatedFeed {
    status: FeedStatus,
    symbols: Vec<Symbol>,
    /// Current prices per symbol (mutable via Mutex).
    prices: Mutex<HashMap<String, f64>>,
    /// GBM configuration.
    gbm: GbmConfig,
    /// Per-symbol BoxMuller normal RNGs (keyed by symbol index).
    rngs: Mutex<Vec<BoxMuller>>,
    /// Monotonic tick counter for round-robin symbol selection.
    tick: Mutex<u64>,
}

impl SimulatedFeed {
    /// Create a new disconnected feed with default GBM parameters.
    pub fn new() -> Self {
        Self {
            status: FeedStatus::Disconnected,
            symbols: Vec::new(),
            prices: Mutex::new(HashMap::new()),
            gbm: GbmConfig::default(),
            rngs: Mutex::new(Vec::new()),
            tick: Mutex::new(0),
        }
    }

    /// Create a feed with symbols and initial prices.
    ///
    /// Uses default GBM configuration. Each symbol gets its own
    /// independent RNG seeded from the base seed + symbol index.
    pub fn with_symbols(symbols: &[&str], base_prices: &[(&str, f64)]) -> Self {
        let mut feed = Self::new();
        let mut prices = HashMap::new();
        for (sym, price) in base_prices {
            prices.insert(sym.to_string(), *price);
        }
        for sym in symbols {
            feed.symbols.push(Symbol(sym.to_string()));
            prices.entry(sym.to_string()).or_insert(100.0);
        }
        // Create one RNG per symbol, each with a unique seed.
        let rngs: Vec<BoxMuller> = symbols
            .iter()
            .enumerate()
            .map(|(i, _)| BoxMuller::new(feed.gbm.seed.wrapping_add(i as u64)))
            .collect();
        *feed.prices.lock().expect("prices lock poisoned") = prices;
        *feed.rngs.lock().expect("rngs lock poisoned") = rngs;
        feed.status = FeedStatus::Connected;
        feed
    }

    /// Create a feed with custom GBM configuration.
    pub fn with_gbm_config(symbols: &[&str], base_prices: &[(&str, f64)], gbm: GbmConfig) -> Self {
        let mut feed = Self::new();
        feed.gbm = gbm;
        let mut prices = HashMap::new();
        for (sym, price) in base_prices {
            prices.insert(sym.to_string(), *price);
        }
        for sym in symbols {
            feed.symbols.push(Symbol(sym.to_string()));
            prices.entry(sym.to_string()).or_insert(100.0);
        }
        let rngs: Vec<BoxMuller> = symbols
            .iter()
            .enumerate()
            .map(|(i, _)| BoxMuller::new(feed.gbm.seed.wrapping_add(i as u64)))
            .collect();
        *feed.prices.lock().expect("prices lock poisoned") = prices;
        *feed.rngs.lock().expect("rngs lock poisoned") = rngs;
        feed.status = FeedStatus::Connected;
        feed
    }

    pub fn set_status(&mut self, status: FeedStatus) {
        self.status = status;
    }

    /// Get the current mid-price for a symbol.
    pub fn current_price(&self, symbol: &str) -> Option<f64> {
        self.prices
            .lock()
            .expect("prices lock poisoned")
            .get(symbol)
            .copied()
    }

    /// Get all current prices.
    pub fn all_prices(&self) -> HashMap<String, f64> {
        self.prices.lock().expect("prices lock poisoned").clone()
    }

    /// Generate a quote using GBM price evolution.
    ///
    /// Each call advances the price of one symbol (round-robin) by one GBM step.
    /// Returns `Heartbeat` if no symbols are subscribed.
    pub fn generate_quote(&self) -> MarketUpdate {
        if self.symbols.is_empty() {
            return MarketUpdate::Heartbeat;
        }

        // Round-robin: pick the next symbol.
        let idx = {
            let mut tick = self.tick.lock().expect("tick lock poisoned");
            let i = (*tick as usize) % self.symbols.len();
            *tick = tick.wrapping_add(1);
            i
        };

        let symbol = &self.symbols[idx];

        // Evolve price via GBM.
        let new_price = {
            let mut prices = self.prices.lock().expect("prices lock poisoned");
            let mut rngs = self.rngs.lock().expect("rngs lock poisoned");

            let current = prices.get(&symbol.0).copied().unwrap_or(100.0);

            // GBM step: S(t+dt) = S(t) * exp((μ - σ²/2)*dt + σ*√dt*Z)
            let mu = self.gbm.drift;
            let sigma = self.gbm.volatility;
            let dt = self.gbm.dt;
            let z = rngs[idx].next_normal();

            let drift_component = (mu - sigma * sigma / 2.0) * dt;
            let diffusion_component = sigma * dt.sqrt() * z;
            let new_price = current * (drift_component + diffusion_component).exp();

            // Clamp to positive (prices cannot be zero or negative).
            let clamped = new_price.max(0.0001);

            prices.insert(symbol.0.clone(), clamped);
            clamped
        };

        // Compute bid/ask with configurable spread.
        let half_spread = new_price * (self.gbm.spread_bps / 10_000.0) / 2.0;
        let bid = new_price - half_spread;
        let ask = new_price + half_spread;

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        MarketUpdate::Quote(Quote {
            symbol: symbol.clone(),
            bid,
            ask,
            timestamp,
        })
    }

    /// Generate N quotes, returning all market updates.
    pub fn generate_n_quotes(&self, n: usize) -> Vec<MarketUpdate> {
        (0..n).map(|_| self.generate_quote()).collect()
    }
}

impl Default for SimulatedFeed {
    fn default() -> Self {
        Self::new()
    }
}

impl MarketFeed for SimulatedFeed {
    fn subscribe(&mut self, symbols: &[Symbol]) -> Result<(), FeedError> {
        if self.status == FeedStatus::Error {
            return Err(FeedError::ConnectionFailed(
                "feed in error state".to_string(),
            ));
        }

        let mut prices = self.prices.lock().expect("prices lock poisoned");
        let mut rngs = self.rngs.lock().expect("rngs lock poisoned");

        for sym in symbols {
            if !self.symbols.iter().any(|s| s.0 == sym.0) {
                self.symbols.push(sym.clone());
                prices.entry(sym.0.clone()).or_insert(100.0);
                // Add a new RNG for this symbol.
                let seed = self.gbm.seed.wrapping_add(rngs.len() as u64);
                rngs.push(BoxMuller::new(seed));
            }
        }

        self.status = FeedStatus::Connected;
        Ok(())
    }

    fn status(&self) -> FeedStatus {
        self.status
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subscribe_to_symbols() {
        let mut feed = SimulatedFeed::new();
        assert_eq!(feed.status(), FeedStatus::Disconnected);

        feed.subscribe(&[Symbol("AAPL".to_string())]).unwrap();
        assert_eq!(feed.status(), FeedStatus::Connected);

        feed.subscribe(&[Symbol("GOOG".to_string())]).unwrap();
    }

    #[test]
    fn test_status_tracking() {
        let mut feed = SimulatedFeed::new();
        assert_eq!(feed.status(), FeedStatus::Disconnected);

        feed.set_status(FeedStatus::Error);
        assert_eq!(feed.status(), FeedStatus::Error);

        feed.set_status(FeedStatus::Connected);
        assert_eq!(feed.status(), FeedStatus::Connected);
    }

    #[test]
    fn test_quote_generation() {
        let feed =
            SimulatedFeed::with_symbols(&["AAPL", "GOOG"], &[("AAPL", 150.0), ("GOOG", 2800.0)]);

        let update = feed.generate_quote();
        match update {
            MarketUpdate::Quote(q) => {
                assert!(q.bid > 0.0);
                assert!(q.ask > 0.0);
                assert!(q.ask > q.bid);
                assert!(q.timestamp > 0);
            },
            MarketUpdate::Heartbeat => panic!("expected a quote"),
        }
    }

    #[test]
    fn test_subscribe_from_error_state_fails() {
        let mut feed = SimulatedFeed::new();
        feed.set_status(FeedStatus::Error);
        let result = feed.subscribe(&[Symbol("AAPL".to_string())]);
        assert!(result.is_err());
    }

    #[test]
    fn test_gbm_prices_vary() {
        // Generate 100 quotes and verify prices are not constant.
        let feed = SimulatedFeed::with_symbols(&["AAPL"], &[("AAPL", 100.0)]);
        let mut prices = Vec::new();

        for _ in 0..100 {
            if let MarketUpdate::Quote(q) = feed.generate_quote() {
                prices.push((q.bid + q.ask) / 2.0);
            }
        }

        let min = prices.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = prices.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

        // With σ=0.20 and 100 steps, prices should vary by at least 0.1%.
        assert!(
            (max - min) / min > 0.001,
            "Prices barely varied: min={min:.4}, max={max:.4}, range={:.6}",
            (max - min) / min
        );
    }

    #[test]
    fn test_gbm_prices_remain_positive() {
        // Even with high volatility, prices must never go below zero.
        let feed = SimulatedFeed::with_gbm_config(
            &["VOL"],
            &[("VOL", 100.0)],
            GbmConfig {
                drift: -0.5,     // Strong negative drift.
                volatility: 1.0, // 100% volatility — extreme.
                dt: 1.0 / 252.0,
                spread_bps: 5.0,
                seed: 42,
            },
        );

        for _ in 0..500 {
            if let MarketUpdate::Quote(q) = feed.generate_quote() {
                assert!(q.bid > 0.0, "bid went negative: {}", q.bid);
                assert!(q.ask > 0.0, "ask went negative: {}", q.ask);
            }
        }
    }

    #[test]
    fn test_gbm_deterministic_with_seed() {
        // Same seed should produce identical price sequences.
        let feed1 = SimulatedFeed::with_gbm_config(
            &["AAPL"],
            &[("AAPL", 150.0)],
            GbmConfig {
                seed: 999,
                ..GbmConfig::default()
            },
        );
        let feed2 = SimulatedFeed::with_gbm_config(
            &["AAPL"],
            &[("AAPL", 150.0)],
            GbmConfig {
                seed: 999,
                ..GbmConfig::default()
            },
        );

        for _ in 0..50 {
            let q1 = feed1.generate_quote();
            let q2 = feed2.generate_quote();
            match (q1, q2) {
                (MarketUpdate::Quote(a), MarketUpdate::Quote(b)) => {
                    assert_eq!(a.bid, b.bid, "Deterministic reproduction failed");
                    assert_eq!(a.ask, b.ask, "Deterministic reproduction failed");
                },
                _ => panic!("expected quotes"),
            }
        }
    }

    #[test]
    fn test_gbm_spread_from_config() {
        // Verify spread matches configured basis points.
        let feed = SimulatedFeed::with_gbm_config(
            &["TSLA"],
            &[("TSLA", 200.0)],
            GbmConfig {
                spread_bps: 20.0, // 0.20% = 20 bps
                ..GbmConfig::default()
            },
        );

        // First quote: price hasn't moved yet from GBM (or moved minimally).
        if let MarketUpdate::Quote(q) = feed.generate_quote() {
            let mid = (q.bid + q.ask) / 2.0;
            let half_spread = mid * 20.0 / 10_000.0 / 2.0;
            // Allow some tolerance for the GBM step that just happened.
            let expected_bid = mid - half_spread;
            let expected_ask = mid + half_spread;
            assert!(
                (q.bid - expected_bid).abs() < 0.01,
                "bid spread mismatch: got {}, expected ~{}",
                q.bid,
                expected_bid
            );
            assert!(
                (q.ask - expected_ask).abs() < 0.01,
                "ask spread mismatch: got {}, expected ~{}",
                q.ask,
                expected_ask
            );
        }
    }

    #[test]
    fn test_round_robin_symbol_selection() {
        // With 3 symbols, round-robin should cycle through all of them.
        let feed = SimulatedFeed::with_symbols(
            &["A", "B", "C"],
            &[("A", 100.0), ("B", 200.0), ("C", 300.0)],
        );

        let mut symbol_counts: HashMap<String, usize> = HashMap::new();
        for _ in 0..30 {
            if let MarketUpdate::Quote(q) = feed.generate_quote() {
                *symbol_counts.entry(q.symbol.0.clone()).or_insert(0) += 1;
            }
        }

        // Each symbol should appear roughly 10 times (30 / 3).
        for (sym, count) in &symbol_counts {
            assert!(
                *count >= 8 && *count <= 12,
                "Symbol {sym} appeared {count} times, expected ~10"
            );
        }
    }

    #[test]
    fn test_current_price_tracking() {
        let feed = SimulatedFeed::with_symbols(&["AAPL"], &[("AAPL", 100.0)]);

        // Generate some quotes to advance the price.
        for _ in 0..10 {
            feed.generate_quote();
        }

        let price = feed.current_price("AAPL");
        assert!(price.is_some());
        let p = price.expect("price should exist");
        // Price should have drifted from 100.0.
        assert_ne!(p, 100.0, "Price should have evolved from initial value");
    }

    #[test]
    fn test_generate_n_quotes() {
        let feed = SimulatedFeed::with_symbols(&["AAPL"], &[("AAPL", 50.0)]);
        let updates = feed.generate_n_quotes(20);

        assert_eq!(updates.len(), 20);
        for update in &updates {
            assert!(matches!(update, MarketUpdate::Quote(_)));
        }
    }

    #[test]
    fn test_empty_feed_heartbeat() {
        let feed = SimulatedFeed::new();
        let update = feed.generate_quote();
        assert!(matches!(update, MarketUpdate::Heartbeat));
    }

    #[test]
    fn test_xoshiro256_period() {
        // Verify the PRNG produces different values.
        let mut rng1 = Xoshiro256::new(42);
        let mut rng2 = Xoshiro256::new(42);

        let v1 = rng1.next_u64();
        let v2 = rng2.next_u64();
        assert_eq!(v1, v2, "Same seed should produce same first value");

        let v3 = rng1.next_u64();
        assert_ne!(v1, v3, "Consecutive values should differ");
    }

    #[test]
    fn test_box_muller_mean_and_variance() {
        // Generate 10,000 samples and check mean ≈ 0, variance ≈ 1.
        let mut bm = BoxMuller::new(42);
        let n = 10_000;
        let mut sum = 0.0;
        let mut sum_sq = 0.0;

        for _ in 0..n {
            let z = bm.next_normal();
            sum += z;
            sum_sq += z * z;
        }

        let mean = sum / n as f64;
        let variance = sum_sq / n as f64 - mean * mean;

        assert!(mean.abs() < 0.1, "Mean should be near 0, got {mean}");
        assert!(
            (variance - 1.0).abs() < 0.1,
            "Variance should be near 1, got {variance}"
        );
    }

    #[test]
    fn test_all_prices_accessor() {
        let feed =
            SimulatedFeed::with_symbols(&["AAPL", "GOOG"], &[("AAPL", 150.0), ("GOOG", 2800.0)]);

        feed.generate_quote();
        feed.generate_quote();

        let prices = feed.all_prices();
        assert!(prices.contains_key("AAPL"));
        assert!(prices.contains_key("GOOG"));
    }
}
