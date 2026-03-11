//! Session manager - high-level session operations

use std::sync::Arc;

use tokio::sync::RwLock;

use super::{Compactor, Message, Session, SessionId, SessionStore};
use crate::config::Config;
use crate::error::{ErrorHelpers, Result};

/// Session manager for handling multiple sessions
pub struct SessionManager {
    store: SessionStore,
    compactor: Compactor,
    active_session: Arc<RwLock<Option<SessionId>>>,
    config: Config,
}

impl SessionManager {
    /// Create a new session manager
    pub fn new(config: &Config) -> Result<Self> {
        let store = SessionStore::open(&config.storage.sessions_path)?;
        let compactor = Compactor::new(config.session.clone());

        Ok(Self {
            store,
            compactor,
            active_session: Arc::new(RwLock::new(None)),
            config: config.clone(),
        })
    }

    /// Create a new session
    pub fn create_session(&self) -> Result<Session> {
        let provider = self
            .config
            .llm
            .default_provider
            .clone()
            .unwrap_or_else(|| "anthropic".to_string());

        let model = match provider.as_str() {
            "anthropic" => self
                .config
                .llm
                .anthropic
                .as_ref()
                .and_then(|c| c.model.clone())
                .unwrap_or_else(|| "claude-3-5-sonnet-20241022".to_string()),
            "openai" => self
                .config
                .llm
                .openai
                .as_ref()
                .and_then(|c| c.model.clone())
                .unwrap_or_else(|| "gpt-4o".to_string()),
            "ollama" => self
                .config
                .llm
                .ollama
                .as_ref()
                .and_then(|c| c.model.clone())
                .unwrap_or_else(|| "llama3.2".to_string()),
            "zai" => self
                .config
                .llm
                .zai
                .as_ref()
                .and_then(|c| c.model.clone())
                .unwrap_or_else(|| "zai-default".to_string()),
            _ => "claude-3-5-sonnet-20241022".to_string(),
        };

        let session = Session::with_provider_model(provider, model);
        self.store.create_session(&session)?;

        // Set as active
        let mut active = self.active_session.blocking_write();
        *active = Some(session.id);

        Ok(session)
    }

    /// Get or create active session
    pub fn get_or_create_active(&self) -> Result<Session> {
        let active_id = {
            let active = self.active_session.blocking_read();
            *active
        };

        if let Some(id) = active_id {
            if let Some(session) = self.store.load_session_full(&id)? {
                return Ok(session);
            }
        }

        // Create new session
        self.create_session()
    }

    /// Load a session by ID
    pub fn load_session(&self, id: &SessionId) -> Result<Option<Session>> {
        self.store.load_session_full(id)
    }

    /// Save a message to the active session
    pub async fn save_message(&self, message: &Message) -> Result<()> {
        let active_id = {
            let active = self.active_session.read().await;
            active.ok_or_else(|| {
                crate::Error::Session(ErrorHelpers::session_not_found("active").to_string())
            })?
        };

        self.store.save_message(&active_id, message)
    }

    /// Add a message and check for compaction
    pub async fn add_message(&self, session: &mut Session, message: Message) -> Result<()> {
        // Save message
        self.store.save_message(&session.id, &message)?;

        // Add to in-memory session
        session.add_message(message);

        // Check if compaction needed
        if self.compactor.needs_compaction(session) {
            let summary = self.compactor.compact(session).await?;

            // Update session in store
            // (compaction replaces old messages with summary)
            self.store
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
    pub fn list_sessions(&self) -> Result<Vec<Session>> {
        self.store.list_sessions()
    }

    /// Delete a session
    pub fn delete_session(&self, id: &SessionId) -> Result<()> {
        self.store.delete_session(id)
    }

    /// Set active session
    pub async fn set_active(&self, id: Option<SessionId>) {
        let mut active = self.active_session.write().await;
        *active = id;
    }

    /// Get active session ID
    pub async fn get_active_id(&self) -> Option<SessionId> {
        *self.active_session.read().await
    }

    /// Search messages across all sessions
    pub fn search_messages(&self, query: &str) -> Result<Vec<(SessionId, Message)>> {
        self.store.search_messages(query)
    }
}
