//! Checkpoint manager for workspace snapshots and restoration

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

use super::diff::{Diff, DiffHunk, DiffLine, DiffLineType};
use super::snapshot::FileSnapshot;
use crate::error::{Error, Result};

/// A checkpoint representing a workspace state at a point in time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    /// Unique checkpoint ID
    pub id: String,
    /// Session ID this checkpoint belongs to
    pub session_id: String,
    /// Message ID at time of checkpoint (optional)
    pub message_id: Option<String>,
    /// Human-readable description
    pub description: String,
    /// Creation timestamp
    pub timestamp: DateTime<Utc>,
    /// File snapshots included in this checkpoint
    pub files: Vec<FileSnapshot>,
}

/// Timeline of checkpoints for a session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Timeline {
    /// Session ID
    pub session_id: String,
    /// Checkpoints in chronological order (oldest first)
    pub checkpoints: Vec<CheckpointSummary>,
    /// Current checkpoint index (for navigation)
    pub current_index: Option<usize>,
}

/// Summary of a checkpoint for timeline display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointSummary {
    /// Checkpoint ID
    pub id: String,
    /// Description
    pub description: String,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Number of files
    pub file_count: usize,
    /// Message ID if associated with a message
    pub message_id: Option<String>,
}

/// Checkpoint manager for creating, listing, and restoring checkpoints
pub struct CheckpointManager {
    conn: Connection,
    workspace_root: PathBuf,
    snapshot_dir: PathBuf,
}

impl CheckpointManager {
    /// Create a new checkpoint manager
    pub fn new(db_path: &Path, workspace_root: PathBuf) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(db_path)?;
        let snapshot_dir = workspace_root.join(".clawdius").join("snapshots");
        fs::create_dir_all(&snapshot_dir)?;

        let manager = Self {
            conn,
            workspace_root,
            snapshot_dir,
        };

        manager.initialize()?;
        Ok(manager)
    }

    /// Initialize database schema
    fn initialize(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS checkpoints (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                message_id TEXT,
                description TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                file_count INTEGER NOT NULL DEFAULT 0
            );
            
            CREATE TABLE IF NOT EXISTS checkpoint_files (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                checkpoint_id TEXT NOT NULL REFERENCES checkpoints(id) ON DELETE CASCADE,
                path TEXT NOT NULL,
                hash TEXT NOT NULL,
                snapshot_file TEXT NOT NULL,
                UNIQUE(checkpoint_id, path)
            );
            
            CREATE INDEX IF NOT EXISTS idx_checkpoints_session 
            ON checkpoints(session_id, timestamp DESC);
            
            CREATE INDEX IF NOT EXISTS idx_checkpoint_files_checkpoint
            ON checkpoint_files(checkpoint_id);
            "#,
        )?;
        Ok(())
    }

    /// Create a checkpoint of current workspace state
    pub async fn create_checkpoint(
        &self,
        session_id: &str,
        description: String,
        message_id: Option<String>,
    ) -> Result<Checkpoint> {
        let checkpoint_id = Uuid::new_v4().to_string();
        let timestamp = Utc::now();

        let files = self.snapshot_workspace(&checkpoint_id).await?;

        let file_count = files.len() as i32;

        self.conn.execute(
            r#"
            INSERT INTO checkpoints (id, session_id, message_id, description, timestamp, file_count)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            params![
                checkpoint_id,
                session_id,
                message_id,
                description,
                timestamp.to_rfc3339(),
                file_count,
            ],
        )?;

        for file in &files {
            let snapshot_file = self.save_file_snapshot(&checkpoint_id, &file).await?;

            self.conn.execute(
                r#"
                INSERT INTO checkpoint_files (checkpoint_id, path, hash, snapshot_file)
                VALUES (?1, ?2, ?3, ?4)
                "#,
                params![
                    checkpoint_id,
                    file.path.to_string_lossy().to_string(),
                    file.hash,
                    snapshot_file,
                ],
            )?;
        }

        Ok(Checkpoint {
            id: checkpoint_id,
            session_id: session_id.to_string(),
            message_id,
            description,
            timestamp,
            files,
        })
    }

    /// Snapshot all tracked files in workspace
    async fn snapshot_workspace(&self, _checkpoint_id: &str) -> Result<Vec<FileSnapshot>> {
        let mut files = Vec::new();

        let tracked_files = self.find_tracked_files()?;

        for file_path in tracked_files {
            if let Ok(snapshot) = self.snapshot_file(&file_path).await {
                files.push(snapshot);
            }
        }

        Ok(files)
    }

    /// Find all tracked files in workspace
    fn find_tracked_files(&self) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        self.collect_files(&self.workspace_root.clone(), &mut files)?;
        Ok(files)
    }

    /// Recursively collect files
    fn collect_files(&self, dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
        if !dir.exists() {
            return Ok(());
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

                if dir_name.starts_with('.') || dir_name == "target" || dir_name == "node_modules" {
                    continue;
                }

                self.collect_files(&path, files)?;
            } else if path.is_file() {
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

                if !ext.is_empty()
                    || path.file_name().map(|f| f.to_str()) == Some(Some("Cargo.toml"))
                {
                    files.push(path);
                }
            }
        }

        Ok(())
    }

    /// Snapshot a single file
    async fn snapshot_file(&self, path: &Path) -> Result<FileSnapshot> {
        let content = fs::read_to_string(path)?;
        let hash = self.hash_content(&content);

        Ok(FileSnapshot {
            path: path.to_path_buf(),
            hash,
            content: Some(content),
        })
    }

    /// Hash file content
    fn hash_content(&self, content: &str) -> String {
        let mut hasher = Sha3_256::new();
        hasher.update(content.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Save file snapshot to disk
    async fn save_file_snapshot(
        &self,
        checkpoint_id: &str,
        snapshot: &FileSnapshot,
    ) -> Result<String> {
        let checkpoint_dir = self.snapshot_dir.join(checkpoint_id);
        fs::create_dir_all(&checkpoint_dir)?;

        let relative_path = snapshot
            .path
            .strip_prefix(&self.workspace_root)
            .unwrap_or(&snapshot.path);

        let snapshot_file = relative_path.to_string_lossy().replace(['/', '\\'], "_") + ".json";

        let snapshot_path = checkpoint_dir.join(&snapshot_file);

        let content = snapshot.content.clone().unwrap_or_default();
        fs::write(&snapshot_path, content)?;

        Ok(snapshot_file)
    }

    /// List all checkpoints for a session
    pub fn list_checkpoints(&self, session_id: &str) -> Result<Vec<Checkpoint>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, session_id, message_id, description, timestamp
            FROM checkpoints
            WHERE session_id = ?1
            ORDER BY timestamp DESC
            "#,
        )?;

        let checkpoints = stmt
            .query_map(params![session_id], |row| {
                Ok(Checkpoint {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    message_id: row.get(2)?,
                    description: row.get(3)?,
                    timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                    files: Vec::new(),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(checkpoints)
    }

    /// Get a specific checkpoint by ID
    pub fn get_checkpoint(&self, checkpoint_id: &str) -> Result<Option<Checkpoint>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, session_id, message_id, description, timestamp
            FROM checkpoints
            WHERE id = ?1
            "#,
        )?;

        let checkpoint = stmt
            .query_row(params![checkpoint_id], |row| {
                Ok(Checkpoint {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    message_id: row.get(2)?,
                    description: row.get(3)?,
                    timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                    files: Vec::new(),
                })
            })
            .optional()?;

        if let Some(mut checkpoint) = checkpoint {
            checkpoint.files = self.load_checkpoint_files(checkpoint_id)?;
            Ok(Some(checkpoint))
        } else {
            Ok(None)
        }
    }

    /// Load file snapshots for a checkpoint
    fn load_checkpoint_files(&self, checkpoint_id: &str) -> Result<Vec<FileSnapshot>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT path, hash, snapshot_file
            FROM checkpoint_files
            WHERE checkpoint_id = ?1
            "#,
        )?;

        let files = stmt
            .query_map(params![checkpoint_id], |row| {
                let path_str: String = row.get(0)?;
                let hash: String = row.get(1)?;
                let snapshot_file: String = row.get(2)?;

                let snapshot_path = self.snapshot_dir.join(checkpoint_id).join(&snapshot_file);
                let content = fs::read_to_string(&snapshot_path).ok();

                Ok(FileSnapshot {
                    path: PathBuf::from(path_str),
                    hash,
                    content,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(files)
    }

    /// Restore workspace to a checkpoint
    pub async fn restore_checkpoint(&self, checkpoint_id: &str) -> Result<()> {
        let checkpoint = self
            .get_checkpoint(checkpoint_id)?
            .ok_or_else(|| Error::NotFound(format!("Checkpoint {} not found", checkpoint_id)))?;

        let checkpoint_paths: std::collections::HashSet<_> =
            checkpoint.files.iter().map(|f| f.path.clone()).collect();

        let current_files = self.find_tracked_files()?;

        for file_path in &current_files {
            if !checkpoint_paths.contains(file_path) {
                if file_path.exists() {
                    fs::remove_file(file_path)?;
                }
            }
        }

        for file_snapshot in &checkpoint.files {
            if let Some(content) = &file_snapshot.content {
                if let Some(parent) = file_snapshot.path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(&file_snapshot.path, content)?;
            }
        }

        Ok(())
    }

    /// Compare two checkpoints
    pub fn compare_checkpoints(
        &self,
        checkpoint_id1: &str,
        checkpoint_id2: &str,
    ) -> Result<CheckpointDiff> {
        let cp1 = self
            .get_checkpoint(checkpoint_id1)?
            .ok_or_else(|| Error::NotFound(format!("Checkpoint {} not found", checkpoint_id1)))?;

        let cp2 = self
            .get_checkpoint(checkpoint_id2)?
            .ok_or_else(|| Error::NotFound(format!("Checkpoint {} not found", checkpoint_id2)))?;

        let mut file_diffs = HashMap::new();

        let files1: HashMap<PathBuf, &FileSnapshot> =
            cp1.files.iter().map(|f| (f.path.clone(), f)).collect();

        let files2: HashMap<PathBuf, &FileSnapshot> =
            cp2.files.iter().map(|f| (f.path.clone(), f)).collect();

        for (path, snapshot1) in &files1 {
            if let Some(snapshot2) = files2.get(path) {
                if snapshot1.hash != snapshot2.hash {
                    let diff = self.diff_files(snapshot1, snapshot2)?;
                    file_diffs.insert(path.clone(), FileChange::Modified(diff));
                }
            } else {
                file_diffs.insert(path.clone(), FileChange::Deleted);
            }
        }

        for (path, _snapshot2) in &files2 {
            if !files1.contains_key(path) {
                file_diffs.insert(path.clone(), FileChange::Added);
            }
        }

        Ok(CheckpointDiff {
            checkpoint1_id: checkpoint_id1.to_string(),
            checkpoint2_id: checkpoint_id2.to_string(),
            file_diffs,
        })
    }

    /// Diff two file snapshots
    fn diff_files(&self, file1: &FileSnapshot, file2: &FileSnapshot) -> Result<Diff> {
        let content1 = file1.content.as_deref().unwrap_or("");
        let content2 = file2.content.as_deref().unwrap_or("");

        let lines1: Vec<&str> = content1.lines().collect();
        let lines2: Vec<&str> = content2.lines().collect();

        let hunks = self.compute_diff_hunks(&lines1, &lines2);

        Ok(Diff { hunks })
    }

    /// Compute diff hunks using simple LCS algorithm
    fn compute_diff_hunks(&self, lines1: &[&str], lines2: &[&str]) -> Vec<DiffHunk> {
        let mut hunks = Vec::new();

        if lines1.is_empty() && lines2.is_empty() {
            return hunks;
        }

        let mut old_line = 0;
        let mut new_line = 0;
        let mut i = 0;
        let mut j = 0;

        while i < lines1.len() || j < lines2.len() {
            let mut diff_lines = Vec::new();
            let start_old = old_line + 1;
            let start_new = new_line + 1;
            let mut old_count = 0;
            let mut new_count = 0;

            while i < lines1.len() && j < lines2.len() && lines1[i] == lines2[j] {
                i += 1;
                j += 1;
                old_line += 1;
                new_line += 1;
            }

            while i < lines1.len() && (j >= lines2.len() || lines1[i] != lines2[j]) {
                diff_lines.push(DiffLine {
                    ty: DiffLineType::Deletion,
                    content: lines1[i].to_string(),
                });
                i += 1;
                old_line += 1;
                old_count += 1;
            }

            while j < lines2.len() && (i >= lines1.len() || lines1[i] != lines2[j]) {
                diff_lines.push(DiffLine {
                    ty: DiffLineType::Addition,
                    content: lines2[j].to_string(),
                });
                j += 1;
                new_line += 1;
                new_count += 1;
            }

            if !diff_lines.is_empty() {
                hunks.push(DiffHunk {
                    old_start: start_old,
                    old_lines: old_count,
                    new_start: start_new,
                    new_lines: new_count,
                    lines: diff_lines,
                });
            }
        }

        hunks
    }

    /// Delete a checkpoint
    pub fn delete_checkpoint(&self, checkpoint_id: &str) -> Result<()> {
        let checkpoint_dir = self.snapshot_dir.join(checkpoint_id);
        if checkpoint_dir.exists() {
            fs::remove_dir_all(checkpoint_dir)?;
        }

        self.conn.execute(
            "DELETE FROM checkpoints WHERE id = ?1",
            params![checkpoint_id],
        )?;

        Ok(())
    }

    /// Auto-create checkpoint before file edit
    pub async fn auto_checkpoint(
        &self,
        session_id: &str,
        file_path: &Path,
    ) -> Result<Option<Checkpoint>> {
        let description = format!("Auto-checkpoint before editing {}", file_path.display());
        let checkpoint = self
            .create_checkpoint(session_id, description, None)
            .await?;
        Ok(Some(checkpoint))
    }

    /// Get timeline of checkpoints for a session
    pub fn get_timeline(&self, session_id: &str) -> Result<Timeline> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, session_id, message_id, description, timestamp, file_count
            FROM checkpoints
            WHERE session_id = ?1
            ORDER BY timestamp ASC
            "#,
        )?;

        let checkpoints = stmt
            .query_map(params![session_id], |row| {
                Ok(CheckpointSummary {
                    id: row.get(0)?,
                    message_id: row.get(2)?,
                    description: row.get(3)?,
                    timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                    file_count: row.get::<_, i32>(5)? as usize,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(Timeline {
            session_id: session_id.to_string(),
            current_index: if checkpoints.is_empty() {
                None
            } else {
                Some(checkpoints.len() - 1)
            },
            checkpoints,
        })
    }

    /// Get summary of a specific checkpoint
    pub fn get_checkpoint_summary(&self, checkpoint_id: &str) -> Result<Option<CheckpointSummary>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, session_id, message_id, description, timestamp, file_count
            FROM checkpoints
            WHERE id = ?1
            "#,
        )?;

        let summary = stmt
            .query_row(params![checkpoint_id], |row| {
                Ok(CheckpointSummary {
                    id: row.get(0)?,
                    message_id: row.get(2)?,
                    description: row.get(3)?,
                    timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                    file_count: row.get::<_, i32>(5)? as usize,
                })
            })
            .optional()?;

        Ok(summary)
    }

    /// Clean up old checkpoints, keeping only the N most recent
    pub fn cleanup_old_checkpoints(&self, session_id: &str, keep_count: usize) -> Result<usize> {
        let checkpoints = self.list_checkpoints(session_id)?;

        if checkpoints.len() <= keep_count {
            return Ok(0);
        }

        let to_delete = &checkpoints[keep_count..];
        let mut deleted = 0;

        for checkpoint in to_delete {
            self.delete_checkpoint(&checkpoint.id)?;
            deleted += 1;
        }

        Ok(deleted)
    }
}

/// Result of comparing two checkpoints
#[derive(Debug, Serialize, Deserialize)]
pub struct CheckpointDiff {
    /// First checkpoint ID
    pub checkpoint1_id: String,
    /// Second checkpoint ID
    pub checkpoint2_id: String,
    /// File changes
    pub file_diffs: HashMap<PathBuf, FileChange>,
}

/// A change to a file between checkpoints
#[derive(Debug, Serialize, Deserialize)]
pub enum FileChange {
    /// File was added
    Added,
    /// File was deleted
    Deleted,
    /// File was modified
    Modified(Diff),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_content() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let manager = CheckpointManager::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

        let hash1 = manager.hash_content("test content");
        let hash2 = manager.hash_content("test content");
        let hash3 = manager.hash_content("different content");

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_compute_diff_hunks() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let manager = CheckpointManager::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

        let lines1 = vec!["line 1", "line 2", "line 3"];
        let lines2 = vec!["line 1", "modified line 2", "line 3"];

        let hunks = manager.compute_diff_hunks(&lines1, &lines2);

        assert!(!hunks.is_empty());
    }
}
