//! Gateway-to-agent bridge.
//!
//! Implements [`MessageHandler`] to connect the messaging gateway
//! to the Clawdius agent engine (LLM + sessions + tools).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::RwLock;

use crate::adapter::{IncomingMessage, Platform};
use crate::error::GatewayError;
use crate::gateway::MessageHandler;
use clawdius_core::llm::{LlmResponse, LlmResponseCache, LlmTokenUsage};

/// System prompt for the gateway chat mode.
const GATEWAY_SYSTEM_PROMPT: &str = "\
You are Clawdius, an AI coding assistant. You help users with software \
development tasks including writing code, debugging, refactoring, and \
explaining code. You have access to tools for file operations, shell \
commands, and more. Be concise and helpful. When showing code, use \
appropriate markdown formatting.";

/// Session mapping: `platform:chat_id` → `session_id`.
type SessionMap = HashMap<String, String>;

/// Bridges the messaging gateway to the Clawdius agent engine.
///
/// Each unique `(platform, chat_id)` pair gets its own session so
/// conversations are isolated per chat room/channel.
pub struct ClawdiusHandler {
    /// Path to the clawdius config file.
    config_path: Option<PathBuf>,

    /// LLM provider name override.
    provider: Option<String>,

    /// LLM model name override.
    model: Option<String>,

    /// Session manager for persistence.
    session_manager: Arc<RwLock<Option<clawdius_core::SessionManager>>>,

    /// LLM client (lazy-initialized).
    llm_client: Arc<RwLock<Option<Arc<dyn clawdius_core::llm::LlmClient>>>>,

    /// Mapping from "`platform:chat_id`" to `session_id`.
    session_map: Arc<RwLock<SessionMap>>,

    /// Per-session message history (`chat_id` → messages).
    /// Kept in memory for context; also persisted to session store.
    message_history: Arc<RwLock<HashMap<String, Vec<clawdius_core::llm::ChatMessage>>>>,

    /// System prompt override.
    system_prompt: String,

    /// Maximum history messages to include in LLM context.
    max_history: usize,

    /// LLM response cache (provider-agnostic, keyed by message hash).
    cache: Arc<LlmResponseCache>,

    /// Optional audit logger for compliance.
    #[cfg(feature = "audit")]
    audit: Option<Arc<clawdius_core::audit::AuditManager>>,
}

impl ClawdiusHandler {
    /// Create a new handler with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config_path: None,
            provider: None,
            model: None,
            session_manager: Arc::new(RwLock::new(None)),
            llm_client: Arc::new(RwLock::new(None)),
            session_map: Arc::new(RwLock::new(HashMap::new())),
            message_history: Arc::new(RwLock::new(HashMap::new())),
            system_prompt: GATEWAY_SYSTEM_PROMPT.to_string(),
            max_history: 50,
            cache: Arc::new(LlmResponseCache::new(Duration::from_secs(300), 1000)),
            #[cfg(feature = "audit")]
            audit: None,
        }
    }

    /// Set the config file path.
    #[must_use]
    pub fn with_config_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.config_path = Some(path.into());
        self
    }

    /// Set the LLM provider override.
    #[must_use]
    pub fn with_provider(mut self, provider: impl Into<String>) -> Self {
        self.provider = Some(provider.into());
        self
    }

    /// Set the LLM model override.
    #[must_use]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Enable audit logging with the given manager (shared via Arc).
    #[cfg(feature = "audit")]
    #[must_use]
    pub fn with_audit_manager(mut self, manager: Arc<clawdius_core::audit::AuditManager>) -> Self {
        self.audit = Some(manager);
        self
    }

    /// Set a custom system prompt.
    #[must_use]
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = prompt.into();
        self
    }

    /// Set the maximum history messages.
    #[must_use]
    pub const fn with_max_history(mut self, max: usize) -> Self {
        self.max_history = max;
        self
    }

    /// Set a custom response cache.
    #[must_use]
    pub fn with_cache(mut self, cache: Arc<LlmResponseCache>) -> Self {
        self.cache = cache;
        self
    }

    /// Get cache statistics.
    #[must_use]
    pub fn cache_stats(&self) -> clawdius_core::llm::cache::CacheStats {
        self.cache.stats()
    }

    /// Get the session key for a given platform + `chat_id`.
    fn session_key(platform: &Platform, chat_id: &str) -> String {
        format!("{platform}:{chat_id}")
    }

    /// Initialize the session manager and LLM client (lazy).
    async fn ensure_initialized(&self) -> Result<(), GatewayError> {
        // Check if already initialized
        {
            let llm = self.llm_client.read().await;
            if llm.is_some() {
                return Ok(());
            }
        }

        // Load config
        let config = if let Some(ref path) = self.config_path {
            clawdius_core::Config::load(path)
                .map_err(|e| GatewayError::Agent(format!("config load failed: {e}")))?
        } else {
            clawdius_core::Config::load_default()
                .map_err(|e| GatewayError::Agent(format!("config load failed: {e}")))?
        };

        // Create session manager
        let session_mgr = clawdius_core::SessionManager::new(&config)
            .map_err(|e| GatewayError::Agent(format!("session manager init failed: {e}")))?;

        {
            let mut mgr = self.session_manager.write().await;
            *mgr = Some(session_mgr);
        }

        // Create LLM client
        let provider_name = self
            .provider
            .as_deref()
            .or(config.llm.default_provider.as_deref())
            .unwrap_or("deepseek");

        let mut llm_config =
            clawdius_core::llm::ResolvedLlmConfig::from_config(&config.llm, provider_name)
                .map_err(|e| GatewayError::Agent(format!("LLM config failed: {e}")))?;

        if let Some(ref model) = self.model {
            llm_config.model = model.clone();
        }

        let client: Arc<dyn clawdius_core::llm::LlmClient> = Arc::new(
            clawdius_core::llm::create_provider(&llm_config)
                .map_err(|e| GatewayError::Agent(format!("LLM provider creation failed: {e}")))?,
        );

        {
            let mut llm = self.llm_client.write().await;
            *llm = Some(client);
        }

        Ok(())
    }

    /// Get or create a session for the given platform + `chat_id`.
    async fn get_or_create_session(
        &self,
        platform: &Platform,
        chat_id: &str,
    ) -> Result<(), GatewayError> {
        let key = Self::session_key(platform, chat_id);

        // Check if we already have a session for this chat
        {
            let map = self.session_map.read().await;
            if map.contains_key(&key) {
                return Ok(());
            }
        }

        // Create a new session
        let session_mgr = self.session_manager.read().await;
        let session_mgr = session_mgr
            .as_ref()
            .ok_or_else(|| GatewayError::Agent("session manager not initialized".to_string()))?;

        let session = session_mgr
            .create_session()
            .map_err(|e| GatewayError::Agent(format!("session creation failed: {e}")))?;

        // Store the mapping
        {
            let mut map = self.session_map.write().await;
            map.insert(key, session.id.to_string());
        }

        // Initialize empty history
        {
            let mut history = self.message_history.write().await;
            history.insert(Self::session_key(platform, chat_id), Vec::new());
        }

        Ok(())
    }

    /// Build the LLM message list for a given chat.
    async fn build_messages(
        &self,
        platform: &Platform,
        chat_id: &str,
        user_text: &str,
    ) -> Result<Vec<clawdius_core::llm::ChatMessage>, GatewayError> {
        let key = Self::session_key(platform, chat_id);

        let mut history = self.message_history.write().await;
        let history = history.entry(key.clone()).or_insert_with(Vec::new);

        // Build messages: system prompt + history (truncated) + new user message
        let mut messages = Vec::new();

        // System prompt
        messages.push(clawdius_core::llm::ChatMessage {
            role: clawdius_core::llm::ChatRole::System,
            content: self.system_prompt.clone(),
        });

        // Add platform context to the first user message
        let platform_context = format!("[Message from {} via {}]", chat_id, platform.as_str());

        // Add history (limited)
        let start = if history.len() > self.max_history {
            history.len() - self.max_history
        } else {
            0
        };

        for msg in &history[start..] {
            messages.push(msg.clone());
        }

        // Add the new user message with platform context
        messages.push(clawdius_core::llm::ChatMessage {
            role: clawdius_core::llm::ChatRole::User,
            content: format!("{platform_context}\n{user_text}"),
        });

        Ok(messages)
    }

    /// Save a message to history.
    async fn save_to_history(
        &self,
        platform: &Platform,
        chat_id: &str,
        role: clawdius_core::llm::ChatRole,
        content: &str,
    ) {
        let key = Self::session_key(platform, chat_id);
        let mut history = self.message_history.write().await;
        let history = history.entry(key).or_insert_with(Vec::new);
        history.push(clawdius_core::llm::ChatMessage {
            role,
            content: content.to_string(),
        });
    }
}

impl Default for ClawdiusHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MessageHandler for ClawdiusHandler {
    async fn handle_message(&self, message: IncomingMessage) -> Result<String, GatewayError> {
        // 1. Ensure we're initialized
        self.ensure_initialized().await?;

        // 2. Get or create session for this chat
        self.get_or_create_session(&message.platform, &message.chat_id)
            .await?;

        // 3. Build LLM messages with history
        let messages = self
            .build_messages(&message.platform, &message.chat_id, &message.text)
            .await?;

        // 4. Check cache for exact message match
        if let Some(cached) = self.cache.get(&messages) {
            self.save_to_history(
                &message.platform,
                &message.chat_id,
                clawdius_core::llm::ChatRole::User,
                &message.text,
            )
            .await;
            self.save_to_history(
                &message.platform,
                &message.chat_id,
                clawdius_core::llm::ChatRole::Assistant,
                &cached.text,
            )
            .await;
            return Ok(cached.text);
        }

        // 5. Call the LLM
        let llm = self.llm_client.read().await;
        let llm = llm
            .as_ref()
            .ok_or_else(|| GatewayError::Agent("LLM client not initialized".to_string()))?;

        let provider_name = self.provider.as_deref().unwrap_or("unknown");
        let model_name = self.model.as_deref().unwrap_or("unknown");

        let start = std::time::Instant::now();
        let llm_result = llm.chat(messages.clone()).await;
        let duration = start.elapsed();

        match &llm_result {
            Ok(response) => {
                clawdius_core::metrics::record_llm_request(
                    provider_name,
                    model_name,
                    duration,
                    0, // prompt tokens (not available from simple chat())
                    0, // completion tokens
                    true,
                );
                clawdius_core::metrics::record_session_count(self.session_map.read().await.len());
                let _ = response; // already used below
            },
            Err(e) => {
                clawdius_core::metrics::record_llm_request(
                    provider_name,
                    model_name,
                    duration,
                    0,
                    0,
                    false,
                );
                return Err(GatewayError::Agent(format!("LLM call failed: {e}")));
            },
        }

        let response =
            llm_result.map_err(|e| GatewayError::Agent(format!("LLM call failed: {e}")))?;

        // Audit log the chat event (if audit is configured)
        #[cfg(feature = "audit")]
        if let Some(ref audit) = self.audit {
            let mut entry = clawdius_core::audit::events::chat_event(
                &message.chat_id,
                self.model.as_deref().unwrap_or("default"),
            );
            entry.user_id = Some(message.user.id.clone());
            entry.resource = Some(format!("{}/{}", message.platform.as_str(), message.chat_id));
            entry.details = serde_json::json!({
                "provider": provider_name,
                "duration_ms": duration.as_millis(),
                "message_length": message.text.len(),
                "user_name": message.user.name,
            });
            audit.buffer(entry);
        }

        // 6. Store in cache for future identical requests
        self.cache.insert(
            &messages,
            LlmResponse {
                text: response.clone(),
                usage: LlmTokenUsage::default(),
                tool_calls: vec![],
            },
        );

        // 7. Save to history
        self.save_to_history(
            &message.platform,
            &message.chat_id,
            clawdius_core::llm::ChatRole::User,
            &message.text,
        )
        .await;

        self.save_to_history(
            &message.platform,
            &message.chat_id,
            clawdius_core::llm::ChatRole::Assistant,
            &response,
        )
        .await;

        Ok(response)
    }
}

// ─────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_key_format() {
        let key = ClawdiusHandler::session_key(&Platform::Telegram, "chat123");
        assert_eq!(key, "telegram:chat123");

        let key = ClawdiusHandler::session_key(&Platform::Discord, "456");
        assert_eq!(key, "discord:456");
    }

    #[test]
    fn test_builder_pattern() {
        let handler = ClawdiusHandler::new()
            .with_provider("anthropic")
            .with_model("claude-3-5-sonnet")
            .with_system_prompt("Custom prompt")
            .with_max_history(100)
            .with_config_path("/tmp/test.toml");

        assert_eq!(handler.provider.as_deref(), Some("anthropic"));
        assert_eq!(handler.model.as_deref(), Some("claude-3-5-sonnet"));
        assert_eq!(handler.system_prompt, "Custom prompt");
        assert_eq!(handler.max_history, 100);
        assert!(handler.config_path.is_some());
    }

    #[test]
    fn test_default_handler() {
        let handler = ClawdiusHandler::default();
        assert_eq!(handler.system_prompt, GATEWAY_SYSTEM_PROMPT);
        assert_eq!(handler.max_history, 50);
        assert!(handler.provider.is_none());
    }

    #[tokio::test]
    async fn test_save_to_history() {
        let handler = ClawdiusHandler::new();

        handler
            .save_to_history(
                &Platform::Telegram,
                "chat1",
                clawdius_core::llm::ChatRole::User,
                "hello",
            )
            .await;

        handler
            .save_to_history(
                &Platform::Telegram,
                "chat1",
                clawdius_core::llm::ChatRole::Assistant,
                "hi there!",
            )
            .await;

        let key = ClawdiusHandler::session_key(&Platform::Telegram, "chat1");
        let history = handler.message_history.read().await;
        assert_eq!(history.get(&key).unwrap().len(), 2);
        assert_eq!(history.get(&key).unwrap()[0].content, "hello");
        assert_eq!(history.get(&key).unwrap()[1].content, "hi there!");
    }

    #[tokio::test]
    async fn test_separate_chat_histories() {
        let handler = ClawdiusHandler::new();

        handler
            .save_to_history(
                &Platform::Discord,
                "ch1",
                clawdius_core::llm::ChatRole::User,
                "discord msg",
            )
            .await;

        handler
            .save_to_history(
                &Platform::Telegram,
                "ch2",
                clawdius_core::llm::ChatRole::User,
                "telegram msg",
            )
            .await;

        let history = handler.message_history.read().await;
        assert_eq!(history.len(), 2);
        assert_eq!(
            history
                .get(&ClawdiusHandler::session_key(&Platform::Discord, "ch1"))
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            history
                .get(&ClawdiusHandler::session_key(&Platform::Telegram, "ch2"))
                .unwrap()
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn test_build_messages_includes_system_prompt() {
        let handler = ClawdiusHandler::new();

        // Save a user message to history
        handler
            .save_to_history(
                &Platform::Slack,
                "ch1",
                clawdius_core::llm::ChatRole::User,
                "previous msg",
            )
            .await;

        let messages = handler
            .build_messages(&Platform::Slack, "ch1", "new msg")
            .await
            .unwrap();

        // Should have: system + previous user + new user
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].role, clawdius_core::llm::ChatRole::System);
        assert_eq!(messages[1].role, clawdius_core::llm::ChatRole::User);
        assert!(messages[1].content.contains("previous msg"));
        assert_eq!(messages[2].role, clawdius_core::llm::ChatRole::User);
        assert!(messages[2].content.contains("new msg"));
    }

    #[tokio::test]
    async fn test_max_history_truncation() {
        let handler = ClawdiusHandler::new().with_max_history(3);

        // Add 5 messages
        for i in 0..5 {
            handler
                .save_to_history(
                    &Platform::Telegram,
                    "ch1",
                    if i % 2 == 0 {
                        clawdius_core::llm::ChatRole::User
                    } else {
                        clawdius_core::llm::ChatRole::Assistant
                    },
                    &format!("msg{i}"),
                )
                .await;
        }

        let messages = handler
            .build_messages(&Platform::Telegram, "ch1", "latest")
            .await
            .unwrap();

        // Should have: system + last 3 history + new = 5 total
        assert_eq!(messages.len(), 5);
        // First history message should be msg2 (indices 2,3,4 of 0-4)
        assert!(messages[1].content.contains("msg2"));
    }
}
