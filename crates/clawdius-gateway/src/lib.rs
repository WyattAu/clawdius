//! Clawdius Gateway — messaging platform abstraction layer.
//!
//! Routes messages from chat platforms (Telegram, Discord, Slack, Matrix, etc.)
//! to the Clawdius agent engine, and streams responses back to the user.
//!
//! # Architecture
//!
//! ```text
//! Platform (Telegram/Discord/...)
//!   → IncomingMessage
//!   → PlatformAdapter (platform-specific)
//!   → MessageGateway (routing + rate limiting + auth)
//!   → Clawdius agent engine
//!   → ResponseStream
//!   → PlatformAdapter (format + send)
//!   → Platform
//! ```

pub mod adapter;
pub mod admin;
pub mod adapters;
pub mod error;
pub mod formatter;
pub mod gateway;
pub mod handler;
pub mod rate_limit;

pub use adapter::{IncomingMessage, OutgoingMessage, PlatformAdapter, PlatformConfig};
pub use error::GatewayError;
pub use formatter::ResponseFormatter;
pub use gateway::MessageGateway;
pub use handler::ClawdiusHandler;
pub use rate_limit::RateLimiter;
