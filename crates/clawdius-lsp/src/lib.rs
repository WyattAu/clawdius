//! Clawdius LSP Server
//!
//! Language Server Protocol integration for Clawdius, exposing Tree-sitter
//! symbol extraction and Graph-RAG code intelligence to IDE clients.
//!
//! # Protocol Support
//!
//! - `textDocument/documentSymbol` -- Symbol extraction via Tree-sitter (10 languages)
//! - `textDocument/hover` -- Documentation lookup via Graph-RAG
//! - `textDocument/definition` -- Go-to-definition via symbol index
//! - `textDocument/references` -- Find-all-references via symbol index
//! - `textDocument/diagnostic` -- Architecture drift and debt detection

#![deny(unsafe_code)]
#![warn(clippy::pedantic, clippy::unwrap_used, clippy::expect_used)]

use anyhow::Result;
use tower_lsp::{LspService, Server};
use tracing_subscriber::EnvFilter;

mod backend;
mod capabilities;
mod handlers;
mod symbol_index;

pub use backend::ClawdiusLspBackend;

/// Run the LSP server over stdio.
///
/// # Errors
/// Returns an error if the tracing filter cannot be parsed or the LSP service fails to start.
pub async fn run_stdio() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("clawdius_lsp=info".parse()?))
        .init();

    let (service, socket) = LspService::build(ClawdiusLspBackend::new)
        .custom_method("clawdius/analyze", ClawdiusLspBackend::analyze)
        .custom_method("clawdius/verify", ClawdiusLspBackend::verify)
        .finish();

    Server::new(tokio::io::stdin(), tokio::io::stdout(), socket)
        .serve(service)
        .await;

    Ok(())
}
