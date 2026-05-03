use super::PostgresBackend;
use crate::error::Result;
use crate::storage::backend::TimelineRepository;
use crate::storage::error::StorageError;
use crate::timeline::{
    CheckpointId, CheckpointInfo, Diff, DiffSummary, ExportedCheckpoint,
    ExportedFile, FileChangeType, FileDiff, FileVersion, RollbackPreview, StorageStats,
};
use chrono::{DateTime, Utc};
use std::path::{Path, PathBuf};
use tokio_postgres::types::ToSql;

impl TimelineRepository for PostgresBackend {
    fn track_file(&self, path: &Path) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let client = self.get_client().await?;
            let path_str = path.to_string_lossy().to_string();
            client
                .execute(
                    "INSERT INTO tracked_files (path) VALUES ($1) ON CONFLICT (path) DO NOTHING",
                    &[&path_str as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "INSERT tracked_file".to_string(),
                    reason: e.to_string(),
                })?;
            Ok(())
        }
    }

    fn tracked_file_count(&self) -> impl std::future::Future<Output = Result<usize>> + Send {
        async move {
            let client = self.get_client().await?;
            let row = client
                .query_one("SELECT COUNT(*) FROM tracked_files", &[])
                .await
                .map_err(|e| StorageError::Query {
                    statement: "COUNT tracked_files".to_string(),
                    reason: e.to_string(),
                })?;
            let count: i64 = row.get(0);
            Ok(count as usize)
        }
    }

    fn create_checkpoint(
        &self,
        name: &str,
        description: Option<&str>,
    ) -> impl std::future::Future<Output = Result<CheckpointId>> + Send {
        async move {
            let id = CheckpointId::new();
            let now = Utc::now();
            let client = self.get_client().await?;
            client
                .execute(
                    r"
                    INSERT INTO checkpoints (id, name, description, timestamp, files_count, total_size)
                    VALUES ($1, $2, $3, $4, 0, 0)
                    ",
                    &[
                        &id.0 as &(dyn ToSql + Sync),
                        &name,
                        &description,
                        &now,
                    ],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "INSERT checkpoint".to_string(),
                    reason: e.to_string(),
                })?;
            Ok(id)
        }
    }

    fn list_checkpoints(&self) -> impl std::future::Future<Output = Result<Vec<CheckpointInfo>>> + Send {
        async move {
            let client = self.get_client().await?;
            let rows = client
                .query(
                    r"
                    SELECT id, name, description, timestamp, files_count, total_size
                    FROM checkpoints ORDER BY timestamp DESC
                    ",
                    &[],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT checkpoints".to_string(),
                    reason: e.to_string(),
                })?;

            let checkpoints: Vec<CheckpointInfo> = rows.iter().map(Self::row_to_checkpoint_info).collect();
            Ok(checkpoints)
        }
    }

    fn get_checkpoint(&self, id: &CheckpointId) -> impl std::future::Future<Output = Result<Option<CheckpointInfo>>> + Send {
        async move {
            let client = self.get_client().await?;
            let row = client
                .query_opt(
                    r"
                    SELECT id, name, description, timestamp, files_count, total_size
                    FROM checkpoints WHERE id = $1
                    ",
                    &[&id.0 as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT checkpoint".to_string(),
                    reason: e.to_string(),
                })?;

            Ok(row.map(|r| Self::row_to_checkpoint_info(&r)))
        }
    }

    fn delete_checkpoint(&self, id: &CheckpointId) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let client = self.get_client().await?;
            client
                .execute(
                    "DELETE FROM checkpoints WHERE id = $1",
                    &[&id.0 as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "DELETE checkpoint".to_string(),
                    reason: e.to_string(),
                })?;
            Ok(())
        }
    }

    fn checkpoint_count(&self) -> impl std::future::Future<Output = Result<usize>> + Send {
        async move {
            let client = self.get_client().await?;
            let row = client
                .query_one("SELECT COUNT(*) FROM checkpoints", &[])
                .await
                .map_err(|e| StorageError::Query {
                    statement: "COUNT checkpoints".to_string(),
                    reason: e.to_string(),
                })?;
            let count: i64 = row.get(0);
            Ok(count as usize)
        }
    }

    fn get_file_history(&self, path: &Path) -> impl std::future::Future<Output = Result<Vec<FileVersion>>> + Send {
        async move {
            let client = self.get_client().await?;
            let path_str = path.to_string_lossy().to_string();
            let rows = client
                .query(
                    r"
                    SELECT path, version, checkpoint_id, checksum, size, timestamp
                    FROM file_versions WHERE path = $1
                    ORDER BY timestamp DESC
                    ",
                    &[&path_str as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT file history".to_string(),
                    reason: e.to_string(),
                })?;

            let versions: Vec<FileVersion> = rows.iter().map(Self::row_to_file_version).collect();
            Ok(versions)
        }
    }

    fn get_file_version_at_checkpoint(
        &self,
        path: &Path,
        checkpoint_id: &CheckpointId,
    ) -> impl std::future::Future<Output = Result<Option<FileVersion>>> + Send {
        async move {
            let client = self.get_client().await?;
            let path_str = path.to_string_lossy().to_string();
            let row = client
                .query_opt(
                    r"
                    SELECT path, version, checkpoint_id, checksum, size, timestamp
                    FROM file_versions WHERE path = $1 AND checkpoint_id = $2
                    ORDER BY version DESC LIMIT 1
                    ",
                    &[&path_str as &(dyn ToSql + Sync), &checkpoint_id.0],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT file version".to_string(),
                    reason: e.to_string(),
                })?;

            Ok(row.map(|r| Self::row_to_file_version(&r)))
        }
    }

    fn get_files_changed_between(
        &self,
        from: &CheckpointId,
        to: &CheckpointId,
    ) -> impl std::future::Future<Output = Result<Vec<(PathBuf, FileChangeType)>>> + Send {
        async move {
            let client = self.get_client().await?;
            let rows = client
                .query(
                    r"
                    SELECT path,
                           CASE
                               WHEN NOT EXISTS (SELECT 1 FROM file_versions fv1 WHERE fv1.path = fv.path AND fv1.checkpoint_id = $1)
                               THEN 'added'
                               WHEN NOT EXISTS (SELECT 1 FROM file_versions fv2 WHERE fv2.path = fv.path AND fv2.checkpoint_id = $2)
                               THEN 'deleted'
                               ELSE 'modified'
                           END as change_type
                    FROM file_versions fv
                    WHERE (checkpoint_id = $1 OR checkpoint_id = $2)
                    GROUP BY path
                    ",
                    &[&from.0 as &(dyn ToSql + Sync), &to.0],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT files changed".to_string(),
                    reason: e.to_string(),
                })?;

            let mut changes = Vec::new();
            for row in &rows {
                let path: String = row.get(0);
                let change_type_str: String = row.get(1);
                let change_type = match change_type_str.as_str() {
                    "added" => FileChangeType::Added,
                    "deleted" => FileChangeType::Deleted,
                    _ => FileChangeType::Modified,
                };
                changes.push((PathBuf::from(path), change_type));
            }

            Ok(changes)
        }
    }

    fn diff_checkpoints(&self, from: &CheckpointId, to: &CheckpointId) -> impl std::future::Future<Output = Result<Diff>> + Send {
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

    fn rollback(&self, _checkpoint_id: &CheckpointId) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            Ok(())
        }
    }

    fn rollback_files(
        &self,
        _checkpoint_id: &CheckpointId,
        _files: &[PathBuf],
    ) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            Ok(())
        }
    }

    fn preview_rollback(&self, checkpoint_id: &CheckpointId) -> impl std::future::Future<Output = Result<RollbackPreview>> + Send {
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
            let client = self.get_client().await?;
            let rows = client
                .query(
                    r"
                    SELECT id, name, description, timestamp, files_count, total_size
                    FROM checkpoints WHERE timestamp >= $1 AND timestamp <= $2
                    ORDER BY timestamp DESC
                    ",
                    &[&start as &(dyn ToSql + Sync), &end],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT checkpoints by time".to_string(),
                    reason: e.to_string(),
                })?;

            let checkpoints: Vec<CheckpointInfo> = rows.iter().map(Self::row_to_checkpoint_info).collect();
            Ok(checkpoints)
        }
    }

    fn query_by_name(&self, pattern: &str) -> impl std::future::Future<Output = Result<Vec<CheckpointInfo>>> + Send {
        async move {
            let client = self.get_client().await?;
            let like_pattern = format!("%{pattern}%");
            let rows = client
                .query(
                    r"
                    SELECT id, name, description, timestamp, files_count, total_size
                    FROM checkpoints WHERE name LIKE $1
                    ORDER BY timestamp DESC
                    ",
                    &[&like_pattern as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT checkpoints by name".to_string(),
                    reason: e.to_string(),
                })?;

            let checkpoints: Vec<CheckpointInfo> = rows.iter().map(Self::row_to_checkpoint_info).collect();
            Ok(checkpoints)
        }
    }

    fn export_checkpoint(&self, checkpoint_id: &CheckpointId) -> impl std::future::Future<Output = Result<ExportedCheckpoint>> + Send {
        async move {
            let checkpoint = self
                .get_checkpoint(checkpoint_id)
                .await?
                .ok_or_else(|| StorageError::checkpoint_not_found(&checkpoint_id.0))?;

            let client = self.get_client().await?;
            let rows = client
                .query(
                    r"
                    SELECT path, checksum, size FROM file_versions WHERE checkpoint_id = $1
                    ",
                    &[&checkpoint_id.0 as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT file versions for export".to_string(),
                    reason: e.to_string(),
                })?;

            let files: Vec<ExportedFile> = rows
                .iter()
                .map(|row| ExportedFile {
                    path: PathBuf::from(row.get::<_, String>(0)),
                    content: String::new(),
                    is_binary: false,
                    hash: row.get(1),
                })
                .collect();

            Ok(ExportedCheckpoint {
                name: checkpoint.name,
                description: checkpoint.description,
                timestamp: checkpoint.timestamp,
                files,
            })
        }
    }

    fn import_checkpoint(&self, exported: ExportedCheckpoint) -> impl std::future::Future<Output = Result<CheckpointId>> + Send {
        async move {
            let id = self.create_checkpoint(&exported.name, exported.description.as_deref()).await?;
            Ok(id)
        }
    }

    fn cleanup_old_checkpoints(&self, keep_count: usize) -> impl std::future::Future<Output = Result<usize>> + Send {
        async move {
            let client = self.get_client().await?;
            let row = client
                .query_one(
                    "SELECT COUNT(*) FROM checkpoints",
                    &[],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "COUNT checkpoints".to_string(),
                    reason: e.to_string(),
                })?;
            let count: i64 = row.get(0);

            if count as usize <= keep_count {
                return Ok(0);
            }

            let result = client
                .execute(
                    r"
                    DELETE FROM checkpoints WHERE id NOT IN (
                        SELECT id FROM checkpoints ORDER BY timestamp DESC LIMIT $1
                    )
                    ",
                    &[&(keep_count as i64) as &(dyn ToSql + Sync)],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "DELETE old checkpoints".to_string(),
                    reason: e.to_string(),
                })?;

            Ok(result as usize)
        }
    }

    fn cleanup_snapshots(&self) -> impl std::future::Future<Output = Result<usize>> + Send {
        async move {
            Ok(0)
        }
    }

    fn storage_stats(&self) -> impl std::future::Future<Output = Result<StorageStats>> + Send {
        async move {
            let client = self.get_client().await?;
            let row = client
                .query_one(
                    r"
                    SELECT
                        (SELECT COUNT(*) FROM checkpoints) AS checkpoint_count,
                        (SELECT COUNT(*) FROM tracked_files) AS tracked_file_count,
                        (SELECT COUNT(*) FROM file_versions) AS version_count
                    ",
                    &[],
                )
                .await
                .map_err(|e| StorageError::Query {
                    statement: "SELECT storage stats".to_string(),
                    reason: e.to_string(),
                })?;

            Ok(StorageStats {
                checkpoint_count: row.get::<_, i64>(0) as usize,
                tracked_file_count: row.get::<_, i64>(1) as usize,
                total_size_bytes: 0,
                version_count: row.get::<_, i64>(2) as usize,
            })
        }
    }
}
