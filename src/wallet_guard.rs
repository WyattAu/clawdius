//! Wallet Guard - Re-export from clawdius_core
//!
//! The canonical implementation lives in `crates/clawdius-core/src/broker/wallet_guard.rs`.
//! This module re-exports all types for backward compatibility with the root binary.

pub use clawdius_core::broker::wallet_guard::{
    RejectReason, RiskDecision, RiskParams, Wallet, WalletGuard, DEFAULT_MARGIN_RATIO,
    MAX_DRAWDOWN, MAX_ORDER_SIZE, MAX_POSITION,
};

pub use clawdius_core::broker::Order;
pub use clawdius_core::broker::OrderSide;
