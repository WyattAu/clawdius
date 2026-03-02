//! Wallet Guard - Pre-trade risk check algorithm
//!
//! Per YP-HFT-BROKER-001:
//! - Implements SEC Rule 15c3-5 risk controls
//! - Position limit check (π_max)
//! - Order size check (σ_max)
//! - Daily drawdown check (λ_max)
//! - Margin requirement check
//! - WCET target: < 100µs

use std::collections::HashMap;

pub const MAX_POSITION: i64 = 10_000;
pub const MAX_ORDER_SIZE: u64 = 1_000;
pub const MAX_DRAWDOWN: i64 = 10_000_000;
pub const DEFAULT_MARGIN_RATIO: u64 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskDecision {
    Approve,
    Reject(RejectReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectReason {
    PositionLimitExceeded { would_be: i64, max: i64 },
    OrderSizeExceeded { requested: u64, max: u64 },
    DailyDrawdownExceeded { current: i64, max: i64 },
    InsufficientMargin { required: u64, available: u64 },
    PositionOverflow,
    NegativePrice,
    NegativeQuantity,
}

#[derive(Debug, Clone)]
pub struct RiskParams {
    pub pi_max: i64,
    pub sigma_max: u64,
    pub lambda_max: i64,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone)]
pub struct Order {
    pub symbol: u32,
    pub side: OrderSide,
    pub quantity: u64,
    pub price: u64,
}

impl Order {
    pub fn new(symbol: u32, side: OrderSide, quantity: u64, price: u64) -> Self {
        Self {
            symbol,
            side,
            quantity,
            price,
        }
    }

    pub fn notional_value(&self) -> Option<u64> {
        self.price.checked_mul(self.quantity)
    }

    pub fn signed_quantity(&self) -> i64 {
        match self.side {
            OrderSide::Buy => self.quantity as i64,
            OrderSide::Sell => -(self.quantity as i64),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Wallet {
    pub cash: u64,
    pub positions: HashMap<u32, i64>,
    pub realized_pnl: i64,
    pub session_start_pnl: i64,
}

impl Wallet {
    pub fn new(cash: u64) -> Self {
        Self {
            cash,
            positions: HashMap::new(),
            realized_pnl: 0,
            session_start_pnl: 0,
        }
    }

    pub fn position(&self, symbol: u32) -> i64 {
        self.positions.get(&symbol).copied().unwrap_or(0)
    }

    pub fn current_drawdown(&self) -> i64 {
        self.session_start_pnl.saturating_sub(self.realized_pnl)
    }

    pub fn total_long_exposure(&self) -> u64 {
        self.positions
            .values()
            .filter(|&&p| p > 0)
            .map(|&p| p as u64)
            .sum()
    }

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

#[derive(Debug)]
pub struct WalletGuard {
    params: RiskParams,
}

impl WalletGuard {
    pub fn new(params: RiskParams) -> Self {
        Self { params }
    }

    pub fn with_defaults() -> Self {
        Self::new(RiskParams::default())
    }

    pub fn check(&self, wallet: &Wallet, order: &Order) -> RiskDecision {
        if order.price == 0 {
            return RiskDecision::Reject(RejectReason::NegativePrice);
        }
        if order.quantity == 0 {
            return RiskDecision::Reject(RejectReason::NegativeQuantity);
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

    fn check_order_size(&self, order: &Order) -> Result<(), RejectReason> {
        if order.quantity > self.params.sigma_max {
            return Err(RejectReason::OrderSizeExceeded {
                requested: order.quantity,
                max: self.params.sigma_max,
            });
        }
        Ok(())
    }

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

    pub fn params(&self) -> &RiskParams {
        &self.params
    }
}

#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use super::*;

    fn create_test_wallet() -> Wallet {
        let mut wallet = Wallet::new(100_000_000);
        wallet.positions.insert(1, 100);
        wallet
    }

    fn create_test_guard() -> WalletGuard {
        WalletGuard::new(RiskParams {
            pi_max: 500,
            sigma_max: 200,
            lambda_max: 10_000_000,
            margin_ratio: 4,
        })
    }

    #[test]
    fn test_valid_buy_order() {
        let wallet = create_test_wallet();
        let guard = create_test_guard();
        let order = Order::new(1, OrderSide::Buy, 50, 150_00);

        let decision = guard.check(&wallet, &order);
        assert_eq!(decision, RiskDecision::Approve);
    }

    #[test]
    fn test_valid_sell_order() {
        let wallet = create_test_wallet();
        let guard = create_test_guard();
        let order = Order::new(1, OrderSide::Sell, 50, 2800_00);

        let decision = guard.check(&wallet, &order);
        assert_eq!(decision, RiskDecision::Approve);
    }

    #[test]
    fn test_position_limit_exceeded() {
        let wallet = create_test_wallet();
        let guard = create_test_guard();
        let order = Order::new(1, OrderSide::Buy, 450, 150_00);

        let decision = guard.check(&wallet, &order);
        match decision {
            RiskDecision::Reject(RejectReason::PositionLimitExceeded { would_be, .. }) => {
                assert_eq!(would_be, 550);
            }
            _ => panic!("Expected PositionLimitExceeded rejection"),
        }
    }

    #[test]
    fn test_order_size_exceeded() {
        let wallet = create_test_wallet();
        let guard = create_test_guard();
        let order = Order::new(1, OrderSide::Buy, 250, 100_00);

        let decision = guard.check(&wallet, &order);
        match decision {
            RiskDecision::Reject(RejectReason::OrderSizeExceeded { requested, max }) => {
                assert_eq!(requested, 250);
                assert_eq!(max, 200);
            }
            _ => panic!("Expected OrderSizeExceeded rejection"),
        }
    }

    #[test]
    fn test_daily_drawdown_exceeded() {
        let mut wallet = create_test_wallet();
        wallet.realized_pnl = -11_000_000;
        wallet.session_start_pnl = 0;

        let guard = create_test_guard();
        let order = Order::new(2, OrderSide::Buy, 10, 100_00);

        let decision = guard.check(&wallet, &order);
        match decision {
            RiskDecision::Reject(RejectReason::DailyDrawdownExceeded { current, max }) => {
                assert_eq!(current, 11_000_000);
                assert_eq!(max, 10_000_000);
            }
            _ => panic!("Expected DailyDrawdownExceeded rejection"),
        }
    }

    #[test]
    fn test_insufficient_margin() {
        let wallet = Wallet::new(1000_00);
        let guard = create_test_guard();
        let order = Order::new(1, OrderSide::Buy, 100, 500_00);

        let decision = guard.check(&wallet, &order);
        match decision {
            RiskDecision::Reject(RejectReason::InsufficientMargin {
                required,
                available,
            }) => {
                assert!(required > available);
            }
            _ => panic!("Expected InsufficientMargin rejection"),
        }
    }

    #[test]
    fn test_zero_position_first_order() {
        let wallet = Wallet::new(1_000_000_00);
        let guard = create_test_guard();
        let order = Order::new(99, OrderSide::Buy, 100, 300_00);

        let decision = guard.check(&wallet, &order);
        assert_eq!(decision, RiskDecision::Approve);
    }

    #[test]
    fn test_position_at_limit() {
        let mut wallet = Wallet::new(100_000_000);
        wallet.positions.insert(1, 450);

        let guard = create_test_guard();
        let order = Order::new(1, OrderSide::Buy, 50, 200_00);

        let decision = guard.check(&wallet, &order);
        assert_eq!(decision, RiskDecision::Approve);
    }

    #[test]
    fn test_drawdown_within_limit() {
        let mut wallet = Wallet::new(50_000_000);
        wallet.realized_pnl = -95_000_00;
        wallet.session_start_pnl = 0;

        let guard = create_test_guard();
        let order = Order::new(1, OrderSide::Buy, 10, 400_00);

        let decision = guard.check(&wallet, &order);
        assert_eq!(decision, RiskDecision::Approve);
    }

    #[test]
    fn test_integer_overflow_protection() {
        let mut wallet = Wallet::new(u64::MAX);
        wallet.positions.insert(1, i64::MAX - 5);

        let guard = WalletGuard::new(RiskParams {
            pi_max: i64::MAX,
            sigma_max: u64::MAX,
            lambda_max: i64::MAX,
            margin_ratio: 1,
        });

        let order = Order::new(1, OrderSide::Buy, 10, 1);
        let decision = guard.check(&wallet, &order);

        match decision {
            RiskDecision::Reject(RejectReason::PositionOverflow) => {}
            _ => panic!("Expected PositionOverflow rejection"),
        }
    }

    #[test]
    fn test_negative_price_rejection() {
        let wallet = create_test_wallet();
        let guard = create_test_guard();
        let order = Order::new(1, OrderSide::Buy, 100, 0);

        let decision = guard.check(&wallet, &order);
        assert_eq!(decision, RiskDecision::Reject(RejectReason::NegativePrice));
    }

    #[test]
    fn test_zero_quantity_rejection() {
        let wallet = create_test_wallet();
        let guard = create_test_guard();
        let order = Order::new(1, OrderSide::Buy, 0, 100_00);

        let decision = guard.check(&wallet, &order);
        assert_eq!(
            decision,
            RiskDecision::Reject(RejectReason::NegativeQuantity)
        );
    }

    #[test]
    fn test_sell_reduces_position() {
        let wallet = create_test_wallet();
        let guard = create_test_guard();
        let order = Order::new(1, OrderSide::Sell, 50, 100_00);

        let decision = guard.check(&wallet, &order);
        assert_eq!(decision, RiskDecision::Approve);
    }

    #[test]
    fn test_short_sell_approved() {
        let wallet = create_test_wallet();
        let guard = create_test_guard();
        let order = Order::new(1, OrderSide::Sell, 200, 100_00);

        let decision = guard.check(&wallet, &order);
        match decision {
            RiskDecision::Approve => {}
            RiskDecision::Reject(RejectReason::PositionLimitExceeded { would_be, .. }) => {
                assert_eq!(would_be, -100);
            }
            _ => panic!("Unexpected rejection"),
        }
    }
}
