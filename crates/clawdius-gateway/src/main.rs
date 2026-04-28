//! Clawdius Gateway binary.
//!
//! Starts the messaging gateway server that connects chat platforms
//! to the Clawdius agent engine.
//!
//! # Usage
//!
//! ```bash
//! clawdius-gateway --config config.toml
//! clawdius serve  # starts both agent + gateway
//! ```

use clap::Parser;
use std::path::PathBuf;

/// Clawdius Messaging Gateway
#[derive(Parser, Debug)]
#[command(name = "clawdius-gateway", version, about = "Connect chat platforms to the Clawdius agent")]
struct Cli {
    /// Path to configuration file
    #[arg(short, long, env = "CLAWDIUS_CONFIG")]
    config: Option<PathBuf>,

    /// Platform to enable (can be specified multiple times)
    #[arg(short = 'p', long = "platform", value_name = "PLATFORM")]
    platforms: Vec<String>,

    /// Telegram bot token
    #[arg(long, env = "TELEGRAM_BOT_TOKEN")]
    telegram_token: Option<String>,

    /// Discord bot token
    #[arg(long, env = "DISCORD_BOT_TOKEN")]
    discord_token: Option<String>,

    /// Slack bot token
    #[arg(long, env = "SLACK_BOT_TOKEN")]
    slack_token: Option<String>,

    /// Matrix homeserver URL
    #[arg(long, env = "MATRIX_HOMESERVER")]
    matrix_homeserver: Option<String>,

    /// Matrix access token
    #[arg(long, env = "MATRIX_ACCESS_TOKEN")]
    matrix_token: Option<String>,

    /// Webhook port
    #[arg(long, default_value_t = 8080)]
    port: u16,

    /// Webhook host
    #[arg(long, default_value = "0.0.0.0")]
    host: String,

    /// Log level
    #[arg(long, default_value = "info", env = "RUST_LOG")]
    log_level: String,

    /// Maximum requests per user per minute
    #[arg(long, default_value_t = 20)]
    rate_limit: usize,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| cli.log_level.parse().unwrap_or_else(|_| "info".parse().unwrap())),
        )
        .init();

    tracing::info!("🪲 Clawdius Gateway v{}", env!("CARGO_PKG_VERSION"));

    // Build and start the gateway
    let gateway = clawdius_gateway::MessageGateway::with_rate_limiter(cli.rate_limit, 60);

    tracing::info!("Registered platforms: {:?}", cli.platforms);
    tracing::info!("Rate limit: {} requests/minute", cli.rate_limit);
    tracing::info!("Webhook: {}:{} (when configured)", cli.host, cli.port);

    // The gateway will be fully wired once platform adapters are implemented.
    // For now, report status and wait for shutdown signal.
    tracing::info!("Gateway initialized. Waiting for platform adapter implementations...");

    // Wait for Ctrl+C
    tokio::signal::ctrl_c().await?;
    tracing::info!("Shutting down...");

    Ok(())
}
