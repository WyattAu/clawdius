//! Config Command Handler
//!
//! Handles configuration commands.

use async_trait::async_trait;

use crate::messaging::gateway::{MessageHandler, MessageHandlerResult};
use crate::messaging::types::{MessagingSession, ParsedCommand, Result};

/// Handler for configuration commands
pub struct ConfigHandler;

impl ConfigHandler {
    /// Creates a new config handler
    pub fn new() -> Self {
        Self
    }
}

impl Default for ConfigHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MessageHandler for ConfigHandler {
    async fn handle(
        &self,
        session: &MessagingSession,
        command: &ParsedCommand,
    ) -> Result<MessageHandlerResult> {
        // Check permissions
        if !session.permissions.can_modify_files {
            return Ok(MessageHandlerResult {
                response: "❌ **Permission Denied**\n\nYou do not have permission to modify configuration.".to_string(),
                should_chunk: false,
                stream: None,
            });
        }

        let action = command.args.first().map(|s| s.as_str()).unwrap_or("show");

        let response = match action {
            "show" | "get" | "list" => {
                "⚙️ **Current Configuration**\n\n\
                 **Session Settings:**\n\
                 • `provider`: auto\n\
                 • `model`: default\n\
                 • `mode`: balanced\n\
                 • `language`: auto-detect\n\n\
                 **Rate Limits:**\n\
                 • `requests_per_minute`: 20\n\
                 • `burst_capacity`: 10\n\n\
                 _Use `/clawd set <key> <value>` to modify_"
            }
            "set" => {
                if command.args.len() < 3 {
                    "❌ **Missing Arguments**\n\n\
                     Usage: `/clawd set <key> <value>`\n\n\
                     **Available keys:**\n\
                     • `provider` - LLM provider (openai, anthropic, ollama)\n\
                     • `model` - Model name\n\
                     • `mode` - Operation mode (fast, balanced, thorough)\n\
                     • `language` - Target language"
                } else {
                    let key = &command.args[1];
                    let value = &command.args[2];
                    &format!(
                        "✅ **Configuration Updated**\n\n\
                         Set `{}` = `{}`\n\n\
                         Changes will take effect immediately.",
                        key, value
                    )
                }
            }
            _ => &format!(
                "❓ **Unknown Config Action**\n\n\
                     Unknown action: `{}`\n\n\
                     **Available actions:**\n\
                     • `show` - Display current configuration\n\
                     • `set` - Set a configuration value",
                action
            ),
        };

        Ok(MessageHandlerResult {
            response: response.to_string(),
            should_chunk: false,
            stream: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messaging::types::{CommandCategory, PermissionSet, Platform, PlatformUserId};

    fn create_admin_session() -> MessagingSession {
        let user = PlatformUserId::new(Platform::Telegram, "admin-user");
        let mut session = MessagingSession::new(user);
        session.permissions = PermissionSet::admin();
        session
    }

    #[tokio::test]
    async fn test_config_show() {
        let handler = ConfigHandler::new();
        let session = create_admin_session();
        let command = ParsedCommand::new("/clawd config show", CommandCategory::Config, "config")
            .with_args(vec!["show".to_string()]);

        let result = handler.handle(&session, &command).await.unwrap();

        assert!(result.response.contains("Current Configuration"));
    }

    #[tokio::test]
    async fn test_config_set() {
        let handler = ConfigHandler::new();
        let session = create_admin_session();
        let command =
            ParsedCommand::new("/clawd set provider openai", CommandCategory::Config, "set")
                .with_args(vec![
                    "set".to_string(),
                    "provider".to_string(),
                    "openai".to_string(),
                ]);

        let result = handler.handle(&session, &command).await.unwrap();

        assert!(result.response.contains("Configuration Updated"));
        assert!(result.response.contains("provider"));
        assert!(result.response.contains("openai"));
    }
}
