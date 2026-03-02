//! Clawdius: High-Assurance Rust-Native Engineering Engine
//!
//! A next-generation AI agentic engine built for developers who can't afford
//! hallucinations and traders who can't afford latency.
//!
//! # Architecture
//! - **Host (Kernel):** Rust with monoio runtime (`io_uring`, thread-per-core)
//! - **Brain:** WASM/Wasmtime for logic isolation
//! - **Hands:** Bubblewrap/Podman for execution sandboxing
//!
//! # Features
//! - 12-phase Nexus R&D Lifecycle FSM
//! - Graph-RAG (AST + Vector) knowledge integration
//! - Sentinel JIT Sandboxing
//! - SOP Enforcement

#![deny(unsafe_code)]
#![warn(
    clippy::all,
    clippy::todo,
    clippy::dbg_macro
)]
#![allow(
    missing_docs,
    dead_code,
    clippy::pedantic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic_in_result_fn,
    clippy::float_cmp,
    clippy::uninlined_format_args,
    clippy::unnecessary_unwrap,
    clippy::len_zero,
    clippy::redundant_closure,
    clippy::cast_precision_loss,
    clippy::derive_partial_eq_without_eq,
    clippy::collapsible_if,
    clippy::trivially_copy_pass_by_ref,
    clippy::only_used_in_recursion,
    clippy::manual_midpoint,
    clippy::missing_fields_in_debug,
    clippy::should_implement_trait,
    clippy::return_self_not_must_use,
    clippy::useless_conversion,
    clippy::needless_pass_by_value,
    clippy::result_large_err,
    clippy::ptr_arg,
    clippy::too_many_arguments,
    clippy::type_complexity,
    clippy::inconsistent_digit_grouping,
    clippy::derivable_impls
)]

use std::process::ExitCode;

mod cli;

mod ast_store;
mod brain;
mod broker;
mod capability;
mod component;
mod config;
mod error;
mod fsm;
mod graph_rag;
mod host;
mod llm;
mod market_data;
mod mcp;
mod parser;
mod ring_buffer;
mod risk_types;
mod rpc;
mod sandbox;
mod signal_dispatch;
mod sentinel;
mod vector_store;
mod version;
mod wallet_guard;
mod tui;
mod wasm_runtime;

pub use ast_store::{
    AstEdge, AstNode, AstQuery, AstStore, CallGraph, EdgeType, IndexStats, Language, NodeId,
    NodeType,
};
pub use brain::{Brain, BrainConfig, BrainState, BRAIN_VERSION};
pub use broker::{Broker, BrokerConfig, BrokerMode, BrokerMetrics, BROKER_VERSION};
pub use capability::{
    CapabilityError, CapabilityRequest, CapabilityToken, HostPattern, PathPattern, Permission,
    PermissionSet, ResourceScope,
};
pub use component::{Component, ComponentId, ComponentInfo, ComponentState};
pub use config::{Config, RigorLevel};
pub use error::{BrainError, BrokerError, ClawdiusError, HostError, Result, SandboxError};
pub use fsm::{Phase, StateMachine};
pub use graph_rag::{GraphRag, GraphRagConfig, HybridQuery, QueryResult, GRAPH_RAG_VERSION};
pub use host::{Host, KernelState};
pub use market_data::{
    MarketData, MarketDataIngestor, MarketDataType, OrderBook, PriceLevel, Quote, Trade,
    WebSocketConfig, MAX_BOOK_DEPTH,
};
pub use ring_buffer::{MarketDataMessage, RingBuffer, RingBufferError};
pub use risk_types::{
    Account, AccountId, Currency, Money, Position, PositionId, PositionSide, RiskCheckResult,
    RiskLimits, RISK_REJECT_DRAWDOWN, RISK_REJECT_MARGIN, RISK_REJECT_ORDER_SIZE,
    RISK_REJECT_POSITION_LIMIT,
};
pub use signal_dispatch::{
    DispatchResult, DispatchStatus, Notification, NotificationChannel, NotificationGateway,
    Signal, SignalAction, SignalDispatcher, SignalId, SignalPriority, StrategyId,
};
pub use wallet_guard::{
    Order, OrderSide, RejectReason, RiskDecision, RiskParams, Wallet, WalletGuard,
    DEFAULT_MARGIN_RATIO, MAX_DRAWDOWN, MAX_ORDER_SIZE, MAX_POSITION,
};
pub use llm::{
    ChatRequest, ChatResponse, Embedding, EmbedRequest, EmbedResponse, FinishReason, LlmClient,
    LlmConfig, Message, MessageRole, Provider, ProviderConfig, Usage,
};
pub use mcp::{McpError, McpHost, McpTool, ToolDefinition, ToolRequest, ToolResponse};
pub use parser::{LanguageDetector, ParsedFile, Parser};
pub use rpc::{
    ProtocolVersion, RpcError, RpcMethod, RpcRequest, RpcResponse, UsageStats, PROTOCOL_VERSION,
};
pub use vector_store::{
    Chunk, Chunker, SearchResult, VectorStore, VectorStoreConfig, EMBEDDING_DIMENSION,
};
pub use sandbox::{
    select_sandbox_tier, CommandSpec, ExitStatus, GlobalPolicy, MountPoint, NativeSandbox,
    PlatformSandbox, SandboxConfig, SandboxHandle, SandboxTier, SettingsError, Toolchain,
    TrustLevel, validate_settings,
};
pub use sentinel::{Sentinel, SpawnRequest, SENTINEL_VERSION};
pub use version::VERSION;
pub use wasm_runtime::{
    create_engine, WasmConfig, WasmError, WasmRuntime, DEFAULT_FUEL, DEFAULT_MEMORY_LIMIT,
    DEFAULT_STACK_LIMIT, DEFAULT_TIMEOUT_SECS,
};

/// Global allocator for high-performance scenarios
/// Uses mimalloc for reduced lock contention in async workloads
#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

/// Application entry point
///
/// # Errors
/// Returns `ExitCode::FAILURE` if the application encounters a fatal error.
fn main() -> ExitCode {
    // Load .env file
    let _ = dotenvy::dotenv();

    init_logging();

    tracing::info!(
        version = %VERSION,
        "Clawdius initializing"
    );

    let args = cli::Cli::parse_args();

    if let Some(command) = &args.command {
        return handle_cli_command(command);
    }

    if args.no_tui || !atty::is(atty::Stream::Stdout) {
        return run_headless();
    }

    run_with_tui_mode()
}

fn handle_cli_command(command: &cli::Commands) -> ExitCode {
    match command {
        cli::Commands::Chat {
            message,
            model,
            provider,
        } => handle_chat_command(message, model.as_deref(), provider),
        cli::Commands::Init { path } => handle_init_command(path),
    }
}

fn handle_chat_command(message: &str, model: Option<&str>, provider_str: &str) -> ExitCode {
    use llm::{ChatRequest, LlmClient, Message};

    // Load .env file
    let _ = dotenvy::dotenv();

    let provider = match cli::Cli::parse_provider(provider_str) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Error: Invalid provider '{}': {}", provider_str, e);
            return ExitCode::FAILURE;
        }
    };

    if provider.load_api_key().is_none() {
        eprintln!(
            "Error: API key not found. Please set the {} environment variable.",
            provider.api_key_env()
        );
        return ExitCode::FAILURE;
    }

    let client = LlmClient::new();
    let mut request = ChatRequest::new(provider, vec![Message::user(message)]);
    if let Some(m) = model {
        request = request.with_model(m);
    }

    match client.chat(request) {
        Ok(response) => {
            println!("{}", response.message.content);
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("Error: Failed to get response from LLM: {}", e);
            ExitCode::FAILURE
        }
    }
}

fn handle_init_command(path: &std::path::Path) -> ExitCode {
    println!("Initializing Clawdius in: {}", path.display());
    match Config::load(path) {
        Ok(_config) => {
            println!("Clawdius initialized successfully");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("Error: Failed to initialize: {}", e);
            ExitCode::FAILURE
        }
    }
}

fn run_headless() -> ExitCode {
    println!("Clawdius running in headless mode");
    println!("Use 'clawdius chat \"message\"' to interact with the LLM");
    println!("Use 'clawdius --help' for more information");
    ExitCode::SUCCESS
}

fn run_with_tui_mode() -> ExitCode {
    let config = match Config::load(std::path::Path::new(".")) {
        Ok(cfg) => cfg,
        Err(e) => {
            tracing::error!("Failed to load configuration: {}", e);
            return ExitCode::FAILURE;
        }
    };

    match run_with_tui(config) {
        Ok(()) => {
            tracing::info!("Clawdius shutdown complete");
            ExitCode::SUCCESS
        }
        Err(e) => {
            tracing::error!("Fatal error: {}", e);
            ExitCode::FAILURE
        }
    }
}

/// Initialize the logging subsystem
fn init_logging() {
    use tracing_subscriber::{fmt, EnvFilter};

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,clawdius=debug"));

    fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_thread_ids(false)
        .compact()
        .init();
}

/// Run the application with TUI
///
/// # Errors
/// Returns an error if the application encounters a fatal error.
fn run_with_tui(config: Config) -> Result<()> {
    let mut host = Host::new(config)?;
    host.initialize()?;

    tracing::info!(
        session_id = %host.metadata().session_id,
        "Host kernel initialized"
    );

    // Get initial phase from FSM
    let phase = host.components().fsm().map_or(0, |fsm| fsm.current_phase().index());

    // Create and run TUI
    let mut tui = tui::Tui::new()?;
    tui.update_phase(phase);
    tui.update_rigor_score(0.85);

    // Run the TUI (blocking)
    tui.run()?;

    // Shutdown host
    host.shutdown()?;

    Ok(())
}
