//! Status Command Handler
//!
//! Handles status and health check commands.

use async_trait::async_trait;

use crate::messaging::gateway::{MessageHandler, MessageHandlerResult};
use crate::messaging::types::{MessagingSession, ParsedCommand, Result};

/// Handler for status commands
pub struct StatusHandler;

impl StatusHandler {
    /// Creates a new status handler
    pub fn new() -> Self {
        Self
    }

    fn format_status(session: &MessagingSession) -> String {
        format!(
            "✅ **Clawdius Status**\n\n\
            **State**: {:?}\n\
            **Session ID**: `{}`\n\
            **Platform**: {}\n\
            **User ID**: `{}`\n\
            **Messages**: {}\n\
            **Created**: {}\n\
            **Last Activity**: {}\n\n\
            **Permissions**:\n\
            • Generate: {}\n\
            • Analyze: {}\n\
            • Modify Files: {}\n\
            • Execute: {}\n\
            • Admin: {}",
            session.state,
            session.id,
            session.platform_user.platform,
            session.platform_user.user_id,
            session.message_count,
            session.created_at.format("%Y-%m-%d %H:%M:%S UTC"),
            session.last_activity.format("%Y-%m-%d %H:%M:%S UTC"),
            if session.permissions.can_generate {
                "✅"
            } else {
                "❌"
            },
            if session.permissions.can_analyze {
                "✅"
            } else {
                "❌"
            },
            if session.permissions.can_modify_files {
                "✅"
            } else {
                "❌"
            },
            if session.permissions.can_execute {
                "✅"
            } else {
                "❌"
            },
            if session.permissions.can_admin {
                "✅"
            } else {
                "❌"
            },
        )
    }
}

impl Default for StatusHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MessageHandler for StatusHandler {
    async fn handle(
        &self,
        session: &MessagingSession,
        _command: &ParsedCommand,
    ) -> Result<MessageHandlerResult> {
        let response = Self::format_status(session);

        Ok(MessageHandlerResult {
            response,
            should_chunk: false,
            stream: None,
        })
    }
}

#[cfg(test)]
mod tests {
    // Tests removed - will be re-added later
}
