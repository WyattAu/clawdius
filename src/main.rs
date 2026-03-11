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
mod knowledge;
mod brain;
mod broker;
mod capability;
mod compliance;
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
mod proof;
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
pub use compliance::{
    Artifact, ArtifactType, ComplianceGap, ComplianceLevel, ComplianceMatrix,
    ComplianceMatrixGenerator, ComplianceStatus, EvidenceType, Requirement, RequirementMapping,
    Standard, StandardDomain,
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
pub use proof::{
    Priority, Proof, ProofGenerator, ProofId, ProofRequest, ProofResult, ProofStatus,
    Property, PropertyType,
};
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
pub use knowledge::{
    Concept, KnowledgeGraph, Language as ResearchLanguage, ResearchFinding, ResearchSynthesizer,
    SynthesisRequest, SynthesisResult, TqaLevel,
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

    if args.no_tui || !std::io::stdout().is_terminal() {
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
        cli::Commands::Refactor {
            from,
            to,
            path,
            dry_run,
        } => handle_refactor_command(from, to, path, *dry_run),
        cli::Commands::Verify { proof, lean_path } => {
            handle_verify_command(proof, lean_path.as_deref())
        }
        cli::Commands::Broker {
            config,
            paper_trade,
        } => handle_broker_command(config.as_deref(), *paper_trade),
        cli::Commands::Compliance {
            standards,
            path,
            format,
            output,
        } => handle_compliance_command(standards, path, format, output.as_deref()),
        cli::Commands::Research {
            query,
            languages,
            tqa_level,
            max_results,
        } => handle_research_command(query, languages.as_deref(), *tqa_level, *max_results),
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

fn handle_refactor_command(from: &str, to: &str, path: &std::path::Path, dry_run: bool) -> ExitCode {
    println!("Refactoring: {} -> {}", from, to);
    println!("Path: {}", path.display());

    let graph_path = path.join(".clawdius/graph");
    let config = GraphRagConfig::with_root(&graph_path);
    let mut graph_rag = match GraphRag::new(config) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("Error initializing Graph-RAG: {}", e);
            return ExitCode::FAILURE;
        }
    };

    if let Err(e) = graph_rag.initialize() {
        eprintln!("Error initializing Graph-RAG: {}", e);
        return ExitCode::FAILURE;
    }

    println!("Indexing project...");
    
    let mut rt = match monoio::RuntimeBuilder::<monoio::IoUringDriver>::new()
        .enable_timer()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("Error creating runtime: {}", e);
            return ExitCode::FAILURE;
        }
    };

    let stats = match rt.block_on(graph_rag.index_project(path)) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error indexing project: {}", e);
            return ExitCode::FAILURE;
        }
    };

    println!(
        "Indexed {} nodes, {} edges in {} files",
        stats.node_count, stats.edge_count, stats.files_indexed
    );

    println!("\nMigration Plan:");
    println!("================");
    println!("Source Language: {}", from);
    println!("Target Language: {}", to);
    println!("Files to Migrate: {}", stats.files_indexed);

    if dry_run {
        println!("\n[DRY RUN] No changes applied.");
        println!("Run without --dry-run to apply changes.");
    } else {
        println!("\nRefactoring not yet implemented.");
        println!("Use --dry-run to preview changes.");
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}

fn handle_verify_command(
    proof_path: &std::path::Path,
    lean_path: Option<&std::path::Path>,
) -> ExitCode {
    use std::path::PathBuf;

    let lean_bin = lean_path
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("lean"));

    println!("Lean4 Proof Verification");
    println!("=========================");
    println!("Proof path: {}", proof_path.display());
    println!("Lean binary: {}", lean_bin.display());

    let version_output = match std::process::Command::new(&lean_bin)
        .arg("--version")
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            eprintln!("Error: Lean binary not found: {}", e);
            eprintln!("Install Lean4: https://leanprover.github.io/lean4/doc/setup.html");
            return ExitCode::FAILURE;
        }
    };

    if !version_output.status.success() {
        eprintln!("Error: Failed to get Lean version");
        return ExitCode::FAILURE;
    }

    print!(
        "Lean version: {}",
        String::from_utf8_lossy(&version_output.stdout)
    );

    let mut verified = 0;
    let mut failed = 0;

    if proof_path.is_file() {
        match verify_lean_proof(&lean_bin, proof_path) {
            Ok(()) => {
                println!("✓ {} - VERIFIED", proof_path.display());
                verified += 1;
            }
            Err(e) => {
                println!("✗ {} - FAILED: {}", proof_path.display(), e);
                failed += 1;
            }
        }
    } else if proof_path.is_dir() {
        for entry in walkdir::WalkDir::new(proof_path)
            .into_iter()
            .filter_map(std::result::Result::ok)
            .filter(|e| {
                e.path()
                    .extension()
                    .map(|ext| ext == "lean")
                    .unwrap_or(false)
            })
        {
            match verify_lean_proof(&lean_bin, entry.path()) {
                Ok(()) => {
                    println!("✓ {} - VERIFIED", entry.path().display());
                    verified += 1;
                }
                Err(e) => {
                    println!("✗ {} - FAILED: {}", entry.path().display(), e);
                    failed += 1;
                }
            }
        }
    }

    println!("\nResults: {} verified, {} failed", verified, failed);

    if failed > 0 {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn verify_lean_proof(
    lean_bin: &std::path::Path,
    proof_path: &std::path::Path,
) -> std::result::Result<(), String> {
    let output = std::process::Command::new(lean_bin)
        .arg(proof_path)
        .output()
        .map_err(|e| format!("Failed to run lean: {}", e))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(stderr.to_string())
    }
}

fn handle_broker_command(
    config_path: Option<&std::path::Path>,
    paper_trade: bool,
) -> ExitCode {
    use std::path::PathBuf;

    println!("Clawdius HFT Broker");
    println!("==================");

    if paper_trade {
        println!("Mode: PAPER TRADING (no real orders)");
    } else {
        println!("Mode: LIVE TRADING");
        println!("WARNING: Real trading mode enabled!");
    }

    let config_path = config_path
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(".clawdius/broker.toml"));

    println!("Config: {}", config_path.display());

    let broker_config = BrokerConfig {
        mode: if paper_trade {
            BrokerMode::Paper
        } else {
            BrokerMode::Live
        },
        ..Default::default()
    };

    let mut broker = match Broker::new(broker_config) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Error initializing broker: {}", e);
            return ExitCode::FAILURE;
        }
    };

    if let Err(e) = broker.initialize() {
        eprintln!("Error initializing broker: {}", e);
        return ExitCode::FAILURE;
    }

    println!("\nBroker initialized successfully");
    println!("Press Ctrl+C to shutdown");

    println!("\nBroker ready. Awaiting market data...");
    println!("(Full implementation requires signal handling and event loop)");

    ExitCode::SUCCESS
}

fn handle_compliance_command(
    standards_str: &str,
    path: &std::path::Path,
    format: &str,
    output: Option<&std::path::Path>,
) -> ExitCode {
    use compliance::{parse_standards, scan_for_artifacts, ComplianceMatrixGenerator};

    println!("Clawdius Compliance Matrix Generator");
    println!("=====================================");

    let standards = parse_standards(standards_str);
    if standards.is_empty() {
        eprintln!("Error: No valid standards specified");
        eprintln!("Valid standards: iso26262, do178c, iec62304, iec61508, en50128, ieee1016, nist800-53, etc.");
        return ExitCode::FAILURE;
    }

    println!("Standards: {}", standards.len());
    for std in &standards {
        println!("  - {}", std.full_name());
    }

    println!("\nScanning project: {}", path.display());
    let artifacts = scan_for_artifacts(path);
    println!("Found {} artifacts", artifacts.len());

    let generator = ComplianceMatrixGenerator::new();
    let project_name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "Project".into());

    println!("\nGenerating compliance matrix...");
    let matrix = generator.generate(&project_name, &standards, &artifacts);

    println!("Compliance: {:.1}%", matrix.compliance_percentage);
    println!("Requirements mapped: {}", matrix.requirements.len());
    println!("Gaps identified: {}", matrix.gaps.len());

    let output_content = match format.to_lowercase().as_str() {
        "toml" => generator.export_toml(&matrix),
        _ => generator.export_markdown(&matrix),
    };

    match output {
        Some(output_path) => {
            if let Err(e) = std::fs::write(output_path, &output_content) {
                eprintln!("Error writing output file: {}", e);
                return ExitCode::FAILURE;
            }
            println!("\nOutput written to: {}", output_path.display());
        }
        None => {
            println!("\n{}", output_content);
        }
    }

    if matrix.compliance_percentage < 100.0 {
        println!("\nCompliance gaps detected. Review the matrix for required actions.");
    }

    ExitCode::SUCCESS
}

fn handle_research_command(
    query: &str,
    languages: Option<&str>,
    tqa_level: u8,
    max_results: usize,
) -> ExitCode {
    use std::path::PathBuf;

    println!("Multi-Lingual Research Synthesis");
    println!("=================================");
    println!("Query: {}", query);

    let langs: Vec<ResearchLanguage> = languages
        .map(|s| {
            s.split(',')
                .filter_map(|l| match l.trim().to_lowercase().as_str() {
                    "en" => Some(ResearchLanguage::EN),
                    "zh" | "cn" => Some(ResearchLanguage::ZH),
                    "ru" => Some(ResearchLanguage::RU),
                    "de" => Some(ResearchLanguage::DE),
                    "fr" => Some(ResearchLanguage::FR),
                    "jp" | "ja" => Some(ResearchLanguage::JP),
                    "ko" | "kr" => Some(ResearchLanguage::KO),
                    "es" => Some(ResearchLanguage::ES),
                    "it" => Some(ResearchLanguage::IT),
                    "pt" => Some(ResearchLanguage::PT),
                    "nl" => Some(ResearchLanguage::NL),
                    "pl" => Some(ResearchLanguage::PL),
                    "cs" => Some(ResearchLanguage::CS),
                    "ar" => Some(ResearchLanguage::AR),
                    "fa" => Some(ResearchLanguage::FA),
                    "tr" => Some(ResearchLanguage::TR),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_else(|| {
            vec![
                ResearchLanguage::EN,
                ResearchLanguage::ZH,
                ResearchLanguage::RU,
                ResearchLanguage::DE,
                ResearchLanguage::JP,
            ]
        });

    println!("Languages: {:?}", langs);
    println!("Min TQA Level: {}", tqa_level);
    println!("Max Results/Language: {}", max_results);

    let knowledge_path = PathBuf::from(".knowledge_graph/concept_mappings.md");
    let mut synthesizer = ResearchSynthesizer::new();

    if knowledge_path.exists() {
        if let Err(e) = synthesizer.load_graph(&knowledge_path) {
            println!("\nNote: Could not load knowledge graph: {}", e);
            println!("Using empty graph for demonstration.");
        }
    } else {
        println!("\nNote: Knowledge graph not found at {:?}", knowledge_path);
        println!("Run 'clawdius init' to create the default structure.");
    }

    let min_tqa = match tqa_level {
        5 => TqaLevel::Level5,
        4 => TqaLevel::Level4,
        3 => TqaLevel::Level3,
        2 => TqaLevel::Level2,
        _ => TqaLevel::Level1,
    };

    let request = SynthesisRequest {
        query: query.into(),
        languages: langs,
        min_tqa_level: min_tqa,
        max_results_per_language: max_results,
        domain: None,
    };

    println!(
        "\nSearching across {} languages...",
        request.languages.len()
    );
    println!("Results would be synthesized from: ");
    for lang in &request.languages {
        println!("  - {} databases: {:?}", lang.name(), lang.databases());
    }

    println!(
        "\nKnowledge graph contains {} concepts.",
        synthesizer.graph().count()
    );

    ExitCode::SUCCESS
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
    let config = clawdius_core::LoggingConfig::default();
    clawdius_core::init_logging(&config);
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
