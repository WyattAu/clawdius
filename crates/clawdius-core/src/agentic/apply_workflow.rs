//! Apply Workflow
//!
//! Defines how changes are applied to the codebase.
//! Users can choose between trust-based application (Option B) or
//! rollback-based application (Option C).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Workflow for applying changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplyWorkflow {
    /// Option B: Apply changes based on configurable trust levels.
    /// Changes are applied if they meet the trust threshold.
    TrustBased {
        /// Required trust level for automatic application
        level: TrustLevel,
        /// Whether to require confirmation for low-trust changes
        confirm_low_trust: bool,
    },

    /// Option C: Create checkpoint before applying, allow rollback.
    /// Safer approach that always preserves the ability to undo.
    RollbackBased {
        /// Maximum number of checkpoints to keep
        max_checkpoints: u32,
        /// Whether to auto-commit after successful verification
        auto_commit: bool,
    },

    /// Preview only - don't apply changes, just show diff.
    PreviewOnly,

    /// Direct apply without any safety measures (not recommended).
    Direct,
}

/// Trust level for applying changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustLevel {
    /// Low trust: Manual review required
    Low = 1,
    /// Medium trust: Some verification required
    Medium = 2,
    /// High trust: Automatic application allowed
    High = 3,
    /// Full trust: No restrictions
    Full = 4,
}

impl TrustLevel {
    /// Creates a low trust level.
    #[must_use]
    pub const fn low() -> Self {
        Self::Low
    }

    /// Creates a medium trust level.
    #[must_use]
    pub const fn medium() -> Self {
        Self::Medium
    }

    /// Creates a high trust level.
    #[must_use]
    pub const fn high() -> Self {
        Self::High
    }

    /// Creates a full trust level.
    #[must_use]
    pub const fn full() -> Self {
        Self::Full
    }

    /// Returns true if this level is at least the given level.
    #[must_use]
    pub const fn at_least(&self, other: Self) -> bool {
        *self as u8 >= other as u8
    }

    /// Returns a human-readable name.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
            Self::Full => "Full",
        }
    }
}

impl Default for TrustLevel {
    fn default() -> Self {
        Self::Medium
    }
}

impl Default for ApplyWorkflow {
    fn default() -> Self {
        Self::RollbackBased {
            max_checkpoints: 10,
            auto_commit: false,
        }
    }
}

impl ApplyWorkflow {
    /// Creates a trust-based workflow with default settings.
    #[must_use]
    pub fn trust_based() -> Self {
        Self::TrustBased {
            level: TrustLevel::Medium,
            confirm_low_trust: true,
        }
    }

    /// Creates a trust-based workflow with a specific level.
    #[must_use]
    pub const fn trust_based_with_level(level: TrustLevel, confirm_low_trust: bool) -> Self {
        Self::TrustBased {
            level,
            confirm_low_trust,
        }
    }

    /// Creates a rollback-based workflow with default settings.
    #[must_use]
    pub fn rollback_based() -> Self {
        Self::RollbackBased {
            max_checkpoints: 10,
            auto_commit: false,
        }
    }

    /// Creates a rollback-based workflow with auto-commit.
    #[must_use]
    pub fn rollback_with_auto_commit() -> Self {
        Self::RollbackBased {
            max_checkpoints: 10,
            auto_commit: true,
        }
    }

    /// Creates a preview-only workflow.
    #[must_use]
    pub const fn preview_only() -> Self {
        Self::PreviewOnly
    }

    /// Creates a direct apply workflow (not recommended).
    #[must_use]
    pub const fn direct() -> Self {
        Self::Direct
    }

    /// Returns true if this is a trust-based workflow.
    #[must_use]
    pub const fn is_trust_based(&self) -> bool {
        matches!(self, Self::TrustBased { .. })
    }

    /// Returns true if this is a rollback-based workflow.
    #[must_use]
    pub const fn is_rollback_based(&self) -> bool {
        matches!(self, Self::RollbackBased { .. })
    }

    /// Returns true if this is preview-only.
    #[must_use]
    pub const fn is_preview_only(&self) -> bool {
        matches!(self, Self::PreviewOnly)
    }

    /// Returns a human-readable name for the workflow.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::TrustBased { level, .. } => match level {
                TrustLevel::Low => "Trust-based (Low)",
                TrustLevel::Medium => "Trust-based (Medium)",
                TrustLevel::High => "Trust-based (High)",
                TrustLevel::Full => "Trust-based (Full)",
            },
            Self::RollbackBased {
                auto_commit: true, ..
            } => "Rollback with Auto-commit",
            Self::RollbackBased {
                auto_commit: false, ..
            } => "Rollback-based",
            Self::PreviewOnly => "Preview Only",
            Self::Direct => "Direct Apply",
        }
    }

    /// Returns the required trust level for this workflow.
    #[must_use]
    pub const fn required_trust_level(&self) -> Option<TrustLevel> {
        match self {
            Self::TrustBased { level, .. } => Some(*level),
            Self::RollbackBased { .. } => Some(TrustLevel::Medium),
            Self::PreviewOnly => None,
            Self::Direct => Some(TrustLevel::Full),
        }
    }
}

/// Result of applying changes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowResult {
    /// Whether the apply was successful.
    pub success: bool,
    /// Checkpoint ID (if created).
    pub checkpoint_id: Option<String>,
    /// Files that were modified.
    pub modified_files: Vec<String>,
    /// Files that were created.
    pub created_files: Vec<String>,
    /// Files that were deleted.
    pub deleted_files: Vec<String>,
    /// Any warnings generated.
    pub warnings: Vec<String>,
    /// Whether a rollback is available.
    pub rollback_available: bool,
    /// Commit hash (if auto-committed).
    pub commit_hash: Option<String>,
    /// Time taken to apply changes.
    pub duration_ms: u64,
}

impl Default for WorkflowResult {
    fn default() -> Self {
        Self {
            success: true,
            checkpoint_id: None,
            modified_files: Vec::new(),
            created_files: Vec::new(),
            deleted_files: Vec::new(),
            warnings: Vec::new(),
            rollback_available: false,
            commit_hash: None,
            duration_ms: 0,
        }
    }
}

impl WorkflowResult {
    /// Creates a successful workflow result.
    #[must_use]
    pub fn success(checkpoint_id: Option<String>, duration_ms: u64) -> Self {
        let rollback_available = checkpoint_id.is_some();
        Self {
            success: true,
            checkpoint_id,
            rollback_available,
            duration_ms,
            ..Self::default()
        }
    }

    /// Creates a failed workflow result.
    #[must_use]
    pub fn failure(duration_ms: u64) -> Self {
        Self {
            success: false,
            duration_ms,
            ..Self::default()
        }
    }

    /// Adds a modified file to the result.
    pub fn add_modified(&mut self, path: String) {
        self.modified_files.push(path);
    }

    /// Adds a created file to the result.
    pub fn add_created(&mut self, path: String) {
        self.created_files.push(path);
    }

    /// Adds a deleted file to the result.
    pub fn add_deleted(&mut self, path: String) {
        self.deleted_files.push(path);
    }

    /// Adds a warning to the result.
    pub fn add_warning(&mut self, warning: String) {
        self.warnings.push(warning);
    }

    /// Returns the total number of files affected.
    #[must_use]
    pub fn total_files(&self) -> usize {
        self.modified_files.len() + self.created_files.len() + self.deleted_files.len()
    }
}

/// A checkpoint for rollback purposes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    /// Unique checkpoint identifier.
    pub id: String,
    /// Timestamp when checkpoint was created.
    pub timestamp: u64,
    /// Description of the checkpoint.
    pub description: String,
    /// Files included in the checkpoint.
    pub files: Vec<CheckpointFile>,
    /// Git commit hash (if applicable).
    pub git_commit: Option<String>,
}

/// A file in a checkpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointFile {
    /// File path.
    pub path: PathBuf,
    /// Original content (before changes).
    pub original_content: String,
    /// Content hash.
    pub hash: String,
}

/// Manager for checkpoints and rollbacks.
pub struct CheckpointManager {
    /// Maximum checkpoints to retain.
    max_checkpoints: u32,
    /// Current checkpoints.
    checkpoints: Vec<Checkpoint>,
}

impl CheckpointManager {
    /// Creates a new checkpoint manager.
    #[must_use]
    pub const fn new(max_checkpoints: u32) -> Self {
        Self {
            max_checkpoints,
            checkpoints: Vec::new(),
        }
    }

    /// Creates a checkpoint of the current state.
    pub fn create_checkpoint(
        &mut self,
        description: &str,
        files: &[PathBuf],
    ) -> crate::error::Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        let checkpoint_files = files
            .iter()
            .filter_map(|path| {
                std::fs::read_to_string(path).ok().map(|content| {
                    let hash = Self::hash_content(&content);
                    CheckpointFile {
                        path: path.clone(),
                        original_content: content,
                        hash,
                    }
                })
            })
            .collect();

        let checkpoint = Checkpoint {
            id: id.clone(),
            timestamp,
            description: description.to_string(),
            files: checkpoint_files,
            git_commit: None,
        };

        self.checkpoints.push(checkpoint);
        self.prune_old_checkpoints();

        Ok(id)
    }

    /// Rolls back to a specific checkpoint.
    pub fn rollback(&mut self, checkpoint_id: &str) -> crate::error::Result<Vec<PathBuf>> {
        let checkpoint = self
            .checkpoints
            .iter()
            .find(|c| c.id == checkpoint_id)
            .ok_or_else(|| {
                crate::error::Error::NotFound(format!("Checkpoint not found: {}", checkpoint_id))
            })?;

        let mut restored = Vec::new();
        for file in &checkpoint.files {
            if let Err(e) = std::fs::write(&file.path, &file.original_content) {
                tracing::warn!("Failed to restore {}: {}", file.path.display(), e);
            } else {
                restored.push(file.path.clone());
            }
        }

        // Remove this and all later checkpoints
        let idx = self.checkpoints.iter().position(|c| c.id == checkpoint_id);
        if let Some(i) = idx {
            self.checkpoints.truncate(i);
        }

        Ok(restored)
    }

    /// Lists all available checkpoints.
    #[must_use]
    pub fn list_checkpoints(&self) -> &[Checkpoint] {
        &self.checkpoints
    }

    /// Returns the most recent checkpoint.
    #[must_use]
    pub fn latest_checkpoint(&self) -> Option<&Checkpoint> {
        self.checkpoints.last()
    }

    fn prune_old_checkpoints(&mut self) {
        while self.checkpoints.len() > self.max_checkpoints as usize {
            self.checkpoints.remove(0);
        }
    }

    fn hash_content(content: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workflow_constructors() {
        assert!(ApplyWorkflow::trust_based().is_trust_based());
        assert!(ApplyWorkflow::rollback_based().is_rollback_based());
        assert!(ApplyWorkflow::preview_only().is_preview_only());
    }

    #[test]
    fn test_trust_level_ordering() {
        assert!(TrustLevel::high().at_least(TrustLevel::medium()));
        assert!(TrustLevel::medium().at_least(TrustLevel::medium()));
        assert!(!TrustLevel::low().at_least(TrustLevel::medium()));
    }

    #[test]
    fn test_workflow_serialization() {
        let workflow = ApplyWorkflow::trust_based();
        let json = serde_json::to_string(&workflow).unwrap();
        assert!(json.contains("trust_based"));
    }

    #[test]
    fn test_workflow_result() {
        let mut result = WorkflowResult::success(Some("cp-123".to_string()), 100);
        result.add_modified("src/lib.rs".to_string());
        result.add_created("src/new.rs".to_string());
        assert!(result.success);
        assert_eq!(result.total_files(), 2);
        assert!(result.rollback_available);
    }

    #[test]
    fn test_checkpoint_manager() {
        let mut manager = CheckpointManager::new(5);
        assert!(manager.list_checkpoints().is_empty());

        // Create checkpoint with empty files list (no actual files)
        let id = manager.create_checkpoint("Test checkpoint", &[]);
        assert!(id.is_ok());
        assert_eq!(manager.list_checkpoints().len(), 1);
    }
}
