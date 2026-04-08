//! Analyze Command Handler
//!
//! Handles code analysis commands.

use crate::messaging::gateway::{MessageHandler, MessageHandlerResult};
use crate::messaging::types::{MessagingSession, ParsedCommand, Result};
use async_trait::async_trait;

/// Handler for code analysis commands
pub struct AnalyzeHandler;

impl AnalyzeHandler {
    /// Creates a new analyze handler
    pub fn new() -> Self {
        Self
    }
}

impl Default for AnalyzeHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MessageHandler for AnalyzeHandler {
    async fn handle(
        &self,
        session: &MessagingSession,
        command: &ParsedCommand,
    ) -> Result<MessageHandlerResult> {
        // Check permissions
        if !session.permissions.can_analyze {
            return Ok(MessageHandlerResult {
                response: "❌ **Permission Denied**\n\nYou do not have permission to analyze code."
                    .to_string(),
                should_chunk: false,
                stream: None,
            });
        }

        // Get the query from args
        if command.args.is_empty() {
            return Ok(MessageHandlerResult {
                response: "❌ **Missing Query**\n\n\
                    Please provide a query for code analysis.\n\n\
                    **Usage:**\n\
                    • `/clawd analyze <query>`\n\
                    • `/clawd explain <code>`\n\n\
                    **Examples:**\n\
                    • `/clawd analyze why is this function slow?`\n\
                    • `/clawd explain this regex pattern`"
                    .to_string(),
                should_chunk: false,
                stream: None,
            });
        }

        let query = command.args.join(" ");

        // FIXME(v1.7.0): Integrate with actual code analysis
        tracing::warn!(
            query = %query,
            "Analyze handler is using a stub response; LLM integration not yet wired"
        );
        let response = "[STUB] Code analysis is not yet integrated via the messaging gateway. \
                         Use 'clawdius analyze' CLI command instead."
            .to_string();

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
    async fn test_analyze_with_query() {
        let handler = AnalyzeHandler::new();
        let session = create_test_session();
        let command = ParsedCommand::new(
            "/clawd analyze why slow",
            CommandCategory::Analyze,
            "analyze",
        )
        .with_args(vec!["why".to_string(), "slow".to_string()]);

        let result = handler.handle(&session, &command).await.unwrap();
        assert!(result.response.contains("[STUB]"));
        assert!(result.response.contains("not yet integrated"));
    }

    #[tokio::test]
    async fn test_analyze_without_query() {
        let handler = AnalyzeHandler::new();
        let session = create_test_session();
        let command = ParsedCommand::new("/clawd analyze", CommandCategory::Analyze, "analyze");

        let result = handler.handle(&session, &command).await.unwrap();
        assert!(result.response.contains("Missing Query"));
    }
}
