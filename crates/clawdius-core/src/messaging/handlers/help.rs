//! Help Command Handler
//!
//! Handles help and command documentation.

use async_trait::async_trait;

use crate::messaging::gateway::{MessageHandler, MessageHandlerResult};
use crate::messaging::types::{MessagingSession, ParsedCommand, Result};

/// Handler for help commands
pub struct HelpHandler;

impl HelpHandler {
    /// Creates a new help handler
    pub fn new() -> Self {
        Self
    }

    fn get_help_text() -> String {
        "
🤖 **Clawdius Remote Control**
==============================

**Available Commands:**

📋 **Session Management:**
• `/clawd session start` - Start a new session
• `/clawd session stop` - Stop current session
• `/clawd sessions` - List active sessions

📊 **Status & Info:**
• `/clawd status` - Check Clawdius status
• `/clawd ping` - Ping the bot

🔧 **Code Generation:**
• `/clawd generate <prompt>` - Generate code
• `/clawd gen function <name>` - Generate a function
• `/clawd gen test <target>` - Generate tests

🔍 **Analysis:**
• `/clawd analyze <query>` - Analyze code
• `/clawd explain <code>` - Explain code

📅 **Timeline:**
• `/clawd timeline list` - List checkpoints
• `/clawd checkpoint create` - Create checkpoint
• `/clawd rollback` - Rollback to checkpoint

⚙️ **Configuration:**
• `/clawd config show` - Show configuration
• `/clawd set <key> <value>` - Set config value

🔐 **Admin:**
• `/clawd admin` - Admin commands (restricted)

**Platform-specific Prefixes:**
| Platform   | Prefix    |
|------------|-----------|
| Telegram   | `/clawd ` |
| Discord    | `/clawd ` |
| Matrix     | `!clawd ` |
| Signal     | `/clawd ` |
| RocketChat | `/clawd ` |
| WhatsApp   | `/clawd ` |
| Slack      | `/clawd ` |

**Examples:**
```
/clawd status
/clawd generate function add_auth_check --lang rust
/clawd analyze why is this slow?
!clawd help
```
 "
        .trim()
        .to_string()
    }
}

impl Default for HelpHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MessageHandler for HelpHandler {
    async fn handle(
        &self,
        _session: &MessagingSession,
        _command: &ParsedCommand,
    ) -> Result<MessageHandlerResult> {
        Ok(MessageHandlerResult {
            response: Self::get_help_text(),
            should_chunk: true, // Help text may need chunking
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
    async fn test_help_handler() {
        let handler = HelpHandler::new();
        let session = create_test_session();
        let command = ParsedCommand::new("/clawd help", CommandCategory::Help, "help");

        let result = handler.handle(&session, &command).await.unwrap();

        assert!(result.response.contains("Available Commands"));
        assert!(result.should_chunk);
    }
}
