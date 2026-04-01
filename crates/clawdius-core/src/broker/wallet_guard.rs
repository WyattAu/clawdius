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
}

impl Wallet {
    /// Create a wallet with initial cash.
    pub fn new(cash: u64) -> Self {
        Self {
            cash,
            positions: HashMap::new(),
            realized_pnl: 0,
            session_start_pnl: 0,
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
}
