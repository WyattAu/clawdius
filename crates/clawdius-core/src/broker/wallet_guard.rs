//! Wallet Guard - SEC 15c3-5 Pre-trade Risk Controls
//!
//! Per YP-HFT-BROKER-001:
//! - Position limit check (π_max)
//! - Order size check (σ_max)
//! - Daily drawdown check (λ_max)
//! - Margin requirement check
//! - Zero-price and zero-quantity rejection
//! - WCET target: < 100µs
//!
//! This is the canonical implementation. The root binary re-exports from here.

use std::collections::HashMap;

/// Maximum position size per symbol (default).
pub const MAX_POSITION: i64 = 10_000;
/// Maximum order quantity (default).
pub const MAX_ORDER_SIZE: u64 = 1_000;
/// Maximum daily drawdown (default).
pub const MAX_DRAWDOWN: i64 = 10_000_000;
/// Default margin ratio (1:N).
pub const DEFAULT_MARGIN_RATIO: u64 = 4;

/// Outcome of a risk check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskDecision {
    /// Order passes all risk checks.
    Approve,
    /// Order rejected with reason.
    Reject(RejectReason),
}

/// Why an order was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectReason {
    /// Resulting position would exceed π_max.
    PositionLimitExceeded { would_be: i64, max: i64 },
    /// Order quantity exceeds σ_max.
    OrderSizeExceeded { requested: u64, max: u64 },
    /// Daily drawdown exceeds λ_max.
    DailyDrawdownExceeded { current: i64, max: i64 },
    /// Insufficient cash for margin.
    InsufficientMargin { required: u64, available: u64 },
    /// Position arithmetic overflow.
    PositionOverflow,
    /// Price is zero (invalid).
    ZeroPrice,
    /// Quantity is zero (invalid).
    ZeroQuantity,
}

/// Risk parameters controlling the guard's behavior.
#[derive(Debug, Clone)]
pub struct RiskParams {
    /// Maximum absolute position per symbol.
    pub pi_max: i64,
    /// Maximum quantity per order.
    pub sigma_max: u64,
    /// Maximum daily drawdown.
    pub lambda_max: i64,
    /// Margin ratio (notional / ratio = required margin).
    pub margin_ratio: u64,
}

impl Default for RiskParams {
    fn default() -> Self {
        Self {
            pi_max: MAX_POSITION,
            sigma_max: MAX_ORDER_SIZE,
            lambda_max: MAX_DRAWDOWN,
            margin_ratio: DEFAULT_MARGIN_RATIO,
        }
    }
}

/// Order side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderSide {
    Buy,
    Sell,
}

/// An order submitted for risk checking.
#[derive(Debug, Clone)]
pub struct Order {
    /// Symbol identifier (u32 for O(1) HashMap lookup).
    pub symbol: u32,
    /// Buy or sell.
    pub side: OrderSide,
    /// Number of units.
    pub quantity: u64,
    /// Price per unit in base currency.
    pub price: u64,
}

impl Order {
    /// Create a new order.
    pub fn new(symbol: u32, side: OrderSide, quantity: u64, price: u64) -> Self {
        Self {
            symbol,
            side,
            quantity,
            price,
        }
    }

    /// Notional value (price × quantity), saturating on overflow.
    pub fn notional_value(&self) -> Option<u64> {
        self.price.checked_mul(self.quantity)
    }

    /// Signed quantity: positive for buys, negative for sells.
    pub fn signed_quantity(&self) -> i64 {
        match self.side {
            OrderSide::Buy => self.quantity as i64,
            OrderSide::Sell => -(self.quantity as i64),
        }
    }
}

/// Tracks cash, positions, and P&L for risk evaluation.
#[derive(Debug, Clone, Default)]
pub struct Wallet {
    /// Available cash for margin.
    pub cash: u64,
    /// Current positions by symbol (positive = long, negative = short).
    pub positions: HashMap<u32, i64>,
    /// Realized profit and loss for the session.
    pub realized_pnl: i64,
    /// P&L at session start (for drawdown calculation).
    pub session_start_pnl: i64,
    /// Weighted average entry price per symbol (for realized/unrealized P&L).
    avg_entry_prices: HashMap<u32, u64>,
}

impl Wallet {
    /// Create a wallet with initial cash.
    pub fn new(cash: u64) -> Self {
        Self {
            cash,
            positions: HashMap::new(),
            realized_pnl: 0,
            session_start_pnl: 0,
            avg_entry_prices: HashMap::new(),
        }
    }

    /// Get current position for a symbol (0 if no position).
    pub fn position(&self, symbol: u32) -> i64 {
        self.positions.get(&symbol).copied().unwrap_or(0)
    }

    /// Current drawdown = session_start_pnl - realized_pnl.
    pub fn current_drawdown(&self) -> i64 {
        self.session_start_pnl.saturating_sub(self.realized_pnl)
    }

    /// Total long exposure across all symbols.
    pub fn total_long_exposure(&self) -> u64 {
        self.positions
            .values()
            .filter(|&&p| p > 0)
            .map(|&p| p as u64)
            .sum()
    }

    /// Update position after a fill (saturating arithmetic).
    pub fn update_position(&mut self, symbol: u32, delta: i64) {
        let current = self.position(symbol);
        let new_position = current.saturating_add(delta);
        if new_position == 0 {
            self.positions.remove(&symbol);
        } else {
            self.positions.insert(symbol, new_position);
        }
    }

    /// Update cash balance (deposits, withdrawals).
    /// Positive delta adds cash, negative subtracts. Saturates at 0.
    pub fn update_cash(&mut self, delta: i64) {
        if delta >= 0 {
            self.cash = self.cash.saturating_add(delta as u64);
        } else {
            self.cash = self.cash.saturating_sub((-delta) as u64);
        }
    }

    /// Process a fill: update position, cash, avg entry price, and realized P&L.
    ///
    /// Cash flow:
    /// - Buy: cash -= price × quantity (pay for the asset)
    /// - Sell: cash += price × quantity (receive proceeds)
    ///
    /// Realized P&L (tracked for drawdown monitoring):
    /// - Closing long: (sell_price − avg_entry) × closed_qty
    /// - Closing short: (avg_entry − buy_price) × closed_qty
    ///
    /// Average entry price uses weighted-average cost method:
    /// - Opening new position: entry = fill_price
    /// - Adding to position: entry = (old_notional + new_notional) / total_qty
    /// - Reducing position: entry unchanged
    /// - Closing position: entry removed
    pub fn process_fill(&mut self, symbol: u32, side: OrderSide, qty: u64, price: u64) {
        let signed_delta = match side {
            OrderSide::Buy => qty as i64,
            OrderSide::Sell => -(qty as i64),
        };
        let prev_position = self.position(symbol);
        let new_position = prev_position.saturating_add(signed_delta);

        // Determine closed quantity (realizes P&L).
        let closed_qty: u64 = if prev_position > 0 && signed_delta < 0 {
            // Reducing/closing a long.
            (signed_delta.abs() as u64).min(prev_position as u64)
        } else if prev_position < 0 && signed_delta > 0 {
            // Reducing/closing a short.
            (signed_delta as u64).min((-prev_position) as u64)
        } else {
            0
        };

        // Calculate and record realized P&L.
        if closed_qty > 0 {
            if let Some(&avg_entry) = self.avg_entry_prices.get(&symbol) {
                let pnl_per_unit = if prev_position > 0 {
                    // Long close: sell_price − entry_price.
                    (price as i64).saturating_sub(avg_entry as i64)
                } else {
                    // Short close: entry_price − buy_price.
                    (avg_entry as i64).saturating_sub(price as i64)
                };
                let realized = pnl_per_unit.saturating_mul(closed_qty as i64);
                self.realized_pnl = self.realized_pnl.saturating_add(realized);
            }
        }

        // Update average entry price.
        if new_position == 0 {
            // Fully closed — remove avg entry.
            self.avg_entry_prices.remove(&symbol);
        } else if closed_qty == 0 {
            // Opening or adding to position — weighted average.
            let old_qty = prev_position.abs() as u64;
            let old_avg = self.avg_entry_prices.get(&symbol).copied().unwrap_or(0);
            if old_qty == 0 {
                self.avg_entry_prices.insert(symbol, price);
            } else {
                let total_notional = (old_avg as u128)
                    .saturating_mul(old_qty as u128)
                    .saturating_add((price as u128).saturating_mul(qty as u128));
                let new_qty = new_position.abs() as u128;
                let new_avg = if new_qty > 0 {
                    (total_notional / new_qty) as u64
                } else {
                    price
                };
                self.avg_entry_prices.insert(symbol, new_avg);
            }
        } else if (prev_position > 0) != (new_position > 0) {
            // Position flipped direction (e.g., long → short). The excess opens
            // a new position at the fill price.
            self.avg_entry_prices.insert(symbol, price);
        } else {
            // Partial close, same direction — avg entry unchanged.
        }

        // Cash settlement: buy pays, sell receives.
        let notional = price.saturating_mul(qty);
        match side {
            OrderSide::Buy => {
                self.cash = self.cash.saturating_sub(notional);
            },
            OrderSide::Sell => {
                self.cash = self.cash.saturating_add(notional);
            },
        }

        // Update position.
        self.update_position(symbol, signed_delta);
    }

    /// Unrealized P&L for a single symbol given current market price.
    /// Returns `None` if no position or no avg entry price.
    pub fn unrealized_pnl(&self, symbol: u32, current_price: u64) -> Option<i64> {
        let pos = self.positions.get(&symbol)?;
        let avg_entry = self.avg_entry_prices.get(&symbol)?;
        let pnl_per_unit = if *pos > 0 {
            // Long: profit when price rises.
            (current_price as i64).saturating_sub(*avg_entry as i64)
        } else {
            // Short: profit when price falls (entry > current).
            (*avg_entry as i64).saturating_sub(current_price as i64)
        };
        // Multiply by absolute position size; sign is already in pnl_per_unit.
        Some(pnl_per_unit.saturating_mul(pos.abs()))
    }

    /// Total unrealized P&L across all positions given current prices.
    pub fn mark_to_market(&self, prices: &HashMap<u32, u64>) -> i64 {
        self.positions
            .iter()
            .filter_map(|(&symbol, _)| self.unrealized_pnl(symbol, *prices.get(&symbol)?))
            .sum()
    }

    /// Total equity = cash + unrealized P&L (approximation ignoring position notional).
    pub fn total_equity(&self, prices: &HashMap<u32, u64>) -> i64 {
        (self.cash as i64).saturating_add(self.mark_to_market(prices))
    }

    /// Average entry price for a symbol (None if no position).
    pub fn avg_entry_price(&self, symbol: u32) -> Option<u64> {
        self.avg_entry_prices.get(&symbol).copied()
    }
}

/// Pre-trade risk guard implementing SEC 15c3-5 controls.
pub struct WalletGuard {
    params: RiskParams,
}

impl WalletGuard {
    /// Create a guard with custom risk parameters.
    pub fn new(params: RiskParams) -> Self {
        Self { params }
    }

    /// Create a guard with default parameters.
    pub fn with_defaults() -> Self {
        Self::new(RiskParams::default())
    }

    /// Full risk check against wallet state and order.
    ///
    /// Evaluates in order: zero-price → zero-qty → position limit → order size → drawdown → margin.
    pub fn check(&self, wallet: &Wallet, order: &Order) -> RiskDecision {
        if order.price == 0 {
            return RiskDecision::Reject(RejectReason::ZeroPrice);
        }
        if order.quantity == 0 {
            return RiskDecision::Reject(RejectReason::ZeroQuantity);
        }

        if let Err(reason) = self.check_position_limit(wallet, order) {
            return RiskDecision::Reject(reason);
        }
        if let Err(reason) = self.check_order_size(order) {
            return RiskDecision::Reject(reason);
        }
        if let Err(reason) = self.check_drawdown(wallet) {
            return RiskDecision::Reject(reason);
        }
        if let Err(reason) = self.check_margin(wallet, order) {
            return RiskDecision::Reject(reason);
        }

        RiskDecision::Approve
    }

    /// Check: |current_position + order_delta| ≤ π_max.
    fn check_position_limit(&self, wallet: &Wallet, order: &Order) -> Result<(), RejectReason> {
        let current_position = wallet.position(order.symbol);
        let delta = order.signed_quantity();
        let new_position = current_position
            .checked_add(delta)
            .ok_or(RejectReason::PositionOverflow)?;
        let abs_position = new_position.abs();
        if abs_position > self.params.pi_max {
            return Err(RejectReason::PositionLimitExceeded {
                would_be: new_position,
                max: self.params.pi_max,
            });
        }
        Ok(())
    }

    /// Check: quantity ≤ σ_max.
    fn check_order_size(&self, order: &Order) -> Result<(), RejectReason> {
        if order.quantity > self.params.sigma_max {
            return Err(RejectReason::OrderSizeExceeded {
                requested: order.quantity,
                max: self.params.sigma_max,
            });
        }
        Ok(())
    }

    /// Check: current_drawdown ≤ λ_max.
    fn check_drawdown(&self, wallet: &Wallet) -> Result<(), RejectReason> {
        let drawdown = wallet.current_drawdown();
        if drawdown > self.params.lambda_max {
            return Err(RejectReason::DailyDrawdownExceeded {
                current: drawdown,
                max: self.params.lambda_max,
            });
        }
        Ok(())
    }

    /// Check: required_margin ≤ wallet.cash (buy orders only).
    fn check_margin(&self, wallet: &Wallet, order: &Order) -> Result<(), RejectReason> {
        if order.side == OrderSide::Sell {
            return Ok(());
        }
        let notional = order
            .notional_value()
            .ok_or(RejectReason::PositionOverflow)?;
        let required_margin = notional
            .checked_div(self.params.margin_ratio)
            .unwrap_or(u64::MAX);
        if required_margin > wallet.cash {
            return Err(RejectReason::InsufficientMargin {
                required: required_margin,
                available: wallet.cash,
            });
        }
        Ok(())
    }

    /// Access the risk parameters.
    pub fn params(&self) -> &RiskParams {
        &self.params
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_wallet() -> Wallet {
        let mut w = Wallet::new(100_000_000);
        w.positions.insert(1, 100);
        w
    }

    fn test_guard() -> WalletGuard {
        WalletGuard::new(RiskParams {
            pi_max: 500,
            sigma_max: 200,
            lambda_max: 10_000_000,
            margin_ratio: 4,
        })
    }

    #[test]
    fn test_valid_buy_order() {
        assert_eq!(
            test_guard().check(&test_wallet(), &Order::new(1, OrderSide::Buy, 50, 150_00)),
            RiskDecision::Approve
        );
    }

    #[test]
    fn test_valid_sell_order() {
        assert_eq!(
            test_guard().check(&test_wallet(), &Order::new(1, OrderSide::Sell, 50, 2800_00)),
            RiskDecision::Approve
        );
    }

    #[test]
    fn test_position_limit_exceeded() {
        let d = test_guard().check(&test_wallet(), &Order::new(1, OrderSide::Buy, 450, 150_00));
        assert!(matches!(
            d,
            RiskDecision::Reject(RejectReason::PositionLimitExceeded { would_be: 550, .. })
        ));
    }

    #[test]
    fn test_order_size_exceeded() {
        let d = test_guard().check(&test_wallet(), &Order::new(1, OrderSide::Buy, 250, 100_00));
        assert!(matches!(
            d,
            RiskDecision::Reject(RejectReason::OrderSizeExceeded {
                requested: 250,
                max: 200
            })
        ));
    }

    #[test]
    fn test_daily_drawdown_exceeded() {
        let mut w = test_wallet();
        w.realized_pnl = -11_000_000;
        let d = test_guard().check(&w, &Order::new(2, OrderSide::Buy, 10, 100_00));
        assert!(matches!(
            d,
            RiskDecision::Reject(RejectReason::DailyDrawdownExceeded {
                current: 11_000_000,
                max: 10_000_000
            })
        ));
    }

    #[test]
    fn test_insufficient_margin() {
        let w = Wallet::new(1000_00);
        let d = test_guard().check(&w, &Order::new(1, OrderSide::Buy, 100, 500_00));
        assert!(matches!(
            d,
            RiskDecision::Reject(RejectReason::InsufficientMargin { .. })
        ));
    }

    #[test]
    fn test_zero_price_rejection() {
        assert_eq!(
            test_guard().check(&test_wallet(), &Order::new(1, OrderSide::Buy, 100, 0)),
            RiskDecision::Reject(RejectReason::ZeroPrice)
        );
    }

    #[test]
    fn test_zero_quantity_rejection() {
        assert_eq!(
            test_guard().check(&test_wallet(), &Order::new(1, OrderSide::Buy, 0, 100_00)),
            RiskDecision::Reject(RejectReason::ZeroQuantity)
        );
    }

    #[test]
    fn test_short_sell_within_limit() {
        assert_eq!(
            test_guard().check(&test_wallet(), &Order::new(1, OrderSide::Sell, 200, 100_00)),
            RiskDecision::Approve
        );
    }

    #[test]
    fn test_integer_overflow_protection() {
        let mut w = Wallet::new(u64::MAX);
        w.positions.insert(1, i64::MAX - 5);
        let g = WalletGuard::new(RiskParams {
            pi_max: i64::MAX,
            sigma_max: u64::MAX,
            lambda_max: i64::MAX,
            margin_ratio: 1,
        });
        assert_eq!(
            g.check(&w, &Order::new(1, OrderSide::Buy, 10, 1)),
            RiskDecision::Reject(RejectReason::PositionOverflow)
        );
    }

    #[test]
    fn test_position_at_exact_limit() {
        let mut w = Wallet::new(100_000_000);
        w.positions.insert(1, 450);
        assert_eq!(
            test_guard().check(&w, &Order::new(1, OrderSide::Buy, 50, 200_00)),
            RiskDecision::Approve
        );
    }

    #[test]
    fn test_drawdown_within_limit() {
        let mut w = Wallet::new(50_000_000);
        w.realized_pnl = -9_500_000;
        assert_eq!(
            test_guard().check(&w, &Order::new(1, OrderSide::Buy, 10, 400_00)),
            RiskDecision::Approve
        );
    }

    // ─── P&L and Cash Tests ──────────────────────────────────────────────

    #[test]
    fn test_update_cash_add() {
        let mut w = Wallet::new(1000);
        w.update_cash(500);
        assert_eq!(w.cash, 1500);
    }

    #[test]
    fn test_update_cash_subtract() {
        let mut w = Wallet::new(1000);
        w.update_cash(-300);
        assert_eq!(w.cash, 700);
    }

    #[test]
    fn test_update_cash_saturates_at_zero() {
        let mut w = Wallet::new(100);
        w.update_cash(-200);
        assert_eq!(w.cash, 0);
    }

    #[test]
    fn test_process_fill_buy_opens_long() {
        let mut w = Wallet::new(1_000_000);
        w.process_fill(1, OrderSide::Buy, 100, 100_00); // Buy 100 @ $100

        assert_eq!(w.position(1), 100);
        assert_eq!(w.cash, 1_000_000 - 100 * 100_00);
        assert_eq!(w.realized_pnl, 0);
        assert_eq!(w.avg_entry_price(1), Some(100_00));
    }

    #[test]
    fn test_process_fill_sell_opens_short() {
        let mut w = Wallet::new(1_000_000);
        w.process_fill(1, OrderSide::Sell, 50, 200_00); // Short 50 @ $200

        assert_eq!(w.position(1), -50);
        assert_eq!(w.cash, 1_000_000 + 50 * 200_00);
        assert_eq!(w.realized_pnl, 0);
        assert_eq!(w.avg_entry_price(1), Some(200_00));
    }

    #[test]
    fn test_process_fill_close_long_with_profit() {
        let mut w = Wallet::new(1_000_000);
        w.process_fill(1, OrderSide::Buy, 100, 100_00); // Buy 100 @ $100
        w.process_fill(1, OrderSide::Sell, 100, 110_00); // Sell 100 @ $110

        assert_eq!(w.position(1), 0); // Fully closed
        assert_eq!(w.realized_pnl, 100 * (110_00 - 100_00)); // +$1000
        assert_eq!(w.avg_entry_price(1), None);
    }

    #[test]
    fn test_process_fill_close_long_with_loss() {
        let mut w = Wallet::new(1_000_000);
        w.process_fill(1, OrderSide::Buy, 100, 100_00); // Buy 100 @ $100
        w.process_fill(1, OrderSide::Sell, 100, 90_00); // Sell 100 @ $90

        assert_eq!(w.position(1), 0);
        assert_eq!(w.realized_pnl, 100 * (90_00 - 100_00)); // -$1000
    }

    #[test]
    fn test_process_fill_close_short_with_profit() {
        let mut w = Wallet::new(1_000_000);
        w.process_fill(1, OrderSide::Sell, 100, 100_00); // Short 100 @ $100
        w.process_fill(1, OrderSide::Buy, 100, 90_00); // Cover 100 @ $90

        assert_eq!(w.position(1), 0);
        // Short profit: entry − exit = 100 − 90 = $10 per unit
        assert_eq!(w.realized_pnl, 100 * (100_00 - 90_00)); // +$1000
    }

    #[test]
    fn test_process_fill_partial_close() {
        let mut w = Wallet::new(1_000_000);
        w.process_fill(1, OrderSide::Buy, 100, 100_00); // Buy 100 @ $100
        w.process_fill(1, OrderSide::Sell, 40, 110_00); // Sell 40 @ $110

        assert_eq!(w.position(1), 60); // 100 − 40
        assert_eq!(w.realized_pnl, 40 * (110_00 - 100_00)); // +$400 on closed portion
        assert_eq!(w.avg_entry_price(1), Some(100_00)); // Unchanged on partial close
    }

    #[test]
    fn test_process_fill_add_to_position() {
        let mut w = Wallet::new(1_000_000);
        w.process_fill(1, OrderSide::Buy, 100, 100_00); // Buy 100 @ $100
        w.process_fill(1, OrderSide::Buy, 100, 200_00); // Buy 100 @ $200

        assert_eq!(w.position(1), 200);
        // Weighted avg: (100×100 + 100×200) / 200 = 150
        assert_eq!(w.avg_entry_price(1), Some(150_00));
    }

    #[test]
    fn test_process_fill_cash_accounting() {
        let mut w = Wallet::new(1_000_000);
        w.process_fill(1, OrderSide::Buy, 10, 100_00); // Spend 10,000
        w.process_fill(1, OrderSide::Sell, 10, 120_00); // Receive 12,000

        assert_eq!(w.cash, 1_000_000 - 10 * 100_00 + 10 * 120_00); // +2,000 net
    }

    #[test]
    fn test_unrealized_pnl_long() {
        let mut w = Wallet::new(1_000_000);
        w.process_fill(1, OrderSide::Buy, 100, 100_00);

        assert_eq!(w.unrealized_pnl(1, 110_00), Some(100 * (110_00 - 100_00))); // +$1000
        assert_eq!(w.unrealized_pnl(1, 90_00), Some(100 * (90_00 - 100_00))); // -$1000
    }

    #[test]
    fn test_unrealized_pnl_short() {
        let mut w = Wallet::new(1_000_000);
        w.process_fill(1, OrderSide::Sell, 50, 200_00);

        // Short: entry=200, price goes to 180 → profit = 200−180 = $20/unit
        assert_eq!(w.unrealized_pnl(1, 180_00), Some(50 * (200_00 - 180_00))); // +$1000
                                                                               // Price goes to 220 → loss = 200−220 = -$20/unit
        assert_eq!(w.unrealized_pnl(1, 220_00), Some(50 * (200_00 - 220_00))); // -$1000
    }

    #[test]
    fn test_unrealized_pnl_no_position() {
        let w = Wallet::new(1_000_000);
        assert_eq!(w.unrealized_pnl(1, 100_00), None);
    }

    #[test]
    fn test_mark_to_market_multi_symbol() {
        let mut w = Wallet::new(1_000_000);
        w.process_fill(1, OrderSide::Buy, 100, 100_00); // Long 100 AAPL @ $100
        w.process_fill(2, OrderSide::Sell, 50, 200_00); // Short 50 GOOG @ $200

        let mut prices = HashMap::new();
        prices.insert(1, 110_00); // AAPL up $10
        prices.insert(2, 190_00); // GOOG down $10

        // Long: 100 × $10 = +$1000
        // Short: 50 × $10 = +$500 (profit when price drops)
        assert_eq!(
            w.mark_to_market(&prices),
            100 * (110_00 - 100_00) + 50 * (200_00 - 190_00)
        );
    }

    #[test]
    fn test_total_equity() {
        let mut w = Wallet::new(1_000_000);
        w.process_fill(1, OrderSide::Buy, 100, 100_00);

        let mut prices = HashMap::new();
        prices.insert(1, 110_00); // Up $10 → unrealized +$100,000 cents

        // Cash = 1M - 100*10000 = 0. Unrealized = 100 * (11000-10000) = 100,000.
        assert_eq!(w.total_equity(&prices), 100_000);
    }

    #[test]
    fn test_process_fill_multi_symbol_independent() {
        let mut w = Wallet::new(5_000_000);
        w.process_fill(1, OrderSide::Buy, 50, 100_00);
        w.process_fill(2, OrderSide::Buy, 30, 200_00);

        assert_eq!(w.position(1), 50);
        assert_eq!(w.position(2), 30);
        assert_eq!(w.avg_entry_price(1), Some(100_00));
        assert_eq!(w.avg_entry_price(2), Some(200_00));

        // Close symbol 1 with profit.
        w.process_fill(1, OrderSide::Sell, 50, 120_00);
        assert_eq!(w.position(1), 0);
        assert_eq!(w.realized_pnl, 50 * (120_00 - 100_00)); // +$1000

        // Symbol 2 unaffected.
        assert_eq!(w.position(2), 30);
        assert_eq!(w.avg_entry_price(2), Some(200_00));
    }

    #[test]
    fn test_process_fill_flip_position_long_to_short() {
        let mut w = Wallet::new(10_000_000);
        w.process_fill(1, OrderSide::Buy, 100, 100_00); // Long 100 @ $100
        w.process_fill(1, OrderSide::Sell, 150, 100_00); // Sell 150 @ $100

        // First 100 close the long (P&L = 0 since same price).
        // Next 50 open a new short.
        assert_eq!(w.position(1), -50);
        assert_eq!(w.realized_pnl, 0); // Closed at same price.
        assert_eq!(w.avg_entry_price(1), Some(100_00)); // New short entry.
    }

    #[test]
    fn test_process_fill_flip_with_profit() {
        let mut w = Wallet::new(10_000_000);
        w.process_fill(1, OrderSide::Buy, 100, 100_00); // Long 100 @ $100
        w.process_fill(1, OrderSide::Sell, 150, 120_00); // Sell 150 @ $120

        // First 100 close the long: P&L = (120−100) × 100 = +$2000
        // Next 50 open short at $120.
        assert_eq!(w.position(1), -50);
        assert_eq!(w.realized_pnl, 100 * (120_00 - 100_00)); // +$2000
        assert_eq!(w.avg_entry_price(1), Some(120_00)); // Short entry price.
    }

    #[test]
    fn test_process_fill_saturating_cash() {
        let mut w = Wallet::new(100);
        // Buy more than we can afford — cash saturates at 0.
        w.process_fill(1, OrderSide::Buy, 1000, 1);
        assert_eq!(w.cash, 0);
        assert_eq!(w.position(1), 1000);
    }
}
