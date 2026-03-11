//! Command parser

use crate::commands::{CommandTemplate, CustomCommand};
use crate::error::Result;

/// Parse a custom command from markdown
pub struct CommandParser;

impl CommandParser {
    /// Parse a command from markdown content
    pub fn parse(content: &str) -> Result<CustomCommand> {
        // Simple parser for command markdown files
        let lines: Vec<&str> = content.lines().collect();

        let name = lines
            .first()
            .map_or("Untitled", |l| l.trim_start_matches('#').trim())
            .to_string();

        Ok(CustomCommand {
            id: name.to_lowercase().replace(' ', "-"),
            name,
            description: String::new(),
            template: CommandTemplate::default(),
            arguments: Vec::new(),
        })
    }
}
