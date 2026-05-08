use super::SqliteBackend;
use crate::error::Result;
use crate::storage::backend::TimelineRepository;
use crate::storage::error::StorageError;
use crate::timeline::{
    CheckpointId, CheckpointInfo, Diff, DiffSummary, ExportedCheckpoint, ExportedFile,
    FileChangeType, FileDiff, FileVersion, RollbackPreview, StorageStats,
};
use chrono::{DateTime, Utc};
use rusqlite::{params, OptionalExtension};
use std::path::{Path, PathBuf};

impl TimelineRepository for SqliteBackend {
    fn track_file(&self, path: &Path) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let path_str = path.to_string_lossy().to_string();
            self.with_conn(|conn| {
                conn.execute(
                    "INSERT OR IGNORE INTO tracked_files (path) VALUES (?1)",
                    params![path_str],
                )
                .map_err(|e| StorageError::Query {
                    statement: "INSERT tracked_file".to_string(),
                    reason: e.to_string(),
                })?;
                Ok(())
            })
        }
    }

    fn tracked_file_count(&self) -> impl std::future::Future<Output = Result<usize>> + Send {
        async move {
            self.with_conn(|conn| {
                let count: i64 = conn
                    .query_row("SELECT COUNT(*) FROM tracked_files", [], |row| row.get(0))
                    .map_err(|e| StorageError::Query {
                        statement: "COUNT tracked_files".to_string(),
                        reason: e.to_string(),
                    })?;
                Ok(count as usize)
            })
        }
    }

    fn create_checkpoint(
        &self,
        name: &str,
        description: Option<&str>,
    ) -> impl std::future::Future<Output = Result<CheckpointId>> + Send {
        async move {
            let id = CheckpointId::new();
            let now = Utc::now().to_rfc3339();
            self.with_conn(|conn| {
                conn.execute(
                    r"
                    INSERT INTO checkpoints (id, name, description, timestamp, files_count, total_size)
                    VALUES (?1, ?2, ?3, ?4, 0, 0)
                    ",
                    params![id.0.clone(), name, description, now],
                )
                .map_err(|e| StorageError::Query {
                    statement: "INSERT checkpoint".to_string(),
                    reason: e.to_string(),
                })?;
                Ok(())
            })?;
            Ok(id)
        }
    }

    fn list_checkpoints(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<CheckpointInfo>>> + Send {
        async move {
            self.with_conn(|conn| {
                let mut stmt = conn
                    .prepare(
                        r"
                    SELECT id, name, description, timestamp, files_count, total_size
                    FROM checkpoints ORDER BY timestamp DESC
                    ",
                    )
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT checkpoints".to_string(),
                        reason: e.to_string(),
                    })?;

                let checkpoints = stmt
                    .query_map([], |row| {
                        Ok(CheckpointInfo {
                            id: CheckpointId::from_string(row.get::<_, String>(0)?),
                            name: row.get(1)?,
                            description: row.get(2)?,
                            timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(3)?)
                                .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc)),
                            files_count: row.get::<_, i64>(4)? as usize,
                            total_size: row.get::<_, i64>(5)? as usize,
                        })
                    })
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT checkpoints".to_string(),
                        reason: e.to_string(),
                    })?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT checkpoints".to_string(),
                        reason: e.to_string(),
                    })?;

                Ok(checkpoints)
            })
        }
    }

    fn get_checkpoint(
        &self,
        id: &CheckpointId,
    ) -> impl std::future::Future<Output = Result<Option<CheckpointInfo>>> + Send {
        async move {
            self.with_conn(|conn| {
                let result = conn
                    .query_row(
                        r"
                    SELECT id, name, description, timestamp, files_count, total_size
                    FROM checkpoints WHERE id = ?1
                    ",
                        params![id.0.clone()],
                        |row| {
                            Ok(CheckpointInfo {
                                id: CheckpointId::from_string(row.get::<_, String>(0)?),
                                name: row.get(1)?,
                                description: row.get(2)?,
                                timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(3)?)
                                    .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc)),
                                files_count: row.get::<_, i64>(4)? as usize,
                                total_size: row.get::<_, i64>(5)? as usize,
                            })
                        },
                    )
                    .optional()
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT checkpoint".to_string(),
                        reason: e.to_string(),
                    })?;
                Ok(result)
            })
        }
    }

    fn delete_checkpoint(
        &self,
        id: &CheckpointId,
    ) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            self.with_conn(|conn| {
                conn.execute(
                    "DELETE FROM checkpoints WHERE id = ?1",
                    params![id.0.clone()],
                )
                .map_err(|e| StorageError::Query {
                    statement: "DELETE checkpoint".to_string(),
                    reason: e.to_string(),
                })?;
                Ok(())
            })
        }
    }

    fn checkpoint_count(&self) -> impl std::future::Future<Output = Result<usize>> + Send {
        async move {
            self.with_conn(|conn| {
                let count: i64 = conn
                    .query_row("SELECT COUNT(*) FROM checkpoints", [], |row| row.get(0))
                    .map_err(|e| StorageError::Query {
                        statement: "COUNT checkpoints".to_string(),
                        reason: e.to_string(),
                    })?;
                Ok(count as usize)
            })
        }
    }

    fn get_file_history(
        &self,
        path: &Path,
    ) -> impl std::future::Future<Output = Result<Vec<FileVersion>>> + Send {
        async move {
            let path_str = path.to_string_lossy().to_string();
            self.with_conn(|conn| {
                let mut stmt = conn
                    .prepare(
                        r"
                    SELECT path, version, checkpoint_id, checksum, size, timestamp
                    FROM file_versions WHERE path = ?1
                    ORDER BY timestamp DESC
                    ",
                    )
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT file history".to_string(),
                        reason: e.to_string(),
                    })?;

                let versions = stmt
                    .query_map(params![path_str], |row| {
                        Ok(FileVersion {
                            path: PathBuf::from(row.get::<_, String>(0)?),
                            version: row.get::<_, i64>(1)? as u64,
                            timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(5)?)
                                .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc)),
                            checksum: row.get(3)?,
                            size: row.get::<_, i64>(4)? as usize,
                            checkpoint_id: CheckpointId::from_string(row.get(2)?),
                        })
                    })
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT file history".to_string(),
                        reason: e.to_string(),
                    })?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT file history".to_string(),
                        reason: e.to_string(),
                    })?;

                Ok(versions)
            })
        }
    }

    fn get_file_version_at_checkpoint(
        &self,
        path: &Path,
        checkpoint_id: &CheckpointId,
    ) -> impl std::future::Future<Output = Result<Option<FileVersion>>> + Send {
        async move {
            let path_str = path.to_string_lossy().to_string();
            self.with_conn(|conn| {
                let result = conn
                    .query_row(
                        r"
                    SELECT path, version, checkpoint_id, checksum, size, timestamp
                    FROM file_versions WHERE path = ?1 AND checkpoint_id = ?2
                    ORDER BY version DESC LIMIT 1
                    ",
                        params![path_str, checkpoint_id.0.clone()],
                        |row| {
                            Ok(FileVersion {
                                path: PathBuf::from(row.get::<_, String>(0)?),
                                version: row.get::<_, i64>(1)? as u64,
                                timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(5)?)
                                    .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc)),
                                checksum: row.get(3)?,
                                size: row.get::<_, i64>(4)? as usize,
                                checkpoint_id: CheckpointId::from_string(row.get(2)?),
                            })
                        },
                    )
                    .optional()
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT file version".to_string(),
                        reason: e.to_string(),
                    })?;
                Ok(result)
            })
        }
    }

    fn get_files_changed_between(
        &self,
        from: &CheckpointId,
        to: &CheckpointId,
    ) -> impl std::future::Future<Output = Result<Vec<(PathBuf, FileChangeType)>>> + Send {
        async move {
            self.with_conn(|conn| {
                let mut stmt = conn
                    .prepare(
                        r"
                    SELECT path,
                           CASE
                               WHEN NOT EXISTS (SELECT 1 FROM file_versions fv1 WHERE fv1.path = fv.path AND fv1.checkpoint_id = ?1)
                               THEN 'added'
                               WHEN NOT EXISTS (SELECT 1 FROM file_versions fv2 WHERE fv2.path = fv.path AND fv2.checkpoint_id = ?2)
                               THEN 'deleted'
                               ELSE 'modified'
                           END as change_type
                    FROM file_versions fv
                    WHERE (checkpoint_id = ?1 OR checkpoint_id = ?2)
                    GROUP BY path
                    ",
                    )
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT files changed".to_string(),
                        reason: e.to_string(),
                    })?;

                let changes = stmt
                    .query_map(params![from.0.clone(), to.0.clone()], |row| {
                        let path: String = row.get(0)?;
                        let change_type_str: String = row.get(1)?;
                        let change_type = match change_type_str.as_str() {
                            "added" => FileChangeType::Added,
                            "deleted" => FileChangeType::Deleted,
                            _ => FileChangeType::Modified,
                        };
                        Ok((PathBuf::from(path), change_type))
                    })
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT files changed".to_string(),
                        reason: e.to_string(),
                    })?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT files changed".to_string(),
                        reason: e.to_string(),
                    })?;

                Ok(changes)
            })
        }
    }

    fn diff_checkpoints(
        &self,
        from: &CheckpointId,
        to: &CheckpointId,
    ) -> impl std::future::Future<Output = Result<Diff>> + Send {
        async move {
            let changes = self.get_files_changed_between(from, to).await?;
            let files_changed: Vec<FileDiff> = changes
                .into_iter()
                .map(|(path, change_type)| FileDiff {
                    path,
                    change_type,
                    additions: 0,
                    deletions: 0,
                })
                .collect();
            let total_additions = files_changed.iter().map(|f| f.additions).sum();
            let total_deletions = files_changed.iter().map(|f| f.deletions).sum();
            let total_files = files_changed.len();
            Ok(Diff {
                from: from.clone(),
                to: to.clone(),
                files_changed,
                summary: DiffSummary {
                    total_files,
                    total_additions,
                    total_deletions,
                },
            })
        }
    }

    fn rollback(
        &self,
        _checkpoint_id: &CheckpointId,
    ) -> impl std::future::Future<Output = Result<()>> + Send {
        async move { Ok(()) }
    }

    fn rollback_files(
        &self,
        _checkpoint_id: &CheckpointId,
        _files: &[PathBuf],
    ) -> impl std::future::Future<Output = Result<()>> + Send {
        async move { Ok(()) }
    }

    fn preview_rollback(
        &self,
        checkpoint_id: &CheckpointId,
    ) -> impl std::future::Future<Output = Result<RollbackPreview>> + Send {
        async move {
            let checkpoint = self
                .get_checkpoint(checkpoint_id)
                .await?
                .ok_or_else(|| StorageError::checkpoint_not_found(&checkpoint_id.0))?;
            Ok(RollbackPreview {
                checkpoint_id: checkpoint_id.clone(),
                files_to_restore: Vec::new(),
                files_to_delete: Vec::new(),
                files_modified: Vec::new(),
                total_files_affected: checkpoint.files_count,
            })
        }
    }

    fn query_by_time_range(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> impl std::future::Future<Output = Result<Vec<CheckpointInfo>>> + Send {
        async move {
            self.with_conn(|conn| {
                let mut stmt = conn
                    .prepare(
                        r"
                    SELECT id, name, description, timestamp, files_count, total_size
                    FROM checkpoints WHERE timestamp >= ?1 AND timestamp <= ?2
                    ORDER BY timestamp DESC
                    ",
                    )
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT checkpoints by time".to_string(),
                        reason: e.to_string(),
                    })?;

                let start_str = start.to_rfc3339();
                let end_str = end.to_rfc3339();
                let checkpoints = stmt
                    .query_map(params![start_str, end_str], |row| {
                        Ok(CheckpointInfo {
                            id: CheckpointId::from_string(row.get::<_, String>(0)?),
                            name: row.get(1)?,
                            description: row.get(2)?,
                            timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(3)?)
                                .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc)),
                            files_count: row.get::<_, i64>(4)? as usize,
                            total_size: row.get::<_, i64>(5)? as usize,
                        })
                    })
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT checkpoints by time".to_string(),
                        reason: e.to_string(),
                    })?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT checkpoints by time".to_string(),
                        reason: e.to_string(),
                    })?;

                Ok(checkpoints)
            })
        }
    }

    fn query_by_name(
        &self,
        pattern: &str,
    ) -> impl std::future::Future<Output = Result<Vec<CheckpointInfo>>> + Send {
        async move {
            let like_pattern = format!("%{pattern}%");
            self.with_conn(|conn| {
                let mut stmt = conn
                    .prepare(
                        r"
                    SELECT id, name, description, timestamp, files_count, total_size
                    FROM checkpoints WHERE name LIKE ?1
                    ORDER BY timestamp DESC
                    ",
                    )
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT checkpoints by name".to_string(),
                        reason: e.to_string(),
                    })?;

                let checkpoints = stmt
                    .query_map(params![like_pattern], |row| {
                        Ok(CheckpointInfo {
                            id: CheckpointId::from_string(row.get::<_, String>(0)?),
                            name: row.get(1)?,
                            description: row.get(2)?,
                            timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(3)?)
                                .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc)),
                            files_count: row.get::<_, i64>(4)? as usize,
                            total_size: row.get::<_, i64>(5)? as usize,
                        })
                    })
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT checkpoints by name".to_string(),
                        reason: e.to_string(),
                    })?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT checkpoints by name".to_string(),
                        reason: e.to_string(),
                    })?;

                Ok(checkpoints)
            })
        }
    }

    fn export_checkpoint(
        &self,
        checkpoint_id: &CheckpointId,
    ) -> impl std::future::Future<Output = Result<ExportedCheckpoint>> + Send {
        async move {
            let checkpoint = self
                .get_checkpoint(checkpoint_id)
                .await?
                .ok_or_else(|| StorageError::checkpoint_not_found(&checkpoint_id.0))?;

            let versions = self.with_conn(|conn| {
                let mut stmt = conn
                    .prepare(
                        r"
                    SELECT path, checksum, size FROM file_versions WHERE checkpoint_id = ?1
                    ",
                    )
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT file versions for export".to_string(),
                        reason: e.to_string(),
                    })?;

                let files = stmt
                    .query_map(params![checkpoint_id.0.clone()], |row| {
                        Ok(ExportedFile {
                            path: PathBuf::from(row.get::<_, String>(0)?),
                            content: String::new(),
                            is_binary: false,
                            hash: row.get(1)?,
                        })
                    })
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT file versions for export".to_string(),
                        reason: e.to_string(),
                    })?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(|e| StorageError::Query {
                        statement: "SELECT file versions for export".to_string(),
                        reason: e.to_string(),
                    })?;

                Ok(files)
            })?;

            Ok(ExportedCheckpoint {
                name: checkpoint.name,
                description: checkpoint.description,
                timestamp: checkpoint.timestamp,
                files: versions,
            })
        }
    }

    fn import_checkpoint(
        &self,
        exported: ExportedCheckpoint,
    ) -> impl std::future::Future<Output = Result<CheckpointId>> + Send {
        async move {
            let id = self
                .create_checkpoint(&exported.name, exported.description.as_deref())
                .await?;
            Ok(id)
        }
    }

    fn cleanup_old_checkpoints(
        &self,
        keep_count: usize,
    ) -> impl std::future::Future<Output = Result<usize>> + Send {
        async move {
            self.with_conn(|conn| {
                let count: i64 = conn
                    .query_row("SELECT COUNT(*) FROM checkpoints", [], |row| row.get(0))
                    .map_err(|e| StorageError::Query {
                        statement: "COUNT checkpoints".to_string(),
                        reason: e.to_string(),
                    })?;

                if count as usize <= keep_count {
                    return Ok(0);
                }

                let deleted = conn
                    .execute(
                        r"
                    DELETE FROM checkpoints WHERE id NOT IN (
                        SELECT id FROM checkpoints ORDER BY timestamp DESC LIMIT ?1
                    )
                    ",
                        params![keep_count as i64],
                    )
                    .map_err(|e| StorageError::Query {
                        statement: "DELETE old checkpoints".to_string(),
                        reason: e.to_string(),
                    })?;

                Ok(deleted)
            })
        }
    }

    fn cleanup_snapshots(&self) -> impl std::future::Future<Output = Result<usize>> + Send {
        async move { Ok(0) }
    }

    fn storage_stats(&self) -> impl std::future::Future<Output = Result<StorageStats>> + Send {
        async move {
            self.with_conn(|conn| {
                let checkpoint_count: i64 = conn
                    .query_row("SELECT COUNT(*) FROM checkpoints", [], |row| row.get(0))
                    .map_err(|e| StorageError::Query {
                        statement: "COUNT checkpoints".to_string(),
                        reason: e.to_string(),
                    })?;
                let tracked_file_count: i64 = conn
                    .query_row("SELECT COUNT(*) FROM tracked_files", [], |row| row.get(0))
                    .map_err(|e| StorageError::Query {
                        statement: "COUNT tracked_files".to_string(),
                        reason: e.to_string(),
                    })?;
                let version_count: i64 = conn
                    .query_row("SELECT COUNT(*) FROM file_versions", [], |row| row.get(0))
                    .map_err(|e| StorageError::Query {
                        statement: "COUNT file_versions".to_string(),
                        reason: e.to_string(),
                    })?;

                Ok(StorageStats {
                    checkpoint_count: checkpoint_count as usize,
                    tracked_file_count: tracked_file_count as usize,
                    total_size_bytes: 0,
                    version_count: version_count as usize,
                })
            })
        }
    }
}
