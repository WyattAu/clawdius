//! JSON-RPC protocol implementation
//!
//! Provides JSON-RPC 2.0 server and client for `VSCode` extension communication.

// Unwrap purge batch 4: rpc module — clean-root deny guard (zero production unwrap/expect in the batch-4 survey).
// Production code must not unwrap/expect; propagate, document a true invariant with an INVARIANT comment, or restructure.
// Lints inherit into all child modules and tests.
#![deny(clippy::unwrap_used, clippy::expect_used)]
// Test builds keep unwrap/expect for brevity (fleet convention, see lib.rs).
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod handlers;
pub mod methods;
pub mod server;
pub mod types;

pub use methods::Method;
pub use server::RpcServer;
pub use types::{Error as RpcError, Id, Request, Response};
