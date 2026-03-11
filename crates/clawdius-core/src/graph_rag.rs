//! Graph-based Retrieval-Augmented Generation (RAG) for code understanding.
//!
//! This module provides semantic code search and retrieval using vector embeddings
//! and graph-based knowledge representation.
//!
//! # Features
//!
//! - **Multi-language support**: Parse and analyze code in Rust, Python, JavaScript, TypeScript, Go
//! - **Vector embeddings**: Generate embeddings for code semantic search
//! - **Graph storage**: Store code relationships in a graph database
//! - **Hybrid search**: Combine vector similarity with structural queries
//! - **AST parsing**: Deep code understanding via tree-sitter
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use clawdius_core::graph_rag::{GraphStore, VectorStore, HybridSearcher};
//! use std::path::Path;
//!
//! # fn main() -> clawdius_core::Result<()> {
//! // Initialize stores
//! let graph_store = GraphStore::new(Path::new(".clawdius/graph"))?;
//! let vector_store = VectorStore::new(Path::new(".clawdius/vectors.lance"))?;
//!
//! // Index a codebase
//! graph_store.index_directory(Path::new("src"))?;
//!
//! // Search for code
//! let searcher = HybridSearcher::new(graph_store, vector_store);
//! let results = searcher.search("function that handles authentication")?;
//!
//! for result in results {
//!     println!("{} (score: {:.2})", result.path, result.score);
//!     println!("  {}", result.snippet);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # Code Analysis
//!
//! Parse and analyze code structure:
//!
//! ```rust,no_run
//! use clawdius_core::graph_rag::parser::CodeParser;
//!
//! # fn main() -> clawdius_core::Result<()> {
//! let parser = CodeParser::new("rust")?;
//! let ast = parser.parse_file(Path::new("src/main.rs"))?;
//!
//! // Extract functions, structs, etc.
//! for node in ast.functions() {
//!     println!("Function: {} at line {}", node.name, node.line);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # Vector Search
//!
//! Perform semantic code search:
//!
//! ```rust,no_run
//! use clawdius_core::graph_rag::{VectorStore, SimpleEmbedder, EmbeddingGenerator};
//!
//! # fn main() -> clawdius_core::Result<()> {
//! let store = VectorStore::new(Path::new(".clawdius/vectors.lance"))?;
//! let embedder = SimpleEmbedder::new();
//!
//! // Generate embedding for query
//! let query_embedding = embedder.embed("error handling code")?;
//!
//! // Search for similar code
//! let results = store.search(&query_embedding, 10)?;
//!
//! for result in results {
//!     println!("{} (similarity: {:.2})", result.path, result.score);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # Hybrid Search
//!
//! Combine multiple search strategies:
//!
//! ```rust,no_run
//! use clawdius_core::graph_rag::{HybridSearcher, ResultSource};
//!
//! # fn main() -> clawdius_core::Result<()> {
//! // Assuming searcher is initialized
//! let searcher = HybridSearcher::new(/* ... */);
//!
//! let results = searcher.search("database connection handling")?;
//!
//! for result in results {
//!     match result.source {
//!         ResultSource::Vector => println!("Vector match: {}", result.path),
//!         ResultSource::Graph => println!("Graph match: {}", result.path),
//!         ResultSource::Hybrid => println!("Hybrid match: {}", result.path),
//!     }
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # Supported Languages
//!
//! - Rust (`rust`)
//! - Python (`python`)
//! - JavaScript (`javascript`)
//! - TypeScript (`typescript`)
//! - Go (`go`)
//!
//! # Configuration
//!
//! ```rust
//! use clawdius_core::graph_rag::GraphRagConfig;
//!
//! let config = GraphRagConfig {
//!     database_path: ".clawdius/graph/index.db".to_string(),
//!     vector_path: ".clawdius/graph/vectors.lance".to_string(),
//! };
//! ```

pub mod ast;
pub mod embedding;
pub mod languages;
pub mod parser;
pub mod store;

#[cfg(feature = "vector-db")]
pub mod search;

#[cfg(feature = "vector-db")]
pub mod vector;

use serde::{Deserialize, Serialize};

#[cfg(feature = "embeddings")]
pub use embedding::SentenceEmbedder;
pub use embedding::{EmbeddingGenerator, SimpleEmbedder};

#[cfg(feature = "vector-db")]
pub use search::{HybridResult, HybridSearcher, ResultSource};

pub use store::GraphStore;

#[cfg(feature = "vector-db")]
pub use vector::{SearchResult, VectorEntry, VectorStore};

/// Graph-RAG configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphRagConfig {
    /// Path to SQLite database
    pub database_path: String,
    /// Path to vector store
    pub vector_path: String,
}
