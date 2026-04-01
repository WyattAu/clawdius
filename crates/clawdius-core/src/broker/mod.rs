//! HFT Broker Module
//!
//! High-frequency trading broker components with lock-free data structures
//! and risk management (SEC 15c3-5 compliance).

pub mod arena;
pub mod execution;
pub mod feed_manager;
pub mod feeds;
pub mod notification;
pub mod ring_buffer;
pub mod signal;
pub mod strategy;
pub mod wallet_guard;

pub use arena::Arena;
pub use execution::{ExecutionAdapter, ExecutionError, Fill, SimulatedExecution};
pub use feed_manager::{FeedManager, FeedSubscriber, LoggingSubscriber};
pub use feeds::{FeedError, FeedStatus, MarketFeed, MarketUpdate, Quote, SimulatedFeed, Symbol};
pub use notification::{NotificationChannel, NotificationGateway, WebhookChannel};
pub use ring_buffer::RingBuffer;
pub use signal::{MarketData, Signal, SignalDirection, SignalEngine};
pub use strategy::Strategy;
pub use wallet_guard::{
    Order, OrderSide, RejectReason, RiskDecision, RiskParams, Wallet, WalletGuard,
};
