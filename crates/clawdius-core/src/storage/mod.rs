//! Storage abstraction layer
//!
//! Provides trait-based storage backends that decouple domain logic from
//! specific database implementations (SQLite, PostgreSQL, MariaDB, InMemory).
//!
//! # Architecture
//!
//! Three domain traits cover all storage operations:
//! - [`SessionRepository`] — session + message CRUD, token usage, search
//! - [`TimelineRepository`] — checkpoints, file tracking, rollback, diff
//! - [`GraphRepository`] — code graph, symbols, references, relationships
//!
//! Implementations:
//! - [`SqliteBackend`] — SQLite (default, local development)
//! - [`InMemoryBackend`] — HashMap-backed (testing, ephemeral)
//!
//! # Design Principles
//!
//! 1. **Domain-aligned traits** — each trait maps to one domain area
//! 2. **Async by default** — all operations are `async fn` for backend flexibility
//! 3. **`Send + Sync`** — all trait objects are thread-safe
//! 4. **Error agnostic** — traits return `crate::error::Result`, not backend-specific errors
//! 5. **Zero-copy where possible** — references over owned values in method signatures

mod backend;
mod error;
mod in_memory;
#[cfg(feature = "mariadb")]
mod mariadb;
#[cfg(feature = "postgres")]
mod postgres;
mod sqlite;

pub use backend::{
    GraphRepository, SessionRepository, StorageBackend, TimelineRepository,
};
pub use error::StorageError;
pub use in_memory::InMemoryBackend;
#[cfg(feature = "mariadb")]
pub use mariadb::MariaDbBackend;
#[cfg(feature = "postgres")]
pub use postgres::PostgresBackend;
pub use sqlite::SqliteBackend;
