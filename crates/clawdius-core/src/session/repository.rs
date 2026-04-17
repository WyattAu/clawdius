use super::types::{Message, Session, SessionId, TokenUsage};
use crate::error::Result;
use std::fmt;

pub trait SessionRepository: Send + fmt::Debug {
    fn create_session(&self, session: &Session) -> Result<()>;
    fn load_session(&self, id: &SessionId) -> Result<Option<Session>>;
    fn load_session_full(&self, id: &SessionId) -> Result<Option<Session>>;
    fn save_message(&self, session_id: &SessionId, message: &Message) -> Result<()>;
    fn update_token_usage(&self, id: &SessionId, usage: &TokenUsage) -> Result<()>;
    fn list_sessions(&self) -> Result<Vec<Session>>;
    fn delete_session(&self, id: &SessionId) -> Result<()>;
    fn search_messages(&self, query: &str) -> Result<Vec<(SessionId, Message)>>;
}

/// Wrapper that makes any `SessionRepository` (including `!Sync` ones like `SessionStore`)
/// usable as `Arc<dyn SessionRepository>` by serializing access behind a `Mutex`.
///
/// `SessionStore` uses `rusqlite::Connection` internally which is `!Sync`. This wrapper
/// provides interior mutability via `std::sync::Mutex` so it can be shared across threads.
pub struct MutexRepository<R>(std::sync::Mutex<R>);

impl<R: SessionRepository> MutexRepository<R> {
    /// Wrap a repository in a mutex for thread-safe sharing.
    pub const fn new(repo: R) -> Self {
        Self(std::sync::Mutex::new(repo))
    }
}

impl<R: SessionRepository> fmt::Debug for MutexRepository<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MutexRepository").finish_non_exhaustive()
    }
}

impl<R: SessionRepository> SessionRepository for MutexRepository<R> {
    fn create_session(&self, session: &Session) -> Result<()> {
        self.0
            .lock()
            .map_err(|e| crate::Error::Session(format!("Lock error: {e}")))?
            .create_session(session)
    }
    fn load_session(&self, id: &SessionId) -> Result<Option<Session>> {
        self.0
            .lock()
            .map_err(|e| crate::Error::Session(format!("Lock error: {e}")))?
            .load_session(id)
    }
    fn load_session_full(&self, id: &SessionId) -> Result<Option<Session>> {
        self.0
            .lock()
            .map_err(|e| crate::Error::Session(format!("Lock error: {e}")))?
            .load_session_full(id)
    }
    fn save_message(&self, session_id: &SessionId, message: &Message) -> Result<()> {
        self.0
            .lock()
            .map_err(|e| crate::Error::Session(format!("Lock error: {e}")))?
            .save_message(session_id, message)
    }
    fn update_token_usage(&self, id: &SessionId, usage: &TokenUsage) -> Result<()> {
        self.0
            .lock()
            .map_err(|e| crate::Error::Session(format!("Lock error: {e}")))?
            .update_token_usage(id, usage)
    }
    fn list_sessions(&self) -> Result<Vec<Session>> {
        self.0
            .lock()
            .map_err(|e| crate::Error::Session(format!("Lock error: {e}")))?
            .list_sessions()
    }
    fn delete_session(&self, id: &SessionId) -> Result<()> {
        self.0
            .lock()
            .map_err(|e| crate::Error::Session(format!("Lock error: {e}")))?
            .delete_session(id)
    }
    fn search_messages(&self, query: &str) -> Result<Vec<(SessionId, Message)>> {
        self.0
            .lock()
            .map_err(|e| crate::Error::Session(format!("Lock error: {e}")))?
            .search_messages(query)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::types::{Message, MessageRole, Session, SessionMeta, TokenUsage};
    use std::sync::Arc;

    /// In-memory session repository for testing the trait abstraction.
    #[derive(Debug)]
    struct InMemoryRepository {
        sessions: std::sync::Mutex<std::collections::HashMap<uuid::Uuid, Session>>,
    }

    impl InMemoryRepository {
        fn new() -> Self {
            Self {
                sessions: std::sync::Mutex::new(std::collections::HashMap::new()),
            }
        }
    }

    impl SessionRepository for InMemoryRepository {
        fn create_session(&self, session: &Session) -> Result<()> {
            self.sessions
                .lock()
                .unwrap()
                .insert(session.id.as_uuid().to_owned(), session.clone());
            Ok(())
        }

        fn load_session(&self, id: &SessionId) -> Result<Option<Session>> {
            Ok(self.sessions.lock().unwrap().get(id.as_uuid()).cloned())
        }

        fn load_session_full(&self, id: &SessionId) -> Result<Option<Session>> {
            // Same as load_session for in-memory
            self.load_session(id)
        }

        fn save_message(&self, session_id: &SessionId, message: &Message) -> Result<()> {
            let mut sessions = self.sessions.lock().unwrap();
            if let Some(session) = sessions.get_mut(session_id.as_uuid()) {
                session.messages.push(message.clone());
                Ok(())
            } else {
                Err(crate::Error::SessionNotFound {
                    id: session_id.to_string(),
                })
            }
        }

        fn update_token_usage(&self, id: &SessionId, usage: &TokenUsage) -> Result<()> {
            let mut sessions = self.sessions.lock().unwrap();
            if let Some(session) = sessions.get_mut(id.as_uuid()) {
                session.token_usage.add(usage);
                Ok(())
            } else {
                Err(crate::Error::SessionNotFound { id: id.to_string() })
            }
        }

        fn list_sessions(&self) -> Result<Vec<Session>> {
            Ok(self.sessions.lock().unwrap().values().cloned().collect())
        }

        fn delete_session(&self, id: &SessionId) -> Result<()> {
            self.sessions.lock().unwrap().remove(id.as_uuid());
            Ok(())
        }

        fn search_messages(&self, _query: &str) -> Result<Vec<(SessionId, Message)>> {
            // Simple stub: return empty
            Ok(Vec::new())
        }
    }

    #[test]
    fn test_inmemory_repository_crud() {
        let repo = InMemoryRepository::new();

        // Create
        let session = Session::new();
        let id = session.id;
        repo.create_session(&session).unwrap();

        // Load
        let loaded = repo.load_session(&id).unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().id, id);

        // List
        let sessions = repo.list_sessions().unwrap();
        assert_eq!(sessions.len(), 1);

        // Delete
        repo.delete_session(&id).unwrap();
        let loaded = repo.load_session(&id).unwrap();
        assert!(loaded.is_none());
    }

    #[test]
    fn test_inmemory_repository_save_message() {
        let repo = InMemoryRepository::new();
        let session = Session::new();
        let id = session.id;
        repo.create_session(&session).unwrap();

        let msg = Message::user("hello");
        repo.save_message(&id, &msg).unwrap();

        let loaded = repo.load_session(&id).unwrap().unwrap();
        assert_eq!(loaded.messages.len(), 1);
        assert_eq!(loaded.messages[0].role, MessageRole::User);
    }

    #[test]
    fn test_inmemory_repository_update_token_usage() {
        let repo = InMemoryRepository::new();
        let session = Session::new();
        let id = session.id;
        repo.create_session(&session).unwrap();

        repo.update_token_usage(
            &id,
            &TokenUsage {
                input: 100,
                output: 50,
                cached: 0,
            },
        )
        .unwrap();

        let loaded = repo.load_session(&id).unwrap().unwrap();
        assert_eq!(loaded.token_usage.input, 100);
        assert_eq!(loaded.token_usage.output, 50);
    }

    #[test]
    fn test_mutex_repository_wraps_correctly() {
        let inner = InMemoryRepository::new();
        let repo: Arc<dyn SessionRepository> = Arc::new(MutexRepository::new(inner));

        let session = Session::new();
        let id = session.id;
        repo.create_session(&session).unwrap();

        let loaded = repo.load_session(&id).unwrap();
        assert!(loaded.is_some());

        let sessions = repo.list_sessions().unwrap();
        assert_eq!(sessions.len(), 1);
    }

    #[test]
    fn test_mutex_repository_delete() {
        let inner = InMemoryRepository::new();
        let repo = MutexRepository::new(inner);

        let session = Session::new();
        let id = session.id;
        repo.create_session(&session).unwrap();
        assert!(repo.load_session(&id).unwrap().is_some());

        repo.delete_session(&id).unwrap();
        assert!(repo.load_session(&id).unwrap().is_none());
    }
}
