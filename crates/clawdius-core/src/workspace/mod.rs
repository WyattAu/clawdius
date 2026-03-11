//! Workspace indexing and multi-file context management
//!
//! Provides intelligent code indexing across entire workspaces with:
//! - File watching and automatic re-indexing
//! - Multi-language support via tree-sitter
//! - Vector embeddings for semantic search (requires `vector-db` feature)
//! - Graph-based relationship tracking

pub mod aggregator;

#[cfg(feature = "vector-db")]
pub mod indexer;

pub use aggregator::{AggregatedContext, ContextAggregator};

#[cfg(feature = "vector-db")]
pub use indexer::{IndexStats, WorkspaceIndexer};
