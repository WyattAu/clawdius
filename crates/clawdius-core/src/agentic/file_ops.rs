//! File Operations
//!
//! Real file operations with backup and rollback support.

use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// A backup of a file before modification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileBackup {
    /// Original file path
    pub path: PathBuf,
    /// Original content
    pub content: Option<String>,
    /// Timestamp of backup
    pub timestamp: u64,
    /// Whether the file existed before
    pub existed: bool,
}

/// Result of a file operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileOperationResult {
    /// Path that was operated on
    pub path: PathBuf,
    /// Type of operation performed
    pub operation: FileOperation,
    /// Whether the operation succeeded
    pub success: bool,
    /// Backup created (if any)
    pub backup: Option<FileBackup>,
    /// Error message (if failed)
    pub error: Option<String>,
}

/// Type of file operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileOperation {
    /// Created a new file
    Create,
    /// Modified an existing file
    Modify,
    /// Deleted a file
    Delete,
}

/// Handles file operations with backup support.
pub struct FileOperations {
    /// Directory for backups
    backup_dir: PathBuf,
    /// Created backups (for rollback)
    backups: Vec<FileBackup>,
}

impl FileOperations {
    /// Creates a new file operations handler.
    #[must_use]
    pub fn new(backup_dir: PathBuf) -> Self {
        Self {
            backup_dir,
            backups: Vec::new(),
        }
    }

    /// Creates a file operations handler with default backup directory.
    #[must_use]
    pub fn with_default_backup_dir() -> Self {
        let backup_dir = std::env::temp_dir().join("clawdius-backups");
        Self::new(backup_dir)
    }

    /// Writes content to a file, creating a backup first.
    ///
    /// # Errors
    ///
    /// Returns an error if the file operation fails.
    pub fn write_file(&mut self, path: &Path, content: &str) -> Result<FileOperationResult> {
        // Create backup if file exists
        let backup = if path.exists() {
            let original_content = std::fs::read_to_string(path)?;
            let backup = FileBackup {
                path: path.to_path_buf(),
                content: Some(original_content),
                timestamp: current_timestamp(),
                existed: true,
            };
            self.backups.push(backup.clone());
            Some(backup)
        } else {
            // Ensure parent directory exists
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            None
        };

        // Write the file
        match std::fs::write(path, content) {
            Ok(()) => Ok(FileOperationResult {
                path: path.to_path_buf(),
                operation: if backup.is_some() {
                    FileOperation::Modify
                } else {
                    FileOperation::Create
                },
                success: true,
                backup,
                error: None,
            }),
            Err(e) => Ok(FileOperationResult {
                path: path.to_path_buf(),
                operation: FileOperation::Create,
                success: false,
                backup: None,
                error: Some(e.to_string()),
            }),
        }
    }

    /// Deletes a file, creating a backup first.
    ///
    /// # Errors
    ///
    /// Returns an error if the file operation fails.
    pub fn delete_file(&mut self, path: &Path) -> Result<FileOperationResult> {
        if !path.exists() {
            return Ok(FileOperationResult {
                path: path.to_path_buf(),
                operation: FileOperation::Delete,
                success: true,
                backup: None,
                error: None,
            });
        }

        // Create backup
        let original_content = std::fs::read_to_string(path)?;
        let backup = FileBackup {
            path: path.to_path_buf(),
            content: Some(original_content),
            timestamp: current_timestamp(),
            existed: true,
        };
        self.backups.push(backup.clone());

        // Delete the file
        match std::fs::remove_file(path) {
            Ok(()) => Ok(FileOperationResult {
                path: path.to_path_buf(),
                operation: FileOperation::Delete,
                success: true,
                backup: Some(backup),
                error: None,
            }),
            Err(e) => Ok(FileOperationResult {
                path: path.to_path_buf(),
                operation: FileOperation::Delete,
                success: false,
                backup: None,
                error: Some(e.to_string()),
            }),
        }
    }

    /// Rolls back all operations, restoring files to their original state.
    ///
    /// # Errors
    ///
    /// Returns an error if rollback fails.
    pub fn rollback(&self) -> Result<Vec<PathBuf>> {
        let mut restored = Vec::new();

        // Roll back in reverse order
        for backup in self.backups.iter().rev() {
            if let Some(content) = &backup.content {
                // Restore original content
                std::fs::write(&backup.path, content)?;
                restored.push(backup.path.clone());
            } else if backup.existed {
                // File was deleted, remove it
                if backup.path.exists() {
                    std::fs::remove_file(&backup.path)?;
                }
                restored.push(backup.path.clone());
            }
        }

        Ok(restored)
    }

    /// Saves all backups to the backup directory.
    ///
    /// # Errors
    ///
    /// Returns an error if saving fails.
    pub fn save_backups(&self) -> Result<()> {
        std::fs::create_dir_all(&self.backup_dir)?;

        for backup in &self.backups {
            if let Some(content) = &backup.content {
                let backup_path = self.backup_dir.join(format!(
                    "{}-{}.bak",
                    backup
                        .path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy(),
                    backup.timestamp
                ));
                std::fs::write(backup_path, content)?;
            }
        }

        Ok(())
    }

    /// Clears all backups (call after successful commit).
    pub fn clear_backups(&mut self) {
        self.backups.clear();
    }

    /// Returns the number of backups.
    #[must_use]
    pub fn backup_count(&self) -> usize {
        self.backups.len()
    }

    /// Returns all backups.
    #[must_use]
    pub fn backups(&self) -> &[FileBackup] {
        &self.backups
    }
}

impl Default for FileOperations {
    fn default() -> Self {
        Self::with_default_backup_dir()
    }
}

/// Applies a diff to a file.
///
/// # Errors
///
/// Returns an error if the diff cannot be applied.
pub fn apply_diff(original: &str, diff: &str) -> Result<String> {
    // Simple unified diff parsing
    let mut result = String::new();
    let mut in_hunk = false;
    let _old_lines: Vec<&str> = original.lines().collect();
    let mut new_lines: Vec<String> = Vec::new();

    for line in diff.lines() {
        if line.starts_with("@@") {
            // Parse hunk header
            in_hunk = true;
            continue;
        }

        if in_hunk {
            if line.starts_with('+') && !line.starts_with("+++") {
                // Added line
                new_lines.push(line[1..].to_string());
            } else if line.starts_with('-') && !line.starts_with("---") {
                // Removed line - skip
            } else if !line.starts_with('\\') {
                // Context line
                new_lines.push(line.to_string());
            }
        }
    }

    // If no hunks were found, treat the diff as the new content
    if new_lines.is_empty() && !diff.lines().any(|l| l.starts_with("@@")) {
        return Ok(diff.to_string());
    }

    result = new_lines.join("\n");
    Ok(result)
}

/// Creates a unified diff between two strings.
#[must_use]
pub fn create_diff(original: &str, new: &str, path: &str) -> String {
    let old_lines: Vec<&str> = original.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();

    let mut diff = format!("--- {}\n+++ {}\n", path, path);

    // Simple diff: show all changes as one hunk
    if old_lines.is_empty() && !new_lines.is_empty() {
        // New file
        diff.push_str("@@ -0,0 +1,0 @@\n");
        for line in &new_lines {
            diff.push_str(&format!("+{}\n", line));
        }
    } else if !old_lines.is_empty() && new_lines.is_empty() {
        // Deleted file
        diff.push_str("@@ -1,0 +0,0 @@\n");
        for line in &old_lines {
            diff.push_str(&format!("-{}\n", line));
        }
    } else {
        // Modified file - show all lines
        let old_count = old_lines.len();
        let new_count = new_lines.len();
        diff.push_str(&format!("@@ -1,{} +1,{} @@\n", old_count, new_count));

        // Simple approach: show all old lines as removed, all new as added
        for line in &old_lines {
            diff.push_str(&format!("-{}\n", line));
        }
        for line in &new_lines {
            diff.push_str(&format!("+{}\n", line));
        }
    }

    diff
}

/// Gets the current timestamp in milliseconds.
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_write_new_file() {
        let temp_dir = TempDir::new().unwrap();
        let mut ops = FileOperations::new(temp_dir.path().to_path_buf());

        let file_path = temp_dir.path().join("new_file.txt");
        let result = ops.write_file(&file_path, "Hello, world!").unwrap();

        assert!(result.success);
        assert_eq!(result.operation, FileOperation::Create);
        assert!(result.backup.is_none());
        assert!(file_path.exists());
    }

    #[test]
    fn test_write_existing_file() {
        let temp_dir = TempDir::new().unwrap();
        let mut ops = FileOperations::new(temp_dir.path().to_path_buf());

        let file_path = temp_dir.path().join("existing.txt");
        std::fs::write(&file_path, "Original content").unwrap();

        let result = ops.write_file(&file_path, "New content").unwrap();

        assert!(result.success);
        assert_eq!(result.operation, FileOperation::Modify);
        assert!(result.backup.is_some());

        let backup = result.backup.unwrap();
        assert_eq!(backup.content, Some("Original content".to_string()));
    }

    #[test]
    fn test_rollback() {
        let temp_dir = TempDir::new().unwrap();
        let mut ops = FileOperations::new(temp_dir.path().to_path_buf());

        let file_path = temp_dir.path().join("rollback_test.txt");
        std::fs::write(&file_path, "Original").unwrap();

        ops.write_file(&file_path, "Modified").unwrap();

        // Rollback
        let restored = ops.rollback().unwrap();
        assert_eq!(restored.len(), 1);

        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "Original");
    }

    #[test]
    fn test_create_diff() {
        let original = "line1\nline2\nline3";
        let new = "line1\nmodified\nline3";
        let diff = create_diff(original, new, "test.txt");

        assert!(diff.contains("--- test.txt"));
        assert!(diff.contains("+++ test.txt"));
        assert!(diff.contains("-line2"));
        assert!(diff.contains("+modified"));
    }

    #[test]
    fn test_create_diff_new_file() {
        let diff = create_diff("", "new content", "new.txt");
        assert!(diff.contains("+new content"));
    }

    #[test]
    fn test_create_diff_delete_file() {
        let diff = create_diff("old content", "", "old.txt");
        assert!(diff.contains("-old content"));
    }
}
