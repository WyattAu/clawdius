//! Clawdius CLI Entry Point
//!
//! High-assurance Rust-native AI coding assistant.

#![deny(unsafe_code)]
#![allow(missing_docs)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

use clap::{CommandFactory, Parser};

use clawdius::cli;
mod tui_app;

pub use clawdius::cli::{Cli, Commands};
pub use tui_app::App as TuiApp;

#[cfg(feature = "mimalloc")]
#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() -> anyhow::Result<()> {
    // Initialize crash reporter early
    let _crash_reporter = clawdius_core::telemetry::CrashReporter::new();

    // Parse CLI arguments
    let cli = Cli::parse();

    // Generate shell completions if requested (before logging init)
    if let Some(shell) = cli.generate_completions.as_deref() {
        let shell = match shell {
            "zsh" => clap_complete::Shell::Zsh,
            "fish" => clap_complete::Shell::Fish,
            "powershell" | "pwsh" => clap_complete::Shell::PowerShell,
            // Default to bash for unknown shells or explicit "bash"
            _ => clap_complete::Shell::Bash,
        };
        let mut cmd = Cli::command();
        clap_complete::generate(shell, &mut cmd, "clawdius", &mut std::io::stdout());
        return Ok(());
    }

    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // Extract config path and output format before the match
    let config_path = cli.config.clone();
    let output_format = cli.output_format;

    // Run appropriate mode
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        match cli.command {
            Some(cmd) => cli::handle_command(cmd, config_path, output_format).await,
            None => {
                if cli.no_tui {
                    // Headless mode - read from stdin
                    cli::run_headless(config_path).await
                } else {
                    // Check onboarding status before running TUI
                    let status = clawdius_core::Onboarding::check_environment();
                    if status != clawdius_core::OnboardingStatus::Complete {
                        cli::first_run_experience();
                    }
                    // TUI mode
                    tui_app::run_tui().await
                }
            },
        }
    })
}
