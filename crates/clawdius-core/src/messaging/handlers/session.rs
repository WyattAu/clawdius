//! Session Command Handler
//!
//! Handles session management commands.

use async_trait::async_trait;

use crate::messaging::gateway::{MessageHandler, MessageHandlerResult};
use crate::messaging::types::{MessagingSession, ParsedCommand, Result};

/// Handler for session commands
pub struct SessionHandler;

impl SessionHandler {
    /// Creates a new session handler
    pub fn new() -> Self {
        Self
    }
}

impl Default for SessionHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MessageHandler for SessionHandler {
    async fn handle(
        &self,
        session: &MessagingSession,
        command: &ParsedCommand,
    ) -> Result<MessageHandlerResult> {
        let response = match command.action.as_str() {
            "start" | "new" => "✅ **Session Started**\n\n\
                 A new Clawdius session has been initialized.\n\
                 You can now use generation and analysis commands."
                .to_string(),
            "stop" | "end" | "close" => "✅ **Session Ended**\n\n\
                 Your Clawdius session has been closed.\n\
                 Start a new session with `/clawd session start`."
                .to_string(),
            "status" => {
                format!(
                    "📋 **Session Status**\n\n\
                     **State**: {:?}\n\
                     **Messages**: {}\n\
                     **Session ID**: `{}`",
                    session.state, session.message_count, session.id
                )
            },
            "list" | "sessions" => "📋 **Active Sessions**\n\n\
                 Session listing is not yet implemented.\n\
                 Use `/clawd status` to check your current session."
                .to_string(),
            _ => {
                format!(
                    "❓ **Unknown Session Command**\n\n\
                     Unknown action: `{}`\n\n\
                     **Available actions:**\n\
                     • `start` - Start a new session\n\
                     • `stop` - End current session\n\
                     • `status` - Check session status",
                    command.action
                )
            },
        };

        Ok(MessageHandlerResult {
            response,
            should_chunk: false,
            stream: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messaging::types::{CommandCategory, Platform, PlatformUserId};

    fn create_test_session() -> MessagingSession {
        let user = PlatformUserId::new(Platform::Telegram, "test-user");
        MessagingSession::new(user)
    }

    #[tokio::test]
    async fn test_session_start() {
        let handler = SessionHandler::new();
        let session = create_test_session();
        let command = ParsedCommand::new("/clawd session start", CommandCategory::Session, "start");

        let result = handler.handle(&session, &command).await.unwrap();

        assert!(result.response.contains("Session Started"));
    }

    #[tokio::test]
    async fn test_session_stop() {
        let handler = SessionHandler::new();
        let session = create_test_session();
        let command = ParsedCommand::new("/clawd session stop", CommandCategory::Session, "stop");

        let result = handler.handle(&session, &command).await.unwrap();

        assert!(result.response.contains("Session Ended"));
    }
}
