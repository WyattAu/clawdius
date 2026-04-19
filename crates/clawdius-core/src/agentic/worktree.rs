//! Git Worktree Support for Parallel Sprints
//!
//! This module provides filesystem isolation for concurrent sprint execution
//! using git worktrees. Each sprint gets its own worktree (working directory)
//! backed by a separate branch, enabling true parallel execution without
//! file conflicts.

use crate::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Error type for worktree operations.
#[derive(Debug, thiserror::Error)]
pub enum WorktreeError {
    #[error("Git command failed: {0}")]
    GitFailed(String),
    #[error("Worktree already exists: {0}")]
    AlreadyExists(String),
    #[error("Worktree not found: {0}")]
    NotFound(String),
    #[error("Not a git repository: {0}")]
    NotGitRepo(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// A worktree session representing an isolated working directory for a sprint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorktreeSession {
    /// Unique session identifier
    pub id: String,
    /// Branch name associated with this worktree
    pub branch: String,
    /// Absolute path to the worktree working directory
    pub worktree_path: PathBuf,
    /// Absolute path to the main repository
    pub repo_root: PathBuf,
    /// Sprint task description (for reference)
    pub task_description: String,
    /// When this session was created
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Session status
    pub status: WorktreeStatus,
}

/// Status of a worktree session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorktreeStatus {
    /// Worktree created and ready
    Active,
    /// Sprint completed successfully
    Completed,
    /// Sprint failed
    Failed,
    /// Changes merged back to main branch
    Merged,
    /// Worktree cleaned up
    Removed,
}

/// Manages git worktree creation, listing, and cleanup for parallel sprints.
pub struct WorktreeManager {
    /// Root directory of the git repository
    repo_root: PathBuf,
}

impl WorktreeManager {
    /// Create a new WorktreeManager for the given repository.
    ///
    /// # Errors
    ///
    /// Returns an error if the path is not a git repository.
    pub fn new(repo_root: PathBuf) -> std::result::Result<Self, WorktreeError> {
        // Verify it's a git repo
        let output = std::process::Command::new("git")
            .args(["rev-parse", "--is-inside-work-tree"])
            .current_dir(&repo_root)
            .output()
            .map_err(|e| WorktreeError::GitFailed(format!("Failed to run git: {e}")))?;

        if !output.status.success() {
            return Err(WorktreeError::NotGitRepo(repo_root.display().to_string()));
        }

        Ok(Self { repo_root })
    }

    /// Create a new worktree for a sprint task.
    ///
    /// Creates a new branch and worktree at `<repo_root>/../.clawdius-worktrees/<id>`.
    ///
    /// # Errors
    ///
    /// Returns an error if git worktree creation fails.
    pub fn create_worktree(
        &self,
        task_description: &str,
    ) -> std::result::Result<WorktreeSession, WorktreeError> {
        let session_id = format!("sprint-{}", chrono::Utc::now().format("%Y%m%d-%H%M%S-%f"));
        let branch = format!("clawdius/{session_id}");

        // Worktrees go in a sibling directory to avoid nesting issues
        let worktree_base = self
            .repo_root
            .parent()
            .unwrap_or(&self.repo_root)
            .join(".clawdius-worktrees");
        std::fs::create_dir_all(&worktree_base)?;

        let worktree_path = worktree_base.join(&session_id);

        // Create the worktree with a new branch
        let output = std::process::Command::new("git")
            .args([
                "worktree",
                "add",
                "-b",
                &branch,
                worktree_path
                    .to_str()
                    .ok_or_else(|| WorktreeError::GitFailed("Invalid worktree path".to_string()))?,
            ])
            .current_dir(&self.repo_root)
            .output()
            .map_err(|e| {
                WorktreeError::GitFailed(format!("Failed to run git worktree add: {e}"))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(WorktreeError::GitFailed(format!(
                "git worktree add failed: {stderr}"
            )));
        }

        tracing::info!(
            "Created worktree: {} (branch: {}, path: {})",
            session_id,
            branch,
            worktree_path.display()
        );

        Ok(WorktreeSession {
            id: session_id,
            branch,
            worktree_path: worktree_path.clone(),
            repo_root: self.repo_root.clone(),
            task_description: task_description.to_string(),
            created_at: chrono::Utc::now(),
            status: WorktreeStatus::Active,
        })
    }

    /// List all clawdius-managed worktrees.
    pub fn list_worktrees(&self) -> std::result::Result<Vec<WorktreeSession>, WorktreeError> {
        let output = std::process::Command::new("git")
            .args(["worktree", "list", "--porcelain"])
            .current_dir(&self.repo_root)
            .output()
            .map_err(|e| WorktreeError::GitFailed(format!("Failed to list worktrees: {e}")))?;

        if !output.status.success() {
            return Ok(Vec::new());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let worktree_base = self
            .repo_root
            .parent()
            .unwrap_or(&self.repo_root)
            .join(".clawdius-worktrees");

        let mut sessions = Vec::new();
        let mut current_path = String::new();
        let mut current_branch = String::new();

        for line in stdout.lines() {
            if let Some(path) = line.strip_prefix("worktree ") {
                current_path = path.to_string();
            } else if let Some(branch) = line.strip_prefix("branch refs/heads/") {
                current_branch = branch.to_string();
            } else if line.is_empty() && !current_path.is_empty() {
                // Check if this is a clawdius-managed worktree
                let path = PathBuf::from(&current_path);
                if path.starts_with(&worktree_base) {
                    if let Some(id) = path.file_name().and_then(|n| n.to_str()) {
                        sessions.push(WorktreeSession {
                            id: id.to_string(),
                            branch: current_branch.clone(),
                            worktree_path: path,
                            repo_root: self.repo_root.clone(),
                            task_description: String::new(),
                            created_at: chrono::Utc::now(), // Unknown from git output
                            status: WorktreeStatus::Active,
                        });
                    }
                }
                current_path.clear();
                current_branch.clear();
            }
        }

        Ok(sessions)
    }

    /// Remove a worktree and its associated branch.
    ///
    /// # Errors
    ///
    /// Returns an error if the worktree cannot be removed.
    pub fn remove_worktree(
        &self,
        session: &WorktreeSession,
    ) -> std::result::Result<(), WorktreeError> {
        // Remove the worktree
        let output = std::process::Command::new("git")
            .args(["worktree", "remove", "--force"])
            .arg(&session.worktree_path)
            .current_dir(&self.repo_root)
            .output()
            .map_err(|e| WorktreeError::GitFailed(format!("Failed to remove worktree: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::warn!("Failed to remove worktree: {stderr}");
        }

        // Prune stale worktree entries
        let _ = std::process::Command::new("git")
            .args(["worktree", "prune"])
            .current_dir(&self.repo_root)
            .output();

        // Delete the branch (force, in case of unmerged commits)
        let output = std::process::Command::new("git")
            .args(["branch", "-D", &session.branch])
            .current_dir(&self.repo_root)
            .output()
            .map_err(|e| WorktreeError::GitFailed(format!("Failed to delete branch: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::warn!("Failed to delete branch {}: {stderr}", session.branch);
        }

        tracing::info!(
            "Removed worktree: {} (branch: {})",
            session.id,
            session.branch
        );

        Ok(())
    }

    /// Merge a worktree's branch back into the current branch of the main repo.
    ///
    /// Returns the merge output for logging.
    pub fn merge_worktree(
        &self,
        session: &WorktreeSession,
    ) -> std::result::Result<String, WorktreeError> {
        let output = std::process::Command::new("git")
            .args(["merge", "--no-ff", &session.branch])
            .current_dir(&self.repo_root)
            .output()
            .map_err(|e| WorktreeError::GitFailed(format!("Failed to merge: {e}")))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if !output.status.success() {
            return Err(WorktreeError::GitFailed(format!("Merge failed: {stderr}")));
        }

        tracing::info!(
            "Merged worktree branch {} into current branch",
            session.branch
        );

        Ok(stdout)
    }

    /// Get the diff between a worktree's branch and the main branch.
    pub fn get_diff(
        &self,
        session: &WorktreeSession,
    ) -> std::result::Result<String, WorktreeError> {
        let output = std::process::Command::new("git")
            .args(["diff", &format!("HEAD...{}", session.branch)])
            .current_dir(&self.repo_root)
            .output()
            .map_err(|e| WorktreeError::GitFailed(format!("Failed to get diff: {e}")))?;

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Clean up all clawdius-managed worktrees.
    pub fn cleanup_all(&self) -> std::result::Result<usize, WorktreeError> {
        let sessions = self.list_worktrees()?;
        let count = sessions.len();
        for session in &sessions {
            if let Err(e) = self.remove_worktree(session) {
                tracing::warn!("Failed to cleanup worktree {}: {e}", session.id);
            }
        }
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_worktree_status_serialization() {
        let status = WorktreeStatus::Active;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"Active\"");

        let parsed: WorktreeStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, WorktreeStatus::Active);
    }

    #[test]
    fn test_worktree_session_serialization() {
        let session = WorktreeSession {
            id: "sprint-test".to_string(),
            branch: "clawdius/sprint-test".to_string(),
            worktree_path: PathBuf::from("/tmp/worktrees/sprint-test"),
            repo_root: PathBuf::from("/project"),
            task_description: "Test task".to_string(),
            created_at: chrono::Utc::now(),
            status: WorktreeStatus::Active,
        };

        let json = serde_json::to_string(&session).unwrap();
        let parsed: WorktreeSession = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, "sprint-test");
        assert_eq!(parsed.branch, "clawdius/sprint-test");
        assert_eq!(parsed.status, WorktreeStatus::Active);
    }
}
