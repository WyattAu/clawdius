//! Core storage traits
//!
//! Three domain-aligned traits that define the complete storage API surface.
//! Each trait maps to one domain area and is implemented by concrete backends.

use crate::checkpoint::FileSnapshot;
use crate::error::Result;
use crate::graph_rag::ast::{
    FileInfo, Reference, Relationship, Symbol, SymbolKind,
};
use crate::session::types::{Message, Session, SessionId, TokenUsage};
use crate::timeline::{
    CheckpointId, CheckpointInfo, Diff, ExportedCheckpoint, FileChangeType,
    FileVersion, RollbackPreview, StorageStats,
};
use chrono::{DateTime, Utc};
use std::path::{Path, PathBuf};

// ─────────────────────────────────────────────────────────
// Session operations (sessions + messages + token usage)
// ─────────────────────────────────────────────────────────

/// Repository for session and message persistence.
///
/// This trait supersedes the legacy `session::repository::SessionRepository`
/// by adding async support and returning `crate::error::Result` consistently.
/// The legacy trait is preserved for backward compatibility and will be
/// deprecated once all consumers migrate.
#[allow(async_fn_in_trait)]
pub trait SessionRepository: Send + Sync + std::fmt::Debug {
    // ── Session CRUD ──

    /// Create a new session.
    async fn create_session(&self, session: &Session) -> Result<()>;

    /// Load a session by ID (metadata only, no messages).
    async fn load_session(&self, id: &SessionId) -> Result<Option<Session>>;

    /// Load a session with full message history.
    async fn load_session_full(&self, id: &SessionId) -> Result<Option<Session>>;

    /// List all sessions, ordered by most recently updated.
    async fn list_sessions(&self) -> Result<Vec<Session>>;

    /// Delete a session and all associated messages.
    async fn delete_session(&self, id: &SessionId) -> Result<()>;

    // ── Message operations ──

    /// Append a message to a session.
    async fn save_message(&self, session_id: &SessionId, message: &Message) -> Result<()>;

    /// Search messages across all sessions (full-text search).
    async fn search_messages(&self, query: &str) -> Result<Vec<(SessionId, Message)>>;

    // ── Token usage ──

    /// Update token usage counters for a session.
    async fn update_token_usage(&self, id: &SessionId, usage: &TokenUsage) -> Result<()>;
}

// ─────────────────────────────────────────────────────────
// Timeline / Checkpoint operations
// ─────────────────────────────────────────────────────────

/// Repository for workspace snapshots, checkpoints, and rollback.
///
/// Covers the domain previously split between `TimelineStore` and
/// `CheckpointManager`, unified under a single async trait.
#[allow(async_fn_in_trait)]
pub trait TimelineRepository: Send + Sync + std::fmt::Debug {
    // ── File tracking ──

    /// Register a file for change tracking.
    async fn track_file(&self, path: &Path) -> Result<()>;

    /// Get the number of tracked files.
    async fn tracked_file_count(&self) -> Result<usize>;

    // ── Checkpoint CRUD ──

    /// Create a named checkpoint (snapshots all tracked files).
    async fn create_checkpoint(
        &self,
        name: &str,
        description: Option<&str>,
    ) -> Result<CheckpointId>;

    /// List all checkpoints, ordered by timestamp descending.
    async fn list_checkpoints(&self) -> Result<Vec<CheckpointInfo>>;

    /// Get a single checkpoint's metadata.
    async fn get_checkpoint(&self, id: &CheckpointId) -> Result<Option<CheckpointInfo>>;

    /// Delete a checkpoint and its file snapshots.
    async fn delete_checkpoint(&self, id: &CheckpointId) -> Result<()>;

    /// Get the total number of checkpoints.
    async fn checkpoint_count(&self) -> Result<usize>;

    // ── File history ──

    /// Get version history for a specific file.
    async fn get_file_history(&self, path: &Path) -> Result<Vec<FileVersion>>;

    /// Get a file's version at a specific checkpoint.
    async fn get_file_version_at_checkpoint(
        &self,
        path: &Path,
        checkpoint_id: &CheckpointId,
    ) -> Result<Option<FileVersion>>;

    /// Get files that changed between two checkpoints.
    async fn get_files_changed_between(
        &self,
        from: &CheckpointId,
        to: &CheckpointId,
    ) -> Result<Vec<(PathBuf, FileChangeType)>>;

    // ── Diff ──

    /// Compute a diff between two checkpoints.
    async fn diff_checkpoints(&self, from: &CheckpointId, to: &CheckpointId) -> Result<Diff>;

    // ── Rollback ──

    /// Rollback the workspace to a checkpoint state.
    async fn rollback(&self, checkpoint_id: &CheckpointId) -> Result<()>;

    /// Rollback specific files to a checkpoint state.
    async fn rollback_files(&self, checkpoint_id: &CheckpointId, files: &[PathBuf]) -> Result<()>;

    /// Preview what a rollback would do (dry-run).
    async fn preview_rollback(&self, checkpoint_id: &CheckpointId) -> Result<RollbackPreview>;

    // ── Queries ──

    /// Query checkpoints by time range.
    async fn query_by_time_range(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<CheckpointInfo>>;

    /// Query checkpoints by name pattern (substring match).
    async fn query_by_name(&self, pattern: &str) -> Result<Vec<CheckpointInfo>>;

    // ── Import / Export ──

    /// Export a checkpoint to a portable format.
    async fn export_checkpoint(&self, checkpoint_id: &CheckpointId) -> Result<ExportedCheckpoint>;

    /// Import a checkpoint from a portable format.
    async fn import_checkpoint(&self, exported: ExportedCheckpoint) -> Result<CheckpointId>;

    // ── Maintenance ──

    /// Delete old checkpoints, keeping the most recent `keep_count`.
    async fn cleanup_old_checkpoints(&self, keep_count: usize) -> Result<usize>;

    /// Clean up orphaned snapshot files on disk.
    async fn cleanup_snapshots(&self) -> Result<usize>;

    /// Get storage statistics.
    async fn storage_stats(&self) -> Result<StorageStats>;
}

// ─────────────────────────────────────────────────────────
// Graph / Code knowledge operations
// ─────────────────────────────────────────────────────────

/// Repository for code knowledge graph (symbols, references, relationships).
///
/// Covers the domain previously split between `GraphStore` (graph_rag)
/// and `AstStore` (AST index), unified under a single async trait.
#[allow(async_fn_in_trait)]
pub trait GraphRepository: Send + Sync + std::fmt::Debug {
    // ── File operations ──

    /// Insert or update a file record.
    async fn insert_file(&self, file: &FileInfo) -> Result<i64>;

    /// Look up a file by its path.
    async fn get_file_by_path(&self, path: &str) -> Result<Option<FileInfo>>;

    /// Look up a file by its database ID.
    async fn get_file_by_id(&self, id: i64) -> Result<Option<FileInfo>>;

    /// Get a file's database ID by path.
    async fn get_file_id(&self, path: &str) -> Result<Option<i64>>;

    /// Delete a file and all associated symbols/refs.
    async fn delete_file(&self, path: &str) -> Result<bool>;

    /// Count total indexed files.
    async fn count_files(&self) -> Result<i64>;

    // ── Symbol operations ──

    /// Insert a symbol (function, struct, enum, etc.).
    async fn insert_symbol(&self, symbol: &Symbol) -> Result<i64>;

    /// Find symbols by exact name match.
    async fn find_symbol(&self, name: &str) -> Result<Vec<Symbol>>;

    /// Find a single symbol by database ID.
    async fn find_symbol_by_id(&self, id: i64) -> Result<Option<Symbol>>;

    /// Find symbols by kind (Function, Struct, Enum, etc.).
    async fn find_symbols_by_kind(&self, kind: &SymbolKind) -> Result<Vec<Symbol>>;

    /// Find all symbols in a file.
    async fn find_symbols_in_file(&self, file_id: i64) -> Result<Vec<Symbol>>;

    /// Full-text search for symbols.
    async fn search_symbols(&self, query: &str) -> Result<Vec<Symbol>>;

    /// Count total indexed symbols.
    async fn count_symbols(&self) -> Result<i64>;

    /// Delete all symbols belonging to a file.
    async fn delete_symbols_for_file(&self, file_id: i64) -> Result<()>;

    // ── Reference operations ──

    /// Insert a symbol reference (usage site).
    async fn insert_reference(&self, reference: &Reference) -> Result<()>;

    /// Find all references to a symbol.
    async fn find_symbol_refs(&self, symbol_id: i64) -> Result<Vec<Reference>>;

    /// Count total symbol references.
    async fn count_symbol_refs(&self) -> Result<i64>;

    /// Delete all references belonging to a file.
    async fn delete_symbol_refs_for_file(&self, file_id: i64) -> Result<()>;

    // ── Relationship operations ──

    /// Insert a relationship between two symbols.
    async fn insert_relationship(&self, relationship: &Relationship) -> Result<()>;

    /// Find all relationships involving a symbol (any direction).
    async fn find_relationships(&self, symbol_id: i64) -> Result<Vec<Relationship>>;

    /// Find outgoing relationships from a symbol.
    async fn find_outgoing_relationships(&self, symbol_id: i64) -> Result<Vec<Relationship>>;

    /// Find incoming relationships to a symbol.
    async fn find_incoming_relationships(&self, symbol_id: i64) -> Result<Vec<Relationship>>;

    /// Count total relationships.
    async fn count_relationships(&self) -> Result<i64>;

    // ── Bulk operations ──

    /// Clear all data (files, symbols, refs, relationships).
    async fn clear(&self) -> Result<()>;
}

// ─────────────────────────────────────────────────────────
// Unified backend (combines all three domain traits)
// ─────────────────────────────────────────────────────────

/// A unified storage backend that implements all domain traits.
///
/// Use this when you need access to sessions, timeline, and graph
/// storage through a single object. Each method delegates to the
/// appropriate domain trait implementation.
///
/// # Example
///
/// ```ignore
/// use clawdius_core::storage::StorageBackend;
/// use std::sync::Arc;
///
/// async fn example(backend: Arc<dyn StorageBackend>) {
///     // Session operations
///     let sessions = backend.list_sessions().await.unwrap();
///
///     // Timeline operations
///     let checkpoints = backend.list_checkpoints().await.unwrap();
///
///     // Graph operations
///     let symbols = backend.search_symbols("parse").await.unwrap();
/// }
/// ```
pub trait StorageBackend:
    SessionRepository + TimelineRepository + GraphRepository + Send + Sync + std::fmt::Debug
{
    /// Get the backend type name (e.g., "sqlite", "postgres", "in_memory").
    fn backend_type(&self) -> &'static str;

    /// Run database migrations to the latest schema version.
    async fn migrate(&self) -> Result<()>;

    /// Check if the backend is healthy (connection alive, schema present).
    async fn health_check(&self) -> Result<()>;

    /// Close the backend and release resources.
    async fn close(&self) -> Result<()>;
}
