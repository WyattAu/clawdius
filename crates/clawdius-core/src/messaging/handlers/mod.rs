//! Command Handlers
//!
//! Handlers for different command categories in the messaging gateway.

mod admin;
mod analyze;
mod config;
mod generate;
mod help;
mod session;
mod status;

use super::gateway::MessageHandler;

// Re-export handlers
pub use admin::AdminHandler;
pub use analyze::AnalyzeHandler;
pub use config::ConfigHandler;
pub use generate::GenerateHandler;
pub use help::HelpHandler;
pub use session::SessionHandler;
pub use status::StatusHandler;

/// Base handler trait with common functionality
pub trait BaseHandler: MessageHandler {
    /// Returns the help text for this handler
    fn help_text(&self) -> &'static str;
}
