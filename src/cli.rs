//! CLI argument parsing for Clawdius
//!
//! Provides command-line interface mode for non-TTY environments.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::llm::Provider;

#[derive(Parser)]
#[command(name = "clawdius")]
#[command(version, about = "High-Assurance Rust-Native Engineering Engine", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    #[arg(short, long)]
    #[arg(help = "Run without TUI (headless mode)")]
    pub no_tui: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    #[command(about = "Send a chat message to the LLM")]
    Chat {
        #[arg(help = "The message to send")]
        message: String,

        #[arg(short, long)]
        #[arg(help = "Model to use (defaults to provider's default model)")]
        model: Option<String>,

        #[arg(short = 'P', long, default_value = "zai")]
        #[arg(help = "Provider to use (anthropic, openai, deepseek, ollama, zai, openrouter)")]
        provider: String,
    },

    #[command(about = "Initialize Clawdius in a project")]
    Init {
        #[arg(default_value = ".")]
        #[arg(help = "Project path")]
        path: PathBuf,
    },
}

impl Cli {
    pub fn parse_args() -> Self {
        Self::parse()
    }

    pub fn parse_provider(s: &str) -> crate::Result<Provider> {
        s.parse::<Provider>()
            .map_err(|e| crate::error::ClawdiusError::Config(format!("Invalid provider: {}", e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parsing() {
        let cli = Cli::try_parse_from(["clawdius", "--version"]);
        assert!(cli.is_err());
    }
}
