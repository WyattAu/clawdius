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

#![allow(clippy::significant_drop_tightening)]
#![allow(clippy::type_complexity)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::trivially_copy_pass_by_ref)]
#![allow(clippy::redundant_closure_for_method_calls)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::double_ended_iterator_last)]
#![allow(clippy::assigning_clones)]
#![allow(clippy::or_fun_call)]
#![allow(clippy::no_effect_underscore_binding)]
#![allow(clippy::option_if_let_else)]
#![allow(clippy::manual_let_else)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::missing_const_for_fn)]
#![allow(clippy::unused_self)]
#![allow(clippy::trait_duplication_in_bounds)]
#![allow(clippy::expect_used)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::panic, dead_code))]

pub mod adapter;
pub mod adapters;
pub mod admin;
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
