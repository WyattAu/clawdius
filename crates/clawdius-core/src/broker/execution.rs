//! Order Execution Adapter
//!
//! Trait for submitting orders to exchanges and receiving fill confirmations.
//! Includes a SimulatedExecution for testing.

use super::wallet_guard::{Order, OrderSide};
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};

/// A fill report from order execution.
#[derive(Debug, Clone)]
pub struct Fill {
    /// Original order symbol.
    pub symbol: u32,
    /// Side that was executed.
    pub side: OrderSide,
    /// Quantity filled.
    pub quantity: u64,
    /// Fill price.
    pub price: u64,
    /// Unix timestamp (nanos).
    pub timestamp: u64,
}

/// Execution error types.
#[derive(Debug, thiserror::Error)]
pub enum ExecutionError {
    #[error("Order rejected: {0}")]
    Rejected(String),
    #[error("Connection error: {0}")]
    ConnectionFailed(String),
    #[error("Not connected")]
    NotConnected,
    #[error("Order not found")]
    OrderNotFound,
}

/// Trait for order execution adapters.
pub trait ExecutionAdapter: Send + Sync {
    /// Submit an order for execution. Returns fill on success.
    fn submit_order(&self, order: &Order) -> Result<Fill, ExecutionError>;

    /// Cancel a pending order.
    fn cancel_order(&self, _symbol: u32) -> Result<(), ExecutionError> {
        Ok(())
    }
}

/// Simulated execution engine for testing.
/// Fills orders instantly at the order price.
pub struct SimulatedExecution {
    /// If true, all orders are rejected.
    reject_all: AtomicBool,
    /// Fill price offset (for simulating slippage).
    slippage_ticks: AtomicI64,
}

impl SimulatedExecution {
    pub fn new() -> Self {
        Self {
            reject_all: AtomicBool::new(false),
            slippage_ticks: AtomicI64::new(0),
        }
    }

    pub fn set_reject_all(&self, reject: bool) {
        self.reject_all.store(reject, Ordering::SeqCst);
    }

    pub fn set_slippage_ticks(&self, ticks: i64) {
        self.slippage_ticks.store(ticks, Ordering::SeqCst);
    }
}

impl Default for SimulatedExecution {
    fn default() -> Self {
        Self::new()
    }
}

impl ExecutionAdapter for SimulatedExecution {
    fn submit_order(&self, order: &Order) -> Result<Fill, ExecutionError> {
        if self.reject_all.load(Ordering::SeqCst) {
            return Err(ExecutionError::Rejected(
                "reject_all mode enabled".to_string(),
            ));
        }

        if order.quantity == 0 {
            return Err(ExecutionError::Rejected("zero quantity".to_string()));
        }

        if order.price == 0 {
            return Err(ExecutionError::Rejected("zero price".to_string()));
        }

        let slippage = self.slippage_ticks.load(Ordering::SeqCst);
        let fill_price = if slippage >= 0 {
            order.price.checked_add(slippage as u64).unwrap_or(u64::MAX)
        } else {
            order.price.checked_sub((-slippage) as u64).unwrap_or(0)
        };

        Ok(Fill {
            symbol: order.symbol,
            side: order.side,
            quantity: order.quantity,
            price: fill_price,
            timestamp: 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_successful_buy_fill() {
        let exec = SimulatedExecution::new();
        let order = Order::new(1, OrderSide::Buy, 100, 5000);

        let fill = exec.submit_order(&order).unwrap();

        assert_eq!(fill.symbol, 1);
        assert_eq!(fill.side, OrderSide::Buy);
        assert_eq!(fill.quantity, 100);
        assert_eq!(fill.price, 5000);
    }

    #[test]
    fn test_successful_sell_fill() {
        let exec = SimulatedExecution::new();
        let order = Order::new(2, OrderSide::Sell, 50, 10000);

        let fill = exec.submit_order(&order).unwrap();

        assert_eq!(fill.symbol, 2);
        assert_eq!(fill.side, OrderSide::Sell);
        assert_eq!(fill.quantity, 50);
        assert_eq!(fill.price, 10000);
    }

    #[test]
    fn test_reject_all_mode() {
        let exec = SimulatedExecution::new();
        exec.set_reject_all(true);

        let order = Order::new(1, OrderSide::Buy, 100, 5000);
        let result = exec.submit_order(&order);

        assert!(result.is_err());
        assert!(matches!(result, Err(ExecutionError::Rejected(_))));
    }

    #[test]
    fn test_slippage_applied_correctly() {
        let exec = SimulatedExecution::new();
        exec.set_slippage_ticks(3);

        let order = Order::new(1, OrderSide::Buy, 100, 5000);
        let fill = exec.submit_order(&order).unwrap();
        assert_eq!(fill.price, 5003);

        exec.set_slippage_ticks(-2);
        let fill = exec.submit_order(&order).unwrap();
        assert_eq!(fill.price, 4998);
    }

    #[test]
    fn test_zero_qty_rejected() {
        let exec = SimulatedExecution::new();
        let order = Order::new(1, OrderSide::Buy, 0, 5000);
        let result = exec.submit_order(&order);
        assert!(result.is_err());
    }

    #[test]
    fn test_zero_price_rejected() {
        let exec = SimulatedExecution::new();
        let order = Order::new(1, OrderSide::Buy, 100, 0);
        let result = exec.submit_order(&order);
        assert!(result.is_err());
    }
}
