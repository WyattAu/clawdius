// Unwrap purge batch 4: audit module — clean-root deny guard (zero production unwrap/expect in the batch-4 survey).
// Production code must not unwrap/expect; propagate, document a true invariant with an INVARIANT comment, or restructure.
// Lints inherit into all child modules and tests.
#![deny(clippy::unwrap_used, clippy::expect_used)]
// Test builds keep unwrap/expect for brevity (fleet convention, see lib.rs).
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod elasticsearch_backend;
pub mod events;
pub mod file_backend;
pub mod logger;
pub mod manager;
pub mod sqlite_backend;
pub mod syslog_backend;
pub mod webhook_backend;

pub use events::*;
pub use logger::*;
pub use manager::*;
