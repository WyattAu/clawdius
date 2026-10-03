//! Language Server Protocol (LSP) Implementation
//!
//! Provides LSP client functionality for code intelligence features:
//! - Code completion
//! - Go to definition
//! - Find references
//! - Diagnostics
//! - Hover information
//! - Document symbols
//!
//! # Example
//!
//! ```rust,ignore
//! use clawdius_core::lsp::{LspClient, LspClientConfig};
//!
//! let config = LspClientConfig::new("rust-analyzer");
//! let mut client = LspClient::new(config);
//! client.start(Some("file:///project")).await?;
//! let completions = client.completion("file:///src/main.rs", position).await?;
//! ```

// Unwrap purge batch 4: lsp module — clean-root deny guard (zero production unwrap/expect in the batch-4 survey).
// Production code must not unwrap/expect; propagate, document a true invariant with an INVARIANT comment, or restructure.
// Lints inherit into all child modules and tests.
#![deny(clippy::unwrap_used, clippy::expect_used)]
// Test builds keep unwrap/expect for brevity (fleet convention, see lib.rs).
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod client;
pub mod protocol;

pub use client::{LspClient, LspClientConfig, ServerCapabilities};
pub use protocol::{
    CodeAction, CompletionItem, CompletionItemKind, CompletionList, Diagnostic,
    DiagnosticRelatedInformation, DiagnosticSeverity, DocumentSymbol, Hover, HoverContents,
    Location, MarkedString, MarkupContent, MarkupKind, Position, Range, SymbolInformation,
    SymbolKind, TextDocumentIdentifier, TextDocumentPositionParams, TextEdit, WorkspaceEdit,
};
