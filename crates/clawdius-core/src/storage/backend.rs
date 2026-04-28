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
pub trait SessionRepository: Send + Sync + std::fmt::Debug {
    // ── Session CRUD ──

    /// Create a new session.
    fn create_session(&self, session: &Session) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Load a session by ID (metadata only, no messages).
    fn load_session(&self, id: &SessionId) -> impl std::future::Future<Output = Result<Option<Session>>> + Send;

    /// Load a session with full message history.
    fn load_session_full(&self, id: &SessionId) -> impl std::future::Future<Output = Result<Option<Session>>> + Send;

    /// List all sessions, ordered by most recently updated.
    fn list_sessions(&self) -> impl std::future::Future<Output = Result<Vec<Session>>> + Send;

    /// Delete a session and all associated messages.
    fn delete_session(&self, id: &SessionId) -> impl std::future::Future<Output = Result<()>> + Send;

    // ── Message operations ──

    /// Append a message to a session.
    fn save_message(&self, session_id: &SessionId, message: &Message) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Search messages across all sessions (full-text search).
    fn search_messages(&self, query: &str) -> impl std::future::Future<Output = Result<Vec<(SessionId, Message)>>> + Send;

    // ── Token usage ──

    /// Update token usage counters for a session.
    fn update_token_usage(&self, id: &SessionId, usage: &TokenUsage) -> impl std::future::Future<Output = Result<()>> + Send;
}

// ─────────────────────────────────────────────────────────
// Timeline / Checkpoint operations
// ─────────────────────────────────────────────────────────

/// Repository for workspace snapshots, checkpoints, and rollback.
///
/// Covers the domain previously split between `TimelineStore` and
/// `CheckpointManager`, unified under a single async trait.
pub trait TimelineRepository: Send + Sync + std::fmt::Debug {
    // ── File tracking ──

    /// Register a file for change tracking.
    fn track_file(&self, path: &Path) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Get the number of tracked files.
    fn tracked_file_count(&self) -> impl std::future::Future<Output = Result<usize>> + Send;

    // ── Checkpoint CRUD ──

    /// Create a named checkpoint (snapshots all tracked files).
    fn create_checkpoint(
        &self,
        name: &str,
        description: Option<&str>,
    ) -> impl std::future::Future<Output = Result<CheckpointId>> + Send;

    /// List all checkpoints, ordered by timestamp descending.
    fn list_checkpoints(&self) -> impl std::future::Future<Output = Result<Vec<CheckpointInfo>>> + Send;

    /// Get a single checkpoint's metadata.
    fn get_checkpoint(&self, id: &CheckpointId) -> impl std::future::Future<Output = Result<Option<CheckpointInfo>>> + Send;

    /// Delete a checkpoint and its file snapshots.
    fn delete_checkpoint(&self, id: &CheckpointId) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Get the total number of checkpoints.
    fn checkpoint_count(&self) -> impl std::future::Future<Output = Result<usize>> + Send;

    // ── File history ──

    /// Get version history for a specific file.
    fn get_file_history(&self, path: &Path) -> impl std::future::Future<Output = Result<Vec<FileVersion>>> + Send;

    /// Get a file's version at a specific checkpoint.
    fn get_file_version_at_checkpoint(
        &self,
        path: &Path,
        checkpoint_id: &CheckpointId,
    ) -> impl std::future::Future<Output = Result<Option<FileVersion>>> + Send;

    /// Get files that changed between two checkpoints.
    fn get_files_changed_between(
        &self,
        from: &CheckpointId,
        to: &CheckpointId,
    ) -> impl std::future::Future<Output = Result<Vec<(PathBuf, FileChangeType)>>> + Send;

    // ── Diff ──

    /// Compute a diff between two checkpoints.
    fn diff_checkpoints(&self, from: &CheckpointId, to: &CheckpointId) -> impl std::future::Future<Output = Result<Diff>> + Send;

    // ── Rollback ──

    /// Rollback the workspace to a checkpoint state.
    fn rollback(&self, checkpoint_id: &CheckpointId) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Rollback specific files to a checkpoint state.
    fn rollback_files(&self, checkpoint_id: &CheckpointId, files: &[PathBuf]) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Preview what a rollback would do (dry-run).
    fn preview_rollback(&self, checkpoint_id: &CheckpointId) -> impl std::future::Future<Output = Result<RollbackPreview>> + Send;

    // ── Queries ──

    /// Query checkpoints by time range.
    fn query_by_time_range(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> impl std::future::Future<Output = Result<Vec<CheckpointInfo>>> + Send;

    /// Query checkpoints by name pattern (substring match).
    fn query_by_name(&self, pattern: &str) -> impl std::future::Future<Output = Result<Vec<CheckpointInfo>>> + Send;

    // ── Import / Export ──

    /// Export a checkpoint to a portable format.
    fn export_checkpoint(&self, checkpoint_id: &CheckpointId) -> impl std::future::Future<Output = Result<ExportedCheckpoint>> + Send;

    /// Import a checkpoint from a portable format.
    fn import_checkpoint(&self, exported: ExportedCheckpoint) -> impl std::future::Future<Output = Result<CheckpointId>> + Send;

    // ── Maintenance ──

    /// Delete old checkpoints, keeping the most recent `keep_count`.
    fn cleanup_old_checkpoints(&self, keep_count: usize) -> impl std::future::Future<Output = Result<usize>> + Send;

    /// Clean up orphaned snapshot files on disk.
    fn cleanup_snapshots(&self) -> impl std::future::Future<Output = Result<usize>> + Send;

    /// Get storage statistics.
    fn storage_stats(&self) -> impl std::future::Future<Output = Result<StorageStats>> + Send;
}

// ─────────────────────────────────────────────────────────
// Graph / Code knowledge operations
// ─────────────────────────────────────────────────────────

/// Repository for code knowledge graph (symbols, references, relationships).
///
/// Covers the domain previously split between `GraphStore` (graph_rag)
/// and `AstStore` (AST index), unified under a single async trait.
pub trait GraphRepository: Send + Sync + std::fmt::Debug {
    // ── File operations ──

    /// Insert or update a file record.
    fn insert_file(&self, file: &FileInfo) -> impl std::future::Future<Output = Result<i64>> + Send;

    /// Look up a file by its path.
    fn get_file_by_path(&self, path: &str) -> impl std::future::Future<Output = Result<Option<FileInfo>>> + Send;

    /// Look up a file by its database ID.
    fn get_file_by_id(&self, id: i64) -> impl std::future::Future<Output = Result<Option<FileInfo>>> + Send;

    /// Get a file's database ID by path.
    fn get_file_id(&self, path: &str) -> impl std::future::Future<Output = Result<Option<i64>>> + Send;

    /// Delete a file and all associated symbols/refs.
    fn delete_file(&self, path: &str) -> impl std::future::Future<Output = Result<bool>> + Send;

    /// Count total indexed files.
    fn count_files(&self) -> impl std::future::Future<Output = Result<i64>> + Send;

    // ── Symbol operations ──

    /// Insert a symbol (function, struct, enum, etc.).
    fn insert_symbol(&self, symbol: &Symbol) -> impl std::future::Future<Output = Result<i64>> + Send;

    /// Find symbols by exact name match.
    fn find_symbol(&self, name: &str) -> impl std::future::Future<Output = Result<Vec<Symbol>>> + Send;

    /// Find a single symbol by database ID.
    fn find_symbol_by_id(&self, id: i64) -> impl std::future::Future<Output = Result<Option<Symbol>>> + Send;

    /// Find symbols by kind (Function, Struct, Enum, etc.).
    fn find_symbols_by_kind(&self, kind: &SymbolKind) -> impl std::future::Future<Output = Result<Vec<Symbol>>> + Send;

    /// Find all symbols in a file.
    fn find_symbols_in_file(&self, file_id: i64) -> impl std::future::Future<Output = Result<Vec<Symbol>>> + Send;

    /// Full-text search for symbols.
    fn search_symbols(&self, query: &str) -> impl std::future::Future<Output = Result<Vec<Symbol>>> + Send;

    /// Count total indexed symbols.
    fn count_symbols(&self) -> impl std::future::Future<Output = Result<i64>> + Send;

    /// Delete all symbols belonging to a file.
    fn delete_symbols_for_file(&self, file_id: i64) -> impl std::future::Future<Output = Result<()>> + Send;

    // ── Reference operations ──

    /// Insert a symbol reference (usage site).
    fn insert_reference(&self, reference: &Reference) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Find all references to a symbol.
    fn find_symbol_refs(&self, symbol_id: i64) -> impl std::future::Future<Output = Result<Vec<Reference>>> + Send;

    /// Count total symbol references.
    fn count_symbol_refs(&self) -> impl std::future::Future<Output = Result<i64>> + Send;

    /// Delete all references belonging to a file.
    fn delete_symbol_refs_for_file(&self, file_id: i64) -> impl std::future::Future<Output = Result<()>> + Send;

    // ── Relationship operations ──

    /// Insert a relationship between two symbols.
    fn insert_relationship(&self, relationship: &Relationship) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Find all relationships involving a symbol (any direction).
    fn find_relationships(&self, symbol_id: i64) -> impl std::future::Future<Output = Result<Vec<Relationship>>> + Send;

    /// Find outgoing relationships from a symbol.
    fn find_outgoing_relationships(&self, symbol_id: i64) -> impl std::future::Future<Output = Result<Vec<Relationship>>> + Send;

    /// Find incoming relationships to a symbol.
    fn find_incoming_relationships(&self, symbol_id: i64) -> impl std::future::Future<Output = Result<Vec<Relationship>>> + Send;

    /// Count total relationships.
    fn count_relationships(&self) -> impl std::future::Future<Output = Result<i64>> + Send;

    // ── Bulk operations ──

    /// Clear all data (files, symbols, refs, relationships).
    fn clear(&self) -> impl std::future::Future<Output = Result<()>> + Send;
}

// ─────────────────────────────────────────────────────────
// Workspace operations (multi-repo support)
// ─────────────────────────────────────────────────────────

use crate::workspace::{Project, ProjectId, Workspace, WorkspaceId};

/// Repository for workspace and project management.
///
/// Supports multi-repo workspaces where a single agent operates
/// across multiple codebases simultaneously.
pub trait WorkspaceRepository: Send + Sync + std::fmt::Debug {
    // ── Workspace CRUD ──

    /// Create a new workspace.
    fn create_workspace(
        &self,
        workspace: &Workspace,
    ) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Load a workspace by ID.
    fn load_workspace(
        &self,
        id: &WorkspaceId,
    ) -> impl std::future::Future<Output = Result<Option<Workspace>>> + Send;

    /// List all workspaces.
    fn list_workspaces(&self) -> impl std::future::Future<Output = Result<Vec<Workspace>>> + Send;

    /// Delete a workspace and all project associations.
    fn delete_workspace(&self, id: &WorkspaceId) -> impl std::future::Future<Output = Result<()>> + Send;

    // ── Project CRUD ──

    /// Add a project to the workspace.
    fn add_project(&self, project: &Project) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Load a project by ID.
    fn load_project(
        &self,
        id: &ProjectId,
    ) -> impl std::future::Future<Output = Result<Option<Project>>> + Send;

    /// Look up a project by its root path.
    fn load_project_by_path(
        &self,
        path: &Path,
    ) -> impl std::future::Future<Output = Result<Option<Project>>> + Send;

    /// List all projects.
    fn list_projects(&self) -> impl std::future::Future<Output = Result<Vec<Project>>> + Send;

    /// Update a project's metadata.
    fn update_project(&self, project: &Project) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Remove a project from all workspaces.
    fn remove_project(&self, id: &ProjectId) -> impl std::future::Future<Output = Result<()>> + Send;

    // ── Workspace ↔ Project association ──

    /// Add a project to a workspace.
    fn add_project_to_workspace(
        &self,
        workspace_id: &WorkspaceId,
        project_id: &ProjectId,
    ) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Remove a project from a workspace.
    fn remove_project_from_workspace(
        &self,
        workspace_id: &WorkspaceId,
        project_id: &ProjectId,
    ) -> impl std::future::Future<Output = Result<()>> + Send;

    /// List all projects in a workspace.
    fn list_workspace_projects(
        &self,
        workspace_id: &WorkspaceId,
    ) -> impl std::future::Future<Output = Result<Vec<Project>>> + Send;

    // ── Default project ──

    /// Set the default project for a workspace.
    fn set_default_project(
        &self,
        workspace_id: &WorkspaceId,
        project_id: &ProjectId,
    ) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Get the default project for a workspace.
    fn get_default_project(
        &self,
        workspace_id: &WorkspaceId,
    ) -> impl std::future::Future<Output = Result<Option<Project>>> + Send;
}

// ─────────────────────────────────────────────────────────
// Unified backend (combines all domain traits)
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
    SessionRepository + TimelineRepository + GraphRepository + WorkspaceRepository + Send + Sync + std::fmt::Debug
{
    /// Get the backend type name (e.g., "sqlite", "postgres", "in_memory").
    fn backend_type(&self) -> &'static str;

    /// Run database migrations to the latest schema version.
    fn migrate(&self) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Check if the backend is healthy (connection alive, schema present).
    fn health_check(&self) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Close the backend and release resources.
    fn close(&self) -> impl std::future::Future<Output = Result<()>> + Send;
}
