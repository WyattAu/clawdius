//! Clawdius Code - `VSCode` Extension Helper
//!
//! This binary communicates with `VSCode` extension via JSON-RPC over stdio.

#![deny(unsafe_code)]
#![warn(missing_docs)]

#[cfg(feature = "mimalloc")]
#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() -> anyhow::Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async { run_server().await })
}

async fn run_server() -> anyhow::Result<()> {
    use clawdius_core::rpc::{
        handlers::{
            ChatHandler, CompletionHandler, ContextHandler, FileHandler, SessionHandler,
            StateHandler,
        },
        RpcServer,
    };
    use std::sync::Arc;

    // Load configuration
    let config = clawdius_core::Config::load_default().unwrap_or_else(|e| {
        eprintln!("Warning: failed to load config: {e}, using defaults");
        clawdius_core::Config::default()
    });

    // Create LLM client from config (best-effort — works even if no provider configured)
    let llm_client = create_llm_client(&config);

    // Create shared session store
    let sessions = SessionHandler::new();

    // Create server
    let server = RpcServer::new();

    // Register session handlers (shared session store)
    let session_create = SessionHandler::with_store(sessions.sessions_clone());
    let session_load = SessionHandler::with_store(sessions.sessions_clone());
    let session_save = SessionHandler::with_store(sessions.sessions_clone());
    let session_list = SessionHandler::with_store(sessions.sessions_clone());
    let session_delete = SessionHandler::with_store(sessions.sessions_clone());

    server
        .register_handler("session/create", Arc::new(session_create))
        .await;
    server
        .register_handler("session/load", Arc::new(session_load))
        .await;
    server
        .register_handler("session/save", Arc::new(session_save))
        .await;
    server
        .register_handler("session/list", Arc::new(session_list))
        .await;
    server
        .register_handler("session/delete", Arc::new(session_delete))
        .await;

    // Register chat handlers (with LLM client)
    let chat_handler = ChatHandler::with_llm_opt(llm_client.clone());
    server
        .register_handler(
            "chat/send",
            Arc::new(ChatHandler::with_llm_opt(llm_client.clone())),
        )
        .await;
    server
        .register_handler(
            "chat/stream",
            Arc::new(ChatHandler::with_llm_opt(llm_client.clone())),
        )
        .await;
    server
        .register_handler(
            "chat/cancel",
            Arc::new(ChatHandler::with_llm_opt(llm_client.clone())),
        )
        .await;
    drop(chat_handler);

    // Register file handlers
    server
        .register_handler("file/read", Arc::new(FileHandler))
        .await;
    server
        .register_handler("file/write", Arc::new(FileHandler))
        .await;

    // Register context handlers
    server
        .register_handler("context/add", Arc::new(ContextHandler::new()))
        .await;
    server
        .register_handler("context/remove", Arc::new(ContextHandler::new()))
        .await;
    server
        .register_handler("context/list", Arc::new(ContextHandler::new()))
        .await;
    server
        .register_handler("context/compact", Arc::new(ContextHandler::new()))
        .await;

    // Register state handlers
    server
        .register_handler("state/get", Arc::new(StateHandler))
        .await;
    server
        .register_handler("state/checkpoint", Arc::new(StateHandler))
        .await;
    server
        .register_handler("state/restore", Arc::new(StateHandler))
        .await;
    server
        .register_handler("state/list", Arc::new(StateHandler))
        .await;

    // Register completion handler (with LLM client)
    let completion_handler = match llm_client {
        Some(ref client) => CompletionHandler::with_llm(Arc::clone(client)),
        None => CompletionHandler::new(),
    };
    server
        .register_handler("completion/inline", Arc::new(completion_handler))
        .await;

    // Log to stderr (for debugging)
    eprintln!(
        "Clawdius Code v{} - JSON-RPC Server",
        env!("CARGO_PKG_VERSION")
    );
    if llm_client.is_some() {
        eprintln!("LLM client: configured");
    } else {
        eprintln!("LLM client: not configured (completions will use mock fallback)");
    }
    eprintln!("Listening on stdin for JSON-RPC requests...");

    // Run stdio server
    server.run_stdio().await?;

    Ok(())
}

/// Create an LLM client from the config file.
/// Returns `None` if no provider is configured or creation fails.
fn create_llm_client(
    config: &clawdius_core::Config,
) -> Option<std::sync::Arc<dyn clawdius_core::llm::LlmClient>> {
    use clawdius_core::llm::{create_provider, LlmConfig};

    let provider_name = match &config.llm.default_provider {
        Some(p) if !p.is_empty() => p.as_str(),
        _ => return None,
    };

    let llm_config = match LlmConfig::from_config(&config.llm, provider_name) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Warning: failed to create LLM config for '{provider_name}': {e}");
            return None;
        },
    };

    match create_provider(&llm_config) {
        Ok(client) => {
            eprintln!(
                "Initialized LLM provider: {provider_name} (model: {})",
                llm_config.model
            );
            Some(std::sync::Arc::new(client))
        },
        Err(e) => {
            eprintln!("Warning: failed to initialize LLM provider '{provider_name}': {e}");
            None
        },
    }
}
