//! Webhooks for event notifications
//!
//! Provides webhook support for external integrations and event-driven workflows.

// Unwrap purge batch 4: webhooks module — clean-root deny guard (zero production unwrap/expect in the batch-4 survey).
// Production code must not unwrap/expect; propagate, document a true invariant with an INVARIANT comment, or restructure.
// Lints inherit into all child modules and tests.
#![deny(clippy::unwrap_used, clippy::expect_used)]
// Test builds keep unwrap/expect for brevity (fleet convention, see lib.rs).
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod manager;
mod types;

pub use manager::*;
pub use types::*;
