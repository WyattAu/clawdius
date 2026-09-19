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
//! - [`PostgresBackend`] — PostgreSQL (feature: `postgres`)
//! - [`MariaDbBackend`] — MariaDB/MySQL (feature: `mariadb`)
//!
//! # Design Principles
//!
//! 1. **Domain-aligned traits** — each trait maps to one domain area
//! 2. **Async by default** — all operations are `async fn` for backend flexibility
//! 3. **`Send + Sync`** — all trait objects are thread-safe
//! 4. **Error agnostic** — traits return `crate::error::Result`, not backend-specific errors
//! 5. **Zero-copy where possible** — references over owned values in method signatures

// Unwrap purge batch 2: storage module — production code must not
// unwrap/expect; propagate, use poison-recovery for lock poisoning, or
// restructure. Lints inherit into all child modules (backend impls, tests).
#![deny(clippy::unwrap_used, clippy::expect_used)]
// Test builds keep unwrap/expect for brevity (fleet convention, see lib.rs).
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod backend;
mod error;
mod in_memory;
#[cfg(feature = "mariadb")]
mod mariadb;
#[cfg(feature = "postgres")]
mod postgres;
mod sqlite;
#[cfg(test)]
mod tests;

pub use backend::{
    GraphRepository, SessionRepository, StorageBackend, TimelineRepository, WorkspaceRepository,
};
pub use error::StorageError;
pub use in_memory::InMemoryBackend;
#[cfg(feature = "mariadb")]
pub use mariadb::MariaDbBackend;
#[cfg(feature = "postgres")]
pub use postgres::PostgresBackend;
pub use sqlite::SqliteBackend;
