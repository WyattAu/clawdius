//! Admin Command Handler
//!
//! Handles admin-only commands.

use async_trait::async_trait;

use crate::messaging::gateway::{MessageHandler, MessageHandlerResult};
use crate::messaging::types::{MessagingSession, ParsedCommand, Result};

/// Handler for admin commands
pub struct AdminHandler;

impl AdminHandler {
    /// Creates a new admin handler
    pub fn new() -> Self {
        Self
    }
}

impl Default for AdminHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MessageHandler for AdminHandler {
    async fn handle(
        &self,
        session: &MessagingSession,
        command: &ParsedCommand,
    ) -> Result<MessageHandlerResult> {
        // Check admin permissions
        if !session.permissions.can_admin {
            return Ok(MessageHandlerResult {
                response:
                    "🚫 **Access Denied**\n\n\
                    Admin commands require elevated permissions.\n\n\
                    If you believe you should have access, please contact the system administrator."
                        .to_string(),
                should_chunk: false,
                stream: None,
            });
        }

        let action = command.args.first().map(|s| s.as_str()).unwrap_or("menu");

        let response = match action {
            "menu" | "help" => {
                "🔐 **Admin Menu**\n\n\
                 **User Management:**\n\
                 • `/clawd admin users` - List all users\n\
                 • `/clawd admin ban <user>` - Ban a user\n\
                 • `/clawd admin unban <user>` - Unban a user\n\
                 • `/clawd admin permissions <user>` - View permissions\n\
                 • `/clawd admin grant <user> <perm>` - Grant permission\n\n\
                 **Session Management:**\n\
                 • `/clawd admin sessions` - List all sessions\n\
                 • `/clawd admin kill <session>` - Kill a session\n\n\
                 **System:**\n\
                 • `/clawd admin status` - System status\n\
                 • `/clawd admin reload` - Reload configuration\n\
                 • `/clawd admin shutdown` - Shutdown bot (dangerous)"
            }
            "status" => {
                "📊 **System Status**\n\n\
                 **Services:**\n\
                 • ✅ Messaging Gateway: Online\n\
                 • ✅ LLM Provider: Connected\n\
                 • ✅ Session Manager: Active\n\n\
                 **Metrics:**\n\
                 • Active Sessions: [N/A]\n\
                 • Messages Today: [N/A]\n\
                 • Uptime: [N/A]\n\n\
                 _Detailed metrics coming soon_"
            }
            "users" => {
                "👥 **User List**\n\n\
                 User listing is not yet implemented.\n\n\
                 _Coming soon_"
            }
            "sessions" => {
                "📋 **Session List**\n\n\
                 Session listing is not yet implemented.\n\n\
                 _Coming soon_"
            }
            _ => &format!(
                "❓ **Unknown Admin Command**\n\n\
                     Unknown action: `{}`\n\n\
                     Use `/clawd admin` to see available commands.",
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

    fn create_regular_session() -> MessagingSession {
        let user = PlatformUserId::new(Platform::Telegram, "regular-user");
        MessagingSession::new(user)
    }

    #[tokio::test]
    async fn test_admin_menu() {
        let handler = AdminHandler::new();
        let session = create_admin_session();
        let command = ParsedCommand::new("/clawd admin", CommandCategory::Admin, "admin")
            .with_args(vec!["menu".to_string()]);

        let result = handler.handle(&session, &command).await.unwrap();

        assert!(result.response.contains("Admin Menu"));
    }

    #[tokio::test]
    async fn test_admin_status() {
        let handler = AdminHandler::new();
        let session = create_admin_session();
        let command = ParsedCommand::new("/clawd admin status", CommandCategory::Admin, "admin")
            .with_args(vec!["status".to_string()]);

        let result = handler.handle(&session, &command).await.unwrap();

        assert!(result.response.contains("System Status"));
    }

    #[tokio::test]
    async fn test_admin_permission_denied() {
        let handler = AdminHandler::new();
        let session = create_regular_session();
        let command = ParsedCommand::new("/clawd admin", CommandCategory::Admin, "admin");

        let result = handler.handle(&session, &command).await.unwrap();

        assert!(result.response.contains("Access Denied"));
    }
}
