//! Granular file change tracking for timeline
//!
//! This module provides detailed tracking of file changes including:
//! - Line-level change detection
//! - Change attribution (which operation caused the change)
//! - Change batching and deduplication
//! - Integration with the timeline store

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

use super::CheckpointId;
use crate::error::{Error, Result};

/// Unique identifier for a change
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ChangeId(pub String);

impl ChangeId {
    #[must_use]
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}

impl Default for ChangeId {
    fn default() -> Self {
        Self::new()
    }
}

/// Type of file change
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ChangeType {
    ContentModified,
    Created,
    Deleted,
    Renamed,
    PermissionsChanged,
}

/// Source/origin of a change
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ChangeSource {
    UserEdit,
    ToolExecution(String),
    AutoSave,
    External,
    Rollback,
    Checkpoint,
    Unknown,
}

/// A single line change
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineChange {
    pub line_number: usize,
    pub old_content: Option<String>,
    pub new_content: Option<String>,
    pub change_type: LineChangeType,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum LineChangeType {
    Added,
    Removed,
    Modified,
}

/// Detailed record of a file change
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChangeRecord {
    pub id: ChangeId,
    pub path: PathBuf,
    pub change_type: ChangeType,
    pub source: ChangeSource,
    pub timestamp: DateTime<Utc>,
    pub old_hash: Option<String>,
    pub new_hash: Option<String>,
    pub old_size: Option<usize>,
    pub new_size: Option<usize>,
    pub line_changes: Vec<LineChange>,
    pub checkpoint_id: Option<CheckpointId>,
    pub session_id: Option<String>,
    pub metadata: HashMap<String, String>,
}

impl FileChangeRecord {
    #[must_use]
    pub fn new(path: PathBuf, change_type: ChangeType, source: ChangeSource) -> Self {
        Self {
            id: ChangeId::new(),
            path,
            change_type,
            source,
            timestamp: Utc::now(),
            old_hash: None,
            new_hash: None,
            old_size: None,
            new_size: None,
            line_changes: Vec::new(),
            checkpoint_id: None,
            session_id: None,
            metadata: HashMap::new(),
        }
    }

    #[must_use]
    pub fn with_hashes(mut self, old_hash: Option<String>, new_hash: Option<String>) -> Self {
        self.old_hash = old_hash;
        self.new_hash = new_hash;
        self
    }

    #[must_use]
    pub fn with_sizes(mut self, old_size: Option<usize>, new_size: Option<usize>) -> Self {
        self.old_size = old_size;
        self.new_size = new_size;
        self
    }

    #[must_use]
    pub fn with_line_changes(mut self, line_changes: Vec<LineChange>) -> Self {
        self.line_changes = line_changes;
        self
    }

    #[must_use]
    pub fn with_checkpoint(mut self, checkpoint_id: CheckpointId) -> Self {
        self.checkpoint_id = Some(checkpoint_id);
        self
    }

    #[must_use]
    pub fn with_session(mut self, session_id: String) -> Self {
        self.session_id = Some(session_id);
        self
    }

    #[must_use]
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }

    #[must_use]
    pub fn additions(&self) -> usize {
        self.line_changes
            .iter()
            .filter(|c| c.change_type == LineChangeType::Added)
            .count()
    }

    #[must_use]
    pub fn deletions(&self) -> usize {
        self.line_changes
            .iter()
            .filter(|c| c.change_type == LineChangeType::Removed)
            .count()
    }

    #[must_use]
    pub fn modifications(&self) -> usize {
        self.line_changes
            .iter()
            .filter(|c| c.change_type == LineChangeType::Modified)
            .count()
    }
}

/// Summary of changes for a file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChangeSummary {
    pub path: PathBuf,
    pub total_changes: usize,
    pub last_modified: DateTime<Utc>,
    pub change_types: Vec<ChangeType>,
    pub sources: Vec<ChangeSource>,
}

/// Change tracker for recording and querying file changes
pub struct ChangeTracker {
    changes: Arc<RwLock<Vec<FileChangeRecord>>>,
    file_index: Arc<RwLock<HashMap<PathBuf, Vec<ChangeId>>>>,
    max_changes: usize,
}

impl ChangeTracker {
    #[must_use]
    pub fn new(max_changes: usize) -> Self {
        Self {
            changes: Arc::new(RwLock::new(Vec::new())),
            file_index: Arc::new(RwLock::new(HashMap::new())),
            max_changes,
        }
    }

    pub async fn record_change(&self, record: FileChangeRecord) -> Result<ChangeId> {
        let id = record.id.clone();
        let path = record.path.clone();

        {
            let mut changes = self.changes.write().await;

            if changes.len() >= self.max_changes {
                if let Some(oldest) = changes.first() {
                    let mut index = self.file_index.write().await;
                    if let Some(ids) = index.get_mut(&oldest.path) {
                        ids.retain(|i| i != &oldest.id);
                    }
                }
                changes.remove(0);
            }

            changes.push(record);
        }

        {
            let mut index = self.file_index.write().await;
            index.entry(path).or_default().push(id.clone());
        }

        Ok(id)
    }

    pub async fn get_changes_for_file(&self, path: &Path) -> Result<Vec<FileChangeRecord>> {
        let index = self.file_index.read().await;
        let changes = self.changes.read().await;

        let change_ids = index.get(path).cloned().unwrap_or_default();

        let result: Vec<FileChangeRecord> = changes
            .iter()
            .filter(|c| change_ids.contains(&c.id))
            .cloned()
            .collect();

        Ok(result)
    }

    pub async fn get_recent_changes(&self, limit: usize) -> Result<Vec<FileChangeRecord>> {
        let changes = self.changes.read().await;
        let result: Vec<FileChangeRecord> = changes.iter().rev().take(limit).cloned().collect();
        Ok(result)
    }

    pub async fn get_changes_in_range(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<FileChangeRecord>> {
        let changes = self.changes.read().await;
        let result: Vec<FileChangeRecord> = changes
            .iter()
            .filter(|c| c.timestamp >= start && c.timestamp <= end)
            .cloned()
            .collect();
        Ok(result)
    }

    pub async fn get_changes_by_source(
        &self,
        source: &ChangeSource,
    ) -> Result<Vec<FileChangeRecord>> {
        let changes = self.changes.read().await;
        let result: Vec<FileChangeRecord> = changes
            .iter()
            .filter(|c| &c.source == source)
            .cloned()
            .collect();
        Ok(result)
    }

    pub async fn get_changes_by_checkpoint(
        &self,
        checkpoint_id: &CheckpointId,
    ) -> Result<Vec<FileChangeRecord>> {
        let changes = self.changes.read().await;
        let result: Vec<FileChangeRecord> = changes
            .iter()
            .filter(|c| c.checkpoint_id.as_ref() == Some(checkpoint_id))
            .cloned()
            .collect();
        Ok(result)
    }

    pub async fn get_file_summary(&self, path: &Path) -> Result<Option<FileChangeSummary>> {
        let changes = self.get_changes_for_file(path).await?;

        if changes.is_empty() {
            return Ok(None);
        }

        let total_changes = changes.len();
        let last_modified = changes
            .iter()
            .map(|c| c.timestamp)
            .max()
            .unwrap_or_else(Utc::now);

        let change_types: Vec<ChangeType> = changes.iter().map(|c| c.change_type).collect();

        let sources: Vec<ChangeSource> = changes.iter().map(|c| c.source.clone()).collect();

        Ok(Some(FileChangeSummary {
            path: path.to_path_buf(),
            total_changes,
            last_modified,
            change_types,
            sources,
        }))
    }

    pub async fn clear(&self) -> Result<()> {
        let mut changes = self.changes.write().await;
        let mut index = self.file_index.write().await;
        changes.clear();
        index.clear();
        Ok(())
    }

    pub async fn change_count(&self) -> usize {
        self.changes.read().await.len()
    }
}

/// Diff calculator for computing line-level changes
pub struct DiffCalculator;

impl DiffCalculator {
    #[must_use]
    pub fn compute_line_diff(old_content: &str, new_content: &str) -> Vec<LineChange> {
        use similar::{ChangeTag, TextDiff};

        let diff = TextDiff::from_lines(old_content, new_content);
        let mut changes = Vec::new();
        let mut old_line = 1;
        let mut new_line = 1;

        for change in diff.iter_all_changes() {
            match change.tag() {
                ChangeTag::Delete => {
                    changes.push(LineChange {
                        line_number: old_line,
                        old_content: Some(change.to_string()),
                        new_content: None,
                        change_type: LineChangeType::Removed,
                    });
                    old_line += 1;
                }
                ChangeTag::Insert => {
                    changes.push(LineChange {
                        line_number: new_line,
                        old_content: None,
                        new_content: Some(change.to_string()),
                        change_type: LineChangeType::Added,
                    });
                    new_line += 1;
                }
                ChangeTag::Equal => {
                    old_line += 1;
                    new_line += 1;
                }
            }
        }

        changes
    }

    #[must_use]
    pub fn hash_content(content: &str) -> String {
        let mut hasher = Sha3_256::new();
        hasher.update(content.as_bytes());
        format!("{:x}", hasher.finalize())
    }
}

/// File state for tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileState {
    pub path: PathBuf,
    pub hash: String,
    pub size: usize,
    pub modified_time: DateTime<Utc>,
}

impl FileState {
    pub fn from_path(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path).map_err(Error::Io)?;
        let _metadata = std::fs::metadata(path).map_err(Error::Io)?;

        Ok(Self {
            path: path.to_path_buf(),
            hash: DiffCalculator::hash_content(&content),
            size: content.len(),
            modified_time: Utc::now(),
        })
    }

    #[must_use]
    pub fn from_content(path: &Path, content: &str) -> Self {
        Self {
            path: path.to_path_buf(),
            hash: DiffCalculator::hash_content(content),
            size: content.len(),
            modified_time: Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_record_change() {
        let tracker = ChangeTracker::new(100);
        let record = FileChangeRecord::new(
            PathBuf::from("test.rs"),
            ChangeType::ContentModified,
            ChangeSource::UserEdit,
        );

        let id = tracker.record_change(record).await.unwrap();
        assert!(!id.0.is_empty());
        assert_eq!(tracker.change_count().await, 1);
    }

    #[tokio::test]
    async fn test_get_changes_for_file() {
        let tracker = ChangeTracker::new(100);
        let path = PathBuf::from("src/main.rs");

        let record1 = FileChangeRecord::new(
            path.clone(),
            ChangeType::ContentModified,
            ChangeSource::UserEdit,
        );
        let record2 = FileChangeRecord::new(
            path.clone(),
            ChangeType::ContentModified,
            ChangeSource::ToolExecution("edit".to_string()),
        );

        tracker.record_change(record1).await.unwrap();
        tracker.record_change(record2).await.unwrap();

        let changes = tracker.get_changes_for_file(&path).await.unwrap();
        assert_eq!(changes.len(), 2);
    }

    #[tokio::test]
    async fn test_max_changes_limit() {
        let tracker = ChangeTracker::new(3);

        for i in 0..5 {
            let record = FileChangeRecord::new(
                PathBuf::from(format!("file{}.rs", i)),
                ChangeType::ContentModified,
                ChangeSource::UserEdit,
            );
            tracker.record_change(record).await.unwrap();
        }

        assert_eq!(tracker.change_count().await, 3);
    }

    #[tokio::test]
    async fn test_get_file_summary() {
        let tracker = ChangeTracker::new(100);
        let path = PathBuf::from("lib.rs");

        tracker
            .record_change(FileChangeRecord::new(
                path.clone(),
                ChangeType::Created,
                ChangeSource::UserEdit,
            ))
            .await
            .unwrap();

        tracker
            .record_change(FileChangeRecord::new(
                path.clone(),
                ChangeType::ContentModified,
                ChangeSource::ToolExecution("refactor".to_string()),
            ))
            .await
            .unwrap();

        let summary = tracker.get_file_summary(&path).await.unwrap().unwrap();
        assert_eq!(summary.total_changes, 2);
        assert_eq!(summary.change_types.len(), 2);
    }

    #[test]
    fn test_compute_line_diff() {
        let old = "line1\nline2\nline3";
        let new = "line1\nline2-modified\nline3\nline4";

        let changes = DiffCalculator::compute_line_diff(old, new);

        assert!(!changes.is_empty());
        assert!(changes
            .iter()
            .any(|c| c.change_type == LineChangeType::Removed));
        assert!(changes
            .iter()
            .any(|c| c.change_type == LineChangeType::Added));
    }

    #[test]
    fn test_hash_content() {
        let content = "test content";
        let hash1 = DiffCalculator::hash_content(content);
        let hash2 = DiffCalculator::hash_content(content);

        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 64);
    }

    #[tokio::test]
    async fn test_get_changes_by_source() {
        let tracker = ChangeTracker::new(100);
        let source = ChangeSource::ToolExecution("edit".to_string());

        tracker
            .record_change(FileChangeRecord::new(
                PathBuf::from("a.rs"),
                ChangeType::ContentModified,
                source.clone(),
            ))
            .await
            .unwrap();

        tracker
            .record_change(FileChangeRecord::new(
                PathBuf::from("b.rs"),
                ChangeType::ContentModified,
                ChangeSource::UserEdit,
            ))
            .await
            .unwrap();

        let changes = tracker.get_changes_by_source(&source).await.unwrap();
        assert_eq!(changes.len(), 1);
    }

    #[tokio::test]
    async fn test_change_record_builder() {
        let checkpoint_id = CheckpointId::new();
        let record = FileChangeRecord::new(
            PathBuf::from("test.rs"),
            ChangeType::ContentModified,
            ChangeSource::ToolExecution("edit".to_string()),
        )
        .with_hashes(Some("old-hash".to_string()), Some("new-hash".to_string()))
        .with_sizes(Some(100), Some(150))
        .with_checkpoint(checkpoint_id.clone())
        .with_session("session-123".to_string())
        .with_metadata("tool".to_string(), "edit_file".to_string());

        assert_eq!(record.old_hash, Some("old-hash".to_string()));
        assert_eq!(record.new_size, Some(150));
        assert_eq!(record.checkpoint_id, Some(checkpoint_id));
        assert_eq!(record.session_id, Some("session-123".to_string()));
        assert_eq!(record.metadata.get("tool"), Some(&"edit_file".to_string()));
    }

    #[test]
    fn test_file_state_from_content() {
        let path = PathBuf::from("test.txt");
        let content = "Hello, World!";
        let state = FileState::from_content(&path, content);

        assert_eq!(state.path, path);
        assert_eq!(state.size, 13);
        assert!(!state.hash.is_empty());
    }

    #[test]
    fn test_file_state_from_path() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, "content").unwrap();

        let state = FileState::from_path(&file_path).unwrap();
        assert_eq!(state.size, 7);
        assert!(!state.hash.is_empty());
    }
}
