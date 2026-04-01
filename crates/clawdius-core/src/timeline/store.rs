//! SQLite-backed storage for timeline data

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::{CheckpointId, Diff, DiffSummary, FileChangeType, FileDiff};
use crate::error::{Error, Result};

/// Storage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageStats {
    /// Number of checkpoints
    pub checkpoint_count: usize,
    /// Number of tracked files
    pub tracked_file_count: usize,
    /// Total storage size in bytes
    pub total_size_bytes: usize,
    /// Total file versions
    pub version_count: usize,
}

/// Checkpoint information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointInfo {
    /// Checkpoint ID
    pub id: CheckpointId,
    /// Checkpoint name
    pub name: String,
    /// Optional description
    pub description: Option<String>,
    /// Creation timestamp
    pub timestamp: DateTime<Utc>,
    /// Number of files in checkpoint
    pub files_count: usize,
    /// Total size in bytes
    pub total_size: usize,
}

/// File version information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileVersion {
    /// File path
    pub path: PathBuf,
    /// Version number
    pub version: u64,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Content hash
    pub checksum: String,
    /// File size in bytes
    pub size: usize,
    /// Checkpoint ID where this version was created
    pub checkpoint_id: CheckpointId,
}

/// Internal checkpoint structure with file data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineCheckpoint {
    /// Checkpoint info
    pub info: CheckpointInfo,
    /// File snapshots
    pub files: Vec<FileSnapshot>,
}

/// A snapshot of a file at a point in time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSnapshot {
    /// File path
    path: PathBuf,
    /// Content hash
    hash: String,
    /// File size
    size: usize,
    /// Whether the file is binary
    is_binary: bool,
    /// File content (stored separately)
    #[serde(skip)]
    #[allow(dead_code)]
    content_path: Option<PathBuf>,
}

/// Timeline storage backend
pub struct TimelineStore {
    conn: Connection,
    workspace_root: PathBuf,
    snapshot_dir: PathBuf,
}

impl TimelineStore {
    /// Create a new timeline store
    pub fn new(db_path: &Path, workspace_root: PathBuf) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(db_path)?;
        let snapshot_dir = workspace_root.join(".clawdius").join("timeline_snapshots");
        fs::create_dir_all(&snapshot_dir)?;

        let store = Self {
            conn,
            workspace_root,
            snapshot_dir,
        };

        store.initialize()?;
        Ok(store)
    }

    /// Initialize database schema
    fn initialize(&self) -> Result<()> {
        self.conn.execute_batch(
            r"
            CREATE TABLE IF NOT EXISTS checkpoints (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT,
                timestamp TEXT NOT NULL,
                files_count INTEGER NOT NULL DEFAULT 0,
                total_size INTEGER NOT NULL DEFAULT 0
            );
            
            CREATE TABLE IF NOT EXISTS tracked_files (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                path TEXT NOT NULL UNIQUE,
                first_seen TEXT NOT NULL,
                last_modified TEXT NOT NULL
            );
            
            CREATE TABLE IF NOT EXISTS file_versions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                checkpoint_id TEXT NOT NULL REFERENCES checkpoints(id) ON DELETE CASCADE,
                file_path TEXT NOT NULL,
                version INTEGER NOT NULL,
                hash TEXT NOT NULL,
                size INTEGER NOT NULL,
                snapshot_path TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                is_binary INTEGER NOT NULL DEFAULT 0,
                UNIQUE(checkpoint_id, file_path)
            );
            
            CREATE INDEX IF NOT EXISTS idx_checkpoints_timestamp 
            ON checkpoints(timestamp DESC);
            
            CREATE INDEX IF NOT EXISTS idx_file_versions_checkpoint
            ON file_versions(checkpoint_id);
            
            CREATE INDEX IF NOT EXISTS idx_file_versions_path
            ON file_versions(file_path, timestamp DESC);
            ",
        )?;

        self.conn
            .execute(
                "ALTER TABLE file_versions ADD COLUMN is_binary INTEGER NOT NULL DEFAULT 0",
                [],
            )
            .ok();

        Ok(())
    }

    /// Track a file
    pub fn track_file(&self, path: &Path) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let path_str = path.to_string_lossy().to_string();

        self.conn.execute(
            r"
            INSERT INTO tracked_files (path, first_seen, last_modified)
            VALUES (?1, ?2, ?2)
            ON CONFLICT(path) DO UPDATE SET last_modified = ?2
            ",
            params![path_str, now],
        )?;

        Ok(())
    }

    /// Create a checkpoint
    pub async fn create_checkpoint(
        &mut self,
        name: &str,
        description: Option<&str>,
    ) -> Result<CheckpointId> {
        let checkpoint_id = CheckpointId::new();
        let timestamp = Utc::now();

        let files = self.snapshot_workspace().await?;
        let files_count = files.len() as i32;
        let total_size: i32 = files.iter().map(|f| f.size as i32).sum();

        self.conn.execute(
            r"
            INSERT INTO checkpoints (id, name, description, timestamp, files_count, total_size)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ",
            params![
                checkpoint_id.0,
                name,
                description,
                timestamp.to_rfc3339(),
                files_count,
                total_size,
            ],
        )?;

        let mut version = 0;
        for file in &files {
            version += 1;
            let snapshot_path = self.save_file_snapshot(&checkpoint_id, file).await?;

            self.conn.execute(
                r"
                INSERT INTO file_versions (checkpoint_id, file_path, version, hash, size, snapshot_path, timestamp, is_binary)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                ",
                params![
                    checkpoint_id.0,
                    file.path.to_string_lossy().to_string(),
                    version,
                    file.hash,
                    file.size as i32,
                    snapshot_path.to_string_lossy().to_string(),
                    timestamp.to_rfc3339(),
                    i32::from(file.is_binary),
                ],
            )?;
        }

        Ok(checkpoint_id)
    }

    /// Snapshot all tracked files in workspace
    async fn snapshot_workspace(&self) -> Result<Vec<FileSnapshot>> {
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

    /// Check if a file is binary by examining its contents
    fn is_binary_file(content: &[u8]) -> bool {
        if content.is_empty() {
            return false;
        }

        let sample_size = content.len().min(8192);
        let sample = &content[..sample_size];

        if sample.contains(&0) {
            return true;
        }

        let non_printable = sample
            .iter()
            .filter(|&&b| b < 0x20 && b != 0x09 && b != 0x0A && b != 0x0D)
            .count();

        non_printable > sample_size / 10
    }

    /// Snapshot a single file
    async fn snapshot_file(&self, path: &Path) -> Result<FileSnapshot> {
        let metadata = fs::metadata(path)?;
        let size = metadata.len() as usize;

        let content = fs::read(path)?;
        let is_binary = Self::is_binary_file(&content);
        let hash = self.hash_bytes(&content);

        Ok(FileSnapshot {
            path: path.to_path_buf(),
            hash,
            size,
            is_binary,
            content_path: None,
        })
    }

    /// Hash file content (string)
    #[allow(dead_code)]
    fn hash_content(&self, content: &str) -> String {
        self.hash_bytes(content.as_bytes())
    }

    /// Hash bytes
    fn hash_bytes(&self, content: &[u8]) -> String {
        let mut hasher = Sha3_256::new();
        hasher.update(content);
        format!("{:x}", hasher.finalize())
    }

    /// Save file snapshot to disk
    async fn save_file_snapshot(
        &self,
        checkpoint_id: &CheckpointId,
        snapshot: &FileSnapshot,
    ) -> Result<PathBuf> {
        let checkpoint_dir = self.snapshot_dir.join(&checkpoint_id.0);
        fs::create_dir_all(&checkpoint_dir)?;

        let relative_path = snapshot
            .path
            .strip_prefix(&self.workspace_root)
            .unwrap_or(&snapshot.path);

        let extension = if snapshot.is_binary { ".bin" } else { ".txt" };
        let snapshot_file = relative_path.to_string_lossy().replace(['/', '\\'], "_") + extension;

        let snapshot_path = checkpoint_dir.join(&snapshot_file);

        if snapshot.path.exists() {
            let content = fs::read(&snapshot.path)?;
            fs::write(&snapshot_path, content)?;
        }

        Ok(snapshot_path)
    }

    /// List all checkpoints
    pub fn list_checkpoints(&self) -> Result<Vec<CheckpointInfo>> {
        let mut stmt = self.conn.prepare(
            r"
            SELECT id, name, description, timestamp, files_count, total_size
            FROM checkpoints
            ORDER BY timestamp DESC
            ",
        )?;

        let checkpoints = stmt
            .query_map([], |row| {
                Ok(CheckpointInfo {
                    id: CheckpointId(row.get(0)?),
                    name: row.get(1)?,
                    description: row.get(2)?,
                    timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(3)?)
                        .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc)),
                    files_count: row.get::<_, i32>(4)? as usize,
                    total_size: row.get::<_, i32>(5)? as usize,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(checkpoints)
    }

    /// Get a specific checkpoint
    pub fn get_checkpoint(&self, id: &CheckpointId) -> Result<Option<CheckpointInfo>> {
        let mut stmt = self.conn.prepare(
            r"
            SELECT id, name, description, timestamp, files_count, total_size
            FROM checkpoints
            WHERE id = ?1
            ",
        )?;

        let checkpoint = stmt
            .query_row(params![id.0], |row| {
                Ok(CheckpointInfo {
                    id: CheckpointId(row.get(0)?),
                    name: row.get(1)?,
                    description: row.get(2)?,
                    timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(3)?)
                        .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc)),
                    files_count: row.get::<_, i32>(4)? as usize,
                    total_size: row.get::<_, i32>(5)? as usize,
                })
            })
            .optional()?;

        Ok(checkpoint)
    }

    /// Get file history
    pub fn get_file_history(&self, path: &Path) -> Result<Vec<FileVersion>> {
        let path_str = path.to_string_lossy().to_string();

        let mut stmt = self.conn.prepare(
            r"
            SELECT file_path, version, hash, size, timestamp, checkpoint_id
            FROM file_versions
            WHERE file_path = ?1
            ORDER BY timestamp DESC
            ",
        )?;

        let versions = stmt
            .query_map(params![path_str], |row| {
                Ok(FileVersion {
                    path: PathBuf::from(row.get::<_, String>(0)?),
                    version: row.get::<_, i32>(1)? as u64,
                    checksum: row.get(2)?,
                    size: row.get::<_, i32>(3)? as usize,
                    timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)
                        .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc)),
                    checkpoint_id: CheckpointId(row.get(5)?),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(versions)
    }

    /// Rollback to a checkpoint
    pub async fn rollback(&self, checkpoint_id: &CheckpointId) -> Result<()> {
        let _checkpoint = self
            .get_checkpoint(checkpoint_id)?
            .ok_or_else(|| Error::NotFound(format!("Checkpoint {} not found", checkpoint_id.0)))?;

        let mut stmt = self.conn.prepare(
            r"
            SELECT file_path, snapshot_path
            FROM file_versions
            WHERE checkpoint_id = ?1
            ",
        )?;

        let file_snapshots = stmt
            .query_map(params![checkpoint_id.0], |row| {
                Ok((
                    PathBuf::from(row.get::<_, String>(0)?),
                    PathBuf::from(row.get::<_, String>(1)?),
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let checkpoint_paths: std::collections::HashSet<_> =
            file_snapshots.iter().map(|(p, _)| p.clone()).collect();

        let current_files = self.find_tracked_files()?;

        for file_path in &current_files {
            if !checkpoint_paths.contains(file_path) && file_path.exists() {
                fs::remove_file(file_path)?;
            }
        }

        for (file_path, snapshot_path) in &file_snapshots {
            if snapshot_path.exists() {
                let content = fs::read(snapshot_path)?;
                if let Some(parent) = file_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(file_path, content)?;
            }
        }

        Ok(())
    }

    /// Diff two checkpoints
    pub fn diff_checkpoints(&self, from: &CheckpointId, to: &CheckpointId) -> Result<Diff> {
        let from_files = self.get_checkpoint_files(from)?;
        let to_files = self.get_checkpoint_files(to)?;

        let mut files_changed = Vec::new();
        let mut total_additions = 0;
        let mut total_deletions = 0;

        let from_map: HashMap<PathBuf, FileSnapshot> = from_files
            .into_iter()
            .map(|f| (f.path.clone(), f))
            .collect();
        let to_map: HashMap<PathBuf, FileSnapshot> =
            to_files.into_iter().map(|f| (f.path.clone(), f)).collect();

        for (path, from_file) in &from_map {
            if let Some(to_file) = to_map.get(path) {
                if from_file.hash != to_file.hash {
                    let (additions, deletions) =
                        self.compute_diff_stats(&from_file.path, from, to)?;
                    files_changed.push(FileDiff {
                        path: path.clone(),
                        change_type: FileChangeType::Modified,
                        additions,
                        deletions,
                    });
                    total_additions += additions;
                    total_deletions += deletions;
                }
            } else {
                files_changed.push(FileDiff {
                    path: path.clone(),
                    change_type: FileChangeType::Deleted,
                    additions: 0,
                    deletions: 0,
                });
            }
        }

        for path in to_map.keys() {
            if !from_map.contains_key(path) {
                files_changed.push(FileDiff {
                    path: path.clone(),
                    change_type: FileChangeType::Added,
                    additions: 0,
                    deletions: 0,
                });
            }
        }

        Ok(Diff {
            from: from.clone(),
            to: to.clone(),
            summary: DiffSummary {
                total_files: files_changed.len(),
                total_additions,
                total_deletions,
            },
            files_changed,
        })
    }

    /// Get files for a checkpoint
    fn get_checkpoint_files(&self, checkpoint_id: &CheckpointId) -> Result<Vec<FileSnapshot>> {
        let mut stmt = self.conn.prepare(
            r"
            SELECT file_path, hash, size, snapshot_path, is_binary
            FROM file_versions
            WHERE checkpoint_id = ?1
            ",
        )?;

        let files = stmt
            .query_map(params![checkpoint_id.0], |row| {
                Ok(FileSnapshot {
                    path: PathBuf::from(row.get::<_, String>(0)?),
                    hash: row.get(1)?,
                    size: row.get::<_, i32>(2)? as usize,
                    content_path: Some(PathBuf::from(row.get::<_, String>(3)?)),
                    is_binary: row.get::<_, i32>(4)? != 0,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(files)
    }

    /// Compute diff stats for a file
    fn compute_diff_stats(
        &self,
        path: &Path,
        from: &CheckpointId,
        to: &CheckpointId,
    ) -> Result<(usize, usize)> {
        let from_content = self.get_file_content_at_checkpoint(path, from)?;
        let to_content = self.get_file_content_at_checkpoint(path, to)?;

        let (additions, deletions) = match (from_content, to_content) {
            (Some(from), Some(to)) => self.compute_line_diff(&from, &to),
            (None, Some(_)) => (0, 0),
            (Some(_), None) => (0, 0),
            (None, None) => (0, 0),
        };

        Ok((additions, deletions))
    }

    /// Get file content at a specific checkpoint
    fn get_file_content_at_checkpoint(
        &self,
        path: &Path,
        checkpoint_id: &CheckpointId,
    ) -> Result<Option<String>> {
        let mut stmt = self.conn.prepare(
            r"
            SELECT snapshot_path, is_binary
            FROM file_versions
            WHERE checkpoint_id = ?1 AND file_path = ?2
            ",
        )?;

        let result = stmt
            .query_row(
                params![checkpoint_id.0, path.to_string_lossy().to_string()],
                |row| {
                    Ok((
                        PathBuf::from(row.get::<_, String>(0)?),
                        row.get::<_, i32>(1)? != 0,
                    ))
                },
            )
            .optional()?;

        if let Some((snapshot_path, is_binary)) = result {
            if is_binary {
                return Ok(None);
            }
            if snapshot_path.exists() {
                let content = fs::read_to_string(&snapshot_path)?;
                return Ok(Some(content));
            }
        }

        Ok(None)
    }

    /// Compute line diff statistics
    fn compute_line_diff(&self, from: &str, to: &str) -> (usize, usize) {
        use similar::{ChangeTag, TextDiff};

        let diff = TextDiff::from_lines(from, to);
        let mut additions = 0;
        let mut deletions = 0;

        for change in diff.iter_all_changes() {
            match change.tag() {
                ChangeTag::Insert => additions += 1,
                ChangeTag::Delete => deletions += 1,
                ChangeTag::Equal => {},
            }
        }

        (additions, deletions)
    }

    /// Delete a checkpoint
    pub fn delete_checkpoint(&mut self, checkpoint_id: &CheckpointId) -> Result<()> {
        let checkpoint_dir = self.snapshot_dir.join(&checkpoint_id.0);
        if checkpoint_dir.exists() {
            fs::remove_dir_all(checkpoint_dir)?;
        }

        self.conn.execute(
            "DELETE FROM checkpoints WHERE id = ?1",
            params![checkpoint_id.0],
        )?;

        Ok(())
    }

    /// Cleanup old checkpoints
    pub fn cleanup_old_checkpoints(&mut self, keep_count: usize) -> Result<usize> {
        let checkpoints = self.list_checkpoints()?;

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

    /// Query checkpoints by time range
    pub fn query_by_time_range(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<CheckpointInfo>> {
        let mut stmt = self.conn.prepare(
            r"
            SELECT id, name, description, timestamp, files_count, total_size
            FROM checkpoints
            WHERE timestamp >= ?1 AND timestamp <= ?2
            ORDER BY timestamp DESC
            ",
        )?;

        let checkpoints = stmt
            .query_map(params![start.to_rfc3339(), end.to_rfc3339()], |row| {
                Ok(CheckpointInfo {
                    id: CheckpointId(row.get(0)?),
                    name: row.get(1)?,
                    description: row.get(2)?,
                    timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(3)?)
                        .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc)),
                    files_count: row.get::<_, i32>(4)? as usize,
                    total_size: row.get::<_, i32>(5)? as usize,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(checkpoints)
    }

    /// Query checkpoints by name pattern (supports SQL LIKE patterns)
    pub fn query_by_name(&self, pattern: &str) -> Result<Vec<CheckpointInfo>> {
        let mut stmt = self.conn.prepare(
            r"
            SELECT id, name, description, timestamp, files_count, total_size
            FROM checkpoints
            WHERE name LIKE ?1
            ORDER BY timestamp DESC
            ",
        )?;

        let search_pattern = format!("%{pattern}%");
        let checkpoints = stmt
            .query_map(params![search_pattern], |row| {
                Ok(CheckpointInfo {
                    id: CheckpointId(row.get(0)?),
                    name: row.get(1)?,
                    description: row.get(2)?,
                    timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(3)?)
                        .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc)),
                    files_count: row.get::<_, i32>(4)? as usize,
                    total_size: row.get::<_, i32>(5)? as usize,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(checkpoints)
    }

    /// Get file version at a specific checkpoint
    pub fn get_file_version_at_checkpoint(
        &self,
        path: &Path,
        checkpoint_id: &CheckpointId,
    ) -> Result<Option<FileVersion>> {
        let path_str = path.to_string_lossy().to_string();

        let mut stmt = self.conn.prepare(
            r"
            SELECT file_path, version, hash, size, timestamp, checkpoint_id
            FROM file_versions
            WHERE checkpoint_id = ?1 AND file_path = ?2
            ",
        )?;

        let version = stmt
            .query_row(params![checkpoint_id.0, path_str], |row| {
                Ok(FileVersion {
                    path: PathBuf::from(row.get::<_, String>(0)?),
                    version: row.get::<_, i32>(1)? as u64,
                    checksum: row.get(2)?,
                    size: row.get::<_, i32>(3)? as usize,
                    timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)
                        .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc)),
                    checkpoint_id: CheckpointId(row.get(5)?),
                })
            })
            .optional()?;

        Ok(version)
    }

    /// Get list of files changed between two checkpoints
    pub fn get_files_changed_between(
        &self,
        from: &CheckpointId,
        to: &CheckpointId,
    ) -> Result<Vec<(PathBuf, FileChangeType)>> {
        let from_files = self.get_checkpoint_files(from)?;
        let to_files = self.get_checkpoint_files(to)?;

        let mut changes = Vec::new();

        let from_map: HashMap<PathBuf, &FileSnapshot> =
            from_files.iter().map(|f| (f.path.clone(), f)).collect();
        let to_map: HashMap<PathBuf, &FileSnapshot> =
            to_files.iter().map(|f| (f.path.clone(), f)).collect();

        for (path, from_file) in &from_map {
            if let Some(to_file) = to_map.get(path) {
                if from_file.hash != to_file.hash {
                    changes.push((path.clone(), FileChangeType::Modified));
                }
            } else {
                changes.push((path.clone(), FileChangeType::Deleted));
            }
        }

        for path in to_map.keys() {
            if !from_map.contains_key(path) {
                changes.push((path.clone(), FileChangeType::Added));
            }
        }

        Ok(changes)
    }

    /// Get total number of checkpoints
    pub fn checkpoint_count(&self) -> Result<usize> {
        let count: i32 = self
            .conn
            .query_row("SELECT COUNT(*) FROM checkpoints", [], |row| row.get(0))?;

        Ok(count as usize)
    }

    /// Get total number of tracked files
    pub fn tracked_file_count(&self) -> Result<usize> {
        let count: i32 = self
            .conn
            .query_row("SELECT COUNT(*) FROM tracked_files", [], |row| row.get(0))?;

        Ok(count as usize)
    }

    /// Get storage statistics
    pub fn storage_stats(&self) -> Result<StorageStats> {
        let checkpoint_count = self.checkpoint_count()?;
        let tracked_file_count = self.tracked_file_count()?;

        let total_size: i64 = self.conn.query_row(
            "SELECT COALESCE(SUM(total_size), 0) FROM checkpoints",
            [],
            |row| row.get(0),
        )?;

        let version_count: i32 =
            self.conn
                .query_row("SELECT COUNT(*) FROM file_versions", [], |row| row.get(0))?;

        Ok(StorageStats {
            checkpoint_count,
            tracked_file_count,
            total_size_bytes: total_size as usize,
            version_count: version_count as usize,
        })
    }

    /// Rollback specific files to a checkpoint
    pub async fn rollback_files(
        &self,
        checkpoint_id: &CheckpointId,
        files: &[PathBuf],
    ) -> Result<()> {
        let _checkpoint = self
            .get_checkpoint(checkpoint_id)?
            .ok_or_else(|| Error::NotFound(format!("Checkpoint {} not found", checkpoint_id.0)))?;

        let files_set: std::collections::HashSet<_> = files.iter().collect();

        let mut stmt = self.conn.prepare(
            r"
            SELECT file_path, snapshot_path
            FROM file_versions
            WHERE checkpoint_id = ?1
            ",
        )?;

        let file_snapshots = stmt
            .query_map(params![checkpoint_id.0], |row| {
                Ok((
                    PathBuf::from(row.get::<_, String>(0)?),
                    PathBuf::from(row.get::<_, String>(1)?),
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        for (file_path, snapshot_path) in &file_snapshots {
            if files_set.contains(file_path) && snapshot_path.exists() {
                let content = fs::read(snapshot_path)?;
                if let Some(parent) = file_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(file_path, content)?;
            }
        }

        Ok(())
    }

    /// Preview a rollback operation (dry-run)
    pub fn preview_rollback(&self, checkpoint_id: &CheckpointId) -> Result<super::RollbackPreview> {
        let checkpoint = self
            .get_checkpoint(checkpoint_id)?
            .ok_or_else(|| Error::NotFound(format!("Checkpoint {} not found", checkpoint_id.0)))?;

        let mut stmt = self.conn.prepare(
            r"
            SELECT file_path, snapshot_path, hash
            FROM file_versions
            WHERE checkpoint_id = ?1
            ",
        )?;

        let checkpoint_files: Vec<(PathBuf, PathBuf, String)> = stmt
            .query_map(params![checkpoint_id.0], |row| {
                Ok((
                    PathBuf::from(row.get::<_, String>(0)?),
                    PathBuf::from(row.get::<_, String>(1)?),
                    row.get(2)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let checkpoint_paths: std::collections::HashSet<_> =
            checkpoint_files.iter().map(|(p, _, _)| p.clone()).collect();

        let current_files = self.find_tracked_files()?;

        let mut files_to_restore = Vec::new();
        let mut files_to_delete = Vec::new();
        let mut files_modified = Vec::new();

        for file_path in &current_files {
            if !checkpoint_paths.contains(file_path) {
                files_to_delete.push(file_path.clone());
            }
        }

        for (file_path, _snapshot_path, checkpoint_hash) in &checkpoint_files {
            if file_path.exists() {
                let current_content = fs::read(file_path).unwrap_or_default();
                let current_hash = self.hash_bytes(&current_content);

                if current_hash != *checkpoint_hash {
                    files_modified.push(file_path.clone());
                }
                files_to_restore.push(file_path.clone());
            } else {
                files_to_restore.push(file_path.clone());
            }
        }

        let total_files_affected = files_to_restore.len() + files_to_delete.len();

        Ok(super::RollbackPreview {
            checkpoint_id: checkpoint.id,
            files_to_restore,
            files_to_delete,
            files_modified,
            total_files_affected,
        })
    }

    /// Export a checkpoint to a portable format
    pub fn export_checkpoint(
        &self,
        checkpoint_id: &CheckpointId,
    ) -> Result<super::ExportedCheckpoint> {
        let checkpoint = self
            .get_checkpoint(checkpoint_id)?
            .ok_or_else(|| Error::NotFound(format!("Checkpoint {} not found", checkpoint_id.0)))?;

        let files = self.get_checkpoint_files(checkpoint_id)?;
        let mut exported_files = Vec::new();

        for file in files {
            let content = if file.content_path.as_ref().is_some_and(|p| p.exists()) {
                let bytes = fs::read(file.content_path.as_ref().unwrap())?;
                base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bytes)
            } else {
                String::new()
            };

            let relative_path = file
                .path
                .strip_prefix(&self.workspace_root)
                .unwrap_or(&file.path)
                .to_path_buf();

            exported_files.push(super::ExportedFile {
                path: relative_path,
                content,
                is_binary: file.is_binary,
                hash: file.hash,
            });
        }

        Ok(super::ExportedCheckpoint {
            name: checkpoint.name,
            description: checkpoint.description,
            timestamp: checkpoint.timestamp,
            files: exported_files,
        })
    }

    /// Import a checkpoint from a portable format
    pub async fn import_checkpoint(
        &mut self,
        exported: super::ExportedCheckpoint,
    ) -> Result<CheckpointId> {
        let checkpoint_id = CheckpointId::new();
        let timestamp = exported.timestamp;

        let mut files_count = 0;
        let mut total_size = 0;

        self.conn.execute(
            r"
            INSERT INTO checkpoints (id, name, description, timestamp, files_count, total_size)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ",
            params![
                checkpoint_id.0,
                exported.name,
                exported.description,
                timestamp.to_rfc3339(),
                0i32,
                0i32,
            ],
        )?;

        for (version, exported_file) in exported.files.iter().enumerate() {
            let content = base64::Engine::decode(
                &base64::engine::general_purpose::STANDARD,
                &exported_file.content,
            )
            .unwrap_or_default();

            let file_path = self.workspace_root.join(&exported_file.path);
            let snapshot = FileSnapshot {
                path: file_path.clone(),
                hash: exported_file.hash.clone(),
                size: content.len(),
                is_binary: exported_file.is_binary,
                content_path: None,
            };

            let snapshot_path = self.save_file_snapshot(&checkpoint_id, &snapshot).await?;
            fs::write(&snapshot_path, &content)?;

            self.conn.execute(
                r"
                INSERT INTO file_versions (checkpoint_id, file_path, version, hash, size, snapshot_path, timestamp, is_binary)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                ",
                params![
                    checkpoint_id.0,
                    file_path.to_string_lossy().to_string(),
                    (version + 1) as i32,
                    exported_file.hash,
                    content.len() as i32,
                    snapshot_path.to_string_lossy().to_string(),
                    timestamp.to_rfc3339(),
                    i32::from(exported_file.is_binary),
                ],
            )?;

            files_count += 1;
            total_size += content.len() as i32;
        }

        self.conn.execute(
            r"
            UPDATE checkpoints SET files_count = ?1, total_size = ?2 WHERE id = ?3
            ",
            params![files_count, total_size, checkpoint_id.0],
        )?;

        Ok(checkpoint_id)
    }

    /// Clean up orphaned snapshot files
    pub fn cleanup_snapshots(&self) -> Result<usize> {
        let mut referenced_paths = std::collections::HashSet::new();

        let mut stmt = self
            .conn
            .prepare("SELECT snapshot_path FROM file_versions")?;
        let paths = stmt
            .query_map([], |row| Ok(PathBuf::from(row.get::<_, String>(0)?)))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        for path in paths {
            referenced_paths.insert(path);
        }

        let mut cleaned = 0;
        if self.snapshot_dir.exists() {
            for entry in fs::read_dir(&self.snapshot_dir)? {
                let entry = entry?;
                let checkpoint_dir = entry.path();

                if checkpoint_dir.is_dir() {
                    for file_entry in fs::read_dir(&checkpoint_dir)? {
                        let file_entry = file_entry?;
                        let file_path = file_entry.path();

                        if !referenced_paths.contains(&file_path) {
                            fs::remove_file(&file_path)?;
                            cleaned += 1;
                        }
                    }

                    if fs::read_dir(&checkpoint_dir)?.next().is_none() {
                        let checkpoint_id = checkpoint_dir
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("");

                        let exists: bool = self
                            .conn
                            .query_row(
                                "SELECT 1 FROM checkpoints WHERE id = ?1",
                                params![checkpoint_id],
                                |_| Ok(true),
                            )
                            .unwrap_or(false);

                        if !exists {
                            fs::remove_dir(&checkpoint_dir)?;
                        }
                    }
                }
            }
        }

        Ok(cleaned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_create_store() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("timeline.db");
        let store = TimelineStore::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

        let checkpoints = store.list_checkpoints().unwrap();
        assert!(checkpoints.is_empty());
    }

    #[test]
    fn test_track_file() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("timeline.db");
        let store = TimelineStore::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, "content").unwrap();

        store.track_file(&file_path).unwrap();
        assert_eq!(store.tracked_file_count().unwrap(), 1);
    }

    #[tokio::test]
    async fn test_create_and_list_checkpoints() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("timeline.db");
        let mut store = TimelineStore::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

        let file_path = temp_dir.path().join("main.rs");
        std::fs::write(&file_path, "fn main() {}").unwrap();

        let _id1 = store.create_checkpoint("checkpoint-1", None).await.unwrap();
        let _id2 = store
            .create_checkpoint("checkpoint-2", Some("Second checkpoint"))
            .await
            .unwrap();

        let checkpoints = store.list_checkpoints().unwrap();
        assert_eq!(checkpoints.len(), 2);

        assert_eq!(store.checkpoint_count().unwrap(), 2);
    }

    #[tokio::test]
    async fn test_get_checkpoint() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("timeline.db");
        let mut store = TimelineStore::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

        let id = store
            .create_checkpoint("test-checkpoint", Some("Description"))
            .await
            .unwrap();

        let checkpoint = store.get_checkpoint(&id).unwrap().unwrap();
        assert_eq!(checkpoint.name, "test-checkpoint");
        assert_eq!(checkpoint.description, Some("Description".to_string()));
    }

    #[tokio::test]
    async fn test_delete_checkpoint() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("timeline.db");
        let mut store = TimelineStore::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

        let id = store.create_checkpoint("to-delete", None).await.unwrap();
        assert_eq!(store.checkpoint_count().unwrap(), 1);

        store.delete_checkpoint(&id).unwrap();
        assert_eq!(store.checkpoint_count().unwrap(), 0);
    }

    #[tokio::test]
    async fn test_cleanup_old_checkpoints() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("timeline.db");
        let mut store = TimelineStore::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

        store.create_checkpoint("cp1", None).await.unwrap();
        store.create_checkpoint("cp2", None).await.unwrap();
        store.create_checkpoint("cp3", None).await.unwrap();

        let deleted = store.cleanup_old_checkpoints(1).unwrap();
        assert_eq!(deleted, 2);
        assert_eq!(store.checkpoint_count().unwrap(), 1);
    }

    #[tokio::test]
    async fn test_query_by_time_range() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("timeline.db");
        let mut store = TimelineStore::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

        store.create_checkpoint("time-test", None).await.unwrap();

        let start = Utc::now() - chrono::Duration::hours(1);
        let end = Utc::now() + chrono::Duration::hours(1);

        let checkpoints = store.query_by_time_range(start, end).unwrap();
        assert_eq!(checkpoints.len(), 1);
    }

    #[tokio::test]
    async fn test_query_by_name() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("timeline.db");
        let mut store = TimelineStore::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

        store.create_checkpoint("feature-xyz", None).await.unwrap();
        store.create_checkpoint("feature-abc", None).await.unwrap();
        store.create_checkpoint("bugfix-123", None).await.unwrap();

        let features = store.query_by_name("feature").unwrap();
        assert_eq!(features.len(), 2);

        let bugfixes = store.query_by_name("bugfix").unwrap();
        assert_eq!(bugfixes.len(), 1);
    }

    #[tokio::test]
    async fn test_file_history() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("timeline.db");
        let mut store = TimelineStore::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

        let file_path = temp_dir.path().join("history.txt");
        std::fs::write(&file_path, "version 1").unwrap();

        let _cp1 = store.create_checkpoint("v1", None).await.unwrap();

        std::fs::write(&file_path, "version 2").unwrap();
        let _cp2 = store.create_checkpoint("v2", None).await.unwrap();

        let history = store.get_file_history(&file_path).unwrap();
        assert!(!history.is_empty());
    }

    #[tokio::test]
    async fn test_diff_checkpoints() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("timeline.db");
        let mut store = TimelineStore::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

        let file1 = temp_dir.path().join("file1.txt");
        std::fs::write(&file1, "content 1").unwrap();

        let cp1 = store.create_checkpoint("before", None).await.unwrap();

        let file2 = temp_dir.path().join("file2.txt");
        std::fs::write(&file2, "content 2").unwrap();
        std::fs::write(&file1, "content 1 modified").unwrap();

        let cp2 = store.create_checkpoint("after", None).await.unwrap();

        let diff = store.diff_checkpoints(&cp1, &cp2).unwrap();
        assert!(diff.summary.total_files >= 1);
    }

    #[tokio::test]
    async fn test_get_file_version_at_checkpoint() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("timeline.db");
        let mut store = TimelineStore::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

        let file_path = temp_dir.path().join("versioned.txt");
        std::fs::write(&file_path, "original").unwrap();

        let cp = store.create_checkpoint("version-test", None).await.unwrap();

        let version = store
            .get_file_version_at_checkpoint(&file_path, &cp)
            .unwrap();
        assert!(version.is_some());
        let version = version.unwrap();
        assert!(!version.checksum.is_empty());
    }

    #[tokio::test]
    async fn test_get_files_changed_between() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("timeline.db");
        let mut store = TimelineStore::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

        let file1 = temp_dir.path().join("a.txt");
        std::fs::write(&file1, "a").unwrap();

        let cp1 = store.create_checkpoint("first", None).await.unwrap();

        let file2 = temp_dir.path().join("b.txt");
        std::fs::write(&file2, "b").unwrap();
        std::fs::write(&file1, "a-modified").unwrap();

        let cp2 = store.create_checkpoint("second", None).await.unwrap();

        let changes = store.get_files_changed_between(&cp1, &cp2).unwrap();
        assert!(!changes.is_empty());
    }

    #[tokio::test]
    async fn test_storage_stats() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("timeline.db");
        let mut store = TimelineStore::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

        let file = temp_dir.path().join("stats.txt");
        std::fs::write(&file, "content").unwrap();

        store.create_checkpoint("stats-test", None).await.unwrap();

        let stats = store.storage_stats().unwrap();
        assert_eq!(stats.checkpoint_count, 1);
    }

    #[tokio::test]
    async fn test_rollback_files() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("timeline.db");
        let store = TimelineStore::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

        let file_path = temp_dir.path().join("rollback_test.txt");
        std::fs::write(&file_path, "original content").unwrap();

        let mut store = store;
        let cp = store
            .create_checkpoint("before-change", None)
            .await
            .unwrap();

        std::fs::write(&file_path, "modified content").unwrap();
        assert_eq!(
            std::fs::read_to_string(&file_path).unwrap(),
            "modified content"
        );

        store
            .rollback_files(&cp, std::slice::from_ref(&file_path))
            .await
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(&file_path).unwrap(),
            "original content"
        );
    }

    #[tokio::test]
    async fn test_preview_rollback() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("timeline.db");
        let mut store = TimelineStore::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

        let file1 = temp_dir.path().join("preview1.txt");
        let file2 = temp_dir.path().join("preview2.txt");
        std::fs::write(&file1, "content 1").unwrap();
        std::fs::write(&file2, "content 2").unwrap();

        let cp = store.create_checkpoint("preview-test", None).await.unwrap();

        std::fs::write(&file1, "modified content 1").unwrap();

        let preview = store.preview_rollback(&cp).unwrap();
        assert!(!preview.files_to_restore.is_empty());
        assert_eq!(preview.checkpoint_id, cp);
    }

    #[tokio::test]
    async fn test_full_rollback() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("timeline.db");
        let mut store = TimelineStore::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

        let file = temp_dir.path().join("full_rollback.txt");
        std::fs::write(&file, "original").unwrap();

        let cp = store.create_checkpoint("pre-rollback", None).await.unwrap();

        std::fs::write(&file, "changed").unwrap();

        store.rollback(&cp).await.unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "original");
    }

    #[test]
    fn test_hash_content() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("timeline.db");
        let store = TimelineStore::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

        let hash1 = store.hash_content("test content");
        let hash2 = store.hash_content("test content");
        let hash3 = store.hash_content("different content");

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_compute_line_diff() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("timeline.db");
        let store = TimelineStore::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

        let old = "line1\nline2\nline3";
        let new = "line1\nline2-modified\nline3\nline4";

        let (additions, deletions) = store.compute_line_diff(old, new);
        assert!(additions > 0);
        assert!(deletions > 0);
    }

    #[test]
    fn test_is_binary_file() {
        let text = b"Hello, World!\nThis is text.\n";
        assert!(!TimelineStore::is_binary_file(text));

        let binary: Vec<u8> = (0..255).collect();
        assert!(TimelineStore::is_binary_file(&binary));

        let with_null = b"Hello\0World";
        assert!(TimelineStore::is_binary_file(with_null));

        let empty: &[u8] = &[];
        assert!(!TimelineStore::is_binary_file(empty));
    }

    #[tokio::test]
    async fn test_binary_file_rollback() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("timeline.db");
        let mut store = TimelineStore::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

        let binary_file = temp_dir.path().join("test.bin");
        let original_binary: Vec<u8> = vec![0x00, 0x01, 0x02, 0xFF, 0xFE, 0x00];
        std::fs::write(&binary_file, &original_binary).unwrap();

        let cp = store.create_checkpoint("binary-test", None).await.unwrap();

        let modified_binary: Vec<u8> = vec![0xAA, 0xBB, 0xCC];
        std::fs::write(&binary_file, &modified_binary).unwrap();

        store.rollback(&cp).await.unwrap();

        let restored = std::fs::read(&binary_file).unwrap();
        assert_eq!(restored, original_binary);
    }

    #[tokio::test]
    async fn test_binary_file_rollback_files() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("timeline.db");
        let store = TimelineStore::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

        let binary_file = temp_dir.path().join("data.bin");
        let original_binary: Vec<u8> = vec![0x00, 0x01, 0x02, 0x03];
        std::fs::write(&binary_file, &original_binary).unwrap();

        let mut store = store;
        let cp = store
            .create_checkpoint("binary-files-test", None)
            .await
            .unwrap();

        let modified_binary: Vec<u8> = vec![0xFF, 0xFE, 0xFD];
        std::fs::write(&binary_file, &modified_binary).unwrap();

        store
            .rollback_files(&cp, std::slice::from_ref(&binary_file))
            .await
            .unwrap();

        let restored = std::fs::read(&binary_file).unwrap();
        assert_eq!(restored, original_binary);
    }

    #[tokio::test]
    async fn test_export_import_checkpoint() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("timeline.db");
        let mut store = TimelineStore::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

        let file = temp_dir.path().join("export_test.txt");
        std::fs::write(&file, "export content").unwrap();

        let cp = store
            .create_checkpoint("export-test", Some("Test export"))
            .await
            .unwrap();

        let exported = store.export_checkpoint(&cp).unwrap();
        assert_eq!(exported.name, "export-test");
        assert_eq!(exported.description, Some("Test export".to_string()));
        assert!(!exported.files.is_empty());

        let imported_cp = store.import_checkpoint(exported).await.unwrap();
        assert!(!imported_cp.0.is_empty());
    }

    #[tokio::test]
    async fn test_export_binary_file() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("timeline.db");
        let mut store = TimelineStore::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

        let binary_file = temp_dir.path().join("export_binary.bin");
        let binary_content: Vec<u8> = vec![0x00, 0x01, 0x02, 0xFF];
        std::fs::write(&binary_file, &binary_content).unwrap();

        let cp = store
            .create_checkpoint("export-binary", None)
            .await
            .unwrap();

        let exported = store.export_checkpoint(&cp).unwrap();
        let binary_exported = exported
            .files
            .iter()
            .find(|f| f.path.to_string_lossy().contains("export_binary.bin"));
        assert!(binary_exported.is_some());

        let exported_file = binary_exported.unwrap();
        assert!(exported_file.is_binary);

        let decoded = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            &exported_file.content,
        )
        .unwrap();
        assert_eq!(decoded, binary_content);
    }

    #[tokio::test]
    async fn test_cleanup_snapshots() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("timeline.db");
        let mut store = TimelineStore::new(&db_path, temp_dir.path().to_path_buf()).unwrap();

        let file = temp_dir.path().join("cleanup_test.txt");
        std::fs::write(&file, "content").unwrap();

        let cp = store.create_checkpoint("cleanup-test", None).await.unwrap();

        let snapshot_dir = store.snapshot_dir.join(&cp.0);
        let orphan_file = snapshot_dir.join("orphan.txt");
        std::fs::write(&orphan_file, "orphan content").unwrap();

        let cleaned = store.cleanup_snapshots().unwrap();
        assert!(cleaned >= 1);
        assert!(!orphan_file.exists());
    }
}
