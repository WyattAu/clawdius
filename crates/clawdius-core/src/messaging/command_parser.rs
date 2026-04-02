//! Command Parser
//!
//! Parses incoming messages from various messaging platforms into structured commands.

use regex::Regex;
use std::collections::HashMap;

use super::types::{CommandCategory, MessagingError, ParsedCommand, Platform, Result};

fn build_regex(pattern: &str) -> Result<Regex> {
    Regex::new(pattern)
        .map_err(|e| MessagingError::ParseError(format!("Invalid regex '{pattern}': {e}")))
}

pub struct CommandParser {
    platform: Platform,
    command_patterns: HashMap<CommandCategory, Vec<Regex>>,
}

impl CommandParser {
    pub fn new(platform: Platform) -> Result<Self> {
        let mut patterns = HashMap::new();

        patterns.insert(
            CommandCategory::Session,
            vec![
                build_regex(r"(?i)^(start|new|create)\s*(session|chat)?")?,
                build_regex(r"(?i)^(stop|end|close)\s*(session|chat)?")?,
                build_regex(r"(?i)^sessions?$")?,
            ],
        );

        patterns.insert(
            CommandCategory::Status,
            vec![
                build_regex(r"(?i)^status$")?,
                build_regex(r"(?i)^ping$")?,
                build_regex(r"(?i)^health$")?,
            ],
        );

        patterns.insert(
            CommandCategory::Generate,
            vec![
                build_regex(
                    r"(?i)^(generate|gen|create)\s+(code|function|class|struct|test|tests)",
                )?,
                build_regex(r"(?i)^(write|add|implement)\s+(code|function|class|struct)")?,
            ],
        );

        patterns.insert(
            CommandCategory::Analyze,
            vec![
                build_regex(r"(?i)^(analyze|analyse|review|explain)\s*")?,
                build_regex(r"(?i)^(what|how|why|where)\s+(is|are|does|do)")?,
            ],
        );

        patterns.insert(
            CommandCategory::Timeline,
            vec![
                build_regex(r"(?i)^timeline\s*(list|history)?")?,
                build_regex(r"(?i)^checkpoint\s*(create|save|list)?")?,
                build_regex(r"(?i)^rollback\s*")?,
                build_regex(r"(?i)^diff\s*")?,
            ],
        );

        patterns.insert(
            CommandCategory::Config,
            vec![
                build_regex(r"(?i)^config\s*(show|get|set|list)?")?,
                build_regex(r"(?i)^set\s+(provider|model|mode)")?,
            ],
        );

        patterns.insert(
            CommandCategory::Admin,
            vec![
                build_regex(r"(?i)^admin\s*")?,
                build_regex(r"(?i)^debug\s*")?,
                build_regex(r"(?i)^clear\s*(cache|history|sessions)")?,
            ],
        );

        patterns.insert(
            CommandCategory::Help,
            vec![
                build_regex(r"(?i)^help$")?,
                build_regex(r"(?i)^commands$")?,
                build_regex(r"(?i)^usage$")?,
            ],
        );

        Ok(Self {
            platform,
            command_patterns: patterns,
        })
    }

    pub fn parse(&self, message: &str) -> Result<ParsedCommand> {
        let prefix = self.platform.command_prefix();
        let content = message.trim();

        if !content.starts_with(prefix) {
            return Err(MessagingError::InvalidCommandFormat {
                command: content.to_string(),
                expected: format!("Command must start with '{}'", prefix.trim()),
            });
        }

        let content = content.strip_prefix(prefix).unwrap_or(content).trim();

        if content.is_empty() {
            return Err(MessagingError::InvalidCommandFormat {
                command: message.to_string(),
                expected: "Command cannot be empty".to_string(),
            });
        }

        let (category, action) = self.categorize_command(content);

        let (args, flags) = self.parse_args_and_flags(content);

        Ok(ParsedCommand::new(message, category, action)
            .with_args(args)
            .with_flags(flags))
    }

    fn categorize_command(&self, content: &str) -> (CommandCategory, String) {
        let first_word = content.split_whitespace().next().unwrap_or("");

        for (category, patterns) in &self.command_patterns {
            for pattern in patterns {
                if pattern.is_match(content) {
                    return (*category, first_word.to_lowercase());
                }
            }
        }

        (CommandCategory::Unknown, first_word.to_lowercase())
    }

    fn parse_args_and_flags(&self, content: &str) -> (Vec<String>, HashMap<String, String>) {
        let tokens: Vec<String> = content.split_whitespace().map(|s| s.to_string()).collect();

        let mut args = Vec::new();
        let mut flags = HashMap::new();
        let mut i = 0;

        while i < tokens.len() {
            let token = &tokens[i];

            if let Some(stripped) = token.strip_prefix("--") {
                if let Some((key, value)) = stripped.split_once('=') {
                    flags.insert(key.to_string(), value.to_string());
                } else if i + 1 < tokens.len() && !tokens[i + 1].starts_with('-') {
                    // --flag value (next token is the value)
                    flags.insert(stripped.to_string(), tokens[i + 1].clone());
                    i += 1; // skip the value token
                } else {
                    // --flag (boolean, no value)
                    flags.insert(stripped.to_string(), "true".to_string());
                }
            } else if let Some(stripped) = token.strip_prefix('-') {
                for ch in stripped.chars() {
                    flags.insert(ch.to_string(), "true".to_string());
                }
            } else {
                args.push(token.clone());
            }

            i += 1;
        }

        (args, flags)
    }
}

pub fn chunk_message(content: &str, max_length: usize) -> Vec<String> {
    if content.len() <= max_length {
        return vec![content.to_string()];
    }

    let mut chunks = Vec::new();
    let mut remaining = content;

    while !remaining.is_empty() {
        if remaining.len() <= max_length {
            chunks.push(remaining.to_string());
            break;
        }

        let split_point = remaining[..max_length]
            .rfind(|c| c == '\n' || c == ' ' || c == '.')
            .unwrap_or(max_length.min(1));

        chunks.push(remaining[..=split_point].to_string());
        remaining = &remaining[split_point + 1..];
    }

    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_command() {
        let parser = CommandParser::new(Platform::Telegram).unwrap();
        let result = parser.parse("/clawd status").unwrap();

        assert_eq!(result.category, CommandCategory::Status);
        assert_eq!(result.action, "status");
    }

    #[test]
    fn test_parse_command_with_args() {
        let parser = CommandParser::new(Platform::Telegram).unwrap();
        let result = parser.parse("/clawd generate code --lang rust").unwrap();

        assert_eq!(result.category, CommandCategory::Generate);
        assert_eq!(result.args, vec!["generate", "code"]);
        assert_eq!(result.flag("lang"), Some("rust"));
    }

    #[test]
    fn test_parse_invalid_command() {
        let parser = CommandParser::new(Platform::Telegram).unwrap();
        let result = parser.parse("invalid command");

        assert!(result.is_err());
    }

    #[test]
    fn test_chunk_message() {
        let content = "This is a test message that is longer than the limit and needs to be chunked into smaller pieces.";
        let chunks = chunk_message(content, 30);

        assert!(chunks.len() > 1);
        for chunk in &chunks {
            assert!(chunk.len() <= 30);
        }
    }

    #[test]
    fn test_matrix_command_prefix() {
        let parser = CommandParser::new(Platform::Matrix).unwrap();
        let result = parser.parse("!clawd status").unwrap();

        assert_eq!(result.category, CommandCategory::Status);
    }
}
