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

    // Create server
    let server = RpcServer::new();

    // Register session handlers
    server
        .register_handler("session/create", Arc::new(SessionHandler))
        .await;
    server
        .register_handler("session/load", Arc::new(SessionHandler))
        .await;
    server
        .register_handler("session/save", Arc::new(SessionHandler))
        .await;
    server
        .register_handler("session/list", Arc::new(SessionHandler))
        .await;
    server
        .register_handler("session/delete", Arc::new(SessionHandler))
        .await;

    // Register chat handlers
    server
        .register_handler("chat/send", Arc::new(ChatHandler))
        .await;
    server
        .register_handler("chat/stream", Arc::new(ChatHandler))
        .await;
    server
        .register_handler("chat/cancel", Arc::new(ChatHandler))
        .await;

    // Register file handlers
    server
        .register_handler("file/read", Arc::new(FileHandler))
        .await;
    server
        .register_handler("file/write", Arc::new(FileHandler))
        .await;

    // Register context handlers
    server
        .register_handler("context/add", Arc::new(ContextHandler))
        .await;
    server
        .register_handler("context/remove", Arc::new(ContextHandler))
        .await;
    server
        .register_handler("context/list", Arc::new(ContextHandler))
        .await;
    server
        .register_handler("context/compact", Arc::new(ContextHandler))
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

    // Register completion handlers
    server
        .register_handler("completion/inline", Arc::new(CompletionHandler::new()))
        .await;

    // Log to stderr (for debugging)
    eprintln!(
        "Clawdius Code v{} - JSON-RPC Server",
        env!("CARGO_PKG_VERSION")
    );
    eprintln!("Listening on stdin for JSON-RPC requests...");

    // Run stdio server
    server.run_stdio().await?;

    Ok(())
}
