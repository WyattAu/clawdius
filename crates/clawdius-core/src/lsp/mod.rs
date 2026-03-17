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

pub mod client;
pub mod protocol;

pub use client::{LspClient, LspClientConfig, ServerCapabilities};
pub use protocol::{
    CodeAction, CompletionItem, CompletionItemKind, CompletionList, Diagnostic,
    DiagnosticRelatedInformation, DiagnosticSeverity, DocumentSymbol, Hover, HoverContents,
    Location, MarkedString, MarkupContent, MarkupKind, Position, Range, SymbolInformation,
    SymbolKind, TextDocumentIdentifier, TextDocumentPositionParams, TextEdit, WorkspaceEdit,
};
