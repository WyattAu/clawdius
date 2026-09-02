#![deny(unsafe_code)]
#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]

//! Clawdius Gateway binary.
//!
//! Starts the messaging gateway server that connects chat platforms
//! to the Clawdius agent engine.
//!
//! # Usage
//!
//! ```bash
//! clawdius-gateway --platform telegram --telegram-bot-token "..."
//! clawdius-gateway --platform discord --platform webhook --webhook-url "..."
//! ```

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::State as AxumState;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use clap::Parser;

use clawdius_gateway::adapter::{Platform, PlatformConfig};
use clawdius_gateway::admin::{admin_router, AdminState};
use clawdius_gateway::handler::ClawdiusHandler;
use clawdius_gateway::MessageGateway;

#[cfg(feature = "discord")]
use clawdius_gateway::adapters::discord::DiscordAdapter;
#[cfg(feature = "matrix")]
use clawdius_gateway::adapters::matrix::MatrixAdapter;
use clawdius_gateway::adapters::rocketchat::RocketChatAdapter;
use clawdius_gateway::adapters::signal::SignalAdapter;
#[cfg(feature = "slack")]
use clawdius_gateway::adapters::slack::SlackAdapter;
use clawdius_gateway::adapters::teams::TeamsAdapter;
#[cfg(feature = "telegram")]
use clawdius_gateway::adapters::telegram::TelegramAdapter;
use clawdius_gateway::adapters::webhook::{WebhookAdapter, WebhookAdapterConfig};
use clawdius_gateway::adapters::whatsapp::WhatsAppAdapter;

use clawdius_core::billing::BillingManager;
use clawdius_core::usage::TenantUsageTracker;

/// Clawdius Messaging Gateway
#[derive(Parser, Debug)]
#[command(
    name = "clawdius-gateway",
    version,
    about = "Connect chat platforms to the Clawdius agent"
)]
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

    /// Slack app token (for Socket Mode)
    #[arg(long, env = "SLACK_APP_TOKEN")]
    slack_app_token: Option<String>,

    /// Matrix homeserver URL
    #[arg(long, env = "MATRIX_HOMESERVER")]
    matrix_homeserver: Option<String>,

    /// Matrix access token
    #[arg(long, env = "MATRIX_ACCESS_TOKEN")]
    matrix_token: Option<String>,

    /// Matrix user ID
    #[arg(long, env = "MATRIX_USER_ID")]
    matrix_user_id: Option<String>,

    /// Signal account number
    #[arg(long, env = "SIGNAL_ACCOUNT_NUMBER")]
    signal_account: Option<String>,

    /// Signal REST API URL
    #[arg(long, env = "SIGNAL_REST_URL")]
    signal_url: Option<String>,

    /// Teams App ID
    #[arg(long, env = "TEAMS_APP_ID")]
    teams_app_id: Option<String>,

    /// Teams App Password
    #[arg(long, env = "TEAMS_APP_PASSWORD")]
    teams_app_password: Option<String>,

    /// Teams Service URL
    #[arg(long, env = "TEAMS_SERVICE_URL")]
    teams_service_url: Option<String>,

    /// `WhatsApp` access token
    #[arg(long, env = "WHATSAPP_ACCESS_TOKEN")]
    whatsapp_token: Option<String>,

    /// `WhatsApp` phone number ID
    #[arg(long, env = "WHATSAPP_PHONE_NUMBER_ID")]
    whatsapp_phone_id: Option<String>,

    /// Rocket.Chat server URL
    #[arg(long, env = "ROCKETCHAT_URL")]
    rocketchat_url: Option<String>,

    /// Rocket.Chat auth token
    #[arg(long, env = "ROCKETCHAT_TOKEN")]
    rocketchat_token: Option<String>,

    /// Rocket.Chat user ID
    #[arg(long, env = "ROCKETCHAT_USER_ID")]
    rocketchat_user_id: Option<String>,

    /// Webhook outgoing URL
    #[arg(long, env = "WEBHOOK_URL")]
    webhook_url: Option<String>,

    /// Webhook secret
    #[arg(long, env = "WEBHOOK_SECRET")]
    webhook_secret: Option<String>,

    /// Webhook listen port
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

    /// Admin API key for authentication
    #[arg(long, env = "CLAWDIUS_ADMIN_API_KEY")]
    admin_api_key: Option<String>,

    /// Admin HTTP server host
    #[arg(long, default_value = "0.0.0.0")]
    admin_host: String,

    /// Admin HTTP server port
    #[arg(long, default_value_t = 8081)]
    admin_port: u16,

    /// LLM provider override
    #[arg(long, env = "CLAWDIUS_PROVIDER")]
    provider: Option<String>,

    /// LLM model override
    #[arg(long, env = "CLAWDIUS_MODEL")]
    model: Option<String>,
}

/// Shared state for the gateway health endpoint.
struct GatewayHealthState {
    gateway: Arc<MessageGateway>,
}

async fn gateway_health(AxumState(state): AxumState<Arc<GatewayHealthState>>) -> impl IntoResponse {
    let health = state.gateway.health_status().await;
    let map: HashMap<String, serde_json::Value> = health
        .into_iter()
        .map(|(platform, h)| {
            (
                platform.as_str().to_string(),
                serde_json::json!({
                    "healthy": h.healthy,
                    "message": h.message,
                    "messages_processed": h.messages_processed,
                    "errors": h.errors,
                    "last_message_at": h.last_message_at.map(|dt| dt.to_rfc3339()),
                }),
            )
        })
        .collect();
    (
        axum::http::StatusCode::OK,
        Json(serde_json::json!({
            "ok": true,
            "data": map,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        })),
    )
}

/// Prometheus metrics endpoint.
async fn metrics_handler() -> impl IntoResponse {
    (
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4",
        )],
        clawdius_core::metrics::render_metrics(),
    )
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
}

/// Register a single platform adapter on the gateway.
#[allow(clippy::too_many_lines)]
async fn register_platform(gateway: &mut MessageGateway, platform: &Platform, cli: &Cli) {
    match platform {
        Platform::Telegram => {
            #[cfg(feature = "telegram")]
            {
                match cli.telegram_token.as_ref() {
                    Some(token) => {
                        let config = PlatformConfig::with_token(*platform, token);
                        gateway
                            .register_adapter(TelegramAdapter::new(token), config)
                            .await;
                        tracing::info!("Registered adapter for platform: {platform}");
                    },
                    None => tracing::warn!("Skipping telegram: TELEGRAM_BOT_TOKEN not set"),
                }
            }
            #[cfg(not(feature = "telegram"))]
            tracing::warn!(
                "Skipping telegram: feature not enabled (compile with --features telegram)"
            );
        },
        Platform::Discord => {
            #[cfg(feature = "discord")]
            {
                match cli.discord_token.as_ref() {
                    Some(token) => {
                        let config = PlatformConfig::with_token(*platform, token);
                        gateway
                            .register_adapter(DiscordAdapter::new(token), config)
                            .await;
                        tracing::info!("Registered adapter for platform: {platform}");
                    },
                    None => tracing::warn!("Skipping discord: DISCORD_BOT_TOKEN not set"),
                }
            }
            #[cfg(not(feature = "discord"))]
            tracing::warn!(
                "Skipping discord: feature not enabled (compile with --features discord)"
            );
        },
        Platform::Slack => {
            #[cfg(feature = "slack")]
            {
                match cli.slack_token.as_ref() {
                    Some(token) => {
                        let config = PlatformConfig::with_token(*platform, token);
                        gateway
                            .register_adapter(SlackAdapter::new(token), config)
                            .await;
                        tracing::info!("Registered adapter for platform: {platform}");
                    },
                    None => tracing::warn!("Skipping slack: SLACK_BOT_TOKEN not set"),
                }
            }
            #[cfg(not(feature = "slack"))]
            tracing::warn!("Skipping slack: feature not enabled (compile with --features slack)");
        },
        Platform::Matrix => {
            #[cfg(feature = "matrix")]
            {
                match (
                    cli.matrix_homeserver.as_ref(),
                    cli.matrix_token.as_ref(),
                    cli.matrix_user_id.as_ref(),
                ) {
                    (Some(homeserver), Some(token), Some(user_id)) => {
                        let config = PlatformConfig::with_token(*platform, token);
                        gateway
                            .register_adapter(MatrixAdapter::new(homeserver, token, user_id), config)
                            .await;
                        tracing::info!("Registered adapter for platform: {platform}");
                    }
                    _ => tracing::warn!("Skipping matrix: MATRIX_HOMESERVER, MATRIX_ACCESS_TOKEN, and MATRIX_USER_ID must all be set"),
                }
            }
            #[cfg(not(feature = "matrix"))]
            tracing::warn!("Skipping matrix: feature not enabled (compile with --features matrix)");
        },
        Platform::Signal => {
            if let Some(account) = cli.signal_account.as_ref() {
                let url = cli.signal_url.as_deref().unwrap_or("http://localhost:7583");
                let config = PlatformConfig::with_token(*platform, account);
                gateway
                    .register_adapter(SignalAdapter::new(url, account), config)
                    .await;
                tracing::info!("Registered adapter for platform: {platform}");
            } else {
                tracing::warn!("Skipping signal: SIGNAL_ACCOUNT_NUMBER not set");
            }
        },
        Platform::Teams => {
            if let (Some(service_url), Some(app_id), Some(app_password)) = (
                cli.teams_service_url.as_ref(),
                cli.teams_app_id.as_ref(),
                cli.teams_app_password.as_ref(),
            ) {
                let config = PlatformConfig::with_token(*platform, app_id);
                gateway
                    .register_adapter(TeamsAdapter::new(service_url, app_id, app_password), config)
                    .await;
                tracing::info!("Registered adapter for platform: {platform}");
            } else {
                tracing::warn!("Skipping teams: TEAMS_SERVICE_URL, TEAMS_APP_ID, and TEAMS_APP_PASSWORD must all be set");
            }
        },
        Platform::WhatsApp => {
            if let (Some(token), Some(phone_id)) =
                (cli.whatsapp_token.as_ref(), cli.whatsapp_phone_id.as_ref())
            {
                let config = PlatformConfig::with_token(*platform, token);
                gateway
                    .register_adapter(WhatsAppAdapter::new(token, phone_id), config)
                    .await;
                tracing::info!("Registered adapter for platform: {platform}");
            } else {
                tracing::warn!("Skipping whatsapp: WHATSAPP_ACCESS_TOKEN and WHATSAPP_PHONE_NUMBER_ID must both be set");
            }
        },
        Platform::RocketChat => {
            if let (Some(server_url), Some(auth_token), Some(rc_user_id)) = (
                cli.rocketchat_url.as_ref(),
                cli.rocketchat_token.as_ref(),
                cli.rocketchat_user_id.as_ref(),
            ) {
                let config = PlatformConfig::with_token(*platform, auth_token);
                gateway
                    .register_adapter(
                        RocketChatAdapter::new(server_url, auth_token, rc_user_id),
                        config,
                    )
                    .await;
                tracing::info!("Registered adapter for platform: {platform}");
            } else {
                tracing::warn!("Skipping rocketchat: ROCKETCHAT_URL, ROCKETCHAT_TOKEN, and ROCKETCHAT_USER_ID must all be set");
            }
        },
        Platform::Webhook => {
            if let Some(outgoing_url) = cli.webhook_url.as_ref() {
                let webhook_config = WebhookAdapterConfig {
                    outgoing_url: outgoing_url.clone(),
                    secret: cli.webhook_secret.clone(),
                    outgoing_headers: HashMap::new(),
                    listen_port: cli.port,
                };
                let config = PlatformConfig::with_token(*platform, outgoing_url);
                gateway
                    .register_adapter(WebhookAdapter::new(webhook_config), config)
                    .await;
                tracing::info!("Registered adapter for platform: {platform}");
            } else {
                tracing::warn!("Skipping webhook: WEBHOOK_URL not set");
            }
        },
        _ => {
            tracing::warn!("Unknown platform variant: '{platform}'. Skipping.");
        },
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                cli.log_level
                    .parse()
                    .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"))
            }),
        )
        .init();

    tracing::info!(
        "Clawdius Gateway v{} starting up",
        env!("CARGO_PKG_VERSION")
    );

    let mut gateway = MessageGateway::with_rate_limiter(cli.rate_limit, 60);
    tracing::info!("Rate limit: {} requests/minute", cli.rate_limit);

    if cli.platforms.is_empty() {
        tracing::warn!(
            "No platforms specified via --platform. Gateway will start with no adapters."
        );
    }

    // Register all requested platform adapters
    for platform_str in &cli.platforms {
        let Some(platform) = Platform::from_str(platform_str) else {
            tracing::error!("Unknown platform: '{platform_str}'. Skipping.");
            continue;
        };
        register_platform(&mut gateway, &platform, &cli).await;
    }

    let gateway = Arc::new(gateway);

    // Configure the Clawdius handler
    let mut handler = ClawdiusHandler::new();
    if let Some(ref config_path) = cli.config {
        handler = handler.with_config_path(config_path);
    }
    if let Some(ref provider) = cli.provider {
        handler = handler.with_provider(provider);
    }
    if let Some(ref model) = cli.model {
        handler = handler.with_model(model);
    }

    // Initialize audit logging if config provides audit settings
    #[cfg(feature = "audit")]
    {
        let config = if let Some(ref path) = cli.config {
            clawdius_core::Config::load(path).ok()
        } else {
            clawdius_core::Config::load_default().ok()
        };
        if let Some(ref config) = config {
            match clawdius_core::audit::AuditManager::from_config(&config.messaging.audit) {
                Ok(manager) => {
                    tracing::info!(
                        "Audit logging enabled (backend: {}, flush: {}s)",
                        config.messaging.audit.backend,
                        config.messaging.audit.flush_interval_secs
                    );

                    // Spawn periodic flush task
                    let flush_interval =
                        std::time::Duration::from_secs(config.messaging.audit.flush_interval_secs);
                    let manager = Arc::new(manager);

                    let flush_mgr = Arc::clone(&manager);
                    tokio::spawn(async move {
                        let mut interval = tokio::time::interval(flush_interval);
                        loop {
                            interval.tick().await;
                            if let Err(e) = flush_mgr.flush().await {
                                tracing::warn!("Audit flush error: {e}");
                            }
                        }
                    });

                    // Pass to handler
                    handler = handler.with_audit_manager(manager);
                },
                Err(e) => {
                    tracing::warn!("Failed to initialize audit logging: {e}");
                },
            }
        }
    }

    gateway.set_handler(Box::new(handler)).await;

    // Wire auth services into gateway for SSO token validation
    #[cfg(feature = "auth")]
    {
        let auth_config = clawdius_auth::AuthConfig::default();
        if let Ok(auth_svc) = clawdius_auth::AuthService::new(auth_config) {
            let auth_arc = Arc::new(auth_svc);
            let rbac_arc = Arc::new(clawdius_auth::RbacService::new(
                clawdius_auth::rbac::RbacPolicy::default(),
            ));
            gateway.set_auth_service(Arc::clone(&auth_arc)).await;
            gateway.set_rbac_service(Arc::clone(&rbac_arc)).await;
            tracing::info!("SSO token validation enabled on gateway");
        }
    }

    // Start all adapters (uses Arc to pass callback without raw pointers)
    let start_results = gateway.start_all_arc().await;
    for (platform, result) in start_results {
        match result {
            Ok(()) => tracing::info!("Platform '{platform}' adapter started"),
            Err(e) => tracing::error!("Platform '{platform}' adapter failed to start: {e}"),
        }
    }

    // Build admin API state and router
    let admin_state = Arc::new(AdminState {
        billing: Arc::new(BillingManager::new()),
        usage: Arc::new(TenantUsageTracker::new()),
        api_key: cli
            .admin_api_key
            .unwrap_or_else(|| "clawdius-admin".to_string()),
        roles: Default::default(),
        #[cfg(feature = "auth")]
        auth: None,
        #[cfg(feature = "auth")]
        rbac: None,
    });

    let health_state = Arc::new(GatewayHealthState {
        gateway: Arc::clone(&gateway),
    });

    let health_router = Router::new()
        .route("/api/gateway/health", get(gateway_health))
        .route("/metrics", get(metrics_handler))
        .with_state(health_state);

    let admin_app = admin_router(admin_state).merge(health_router);

    // Mount OIDC + SAML auth routes (if auth feature is enabled)
    #[cfg(feature = "auth")]
    let admin_app = {
        let auth_config = clawdius_auth::AuthConfig::default();
        match clawdius_auth::AuthService::new(auth_config) {
            Ok(service) => {
                let auth_arc = Arc::new(service);
                let rbac_arc = Arc::new(clawdius_auth::RbacService::new(
                    clawdius_auth::rbac::RbacPolicy::default(),
                ));

                // Rebuild admin state with auth services injected
                let admin_state = Arc::new(AdminState {
                    billing: Arc::clone(&admin_state.billing),
                    usage: Arc::clone(&admin_state.usage),
                    api_key: admin_state.api_key.clone(),
                    roles: admin_state.roles.clone(),
                    auth: Some(Arc::clone(&auth_arc)),
                    rbac: Some(Arc::clone(&rbac_arc)),
                });

                let sp_config = Arc::new(clawdius_auth::SamlSpConfig {
                    entity_id: std::env::var("SAML_ENTITY_ID")
                        .unwrap_or_else(|_| "https://clawdius.local".to_string()),
                    acs_url: std::env::var("SAML_ACS_URL")
                        .unwrap_or_else(|_| "https://clawdius.local/saml/acs".to_string()),
                    slo_url: std::env::var("SAML_SLO_URL").ok(),
                    certificate: std::env::var("SAML_CERTIFICATE").ok(),
                    idp_certificate: std::env::var("SAML_IDP_CERTIFICATE").ok(),
                    enabled: true,
                });
                tracing::info!("OIDC routes: /login, /callback, /logout, /me, /refresh");
                tracing::info!("SAML routes: /saml/metadata, /saml/acs");
                admin_app
                    .merge(clawdius_auth::auth_routes(Arc::clone(&auth_arc)))
                    .merge(clawdius_auth::saml_routes(sp_config))
                    .layer(axum::Extension(auth_arc))
            },
            Err(e) => {
                tracing::warn!("Failed to initialize auth service: {e}");
                admin_app
            },
        }
    };

    // Start admin server
    let admin_addr = format!("{}:{}", cli.admin_host, cli.admin_port);
    let admin_listener = tokio::net::TcpListener::bind(&admin_addr).await?;
    tracing::info!("Admin API listening on {admin_addr}");

    let admin_server =
        axum::serve(admin_listener, admin_app).with_graceful_shutdown(shutdown_signal());

    tracing::info!("Gateway is running. Press Ctrl+C to shut down.");
    admin_server.await?;

    // Graceful shutdown
    tracing::info!("Shutting down gateway...");
    let stop_results = gateway.stop_all().await;
    for (platform, result) in &stop_results {
        match result {
            Ok(()) => tracing::info!("Platform '{platform}' adapter stopped"),
            Err(e) => tracing::warn!("Platform '{platform}' adapter stop error: {e}"),
        }
    }

    tracing::info!("Gateway shutdown complete.");
    Ok(())
}
