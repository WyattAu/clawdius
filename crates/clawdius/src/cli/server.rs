use std::path::{Path, PathBuf};

pub(super) async fn handle_server(host: &str, port: u16) -> anyhow::Result<()> {
    use clawdius_core::api::rest::ApiState;
    use clawdius_core::session::SessionStore;
    use tower_http::cors::CorsLayer;

    let config = clawdius_core::config::Config::load_or_default();

    let sessions_dir = std::path::Path::new(".clawdius");
    std::fs::create_dir_all(sessions_dir)?;
    let sessions_path = sessions_dir.join("sessions.db");
    let store = SessionStore::open(&sessions_path)
        .map_err(|e| anyhow::anyhow!("Failed to create session store: {e}"))?;

    let mut state = ApiState::new(store);

    let provider_name = config
        .llm
        .default_provider
        .as_deref()
        .unwrap_or("anthropic");
    match clawdius_core::llm::LlmConfig::from_config(&config.llm, provider_name)
        .and_then(|llm_config| clawdius_core::llm::create_provider(&llm_config))
    {
        Ok(provider) => {
            state = state.with_llm_client(provider);
        },
        Err(e) => {
            eprintln!("Warning: LLM provider not configured: {e}. Chat endpoint will return 503.");
        },
    }

    let app = clawdius_core::api::rest::create_router(state).layer(CorsLayer::permissive());

    // Start the admin API (gateway health + billing) on port+1
    let admin_port = port.saturating_add(1);
    let admin_addr = std::net::SocketAddr::from(([0, 0, 0, 0], admin_port));
    let admin_state = std::sync::Arc::new(clawdius_gateway::admin::AdminState {
        billing: std::sync::Arc::new(clawdius_core::billing::BillingManager::new()),
        usage: std::sync::Arc::new(clawdius_core::usage::TenantUsageTracker::new()),
        api_key: std::env::var("CLAWDIUS_ADMIN_API_KEY").unwrap_or_else(|_| "clawdius-admin".to_string()),
    });
    let admin_router = clawdius_gateway::admin::admin_router(admin_state);
    let admin_listener = tokio::net::TcpListener::bind(admin_addr).await?;
    println!("Clawdius admin API listening on {host}:{admin_port}");

    // Spawn admin server as background task
    let admin_handle = tokio::spawn(async move {
        if let Err(e) = axum::serve(admin_listener, admin_router).await {
            eprintln!("Admin server error: {e}");
        }
    });

    // Start the REST API server
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    println!("Clawdius server listening on {host}:{port}");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    // Shutdown admin server
    admin_handle.abort();
    println!("Clawdius server shut down.");

    Ok(())
}

pub(super) async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to install Ctrl+C handler: {e}")).ok();
}
