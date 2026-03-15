//! Webhooks for event notifications
//!
//! Provides webhook support for external integrations and event-driven workflows.

mod manager;
mod types;

pub use manager::*;
pub use types::*;
