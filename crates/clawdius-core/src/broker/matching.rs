//! Order Matching Engine
//!
//! Implements a price-time priority order book with support for:
//! - Limit orders and market orders
//! - Partial fills
//! - Bid/ask spread tracking
//! - Configurable simulated exchange latency
//!
//! Per YP-HFT-BROKER-001:
//! - Orders matched at best available price (price improvement)
//! - Time priority within same price level
//! - Partial fills when counterparty quantity is insufficient

use super::wallet_guard::{Order, OrderSide};
use std::cmp::Reverse;
use std::collections::{BTreeMap, HashMap, VecDeque};

use std::sync::Mutex;
use std::time::SystemTime;

// ─── Book Order ────────────────────────────────────────────────────────────

/// An order resting in the book.
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct BookOrder {
    /// Order ID (auto-incremented).
    id: u64,
    /// Symbol (u32 for O(1) lookup).
    symbol: u32,
    /// Side.
    side: OrderSide,
    /// Original quantity.
    original_qty: u64,
    /// Remaining (unfilled) quantity.
    remaining_qty: u64,
    /// Limit price.
    price: u64,
    /// Submission timestamp.
    timestamp: u64,
}

// ─── Price Level ──────────────────────────────────────────────────────────

/// A price level in the order book.
#[derive(Debug, Clone, Default)]
struct PriceLevel {
    /// Orders at this price, in time priority (FIFO).
    orders: VecDeque<BookOrder>,
}

impl PriceLevel {
    fn total_quantity(&self) -> u64 {
        self.orders.iter().map(|o| o.remaining_qty).sum()
    }

    fn is_empty(&self) -> bool {
        self.orders.is_empty()
    }

    fn remove_filled(&mut self) {
        self.orders.retain(|o| o.remaining_qty > 0);
    }
}

// ─── Order Types ──────────────────────────────────────────────────────────

/// Order type: limit or market.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderType {
    /// Limit order: execute at or better than specified price.
    Limit,
    /// Market order: execute immediately at best available price.
    Market,
}

// ─── Match Result ─────────────────────────────────────────────────────────

/// Result of matching an incoming order against the book.
#[derive(Debug, Clone)]
pub struct MatchResult {
    /// Individual fills produced.
    pub fills: Vec<MatchFill>,
    /// Any remaining quantity that was not filled (placed in book).
    pub remaining_qty: u64,
}

/// A single fill from matching.
#[derive(Debug, Clone)]
pub struct MatchFill {
    /// Aggressor order ID.
    pub aggressor_id: u64,
    /// Passive (resting) order ID that was matched.
    pub passive_id: u64,
    /// Symbol.
    pub symbol: u32,
    /// Side of the passive order (opposite of aggressor).
    pub passive_side: OrderSide,
    /// Fill price (always the passive order's price — price improvement).
    pub price: u64,
    /// Fill quantity.
    pub quantity: u64,
    /// Timestamp of the fill.
    pub timestamp: u64,
}

// ─── Order Book ───────────────────────────────────────────────────────────

/// Order book for a single symbol.
///
/// Maintains bid (buy) and ask (sell) sides with price-time priority.
/// Bids are sorted descending (highest first), asks ascending (lowest first).
pub struct OrderBook {
    symbol: u32,
    /// Bid side: price → price level (sorted descending via Reverse).
    bids: BTreeMap<std::cmp::Reverse<u64>, PriceLevel>,
    /// Ask side: price → price level (sorted ascending).
    asks: BTreeMap<u64, PriceLevel>,
    /// Next order ID.
    next_id: u64,
}

impl OrderBook {
    /// Create a new order book for a symbol.
    pub fn new(symbol: u32) -> Self {
        Self {
            symbol,
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            next_id: 1,
        }
    }

    fn alloc_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        id
    }

    fn now_nanos() -> u64 {
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64
    }

    /// Best bid price (highest). Returns `None` if no bids.
    pub fn best_bid(&self) -> Option<u64> {
        self.bids
            .iter()
            .find(|(_, level)| !level.is_empty())
            .map(|(Reverse(price), _)| *price)
    }

    /// Best ask price (lowest). Returns `None` if no asks.
    pub fn best_ask(&self) -> Option<u64> {
        self.asks
            .iter()
            .find(|(_, level)| !level.is_empty())
            .map(|(price, _)| *price)
    }

    /// Mid price = (best_bid + best_ask) / 2.
    pub fn mid_price(&self) -> Option<u64> {
        let bid = self.best_bid()?;
        let ask = self.best_ask()?;
        Some(bid / 2 + ask / 2)
    }

    /// Spread = best_ask - best_bid.
    pub fn spread(&self) -> Option<u64> {
        let bid = self.best_bid()?;
        let ask = self.best_ask()?;
        Some(ask.saturating_sub(bid))
    }

    /// Total quantity on the bid side.
    pub fn total_bid_qty(&self) -> u64 {
        self.bids.values().map(|l| l.total_quantity()).sum()
    }

    /// Total quantity on the ask side.
    pub fn total_ask_qty(&self) -> u64 {
        self.asks.values().map(|l| l.total_quantity()).sum()
    }

    /// Number of price levels on the bid side.
    pub fn bid_depth(&self) -> usize {
        self.bids.iter().filter(|(_, l)| !l.is_empty()).count()
    }

    /// Number of price levels on the ask side.
    pub fn ask_depth(&self) -> usize {
        self.asks.iter().filter(|(_, l)| !l.is_empty()).count()
    }

    /// Submit an incoming order for matching.
    ///
    /// For a buy aggressor, matches against asks (lowest first).
    /// For a sell aggressor, matches against bids (highest first).
    /// Any unfilled remainder is added to the book (for limit orders only).
    pub fn submit(
        &mut self,
        order_type: OrderType,
        side: OrderSide,
        quantity: u64,
        price: u64,
    ) -> MatchResult {
        let id = self.alloc_id();
        let timestamp = Self::now_nanos();

        let mut fills = Vec::new();
        let mut remaining = quantity;

        match side {
            OrderSide::Buy => {
                remaining = self.match_buy(id, remaining, price, order_type, &mut fills, timestamp);
            },
            OrderSide::Sell => {
                remaining =
                    self.match_sell(id, remaining, price, order_type, &mut fills, timestamp);
            },
        }

        // Place remainder in book (limit orders only).
        if remaining > 0 && order_type == OrderType::Limit {
            self.add_to_book(BookOrder {
                id,
                symbol: self.symbol,
                side,
                original_qty: quantity,
                remaining_qty: remaining,
                price,
                timestamp,
            });
        }

        MatchResult {
            fills,
            remaining_qty: remaining,
        }
    }

    /// Match a buy order against the ask side.
    fn match_buy(
        &mut self,
        aggressor_id: u64,
        mut remaining: u64,
        limit_price: u64,
        order_type: OrderType,
        fills: &mut Vec<MatchFill>,
        timestamp: u64,
    ) -> u64 {
        let ask_prices: Vec<u64> = self.asks.keys().copied().collect();

        for ask_price in ask_prices {
            if remaining == 0 {
                break;
            }
            if order_type == OrderType::Limit && ask_price > limit_price {
                break;
            }

            let level = match self.asks.get_mut(&ask_price) {
                Some(l) if !l.is_empty() => l,
                _ => continue,
            };

            while remaining > 0 {
                let passive = match level.orders.front_mut() {
                    Some(o) if o.remaining_qty > 0 => o,
                    Some(_) => {
                        // Exhausted order at front — pop and try next.
                        level.orders.pop_front();
                        continue;
                    },
                    None => break,
                };

                let fill_qty = remaining.min(passive.remaining_qty);
                remaining = remaining.saturating_sub(fill_qty);
                passive.remaining_qty = passive.remaining_qty.saturating_sub(fill_qty);

                fills.push(MatchFill {
                    aggressor_id,
                    passive_id: passive.id,
                    symbol: self.symbol,
                    passive_side: OrderSide::Sell,
                    price: ask_price,
                    quantity: fill_qty,
                    timestamp,
                });
            }

            level.remove_filled();
            if level.is_empty() {
                self.asks.remove(&ask_price);
            }
        }

        remaining
    }

    /// Match a sell order against the bid side.
    fn match_sell(
        &mut self,
        aggressor_id: u64,
        mut remaining: u64,
        limit_price: u64,
        order_type: OrderType,
        fills: &mut Vec<MatchFill>,
        timestamp: u64,
    ) -> u64 {
        let bid_prices: Vec<u64> = self.bids.keys().map(|Reverse(p)| *p).collect();

        for bid_price in bid_prices {
            if remaining == 0 {
                break;
            }
            if order_type == OrderType::Limit && bid_price < limit_price {
                break;
            }

            let key = std::cmp::Reverse(bid_price);
            let level = match self.bids.get_mut(&key) {
                Some(l) if !l.is_empty() => l,
                _ => continue,
            };

            while remaining > 0 {
                let passive = match level.orders.front_mut() {
                    Some(o) if o.remaining_qty > 0 => o,
                    Some(_) => {
                        // Exhausted order at front — pop and try next.
                        level.orders.pop_front();
                        continue;
                    },
                    None => break,
                };

                let fill_qty = remaining.min(passive.remaining_qty);
                remaining = remaining.saturating_sub(fill_qty);
                passive.remaining_qty = passive.remaining_qty.saturating_sub(fill_qty);

                fills.push(MatchFill {
                    aggressor_id,
                    passive_id: passive.id,
                    symbol: self.symbol,
                    passive_side: OrderSide::Buy,
                    price: bid_price,
                    quantity: fill_qty,
                    timestamp,
                });
            }

            level.remove_filled();
            if level.is_empty() {
                self.bids.remove(&key);
            }
        }

        remaining
    }

    /// Add a resting order to the book.
    fn add_to_book(&mut self, order: BookOrder) {
        match order.side {
            OrderSide::Buy => {
                self.bids
                    .entry(std::cmp::Reverse(order.price))
                    .or_default()
                    .orders
                    .push_back(order);
            },
            OrderSide::Sell => {
                self.asks
                    .entry(order.price)
                    .or_default()
                    .orders
                    .push_back(order);
            },
        }
    }

    /// Cancel an order by ID. Returns the cancelled quantity.
    pub fn cancel_order(&mut self, order_id: u64) -> Option<u64> {
        for (_, level) in self.bids.iter_mut() {
            for order in level.orders.iter_mut() {
                if order.id == order_id {
                    let qty = order.remaining_qty;
                    order.remaining_qty = 0;
                    return Some(qty);
                }
            }
        }
        for (_, level) in self.asks.iter_mut() {
            for order in level.orders.iter_mut() {
                if order.id == order_id {
                    let qty = order.remaining_qty;
                    order.remaining_qty = 0;
                    return Some(qty);
                }
            }
        }
        None
    }

    /// Seed the book with liquidity (for testing/simulation).
    pub fn add_liquidity(&mut self, side: OrderSide, price: u64, quantity: u64) {
        let id = self.alloc_id();
        let timestamp = Self::now_nanos();
        self.add_to_book(BookOrder {
            id,
            symbol: self.symbol,
            side,
            original_qty: quantity,
            remaining_qty: quantity,
            price,
            timestamp,
        });
    }

    /// Clear all orders from the book.
    pub fn clear(&mut self) {
        self.bids.clear();
        self.asks.clear();
    }
}

// ─── Matching Execution ───────────────────────────────────────────────────

/// Configuration for the simulated execution engine.
#[derive(Debug, Clone)]
pub struct ExecutionConfig {
    /// Simulated exchange latency in microseconds. Default: 0.
    pub latency_us: u64,
    /// Slippage ticks (added to fill price). Default: 0.
    pub slippage_ticks: i64,
    /// If true, reject all orders. Default: false.
    pub reject_all: bool,
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            latency_us: 0,
            slippage_ticks: 0,
            reject_all: false,
        }
    }
}

/// Simulated execution engine backed by an order book matching engine.
///
/// Supports:
/// - Price-time priority matching
/// - Partial fills
/// - Market and limit orders
/// - Configurable latency and slippage
/// - Multi-symbol order books
pub struct MatchingExecution {
    /// Per-symbol order books.
    books: Mutex<HashMap<u32, OrderBook>>,
    /// Execution configuration.
    config: Mutex<ExecutionConfig>,
}

impl MatchingExecution {
    /// Create a new matching execution engine.
    pub fn new() -> Self {
        Self {
            books: Mutex::new(HashMap::new()),
            config: Mutex::new(ExecutionConfig::default()),
        }
    }

    /// Submit a market order for immediate execution.
    ///
    /// Returns fills at best available prices. Any unfilled quantity is lost
    /// (market orders are not placed in the book).
    pub fn submit_market_order(
        &self,
        order: &Order,
    ) -> Result<Vec<super::execution::Fill>, super::execution::ExecutionError> {
        self.check_reject()?;

        let slippage = self
            .config
            .lock()
            .expect("config lock poisoned")
            .slippage_ticks;

        let mut books = self.books.lock().expect("books lock poisoned");
        let book = books
            .entry(order.symbol)
            .or_insert_with(|| OrderBook::new(order.symbol));

        let result = book.submit(OrderType::Market, order.side, order.quantity, order.price);

        Ok(result
            .fills
            .into_iter()
            .map(|mf| self.to_fill(mf, order.side, slippage))
            .collect())
    }

    /// Submit a limit order. Returns fills and remaining unfilled quantity.
    pub fn submit_limit_order(
        &self,
        order: &Order,
    ) -> Result<(Vec<super::execution::Fill>, u64), super::execution::ExecutionError> {
        self.check_reject()?;

        let slippage = self
            .config
            .lock()
            .expect("config lock poisoned")
            .slippage_ticks;

        let mut books = self.books.lock().expect("books lock poisoned");
        let book = books
            .entry(order.symbol)
            .or_insert_with(|| OrderBook::new(order.symbol));

        let result = book.submit(OrderType::Limit, order.side, order.quantity, order.price);

        let fills: Vec<super::execution::Fill> = result
            .fills
            .into_iter()
            .map(|mf| self.to_fill(mf, order.side, slippage))
            .collect();

        Ok((fills, result.remaining_qty))
    }

    /// Add liquidity to the book (seed orders for testing).
    pub fn add_liquidity(&self, symbol: u32, side: OrderSide, price: u64, quantity: u64) {
        let mut books = self.books.lock().expect("books lock poisoned");
        let book = books
            .entry(symbol)
            .or_insert_with(|| OrderBook::new(symbol));
        book.add_liquidity(side, price, quantity);
    }

    /// Get the best bid/ask for a symbol.
    pub fn best_bid_ask(&self, symbol: u32) -> Option<(u64, u64)> {
        let books = self.books.lock().expect("books lock poisoned");
        let book = books.get(&symbol)?;
        Some((book.best_bid()?, book.best_ask()?))
    }

    /// Get the spread for a symbol.
    pub fn spread(&self, symbol: u32) -> Option<u64> {
        let books = self.books.lock().expect("books lock poisoned");
        books.get(&symbol)?.spread()
    }

    /// Set execution configuration.
    pub fn set_config(&self, config: ExecutionConfig) {
        *self.config.lock().expect("config lock poisoned") = config;
    }

    /// Set slippage ticks.
    pub fn set_slippage_ticks(&self, ticks: i64) {
        self.config
            .lock()
            .expect("config lock poisoned")
            .slippage_ticks = ticks;
    }

    /// Set reject-all mode.
    pub fn set_reject_all(&self, reject: bool) {
        self.config.lock().expect("config lock poisoned").reject_all = reject;
    }

    /// Get a read-only snapshot of the order book.
    pub fn book_info(&self, symbol: u32) -> Option<BookInfo> {
        let books = self.books.lock().expect("books lock poisoned");
        let book = books.get(&symbol)?;
        Some(BookInfo {
            symbol,
            best_bid: book.best_bid(),
            best_ask: book.best_ask(),
            mid_price: book.mid_price(),
            spread: book.spread(),
            total_bid_qty: book.total_bid_qty(),
            total_ask_qty: book.total_ask_qty(),
            bid_depth: book.bid_depth(),
            ask_depth: book.ask_depth(),
        })
    }

    /// Clear a symbol's order book.
    pub fn clear_book(&self, symbol: u32) {
        let mut books = self.books.lock().expect("books lock poisoned");
        if let Some(book) = books.get_mut(&symbol) {
            book.clear();
        }
    }

    fn to_fill(
        &self,
        mf: MatchFill,
        aggressor_side: OrderSide,
        slippage: i64,
    ) -> super::execution::Fill {
        let fill_price = if slippage >= 0 {
            mf.price.saturating_add(slippage as u64)
        } else {
            mf.price.saturating_sub((-slippage) as u64)
        };
        super::execution::Fill {
            symbol: mf.symbol,
            side: aggressor_side,
            quantity: mf.quantity,
            price: fill_price,
            timestamp: mf.timestamp,
        }
    }

    fn check_reject(&self) -> Result<(), super::execution::ExecutionError> {
        if self.config.lock().expect("config lock poisoned").reject_all {
            return Err(super::execution::ExecutionError::Rejected(
                "reject_all mode enabled".to_string(),
            ));
        }
        Ok(())
    }
}

impl Default for MatchingExecution {
    fn default() -> Self {
        Self::new()
    }
}

/// Read-only snapshot of an order book.
#[derive(Debug, Clone)]
pub struct BookInfo {
    pub symbol: u32,
    pub best_bid: Option<u64>,
    pub best_ask: Option<u64>,
    pub mid_price: Option<u64>,
    pub spread: Option<u64>,
    pub total_bid_qty: u64,
    pub total_ask_qty: u64,
    pub bid_depth: usize,
    pub ask_depth: usize,
}

// ─── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_order_book_best_bid_ask() {
        let mut book = OrderBook::new(1);
        assert!(book.best_bid().is_none());
        assert!(book.best_ask().is_none());

        book.add_liquidity(OrderSide::Buy, 100, 50);
        book.add_liquidity(OrderSide::Sell, 105, 30);

        assert_eq!(book.best_bid(), Some(100));
        assert_eq!(book.best_ask(), Some(105));
        assert_eq!(book.spread(), Some(5));
    }

    #[test]
    fn test_limit_buy_fills_against_asks() {
        let mut book = OrderBook::new(1);
        book.add_liquidity(OrderSide::Sell, 100, 50);
        book.add_liquidity(OrderSide::Sell, 101, 30);

        let result = book.submit(OrderType::Limit, OrderSide::Buy, 60, 101);
        assert_eq!(result.fills.len(), 2);
        assert_eq!(result.fills[0].quantity, 50);
        assert_eq!(result.fills[0].price, 100);
        assert_eq!(result.fills[1].quantity, 10);
        assert_eq!(result.fills[1].price, 101);
        assert_eq!(result.remaining_qty, 0);
    }

    #[test]
    fn test_limit_buy_partial_fill_rests_in_book() {
        let mut book = OrderBook::new(1);
        book.add_liquidity(OrderSide::Sell, 100, 20);

        let result = book.submit(OrderType::Limit, OrderSide::Buy, 50, 100);
        assert_eq!(result.fills.len(), 1);
        assert_eq!(result.fills[0].quantity, 20);
        assert_eq!(result.remaining_qty, 30);

        assert_eq!(book.best_bid(), Some(100));
        assert_eq!(book.total_bid_qty(), 30);
    }

    #[test]
    fn test_market_buy_no_book_returns_empty() {
        let mut book = OrderBook::new(1);
        let result = book.submit(OrderType::Market, OrderSide::Buy, 100, 0);
        assert!(result.fills.is_empty());
        assert_eq!(result.remaining_qty, 100);
    }

    #[test]
    fn test_price_time_priority() {
        let mut book = OrderBook::new(1);
        book.add_liquidity(OrderSide::Sell, 100, 30);
        book.add_liquidity(OrderSide::Sell, 100, 20);

        let result = book.submit(OrderType::Limit, OrderSide::Buy, 40, 100);
        assert_eq!(result.fills.len(), 2);
        assert_eq!(result.fills[0].quantity, 30);
        assert_eq!(result.fills[1].quantity, 10);
    }

    #[test]
    fn test_limit_buy_no_match_if_price_too_low() {
        let mut book = OrderBook::new(1);
        book.add_liquidity(OrderSide::Sell, 100, 50);

        let result = book.submit(OrderType::Limit, OrderSide::Buy, 30, 99);
        assert!(result.fills.is_empty());
        assert_eq!(result.remaining_qty, 30);
        assert_eq!(book.best_bid(), Some(99));
    }

    #[test]
    fn test_matching_execution_market_order() {
        let exec = MatchingExecution::new();
        exec.add_liquidity(1, OrderSide::Sell, 100, 50);

        let order = Order::new(1, OrderSide::Buy, 30, 0);
        let fills = exec.submit_market_order(&order).expect("fill");

        assert_eq!(fills.len(), 1);
        assert_eq!(fills[0].quantity, 30);
        assert_eq!(fills[0].price, 100);
    }

    #[test]
    fn test_matching_execution_limit_order() {
        let exec = MatchingExecution::new();
        exec.add_liquidity(1, OrderSide::Sell, 100, 20);

        let order = Order::new(1, OrderSide::Buy, 50, 100);
        let (fills, remaining) = exec.submit_limit_order(&order).expect("fill");

        assert_eq!(fills.len(), 1);
        assert_eq!(fills[0].quantity, 20);
        assert_eq!(remaining, 30);

        let info = exec.book_info(1).expect("book");
        assert_eq!(info.best_bid, Some(100));
        assert_eq!(info.total_bid_qty, 30);
    }

    #[test]
    fn test_matching_execution_slippage() {
        let exec = MatchingExecution::new();
        exec.set_slippage_ticks(3);
        exec.add_liquidity(1, OrderSide::Sell, 100, 50);

        let order = Order::new(1, OrderSide::Buy, 20, 0);
        let fills = exec.submit_market_order(&order).expect("fill");

        assert_eq!(fills[0].price, 103);
    }

    #[test]
    fn test_matching_execution_negative_slippage() {
        let exec = MatchingExecution::new();
        exec.set_slippage_ticks(-2);
        exec.add_liquidity(1, OrderSide::Sell, 100, 50);

        let order = Order::new(1, OrderSide::Buy, 20, 0);
        let fills = exec.submit_market_order(&order).expect("fill");

        assert_eq!(fills[0].price, 98);
    }

    #[test]
    fn test_matching_execution_reject_all() {
        let exec = MatchingExecution::new();
        exec.set_reject_all(true);

        let order = Order::new(1, OrderSide::Buy, 20, 0);
        let result = exec.submit_market_order(&order);
        assert!(result.is_err());
    }

    #[test]
    fn test_matching_execution_best_bid_ask() {
        let exec = MatchingExecution::new();
        exec.add_liquidity(1, OrderSide::Buy, 99, 100);
        exec.add_liquidity(1, OrderSide::Sell, 101, 50);

        let bba = exec.best_bid_ask(1).expect("bba");
        assert_eq!(bba, (99, 101));
    }

    #[test]
    fn test_book_depth_tracking() {
        let mut book = OrderBook::new(1);
        book.add_liquidity(OrderSide::Buy, 100, 50);
        book.add_liquidity(OrderSide::Buy, 99, 30);
        book.add_liquidity(OrderSide::Buy, 98, 20);
        book.add_liquidity(OrderSide::Sell, 101, 40);
        book.add_liquidity(OrderSide::Sell, 102, 25);

        assert_eq!(book.bid_depth(), 3);
        assert_eq!(book.ask_depth(), 2);
        assert_eq!(book.total_bid_qty(), 100);
        assert_eq!(book.total_ask_qty(), 65);
    }

    #[test]
    fn test_mid_price() {
        let mut book = OrderBook::new(1);
        book.add_liquidity(OrderSide::Buy, 100, 10);
        book.add_liquidity(OrderSide::Sell, 102, 10);

        assert_eq!(book.mid_price(), Some(101));
    }

    #[test]
    fn test_sell_order_matches_bids() {
        let mut book = OrderBook::new(1);
        book.add_liquidity(OrderSide::Buy, 100, 30);
        book.add_liquidity(OrderSide::Buy, 99, 20);

        let result = book.submit(OrderType::Limit, OrderSide::Sell, 40, 99);
        assert_eq!(result.fills.len(), 2);
        assert_eq!(result.fills[0].quantity, 30);
        assert_eq!(result.fills[0].price, 100);
        assert_eq!(result.fills[1].quantity, 10);
        assert_eq!(result.fills[1].price, 99);
        assert_eq!(result.remaining_qty, 0);
    }

    #[test]
    fn test_cancel_order() {
        let mut book = OrderBook::new(1);
        book.add_liquidity(OrderSide::Buy, 100, 50);

        let cancelled = book.cancel_order(1);
        assert_eq!(cancelled, Some(50));
        assert_eq!(book.total_bid_qty(), 0);
    }

    #[test]
    fn test_clear_book() {
        let mut book = OrderBook::new(1);
        book.add_liquidity(OrderSide::Buy, 100, 50);
        book.add_liquidity(OrderSide::Sell, 105, 30);
        book.clear();
        assert!(book.best_bid().is_none());
        assert!(book.best_ask().is_none());
    }

    #[test]
    fn test_multi_symbol_books() {
        let exec = MatchingExecution::new();
        exec.add_liquidity(1, OrderSide::Sell, 100, 50);
        exec.add_liquidity(2, OrderSide::Sell, 200, 30);

        let fills1 = exec
            .submit_market_order(&Order::new(1, OrderSide::Buy, 20, 0))
            .expect("fill");
        let fills2 = exec
            .submit_market_order(&Order::new(2, OrderSide::Buy, 10, 0))
            .expect("fill");

        assert_eq!(fills1[0].price, 100);
        assert_eq!(fills2[0].price, 200);
    }

    #[test]
    fn test_market_sell_partial_fill() {
        let exec = MatchingExecution::new();
        exec.add_liquidity(1, OrderSide::Buy, 100, 20);

        // Sell 50 but only 20 on bid side.
        let fills = exec
            .submit_market_order(&Order::new(1, OrderSide::Sell, 50, 0))
            .expect("fill");

        assert_eq!(fills.len(), 1);
        assert_eq!(fills[0].quantity, 20);
    }

    #[test]
    fn test_spread_after_partial_execution() {
        let mut book = OrderBook::new(1);
        book.add_liquidity(OrderSide::Sell, 100, 50);
        book.add_liquidity(OrderSide::Sell, 101, 30);

        // Buy 60 — consumes all of 100 level and 10 of 101 level.
        book.submit(OrderType::Limit, OrderSide::Buy, 60, 101);

        assert_eq!(book.best_ask(), Some(101));
        assert_eq!(book.total_ask_qty(), 20); // 30 - 10 filled.
    }

    #[test]
    fn test_execution_config_setters() {
        let exec = MatchingExecution::new();

        exec.set_slippage_ticks(5);
        assert_eq!(exec.config.lock().expect("lock").slippage_ticks, 5);

        exec.set_reject_all(true);
        assert!(exec.config.lock().expect("lock").reject_all);

        exec.set_config(ExecutionConfig {
            latency_us: 100,
            slippage_ticks: 0,
            reject_all: false,
        });
        assert_eq!(exec.config.lock().expect("lock").latency_us, 100);
    }
}
