//! File timeline system for tracking changes and enabling rollback
//!
//! This module provides comprehensive file change tracking with:
//! - File watching and automatic change detection
//! - Named checkpoints with full file snapshots
//! - File history tracking
//! - Diff between checkpoints
//! - Rollback capabilities
//!
//! # Example
//!
//! ```rust,no_run
//! use clawdius_core::timeline::TimelineManager;
//! use std::path::PathBuf;
//!
//! # async fn example() -> clawdius_core::Result<()> {
//! let manager = TimelineManager::new(
//!     &PathBuf::from(".clawdius/timeline.db"),
//!     PathBuf::from("."),
//! )?;
//!
//! // Create a checkpoint
//! let checkpoint_id = manager.create_checkpoint("before-refactor").await?;
//!
//! // List checkpoints
//! let checkpoints = manager.list_checkpoints().await?;
//!
//! // Get file history
//! let history = manager.get_file_history(&PathBuf::from("src/main.rs")).await?;
//!
//! // Rollback to a checkpoint
//! manager.rollback(&checkpoint_id).await?;
//! # Ok(())
//! # }
//! ```

mod change_tracker;
mod store;
mod watcher;

pub use change_tracker::{
    ChangeId, ChangeSource, ChangeTracker, ChangeType, DiffCalculator, FileChangeRecord,
    FileChangeSummary, FileState, LineChange, LineChangeType,
};
pub use store::{CheckpointInfo, FileVersion, StorageStats, TimelineCheckpoint, TimelineStore};
pub use watcher::{ChangeKind, FileChangeEvent, FileWatcher, WatcherConfig};

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::error::Result;

/// Unique checkpoint identifier
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct CheckpointId(pub String);

impl CheckpointId {
    /// Create a new checkpoint ID
    #[must_use]
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    /// Create from string
    #[must_use]
    pub fn from_string(s: String) -> Self {
        Self(s)
    }
}

impl Default for CheckpointId {
    fn default() -> Self {
        Self::new()
    }
}

/// Diff between two checkpoints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diff {
    /// From checkpoint ID
    pub from: CheckpointId,
    /// To checkpoint ID
    pub to: CheckpointId,
    /// Files changed
    pub files_changed: Vec<FileDiff>,
    /// Summary
    pub summary: DiffSummary,
}

/// A diff for a single file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDiff {
    /// File path
    pub path: PathBuf,
    /// Change type
    pub change_type: FileChangeType,
    /// Number of additions
    pub additions: usize,
    /// Number of deletions
    pub deletions: usize,
}

/// Type of file change
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FileChangeType {
    /// File was added
    Added,
    /// File was modified
    Modified,
    /// File was deleted
    Deleted,
}

/// Summary of diff
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffSummary {
    /// Total files changed
    pub total_files: usize,
    /// Total additions
    pub total_additions: usize,
    /// Total deletions
    pub total_deletions: usize,
}

/// Preview of a rollback operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackPreview {
    /// Checkpoint ID
    pub checkpoint_id: CheckpointId,
    /// Files that will be restored
    pub files_to_restore: Vec<PathBuf>,
    /// Files that will be deleted (not in checkpoint)
    pub files_to_delete: Vec<PathBuf>,
    /// Files that will be modified
    pub files_modified: Vec<PathBuf>,
    /// Total files affected
    pub total_files_affected: usize,
}

/// Timeline manager for tracking file changes and checkpoints
pub struct TimelineManager {
    store: TimelineStore,
    #[allow(dead_code)]
    watcher: Option<Arc<FileWatcher>>,
    workspace_root: PathBuf,
}

impl TimelineManager {
    /// Create a new timeline manager
    pub fn new(db_path: &Path, workspace_root: PathBuf) -> Result<Self> {
        let store = TimelineStore::new(db_path, workspace_root.clone())?;

        Ok(Self {
            store,
            watcher: None,
            workspace_root,
        })
    }

    /// Create timeline manager with file watching enabled
    pub fn with_watcher(
        db_path: &Path,
        workspace_root: PathBuf,
        config: WatcherConfig,
    ) -> Result<Self> {
        let store = TimelineStore::new(db_path, workspace_root.clone())?;
        let watcher = FileWatcher::new(workspace_root.clone(), config);

        Ok(Self {
            store,
            watcher: Some(Arc::new(watcher)),
            workspace_root,
        })
    }

    /// Create file watcher for this timeline with custom config
    pub fn create_watcher(&self, config: WatcherConfig) -> FileWatcher {
        FileWatcher::new(self.workspace_root.clone(), config)
    }

    /// Track a file for changes
    pub fn track_file(&self, path: &Path) -> Result<()> {
        self.store.track_file(path)
    }

    /// Create a named checkpoint
    pub async fn create_checkpoint(&mut self, name: &str) -> Result<CheckpointId> {
        self.store.create_checkpoint(name, None).await
    }

    /// Create a checkpoint with description
    pub async fn create_checkpoint_with_description(
        &mut self,
        name: &str,
        description: &str,
    ) -> Result<CheckpointId> {
        self.store.create_checkpoint(name, Some(description)).await
    }

    /// List all checkpoints
    pub fn list_checkpoints(&self) -> Result<Vec<CheckpointInfo>> {
        self.store.list_checkpoints()
    }

    /// Get checkpoint info
    pub fn get_checkpoint(&self, id: &CheckpointId) -> Result<Option<CheckpointInfo>> {
        self.store.get_checkpoint(id)
    }

    /// Rollback to a checkpoint
    pub async fn rollback(&self, checkpoint_id: &CheckpointId) -> Result<()> {
        self.store.rollback(checkpoint_id).await
    }

    /// Diff between two checkpoints
    pub fn diff(&self, from: &CheckpointId, to: &CheckpointId) -> Result<Diff> {
        self.store.diff_checkpoints(from, to)
    }

    /// Get file history
    pub fn get_file_history(&self, path: &Path) -> Result<Vec<FileVersion>> {
        self.store.get_file_history(path)
    }

    /// Delete a checkpoint
    pub fn delete_checkpoint(&mut self, checkpoint_id: &CheckpointId) -> Result<()> {
        self.store.delete_checkpoint(checkpoint_id)
    }

    /// Cleanup old checkpoints
    pub fn cleanup_old_checkpoints(&mut self, keep_count: usize) -> Result<usize> {
        self.store.cleanup_old_checkpoints(keep_count)
    }

    /// Start watching files (if watcher is enabled)
    pub fn start_watching(&self) -> Result<()> {
        // File watching is handled by the FileWatcher independently
        // This is a placeholder for future integration
        Ok(())
    }

    /// Stop watching files
    pub fn stop_watching(&self) -> Result<()> {
        // File watching is handled by the FileWatcher independently
        // This is a placeholder for future integration
        Ok(())
    }

    /// Query checkpoints by time range
    pub fn query_by_time_range(
        &self,
        start: chrono::DateTime<chrono::Utc>,
        end: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<CheckpointInfo>> {
        self.store.query_by_time_range(start, end)
    }

    /// Query checkpoints by name pattern
    pub fn query_by_name(&self, pattern: &str) -> Result<Vec<CheckpointInfo>> {
        self.store.query_by_name(pattern)
    }

    /// Get file version at a specific checkpoint
    pub fn get_file_version_at_checkpoint(
        &self,
        path: &Path,
        checkpoint_id: &CheckpointId,
    ) -> Result<Option<FileVersion>> {
        self.store
            .get_file_version_at_checkpoint(path, checkpoint_id)
    }

    /// Get files changed between two checkpoints
    pub fn get_files_changed_between(
        &self,
        from: &CheckpointId,
        to: &CheckpointId,
    ) -> Result<Vec<(PathBuf, FileChangeType)>> {
        self.store.get_files_changed_between(from, to)
    }

    /// Get storage statistics
    pub fn storage_stats(&self) -> Result<StorageStats> {
        self.store.storage_stats()
    }

    /// Rollback specific files to a checkpoint
    pub async fn rollback_files(
        &self,
        checkpoint_id: &CheckpointId,
        files: &[PathBuf],
    ) -> Result<()> {
        self.store.rollback_files(checkpoint_id, files).await
    }

    /// Preview rollback (dry-run)
    pub fn preview_rollback(&self, checkpoint_id: &CheckpointId) -> Result<RollbackPreview> {
        self.store.preview_rollback(checkpoint_id)
    }

    /// Get checkpoint count
    pub fn checkpoint_count(&self) -> Result<usize> {
        self.store.checkpoint_count()
    }

    /// Get tracked file count
    pub fn tracked_file_count(&self) -> Result<usize> {
        self.store.tracked_file_count()
    }

    /// Export a checkpoint to a portable format
    pub fn export_checkpoint(&self, checkpoint_id: &CheckpointId) -> Result<ExportedCheckpoint> {
        self.store.export_checkpoint(checkpoint_id)
    }

    /// Import a checkpoint from a portable format
    pub async fn import_checkpoint(
        &mut self,
        exported: ExportedCheckpoint,
    ) -> Result<CheckpointId> {
        self.store.import_checkpoint(exported).await
    }

    /// Clean up orphaned snapshot files
    pub fn cleanup_snapshots(&self) -> Result<usize> {
        self.store.cleanup_snapshots()
    }
}

/// Exported checkpoint for portability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportedCheckpoint {
    /// Checkpoint name
    pub name: String,
    /// Checkpoint description
    pub description: Option<String>,
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Files with their content
    pub files: Vec<ExportedFile>,
}

/// Exported file content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportedFile {
    /// Relative path
    pub path: PathBuf,
    /// File content (base64 encoded for binary safety)
    pub content: String,
    /// Whether the file is binary
    pub is_binary: bool,
    /// Content hash
    pub hash: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_create_checkpoint() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("timeline.db");
        let mut manager = TimelineManager::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

        let checkpoint_id = manager.create_checkpoint("test").await.unwrap();
        assert!(!checkpoint_id.0.is_empty());
    }

    #[test]
    fn test_list_checkpoints() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("timeline.db");
        let mut manager = TimelineManager::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            manager.create_checkpoint("checkpoint1").await.unwrap();
            manager.create_checkpoint("checkpoint2").await.unwrap();
        });

        let checkpoints = manager.list_checkpoints().unwrap();
        assert_eq!(checkpoints.len(), 2);
    }

    #[tokio::test]
    async fn test_create_checkpoint_with_description() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("timeline.db");
        let mut manager = TimelineManager::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

        let id = manager
            .create_checkpoint_with_description("test", "Test description")
            .await
            .unwrap();
        let checkpoint = manager.get_checkpoint(&id).unwrap().unwrap();

        assert_eq!(checkpoint.description, Some("Test description".to_string()));
    }

    #[tokio::test]
    async fn test_get_checkpoint() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("timeline.db");
        let mut manager = TimelineManager::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

        let id = manager.create_checkpoint("get-test").await.unwrap();
        let checkpoint = manager.get_checkpoint(&id).unwrap().unwrap();

        assert_eq!(checkpoint.name, "get-test");
    }

    #[tokio::test]
    async fn test_delete_checkpoint() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("timeline.db");
        let mut manager = TimelineManager::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

        let id = manager.create_checkpoint("to-delete").await.unwrap();
        assert_eq!(manager.checkpoint_count().unwrap(), 1);

        manager.delete_checkpoint(&id).unwrap();
        assert_eq!(manager.checkpoint_count().unwrap(), 0);
    }

    #[tokio::test]
    async fn test_cleanup_old_checkpoints() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("timeline.db");
        let mut manager = TimelineManager::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

        manager.create_checkpoint("cp1").await.unwrap();
        manager.create_checkpoint("cp2").await.unwrap();
        manager.create_checkpoint("cp3").await.unwrap();

        let deleted = manager.cleanup_old_checkpoints(1).unwrap();
        assert_eq!(deleted, 2);
    }

    #[tokio::test]
    async fn test_track_file() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("timeline.db");
        let manager = TimelineManager::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

        let file_path = temp_dir.path().join("tracked.txt");
        std::fs::write(&file_path, "content").unwrap();

        manager.track_file(&file_path).unwrap();
        assert_eq!(manager.tracked_file_count().unwrap(), 1);
    }

    #[tokio::test]
    async fn test_diff_checkpoints() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("timeline.db");
        let mut manager = TimelineManager::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

        let file = temp_dir.path().join("diff_test.txt");
        std::fs::write(&file, "original").unwrap();

        let cp1 = manager.create_checkpoint("before").await.unwrap();

        std::fs::write(&file, "modified").unwrap();
        let cp2 = manager.create_checkpoint("after").await.unwrap();

        let diff = manager.diff(&cp1, &cp2).unwrap();
        assert_eq!(diff.from, cp1);
        assert_eq!(diff.to, cp2);
    }

    #[tokio::test]
    async fn test_get_file_history() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("timeline.db");
        let mut manager = TimelineManager::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

        let file = temp_dir.path().join("history.txt");
        std::fs::write(&file, "v1").unwrap();
        manager.create_checkpoint("v1").await.unwrap();

        std::fs::write(&file, "v2").unwrap();
        manager.create_checkpoint("v2").await.unwrap();

        let history = manager.get_file_history(&file).unwrap();
        assert!(!history.is_empty());
    }

    #[tokio::test]
    async fn test_query_by_time_range() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("timeline.db");
        let mut manager = TimelineManager::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

        manager.create_checkpoint("time-test").await.unwrap();

        let start = chrono::Utc::now() - chrono::Duration::hours(1);
        let end = chrono::Utc::now() + chrono::Duration::hours(1);

        let checkpoints = manager.query_by_time_range(start, end).unwrap();
        assert_eq!(checkpoints.len(), 1);
    }

    #[tokio::test]
    async fn test_query_by_name() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("timeline.db");
        let mut manager = TimelineManager::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

        manager.create_checkpoint("feature-a").await.unwrap();
        manager.create_checkpoint("feature-b").await.unwrap();
        manager.create_checkpoint("bugfix-x").await.unwrap();

        let features = manager.query_by_name("feature").unwrap();
        assert_eq!(features.len(), 2);
    }

    #[tokio::test]
    async fn test_rollback() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("timeline.db");
        let mut manager = TimelineManager::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

        let file = temp_dir.path().join("rollback.txt");
        std::fs::write(&file, "original content").unwrap();

        let cp = manager.create_checkpoint("pre-change").await.unwrap();

        std::fs::write(&file, "modified content").unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "modified content");

        manager.rollback(&cp).await.unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "original content");
    }

    #[tokio::test]
    async fn test_rollback_files() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("timeline.db");
        let mut manager = TimelineManager::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

        let file1 = temp_dir.path().join("file1.txt");
        let file2 = temp_dir.path().join("file2.txt");
        std::fs::write(&file1, "original 1").unwrap();
        std::fs::write(&file2, "original 2").unwrap();

        let cp = manager.create_checkpoint("multi").await.unwrap();

        std::fs::write(&file1, "modified 1").unwrap();
        std::fs::write(&file2, "modified 2").unwrap();

        manager
            .rollback_files(&cp, std::slice::from_ref(&file1))
            .await
            .unwrap();

        assert_eq!(std::fs::read_to_string(&file1).unwrap(), "original 1");
        assert_eq!(std::fs::read_to_string(&file2).unwrap(), "modified 2");
    }

    #[tokio::test]
    async fn test_preview_rollback() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("timeline.db");
        let mut manager = TimelineManager::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

        let file = temp_dir.path().join("preview.txt");
        std::fs::write(&file, "content").unwrap();

        let cp = manager.create_checkpoint("preview-test").await.unwrap();

        std::fs::write(&file, "modified").unwrap();

        let preview = manager.preview_rollback(&cp).unwrap();
        assert_eq!(preview.checkpoint_id, cp);
        assert!(!preview.files_to_restore.is_empty());
    }

    #[tokio::test]
    async fn test_get_file_version_at_checkpoint() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("timeline.db");
        let mut manager = TimelineManager::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

        let file = temp_dir.path().join("versioned.txt");
        std::fs::write(&file, "version content").unwrap();

        let cp = manager.create_checkpoint("versioned").await.unwrap();

        let version = manager.get_file_version_at_checkpoint(&file, &cp).unwrap();
        assert!(version.is_some());
    }

    #[tokio::test]
    async fn test_get_files_changed_between() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("timeline.db");
        let mut manager = TimelineManager::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

        let file1 = temp_dir.path().join("a.txt");
        std::fs::write(&file1, "a").unwrap();

        let cp1 = manager.create_checkpoint("first").await.unwrap();

        let file2 = temp_dir.path().join("b.txt");
        std::fs::write(&file2, "b").unwrap();
        std::fs::write(&file1, "a-mod").unwrap();

        let cp2 = manager.create_checkpoint("second").await.unwrap();

        let changes = manager.get_files_changed_between(&cp1, &cp2).unwrap();
        assert!(!changes.is_empty());
    }

    #[tokio::test]
    async fn test_storage_stats() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("timeline.db");
        let mut manager = TimelineManager::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

        let file = temp_dir.path().join("stats.txt");
        std::fs::write(&file, "content").unwrap();

        manager.create_checkpoint("stats").await.unwrap();

        let stats = manager.storage_stats().unwrap();
        assert_eq!(stats.checkpoint_count, 1);
    }

    #[test]
    fn test_checkpoint_id_new() {
        let id = CheckpointId::new();
        assert!(!id.0.is_empty());
        assert!(uuid::Uuid::parse_str(&id.0).is_ok());
    }

    #[test]
    fn test_checkpoint_id_from_string() {
        let id = CheckpointId::from_string("custom-id".to_string());
        assert_eq!(id.0, "custom-id");
    }

    #[tokio::test]
    async fn test_timeline_manager_with_watcher() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("timeline.db");
        let config = WatcherConfig::default();

        let manager =
            TimelineManager::with_watcher(&db_path, temp_dir.path().to_path_buf(), config).unwrap();

        assert!(manager.watcher.is_some());
    }

    #[tokio::test]
    async fn test_change_tracker_integration() {
        use change_tracker::{ChangeSource, ChangeTracker, ChangeType, FileChangeRecord};

        let tracker = ChangeTracker::new(100);

        let record = FileChangeRecord::new(
            PathBuf::from("test.rs"),
            ChangeType::ContentModified,
            ChangeSource::UserEdit,
        );

        tracker.record_change(record).await.unwrap();

        let changes = tracker.get_recent_changes(10).await.unwrap();
        assert_eq!(changes.len(), 1);
    }

    #[tokio::test]
    async fn test_export_import_checkpoint() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("timeline.db");
        let mut manager = TimelineManager::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

        let file = temp_dir.path().join("export_test.txt");
        std::fs::write(&file, "export content").unwrap();

        let cp = manager
            .create_checkpoint_with_description("export-test", "Test export")
            .await
            .unwrap();

        let exported = manager.export_checkpoint(&cp).unwrap();
        assert_eq!(exported.name, "export-test");
        assert_eq!(exported.description, Some("Test export".to_string()));

        let imported_cp = manager.import_checkpoint(exported).await.unwrap();
        assert!(!imported_cp.0.is_empty());
    }

    #[tokio::test]
    async fn test_cleanup_snapshots() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("timeline.db");
        let manager = TimelineManager::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

        let cleaned = manager.cleanup_snapshots().unwrap();
        assert_eq!(cleaned, 0);
    }
}
