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

/// Simulated market data feed for testing.
/// Generates synthetic quotes at configurable intervals.
pub struct SimulatedFeed {
    status: FeedStatus,
    symbols: Vec<Symbol>,
    base_prices: std::collections::HashMap<String, f64>,
}

impl SimulatedFeed {
    pub fn new() -> Self {
        Self {
            status: FeedStatus::Disconnected,
            symbols: Vec::new(),
            base_prices: std::collections::HashMap::new(),
        }
    }

    pub fn with_symbols(symbols: &[&str], base_prices: &[(&str, f64)]) -> Self {
        let mut feed = Self::new();
        for (sym, price) in base_prices {
            feed.base_prices.insert(sym.to_string(), *price);
        }
        for sym in symbols {
            feed.symbols.push(Symbol(sym.to_string()));
            feed.base_prices.entry(sym.to_string()).or_insert(100.0);
        }
        feed.status = FeedStatus::Connected;
        feed
    }

    pub fn set_status(&mut self, status: FeedStatus) {
        self.status = status;
    }

    pub fn generate_quote(&self) -> MarketUpdate {
        if self.symbols.is_empty() {
            return MarketUpdate::Heartbeat;
        }

        let idx = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as usize)
            % self.symbols.len();

        let symbol = &self.symbols[idx];
        let base = self.base_prices.get(&symbol.0).copied().unwrap_or(100.0);
        let spread = base * 0.001;
        let bid = base - spread / 2.0;
        let ask = base + spread / 2.0;
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
        for sym in symbols {
            if !self.symbols.iter().any(|s| s.0 == sym.0) {
                self.symbols.push(sym.clone());
                self.base_prices.entry(sym.0.clone()).or_insert(100.0);
            }
        }
        self.status = FeedStatus::Connected;
        Ok(())
    }

    fn status(&self) -> FeedStatus {
        self.status
    }
}

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
}
