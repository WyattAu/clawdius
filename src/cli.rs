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

    #[command(about = "Initialize a new Clawdius project in the current directory")]
    Init {
        /// Project name (defaults to directory name)
        name: Option<String>,
    },

    #[command(about = "Plan and execute a cross-language refactor")]
    Refactor {
        #[arg(short, long)]
        #[arg(help = "Source language (e.g., typescript, python)")]
        from: String,

        #[arg(short, long)]
        #[arg(help = "Target language (e.g., rust, go)")]
        to: String,

        #[arg(short, long, default_value = ".")]
        #[arg(help = "Path to file or directory")]
        path: PathBuf,

        #[arg(long)]
        #[arg(help = "Preview changes without applying")]
        dry_run: bool,
    },

    #[command(about = "Run Lean4 proof verification")]
    Verify {
        #[arg(short, long)]
        #[arg(help = "Path to .lean proof file or directory")]
        proof: PathBuf,

        #[arg(long)]
        #[arg(help = "Path to lean binary")]
        lean_path: Option<PathBuf>,
    },

    #[command(about = "Activate HFT broker mode")]
    Broker {
        #[arg(short, long)]
        #[arg(help = "Path to broker config")]
        config: Option<PathBuf>,

        #[arg(long)]
        #[arg(help = "Enable paper trading (no real orders)")]
        paper_trade: bool,
    },

    #[command(about = "Generate compliance matrix")]
    Compliance {
        #[arg(short, long)]
        #[arg(help = "Standards to include (comma-separated: iso26262,do178c,iec62304)")]
        standards: String,

        #[arg(short, long, default_value = ".")]
        #[arg(help = "Project root path")]
        path: PathBuf,

        #[arg(short, long, default_value = "markdown")]
        #[arg(help = "Output format (markdown, toml)")]
        format: String,

        #[arg(short, long)]
        #[arg(help = "Output file path")]
        output: Option<PathBuf>,
    },

    #[command(about = "Multi-lingual research synthesis")]
    Research {
        #[arg(help = "Research query")]
        query: String,

        #[arg(short, long)]
        #[arg(help = "Languages to search (comma-separated: en,zh,ru,de,jp)")]
        languages: Option<String>,

        #[arg(short = 'L', long, default_value = "3")]
        #[arg(help = "Minimum TQA level (1-5)")]
        tqa_level: u8,

        #[arg(short, long, default_value = "10")]
        #[arg(help = "Maximum results per language")]
        max_results: usize,
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
