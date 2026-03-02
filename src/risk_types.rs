//! Risk Types for HFT Broker
//!
//! Per BP-HFT-BROKER-001 and YP-HFT-BROKER-001:
//! - Risk parameter types
//! - Position, Account, RiskLimits types
//! - Currency types with rust_decimal

use std::collections::HashMap;

pub type PositionId = u64;
pub type AccountId = u64;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Currency(pub u8);

impl Currency {
    pub const USD: Self = Self(0);
    pub const EUR: Self = Self(1);
    pub const GBP: Self = Self(2);
    pub const JPY: Self = Self(3);
    pub const BTC: Self = Self(4);
    pub const ETH: Self = Self(5);
}

impl std::fmt::Display for Currency {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            0 => write!(f, "USD"),
            1 => write!(f, "EUR"),
            2 => write!(f, "GBP"),
            3 => write!(f, "JPY"),
            4 => write!(f, "BTC"),
            5 => write!(f, "ETH"),
            _ => write!(f, "UNKNOWN"),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Money {
    pub amount: i64,
    pub currency: Currency,
}

impl Money {
    pub fn new(amount: i64, currency: Currency) -> Self {
        Self { amount, currency }
    }

    pub fn usd(amount: i64) -> Self {
        Self::new(amount, Currency::USD)
    }

    pub fn checked_add(&self, other: &Money) -> Option<Money> {
        if self.currency != other.currency {
            return None;
        }
        self.amount
            .checked_add(other.amount)
            .map(|a| Money::new(a, self.currency))
    }

    pub fn checked_sub(&self, other: &Money) -> Option<Money> {
        if self.currency != other.currency {
            return None;
        }
        self.amount
            .checked_sub(other.amount)
            .map(|a| Money::new(a, self.currency))
    }

    pub fn abs(&self) -> Self {
        Self::new(self.amount.abs(), self.currency)
    }

    pub fn is_positive(&self) -> bool {
        self.amount > 0
    }

    pub fn is_negative(&self) -> bool {
        self.amount < 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PositionSide {
    Long,
    Short,
    Flat,
}

impl Default for PositionSide {
    fn default() -> Self {
        Self::Flat
    }
}

#[derive(Debug, Clone)]
pub struct Position {
    pub id: PositionId,
    pub symbol: crate::market_data::Symbol,
    pub quantity: i64,
    pub avg_price: i64,
    pub unrealized_pnl: i64,
    pub realized_pnl: i64,
    pub opened_at: std::time::Instant,
    pub updated_at: std::time::Instant,
}

impl Position {
    pub fn new(id: PositionId, symbol: crate::market_data::Symbol) -> Self {
        let now = std::time::Instant::now();
        Self {
            id,
            symbol,
            quantity: 0,
            avg_price: 0,
            unrealized_pnl: 0,
            realized_pnl: 0,
            opened_at: now,
            updated_at: now,
        }
    }

    pub fn side(&self) -> PositionSide {
        if self.quantity > 0 {
            PositionSide::Long
        } else if self.quantity < 0 {
            PositionSide::Short
        } else {
            PositionSide::Flat
        }
    }

    pub fn is_flat(&self) -> bool {
        self.quantity == 0
    }

    pub fn market_value(&self, current_price: i64) -> i64 {
        self.quantity * current_price
    }

    pub fn update_unrealized_pnl(&mut self, current_price: i64) {
        if self.quantity != 0 {
            self.unrealized_pnl = (current_price - self.avg_price) * self.quantity;
        } else {
            self.unrealized_pnl = 0;
        }
        self.updated_at = std::time::Instant::now();
    }
}

#[derive(Debug, Clone)]
pub struct RiskLimits {
    pub max_position_size: i64,
    pub max_order_size: u64,
    pub max_daily_drawdown: i64,
    pub max_delta_exposure: i64,
    pub margin_requirement: u64,
    pub max_orders_per_second: u32,
    pub max_notional_per_order: i64,
}

impl Default for RiskLimits {
    fn default() -> Self {
        Self {
            max_position_size: 1_000_000,
            max_order_size: 10_000,
            max_daily_drawdown: 10_000_000,
            max_delta_exposure: 500_000_000,
            margin_requirement: 4,
            max_orders_per_second: 100,
            max_notional_per_order: 100_000_000,
        }
    }
}

impl RiskLimits {
    pub fn conservative() -> Self {
        Self {
            max_position_size: 100_000,
            max_order_size: 1_000,
            max_daily_drawdown: 1_000_000,
            max_delta_exposure: 50_000_000,
            margin_requirement: 2,
            max_orders_per_second: 10,
            max_notional_per_order: 10_000_000,
        }
    }

    pub fn aggressive() -> Self {
        Self {
            max_position_size: 10_000_000,
            max_order_size: 100_000,
            max_daily_drawdown: 100_000_000,
            max_delta_exposure: 5_000_000_000,
            margin_requirement: 10,
            max_orders_per_second: 1000,
            max_notional_per_order: 1_000_000_000,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Account {
    pub id: AccountId,
    pub cash: Money,
    pub margin_used: Money,
    pub available_margin: Money,
    pub positions: HashMap<crate::market_data::Symbol, Position>,
    pub total_pnl: i64,
    pub daily_pnl: i64,
    pub session_start_pnl: i64,
    pub limits: RiskLimits,
}

impl Account {
    pub fn new(id: AccountId, initial_cash: Money) -> Self {
        let available_margin = Money::new(initial_cash.amount, initial_cash.currency);
        Self {
            id,
            cash: initial_cash,
            margin_used: Money::default(),
            available_margin,
            positions: HashMap::new(),
            total_pnl: 0,
            daily_pnl: 0,
            session_start_pnl: 0,
            limits: RiskLimits::default(),
        }
    }

    pub fn equity(&self) -> i64 {
        let unrealized: i64 = self.positions.values().map(|p| p.unrealized_pnl).sum();
        self.cash.amount + unrealized
    }

    pub fn buying_power(&self) -> i64 {
        self.available_margin.amount * self.limits.margin_requirement as i64
    }

    pub fn margin_level(&self) -> u64 {
        if self.margin_used.amount == 0 {
            return 100;
        }
        let level = (self.equity() as f64 / self.margin_used.amount as f64 * 100.0) as u64;
        level.min(100)
    }

    pub fn position(&self, symbol: crate::market_data::Symbol) -> Option<&Position> {
        self.positions.get(&symbol)
    }

    pub fn position_mut(&mut self, symbol: crate::market_data::Symbol) -> Option<&mut Position> {
        self.positions.get_mut(&symbol)
    }

    pub fn total_position_count(&self) -> usize {
        self.positions.values().filter(|p| !p.is_flat()).count()
    }

    pub fn daily_drawdown(&self) -> i64 {
        self.session_start_pnl - self.daily_pnl
    }

    pub fn is_within_limits(&self) -> bool {
        self.daily_drawdown() <= self.limits.max_daily_drawdown
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RiskCheckResult {
    pub approved: bool,
    pub reason_code: Option<u16>,
    pub check_duration_us: u64,
}

impl RiskCheckResult {
    pub fn approved(duration_us: u64) -> Self {
        Self {
            approved: true,
            reason_code: None,
            check_duration_us: duration_us,
        }
    }

    pub fn rejected(reason_code: u16, duration_us: u64) -> Self {
        Self {
            approved: false,
            reason_code: Some(reason_code),
            check_duration_us: duration_us,
        }
    }
}

pub const RISK_REJECT_POSITION_LIMIT: u16 = 0x5002;
pub const RISK_REJECT_ORDER_SIZE: u16 = 0x5003;
pub const RISK_REJECT_DRAWDOWN: u16 = 0x5004;
pub const RISK_REJECT_MARGIN: u16 = 0x5005;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_money_operations() {
        let a = Money::usd(10000);
        let b = Money::usd(5000);

        assert_eq!(a.checked_add(&b), Some(Money::usd(15000)));
        assert_eq!(a.checked_sub(&b), Some(Money::usd(5000)));

        let c = Money::new(5000, Currency::EUR);
        assert_eq!(a.checked_add(&c), None);
    }

    #[test]
    fn test_position_side() {
        let mut pos = Position::new(1, 42);
        assert!(pos.is_flat());
        assert_eq!(pos.side(), PositionSide::Flat);

        pos.quantity = 100;
        assert!(!pos.is_flat());
        assert_eq!(pos.side(), PositionSide::Long);

        pos.quantity = -100;
        assert_eq!(pos.side(), PositionSide::Short);
    }

    #[test]
    fn test_position_pnl() {
        let mut pos = Position::new(1, 42);
        pos.quantity = 100;
        pos.avg_price = 10000;

        pos.update_unrealized_pnl(10500);
        assert_eq!(pos.unrealized_pnl, 50000);
    }

    #[test]
    fn test_account_equity() {
        let account = Account::new(1, Money::usd(100_000_000));
        assert_eq!(account.equity(), 100_000_000);
    }

    #[test]
    fn test_risk_limits_presets() {
        let default_limits = RiskLimits::default();
        let conservative = RiskLimits::conservative();
        let aggressive = RiskLimits::aggressive();

        assert!(conservative.max_position_size < default_limits.max_position_size);
        assert!(aggressive.max_position_size > default_limits.max_position_size);
    }

    #[test]
    fn test_risk_check_result() {
        let approved = RiskCheckResult::approved(50);
        assert!(approved.approved);
        assert!(approved.reason_code.is_none());

        let rejected = RiskCheckResult::rejected(RISK_REJECT_DRAWDOWN, 25);
        assert!(!rejected.approved);
        assert_eq!(rejected.reason_code, Some(RISK_REJECT_DRAWDOWN));
    }
}
