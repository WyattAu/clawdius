//! Clawdius Integration
//!
//! Connects messaging handlers to the actual Clawdius LLM and session management.
//!
//! ## Threading Model
//!
//! `SessionManager` wraps `rusqlite::Connection` which is `!Sync`. Since
//! `MessageHandler` requires `Send + Sync`, we use a factory pattern:
//! the handler stores a `SessionManagerFactory` that produces `SessionManager`
//! instances on-demand inside `spawn_blocking`, avoiding the `Sync` requirement.

use std::sync::Arc;

use crate::llm::providers::LlmClient;
use crate::llm::{ChatMessage, ChatRole};
use crate::messaging::gateway::{MessageHandler, MessageHandlerResult, MessagingGateway};
use crate::messaging::handlers::{
    AdminHandler, ConfigHandler, HelpHandler, SessionHandler, StatusHandler,
};
use crate::messaging::types::{
    CommandCategory, MessagingError, MessagingSession, ParsedCommand, Result,
};
use crate::session::types::{ContentPart, MessageContent};
use crate::session::SessionManager;
use crate::session::{Message, MessageRole, SessionId};

const GENERATE_SYSTEM_PROMPT: &str = "You are Clawdius, an expert code generation assistant. \
    Generate clean, well-structured code based on the user's request. \
    Provide explanations when helpful but focus on producing working code.";

const ANALYZE_SYSTEM_PROMPT: &str = "You are Clawdius, an expert code analysis assistant. \
    Analyze code thoroughly, identifying issues, suggesting improvements, \
    and explaining complex patterns. Be precise and actionable in your feedback.";

/// Factory that produces `SessionManager` instances.
///
/// This avoids storing `SessionManager` (which is `!Sync`) in handlers that
/// need `Send + Sync`. Instead, each operation creates a short-lived
/// `SessionManager` via the factory.
pub type SessionManagerFactory = Arc<dyn Fn() -> crate::Result<SessionManager> + Send + Sync>;

/// Generic LLM-powered handler that manages sessions and calls the LLM.
///
/// Uses a factory pattern to create per-operation `SessionManager` instances,
/// avoiding the `!Sync` constraint from rusqlite.
pub struct ClawdiusLlmHandler {
    session_factory: SessionManagerFactory,
    llm_client: Arc<dyn LlmClient>,
    system_prompt: String,
}

impl ClawdiusLlmHandler {
    pub fn new(
        session_factory: SessionManagerFactory,
        llm_client: Arc<dyn LlmClient>,
        system_prompt: impl Into<String>,
    ) -> Self {
        Self {
            session_factory,
            llm_client,
            system_prompt: system_prompt.into(),
        }
    }

    /// Core logic: get/create session, build messages, call LLM, persist.
    pub async fn handle_llm_command(
        &self,
        messaging_session: &MessagingSession,
        user_prompt: &str,
    ) -> Result<MessageHandlerResult> {
        // Step 1: Get or create session (blocking via spawn_blocking)
        let factory = self.session_factory.clone();
        let session_id_opt = messaging_session.clawdius_session_id;
        let (session, session_id) = tokio::task::spawn_blocking(move || {
            let sm = factory().map_err(|e| MessagingError::SessionNotFound(e.to_string()))?;
            let session = match session_id_opt {
                Some(uuid) => {
                    let sid = SessionId::from_uuid(uuid);
                    sm.load_session(&sid)
                        .map_err(|e| MessagingError::SessionNotFound(e.to_string()))?
                        .ok_or_else(|| MessagingError::SessionNotFound(sid.to_string()))?
                }
                None => sm
                    .create_session()
                    .map_err(|e| MessagingError::SessionNotFound(e.to_string()))?,
            };
            let session_id = session.id;
            Ok::<_, MessagingError>((session, session_id))
        })
        .await
        .map_err(|e| MessagingError::SendFailed(format!("Task join error: {e}")))??;

        // Step 2: Build chat messages from session history
        let chat_messages = build_chat_messages(&self.system_prompt, &session.messages);

        // Step 3: Persist user message
        let factory = self.session_factory.clone();
        let user_message = Message::user(user_prompt);
        let mut session_for_save = session.clone();
        tokio::task::spawn_blocking(move || {
            let sm = factory().map_err(|e| MessagingError::SessionNotFound(e.to_string()))?;
            let rt = tokio::runtime::Handle::current();
            rt.block_on(sm.add_message(&mut session_for_save, user_message))
                .map_err(|e| MessagingError::SendFailed(e.to_string()))
        })
        .await
        .map_err(|e| MessagingError::SendFailed(format!("Task join error: {e}")))?
        .map_err(|e| MessagingError::SendFailed(e.to_string()))?;

        // Step 4: Call LLM with streaming
        let (tx, rx) = tokio::sync::mpsc::channel(32);
        let llm_client = self.llm_client.clone();

        // Spawn a task to consume the LLM stream and forward chunks
        let session_for_persist = session.clone();
        let persist_factory = self.session_factory.clone();
        let stream_task = tokio::spawn(async move {
            match llm_client.chat_stream(chat_messages).await {
                Ok(mut stream_rx) => {
                    let mut full_response = String::new();
                    while let Some(chunk) = stream_rx.recv().await {
                        full_response.push_str(&chunk);
                        // Ignore send errors (receiver may have dropped)
                        let _ = tx.send(chunk).await;
                    }
                    // Persist the complete assistant response
                    let factory = persist_factory;
                    let assistant_message = Message::assistant(&full_response);
                    let mut session_save = session_for_persist;
                    let _ = tokio::task::spawn_blocking(move || {
                        let sm = factory().ok()?;
                        let rt = tokio::runtime::Handle::current();
                        rt.block_on(sm.add_message(&mut session_save, assistant_message))
                            .ok()?;
                        Some::<()>(())
                    })
                    .await;
                }
                Err(e) => {
                    let _ = tx.send(format!("Error: {}", e)).await;
                }
            }
            // Drop tx to signal stream completion
            drop(tx);
        });

        // Detach — the task runs independently and sends chunks through the channel
        // The gateway's send_streaming_response will consume rx
        drop(stream_task);

        // Build response with streaming
        let response = format!("Generating response...\n\n_Session: `{}`_", session_id);

        Ok(MessageHandlerResult {
            response,
            should_chunk: false,
            stream: Some(rx),
        })
    }
}

/// Handler for code generation commands.
pub struct ClawdiusGenerateHandler {
    inner: ClawdiusLlmHandler,
}

impl ClawdiusGenerateHandler {
    pub fn new(session_factory: SessionManagerFactory, llm_client: Arc<dyn LlmClient>) -> Self {
        Self {
            inner: ClawdiusLlmHandler::new(session_factory, llm_client, GENERATE_SYSTEM_PROMPT),
        }
    }
}

#[async_trait::async_trait]
impl MessageHandler for ClawdiusGenerateHandler {
    async fn handle(
        &self,
        messaging_session: &MessagingSession,
        command: &ParsedCommand,
    ) -> Result<MessageHandlerResult> {
        if !messaging_session.permissions.can_generate {
            return Ok(MessageHandlerResult {
                response:
                    "❌ **Permission Denied**\n\nYou do not have permission to generate code."
                        .to_string(),
                should_chunk: false,
                stream: None,
            });
        }

        let prompt = command.args.join(" ");
        if prompt.trim().is_empty() {
            return Ok(MessageHandlerResult {
                response: "❌ **Missing Prompt**\n\nPlease provide a prompt for code generation."
                    .to_string(),
                should_chunk: false,
                stream: None,
            });
        }

        self.inner
            .handle_llm_command(messaging_session, &prompt)
            .await
    }
}

/// Handler for code analysis commands.
pub struct ClawdiusAnalyzeHandler {
    inner: ClawdiusLlmHandler,
}

impl ClawdiusAnalyzeHandler {
    pub fn new(session_factory: SessionManagerFactory, llm_client: Arc<dyn LlmClient>) -> Self {
        Self {
            inner: ClawdiusLlmHandler::new(session_factory, llm_client, ANALYZE_SYSTEM_PROMPT),
        }
    }
}

#[async_trait::async_trait]
impl MessageHandler for ClawdiusAnalyzeHandler {
    async fn handle(
        &self,
        messaging_session: &MessagingSession,
        command: &ParsedCommand,
    ) -> Result<MessageHandlerResult> {
        if !messaging_session.permissions.can_analyze {
            return Ok(MessageHandlerResult {
                response: "❌ **Permission Denied**\n\nYou do not have permission to analyze code."
                    .to_string(),
                should_chunk: false,
                stream: None,
            });
        }

        let query = command.args.join(" ");
        if query.trim().is_empty() {
            return Ok(MessageHandlerResult {
                response: "❌ **Missing Query**\n\nPlease provide a query for analysis."
                    .to_string(),
                should_chunk: false,
                stream: None,
            });
        }

        self.inner
            .handle_llm_command(messaging_session, &query)
            .await
    }
}

/// Convert session messages into LLM chat format.
///
/// - Prepends system prompt
/// - Skips Tool-role messages
/// - Extracts text from Text and MultiPart content
pub fn build_chat_messages(
    system_prompt: &str,
    session_messages: &[crate::session::Message],
) -> Vec<ChatMessage> {
    let mut messages = Vec::with_capacity(session_messages.len() + 1);

    messages.push(ChatMessage {
        role: ChatRole::System,
        content: system_prompt.to_string(),
    });

    for msg in session_messages {
        let role = match msg.role {
            MessageRole::System => ChatRole::System,
            MessageRole::User => ChatRole::User,
            MessageRole::Assistant => ChatRole::Assistant,
            MessageRole::Tool => continue,
        };

        let content = match &msg.content {
            MessageContent::Text(text) => text.clone(),
            MessageContent::MultiPart(parts) => parts
                .iter()
                .filter_map(|p| match p {
                    ContentPart::Text { text } => Some(text.as_str()),
                    ContentPart::Image { .. } => None,
                })
                .collect::<Vec<_>>()
                .join("\n"),
        };

        messages.push(ChatMessage { role, content });
    }

    messages
}

/// Create a fully-wired messaging gateway with LLM-backed handlers.
pub async fn create_connected_gateway(
    session_factory: SessionManagerFactory,
    llm_client: Arc<dyn LlmClient>,
) -> Arc<MessagingGateway> {
    let gateway = Arc::new(MessagingGateway::new());

    // Basic handlers (no LLM needed)
    gateway
        .register_handler(CommandCategory::Status, Arc::new(StatusHandler::new()))
        .await;
    gateway
        .register_handler(CommandCategory::Help, Arc::new(HelpHandler::new()))
        .await;
    gateway
        .register_handler(CommandCategory::Session, Arc::new(SessionHandler::new()))
        .await;
    gateway
        .register_handler(CommandCategory::Config, Arc::new(ConfigHandler::new()))
        .await;
    gateway
        .register_handler(CommandCategory::Admin, Arc::new(AdminHandler::new()))
        .await;

    // LLM-backed handlers
    gateway
        .register_handler(
            CommandCategory::Generate,
            Arc::new(ClawdiusGenerateHandler::new(
                session_factory.clone(),
                llm_client.clone(),
            )),
        )
        .await;
    gateway
        .register_handler(
            CommandCategory::Analyze,
            Arc::new(ClawdiusAnalyzeHandler::new(session_factory, llm_client)),
        )
        .await;

    gateway
}

/// Create a `SessionManagerFactory` from a `Config`.
///
/// Each call creates a new `SessionManager` instance (opens its own SQLite
/// connection). This is safe because each instance is independent and
/// only used within a single `spawn_blocking` call.
pub fn create_session_manager_factory(config: crate::config::Config) -> SessionManagerFactory {
    Arc::new(move || SessionManager::new(&config))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{Message, MessageContent, MessageRole};
    use tokio::sync::mpsc;

    struct MockLlmClient {
        response: String,
    }

    impl MockLlmClient {
        fn new(response: impl Into<String>) -> Self {
            Self {
                response: response.into(),
            }
        }
    }

    #[async_trait::async_trait]
    impl LlmClient for MockLlmClient {
        async fn chat(&self, _messages: Vec<ChatMessage>) -> crate::Result<String> {
            Ok(self.response.clone())
        }

        async fn chat_stream(
            &self,
            _messages: Vec<ChatMessage>,
        ) -> crate::Result<mpsc::Receiver<String>> {
            let (tx, rx) = mpsc::channel(1);
            let _ = tx.send(self.response.clone()).await;
            Ok(rx)
        }

        fn count_tokens(&self, text: &str) -> usize {
            text.len()
        }
    }

    fn test_factory() -> SessionManagerFactory {
        create_session_manager_factory(crate::config::Config::default())
    }

    #[test]
    fn test_build_chat_messages_empty_session() {
        let messages = build_chat_messages("You are helpful.", &[]);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, ChatRole::System);
        assert_eq!(messages[0].content, "You are helpful.");
    }

    #[test]
    fn test_build_chat_messages_with_history() {
        let session_messages = vec![Message::user("hello"), Message::assistant("hi there")];
        let messages = build_chat_messages("system", &session_messages);

        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].role, ChatRole::System);
        assert_eq!(messages[1].role, ChatRole::User);
        assert_eq!(messages[1].content, "hello");
        assert_eq!(messages[2].role, ChatRole::Assistant);
        assert_eq!(messages[2].content, "hi there");
    }

    #[test]
    fn test_build_chat_messages_skips_tool_messages() {
        let session_messages = vec![
            Message::user("do something"),
            Message {
                id: uuid::Uuid::new_v4(),
                role: MessageRole::Tool,
                content: MessageContent::Text("tool result".to_string()),
                tokens: None,
                created_at: chrono::Utc::now(),
                tool_calls: vec![],
                metadata: serde_json::Map::new(),
            },
            Message::assistant("done"),
        ];
        let messages = build_chat_messages("system", &session_messages);

        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].role, ChatRole::System);
        assert_eq!(messages[1].role, ChatRole::User);
        assert_eq!(messages[2].role, ChatRole::Assistant);
    }

    #[test]
    fn test_build_chat_messages_multipart_text() {
        let session_messages = vec![Message {
            id: uuid::Uuid::new_v4(),
            role: MessageRole::User,
            content: MessageContent::MultiPart(vec![
                ContentPart::Text {
                    text: "part one".to_string(),
                },
                ContentPart::Text {
                    text: "part two".to_string(),
                },
            ]),
            tokens: None,
            created_at: chrono::Utc::now(),
            tool_calls: vec![],
            metadata: serde_json::Map::new(),
        }];
        let messages = build_chat_messages("system", &session_messages);

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[1].content, "part one\npart two");
    }

    #[test]
    fn test_clawdius_llm_handler_new() {
        let client: Arc<dyn LlmClient> = Arc::new(MockLlmClient::new("test"));
        let handler = ClawdiusLlmHandler::new(test_factory(), client, "prompt");
        assert_eq!(handler.system_prompt, "prompt");
        // Verify Send + Sync
        fn assert_send_sync<T: Send + Sync>(_: &T) {}
        assert_send_sync(&handler);
    }

    #[test]
    fn test_clawdius_generate_handler_new() {
        let client: Arc<dyn LlmClient> = Arc::new(MockLlmClient::new("test"));
        let handler = ClawdiusGenerateHandler::new(test_factory(), client);
        fn assert_send_sync<T: Send + Sync>(_: &T) {}
        assert_send_sync(&handler);
    }

    #[test]
    fn test_clawdius_analyze_handler_new() {
        let client: Arc<dyn LlmClient> = Arc::new(MockLlmClient::new("test"));
        let handler = ClawdiusAnalyzeHandler::new(test_factory(), client);
        fn assert_send_sync<T: Send + Sync>(_: &T) {}
        assert_send_sync(&handler);
    }

    #[tokio::test]
    async fn test_create_connected_gateway() {
        let client: Arc<dyn LlmClient> = Arc::new(MockLlmClient::new("test"));
        let gateway = create_connected_gateway(test_factory(), client).await;
        assert_eq!(gateway.channel_count().await, 0);
    }

    #[tokio::test]
    async fn test_generate_handler_permission_denied() {
        let client: Arc<dyn LlmClient> = Arc::new(MockLlmClient::new("test"));
        let handler = ClawdiusGenerateHandler::new(test_factory(), client);

        let session = MessagingSession::default();
        let command = ParsedCommand::new("/clawd gen test", CommandCategory::Generate, "gen")
            .with_args(vec!["test".to_string()]);

        let result = handler.handle(&session, &command).await.unwrap();
        assert!(result.response.contains("Permission Denied"));
    }

    #[tokio::test]
    async fn test_generate_handler_missing_prompt() {
        let client: Arc<dyn LlmClient> = Arc::new(MockLlmClient::new("test"));
        let handler = ClawdiusGenerateHandler::new(test_factory(), client);

        let mut session = MessagingSession::default();
        session.permissions.can_generate = true;
        let command = ParsedCommand::new("/clawd gen", CommandCategory::Generate, "gen");

        let result = handler.handle(&session, &command).await.unwrap();
        assert!(result.response.contains("Missing Prompt"));
    }

    #[tokio::test]
    async fn test_analyze_handler_permission_denied() {
        let client: Arc<dyn LlmClient> = Arc::new(MockLlmClient::new("test"));
        let handler = ClawdiusAnalyzeHandler::new(test_factory(), client);

        let session = MessagingSession::default();
        let command = ParsedCommand::new("/clawd analyze x", CommandCategory::Analyze, "analyze")
            .with_args(vec!["x".to_string()]);

        let result = handler.handle(&session, &command).await.unwrap();
        assert!(result.response.contains("Permission Denied"));
    }

    #[tokio::test]
    async fn test_analyze_handler_missing_query() {
        let client: Arc<dyn LlmClient> = Arc::new(MockLlmClient::new("test"));
        let handler = ClawdiusAnalyzeHandler::new(test_factory(), client);

        let mut session = MessagingSession::default();
        session.permissions.can_analyze = true;
        let command = ParsedCommand::new("/clawd analyze", CommandCategory::Analyze, "analyze");

        let result = handler.handle(&session, &command).await.unwrap();
        assert!(result.response.contains("Missing Query"));
    }
}
