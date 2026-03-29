//! Help Handler
//!
//! Provides help and command documentation for messaging users.

use async_trait::async_trait;

use super::gateway::{MessageHandler, MessageHandlerResult};
use crate::messaging::command_parser::CommandParser;
use crate::messaging::types::{CommandCategory, MessagingSession, ParsedCommand, Platform, Result};

pub struct HelpHandler;

impl HelpHandler {
    pub fn new() -> Self {
        Self
    }

    fn generate_help(&self, platform: Platform) -> String {
        let prefix = platform.command_prefix().trim();
        let parser = CommandParser::new(platform);

        format!(
            r#"🤖 **Clawdius Help**

Available commands (prefix: `{}`):

**Session Management**
• `{}start` - Start a new session
• `{}stop` - End current session
• `{}sessions` - List your sessions

**Status & Info**
• `{}status` - Check Clawdius status
• `{}ping` - Ping the bot
• `{}help` - Show this help

**Code Generation**
• `{}generate code <description>` - Generate code
• `{}generate test <description>` - Generate tests

**Analysis**
• `{}analyze <file or question>` - Analyze code or ask questions

**Timeline**
• `{}timeline list` - List checkpoints
• `{}checkpoint create <name>` - Create checkpoint
• `{}rollback <id>` - Rollback to checkpoint

**Configuration**
• `{}config show` - Show current config
• `{}set provider <name>` - Set LLM provider

---
💡 Max message length: {} chars
📝 Markdown: {}
🧵 Threads: {}"#,
            prefix,
            prefix, prefix, prefix, prefix, prefix,
            prefix, prefix, prefix, prefix, prefix, prefix,
            platform.max_message_length(),
            if platform.supports_markdown() { "enabled" } else { "disabled" },
            if platform.supports_threads() { "supported" } else { "not supported" }
        )
    }
}

#[async_trait]
impl MessageHandler for HelpHandler {
    async fn handle(
        &self,
        session: &MessagingSession,
        command: &ParsedCommand,
    ) -> Result<MessageHandlerResult> {
        let response = self.generate_help(session.platform_user.platform);
        Ok(MessageHandlerResult {
            response,
            should_chunk: false,
        })
    }
}

impl Default for HelpHandler {
    fn default() -> Self {
        Self::new()
    }
}

pub struct StatusHandler;

impl StatusHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl MessageHandler for StatusHandler {
    async fn handle(
        &self,
        _session: &MessagingSession,
        _command: &ParsedCommand,
    ) -> Result<MessageHandlerResult> {
        let response = format!(
            r#"✅ **Clawdius Status**

🟢 **Status:** Online
📊 **Sessions Active:** Running
⏰ **Uptime:** Healthy
🔧 **Version:** {}

Use `help` for available commands."#,
            env!("CARGO_PKG_VERSION")
        );

        Ok(MessageHandlerResult {
            response,
            should_chunk: false,
        })
    }
}

impl Default for StatusHandler {
    fn default() -> Self {
        Self::new()
    }
}
