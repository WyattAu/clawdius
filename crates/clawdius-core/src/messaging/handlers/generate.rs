//! Generate Command Handler
//!
//! Handles code generation commands.

use async_trait::async_trait;

use crate::messaging::gateway::{MessageHandler, MessageHandlerResult};
use crate::messaging::types::{MessagingSession, ParsedCommand, Result};

/// Handler for generate commands
pub struct GenerateHandler;

impl GenerateHandler {
    /// Creates a new generate handler
    pub fn new() -> Self {
        Self
    }
}

impl Default for GenerateHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MessageHandler for GenerateHandler {
    async fn handle(
        &self,
        session: &MessagingSession,
        command: &ParsedCommand,
    ) -> Result<MessageHandlerResult> {
        // Check permissions
        if !session.permissions.can_generate {
            return Ok(MessageHandlerResult {
                response:
                    "❌ **Permission Denied**\n\nYou do not have permission to generate code."
                        .to_string(),
                should_chunk: false,
                stream: None,
            });
        }

        // Check for prompt
        if command.args.is_empty() {
            return Ok(MessageHandlerResult {
                response: r#"❌ **Missing Prompt**

**Usage:**
• `/clawd generate <prompt>` - Generate code
• `/clawd gen function <name>` - Generate a function
• `/clawd gen test <target>` - Generate tests

**Examples:**
• `/clawd generate a function that validates email addresses`
• `/clawd gen function add_auth_check --lang rust`"#
                    .to_string(),
                should_chunk: false,
                stream: None,
            });
        }

        // Get the prompt from args
        let prompt = command.args.join(" ");

        // Get language flag if provided
        let lang = command.flag("lang").unwrap_or("auto");

        // TODO: Integrate with actual code generation
        let response = format!(
            "🔧 **Code Generation**\n\n\
             **Prompt:** {}\n\
             **Language:** {}\n\n\
             ⏳ Processing with Clawdius LLM...\n\n\
             _Full LLM integration coming soon!_",
            prompt, lang
        );

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
    async fn test_generate_with_prompt() {
        let handler = GenerateHandler::new();
        let session = create_test_session();
        let command = ParsedCommand::new(
            "/clawd generate test function",
            CommandCategory::Generate,
            "generate",
        )
        .with_args(vec!["test".to_string(), "function".to_string()]);

        let result = handler.handle(&session, &command).await.unwrap();

        assert!(result.response.contains("Code Generation"));
        assert!(result.response.contains("test function"));
    }

    #[tokio::test]
    async fn test_generate_without_prompt() {
        let handler = GenerateHandler::new();
        let session = create_test_session();
        let command = ParsedCommand::new("/clawd generate", CommandCategory::Generate, "generate");

        let result = handler.handle(&session, &command).await.unwrap();

        assert!(result.response.contains("Missing Prompt"));
    }
}
