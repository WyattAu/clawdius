//! SQLite storage backend implementation.
//!
//! Implements all three domain traits (`SessionRepository`, `TimelineRepository`,
//! `GraphRepository`) and the unified `StorageBackend` trait using `rusqlite`.
//!
//! The session operations are fully ported from the legacy `SessionStore`.
//! Timeline and graph operations are delegated to the existing store types
//! (`TimelineStore`, `GraphStore`) until they are independently migrated.

mod graph;
mod migrations;
mod sessions;
mod timeline;
mod workspaces;

use super::backend::StorageBackend;
use super::error::StorageError;
use crate::error::Result;
use rusqlite::Connection;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct SqliteBackend {
    conn: std::sync::Mutex<Connection>,
    path: PathBuf,
}

impl SqliteBackend {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path).map_err(|e| StorageError::Connection(e.to_string()))?;
        let backend = Self {
            conn: std::sync::Mutex::new(conn),
            path: path.to_path_buf(),
        };
        Ok(backend)
    }

    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()
            .map_err(|e| StorageError::Connection(e.to_string()))?;
        let backend = Self {
            conn: std::sync::Mutex::new(conn),
            path: PathBuf::from(":memory:"),
        };
        Ok(backend)
    }

    pub fn with_conn<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&Connection) -> Result<T>,
    {
        let guard = self
            .conn
            .lock()
            .map_err(|e| StorageError::Connection(format!("lock poisoned: {e}")))?;
        f(&guard)
    }
}

impl StorageBackend for SqliteBackend {
    fn backend_type(&self) -> &'static str {
        "sqlite"
    }

    fn migrate(&self) -> impl std::future::Future<Output = Result<()>> + Send {
        async move { self.initialize() }
    }

    fn health_check(&self) -> impl std::future::Future<Output = Result<()>> + Send {
        let this = self;
        async move {
            this.with_conn(|conn| {
                conn.execute_batch("SELECT 1")
                    .map_err(|e| StorageError::Connection(e.to_string()))?;
                Ok(())
            })
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
    use crate::graph_rag::ast::FileInfo;
    use crate::session::types::{Message, Session, TokenUsage};
    use crate::storage::backend::{GraphRepository, SessionRepository, TimelineRepository};
    use crate::timeline::StorageStats;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_sqlite_session_crud() {
        let backend = SqliteBackend::in_memory().unwrap();
        backend.migrate().await.unwrap();

        let session = Session::new();
        let id = session.id;
        backend.create_session(&session).await.unwrap();

        let loaded = backend.load_session(&id).await.unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().id, id);

        let sessions = backend.list_sessions().await.unwrap();
        assert_eq!(sessions.len(), 1);

        backend.delete_session(&id).await.unwrap();
        assert!(backend.load_session(&id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_sqlite_save_message() {
        let backend = SqliteBackend::in_memory().unwrap();
        backend.migrate().await.unwrap();

        let session = Session::new();
        let id = session.id;
        backend.create_session(&session).await.unwrap();

        let msg = Message::user("hello world");
        backend.save_message(&id, &msg).await.unwrap();

        let full = backend.load_session_full(&id).await.unwrap().unwrap();
        assert_eq!(full.messages.len(), 1);
        assert_eq!(full.messages[0].as_text(), Some("hello world"));
    }

    #[tokio::test]
    async fn test_sqlite_search_messages() {
        let backend = SqliteBackend::in_memory().unwrap();
        backend.migrate().await.unwrap();

        let session = Session::new();
        let id = session.id;
        backend.create_session(&session).await.unwrap();

        backend
            .save_message(&id, &Message::user("find me if you can"))
            .await
            .unwrap();

        let results = backend.search_messages("find me").await.unwrap();
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_sqlite_update_token_usage() {
        let backend = SqliteBackend::in_memory().unwrap();
        backend.migrate().await.unwrap();

        let session = Session::new();
        let id = session.id;
        backend.create_session(&session).await.unwrap();

        backend
            .update_token_usage(
                &id,
                &TokenUsage {
                    input: 100,
                    output: 50,
                    cached: 10,
                },
            )
            .await
            .unwrap();

        let loaded = backend.load_session(&id).await.unwrap().unwrap();
        assert_eq!(loaded.token_usage.input, 100);
        assert_eq!(loaded.token_usage.output, 50);
        assert_eq!(loaded.token_usage.cached, 10);
    }

    #[tokio::test]
    async fn test_sqlite_checkpoint_crud() {
        let backend = SqliteBackend::in_memory().unwrap();
        backend.migrate().await.unwrap();

        let cp = backend
            .create_checkpoint("test-cp", Some("description"))
            .await
            .unwrap();
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
    async fn test_sqlite_graph_file_crud() {
        let backend = SqliteBackend::in_memory().unwrap();
        backend.migrate().await.unwrap();

        let file = FileInfo {
            path: "src/main.rs".to_string(),
            hash: "abc123".to_string(),
            language: Some("Rust".to_string()),
            last_modified: None,
        };

        let file_id = backend.insert_file(&file).await.unwrap();
        assert!(file_id > 0);

        let found = backend.get_file_by_path("src/main.rs").await.unwrap();
        assert!(found.is_some());

        let by_id = backend.get_file_by_id(file_id).await.unwrap();
        assert!(by_id.is_some());

        assert_eq!(backend.count_files().await.unwrap(), 1);

        let deleted = backend.delete_file("src/main.rs").await.unwrap();
        assert!(deleted);
        assert_eq!(backend.count_files().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_sqlite_storage_backend_trait() {
        let backend = SqliteBackend::in_memory().unwrap();
        assert_eq!(backend.backend_type(), "sqlite");
        backend.migrate().await.unwrap();
        backend.health_check().await.unwrap();
        backend.close().await.unwrap();
    }

    #[tokio::test]
    async fn test_sqlite_storage_stats() {
        let backend = SqliteBackend::in_memory().unwrap();
        backend.migrate().await.unwrap();
        let stats = backend.storage_stats().await.unwrap();
        assert_eq!(stats.checkpoint_count, 0);
        assert_eq!(stats.tracked_file_count, 0);
    }

    #[tokio::test]
    async fn test_sqlite_track_file() {
        let backend = SqliteBackend::in_memory().unwrap();
        backend.migrate().await.unwrap();
        backend.track_file(&PathBuf::from("test.rs")).await.unwrap();
        assert_eq!(backend.tracked_file_count().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_sqlite_cleanup_old_checkpoints() {
        let backend = SqliteBackend::in_memory().unwrap();
        backend.migrate().await.unwrap();

        backend.create_checkpoint("cp1", None).await.unwrap();
        backend.create_checkpoint("cp2", None).await.unwrap();
        backend.create_checkpoint("cp3", None).await.unwrap();

        let deleted = backend.cleanup_old_checkpoints(1).await.unwrap();
        assert_eq!(deleted, 2);
        assert_eq!(backend.checkpoint_count().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_sqlite_health_check() {
        let backend = SqliteBackend::in_memory().unwrap();
        backend.migrate().await.unwrap();
        backend.health_check().await.unwrap();
    }

    #[tokio::test]
    async fn test_sqlite_graph_clear() {
        let backend = SqliteBackend::in_memory().unwrap();
        backend.migrate().await.unwrap();

        let file = FileInfo {
            path: "test.rs".to_string(),
            hash: "hash".to_string(),
            language: None,
            last_modified: None,
        };
        backend.insert_file(&file).await.unwrap();
        assert_eq!(backend.count_files().await.unwrap(), 1);

        backend.clear().await.unwrap();
        assert_eq!(backend.count_files().await.unwrap(), 0);
    }
}
