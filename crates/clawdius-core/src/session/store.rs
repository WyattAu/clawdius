//! Session persistence using the `StorageBackend` trait.
//!
//! `SessionStore` is a thin backward-compatible wrapper around `SqliteBackend`.
//! All database operations delegate to the backend, which can be swapped
//! for PostgreSQL, MariaDB, or in-memory implementations.

use std::path::Path;

use super::types::{Message, Session, SessionId, TokenUsage};
use crate::error::Result;
use crate::storage::{SessionRepository, SqliteBackend, StorageBackend};

/// Helper: run an async closure from a sync context.
///
/// When inside a **multi-threaded** tokio runtime, uses `block_in_place`
/// to avoid starving the executor.  When inside a **current-thread**
/// runtime (the default `#[tokio::test]` flavor) or outside any runtime
/// at all, spawns a dedicated OS thread with its own runtime so we never
/// attempt to nest `block_on` on the current thread.
fn run_async<F>(f: F) -> F::Output
where
    F: std::future::Future + Send,
    F::Output: Send + 'static,
{
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => {
            if handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread {
                // Multi-threaded runtime: block_in_place is safe here.
                let mut fut = std::pin::pin!(f);
                tokio::task::block_in_place(|| handle.block_on(&mut fut))
            } else {
                // Current-thread runtime: cannot nest block_on, so offload
                // to a new OS thread that owns its own runtime.
                std::thread::scope(|s| {
                    s.spawn(|| {
                        let rt = tokio::runtime::Builder::new_current_thread()
                            .enable_all()
                            .build()
                            .expect("failed to create tokio runtime");
                        rt.block_on(f)
                    })
                    .join()
                    .expect("run_async worker thread panicked")
                })
            }
        }
        Err(_) => {
            // No runtime present — create a temporary one (e.g., plain #[test])
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to create tokio runtime");
            rt.block_on(f)
        }
    }
}

/// Session storage backend.
///
/// Wraps a `SqliteBackend` and exposes the synchronous API that existing
/// consumers expect. Each method delegates to the async `StorageBackend`
/// trait methods via `run_async`.
///
/// # Migration Path
///
/// New code should use `Arc<dyn StorageBackend>` directly. This wrapper
/// exists for backward compatibility with the 16+ call sites that reference
/// `SessionStore` by concrete type.
#[derive(Debug)]
pub struct SessionStore {
    backend: SqliteBackend,
}


impl SessionStore {
    /// Open or create session store at path.
    pub fn open(path: &Path) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let backend = SqliteBackend::open(path)?;
        run_async(backend.migrate())?;

        Ok(Self { backend })
    }

    /// Open in-memory store (for testing).
    pub fn in_memory() -> Result<Self> {
        let backend = SqliteBackend::in_memory()?;
        run_async(backend.migrate())?;

        Ok(Self { backend })
    }

    /// Create a new session.
    pub fn create_session(&self, session: &Session) -> Result<()> {
        run_async(self.backend.create_session(session))
    }

    /// Load a session by ID (metadata only, no messages).
    pub fn load_session(&self, id: &SessionId) -> Result<Option<Session>> {
        run_async(self.backend.load_session(id))
    }

    /// Load a session with all messages.
    pub fn load_session_full(&self, id: &SessionId) -> Result<Option<Session>> {
        run_async(self.backend.load_session_full(id))
    }

    /// Save a message to a session.
    pub fn save_message(&self, session_id: &SessionId, message: &Message) -> Result<()> {
        run_async(self.backend.save_message(session_id, message))
    }

    /// Update session token usage.
    pub fn update_token_usage(&self, id: &SessionId, usage: &TokenUsage) -> Result<()> {
        run_async(self.backend.update_token_usage(id, usage))
    }

    /// List all sessions (without messages).
    pub fn list_sessions(&self) -> Result<Vec<Session>> {
        run_async(self.backend.list_sessions())
    }

    /// Delete a session.
    pub fn delete_session(&self, id: &SessionId) -> Result<()> {
        run_async(self.backend.delete_session(id))
    }

    /// Search messages by content.
    pub fn search_messages(&self, query: &str) -> Result<Vec<(SessionId, Message)>> {
        run_async(self.backend.search_messages(query))
    }

    /// Get a reference to the underlying backend.
    #[must_use]
    pub fn backend(&self) -> &SqliteBackend {
        &self.backend
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_session_store_crud() -> Result<()> {
        let temp = NamedTempFile::new()?;
        let store = SessionStore::open(temp.path())?;

        let mut session = Session::new();
        session.title = Some("Test Session".to_string());
        session.meta.provider = Some("anthropic".to_string());
        session.meta.model = Some("claude-3-5-sonnet".to_string());

        store.create_session(&session)?;

        let loaded = store
            .load_session(&session.id)?
            .expect("session should exist");
        assert_eq!(loaded.title, Some("Test Session".to_string()));

        let msg = Message::user("Hello, world!");
        store.save_message(&session.id, &msg)?;

        let full = store
            .load_session_full(&session.id)?
            .expect("session should exist");
        assert_eq!(full.messages.len(), 1);
        assert_eq!(full.messages[0].as_text(), Some("Hello, world!"));

        let sessions = store.list_sessions()?;
        assert_eq!(sessions.len(), 1);

        store.delete_session(&session.id)?;
        let sessions = store.list_sessions()?;
        assert!(sessions.is_empty());

        Ok(())
    }

    #[test]
    fn test_session_store_in_memory() -> Result<()> {
        let store = SessionStore::in_memory()?;

        let session = Session::new();
        let id = session.id;
        store.create_session(&session)?;

        let loaded = store.load_session(&id)?;
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().id, id);

        let sessions = store.list_sessions()?;
        assert_eq!(sessions.len(), 1);

        store.delete_session(&id)?;
        assert!(store.load_session(&id)?.is_none());

        Ok(())
    }

    #[test]
    fn test_session_store_update_token_usage() -> Result<()> {
        let store = SessionStore::in_memory()?;

        let session = Session::new();
        let id = session.id;
        store.create_session(&session)?;

        store.update_token_usage(
            &id,
            &TokenUsage {
                input: 100,
                output: 50,
                cached: 10,
            },
        )?;

        let loaded = store.load_session(&id)?.unwrap();
        assert_eq!(loaded.token_usage.input, 100);
        assert_eq!(loaded.token_usage.output, 50);
        assert_eq!(loaded.token_usage.cached, 10);

        Ok(())
    }

    #[test]
    fn test_session_store_search_messages() -> Result<()> {
        let store = SessionStore::in_memory()?;

        let session = Session::new();
        let id = session.id;
        store.create_session(&session)?;

        store.save_message(&id, &Message::user("find me if you can"))?;

        let results = store.search_messages("find me")?;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, id);

        Ok(())
    }

    #[test]
    fn test_backend_accessor() -> Result<()> {
        let store = SessionStore::in_memory()?;
        let backend = store.backend();
        assert_eq!(backend.backend_type(), "sqlite");
        Ok(())
    }
}
