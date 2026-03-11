//! Wallet Guard - SEC 15c3-5 Risk Checks
//!
//! Pre-trade risk controls for market access compliance.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Order representation for risk checking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    /// Symbol being traded
    pub symbol: String,
    /// Order quantity
    pub quantity: Decimal,
    /// Order price
    pub price: Decimal,
    /// Side (buy/sell)
    pub side: OrderSide,
}

/// Order side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderSide {
    /// Buy order
    Buy,
    /// Sell order
    Sell,
}

impl Order {
    /// Calculate the total order value.
    #[must_use]
    pub fn value(&self) -> Decimal {
        self.quantity * self.price
    }
}

/// Types of risk check failures.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskCheck {
    /// Order value exceeds maximum allowed
    OrderValueLimit,
    /// Daily volume limit exceeded
    DailyVolumeLimit,
    /// Position size exceeds maximum allowed
    PositionSizeLimit,
    /// Symbol is restricted from trading
    RestrictedSymbol,
}

/// Wallet guard implementing SEC 15c3-5 pre-trade risk controls.
#[derive(Debug, Clone)]
pub struct WalletGuard {
    /// Maximum value per order
    pub max_order_value: Decimal,
    /// Maximum daily trading volume
    pub max_daily_volume: Decimal,
    /// Maximum position size per symbol
    pub max_position_size: Decimal,
    /// Symbols restricted from trading
    pub restricted_symbols: HashSet<String>,
}

impl WalletGuard {
    /// Creates a new wallet guard with specified limits.
    #[must_use]
    pub fn new(
        max_order_value: Decimal,
        max_daily_volume: Decimal,
        max_position_size: Decimal,
    ) -> Self {
        Self {
            max_order_value,
            max_daily_volume,
            max_position_size,
            restricted_symbols: HashSet::new(),
        }
    }

    /// Adds a symbol to the restricted list.
    pub fn restrict_symbol(&mut self, symbol: impl Into<String>) {
        self.restricted_symbols.insert(symbol.into());
    }

    /// Checks an order against all risk controls.
    ///
    /// Returns `Ok(())` if the order passes all checks.
    /// Returns `Err(Vec<RiskCheck>)` with all failed checks.
    pub fn check_order(&self, order: &Order) -> Result<(), Vec<RiskCheck>> {
        let mut failures = Vec::new();

        if order.value() > self.max_order_value {
            failures.push(RiskCheck::OrderValueLimit);
        }

        if self.restricted_symbols.contains(&order.symbol) {
            failures.push(RiskCheck::RestrictedSymbol);
        }

        if order.quantity > self.max_position_size {
            failures.push(RiskCheck::PositionSizeLimit);
        }

        if failures.is_empty() {
            Ok(())
        } else {
            Err(failures)
        }
    }

    /// Validates market access is properly configured.
    #[must_use]
    pub fn validate_market_access(&self) -> bool {
        self.max_order_value > Decimal::ZERO
            && self.max_daily_volume > Decimal::ZERO
            && self.max_position_size > Decimal::ZERO
    }

    /// Checks daily volume against limit.
    ///
    /// Returns `true` if within limits.
    #[must_use]
    pub fn check_daily_volume(&self, current_volume: Decimal) -> bool {
        current_volume <= self.max_daily_volume
    }
}

impl Default for WalletGuard {
    fn default() -> Self {
        Self::new(
            Decimal::from(1_000_000),
            Decimal::from(10_000_000),
            Decimal::from(100_000),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_order(value: Decimal, symbol: &str, quantity: Decimal) -> Order {
        Order {
            symbol: symbol.to_string(),
            quantity,
            price: value / quantity,
            side: OrderSide::Buy,
        }
    }

    #[test]
    fn test_order_value_limit() {
        let guard = WalletGuard::new(Decimal::from(1000), Decimal::MAX, Decimal::MAX);
        let order = test_order(Decimal::from(2000), "AAPL", Decimal::from(10));
        let result = guard.check_order(&order);
        assert!(matches!(result, Err(ref v) if v.contains(&RiskCheck::OrderValueLimit)));
    }

    #[test]
    fn test_restricted_symbol() {
        let mut guard = WalletGuard::default();
        guard.restrict_symbol("PENN");
        let order = test_order(Decimal::from(100), "PENN", Decimal::from(1));
        let result = guard.check_order(&order);
        assert!(matches!(result, Err(ref v) if v.contains(&RiskCheck::RestrictedSymbol)));
    }

    #[test]
    fn test_valid_order() {
        let guard = WalletGuard::default();
        let order = test_order(Decimal::from(1000), "AAPL", Decimal::from(10));
        assert!(guard.check_order(&order).is_ok());
    }

    #[test]
    fn test_validate_market_access() {
        let guard = WalletGuard::default();
        assert!(guard.validate_market_access());
    }
}
