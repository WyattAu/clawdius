//! Session manager - high-level session operations
//!
//! This module provides [`SessionManager`] for handling multiple sessions.
//!
//! # Design Notes
//!
//! Uses `std::sync::RwLock` instead of `tokio::sync::RwLock` because:
//! - All underlying `SQLite` operations are blocking
//! - No benefit to async locks when I/O is blocking anyway
//! - Avoids "Cannot block the current thread from within a runtime" panics

use std::sync::{Arc, RwLock};

use super::types::{Message, Session, SessionId};
use super::{Compactor, SessionStore};
use crate::config::Config;
use crate::error::{ErrorHelpers, Result};

/// Inner state for `SessionManager`
struct SessionManagerInner {
    store: SessionStore,
    compactor: Compactor,
    active_session: RwLock<Option<SessionId>>,
    #[allow(dead_code)]
    config: Config,
}

/// Session manager for handling multiple sessions
///
/// # Thread Safety
///
/// Uses `Arc<RwLock>` for the active session tracking. All `SQLite` operations
/// are inherently blocking, so async methods are provided for convenience
/// but still perform blocking I/O internally.
pub struct SessionManager {
    inner: Arc<SessionManagerInner>,
}

impl Clone for SessionManager {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl SessionManager {
    /// Create a new session manager
    ///
    /// # Errors
    ///
    /// Returns an error if the session store cannot be opened.
    #[allow(clippy::arc_with_non_send_sync)]
    pub fn new(config: &Config) -> Result<Self> {
        let store = SessionStore::open(&config.storage.sessions_path)?;
        let compactor = Compactor::new(config.session.clone());

        Ok(Self {
            inner: Arc::new(SessionManagerInner {
                store,
                compactor,
                active_session: RwLock::new(None),
                config: config.clone(),
            }),
        })
    }

    /// Create a new session
    ///
    /// # Errors
    ///
    /// Returns an error if the session cannot be created or saved.
    pub fn create_session(&self) -> Result<Session> {
        let session = Session::new();
        self.inner.store.create_session(&session)?;

        let mut active = self
            .inner
            .active_session
            .write()
            .map_err(|e| crate::Error::Session(format!("Lock error: {e}")))?;
        *active = Some(session.id);

        Ok(session)
    }

    /// Get or create active session (synchronous)
    ///
    /// # Errors
    ///
    /// Returns an error if the session cannot be loaded or created.
    pub fn get_or_create_active(&self) -> Result<Session> {
        let active_id = *self
            .inner
            .active_session
            .read()
            .map_err(|e| crate::Error::Session(format!("Lock error: {e}")))?;

        if let Some(id) = active_id {
            if let Some(session) = self.inner.store.load_session_full(&id)? {
                return Ok(session);
            }
        }

        self.create_session()
    }

    /// Get or create active session (async wrapper)
    ///
    /// Note: This still performs blocking I/O internally.
    pub async fn get_or_create_active_async(&self) -> Result<Session> {
        self.get_or_create_active()
    }

    /// Load a session by ID
    ///
    /// # Errors
    ///
    /// Returns an error if the session cannot be loaded.
    pub fn load_session(&self, id: &SessionId) -> Result<Option<Session>> {
        self.inner.store.load_session_full(id)
    }

    /// Save a message to the active session
    ///
    /// # Errors
    ///
    /// Returns an error if there is no active session or the message cannot be saved.
    pub async fn save_message(&self, message: &Message) -> Result<()> {
        let active_id = {
            let active = self
                .inner
                .active_session
                .read()
                .map_err(|e| crate::Error::Session(format!("Lock error: {e}")))?;
            *active.as_ref().ok_or_else(|| {
                crate::Error::Session(ErrorHelpers::session_not_found("active").to_string())
            })?
        };

        self.inner.store.save_message(&active_id, message)
    }

    /// Add a message and check for compaction
    ///
    /// # Errors
    ///
    /// Returns an error if the message cannot be saved or compaction fails.
    pub async fn add_message(&self, session: &mut Session, message: Message) -> Result<()> {
        self.inner.store.save_message(&session.id, &message)?;

        session.add_message(message);

        if self.inner.compactor.needs_compaction(session) {
            let summary = self.inner.compactor.compact(session).await?;

            self.inner
                .store
                .update_token_usage(&session.id, &session.token_usage)?;

            tracing::info!(
                session_id = %session.id,
                summarized = summary.summarized_count,
                tokens_before = summary.tokens_before,
                tokens_after = summary.tokens_after,
                "Session compacted"
            );
        }

        Ok(())
    }

    /// List all sessions
    ///
    /// # Errors
    ///
    /// Returns an error if sessions cannot be listed.
    pub fn list_sessions(&self) -> Result<Vec<Session>> {
        self.inner.store.list_sessions()
    }

    /// Delete a session
    ///
    /// # Errors
    ///
    /// Returns an error if the session cannot be deleted.
    pub fn delete_session(&self, id: &SessionId) -> Result<()> {
        self.inner.store.delete_session(id)
    }

    /// Set active session
    pub async fn set_active(&self, id: Option<SessionId>) {
        if let Ok(mut active) = self.inner.active_session.write() {
            *active = id;
        }
    }

    /// Get active session ID
    pub async fn get_active_id(&self) -> Option<SessionId> {
        self.inner
            .active_session
            .read()
            .ok()
            .and_then(|guard| *guard)
    }

    /// Search messages across all sessions
    ///
    /// # Errors
    ///
    /// Returns an error if the search fails.
    pub fn search_messages(&self, query: &str) -> Result<Vec<(SessionId, Message)>> {
        self.inner.store.search_messages(query)
    }
}
