//! In-memory storage backend (HashMap-backed, for testing and ephemeral use).

use super::backend::{
    GraphRepository, SessionRepository, StorageBackend, TimelineRepository,
};
use crate::error::Result;
use crate::graph_rag::ast::{
    FileInfo, Reference, Relationship, Symbol, SymbolKind,
};
use crate::session::types::{Message, Session, SessionId, TokenUsage};
use crate::timeline::{
    CheckpointId, CheckpointInfo, Diff, DiffSummary, ExportedCheckpoint,
    ExportedFile, FileChangeType, FileSnapshot, FileVersion, RollbackPreview, StorageStats,
};
use crate::timeline::FileDiff;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use uuid::Uuid;

/// In-memory implementation of all storage traits.
///
/// Thread-safe via `Mutex`. Intended for:
/// - Unit tests
/// - Ephemeral mode (no persistence)
/// - CI/CD environments
#[derive(Debug)]
pub struct InMemoryBackend {
    // Sessions domain
    sessions: Mutex<HashMap<String, Session>>,
    messages: Mutex<HashMap<String, Vec<Message>>>,

    // Timeline domain
    checkpoints: Mutex<HashMap<String, crate::timeline::TimelineCheckpoint>>,
    tracked_files: Mutex<Vec<PathBuf>>,
    file_versions: Mutex<Vec<FileVersion>>,

    // Graph domain
    files: Mutex<HashMap<String, FileInfo>>,
    symbols: Mutex<HashMap<i64, Symbol>>,
    symbol_refs: Mutex<Vec<Reference>>,
    relationships: Mutex<Vec<Relationship>>,
    next_file_id: Mutex<i64>,
    next_symbol_id: Mutex<i64>,
}

impl InMemoryBackend {
    /// Create a new empty in-memory backend.
    #[must_use]
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            messages: Mutex::new(HashMap::new()),
            checkpoints: Mutex::new(HashMap::new()),
            tracked_files: Mutex::new(Vec::new()),
            file_versions: Mutex::new(Vec::new()),
            files: Mutex::new(HashMap::new()),
            symbols: Mutex::new(HashMap::new()),
            symbol_refs: Mutex::new(Vec::new()),
            relationships: Mutex::new(Vec::new()),
            next_file_id: Mutex::new(1),
            next_symbol_id: Mutex::new(1),
        }
    }
}

impl Default for InMemoryBackend {
    fn default() -> Self {
        Self::new()
    }
}

// ─────────────────────────────────────────────────────────
// SessionRepository
// ─────────────────────────────────────────────────────────

impl SessionRepository for InMemoryBackend {
    fn create_session(&self, session: &Session) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let key = session.id.to_string();
            self.sessions.lock().unwrap().insert(key.clone(), session.clone());
            self.messages.lock().unwrap().insert(key, Vec::new());
            Ok(())
        }
    }

    fn load_session(&self, id: &SessionId) -> impl std::future::Future<Output = Result<Option<Session>>> + Send {
        async move {
            let sessions = self.sessions.lock().unwrap();
            Ok(sessions.get(&id.to_string()).cloned())
        }
    }

    fn load_session_full(&self, id: &SessionId) -> impl std::future::Future<Output = Result<Option<Session>>> + Send {
        async move {
            let sessions = self.sessions.lock().unwrap();
            let messages = self.messages.lock().unwrap();
            let key = id.to_string();
            let mut session = sessions.get(&key).cloned();
            if let Some(ref mut s) = session {
                if let Some(msgs) = messages.get(&key) {
                    s.messages = msgs.clone();
                }
            }
            Ok(session)
        }
    }

    fn list_sessions(&self) -> impl std::future::Future<Output = Result<Vec<Session>>> + Send {
        async move {
            let sessions = self.sessions.lock().unwrap();
            let mut list: Vec<Session> = sessions.values().cloned().collect();
            list.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
            Ok(list)
        }
    }

    fn delete_session(&self, id: &SessionId) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let key = id.to_string();
            self.sessions.lock().unwrap().remove(&key);
            self.messages.lock().unwrap().remove(&key);
            Ok(())
        }
    }

    fn save_message(&self, session_id: &SessionId, message: &Message) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let key = session_id.to_string();
            let sessions = self.sessions.lock().unwrap();
            if !sessions.contains_key(&key) {
                return Err(crate::error::Error::SessionNotFound {
                    id: session_id.to_string(),
                });
            }
            drop(sessions);
            self.messages.lock().unwrap().entry(key).or_default().push(message.clone());
            Ok(())
        }
    }

    fn search_messages(&self, query: &str) -> impl std::future::Future<Output = Result<Vec<(SessionId, Message)>>> + Send {
        async move {
            let messages = self.messages.lock().unwrap();
            let mut results = Vec::new();
            let query_lower = query.to_lowercase();
            for (key, msgs) in messages.iter() {
                for msg in msgs {
                    if let Some(text) = msg.as_text() {
                        if text.to_lowercase().contains(&query_lower) {
                            if let Ok(uuid) = Uuid::parse_str(key) {
                            let id = SessionId::from_uuid(uuid);
                            results.push((id, msg.clone()));
                        }
                        }
                    }
                }
            }
            Ok(results)
        }
    }

    fn update_token_usage(&self, id: &SessionId, usage: &TokenUsage) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let mut sessions = self.sessions.lock().unwrap();
            let session = sessions.get_mut(&id.to_string()).ok_or_else(|| {
                crate::error::Error::SessionNotFound { id: id.to_string() }
            })?;
            session.token_usage.add(usage);
            Ok(())
        }
    }
}

// ─────────────────────────────────────────────────────────
// TimelineRepository
// ─────────────────────────────────────────────────────────

impl TimelineRepository for InMemoryBackend {
    fn track_file(&self, path: &Path) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let mut files = self.tracked_files.lock().unwrap();
            if !files.contains(&path.to_path_buf()) {
                files.push(path.to_path_buf());
            }
            Ok(())
        }
    }

    fn tracked_file_count(&self) -> impl std::future::Future<Output = Result<usize>> + Send {
        async move {
            Ok(self.tracked_files.lock().unwrap().len())
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
            let info = CheckpointInfo {
                id: id.clone(),
                name: name.to_string(),
                description: description.map(|s| s.to_string()),
                timestamp: now,
                files_count: 0,
                total_size: 0,
            };
            let checkpoint = crate::timeline::TimelineCheckpoint {
                info,
                files: Vec::new(),
            };
            self.checkpoints.lock().unwrap().insert(id.0.clone(), checkpoint);
            Ok(id)
        }
    }

    fn list_checkpoints(&self) -> impl std::future::Future<Output = Result<Vec<CheckpointInfo>>> + Send {
        async move {
            let checkpoints = self.checkpoints.lock().unwrap();
            let mut list: Vec<CheckpointInfo> = checkpoints.values().map(|cp| cp.info.clone()).collect();
            list.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
            Ok(list)
        }
    }

    fn get_checkpoint(&self, id: &CheckpointId) -> impl std::future::Future<Output = Result<Option<CheckpointInfo>>> + Send {
        async move {
            let checkpoints = self.checkpoints.lock().unwrap();
            Ok(checkpoints.get(&id.0).map(|cp| cp.info.clone()))
        }
    }

    fn delete_checkpoint(&self, id: &CheckpointId) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            self.checkpoints.lock().unwrap().remove(&id.0);
            Ok(())
        }
    }

    fn checkpoint_count(&self) -> impl std::future::Future<Output = Result<usize>> + Send {
        async move {
            Ok(self.checkpoints.lock().unwrap().len())
        }
    }

    fn get_file_history(&self, path: &Path) -> impl std::future::Future<Output = Result<Vec<FileVersion>>> + Send {
        async move {
            let versions = self.file_versions.lock().unwrap();
            Ok(versions.iter().filter(|v| v.path == path).cloned().collect())
        }
    }

    fn get_file_version_at_checkpoint(
        &self,
        path: &Path,
        checkpoint_id: &CheckpointId,
    ) -> impl std::future::Future<Output = Result<Option<FileVersion>>> + Send {
        async move {
            let versions = self.file_versions.lock().unwrap();
            Ok(versions
                .iter()
                .find(|v| v.path == path && v.checkpoint_id == *checkpoint_id)
                .cloned())
        }
    }

    fn get_files_changed_between(
        &self,
        from: &CheckpointId,
        to: &CheckpointId,
    ) -> impl std::future::Future<Output = Result<Vec<(PathBuf, FileChangeType)>>> + Send {
        async move {
            let versions = self.file_versions.lock().unwrap();
            let from_files: std::collections::HashSet<_> = versions
                .iter()
                .filter(|v| v.checkpoint_id == *from)
                .map(|v| v.path.clone())
                .collect();
            let to_files: std::collections::HashSet<_> = versions
                .iter()
                .filter(|v| v.checkpoint_id == *to)
                .map(|v| v.path.clone())
                .collect();

            let mut changes = Vec::new();
            for path in to_files.difference(&from_files) {
                changes.push((path.clone(), FileChangeType::Added));
            }
            for path in from_files.difference(&to_files) {
                changes.push((path.clone(), FileChangeType::Deleted));
            }
            for path in from_files.intersection(&to_files) {
                changes.push((path.clone(), FileChangeType::Modified));
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
            let checkpoints = self.checkpoints.lock().unwrap();
            let checkpoint = checkpoints.get(&checkpoint_id.0);
            let files_to_restore: Vec<PathBuf> = checkpoint
                .map(|cp| cp.files.iter().map(|f| f.path.clone()).collect())
                .unwrap_or_default();
            Ok(RollbackPreview {
                checkpoint_id: checkpoint_id.clone(),
                files_to_restore,
                files_to_delete: Vec::new(),
                files_modified: Vec::new(),
                total_files_affected: 0,
            })
        }
    }

    fn query_by_time_range(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> impl std::future::Future<Output = Result<Vec<CheckpointInfo>>> + Send {
        async move {
            let checkpoints = self.checkpoints.lock().unwrap();
            Ok(checkpoints
                .values()
                .filter(|cp| cp.info.timestamp >= start && cp.info.timestamp <= end)
                .map(|cp| cp.info.clone())
                .collect())
        }
    }

    fn query_by_name(&self, pattern: &str) -> impl std::future::Future<Output = Result<Vec<CheckpointInfo>>> + Send {
        async move {
            let checkpoints = self.checkpoints.lock().unwrap();
            Ok(checkpoints
                .values()
                .filter(|cp| cp.info.name.contains(pattern))
                .map(|cp| cp.info.clone())
                .collect())
        }
    }

    fn export_checkpoint(&self, checkpoint_id: &CheckpointId) -> impl std::future::Future<Output = Result<ExportedCheckpoint>> + Send {
        async move {
            let checkpoints = self.checkpoints.lock().unwrap();
            let checkpoint = checkpoints.get(&checkpoint_id.0).ok_or_else(|| {
                crate::error::Error::Checkpoint(format!(
                    "checkpoint not found: {}",
                    checkpoint_id.0
                ))
            })?;
            Ok(ExportedCheckpoint {
                name: checkpoint.info.name.clone(),
                description: checkpoint.info.description.clone(),
                timestamp: checkpoint.info.timestamp,
                files: checkpoint
                    .files
                    .iter()
                    .map(|f| ExportedFile {
                        path: f.path.clone(),
                        content: String::new(),
                        is_binary: f.is_binary,
                        hash: f.hash.clone(),
                    })
                    .collect(),
            })
        }
    }

    fn import_checkpoint(&self, exported: ExportedCheckpoint) -> impl std::future::Future<Output = Result<CheckpointId>> + Send {
        async move {
            let id = CheckpointId::new();
            let info = CheckpointInfo {
                id: id.clone(),
                name: exported.name,
                description: exported.description,
                timestamp: exported.timestamp,
                files_count: exported.files.len(),
                total_size: 0,
            };
            let files: Vec<FileSnapshot> = exported
                .files
                .into_iter()
                .map(|f| FileSnapshot {
                    path: f.path,
                    hash: f.hash,
                    size: f.content.len(),
                    is_binary: f.is_binary,
                    content_path: None,
                })
                .collect();
            let checkpoint = crate::timeline::TimelineCheckpoint { info, files };
            self.checkpoints.lock().unwrap().insert(id.0.clone(), checkpoint);
            Ok(id)
        }
    }

    fn cleanup_old_checkpoints(&self, keep_count: usize) -> impl std::future::Future<Output = Result<usize>> + Send {
        async move {
            let mut checkpoints = self.checkpoints.lock().unwrap();
            if checkpoints.len() <= keep_count {
                return Ok(0);
            }
            let mut list: Vec<_> = checkpoints.iter().collect();
            list.sort_by(|a, b| b.1.info.timestamp.cmp(&a.1.info.timestamp));
            let to_remove: Vec<String> = list
                .into_iter()
                .skip(keep_count)
                .map(|(k, _)| k.clone())
                .collect();
            let count = to_remove.len();
            for key in to_remove {
                checkpoints.remove(&key);
            }
            Ok(count)
        }
    }

    fn cleanup_snapshots(&self) -> impl std::future::Future<Output = Result<usize>> + Send {
        async move {
            Ok(0)
        }
    }

    fn storage_stats(&self) -> impl std::future::Future<Output = Result<StorageStats>> + Send {
        async move {
            Ok(StorageStats {
                checkpoint_count: self.checkpoints.lock().unwrap().len(),
                tracked_file_count: self.tracked_files.lock().unwrap().len(),
                total_size_bytes: 0,
                version_count: self.file_versions.lock().unwrap().len(),
            })
        }
    }
}

// ─────────────────────────────────────────────────────────
// GraphRepository
// ─────────────────────────────────────────────────────────

impl GraphRepository for InMemoryBackend {
    fn insert_file(&self, file: &FileInfo) -> impl std::future::Future<Output = Result<i64>> + Send {
        async move {
            let mut next_id = self.next_file_id.lock().unwrap();
            let id = *next_id;
            *next_id += 1;
            drop(next_id);
            self.files.lock().unwrap().insert(file.path.clone(), file.clone());
            Ok(id)
        }
    }

    fn get_file_by_path(&self, path: &str) -> impl std::future::Future<Output = Result<Option<FileInfo>>> + Send {
        async move {
            Ok(self.files.lock().unwrap().get(path).cloned())
        }
    }

    fn get_file_by_id(&self, _id: i64) -> impl std::future::Future<Output = Result<Option<FileInfo>>> + Send {
        async move {
            Ok(None)
        }
    }

    fn get_file_id(&self, path: &str) -> impl std::future::Future<Output = Result<Option<i64>>> + Send {
        async move {
            let files = self.files.lock().unwrap();
            if files.contains_key(path) {
                Ok(Some(1))
            } else {
                Ok(None)
            }
        }
    }

    fn delete_file(&self, path: &str) -> impl std::future::Future<Output = Result<bool>> + Send {
        async move {
            Ok(self.files.lock().unwrap().remove(path).is_some())
        }
    }

    fn count_files(&self) -> impl std::future::Future<Output = Result<i64>> + Send {
        async move {
            Ok(self.files.lock().unwrap().len() as i64)
        }
    }

    fn insert_symbol(&self, symbol: &Symbol) -> impl std::future::Future<Output = Result<i64>> + Send {
        async move {
            let mut next_id = self.next_symbol_id.lock().unwrap();
            let id = *next_id;
            *next_id += 1;
            drop(next_id);
            self.symbols.lock().unwrap().insert(id, symbol.clone());
            Ok(id)
        }
    }

    fn find_symbol(&self, name: &str) -> impl std::future::Future<Output = Result<Vec<Symbol>>> + Send {
        async move {
            let symbols = self.symbols.lock().unwrap();
            Ok(symbols.values().filter(|s| s.name == name).cloned().collect())
        }
    }

    fn find_symbol_by_id(&self, id: i64) -> impl std::future::Future<Output = Result<Option<Symbol>>> + Send {
        async move {
            Ok(self.symbols.lock().unwrap().get(&id).cloned())
        }
    }

    fn find_symbols_by_kind(&self, kind: &SymbolKind) -> impl std::future::Future<Output = Result<Vec<Symbol>>> + Send {
        async move {
            let symbols = self.symbols.lock().unwrap();
            Ok(symbols.values().filter(|s| &s.kind == kind).cloned().collect())
        }
    }

    fn find_symbols_in_file(&self, file_id: i64) -> impl std::future::Future<Output = Result<Vec<Symbol>>> + Send {
        async move {
            let symbols = self.symbols.lock().unwrap();
            Ok(symbols.values().filter(|s| s.file_id == file_id).cloned().collect())
        }
    }

    fn search_symbols(&self, query: &str) -> impl std::future::Future<Output = Result<Vec<Symbol>>> + Send {
        async move {
            let symbols = self.symbols.lock().unwrap();
            let query_lower = query.to_lowercase();
            Ok(symbols
                .values()
                .filter(|s| s.name.to_lowercase().contains(&query_lower))
                .cloned()
                .collect())
        }
    }

    fn count_symbols(&self) -> impl std::future::Future<Output = Result<i64>> + Send {
        async move {
            Ok(self.symbols.lock().unwrap().len() as i64)
        }
    }

    fn delete_symbols_for_file(&self, file_id: i64) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let mut symbols = self.symbols.lock().unwrap();
            symbols.retain(|_, s| s.file_id != file_id);
            Ok(())
        }
    }

    fn insert_reference(&self, reference: &Reference) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            self.symbol_refs.lock().unwrap().push(reference.clone());
            Ok(())
        }
    }

    fn find_symbol_refs(&self, symbol_id: i64) -> impl std::future::Future<Output = Result<Vec<Reference>>> + Send {
        async move {
            let refs = self.symbol_refs.lock().unwrap();
            Ok(refs.iter().filter(|r| r.symbol_id == symbol_id).cloned().collect())
        }
    }

    fn count_symbol_refs(&self) -> impl std::future::Future<Output = Result<i64>> + Send {
        async move {
            Ok(self.symbol_refs.lock().unwrap().len() as i64)
        }
    }

    fn delete_symbol_refs_for_file(&self, file_id: i64) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let mut refs = self.symbol_refs.lock().unwrap();
            refs.retain(|r| r.file_id != file_id);
            Ok(())
        }
    }

    fn insert_relationship(&self, relationship: &Relationship) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            self.relationships.lock().unwrap().push(relationship.clone());
            Ok(())
        }
    }

    fn find_relationships(&self, symbol_id: i64) -> impl std::future::Future<Output = Result<Vec<Relationship>>> + Send {
        async move {
            let rels = self.relationships.lock().unwrap();
            Ok(rels
                .iter()
                .filter(|r| r.from_symbol == symbol_id || r.to_symbol == symbol_id)
                .cloned()
                .collect())
        }
    }

    fn find_outgoing_relationships(&self, symbol_id: i64) -> impl std::future::Future<Output = Result<Vec<Relationship>>> + Send {
        async move {
            let rels = self.relationships.lock().unwrap();
            Ok(rels
                .iter()
                .filter(|r| r.from_symbol == symbol_id)
                .cloned()
                .collect())
        }
    }

    fn find_incoming_relationships(&self, symbol_id: i64) -> impl std::future::Future<Output = Result<Vec<Relationship>>> + Send {
        async move {
            let rels = self.relationships.lock().unwrap();
            Ok(rels
                .iter()
                .filter(|r| r.to_symbol == symbol_id)
                .cloned()
                .collect())
        }
    }

    fn count_relationships(&self) -> impl std::future::Future<Output = Result<i64>> + Send {
        async move {
            Ok(self.relationships.lock().unwrap().len() as i64)
        }
    }

    fn clear(&self) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            self.files.lock().unwrap().clear();
            self.symbols.lock().unwrap().clear();
            self.symbol_refs.lock().unwrap().clear();
            self.relationships.lock().unwrap().clear();
            *self.next_file_id.lock().unwrap() = 1;
            *self.next_symbol_id.lock().unwrap() = 1;
            Ok(())
        }
    }
}

// ─────────────────────────────────────────────────────────
// StorageBackend (unified)
// ─────────────────────────────────────────────────────────

impl StorageBackend for InMemoryBackend {
    fn backend_type(&self) -> &'static str {
        "in_memory"
    }

    fn migrate(&self) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            Ok(())
        }
    }

    fn health_check(&self) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            Ok(())
        }
    }

    fn close(&self) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_session_crud() {
        let backend = InMemoryBackend::new();

        let session = Session::new();
        let id = session.id;
        backend.create_session(&session).await.unwrap();

        let loaded = backend.load_session(&id).await.unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().id, id);

        let sessions = backend.list_sessions().await.unwrap();
        assert_eq!(sessions.len(), 1);

        backend.delete_session(&id).await.unwrap();
        let loaded = backend.load_session(&id).await.unwrap();
        assert!(loaded.is_none());
    }

    #[tokio::test]
    async fn test_save_and_search_messages() {
        let backend = InMemoryBackend::new();
        let session = Session::new();
        let id = session.id;
        backend.create_session(&session).await.unwrap();

        let msg = Message::user("hello world");
        backend.save_message(&id, &msg).await.unwrap();

        let loaded = backend.load_session_full(&id).await.unwrap().unwrap();
        assert_eq!(loaded.messages.len(), 1);

        let results = backend.search_messages("hello").await.unwrap();
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_checkpoint_crud() {
        let backend = InMemoryBackend::new();

        let cp = backend.create_checkpoint("test-cp", Some("desc")).await.unwrap();
        assert_eq!(backend.checkpoint_count().await.unwrap(), 1);

        let loaded = backend.get_checkpoint(&cp).await.unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().name, "test-cp");

        let by_name = backend.query_by_name("test").await.unwrap();
        assert_eq!(by_name.len(), 1);

        backend.delete_checkpoint(&cp).await.unwrap();
        assert_eq!(backend.checkpoint_count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_graph_symbol_crud() {
        let backend = InMemoryBackend::new();

        let file = FileInfo {
            path: "src/main.rs".to_string(),
            hash: "abc123".to_string(),
            language: Some("Rust".to_string()),
            last_modified: None,
        };
        let file_id = backend.insert_file(&file).await.unwrap();
        assert_eq!(file_id, 1);
        assert_eq!(backend.count_files().await.unwrap(), 1);

        let found = backend.get_file_by_path("src/main.rs").await.unwrap();
        assert!(found.is_some());

        let deleted = backend.delete_file("src/main.rs").await.unwrap();
        assert!(deleted);
        assert_eq!(backend.count_files().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_storage_backend_trait() {
        let backend = InMemoryBackend::new();

        assert_eq!(backend.backend_type(), "in_memory");
        backend.migrate().await.unwrap();
        backend.health_check().await.unwrap();
        backend.close().await.unwrap();
    }

    #[tokio::test]
    async fn test_cleanup_old_checkpoints() {
        let backend = InMemoryBackend::new();

        backend.create_checkpoint("cp1", None).await.unwrap();
        backend.create_checkpoint("cp2", None).await.unwrap();
        backend.create_checkpoint("cp3", None).await.unwrap();

        let deleted = backend.cleanup_old_checkpoints(1).await.unwrap();
        assert_eq!(deleted, 2);
        assert_eq!(backend.checkpoint_count().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_storage_stats() {
        let backend = InMemoryBackend::new();
        let stats = backend.storage_stats().await.unwrap();
        assert_eq!(stats.checkpoint_count, 0);
        assert_eq!(stats.tracked_file_count, 0);
    }

    #[tokio::test]
    async fn test_track_file() {
        let backend = InMemoryBackend::new();
        backend.track_file(&PathBuf::from("test.rs")).await.unwrap();
        assert_eq!(backend.tracked_file_count().await.unwrap(), 1);
    }
}
