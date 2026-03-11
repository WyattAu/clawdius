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
