//! JSON-RPC protocol implementation
//!
//! Provides JSON-RPC 2.0 server and client for VSCode extension communication.

pub mod handlers;
pub mod methods;
pub mod server;
pub mod types;

pub use methods::Method;
pub use server::RpcServer;
pub use types::{Error as RpcError, Id, Request, Response};
