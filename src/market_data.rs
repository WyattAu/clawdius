//! Market Data Types for HFT Broker
//!
//! Per BP-HFT-BROKER-001:
//! - PriceLevel, Trade, Quote types
//! - OrderBook representation
//! - WebSocket ingestion interface (stub)

use std::time::{Duration, Instant};

pub type Symbol = u32;
pub type Price = i64;
pub type Quantity = u64;
pub type Timestamp = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Bid,
    Ask,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PriceLevel {
    pub price: Price,
    pub quantity: Quantity,
    pub order_count: u32,
}

impl PriceLevel {
    pub fn new(price: Price, quantity: Quantity, order_count: u32) -> Self {
        Self {
            price,
            quantity,
            order_count,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Quote {
    pub symbol: Symbol,
    pub bid: Price,
    pub ask: Price,
    pub bid_size: Quantity,
    pub ask_size: Quantity,
    pub timestamp: Timestamp,
}

impl Quote {
    pub fn new(
        symbol: Symbol,
        bid: Price,
        ask: Price,
        bid_size: Quantity,
        ask_size: Quantity,
        timestamp: Timestamp,
    ) -> Self {
        Self {
            symbol,
            bid,
            ask,
            bid_size,
            ask_size,
            timestamp,
        }
    }

    pub fn spread(&self) -> Price {
        self.ask - self.bid
    }

    pub fn mid_price(&self) -> Price {
        (self.bid + self.ask) / 2
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Trade {
    pub symbol: Symbol,
    pub price: Price,
    pub quantity: Quantity,
    pub side: Side,
    pub timestamp: Timestamp,
    pub trade_id: u64,
}

impl Trade {
    pub fn new(
        symbol: Symbol,
        price: Price,
        quantity: Quantity,
        side: Side,
        timestamp: Timestamp,
        trade_id: u64,
    ) -> Self {
        Self {
            symbol,
            price,
            quantity,
            side,
            timestamp,
            trade_id,
        }
    }
}

pub const MAX_BOOK_DEPTH: usize = 10;

#[derive(Debug, Clone)]
pub struct OrderBook {
    pub symbol: Symbol,
    pub bids: [PriceLevel; MAX_BOOK_DEPTH],
    pub asks: [PriceLevel; MAX_BOOK_DEPTH],
    pub bid_count: usize,
    pub ask_count: usize,
    pub timestamp: Timestamp,
    pub sequence: u64,
}

impl OrderBook {
    pub fn new(symbol: Symbol, timestamp: Timestamp, sequence: u64) -> Self {
        Self {
            symbol,
            bids: [PriceLevel::default(); MAX_BOOK_DEPTH],
            asks: [PriceLevel::default(); MAX_BOOK_DEPTH],
            bid_count: 0,
            ask_count: 0,
            timestamp,
            sequence,
        }
    }

    pub fn best_bid(&self) -> Option<&PriceLevel> {
        if self.bid_count > 0 {
            Some(&self.bids[0])
        } else {
            None
        }
    }

    pub fn best_ask(&self) -> Option<&PriceLevel> {
        if self.ask_count > 0 {
            Some(&self.asks[0])
        } else {
            None
        }
    }

    pub fn spread(&self) -> Option<Price> {
        match (self.best_bid(), self.best_ask()) {
            (Some(bid), Some(ask)) => Some(ask.price - bid.price),
            _ => None,
        }
    }

    pub fn mid_price(&self) -> Option<Price> {
        match (self.best_bid(), self.best_ask()) {
            (Some(bid), Some(ask)) => Some((bid.price + ask.price) / 2),
            _ => None,
        }
    }

    pub fn update_bid(&mut self, level: usize, price_level: PriceLevel) {
        if level < MAX_BOOK_DEPTH {
            self.bids[level] = price_level;
            if level >= self.bid_count {
                self.bid_count = level + 1;
            }
        }
    }

    pub fn update_ask(&mut self, level: usize, price_level: PriceLevel) {
        if level < MAX_BOOK_DEPTH {
            self.asks[level] = price_level;
            if level >= self.ask_count {
                self.ask_count = level + 1;
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum MarketDataType {
    Quote,
    Trade,
    BookUpdate,
}

#[derive(Debug, Clone)]
pub struct MarketData {
    pub data_type: MarketDataType,
    pub symbol: Symbol,
    pub quote: Option<Quote>,
    pub trade: Option<Trade>,
    pub book: Option<OrderBook>,
    pub received_at: Instant,
}

impl MarketData {
    pub fn from_quote(quote: Quote) -> Self {
        Self {
            data_type: MarketDataType::Quote,
            symbol: quote.symbol,
            quote: Some(quote),
            trade: None,
            book: None,
            received_at: Instant::now(),
        }
    }

    pub fn from_trade(trade: Trade) -> Self {
        Self {
            data_type: MarketDataType::Trade,
            symbol: trade.symbol,
            quote: None,
            trade: Some(trade),
            book: None,
            received_at: Instant::now(),
        }
    }

    pub fn from_book(book: OrderBook) -> Self {
        Self {
            data_type: MarketDataType::BookUpdate,
            symbol: book.symbol,
            quote: None,
            trade: None,
            book: Some(book),
            received_at: Instant::now(),
        }
    }

    pub fn age(&self) -> Duration {
        self.received_at.elapsed()
    }
}

#[derive(Debug, Clone)]
pub struct WebSocketConfig {
    pub url: String,
    pub reconnect_interval_ms: u64,
    pub ping_interval_ms: u64,
    pub max_message_size: usize,
}

impl Default for WebSocketConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            reconnect_interval_ms: 5000,
            ping_interval_ms: 30000,
            max_message_size: 1024 * 1024,
        }
    }
}

#[derive(Debug)]
pub struct MarketDataIngestor {
    config: WebSocketConfig,
    connected: bool,
    last_sequence: u64,
}

impl MarketDataIngestor {
    pub fn new(config: WebSocketConfig) -> Self {
        Self {
            config,
            connected: false,
            last_sequence: 0,
        }
    }

    pub fn is_connected(&self) -> bool {
        self.connected
    }

    pub fn connect(&mut self) -> crate::error::Result<()> {
        self.connected = true;
        tracing::info!(url = %self.config.url, "Market data connection established");
        Ok(())
    }

    pub fn disconnect(&mut self) {
        self.connected = false;
        tracing::info!("Market data connection closed");
    }

    pub fn validate_sequence(&mut self, sequence: u64) -> bool {
        if sequence == 0 || sequence == self.last_sequence + 1 {
            self.last_sequence = sequence;
            true
        } else {
            tracing::warn!(
                expected = self.last_sequence + 1,
                actual = sequence,
                "Sequence gap detected"
            );
            self.last_sequence = sequence;
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quote_spread() {
        let quote = Quote::new(1, 10000, 10005, 100, 200, 12345);
        assert_eq!(quote.spread(), 5);
        assert_eq!(quote.mid_price(), 10002);
    }

    #[test]
    fn test_order_book_best_prices() {
        let mut book = OrderBook::new(1, 12345, 1);

        book.update_bid(0, PriceLevel::new(9999, 100, 5));
        book.update_ask(0, PriceLevel::new(10001, 200, 3));

        assert_eq!(book.best_bid().map(|b| b.price), Some(9999));
        assert_eq!(book.best_ask().map(|a| a.price), Some(10001));
        assert_eq!(book.spread(), Some(2));
        assert_eq!(book.mid_price(), Some(10000));
    }

    #[test]
    fn test_market_data_age() {
        let quote = Quote::new(1, 10000, 10005, 100, 200, 12345);
        let md = MarketData::from_quote(quote);
        std::thread::sleep(Duration::from_millis(10));
        assert!(md.age() >= Duration::from_millis(10));
    }

    #[test]
    fn test_sequence_validation() {
        let mut ingestor = MarketDataIngestor::new(WebSocketConfig::default());

        assert!(ingestor.validate_sequence(0));
        assert!(ingestor.validate_sequence(1));
        assert!(ingestor.validate_sequence(2));
        assert!(!ingestor.validate_sequence(5));
        assert!(ingestor.validate_sequence(6));
    }
}
