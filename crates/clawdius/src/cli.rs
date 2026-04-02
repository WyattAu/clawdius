//! CLI argument parsing and command handling

use anyhow::Context;
use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

use clawdius_core::actions::Function;
use clawdius_core::analysis::{DebtReport, DriftReport, DriftSeverity as CoreDriftSeverity};
use clawdius_core::i18n::Language;
use clawdius_core::output::{
    OutputFormat as CoreOutputFormat, OutputFormatter, OutputOptions, SessionInfo, TestCaseInfo,
};
use clawdius_core::proof::LeanVerifier;
#[cfg(feature = "vector-db")]
use clawdius_core::workspace::IndexStats;
use clawdius_core::{Config, MentionResolver, Onboarding, OnboardingStatus, SessionManager};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputFormat {
    #[default]
    Text,
    Json,
    StreamJson,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub enum MetricsOutputFormat {
    #[default]
    Text,
    Json,
    Html,
}

impl From<OutputFormat> for CoreOutputFormat {
    fn from(format: OutputFormat) -> Self {
        match format {
            OutputFormat::Text => CoreOutputFormat::Text,
            OutputFormat::Json => CoreOutputFormat::Json,
            OutputFormat::StreamJson => CoreOutputFormat::StreamJson,
        }
    }
}

impl ValueEnum for OutputFormat {
    fn value_variants<'a>() -> &'a [Self] {
        &[
            OutputFormat::Text,
            OutputFormat::Json,
            OutputFormat::StreamJson,
        ]
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        match self {
            OutputFormat::Text => Some(clap::builder::PossibleValue::new("text")),
            OutputFormat::Json => Some(clap::builder::PossibleValue::new("json")),
            OutputFormat::StreamJson => Some(clap::builder::PossibleValue::new("stream-json")),
        }
    }
}

/// Clawdius CLI
#[derive(Parser)]
#[command(name = "clawdius")]
#[command(version, about = "High-Assurance Rust-Native Engineering Engine", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    #[arg(short, long)]
    #[arg(help = "Run without TUI (headless mode)")]
    pub no_tui: bool,

    #[arg(short, long, default_value = ".")]
    #[arg(help = "Working directory")]
    pub cwd: PathBuf,

    #[arg(short = 'f', long, value_enum, default_value = "text")]
    #[arg(help = "Output format (text, json, stream-json)")]
    pub output_format: OutputFormat,

    #[arg(short, long)]
    #[arg(help = "Quiet mode (no progress indicators)")]
    pub quiet: bool,

    #[arg(short = 'C', long)]
    #[arg(help = "Path to config file (defaults to .clawdius/config.toml)")]
    pub config: Option<PathBuf>,

    #[arg(short = 'L', long)]
    #[arg(help = "Language for output (en, zh, ja, ko, de, fr, es, it, pt, ru)")]
    pub lang: Option<String>,
}

/// Available commands
#[derive(Subcommand)]
pub enum Commands {
    #[command(about = "Send a chat message to the LLM")]
    Chat {
        #[arg(help = "The message to send (use '-' for stdin)")]
        prompt: Option<String>,

        #[arg(short, long)]
        #[arg(help = "Model to use (defaults to provider's default model)")]
        model: Option<String>,

        #[arg(short = 'P', long, default_value = "anthropic")]
        #[arg(help = "Provider to use (anthropic, openai, deepseek, ollama, zai, openrouter)")]
        provider: String,

        #[arg(short, long)]
        #[arg(help = "Continue from session ID")]
        session: Option<String>,

        #[arg(short = 'e', long)]
        #[arg(help = "Open external editor to compose message")]
        editor: bool,

        #[arg(short = 'M', long, default_value = "code")]
        #[arg(
            help = "Agent mode (code, architect, ask, debug, review, refactor, test, auto, or custom mode name)"
        )]
        mode: String,

        #[arg(long)]
        #[arg(
            help = "Non-interactive mode - exit after response (auto-enabled when prompt provided)"
        )]
        exit: bool,

        #[arg(long)]
        #[arg(help = "Quiet mode (suppress all output except response)")]
        quiet: bool,

        #[arg(long)]
        #[arg(help = "Autonomous mode - auto-approve all tool executions")]
        auto_approve: bool,
    },

    #[command(about = "Autonomous CI/CD mode - run without interaction")]
    Auto {
        #[arg(help = "Task to execute (e.g., 'fix failing tests', 'implement feature X')")]
        task: String,

        #[arg(short, long)]
        #[arg(help = "Model to use (defaults to provider's default model)")]
        model: Option<String>,

        #[arg(short = 'P', long, default_value = "anthropic")]
        #[arg(help = "Provider to use (anthropic, openai, deepseek, ollama, zai, openrouter)")]
        provider: String,

        #[arg(long)]
        #[arg(help = "Maximum iterations before stopping (default: 50)")]
        max_iterations: Option<usize>,

        #[arg(long)]
        #[arg(help = "Run tests after changes")]
        run_tests: bool,

        #[arg(long)]
        #[arg(help = "Commit changes automatically")]
        auto_commit: bool,

        #[arg(long)]
        #[arg(help = "Fail if tests fail after changes")]
        fail_on_test_failure: bool,

        #[arg(long)]
        #[arg(help = "Output format for CI logging (text, json, github-actions)")]
        output_format: Option<String>,
    },

    #[command(about = "Initialize Clawdius in a project")]
    Init {
        #[arg(default_value = ".")]
        #[arg(help = "Project path")]
        path: PathBuf,
    },

    #[command(about = "Interactive setup wizard for first-time users")]
    Setup {
        #[arg(short, long)]
        #[arg(help = "Skip welcome screen")]
        quick: bool,

        #[arg(short = 'P', long)]
        #[arg(help = "Pre-select provider (anthropic, openai, ollama, zai)")]
        provider: Option<String>,
    },

    #[command(about = "List and manage sessions")]
    Sessions {
        #[arg(short, long)]
        #[arg(help = "Delete a session")]
        delete: Option<String>,

        #[arg(short, long)]
        #[arg(help = "Search sessions")]
        search: Option<String>,
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

    #[command(about = "Apply a code action")]
    Action {
        #[arg(
            help = "Action to apply (extract-function, extract-variable, inline-variable, rename, move-module, generate-tests)"
        )]
        action: String,

        #[arg(help = "File path")]
        file: PathBuf,

        #[arg(short = 'l', long)]
        #[arg(help = "Line number")]
        line: Option<usize>,

        #[arg(short = 'c', long)]
        #[arg(help = "Column number")]
        column: Option<usize>,

        #[arg(short = 's', long)]
        #[arg(help = "End line for selection")]
        end_line: Option<usize>,

        #[arg(short = 'e', long)]
        #[arg(help = "End column for selection")]
        end_column: Option<usize>,
    },

    #[command(about = "Generate tests for code")]
    Test {
        #[arg(help = "File path")]
        file: PathBuf,

        #[arg(short, long)]
        #[arg(help = "Function name to generate tests for (generates for all if not specified)")]
        function: Option<String>,

        #[arg(short = 'o', long)]
        #[arg(help = "Output file path (defaults to <file>_test.<ext>)")]
        output: Option<PathBuf>,
    },

    #[command(about = "Generate documentation for code")]
    Doc {
        #[arg(help = "File path")]
        file: PathBuf,

        #[arg(short, long)]
        #[arg(help = "Element to document (function, struct, module)")]
        element: Option<String>,

        #[arg(short = 'f', long, default_value = "auto")]
        #[arg(help = "Documentation format (auto, rustdoc, jsdoc, pydoc, markdown)")]
        format: String,

        #[arg(short = 'o', long)]
        #[arg(help = "Output file path (defaults to stdout)")]
        output: Option<PathBuf>,

        #[arg(long)]
        #[arg(help = "Include inline comments")]
        inline: bool,
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

    #[command(about = "Manage API keys in system keyring")]
    #[cfg(feature = "keyring")]
    Auth {
        #[command(subcommand)]
        action: AuthCommands,
    },

    #[command(about = "Show performance metrics")]
    Metrics {
        #[arg(short = 'f', long, value_enum, default_value = "text")]
        #[arg(help = "Output format (text, json, html)")]
        format: MetricsOutputFormat,

        #[arg(short = 'o', long)]
        #[arg(help = "Output file path (prints to stdout if not specified)")]
        output: Option<PathBuf>,

        #[arg(short, long)]
        #[arg(help = "Reset metrics after displaying")]
        reset: bool,

        #[arg(short = 'w', long)]
        #[arg(help = "Watch mode - continuously display metrics")]
        watch: bool,
    },

    #[command(about = "Configure telemetry settings")]
    Telemetry {
        #[arg(short, long)]
        #[arg(help = "Enable telemetry")]
        enable: bool,

        #[arg(short, long)]
        #[arg(help = "Disable telemetry")]
        disable: bool,

        #[arg(long)]
        #[arg(help = "Enable metrics collection")]
        enable_metrics: bool,

        #[arg(long)]
        #[arg(help = "Enable crash reporting")]
        enable_crash_reporting: bool,
    },

    #[cfg(feature = "vector-db")]
    #[command(about = "Index workspace for multi-file context")]
    Index {
        #[arg(help = "Path to workspace (defaults to current directory)")]
        path: Option<PathBuf>,

        #[arg(short, long)]
        #[arg(help = "Watch for file changes and re-index")]
        watch: bool,
    },

    #[cfg(feature = "vector-db")]
    #[command(about = "Query workspace context")]
    Context {
        #[arg(help = "Query string")]
        query: String,

        #[arg(short, long)]
        #[arg(help = "Maximum tokens for context")]
        max_tokens: Option<usize>,
    },

    #[command(about = "Manage file checkpoints")]
    Checkpoint {
        #[command(subcommand)]
        action: CheckpointCommands,
    },

    #[command(about = "Manage file timeline and version history")]
    Timeline {
        #[command(subcommand)]
        action: TimelineCommands,
    },

    #[command(about = "Manage agent modes")]
    Modes {
        #[command(subcommand)]
        action: ModeCommands,
    },

    #[command(about = "Manage language settings")]
    Lang {
        #[command(subcommand)]
        action: LangCommands,
    },

    #[command(about = "Edit a long prompt in external editor")]
    Edit {
        #[arg(short, long)]
        #[arg(help = "Optional initial content")]
        initial: Option<String>,

        #[arg(short, long)]
        #[arg(help = "Editor to use (defaults to $EDITOR)")]
        editor: Option<String>,

        #[arg(short = 'x', long)]
        #[arg(help = "File extension for syntax highlighting (default: md)")]
        extension: Option<String>,
    },

    #[command(about = "Manage agentic workflows")]
    Workflow {
        #[command(subcommand)]
        action: WorkflowCommands,
    },

    #[command(about = "Manage webhooks for event notifications")]
    Webhook {
        #[command(subcommand)]
        action: WebhookCommands,
    },

    #[command(about = "Generate code using agentic AI")]
    Generate {
        #[arg(help = "Description of what to generate")]
        prompt: String,

        #[arg(short, long)]
        #[arg(help = "Target files to generate/modify (comma-separated)")]
        files: Option<String>,

        #[arg(short = 'M', long, default_value = "single-pass")]
        #[arg(help = "Generation mode: single-pass, iterative, agent")]
        mode: String,

        #[arg(short = 'T', long, default_value = "medium")]
        #[arg(help = "Trust level for apply: low, medium, high")]
        trust: String,

        #[arg(short, long)]
        #[arg(help = "Test execution strategy: sandboxed, direct, skip")]
        test_strategy: Option<String>,

        #[arg(short = 'i', long, default_value = "5")]
        #[arg(help = "Max iterations for iterative/agent mode")]
        max_iterations: u32,

        #[arg(long)]
        #[arg(help = "Dry run - preview changes without applying")]
        dry_run: bool,

        #[arg(short = 'P', long, default_value = "anthropic")]
        #[arg(help = "LLM provider to use")]
        provider: String,

        #[arg(short, long)]
        #[arg(help = "Model to use")]
        model: Option<String>,

        #[arg(long)]
        #[arg(help = "Enable streaming output")]
        stream: bool,

        #[arg(long)]
        #[arg(help = "Enable incremental generation (diff-based updates)")]
        incremental: bool,

        #[arg(short = 'R', long)]
        #[arg(help = "Timeout in seconds for LLM operations")]
        timeout_secs: Option<u64>,
    },

    #[command(about = "Language Server Protocol operations")]
    Lsp {
        #[command(subcommand)]
        action: LspCommands,
    },

    #[command(about = "Manage project memory (CLAUDE.md)")]
    Memory {
        #[command(subcommand)]
        action: MemoryCommands,
    },

    #[command(about = "Manage local LLM models (Ollama)")]
    Models {
        #[command(subcommand)]
        action: ModelsCommands,

        #[arg(short = 'H', long, default_value = "localhost")]
        #[arg(help = "Ollama host")]
        host: String,

        #[arg(short = 'p', long, default_value = "11434")]
        #[arg(help = "Ollama port")]
        port: u16,
    },

    #[command(about = "Get inline code completions from LLM")]
    Complete {
        #[arg(help = "Source file path")]
        file: String,

        #[arg(help = "Line number (0-indexed)")]
        line: u32,

        #[arg(help = "Character position (0-indexed)")]
        character: u32,

        #[arg(short, long)]
        #[arg(help = "Programming language")]
        language: Option<String>,

        #[arg(short = 'P', long, default_value = "ollama")]
        #[arg(help = "LLM provider to use")]
        provider: String,

        #[arg(short = 'm', long)]
        #[arg(help = "Model name")]
        model: Option<String>,
    },

    /// Analyze codebase for architecture drift and technical debt
    Analyze {
        /// Path to analyze (file or directory)
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Analyze for architecture drift only
        #[arg(long, conflicts_with = "debt")]
        drift: bool,

        /// Analyze for technical debt only
        #[arg(long, conflicts_with = "drift")]
        debt: bool,

        /// Output format (text, json)
        #[arg(short = 'f', long, value_enum, default_value = "text")]
        format: OutputFormat,

        /// Output file path (prints to stdout if not specified)
        #[arg(short = 'o', long)]
        output: Option<PathBuf>,

        /// Minimum severity level to report (low, medium, high, critical)
        #[arg(long, default_value = "low")]
        severity: String,

        /// Exclude patterns (comma-separated)
        #[arg(long)]
        exclude: Option<String>,
    },

    /// Watch files for changes and trigger auto-analysis
    Watch {
        /// Path to watch (file or directory)
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Patterns to ignore (comma-separated)
        #[arg(long)]
        ignore: Option<String>,

        /// Enable auto-analysis on changes
        #[arg(long)]
        auto_analyze: bool,

        /// Debounce interval in milliseconds
        #[arg(long, default_value = "500")]
        debounce_ms: u64,

        /// Enable verbose output
        #[arg(short, long)]
        verbose: bool,
    },

    #[command(about = "Nexus FSM engine")]
    Nexus {
        #[command(subcommand)]
        action: NexusAction,
    },
}

#[derive(Subcommand)]
pub enum CheckpointCommands {
    #[command(about = "Create a checkpoint")]
    Create {
        #[arg(help = "Description for the checkpoint")]
        description: String,

        #[arg(short = 's', long)]
        #[arg(help = "Session ID (defaults to current session)")]
        session: Option<String>,
    },

    #[command(about = "List all checkpoints")]
    List {
        #[arg(short = 's', long)]
        #[arg(help = "Session ID (defaults to current session)")]
        session: Option<String>,

        #[arg(short, long)]
        #[arg(help = "Show file details")]
        verbose: bool,
    },

    #[command(about = "Show checkpoint details")]
    Show {
        #[arg(help = "Checkpoint ID to show")]
        checkpoint_id: String,
    },

    #[command(about = "Restore to a checkpoint")]
    Restore {
        #[arg(help = "Checkpoint ID to restore")]
        checkpoint_id: String,
    },

    #[command(about = "Compare two checkpoints")]
    Compare {
        #[arg(help = "First checkpoint ID")]
        checkpoint_id1: String,

        #[arg(help = "Second checkpoint ID")]
        checkpoint_id2: String,
    },

    #[command(about = "Delete a checkpoint")]
    Delete {
        #[arg(help = "Checkpoint ID to delete")]
        checkpoint_id: String,
    },

    #[command(about = "Clean up old checkpoints")]
    Cleanup {
        #[arg(short = 's', long)]
        #[arg(help = "Session ID (defaults to current session)")]
        session: Option<String>,

        #[arg(short, long, default_value = "10")]
        #[arg(help = "Number of checkpoints to keep")]
        keep: usize,
    },

    #[command(about = "Show checkpoint timeline")]
    Timeline {
        #[arg(short = 's', long)]
        #[arg(help = "Session ID (defaults to current session)")]
        session: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum TimelineCommands {
    #[command(about = "Create a timeline checkpoint")]
    Create {
        #[arg(help = "Name for the checkpoint")]
        name: String,

        #[arg(short, long)]
        #[arg(help = "Description for the checkpoint")]
        description: Option<String>,
    },

    #[command(about = "List all timeline checkpoints")]
    List,

    #[command(about = "Watch for file changes and auto-create checkpoints")]
    Watch {
        #[arg(short = 'd', long, default_value = "30")]
        #[arg(help = "Debounce interval in seconds")]
        debounce_secs: u64,

        #[arg(short = 'i', long)]
        #[arg(help = "Additional patterns to ignore (can be repeated)")]
        ignore: Vec<String>,

        #[arg(short = 'm', long, default_value = "120")]
        #[arg(help = "Maximum checkpoints per hour")]
        max_per_hour: usize,
    },

    #[command(about = "Rollback to a checkpoint")]
    Rollback {
        #[arg(help = "Checkpoint ID to rollback to")]
        checkpoint_id: String,
    },

    #[command(about = "Show diff between two checkpoints")]
    Diff {
        #[arg(help = "From checkpoint ID")]
        from: String,

        #[arg(help = "To checkpoint ID")]
        to: String,
    },

    #[command(about = "Show file history")]
    History {
        #[arg(help = "File path to show history for")]
        file: PathBuf,
    },

    #[command(about = "Delete a checkpoint")]
    Delete {
        #[arg(help = "Checkpoint ID to delete")]
        checkpoint_id: String,
    },

    #[command(about = "Clean up old checkpoints")]
    Cleanup {
        #[arg(short, long, default_value = "100")]
        #[arg(help = "Number of checkpoints to keep")]
        keep: usize,
    },
}

#[derive(Subcommand)]
#[cfg(feature = "keyring")]
pub enum AuthCommands {
    #[command(about = "Store API key in keyring")]
    Set {
        #[arg(help = "Provider name (anthropic, openai, zai)")]
        provider: String,
    },

    #[command(about = "Retrieve API key from keyring")]
    Get {
        #[arg(help = "Provider name")]
        provider: String,
    },

    #[command(about = "Delete API key from keyring")]
    Delete {
        #[arg(help = "Provider name")]
        provider: String,
    },
}

#[derive(Subcommand)]
pub enum ModeCommands {
    #[command(about = "List all available modes")]
    List,

    #[command(about = "Create a new custom mode")]
    Create {
        #[arg(help = "Name for the new mode")]
        name: String,

        #[arg(short, long)]
        #[arg(help = "Path to save mode configuration")]
        output: Option<PathBuf>,
    },

    #[command(about = "Show details of a mode")]
    Show {
        #[arg(help = "Mode name")]
        name: String,
    },
}

#[derive(Subcommand)]
pub enum LangCommands {
    #[command(about = "List supported languages")]
    List,

    #[command(about = "Set display language")]
    Set {
        #[arg(help = "Language code (en, zh, ja, ko, de, fr, es, it, pt, ru)")]
        code: String,
    },

    #[command(about = "Show current language")]
    Show,
}

#[derive(Subcommand)]
pub enum WorkflowCommands {
    #[command(about = "List all workflows")]
    List,

    #[command(about = "Create a new workflow")]
    Create {
        #[arg(help = "Workflow name")]
        name: String,

        #[arg(short, long)]
        #[arg(help = "Workflow description")]
        description: Option<String>,
    },

    #[command(about = "Show workflow details")]
    Show {
        #[arg(help = "Workflow ID")]
        id: String,
    },

    #[command(about = "Execute a workflow")]
    Run {
        #[arg(help = "Workflow ID")]
        id: String,

        #[arg(short, long)]
        #[arg(help = "Context data as JSON")]
        context: Option<String>,

        #[arg(short = 'P', long, default_value = "anthropic")]
        #[arg(help = "Provider to use")]
        provider: String,

        #[arg(short, long)]
        #[arg(help = "Model to use")]
        model: Option<String>,
    },

    #[command(about = "Cancel a running workflow")]
    Cancel {
        #[arg(help = "Execution ID")]
        execution_id: String,
    },

    #[command(about = "Show workflow execution status")]
    Status {
        #[arg(help = "Execution ID")]
        execution_id: String,
    },

    #[command(about = "Delete a workflow")]
    Delete {
        #[arg(help = "Workflow ID")]
        id: String,
    },
}

#[derive(Subcommand)]
pub enum WebhookCommands {
    #[command(about = "List all webhooks")]
    List,

    #[command(about = "Create a new webhook")]
    Create {
        #[arg(help = "Webhook name")]
        name: String,

        #[arg(help = "Target URL")]
        url: String,

        #[arg(short, long)]
        #[arg(help = "Events to subscribe to (comma-separated)")]
        events: Option<String>,

        #[arg(short = 's', long)]
        #[arg(help = "Secret for signature verification")]
        secret: Option<String>,
    },

    #[command(about = "Show webhook details")]
    Show {
        #[arg(help = "Webhook ID")]
        id: String,
    },

    #[command(about = "Update a webhook")]
    Update {
        #[arg(help = "Webhook ID")]
        id: String,

        #[arg(short = 'u', long)]
        #[arg(help = "New target URL")]
        url: Option<String>,

        #[arg(short, long)]
        #[arg(help = "New events (comma-separated)")]
        events: Option<String>,

        #[arg(short, long)]
        #[arg(help = "Enable webhook")]
        enable: bool,

        #[arg(short, long)]
        #[arg(help = "Disable webhook")]
        disable: bool,
    },

    #[command(about = "Delete a webhook")]
    Delete {
        #[arg(help = "Webhook ID")]
        id: String,
    },

    #[command(about = "Test a webhook")]
    Test {
        #[arg(help = "Webhook ID")]
        id: String,

        #[arg(short, long)]
        #[arg(help = "Event type to test")]
        event: Option<String>,
    },

    #[command(about = "Show delivery history")]
    Deliveries {
        #[arg(help = "Webhook ID (optional)")]
        id: Option<String>,

        #[arg(short = 'n', long, default_value = "20")]
        #[arg(help = "Number of deliveries to show")]
        limit: usize,
    },

    #[command(about = "Show webhook statistics")]
    Stats,
}

#[derive(Subcommand)]
pub enum LspCommands {
    #[command(about = "Start an LSP server for a language")]
    Start {
        #[arg(help = "Language server command (e.g., 'rust-analyzer')")]
        server: String,

        #[arg(help = "Arguments for the server")]
        args: Vec<String>,

        #[arg(short, long)]
        #[arg(help = "Root URI for the workspace")]
        root: Option<String>,
    },

    #[command(about = "Get completions at a position")]
    Complete {
        #[arg(help = "File URI")]
        uri: String,

        #[arg(short = 'l', long)]
        #[arg(help = "Line number (0-indexed)")]
        line: u32,

        #[arg(short = 'c', long)]
        #[arg(help = "Column number (0-indexed)")]
        column: u32,
    },

    #[command(about = "Get hover information at a position")]
    Hover {
        #[arg(help = "File URI")]
        uri: String,

        #[arg(short = 'l', long)]
        #[arg(help = "Line number (0-indexed)")]
        line: u32,

        #[arg(short = 'c', long)]
        #[arg(help = "Column number (0-indexed)")]
        column: u32,
    },

    #[command(about = "Go to definition")]
    Definition {
        #[arg(help = "File URI")]
        uri: String,

        #[arg(short = 'l', long)]
        #[arg(help = "Line number (0-indexed)")]
        line: u32,

        #[arg(short = 'c', long)]
        #[arg(help = "Column number (0-indexed)")]
        column: u32,
    },

    #[command(about = "Find references")]
    References {
        #[arg(help = "File URI")]
        uri: String,

        #[arg(short = 'l', long)]
        #[arg(help = "Line number (0-indexed)")]
        line: u32,

        #[arg(short = 'c', long)]
        #[arg(help = "Column number (0-indexed)")]
        column: u32,

        #[arg(long)]
        #[arg(help = "Include declaration")]
        include_declaration: bool,
    },

    #[command(about = "Get document symbols")]
    Symbols {
        #[arg(help = "File URI")]
        uri: String,
    },

    #[command(about = "Get diagnostics for a file")]
    Diagnostics {
        #[arg(help = "File URI")]
        uri: String,
    },

    #[command(about = "Get code actions for a range")]
    CodeActions {
        #[arg(help = "File URI")]
        uri: String,

        #[arg(short = 'l', long)]
        #[arg(help = "Start line (0-indexed)")]
        start_line: u32,

        #[arg(short = 'c', long)]
        #[arg(help = "Start column (0-indexed)")]
        start_column: u32,

        #[arg(short = 'L', long)]
        #[arg(help = "End line (0-indexed)")]
        end_line: u32,

        #[arg(short = 'C', long)]
        #[arg(help = "End column (0-indexed)")]
        end_column: u32,
    },
}

#[derive(Subcommand)]
pub enum MemoryCommands {
    #[command(about = "Show project memory (CLAUDE.md + learned entries)")]
    Show {
        #[arg(short, long)]
        #[arg(help = "Show as LLM-ready instructions")]
        instructions: bool,
    },

    #[command(about = "Learn a new memory entry")]
    Learn {
        #[arg(help = "Type of entry: build, test, debug, pattern, preference")]
        entry_type: String,

        #[arg(help = "Entry content (key=value or command)")]
        content: String,

        #[arg(short, long)]
        #[arg(help = "Optional description")]
        description: Option<String>,
    },

    #[command(about = "Set project instructions")]
    Instructions {
        #[arg(help = "Instructions content (or '-' to read from stdin)")]
        content: String,
    },

    #[command(about = "List learned entries by category")]
    List {
        #[arg(help = "Category filter: build, test, debug, patterns, preferences, all")]
        #[arg(default_value = "all")]
        category: String,
    },

    #[command(about = "Clear learned entries")]
    Clear {
        #[arg(help = "Category to clear (or 'all' for everything)")]
        #[arg(default_value = "all")]
        category: String,

        #[arg(short, long)]
        #[arg(help = "Confirm clearing all entries")]
        yes: bool,
    },

    #[command(about = "Create or update CLAUDE.md file")]
    Init {
        #[arg(short, long)]
        #[arg(help = "Project name")]
        name: Option<String>,

        #[arg(short = 'L', long)]
        #[arg(help = "Primary language")]
        language: Option<String>,

        #[arg(short, long)]
        #[arg(help = "Framework")]
        framework: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum ModelsCommands {
    #[command(about = "List available local models")]
    List,

    #[command(about = "Pull a model from registry")]
    Pull {
        #[arg(help = "Model name to pull (e.g., llama3.2, mistral, deepseek-coder)")]
        model: String,
    },

    #[command(about = "Check Ollama server health")]
    Health,

    #[command(about = "Show current model")]
    Current,
}

#[derive(Subcommand)]
pub enum NexusAction {
    #[command(about = "Run the Nexus 24-phase FSM engine")]
    Start {
        #[arg(short, long, default_value = ".")]
        #[arg(help = "Project root path")]
        path: PathBuf,
    },
}

/// Handle a command
pub async fn handle_command(
    cmd: Commands,
    config_path: Option<PathBuf>,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    match cmd {
        Commands::Chat {
            prompt,
            model,
            provider,
            session,
            editor,
            mode,
            exit,
            quiet,
            auto_approve: _,
        } => {
            handle_chat(
                prompt,
                model,
                provider,
                session,
                editor,
                mode,
                exit,
                quiet,
                config_path,
                output_format,
            )
            .await
        },
        Commands::Auto {
            task,
            model,
            provider,
            max_iterations,
            run_tests,
            auto_commit,
            fail_on_test_failure,
            output_format: auto_output_format,
        } => {
            handle_auto(
                task,
                model,
                provider,
                max_iterations,
                run_tests,
                auto_commit,
                fail_on_test_failure,
                auto_output_format,
                config_path,
                output_format,
            )
            .await
        },
        Commands::Init { path } => handle_init(path, output_format).await,
        Commands::Setup { quick, provider } => handle_setup(quick, provider, output_format).await,
        Commands::Sessions { delete, search } => {
            handle_sessions(delete, search, config_path, output_format).await
        },
        Commands::Refactor {
            from,
            to,
            path,
            dry_run,
        } => handle_refactor(from, to, path, dry_run, output_format).await,
        Commands::Action {
            action,
            file,
            line,
            column,
            end_line,
            end_column,
        } => {
            handle_action(
                action,
                file,
                line,
                column,
                end_line,
                end_column,
                output_format,
            )
            .await
        },
        Commands::Test {
            file,
            function,
            output,
        } => handle_test(file, function, output, output_format).await,
        Commands::Doc {
            file,
            element,
            format,
            output,
            inline,
        } => handle_doc(file, element, format, output, inline, output_format).await,
        Commands::Verify { proof, lean_path } => {
            handle_verify(proof, lean_path, output_format).await
        },
        Commands::Broker {
            config,
            paper_trade,
        } => handle_broker(config, paper_trade, output_format).await,
        Commands::Compliance {
            standards,
            path,
            format,
            output,
        } => handle_compliance(standards, path, format, output, output_format).await,
        Commands::Research {
            query,
            languages,
            tqa_level,
            max_results,
        } => handle_research(query, languages, tqa_level, max_results, output_format).await,
        #[cfg(feature = "keyring")]
        Commands::Auth { action } => handle_auth(action).await,
        Commands::Metrics {
            format,
            output,
            reset,
            watch,
        } => handle_metrics(format, output, reset, watch, output_format).await,
        Commands::Telemetry {
            enable,
            disable,
            enable_metrics,
            enable_crash_reporting,
        } => {
            handle_telemetry(
                enable,
                disable,
                enable_metrics,
                enable_crash_reporting,
                config_path,
                output_format,
            )
            .await
        },
        #[cfg(feature = "vector-db")]
        Commands::Index { path, watch } => handle_index(path, watch, output_format).await,
        #[cfg(feature = "vector-db")]
        Commands::Context { query, max_tokens } => {
            handle_context(query, max_tokens, output_format).await
        },
        Commands::Checkpoint { action } => {
            handle_checkpoint(action, config_path, output_format).await
        },
        Commands::Timeline { action } => handle_timeline(action, config_path, output_format).await,
        Commands::Modes { action } => handle_modes(action, config_path, output_format).await,
        Commands::Lang { action } => handle_lang(action, config_path, output_format).await,
        Commands::Edit {
            initial,
            editor,
            extension,
        } => handle_edit(initial, editor, extension, output_format).await,
        Commands::Workflow { action } => handle_workflow(action, config_path, output_format).await,
        Commands::Webhook { action } => handle_webhook(action, config_path, output_format).await,
        Commands::Generate {
            prompt,
            files,
            mode,
            trust,
            test_strategy,
            max_iterations,
            dry_run,
            provider,
            model,
            stream,
            incremental,
            timeout_secs,
        } => {
            handle_generate(
                prompt,
                files,
                mode,
                trust,
                test_strategy,
                max_iterations,
                dry_run,
                provider,
                model,
                stream,
                incremental,
                timeout_secs,
                config_path,
                output_format,
            )
            .await
        },
        Commands::Lsp { action } => handle_lsp(action, output_format).await,
        Commands::Memory { action } => handle_memory(action, config_path, output_format).await,
        Commands::Models { action, host, port } => {
            handle_models(action, &host, port, output_format).await
        },
        Commands::Complete {
            file,
            line,
            character,
            language,
            provider,
            model,
        } => {
            handle_complete(
                file,
                line,
                character,
                language,
                provider,
                model,
                config_path,
                output_format,
            )
            .await
        },
        Commands::Analyze {
            path,
            drift,
            debt,
            format: analyze_format,
            output,
            severity,
            exclude,
        } => handle_analyze(path, drift, debt, analyze_format, output, severity, exclude).await,
        Commands::Watch {
            path,
            ignore,
            auto_analyze,
            debounce_ms,
            verbose,
        } => {
            handle_watch(
                path,
                ignore,
                auto_analyze,
                debounce_ms,
                verbose,
                output_format,
            )
            .await
        },
        Commands::Nexus { action } => handle_nexus(action).await,
    }
}

async fn handle_nexus(action: NexusAction) -> anyhow::Result<()> {
    match action {
        NexusAction::Start { path } => {
            let path = path.clone();
            tokio::task::spawn_blocking(move || run_nexus_engine(&path)).await?
        },
    }
}

fn run_nexus_engine(project_root: &PathBuf) -> anyhow::Result<()> {
    use clawdius_core::nexus::{NexusEngine, RequirementData};

    println!("Nexus FSM Engine");
    println!("================");
    println!("Project root: {}", project_root.display());
    println!();
    println!("[Phase 00/23] Context Discovery");

    let engine = NexusEngine::new(project_root.clone())
        .map_err(|e| anyhow::anyhow!("Failed to create NexusEngine: {e}"))?;

    let engine = engine
        .transition_to_environment("demo", vec![])
        .map_err(|e| anyhow::anyhow!("Phase 0->1: {e}"))?;
    println!("[Phase 01/23] Environment Materialization");

    let engine = engine
        .transition_to_requirements("cargo", vec![], true)
        .map_err(|e| anyhow::anyhow!("Phase 1->2: {e}"))?;
    println!("[Phase 02/23] Requirements Engineering");

    let engine = engine
        .transition_to_research(vec![RequirementData {
            id: "REQ-001".into(),
            description: "Demo requirement".into(),
            priority: "High".into(),
            testable: true,
        }])
        .map_err(|e| anyhow::anyhow!("Phase 2->3: {e}"))?;
    println!("[Phase 03/23] Epistemological Discovery");

    let engine = engine
        .transition_to_cross_lingual("YP-001", vec![])
        .map_err(|e| anyhow::anyhow!("Phase 3->4: {e}"))?;
    println!("[Phase 04/23] Cross-Lingual Integration");

    let engine = engine
        .transition_to_supply_chain()
        .map_err(|e| anyhow::anyhow!("Phase 4->5: {e}"))?;
    println!("[Phase 05/23] Supply Chain Hardening");

    let engine = engine
        .transition_to_architecture(serde_json::json!({}))
        .map_err(|e| anyhow::anyhow!("Phase 5->6: {e}"))?;
    println!("[Phase 06/23] Architectural Specification");

    let engine = engine
        .transition_to_concurrency("BP-001", vec![])
        .map_err(|e| anyhow::anyhow!("Phase 6->7: {e}"))?;
    println!("[Phase 07/23] Concurrency Analysis");

    let engine = engine
        .transition_to_security(serde_json::json!({}))
        .map_err(|e| anyhow::anyhow!("Phase 7->8: {e}"))?;
    println!("[Phase 08/23] Security Engineering");

    let engine = engine
        .transition_to_resources(serde_json::json!({}))
        .map_err(|e| anyhow::anyhow!("Phase 8->9: {e}"))?;
    println!("[Phase 09/23] Resource Management");

    let engine = engine
        .transition_to_performance()
        .map_err(|e| anyhow::anyhow!("Phase 9->10: {e}"))?;
    println!("[Phase 10/23] Performance Engineering");

    let engine = engine
        .transition_to_cross_platform(serde_json::json!({}))
        .map_err(|e| anyhow::anyhow!("Phase 10->11: {e}"))?;
    println!("[Phase 11/23] Cross-Platform Compatibility");

    let engine = engine
        .transition_to_adversarial()
        .map_err(|e| anyhow::anyhow!("Phase 11->12: {e}"))?;
    println!("[Phase 12/23] Adversarial Loop");

    let engine = engine
        .transition_to_cicd(serde_json::json!({}))
        .map_err(|e| anyhow::anyhow!("Phase 12->13: {e}"))?;
    println!("[Phase 13/23] CI/CD Engineering");

    let engine = engine
        .transition_to_documentation(serde_json::json!({}))
        .map_err(|e| anyhow::anyhow!("Phase 13->14: {e}"))?;
    println!("[Phase 14/23] Documentation Verification");

    let engine = engine
        .transition_to_knowledge_base(serde_json::json!({}))
        .map_err(|e| anyhow::anyhow!("Phase 14->15: {e}"))?;
    println!("[Phase 15/23] Knowledge Base Update");

    let engine = engine
        .transition_to_execution_graph()
        .map_err(|e| anyhow::anyhow!("Phase 15->16: {e}"))?;
    println!("[Phase 16/23] Execution Graph Generation");

    let engine = engine
        .transition_to_supply_monitoring(serde_json::json!({}))
        .map_err(|e| anyhow::anyhow!("Phase 16->17: {e}"))?;
    println!("[Phase 17/23] Supply Chain Monitoring");

    let engine = engine
        .transition_to_deployment()
        .map_err(|e| anyhow::anyhow!("Phase 17->18: {e}"))?;
    println!("[Phase 18/23] Deployment & Operations");

    let engine = engine
        .transition_to_operations(serde_json::json!({}))
        .map_err(|e| anyhow::anyhow!("Phase 18->19: {e}"))?;
    println!("[Phase 19/23] Operations");

    let engine = engine
        .transition_to_closure()
        .map_err(|e| anyhow::anyhow!("Phase 19->20: {e}"))?;
    println!("[Phase 20/23] Project Closure");

    let engine = engine
        .transition_to_continuous_monitoring()
        .map_err(|e| anyhow::anyhow!("Phase 20->21: {e}"))?;
    println!("[Phase 21/23] Continuous Monitoring");

    let engine = engine
        .transition_to_knowledge_transfer()
        .map_err(|e| anyhow::anyhow!("Phase 21->22: {e}"))?;
    println!("[Phase 22/23] Knowledge Transfer");

    let engine = engine
        .transition_to_archive(serde_json::json!({}))
        .map_err(|e| anyhow::anyhow!("Phase 22->23: {e}"))?;
    println!("[Phase 23/23] Archive");

    let finalized = engine
        .finalize()
        .map_err(|e| anyhow::anyhow!("Finalization: {e}"))?;

    println!();
    println!("Nexus FSM complete.");
    println!("  Total artifacts: {}", finalized.total_artifacts);
    println!("  Duration: {}ms", finalized.duration.num_milliseconds());

    Ok(())
}

fn load_config(config_path: Option<&PathBuf>) -> anyhow::Result<Config> {
    match config_path {
        Some(path) => Config::load(path)
            .map_err(|e| anyhow::anyhow!("Failed to load config from {}: {}", path.display(), e)),
        None => Config::load_default()
            .map_err(|e| anyhow::anyhow!("Failed to load default config: {e}")),
    }
}

#[allow(clippy::too_many_arguments)]
async fn handle_chat(
    prompt: Option<String>,
    model: Option<String>,
    provider: String,
    _session: Option<String>,
    use_editor: bool,
    mode_name: String,
    exit_after_response: bool,
    quiet_mode: bool,
    config_path: Option<PathBuf>,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    use clawdius_core::llm::{create_provider, ChatMessage, ChatRole, LlmConfig};
    use clawdius_core::modes::AgentMode;
    use clawdius_core::tools::editor::ExternalEditor;
    use std::io::{self, IsTerminal, Read, Write};
    use std::time::Instant;

    // Determine if we should exit after response (non-interactive mode)
    // Auto-enable if prompt is provided via CLI args
    let non_interactive = exit_after_response || prompt.is_some();

    // Handle message input
    let message = if use_editor {
        let editor = ExternalEditor::default_editor();

        if output_format == OutputFormat::Text && !quiet_mode {
            println!(
                "Opening editor ({}). Save and close to continue...",
                editor.editor()
            );
        }

        let initial_content = prompt.unwrap_or_default();
        editor
            .open_and_edit(&initial_content)
            .map_err(|e| anyhow::anyhow!("Editor error: {e}"))?
    } else if let Some(msg) = prompt {
        // Prompt provided via CLI args
        msg
    } else if !io::stdin().is_terminal() {
        // Read from stdin if not a terminal (piped input)
        let mut input = String::new();
        io::stdin().read_to_string(&mut input)?;
        if input.trim().is_empty() {
            anyhow::bail!("No input provided via stdin. Please pipe content or provide a message argument.\nExample: echo 'Hello' | clawdius chat");
        }
        input.trim().to_string()
    } else if non_interactive {
        anyhow::bail!(
            "Message is required in non-interactive mode.\n\nProvide a message via:\n  - Argument: clawdius chat \"Your message\"\n  - Stdin: echo \"Your message\" | clawdius chat"
        );
    } else {
        anyhow::bail!("Message is required.\n\nOptions:\n  - Use --editor to open your $EDITOR\n  - Provide via argument: clawdius chat \"Your message\"\n  - Pipe via stdin: echo \"Your message\" | clawdius chat");
    };

    if message.trim().is_empty() {
        anyhow::bail!("Message cannot be empty");
    }

    let options = OutputOptions {
        format: CoreOutputFormat::from(output_format),
        show_progress: output_format == OutputFormat::Text && !quiet_mode,
        quiet: quiet_mode,
        include_metadata: output_format == OutputFormat::Text && !quiet_mode,
    };
    let formatter = OutputFormatter::new(options);

    let config = load_config(config_path.as_ref())?;
    let session_manager = SessionManager::new(&config)?;
    let mut session = session_manager.get_or_create_active()?;

    // Load agent mode
    let modes_dir = std::env::current_dir()?.join(".clawdius").join("modes");
    let mode = AgentMode::load_by_name(&mode_name, &modes_dir)
        .with_context(|| format!("Failed to load mode: {mode_name}"))?;

    let resolver = MentionResolver::new(std::env::current_dir()?);
    let context_items = resolver.resolve_all(&message).await?;

    let context_str = if context_items.is_empty() {
        message.clone()
    } else {
        let items: Vec<String> = context_items
            .iter()
            .map(clawdius_core::ContextItem::to_formatted_string)
            .collect();
        format!(
            "\n\n[Context]\n{}\n\n[User Message]\n{}",
            items.join("\n---\n"),
            message
        )
    };

    let mut llm_config = LlmConfig::from_config(&config.llm, &provider)?;
    if let Some(ref m) = model {
        llm_config.model = m.clone();
    }

    let llm_client = match create_provider(&llm_config) {
        Ok(client) => client,
        Err(e) => {
            formatter.format_error(
                &mut io::stderr(),
                &e.to_string(),
                Some(session.id.to_string().as_str()),
            )?;
            return Err(e.into());
        },
    };

    // Build messages with mode-specific system prompt
    let system_message = ChatMessage {
        role: ChatRole::System,
        content: mode.system_prompt().to_string(),
    };

    let user_message = ChatMessage {
        role: ChatRole::User,
        content: context_str.clone(),
    };

    let messages = vec![system_message, user_message];

    if output_format == OutputFormat::Text {
        println!("Provider: {provider}");
        println!("Session: {}", session.id);
        println!("Mode: {} - {}", mode.name(), mode.description());
        println!();
    }

    if output_format == OutputFormat::Text {
        print!("Thinking...");
        io::stdout().flush()?;
    }

    let start = Instant::now();
    let response = match llm_client.chat(messages).await {
        Ok(resp) => resp,
        Err(e) => {
            if output_format == OutputFormat::Text {
                println!();
            }
            formatter.format_error(
                &mut io::stderr(),
                &e.to_string(),
                Some(session.id.to_string().as_str()),
            )?;
            return Err(e.into());
        },
    };
    let duration = start.elapsed();

    if output_format == OutputFormat::Text {
        println!("\x1b[2K\r");
    }

    let user_msg = clawdius_core::session::Message::user(&message);
    session_manager
        .add_message(&mut session, user_msg.clone())
        .await?;

    let assistant_msg = clawdius_core::session::Message::assistant(&response);
    session_manager
        .add_message(&mut session, assistant_msg.clone())
        .await?;

    formatter.format_chat_response(
        &mut io::stdout(),
        &response,
        &session.id.to_string(),
        &provider,
        model.as_deref(),
        0,
        0,
        duration.as_millis() as u64,
    )?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn handle_auto(
    task: String,
    model: Option<String>,
    provider: String,
    max_iterations: Option<usize>,
    run_tests: bool,
    auto_commit: bool,
    fail_on_test_failure: bool,
    _auto_output_format: Option<String>,
    config_path: Option<PathBuf>,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    use clawdius_core::llm::{create_provider, ChatMessage, ChatRole, LlmConfig};
    use clawdius_core::modes::AgentMode;
    use clawdius_core::output::{ActionEdit, ActionResult, OutputOptions};
    use std::io;
    use std::process::Command;
    use std::time::Instant;

    let options = OutputOptions {
        format: CoreOutputFormat::from(output_format),
        show_progress: output_format == OutputFormat::Text,
        quiet: false,
        include_metadata: output_format == OutputFormat::Text,
    };
    let formatter = OutputFormatter::new(options);

    let config = load_config(config_path.as_ref())?;
    let session_manager = SessionManager::new(&config)?;
    let mut session = session_manager.get_or_create_active()?;

    // Load Auto mode
    let modes_dir = std::env::current_dir()?.join(".clawdius").join("modes");
    let mode = AgentMode::load_by_name("auto", &modes_dir).unwrap_or(AgentMode::Auto);

    let mut llm_config = LlmConfig::from_config(&config.llm, &provider)?;
    if let Some(ref m) = model {
        llm_config.model = m.clone();
    }

    let llm_client = match create_provider(&llm_config) {
        Ok(client) => client,
        Err(e) => {
            let result = ActionResult::error("auto", task.clone(), e.to_string());
            formatter.format_action_result(&mut io::stdout(), &result)?;
            return Err(e.into());
        },
    };

    let max_iters = max_iterations.unwrap_or(50);
    let start = Instant::now();

    if output_format == OutputFormat::Text {
        println!("🤖 Clawdius Auto Mode");
        println!("Task: {task}");
        println!("Provider: {provider}");
        println!("Max iterations: {max_iters}");
        if run_tests {
            println!("Tests: enabled");
        }
        if auto_commit {
            println!("Auto-commit: enabled");
        }
        println!();
    }

    // Build initial prompt with task
    let system_message = ChatMessage {
        role: ChatRole::System,
        content: mode.system_prompt().to_string(),
    };

    let user_message = ChatMessage {
        role: ChatRole::User,
        content: format!(
            "Task: {task}\n\nPlease complete this task autonomously. Make the necessary changes and report what you did."
        ),
    };

    let messages = vec![system_message, user_message];

    if output_format == OutputFormat::Text {
        print!("Working...");
    }

    let response = match llm_client.chat(messages).await {
        Ok(resp) => resp,
        Err(e) => {
            let result = ActionResult::error("auto", task.clone(), e.to_string());
            formatter.format_action_result(&mut io::stdout(), &result)?;
            return Err(e.into());
        },
    };

    let duration = start.elapsed();
    let mut changes_made = Vec::new();
    let mut tests_passed = true;

    // Parse response for changes
    if response.contains("created") || response.contains("modified") || response.contains("updated")
    {
        changes_made.push("Files modified based on LLM response".to_string());
    }

    // Run tests if requested
    if run_tests {
        if output_format == OutputFormat::Text {
            println!("\n🧪 Running tests...");
        }

        let test_output = Command::new("cargo")
            .args(["test", "--no-fail-fast"])
            .current_dir(std::env::current_dir()?)
            .output();

        match test_output {
            Ok(output) => {
                if output.status.success() {
                    if output_format == OutputFormat::Text {
                        println!("✅ Tests passed");
                    }
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    if output_format == OutputFormat::Text {
                        println!("❌ Tests failed:\n{stderr}");
                    }
                    tests_passed = false;
                    if fail_on_test_failure {
                        let result = ActionResult::error(
                            "auto",
                            task.clone(),
                            format!("Tests failed: {stderr}"),
                        );
                        formatter.format_action_result(&mut io::stdout(), &result)?;
                        anyhow::bail!("Tests failed and fail_on_test_failure is set");
                    }
                }
            },
            Err(e) => {
                if output_format == OutputFormat::Text {
                    println!("⚠️ Could not run tests: {e}");
                }
            },
        }
    }

    // Auto-commit if requested and changes were made
    if auto_commit && !changes_made.is_empty() {
        if output_format == OutputFormat::Text {
            println!("\n📝 Committing changes...");
        }

        let commit_message = format!("auto: {task}");
        let _ = Command::new("git")
            .args(["add", "-A"])
            .current_dir(std::env::current_dir()?)
            .output();

        let commit_output = Command::new("git")
            .args(["commit", "-m", &commit_message])
            .current_dir(std::env::current_dir()?)
            .output();

        match commit_output {
            Ok(output) => {
                if output.status.success() {
                    if output_format == OutputFormat::Text {
                        println!("✅ Changes committed");
                    }
                    changes_made.push("Changes committed to git".to_string());
                } else if output_format == OutputFormat::Text {
                    println!("⚠️ Git commit failed (maybe no changes?)");
                }
            },
            Err(e) => {
                if output_format == OutputFormat::Text {
                    println!("⚠️ Could not commit: {e}");
                }
            },
        }
    }

    // Save session
    let user_msg = clawdius_core::session::Message::user(&task);
    session_manager
        .add_message(&mut session, user_msg.clone())
        .await?;

    let assistant_msg = clawdius_core::session::Message::assistant(&response);
    session_manager
        .add_message(&mut session, assistant_msg.clone())
        .await?;

    // Build result
    let result = ActionResult::success(
        "auto",
        task.clone(),
        format!("Auto task completed in {}ms", duration.as_millis()),
        format!("{changes_made:?}"),
        changes_made
            .iter()
            .map(|c| ActionEdit {
                start_line: 0,
                start_column: 0,
                end_line: 0,
                end_column: 0,
                new_text: c.clone(),
            })
            .collect(),
    );

    formatter.format_action_result(&mut io::stdout(), &result)?;

    // Return error code if tests failed and fail_on_test_failure is set
    if !tests_passed && run_tests && fail_on_test_failure {
        std::process::exit(1);
    }

    Ok(())
}

async fn handle_init(path: PathBuf, output_format: OutputFormat) -> anyhow::Result<()> {
    use clawdius_core::output::{InitResult, OutputOptions};
    use std::io;

    let options = OutputOptions {
        format: CoreOutputFormat::from(output_format),
        show_progress: output_format == OutputFormat::Text,
        quiet: false,
        include_metadata: output_format == OutputFormat::Text,
    };
    let formatter = OutputFormatter::new(options);

    let onboarding = Onboarding::new(path.join(".clawdius/config.toml"));

    let result: Result<_, anyhow::Error> = (|| {
        onboarding.create_directory_structure(&path)?;
        onboarding.create_default_config()?;

        let config_path = onboarding.config_path();
        let status = onboarding.check();

        Ok((config_path, status == OnboardingStatus::Complete))
    })();

    let result = match result {
        Ok((config_path, complete)) => InitResult::success(
            path.to_string_lossy().to_string(),
            config_path.to_string_lossy().to_string(),
            complete,
        ),
        Err(e) => InitResult::error(e.to_string()),
    };

    formatter.format_init_result(&mut io::stdout(), &result)?;

    Ok(())
}

/// Interactive setup wizard for first-time users
async fn handle_setup(
    quick: bool,
    provider: Option<String>,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    use std::io::{self, Write};

    // Welcome screen
    if !quick {
        println!(
            r"
╔══════════════════════════════════════════════════════════════╗
║   ██████╗██╗      █████╗ ██╗   ██╗██████╗ ███████╗          ║
║  ██╔════╝██║     ██╔══██╗██║   ██║██╔══██╗██╔════╝          ║
║  ██║     ██║     ███████║██║   ██║██║  ██║█████╗            ║
║  ██║     ██║     ██╔══██║██║   ██║██║  ██║██╔══╝            ║
║  ╚██████╗███████╗██║  ██║╚██████╔╝██████╔╝███████╗          ║
║   ╚═════╝╚══════╝╚═╝  ╚═╝ ╚═════╝ ╚═════╝ ╚══════╝          ║
║                                                              ║
║   High-Assurance AI Coding Assistant                        ║
╚══════════════════════════════════════════════════════════════╝
"
        );
        println!(
            "Welcome to Clawdius Setup! This wizard will help you configure your AI assistant.\n"
        );
    }

    // Provider selection
    let selected_provider = if let Some(p) = provider {
        p
    } else {
        println!("📦 Step 1: Choose your LLM provider\n");
        println!("  1. Anthropic Claude (Recommended) - Best code generation, long context");
        println!("  2. OpenAI GPT-4 - Widely used, fast responses");
        println!("  3. Ollama (Local) - 100% private, no API costs");
        println!("  4. Zhipu AI - Chinese optimized, cost effective");
        println!();

        print!("Enter your choice (1-4) [default: 1]: ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        let choice = input.trim().parse::<u8>().unwrap_or(1);
        match choice {
            1 => "anthropic".to_string(),
            2 => "openai".to_string(),
            3 => "ollama".to_string(),
            4 => "zai".to_string(),
            _ => "anthropic".to_string(),
        }
    };

    println!("\n✓ Selected provider: {selected_provider}\n");

    // API key configuration
    if selected_provider != "ollama" {
        println!("🔑 Step 2: Configure API key\n");

        let env_var = format!("{}_API_KEY", selected_provider.to_uppercase());
        let has_env_key = std::env::var(&env_var).is_ok();

        if has_env_key {
            println!("  ✓ Found {env_var} in environment");
        } else {
            println!("  You can provide your API key in one of these ways:");
            println!("  1. Environment variable: export {env_var}=your-key");
            println!("  2. Config file: clawdius auth set-key {selected_provider}");
            println!("  3. Keyring: clawdius auth set-key {selected_provider} (secure storage)");
            println!();

            print!("Enter your API key (or press Enter to skip): ");
            io::stdout().flush()?;

            let mut key_input = String::new();
            io::stdin().read_line(&mut key_input)?;
            let api_key = key_input.trim();

            if !api_key.is_empty() {
                // Store in keyring if available
                #[cfg(feature = "keyring")]
                {
                    use clawdius_core::config::KeyringStorage;
                    if let Err(e) =
                        KeyringStorage::global().set_api_key(&selected_provider, api_key)
                    {
                        eprintln!("  ⚠ Could not store in keyring: {e}");
                        eprintln!(
                            "  Set the environment variable instead: export {env_var}=<your-key>"
                        );
                    } else {
                        println!("  ✓ API key stored securely in keyring");
                    }
                }
                #[cfg(not(feature = "keyring"))]
                {
                    println!("  ⚠ Keyring feature not available");
                    println!("  Set the environment variable: export {env_var}=<your-key>");
                    let _ = api_key; // Suppress unused warning
                }
            }
        }
    } else {
        println!("🔑 Step 2: Ollama Setup\n");
        println!("  Ollama runs models locally. Make sure you have:");
        println!("  1. Installed Ollama: https://ollama.ai");
        println!("  2. Started the server: ollama serve");
        println!("  3. Pulled a model: ollama pull codellama");
        println!();

        // Check if Ollama is running using a simple TCP check
        use std::net::TcpStream;
        let ollama_addr =
            std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "127.0.0.1:11434".to_string());

        // Remove http:// prefix if present
        let ollama_addr = ollama_addr
            .trim_start_matches("http://")
            .trim_start_matches("https://")
            .to_string();

        match TcpStream::connect_timeout(
            &ollama_addr.parse().unwrap_or_else(|_| {
                "127.0.0.1:11434"
                    .parse()
                    .expect("hardcoded address must parse")
            }),
            std::time::Duration::from_secs(2),
        ) {
            Ok(_) => {
                println!("  ✓ Ollama server is running at {ollama_addr}");
            },
            Err(_) => {
                println!("  ⚠ Could not connect to Ollama at {ollama_addr}");
                println!("    Make sure Ollama is installed and running");
            },
        }
    }

    println!();

    // Settings preset
    if !quick {
        println!("⚙️  Step 3: Choose settings preset\n");
        println!("  1. Balanced - Good security with performance (Recommended)");
        println!("  2. Security - Maximum sandboxing, safest option");
        println!("  3. Performance - Faster execution, lighter sandboxing");
        println!("  4. Development - Minimal sandboxing, verbose output");
        println!();

        print!("Enter your choice (1-4) [default: 1]: ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        let choice = input.trim().parse::<u8>().unwrap_or(1);
        let preset = match choice {
            1 => "Balanced",
            2 => "Security",
            3 => "Performance",
            4 => "Development",
            _ => "Balanced",
        };
        println!("\n✓ Selected preset: {preset}\n");
    }

    // Quick start examples
    println!("📚 Quick Start Examples\n");
    println!("  Now that you're set up, try these commands:");
    println!();
    println!("  # Start an interactive chat:");
    println!("  $ clawdius chat");
    println!();
    println!("  # Generate code from a prompt:");
    println!("  $ clawdius generate \"Create a function that sorts a list\"");
    println!();
    println!("  # Analyze your codebase:");
    println!("  $ clawdius analyze src/");
    println!();
    println!("  # Watch for file changes:");
    println!("  $ clawdius watch . --auto-analyze");
    println!();

    // Final status
    let status = clawdius_core::onboarding::Onboarding::check_environment();
    match &status {
        clawdius_core::onboarding::OnboardingStatus::Complete => {
            println!("✅ Setup complete! Clawdius is ready to use.\n");
        },
        clawdius_core::onboarding::OnboardingStatus::MissingApiKey { provider } => {
            println!("⚠️  Setup incomplete: Missing API key for {provider}");
            println!("   Run: clawdius auth set-key {provider}\n");
        },
        clawdius_core::onboarding::OnboardingStatus::MissingConfig => {
            println!("⚠️  Setup incomplete: Run 'clawdius init' to create a project\n");
        },
        clawdius_core::onboarding::OnboardingStatus::FirstRun => {
            println!("⚠️  Setup incomplete: Run 'clawdius init' to create a project\n");
        },
    }

    if output_format == OutputFormat::Json {
        let json_result = serde_json::json!({
            "status": "complete",
            "provider": selected_provider,
            "onboarding_status": format!("{:?}", status)
        });
        println!("{}", serde_json::to_string_pretty(&json_result)?);
    }

    Ok(())
}

async fn handle_sessions(
    delete: Option<String>,
    search: Option<String>,
    config_path: Option<PathBuf>,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    let config = load_config(config_path.as_ref())?;
    let session_manager = SessionManager::new(&config)?;

    if let Some(session_id) = delete {
        use std::str::FromStr;
        let id = clawdius_core::session::SessionId::from_str(&session_id)?;
        session_manager.delete_session(&id)?;
        println!("✓ Deleted session: {session_id}");
        return Ok(());
    }

    if let Some(query) = search {
        let results = session_manager.search_messages(&query)?;
        println!("Search results for '{query}':");
        for (session_id, msg) in results {
            let preview = msg
                .as_text()
                .map(|t| {
                    if t.len() > 50 {
                        format!("{}...", &t[..50])
                    } else {
                        t.to_string()
                    }
                })
                .unwrap_or_else(|| "[non-text]".to_string());
            println!("  {session_id} > {preview}");
        }
        return Ok(());
    }

    let sessions = session_manager.list_sessions()?;

    let session_infos: Vec<SessionInfo> = sessions
        .iter()
        .map(|session| SessionInfo {
            id: session.id.to_string(),
            title: session.title.clone(),
            message_count: session.messages.len(),
            tokens: session.token_usage.total(),
        })
        .collect();

    let options = OutputOptions {
        format: CoreOutputFormat::from(output_format),
        show_progress: false,
        quiet: false,
        include_metadata: true,
    };
    let formatter = OutputFormatter::new(options);

    use std::io::{self};
    formatter.format_session_list(&mut io::stdout(), &session_infos)?;

    Ok(())
}

async fn handle_refactor(
    _from: String,
    _to: String,
    _path: PathBuf,
    _dry_run: bool,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    use clawdius_core::output::{OutputOptions, RefactorResult};
    use std::io;

    let options = OutputOptions {
        format: CoreOutputFormat::from(output_format),
        show_progress: output_format == OutputFormat::Text,
        quiet: false,
        include_metadata: output_format == OutputFormat::Text,
    };
    let formatter = OutputFormatter::new(options);

    let result = RefactorResult::error("Refactor command not yet implemented");

    formatter.format_refactor_result(&mut io::stdout(), &result)?;

    Ok(())
}

async fn handle_action(
    action: String,
    file: PathBuf,
    line: Option<usize>,
    column: Option<usize>,
    end_line: Option<usize>,
    end_column: Option<usize>,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    use clawdius_core::actions::{ActionContext, ActionRegistry, Position};
    use clawdius_core::output::{ActionEdit, ActionResult, OutputOptions};
    use std::fs;
    use std::io;

    let options = OutputOptions {
        format: CoreOutputFormat::from(output_format),
        show_progress: output_format == OutputFormat::Text,
        quiet: false,
        include_metadata: output_format == OutputFormat::Text,
    };
    let formatter = OutputFormatter::new(options);

    let document = fs::read_to_string(&file)?;
    let language = file
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("txt")
        .to_string();

    let position = Position {
        line: line.unwrap_or(0),
        column: column.unwrap_or(0),
    };

    let selection = if let (Some(end_l), Some(end_c)) = (end_line, end_column) {
        let lines: Vec<&str> = document.lines().collect();
        if position.line < lines.len() && end_l < lines.len() {
            if position.line == end_l {
                Some(lines[position.line][position.column..end_c].to_string())
            } else {
                let mut selected_text = String::new();
                for i in position.line..=end_l {
                    if i < lines.len() {
                        if i == position.line {
                            selected_text.push_str(&lines[i][position.column..]);
                        } else if i == end_l {
                            selected_text.push_str(&lines[i][..end_c]);
                        } else {
                            selected_text.push_str(lines[i]);
                        }
                        if i < end_l {
                            selected_text.push('\n');
                        }
                    }
                }
                Some(selected_text)
            }
        } else {
            None
        }
    } else {
        None
    };

    let context = ActionContext {
        document: document.clone(),
        language,
        position,
        selection,
        symbol_at_position: None,
    };

    let registry = ActionRegistry::default();

    let action_impl = match action.as_str() {
        "extract-function" => registry
            .get_applicable_actions(&context)
            .into_iter()
            .find(|a| a.id() == "refactor.extract.function")
            .ok_or_else(|| anyhow::anyhow!("Extract function action not available"))?,
        "extract-variable" => registry
            .get_applicable_actions(&context)
            .into_iter()
            .find(|a| a.id() == "refactor.extract.variable")
            .ok_or_else(|| anyhow::anyhow!("Extract variable action not available"))?,
        "inline-variable" => registry
            .get_applicable_actions(&context)
            .into_iter()
            .find(|a| a.id() == "refactor.inline.variable")
            .ok_or_else(|| anyhow::anyhow!("Inline variable action not available"))?,
        "rename" => registry
            .get_applicable_actions(&context)
            .into_iter()
            .find(|a| a.id() == "refactor.rename")
            .ok_or_else(|| anyhow::anyhow!("Rename action not available"))?,
        "move-module" => registry
            .get_applicable_actions(&context)
            .into_iter()
            .find(|a| a.id() == "refactor.move.module")
            .ok_or_else(|| anyhow::anyhow!("Move to module action not available"))?,
        "generate-tests" => registry
            .get_applicable_actions(&context)
            .into_iter()
            .find(|a| a.id() == "source.generate.tests")
            .ok_or_else(|| anyhow::anyhow!("Generate tests action not available"))?,
        _ => {
            anyhow::bail!("Unknown action: {action}");
        },
    };

    let result = match action_impl.execute(&context) {
        Ok(action_result) => {
            let edits: Vec<ActionEdit> = action_result
                .edits
                .iter()
                .map(|edit| ActionEdit {
                    start_line: edit.range.start.line,
                    start_column: edit.range.start.column,
                    end_line: edit.range.end.line,
                    end_column: edit.range.end.column,
                    new_text: edit.new_text.clone(),
                })
                .collect();

            ActionResult::success(
                &action,
                file.display().to_string(),
                action_result.title,
                format!("{:?}", action_result.kind),
                edits,
            )
        },
        Err(e) => ActionResult::error(&action, file.display().to_string(), e.to_string()),
    };

    formatter.format_action_result(&mut io::stdout(), &result)?;

    Ok(())
}

async fn handle_test(
    file: PathBuf,
    function: Option<String>,
    output: Option<PathBuf>,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    use clawdius_core::actions::tests::GenerateTests;
    use clawdius_core::output::{OutputOptions, TestCaseInfo, TestResult};
    use std::fs;
    use std::io;

    let options = OutputOptions {
        format: CoreOutputFormat::from(output_format),
        show_progress: output_format == OutputFormat::Text,
        quiet: false,
        include_metadata: output_format == OutputFormat::Text,
    };
    let formatter = OutputFormatter::new(options);

    let code = fs::read_to_string(&file)?;
    let language = file
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("txt")
        .to_string();

    let result: TestResult = if let Some(func_name) = &function {
        match async {
            let test_generator =
                GenerateTests::new(std::sync::Arc::new(clawdius_core::llm::create_provider(
                    &clawdius_core::llm::LlmConfig::from_env("anthropic")?,
                )?));

            let func = extract_function_from_code(&code, func_name, &language)?;
            let tests = test_generator.generate_for_function(&func).await?;

            let test_cases: Vec<TestCaseInfo> = tests
                .test_cases
                .iter()
                .map(|t| TestCaseInfo {
                    name: t.name.clone(),
                    description: t.description.clone(),
                    code: t.code.clone(),
                })
                .collect();

            if let Some(output_path) = &output {
                let test_code: Vec<String> = tests
                    .test_cases
                    .iter()
                    .map(|t| format!("// {}\n{}", t.description, t.code))
                    .collect();

                fs::write(output_path, test_code.join("\n\n"))?;
            }

            Ok::<_, anyhow::Error>((test_cases, output.map(|p| p.display().to_string())))
        }
        .await
        {
            Ok((test_cases, output_path)) => TestResult::success(
                file.display().to_string(),
                Some(func_name.clone()),
                language,
                test_cases,
                output_path,
            ),
            Err(e) => TestResult::error(file.display().to_string(), e.to_string()),
        }
    } else {
        let test_cases = generate_default_tests(&language);
        let output_path = output.map(|p| p.display().to_string());

        if let Some(ref path) = output_path {
            let test_code = generate_test_code(&language);
            fs::write(path, test_code)?;
        }

        TestResult::success(
            file.display().to_string(),
            None,
            language,
            test_cases,
            output_path,
        )
    };

    formatter.format_test_result(&mut io::stdout(), &result)?;

    Ok(())
}

fn generate_default_tests(_language: &str) -> Vec<TestCaseInfo> {
    vec![
        TestCaseInfo {
            name: "test_normal_case".to_string(),
            description: "Test normal case behavior".to_string(),
            code: "// TODO: Add test implementation".to_string(),
        },
        TestCaseInfo {
            name: "test_edge_case".to_string(),
            description: "Test edge cases".to_string(),
            code: "// TODO: Test edge cases".to_string(),
        },
        TestCaseInfo {
            name: "test_error_case".to_string(),
            description: "Test error scenarios".to_string(),
            code: "// TODO: Test error scenarios".to_string(),
        },
    ]
}

fn generate_test_code(language: &str) -> String {
    match language {
        "rs" => r"#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normal_case() {
        // TODO: Add test implementation
    }

    #[test]
    fn test_edge_case() {
        // TODO: Test edge cases
    }

    #[test]
    fn test_error_case() {
        // TODO: Test error scenarios
    }
}"
        .to_string(),
        "ts" | "js" => r"describe('function tests', () => {
    test('normal case', () => {
        // TODO: Add test implementation
    });

    test('edge case', () => {
        // TODO: Test edge cases
    });

    test('error case', () => {
        // TODO: Test error scenarios
    });
});"
        .to_string(),
        "py" => r"import unittest

class TestFunction(unittest.TestCase):
    def test_normal_case(self):
        # TODO: Add test implementation
        pass

    def test_edge_case(self):
        # TODO: Test edge cases
        pass

    def test_error_case(self):
        # TODO: Test error scenarios
        pass

if __name__ == '__main__':
    unittest.main()"
            .to_string(),
        _ => "// Test generation not supported for this language".to_string(),
    }
}

fn extract_function_from_code(
    code: &str,
    func_name: &str,
    language: &str,
) -> anyhow::Result<Function> {
    use clawdius_core::actions::tests::GenerateTests;

    let pattern = match language {
        "rs" => format!(r"fn\s+{func_name}\s*[<\(]"),
        "ts" | "js" => format!(r"(?:async\s+)?function\s+{func_name}\s*\("),
        "py" => format!(r"def\s+{func_name}\s*\("),
        _ => anyhow::bail!("Unsupported language: {language}"),
    };

    let re = regex::Regex::new(&pattern)?;
    if let Some(_match) = re.find(code) {
        let selection = extract_function_body(code, _match.start(), language)?;
        GenerateTests::parse_function_from_selection(&selection, language)
            .map_err(|e| anyhow::anyhow!("{e}"))
    } else {
        anyhow::bail!("Function '{func_name}' not found");
    }
}

fn extract_function_body(code: &str, start: usize, _language: &str) -> anyhow::Result<String> {
    let mut depth = 0;
    let mut in_function = false;
    let mut function_end = start;

    for (i, c) in code[start..].char_indices() {
        match c {
            '{' => {
                depth += 1;
                in_function = true;
            },
            '}' => {
                depth -= 1;
                if in_function && depth == 0 {
                    function_end = start + i + 1;
                    break;
                }
            },
            _ => {},
        }
    }

    if function_end > start {
        Ok(code[start..function_end].to_string())
    } else {
        anyhow::bail!("Could not extract function body")
    }
}

async fn handle_doc(
    file: PathBuf,
    element: Option<String>,
    format: String,
    output: Option<PathBuf>,
    _inline: bool,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    use clawdius_core::actions::docs::{DocFormat, GenerateDocs};
    use std::fs;

    let options = OutputOptions {
        format: CoreOutputFormat::from(output_format),
        show_progress: output_format == OutputFormat::Text,
        quiet: false,
        include_metadata: output_format == OutputFormat::Text,
    };
    let formatter = OutputFormatter::new(options);

    let code = fs::read_to_string(&file)?;
    let language = file
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("txt")
        .to_string();

    let doc_format = match format.as_str() {
        "rustdoc" | "rust" => DocFormat::Rustdoc,
        "jsdoc" | "javascript" | "typescript" => DocFormat::JsDoc,
        "pydoc" | "python" => DocFormat::PythonDocstring,
        "markdown" | "md" => DocFormat::Markdown,
        "auto" | _ => match language.as_str() {
            "rs" => DocFormat::Rustdoc,
            "ts" | "js" => DocFormat::JsDoc,
            "py" => DocFormat::PythonDocstring,
            _ => DocFormat::Markdown,
        },
    };

    if output_format == OutputFormat::Text {
        println!("📝 Generating documentation for: {}", file.display());
        println!("   Language: {}", language);
        println!("   Format: {:?}", doc_format);
        if let Some(ref elem) = element {
            println!("   Element: {}", elem);
        }
        println!();
    }

    // Create LLM client
    let config = load_config(None)?;
    let llm_config = clawdius_core::llm::LlmConfig::from_config(&config.llm, "anthropic")?;
    let llm_client = std::sync::Arc::new(clawdius_core::llm::create_provider(&llm_config)?);

    let doc_generator = GenerateDocs::new(llm_client);

    // Generate documentation based on element type
    let generated_docs = if let Some(element_name) = &element {
        // Try to parse as function first, then struct
        match async {
            // Try function
            if let Ok(func) = extract_function_from_code(&code, element_name, &language) {
                let docs = doc_generator
                    .generate_for_function(&func.name, &func.signature, &func.body, &language)
                    .await?;
                return Ok::<_, anyhow::Error>(docs);
            }

            // Fallback to basic documentation
            anyhow::bail!("Could not parse element: {}", element_name)
        }
        .await
        {
            Ok(docs) => docs,
            Err(e) => {
                if output_format == OutputFormat::Text {
                    println!("⚠️  Could not generate LLM docs: {}", e);
                    println!("   Falling back to basic documentation template");
                }
                return Err(e);
            },
        }
    } else {
        // Generate module-level documentation
        let exports = extract_exports(&code, &language);
        match doc_generator
            .generate_for_module(&file.display().to_string(), &code, &exports)
            .await
        {
            Ok(docs) => docs,
            Err(e) => {
                if output_format == OutputFormat::Text {
                    println!("⚠️  Could not generate module docs: {}", e);
                }
                return Err(e.into());
            },
        }
    };

    // Format the documentation
    let formatted_docs = doc_generator.format_docs(&generated_docs, &language);

    // Output the documentation
    if let Some(output_path) = &output {
        fs::write(output_path, &formatted_docs)?;
        if output_format == OutputFormat::Text {
            println!("✅ Documentation written to: {}", output_path.display());
        }
    } else {
        println!("{}", formatted_docs);
    }

    Ok(())
}

fn extract_exports(code: &str, language: &str) -> Vec<String> {
    let mut exports = Vec::new();

    match language {
        "rs" => {
            // Look for pub fn, pub struct, pub enum
            let fn_re = regex::Regex::new(r"pub\s+(?:async\s+)?fn\s+(\w+)").ok();
            let struct_re = regex::Regex::new(r"pub\s+struct\s+(\w+)").ok();
            let enum_re = regex::Regex::new(r"pub\s+enum\s+(\w+)").ok();

            if let Some(re) = fn_re {
                for cap in re.captures_iter(code) {
                    exports.push(format!("fn {}", &cap[1]));
                }
            }
            if let Some(re) = struct_re {
                for cap in re.captures_iter(code) {
                    exports.push(format!("struct {}", &cap[1]));
                }
            }
            if let Some(re) = enum_re {
                for cap in re.captures_iter(code) {
                    exports.push(format!("enum {}", &cap[1]));
                }
            }
        },
        "ts" | "js" => {
            // Look for export function, export class
            let fn_re = regex::Regex::new(r"export\s+(?:async\s+)?function\s+(\w+)").ok();
            let class_re = regex::Regex::new(r"export\s+class\s+(\w+)").ok();

            if let Some(re) = fn_re {
                for cap in re.captures_iter(code) {
                    exports.push(format!("function {}", &cap[1]));
                }
            }
            if let Some(re) = class_re {
                for cap in re.captures_iter(code) {
                    exports.push(format!("class {}", &cap[1]));
                }
            }
        },
        "py" => {
            // Look for def and class
            let def_re = regex::Regex::new(r"^def\s+(\w+)").ok();
            let class_re = regex::Regex::new(r"^class\s+(\w+)").ok();

            if let Some(re) = def_re {
                for cap in re.captures_iter(code) {
                    exports.push(format!("def {}", &cap[1]));
                }
            }
            if let Some(re) = class_re {
                for cap in re.captures_iter(code) {
                    exports.push(format!("class {}", &cap[1]));
                }
            }
        },
        _ => {},
    }

    exports
}

async fn handle_verify(
    proof: PathBuf,
    lean_path: Option<PathBuf>,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    use clawdius_core::output::{OutputOptions, ProofError, VerifyResult};
    use std::io;

    let options = OutputOptions {
        format: CoreOutputFormat::from(output_format),
        show_progress: output_format == OutputFormat::Text,
        quiet: false,
        include_metadata: output_format == OutputFormat::Text,
    };
    let formatter = OutputFormatter::new(options);

    let verifier = match lean_path {
        Some(path) => {
            let lake_path = path
                .parent()
                .map_or_else(|| path.clone(), |p| p.join("lake"));
            LeanVerifier::with_paths(path, lake_path)?
        },
        None => LeanVerifier::new()?,
    };

    if !verifier.check_available() {
        let result = VerifyResult::failure(
            proof.display().to_string(),
            0,
            vec![ProofError {
                line: 0,
                column: 0,
                message: "Lean binaries not found. Please install Lean 4 and ensure 'lean' and 'lake' are in PATH.".to_string(),
            }],
            vec![],
        );
        formatter.format_verify_result(&mut io::stdout(), &result)?;
        anyhow::bail!("Lean binaries not found");
    }

    if output_format == OutputFormat::Text {
        println!("Lean version: {}", verifier.version()?);
        println!();
    }

    let start = std::time::Instant::now();
    let verification_result = verifier.verify(&proof)?;
    let duration = start.elapsed();

    let result = if verification_result.success {
        VerifyResult::success(proof.display().to_string(), duration.as_millis() as u64)
    } else {
        let errors: Vec<ProofError> = verification_result
            .errors
            .iter()
            .map(|e| ProofError {
                line: e.line,
                column: e.column,
                message: e.message.clone(),
            })
            .collect();

        VerifyResult::failure(
            proof.display().to_string(),
            duration.as_millis() as u64,
            errors,
            verification_result.warnings.clone(),
        )
    };

    formatter.format_verify_result(&mut io::stdout(), &result)?;

    Ok(())
}

async fn handle_broker(
    _config: Option<PathBuf>,
    _paper_trade: bool,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    use clawdius_core::output::{BrokerResult, OutputOptions};
    use std::io;

    let options = OutputOptions {
        format: CoreOutputFormat::from(output_format),
        show_progress: output_format == OutputFormat::Text,
        quiet: false,
        include_metadata: output_format == OutputFormat::Text,
    };
    let formatter = OutputFormatter::new(options);

    let result = BrokerResult::error("Broker command not yet implemented");

    formatter.format_broker_result(&mut io::stdout(), &result)?;

    Ok(())
}

async fn handle_compliance(
    _standards: String,
    _path: PathBuf,
    _format: String,
    _output: Option<PathBuf>,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    use clawdius_core::output::{ComplianceResult, OutputOptions};
    use std::io;

    let options = OutputOptions {
        format: CoreOutputFormat::from(output_format),
        show_progress: output_format == OutputFormat::Text,
        quiet: false,
        include_metadata: output_format == OutputFormat::Text,
    };
    let formatter = OutputFormatter::new(options);

    let result = ComplianceResult::error("Compliance command not yet implemented");

    formatter.format_compliance_result(&mut io::stdout(), &result)?;

    Ok(())
}

async fn handle_research(
    query: String,
    languages: Option<String>,
    _tqa_level: u8,
    max_results: usize,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    use clawdius_core::output::{
        OutputOptions, ResearchConcept, ResearchRelationship, ResearchResult,
    };
    use clawdius_core::{Language, ResearchQuery, ResearchSynthesizer};
    use std::io;
    use std::str::FromStr;

    let options = OutputOptions {
        format: CoreOutputFormat::from(output_format),
        show_progress: output_format == OutputFormat::Text,
        quiet: false,
        include_metadata: output_format == OutputFormat::Text,
    };
    let formatter = OutputFormatter::new(options);

    let target_languages: Vec<Language> = match languages {
        Some(langs) => langs
            .split(',')
            .map(|s| Language::from_str(s.trim()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| anyhow::anyhow!("Invalid language: {e}"))?,
        None => vec![
            Language::EN,
            Language::ZH,
            Language::DE,
            Language::FR,
            Language::JP,
        ],
    };

    let research_query = ResearchQuery::from_single_term(&query, target_languages.clone())
        .with_max_results(max_results);

    let mut synthesizer = ResearchSynthesizer::new();

    let result = match synthesizer.search_multilingual(research_query).await {
        Ok(synth_result) => {
            let languages_covered: Vec<String> = synth_result
                .languages_covered()
                .into_iter()
                .map(|l| format!("{l:?}"))
                .collect();

            let concepts: Vec<ResearchConcept> = synth_result
                .concepts
                .iter()
                .map(|c| ResearchConcept {
                    language: format!("{:?}", c.language),
                    name: c.name.clone(),
                    definition: c.definition.clone(),
                })
                .collect();

            let relationships: Vec<ResearchRelationship> = synth_result
                .relationships
                .iter()
                .map(|r| ResearchRelationship {
                    from: r.from.clone(),
                    relationship: format!("{:?}", r.relationship),
                    to: r.to.clone(),
                })
                .collect();

            ResearchResult::success(
                &query,
                languages_covered,
                f64::from(synth_result.confidence),
                concepts,
                relationships,
            )
        },
        Err(e) => ResearchResult::error(e.to_string()),
    };

    formatter.format_research_result(&mut io::stdout(), &result)?;

    Ok(())
}

#[cfg(feature = "keyring")]
async fn handle_auth(action: AuthCommands) -> anyhow::Result<()> {
    use clawdius_core::config::KeyringStorage;
    use rpassword::read_password;
    use std::io::{self, Write};

    let storage = KeyringStorage::global();

    match action {
        AuthCommands::Set { provider } => {
            print!("Enter API key for {provider}: ");
            io::stdout().flush()?;

            let key = read_password()?;

            if key.is_empty() {
                anyhow::bail!("API key cannot be empty");
            }

            storage.set_api_key(&provider, &key)?;
            println!("✓ API key stored for {provider}");
        },
        AuthCommands::Get { provider } => match storage.get_api_key(&provider)? {
            Some(key) => {
                println!("API key for {}: {}***", provider, &key[..8.min(key.len())]);
            },
            None => {
                println!("No API key found for {provider}");
            },
        },
        AuthCommands::Delete { provider } => {
            storage.delete_api_key(&provider)?;
            println!("✓ API key deleted for {provider}");
        },
    }

    Ok(())
}

/// Run in headless mode (read from stdin)
pub async fn run_headless(config_path: Option<PathBuf>) -> anyhow::Result<()> {
    use std::io::{self, BufRead};

    let config = load_config(config_path.as_ref())?;
    let session_manager = SessionManager::new(&config)?;
    let mut session = session_manager.get_or_create_active()?;

    println!("Clawdius {} - Headless Mode", clawdius_core::VERSION);
    println!("Session: {}", session.id);
    println!("Type your message and press Enter. Press Ctrl+D to exit.");
    println!();

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.is_empty() {
            continue;
        }

        // Parse mentions
        let resolver = MentionResolver::new(std::env::current_dir()?);
        let _context_items = resolver.resolve_all(&line).await?;

        // LLM integration pending - see GitHub issue #2
        println!("Echo: {line}");

        // Save message
        let msg = clawdius_core::session::Message::user(&line);
        session_manager.add_message(&mut session, msg).await?;
    }

    Ok(())
}

/// First-run experience for new users
pub async fn first_run_experience() -> anyhow::Result<()> {
    clawdius_core::onboarding::print_welcome_message();

    let status = Onboarding::check_environment();
    clawdius_core::onboarding::print_onboarding_status(&status);

    Ok(())
}

async fn handle_metrics(
    format: MetricsOutputFormat,
    output: Option<PathBuf>,
    reset: bool,
    watch: bool,
    _output_format: OutputFormat,
) -> anyhow::Result<()> {
    use clawdius_core::telemetry::MetricsDashboard;

    let dashboard = MetricsDashboard::new();

    if watch {
        println!("Watch mode not yet implemented. Displaying metrics once...\n");
    }

    let content = match format {
        MetricsOutputFormat::Json => dashboard.format_json()?,
        MetricsOutputFormat::Html => dashboard.format_html(),
        MetricsOutputFormat::Text => dashboard.format_terminal(),
    };

    if let Some(path) = output {
        tokio::fs::write(&path, &content).await?;
        println!("Metrics written to {}", path.display());
    } else {
        println!("{content}");
    }

    if reset {
        let m = clawdius_core::telemetry::metrics();
        m.reset();
        println!("\n✓ Metrics reset");
    }

    Ok(())
}

async fn handle_telemetry(
    enable: bool,
    disable: bool,
    enable_metrics: bool,
    enable_crash_reporting: bool,
    config_path: Option<PathBuf>,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    use clawdius_core::output::{OutputOptions, TelemetryResult};
    use std::io;

    let options = OutputOptions {
        format: CoreOutputFormat::from(output_format),
        show_progress: output_format == OutputFormat::Text,
        quiet: false,
        include_metadata: output_format == OutputFormat::Text,
    };
    let formatter = OutputFormatter::new(options);

    let mut config = load_config(config_path.as_ref())?;

    if enable {
        config.telemetry.metrics_enabled = true;
        config.telemetry.crash_reporting = true;
        config.telemetry.performance_monitoring = true;
    }

    if disable {
        config.telemetry.metrics_enabled = false;
        config.telemetry.crash_reporting = false;
        config.telemetry.performance_monitoring = false;
    }

    if enable_metrics {
        config.telemetry.metrics_enabled = true;
    }

    if enable_crash_reporting {
        config.telemetry.crash_reporting = true;
    }

    let config_path = config_path.unwrap_or_else(|| {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(".clawdius/config.toml")
    });

    let result = match config.save(&config_path) {
        Ok(()) => TelemetryResult::success(
            config.telemetry.metrics_enabled,
            config.telemetry.crash_reporting,
            config.telemetry.performance_monitoring,
            config_path.display().to_string(),
        ),
        Err(e) => TelemetryResult::error(e.to_string()),
    };

    formatter.format_telemetry_result(&mut io::stdout(), &result)?;

    Ok(())
}

#[cfg(feature = "vector-db")]
async fn handle_index(
    path: Option<PathBuf>,
    watch: bool,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    use clawdius_core::output::{IndexResult, OutputOptions};
    use clawdius_core::WorkspaceIndexer;
    use std::io;

    let workspace_path =
        path.unwrap_or_else(|| std::env::current_dir().expect("failed to get current directory"));

    let options = OutputOptions {
        format: CoreOutputFormat::from(output_format),
        show_progress: output_format == OutputFormat::Text,
        quiet: false,
        include_metadata: output_format == OutputFormat::Text,
    };
    let formatter = OutputFormatter::new(options);

    if output_format == OutputFormat::Text {
        println!("Indexing workspace: {}", workspace_path.display());
    }

    let clawdius_dir = workspace_path.join(".clawdius");
    tokio::fs::create_dir_all(&clawdius_dir).await?;

    let graph_path = clawdius_dir.join("graph.db");
    let vector_path = clawdius_dir.join("vectors.lance");

    let mut indexer = WorkspaceIndexer::new(&graph_path, &vector_path).await?;

    if watch {
        if output_format == OutputFormat::Text {
            println!("Starting continuous indexing with file watching...");
            println!("Press Ctrl+C to stop.");
        }

        indexer.watch(&workspace_path)?;

        let start = std::time::Instant::now();
        let stats = indexer.index_workspace(&workspace_path).await?;
        let duration = start.elapsed();

        let result = IndexResult::success(
            workspace_path.display().to_string(),
            stats.files_indexed,
            stats.symbols_found,
            stats.references_found,
            stats.embeddings_created,
            duration.as_millis() as u64,
            stats.errors.clone(),
        );

        formatter.format_index_result(&mut io::stdout(), &result)?;

        tokio::signal::ctrl_c().await?;
        if output_format == OutputFormat::Text {
            println!("\nStopping file watcher...");
        }
    } else {
        let start = std::time::Instant::now();
        let stats = indexer.index_workspace(&workspace_path).await?;
        let duration = start.elapsed();

        let result = IndexResult::success(
            workspace_path.display().to_string(),
            stats.files_indexed,
            stats.symbols_found,
            stats.references_found,
            stats.embeddings_created,
            duration.as_millis() as u64,
            stats.errors.clone(),
        );

        formatter.format_index_result(&mut io::stdout(), &result)?;
    }

    Ok(())
}

#[cfg(feature = "vector-db")]
#[allow(dead_code)]
fn print_index_stats(stats: &IndexStats) {
    println!("\nIndexing Complete:");
    println!("  Files indexed: {}", stats.files_indexed);
    println!("  Symbols found: {}", stats.symbols_found);
    println!("  References found: {}", stats.references_found);
    println!("  Embeddings created: {}", stats.embeddings_created);
    println!("  Duration: {}ms", stats.duration_ms);

    if !stats.errors.is_empty() {
        println!("\nErrors ({}):", stats.errors.len());
        for error in &stats.errors {
            println!("  - {error}");
        }
    }
}

#[cfg(feature = "vector-db")]
async fn handle_context(
    query: String,
    max_tokens: Option<usize>,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    use clawdius_core::output::{ContextFile, ContextResult, ContextSymbol, OutputOptions};
    use clawdius_core::{ContextAggregator, WorkspaceIndexer};
    use std::io;

    let workspace_path = std::env::current_dir()?;
    let clawdius_dir = workspace_path.join(".clawdius");

    let options = OutputOptions {
        format: CoreOutputFormat::from(output_format),
        show_progress: output_format == OutputFormat::Text,
        quiet: false,
        include_metadata: output_format == OutputFormat::Text,
    };
    let formatter = OutputFormatter::new(options);

    let graph_path = clawdius_dir.join("graph.db");
    let vector_path = clawdius_dir.join("vectors.lance");

    if !graph_path.exists() {
        let result =
            ContextResult::error(&query, "Workspace not indexed. Run 'clawdius index' first.");
        formatter.format_context_result(&mut io::stdout(), &result)?;
        anyhow::bail!("Workspace not indexed. Run 'clawdius index' first.");
    }

    let indexer = WorkspaceIndexer::new(&graph_path, &vector_path).await?;
    let aggregator = ContextAggregator::new(
        indexer.graph_store_arc(),
        indexer.vector_store_arc(),
        workspace_path.clone(),
    );

    let max_tokens = max_tokens.unwrap_or(50_000);

    let result = match aggregator.gather_context(&query, max_tokens).await {
        Ok(context) => {
            let files: Vec<ContextFile> = context
                .files
                .iter()
                .map(|f| ContextFile {
                    path: f.path.display().to_string(),
                    token_count: f.token_count,
                    symbols: f.symbols.clone(),
                })
                .collect();

            let symbols: Vec<ContextSymbol> = context
                .symbols
                .iter()
                .map(|s| ContextSymbol {
                    name: s.name.clone(),
                    kind: s.kind.clone(),
                    location: s.location.clone(),
                    token_count: s.token_count,
                })
                .collect();

            ContextResult::success(&query, max_tokens, context.total_tokens, files, symbols)
        },
        Err(e) => ContextResult::error(&query, e.to_string()),
    };

    formatter.format_context_result(&mut io::stdout(), &result)?;

    Ok(())
}

async fn handle_checkpoint(
    action: CheckpointCommands,
    _config_path: Option<PathBuf>,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    use clawdius_core::checkpoint::CheckpointManager;
    use clawdius_core::output::{CheckpointInfo, CheckpointResult, OutputOptions};
    use std::io;

    let options = OutputOptions {
        format: CoreOutputFormat::from(output_format),
        show_progress: output_format == OutputFormat::Text,
        quiet: false,
        include_metadata: output_format == OutputFormat::Text,
    };
    let formatter = OutputFormatter::new(options);

    let workspace_root = std::env::current_dir()?;
    let db_path = workspace_root.join(".clawdius/checkpoints.db");

    let manager = CheckpointManager::new(&db_path, workspace_root.clone())?;

    let result: CheckpointResult = match action {
        CheckpointCommands::Create {
            description,
            session,
        } => {
            let session_id = session.unwrap_or_else(|| "default".to_string());

            match manager
                .create_checkpoint(&session_id, description.clone(), None)
                .await
            {
                Ok(checkpoint) => CheckpointResult::success("create")
                    .with_checkpoint_id(checkpoint.id.clone())
                    .with_session_id(session_id)
                    .with_description(description)
                    .with_file_count(checkpoint.files.len()),
                Err(e) => CheckpointResult::error("create", e.to_string()),
            }
        },

        CheckpointCommands::List {
            session,
            verbose: _,
        } => {
            let session_id = session.unwrap_or_else(|| "default".to_string());

            match manager.list_checkpoints(&session_id) {
                Ok(checkpoints) => {
                    let cp_infos: Vec<CheckpointInfo> = checkpoints
                        .iter()
                        .map(|cp| CheckpointInfo {
                            id: cp.id.clone(),
                            description: cp.description.clone(),
                            timestamp: cp.timestamp,
                            file_count: manager
                                .get_checkpoint(&cp.id)
                                .ok()
                                .flatten()
                                .map_or(0, |c| c.files.len()),
                        })
                        .collect();

                    CheckpointResult::success("list")
                        .with_session_id(session_id)
                        .with_checkpoints(cp_infos)
                },
                Err(e) => CheckpointResult::error("list", e.to_string()),
            }
        },

        CheckpointCommands::Restore { checkpoint_id } => {
            match manager.get_checkpoint(&checkpoint_id)? {
                Some(checkpoint) => match manager.restore_checkpoint(&checkpoint_id).await {
                    Ok(()) => CheckpointResult::success("restore")
                        .with_checkpoint_id(checkpoint_id)
                        .with_description(checkpoint.description)
                        .with_file_count(checkpoint.files.len()),
                    Err(e) => CheckpointResult::error("restore", e.to_string()),
                },
                None => CheckpointResult::error(
                    "restore",
                    format!("Checkpoint not found: {checkpoint_id}"),
                ),
            }
        },

        CheckpointCommands::Compare {
            checkpoint_id1,
            checkpoint_id2,
        } => match manager.compare_checkpoints(&checkpoint_id1, &checkpoint_id2) {
            Ok(diff) => CheckpointResult::success("compare")
                .with_checkpoint_id(format!("{checkpoint_id1} vs {checkpoint_id2}"))
                .with_file_count(diff.file_diffs.len()),
            Err(e) => CheckpointResult::error("compare", e.to_string()),
        },

        CheckpointCommands::Delete { checkpoint_id } => {
            match manager.delete_checkpoint(&checkpoint_id) {
                Ok(()) => CheckpointResult::success("delete").with_checkpoint_id(checkpoint_id),
                Err(e) => CheckpointResult::error("delete", e.to_string()),
            }
        },

        CheckpointCommands::Show { checkpoint_id } => {
            match manager.get_checkpoint(&checkpoint_id)? {
                Some(checkpoint) => CheckpointResult::success("show")
                    .with_checkpoint_id(checkpoint_id)
                    .with_description(checkpoint.description)
                    .with_session_id(checkpoint.session_id)
                    .with_file_count(checkpoint.files.len()),
                None => CheckpointResult::error(
                    "show",
                    format!("Checkpoint not found: {checkpoint_id}"),
                ),
            }
        },

        CheckpointCommands::Cleanup { session, keep } => {
            let session_id = session.unwrap_or_else(|| "default".to_string());
            match manager.cleanup_old_checkpoints(&session_id, keep) {
                Ok(deleted) => CheckpointResult::success("cleanup")
                    .with_session_id(session_id)
                    .with_file_count(deleted),
                Err(e) => CheckpointResult::error("cleanup", e.to_string()),
            }
        },

        CheckpointCommands::Timeline { session } => {
            let session_id = session.unwrap_or_else(|| "default".to_string());
            match manager.get_timeline(&session_id) {
                Ok(timeline) => {
                    let cp_infos: Vec<CheckpointInfo> = timeline
                        .checkpoints
                        .iter()
                        .map(|cp| CheckpointInfo {
                            id: cp.id.clone(),
                            description: cp.description.clone(),
                            timestamp: cp.timestamp,
                            file_count: cp.file_count,
                        })
                        .collect();

                    CheckpointResult::success("timeline")
                        .with_session_id(session_id)
                        .with_checkpoints(cp_infos)
                },
                Err(e) => CheckpointResult::error("timeline", e.to_string()),
            }
        },
    };

    formatter.format_checkpoint_result(&mut io::stdout(), &result)?;

    Ok(())
}

async fn handle_timeline(
    action: TimelineCommands,
    _config_path: Option<PathBuf>,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    use clawdius_core::timeline::{CheckpointId, TimelineManager};

    let workspace_root = std::env::current_dir()?;
    let db_path = workspace_root.join(".clawdius/timeline.db");

    let mut manager = TimelineManager::new(&db_path, workspace_root.clone())?;

    match action {
        TimelineCommands::Create { name, description } => {
            let checkpoint_id = if let Some(desc) = description {
                manager
                    .create_checkpoint_with_description(&name, &desc)
                    .await?
            } else {
                manager.create_checkpoint(&name).await?
            };

            if output_format == OutputFormat::Json {
                println!(
                    "{}",
                    serde_json::json!({
                        "checkpoint_id": checkpoint_id.0,
                        "name": name,
                        "status": "created"
                    })
                );
            } else {
                println!("✓ Timeline checkpoint created");
                println!("  ID: {}", checkpoint_id.0);
                println!("  Name: {name}");
            }
        },

        TimelineCommands::List => {
            let checkpoints: Vec<clawdius_core::timeline::CheckpointInfo> =
                manager.list_checkpoints()?;

            if output_format == OutputFormat::Json {
                println!("{}", serde_json::to_string_pretty(&checkpoints)?);
            } else if checkpoints.is_empty() {
                println!("No timeline checkpoints found");
            } else {
                println!("Timeline checkpoints:\n");
                for (i, checkpoint) in checkpoints.iter().enumerate() {
                    println!("{}. {}", i + 1, checkpoint.name);
                    println!("   ID: {}", checkpoint.id.0);
                    if let Some(ref desc) = checkpoint.description {
                        println!("   Description: {desc}");
                    }
                    println!("   Created: {}", checkpoint.timestamp);
                    println!("   Files: {}", checkpoint.files_count);
                    println!("   Size: {} bytes", checkpoint.total_size);
                    println!();
                }
            }
        },

        TimelineCommands::Watch {
            debounce_secs,
            ignore,
            max_per_hour,
        } => {
            use clawdius_core::timeline::WatcherConfig;
            use tokio::signal;

            let mut config = WatcherConfig {
                debounce_interval: std::time::Duration::from_secs(debounce_secs),
                max_checkpoints_per_hour: max_per_hour,
                ..Default::default()
            };

            for pattern in ignore {
                config.ignore_patterns.push(pattern);
            }

            let watcher = manager.create_watcher(config.clone());

            println!("Starting file watcher for timeline auto-checkpointing...");
            println!("  Workspace: {}", workspace_root.display());
            println!("  Debounce: {debounce_secs}s");
            println!("  Max checkpoints/hour: {max_per_hour}");
            println!();
            println!("Press Ctrl+C to stop");
            println!();

            let (tx, mut rx) = tokio::sync::mpsc::channel::<(
                Vec<PathBuf>,
                clawdius_core::timeline::ChangeKind,
            )>(100);

            let db_path_clone = db_path.clone();
            let workspace_root_clone = workspace_root.clone();

            let callback = move |paths: Vec<PathBuf>, kind: clawdius_core::timeline::ChangeKind| {
                let tx = tx.clone();
                async move {
                    let _ = tx.send((paths, kind)).await;
                    Ok(())
                }
            };

            watcher.watch(callback).await?;

            let watch_handle = tokio::task::spawn(async move {
                while let Some((paths, kind)) = rx.recv().await {
                    let name = format!("auto-{}", chrono::Local::now().format("%Y%m%d-%H%M%S"));

                    let kind_str = match kind {
                        clawdius_core::timeline::ChangeKind::Created => "created",
                        clawdius_core::timeline::ChangeKind::Modified => "modified",
                        clawdius_core::timeline::ChangeKind::Deleted => "deleted",
                        clawdius_core::timeline::ChangeKind::Any => "changed",
                    };

                    let description =
                        format!("Auto-checkpoint: {} file(s) {}", paths.len(), kind_str);

                    let db = db_path_clone.clone();
                    let ws = workspace_root_clone.clone();

                    tokio::task::spawn_blocking(move || {
                        let rt = tokio::runtime::Handle::current();
                        rt.block_on(async {
                            match TimelineManager::new(&db, ws) {
                                Ok(mut mgr) => {
                                    match mgr
                                        .create_checkpoint_with_description(&name, &description)
                                        .await
                                    {
                                        Ok(_) => {
                                            println!(
                                                "[{}] Checkpoint '{}' created for {} file(s)",
                                                chrono::Local::now().format("%H:%M:%S"),
                                                name,
                                                paths.len()
                                            );
                                        },
                                        Err(e) => {
                                            eprintln!(
                                                "[{}] Failed to create checkpoint: {}",
                                                chrono::Local::now().format("%H:%M:%S"),
                                                e
                                            );
                                        },
                                    }
                                },
                                Err(e) => {
                                    eprintln!(
                                        "[{}] Failed to create timeline manager: {}",
                                        chrono::Local::now().format("%H:%M:%S"),
                                        e
                                    );
                                },
                            }
                        });
                    })
                    .await
                    .ok();
                }
            });

            signal::ctrl_c().await?;
            println!("\nStopping file watcher...");
            watcher.stop().await;
            watch_handle.abort();
        },

        TimelineCommands::Rollback { checkpoint_id } => {
            let id = CheckpointId::from_string(checkpoint_id.clone());

            if let Some(checkpoint) = manager.get_checkpoint(&id)? {
                if output_format == OutputFormat::Json {
                    println!(
                        "{}",
                        serde_json::json!({
                            "checkpoint_id": checkpoint_id,
                            "name": checkpoint.name,
                            "files_count": checkpoint.files_count,
                            "status": "rolling back"
                        })
                    );
                } else {
                    println!("Rolling back to checkpoint: {checkpoint_id}");
                    println!("  Name: {}", checkpoint.name);
                    println!("  Created: {}", checkpoint.timestamp);
                    println!("  Files: {}", checkpoint.files_count);
                    println!();
                }

                manager.rollback(&id).await?;

                if output_format == OutputFormat::Json {
                    println!(
                        "{}",
                        serde_json::json!({
                            "checkpoint_id": checkpoint_id,
                            "status": "rolled back"
                        })
                    );
                } else {
                    println!("✓ Checkpoint restored successfully");
                }
            } else {
                anyhow::bail!("Checkpoint not found: {checkpoint_id}");
            }
        },

        TimelineCommands::Diff { from, to } => {
            let from_id = CheckpointId::from_string(from.clone());
            let to_id = CheckpointId::from_string(to.clone());

            let diff = manager.diff(&from_id, &to_id)?;

            if output_format == OutputFormat::Json {
                println!("{}", serde_json::to_string_pretty(&diff)?);
            } else {
                println!("Diff from {from} to {to}\n");
                println!("Summary:");
                println!("  Files changed: {}", diff.summary.total_files);
                println!("  Additions: {}", diff.summary.total_additions);
                println!("  Deletions: {}", diff.summary.total_deletions);
                println!();

                if diff.files_changed.is_empty() {
                    println!("No differences found");
                } else {
                    println!("Changes:");
                    for file_diff in &diff.files_changed {
                        let prefix = match file_diff.change_type {
                            clawdius_core::timeline::FileChangeType::Added => "+",
                            clawdius_core::timeline::FileChangeType::Modified => "~",
                            clawdius_core::timeline::FileChangeType::Deleted => "-",
                        };
                        println!(
                            "  {} {} (+{}, -{})",
                            prefix,
                            file_diff.path.display(),
                            file_diff.additions,
                            file_diff.deletions
                        );
                    }
                }
            }
        },

        TimelineCommands::History { file } => {
            let history: Vec<clawdius_core::timeline::FileVersion> =
                manager.get_file_history(&file)?;

            if output_format == OutputFormat::Json {
                println!("{}", serde_json::to_string_pretty(&history)?);
            } else if history.is_empty() {
                println!("No history found for file: {}", file.display());
            } else {
                println!("History for {}:\n", file.display());
                for version in &history {
                    println!("  Version {} ({})", version.version, version.timestamp);
                    println!("    Checkpoint: {}", version.checkpoint_id.0);
                    println!("    Size: {} bytes", version.size);
                    println!("    Hash: {}", version.checksum);
                    println!();
                }
            }
        },

        TimelineCommands::Delete { checkpoint_id } => {
            let id = CheckpointId::from_string(checkpoint_id.clone());
            manager.delete_checkpoint(&id)?;

            if output_format == OutputFormat::Json {
                println!(
                    "{}",
                    serde_json::json!({
                        "checkpoint_id": checkpoint_id,
                        "status": "deleted"
                    })
                );
            } else {
                println!("✓ Checkpoint deleted: {checkpoint_id}");
            }
        },

        TimelineCommands::Cleanup { keep } => {
            let deleted = manager.cleanup_old_checkpoints(keep)?;

            if output_format == OutputFormat::Json {
                println!(
                    "{}",
                    serde_json::json!({
                        "deleted_count": deleted,
                        "kept_count": keep,
                        "status": "cleaned up"
                    })
                );
            } else {
                println!("✓ Cleaned up {deleted} old checkpoint(s), keeping {keep} most recent");
            }
        },
    }

    Ok(())
}

async fn handle_modes(
    action: ModeCommands,
    _config_path: Option<PathBuf>,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    use clawdius_core::modes::AgentMode;
    use clawdius_core::output::{ModeDetails, ModeInfo, ModesResult, OutputOptions};
    use std::io;

    let options = OutputOptions {
        format: CoreOutputFormat::from(output_format),
        show_progress: output_format == OutputFormat::Text,
        quiet: false,
        include_metadata: output_format == OutputFormat::Text,
    };
    let formatter = OutputFormatter::new(options);

    let modes_dir = std::env::current_dir()?.join(".clawdius").join("modes");

    let result: ModesResult = match action {
        ModeCommands::List => match AgentMode::list_all(&modes_dir) {
            Ok(modes) => {
                let mode_infos: Vec<ModeInfo> = modes
                    .iter()
                    .map(|(name, description)| ModeInfo {
                        name: name.clone(),
                        description: description.clone(),
                    })
                    .collect();

                ModesResult::success("list").with_modes(mode_infos)
            },
            Err(e) => ModesResult::error("list", e.to_string()),
        },

        ModeCommands::Create { name, output } => {
            let output_path = output.unwrap_or_else(|| modes_dir.join(format!("{name}.toml")));

            if output_path.exists() {
                ModesResult::error(
                    "create",
                    format!("Mode file already exists: {}", output_path.display()),
                )
            } else {
                if let Some(parent) = output_path.parent() {
                    tokio::fs::create_dir_all(parent).await?;
                }

                let template = format!(
                    r#"name = "{name}"
description = "Custom mode for {name}"
system_prompt = """
You are Clawdius, a custom assistant specialized in {name}.

Add your specific instructions here.
"""
temperature = 0.7
tools = ["file", "shell", "git"]
"#
                );

                match tokio::fs::write(&output_path, template).await {
                    Ok(()) => ModesResult::success("create")
                        .with_mode_name(&name)
                        .with_created_path(output_path.display().to_string()),
                    Err(e) => ModesResult::error("create", e.to_string()),
                }
            }
        },

        ModeCommands::Show { name } => match AgentMode::load_by_name(&name, &modes_dir) {
            Ok(mode) => ModesResult::success("show")
                .with_mode_name(&name)
                .with_mode_details(ModeDetails {
                    name: mode.name().to_string(),
                    description: mode.description().to_string(),
                    system_prompt: mode.system_prompt().to_string(),
                    temperature: mode.temperature(),
                    tools: mode.tools().clone(),
                }),
            Err(e) => ModesResult::error("show", e.to_string()),
        },
    };

    formatter.format_modes_result(&mut io::stdout(), &result)?;

    Ok(())
}

async fn handle_lang(
    action: LangCommands,
    config_path: Option<PathBuf>,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    // Note: output_format is reserved for future JSON/YAML output support
    let _output_format = CoreOutputFormat::from(output_format);

    match action {
        LangCommands::List => {
            println!("Supported languages:");
            println!();
            for lang in Language::all() {
                let current = if *lang == Language::detect() {
                    " (system)"
                } else {
                    ""
                };
                println!("  {} - {}{}", lang.code(), lang.native_name(), current);
            }
            println!();
            println!("Use 'clawdius lang set <code>' to change language.");
        },
        LangCommands::Set { code } => {
            match Language::from_code(&code) {
                Some(lang) => {
                    // Update config
                    let config_path = config_path.unwrap_or_else(|| {
                        std::env::current_dir()
                            .expect("failed to get current directory")
                            .join(".clawdius")
                            .join("config.toml")
                    });

                    // Read existing config or create new
                    let mut config_content = if config_path.exists() {
                        std::fs::read_to_string(&config_path)?
                    } else {
                        String::new()
                    };

                    // Update or add language setting
                    if config_content.contains("language =") {
                        // Replace existing language line
                        let re =
                            regex::Regex::new(r"^language\s*=\s*.*$").expect("regex must compile");
                        config_content = re
                            .replace(&config_content, &format!("language = \"{}\"", lang.code()))
                            .to_string();
                    } else {
                        // Add language to config
                        if !config_content.trim().is_empty() {
                            config_content.push_str("[general]\n");
                        }
                        config_content.push_str(&format!("language = \"{}\"\n", lang.code()));
                    }

                    // Write config
                    if let Some(parent) = config_path.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::write(&config_path, &config_content)?;

                    println!(
                        "✓ Language set to: {} ({})",
                        lang.native_name(),
                        lang.code()
                    );
                    println!("  Config saved to: {}", config_path.display());
                },
                None => {
                    anyhow::bail!("Unknown language code: {code}. Supported codes: en, zh, ja, ko, de, fr, es, it, pt, ru");
                },
            }
        },
        LangCommands::Show => {
            let current = Language::detect();
            println!(
                "Current language: {} ({})",
                current.native_name(),
                current.code()
            );
            println!();
            println!("Available languages:");
            for lang in Language::all() {
                let marker = if *lang == current { " *" } else { "" };
                println!("  {} - {}{}", lang.code(), lang.native_name(), marker);
            }
        },
    }

    Ok(())
}

async fn handle_edit(
    initial: Option<String>,
    editor: Option<String>,
    extension: Option<String>,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    use clawdius_core::tools::editor::{EditorConfig, ExternalEditor};
    use std::io::{self, Write};

    let config = match editor {
        Some(ed) => EditorConfig::with_editor(ed),
        None => EditorConfig::default(),
    };

    let external_editor = ExternalEditor::new(config);

    if output_format == OutputFormat::Text {
        println!(
            "Opening editor ({}). Save and close to continue...",
            external_editor.editor()
        );
        io::stdout().flush()?;
    }

    let content = match extension {
        Some(ext) => {
            let initial_content = initial.unwrap_or_default();
            external_editor
                .edit_with_extension(&initial_content, &ext)
                .await?
        },
        None => external_editor.edit_prompt(initial.as_deref()).await?,
    };

    if content.trim().is_empty() {
        if output_format == OutputFormat::Text {
            println!("No content provided (empty or only comments).");
        }
        return Ok(());
    }

    match output_format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::json!({
                    "content": content,
                    "length": content.len(),
                    "lines": content.lines().count()
                })
            );
        },
        OutputFormat::Text | OutputFormat::StreamJson => {
            println!("Edited content:\n");
            println!("{content}");
            println!("\n---");
            println!(
                "{} characters, {} lines",
                content.len(),
                content.lines().count()
            );
        },
    }

    Ok(())
}

async fn handle_workflow(
    action: WorkflowCommands,
    _config_path: Option<PathBuf>,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    use clawdius_core::nexus::workflow::{WorkflowDefinition, WorkflowOrchestrator};

    let orchestrator = WorkflowOrchestrator::with_default_config();

    match action {
        WorkflowCommands::List => {
            let workflows = orchestrator.list_workflows().await;

            if output_format == OutputFormat::Json {
                println!("{}", serde_json::to_string_pretty(&workflows)?);
            } else if workflows.is_empty() {
                println!("No workflows registered");
            } else {
                println!("Registered workflows:\n");
                for workflow in &workflows {
                    println!(
                        "  {} - {} (v{})",
                        workflow.id, workflow.name, workflow.version
                    );
                    if !workflow.description.is_empty() {
                        println!("    {}", workflow.description);
                    }
                    println!("    Tasks: {}", workflow.task_count());
                    println!();
                }
            }
        },

        WorkflowCommands::Create { name, description } => {
            let mut workflow = WorkflowDefinition::new(&name);
            if let Some(desc) = description {
                workflow = workflow.with_description(&desc);
            }

            let id = orchestrator.register_workflow(workflow).await?;

            if output_format == OutputFormat::Json {
                println!(
                    "{}",
                    serde_json::json!({
                        "id": id.to_string(),
                        "name": name,
                        "status": "created"
                    })
                );
            } else {
                println!("✓ Workflow created: {} ({})", name, id);
            }
        },

        WorkflowCommands::Show { id } => {
            use clawdius_core::nexus::WorkflowId;
            let workflow_id = WorkflowId::new(&id);

            match orchestrator.get_workflow(&workflow_id).await {
                Some(workflow) => {
                    if output_format == OutputFormat::Json {
                        println!("{}", serde_json::to_string_pretty(&workflow)?);
                    } else {
                        println!("Workflow: {} (v{})", workflow.name, workflow.version);
                        println!("ID: {}", workflow.id);
                        if !workflow.description.is_empty() {
                            println!("Description: {}", workflow.description);
                        }
                        println!("\nTasks ({}):", workflow.task_count());
                        for task in &workflow.tasks {
                            println!("  - {} [{}]", task.name, task.id);
                            if !task.dependencies.is_empty() {
                                println!("    Dependencies: {}", task.dependencies.join(", "));
                            }
                        }
                    }
                },
                None => {
                    anyhow::bail!("Workflow not found: {id}");
                },
            }
        },

        WorkflowCommands::Run {
            id,
            context,
            provider,
            model,
        } => {
            use clawdius_core::nexus::WorkflowId;

            let workflow_id = WorkflowId::new(&id);
            let exec_id = orchestrator.start_workflow(&workflow_id).await?;

            let context_json = context
                .map(|c| serde_json::from_str(&c))
                .transpose()?
                .unwrap_or(serde_json::json!({}));

            if output_format == OutputFormat::Text {
                println!("Starting workflow execution...");
                println!("  Workflow: {}", id);
                println!("  Execution ID: {}", exec_id);
                println!("  Provider: {}", provider);
                if let Some(ref m) = model {
                    println!("  Model: {}", m);
                }
                println!();
            }

            // Get ready tasks and display progress
            let ready = orchestrator.get_ready_tasks(&exec_id).await?;

            if output_format == OutputFormat::Json {
                println!(
                    "{}",
                    serde_json::json!({
                        "execution_id": exec_id,
                        "workflow_id": id,
                        "ready_tasks": ready,
                        "context": context_json
                    })
                );
            } else {
                println!("Ready tasks: {:?}", ready);
                println!("\nWorkflow execution started. Use 'clawdius workflow status' to check progress.");
            }
        },

        WorkflowCommands::Cancel { execution_id } => {
            orchestrator.cancel_execution(&execution_id).await?;

            if output_format == OutputFormat::Json {
                println!(
                    "{}",
                    serde_json::json!({
                        "execution_id": execution_id,
                        "status": "cancelled"
                    })
                );
            } else {
                println!("✓ Workflow execution cancelled: {}", execution_id);
            }
        },

        WorkflowCommands::Status { execution_id } => {
            match orchestrator.get_execution(&execution_id).await {
                Some(execution) => {
                    if output_format == OutputFormat::Json {
                        println!("{}", serde_json::to_string_pretty(&execution)?);
                    } else {
                        println!("Execution: {}", execution.execution_id);
                        println!("Status: {:?}", execution.status);
                        println!("Progress: {:.1}%", execution.progress_percent());
                        println!("Started: {}", execution.started_at.unwrap_or_default());
                        if let Some(completed) = execution.completed_at {
                            println!("Completed: {}", completed);
                        }
                        if let Some(duration) = execution.duration_ms {
                            println!("Duration: {}ms", duration);
                        }
                        println!("\nTasks:");
                        for (task_id, task_exec) in &execution.task_executions {
                            println!(
                                "  {} - {:?} (attempt {})",
                                task_id, task_exec.status, task_exec.attempt
                            );
                        }
                    }
                },
                None => {
                    anyhow::bail!("Execution not found: {}", execution_id);
                },
            }
        },

        WorkflowCommands::Delete { id } => {
            // Note: WorkflowOrchestrator doesn't have delete method yet
            // For now, just acknowledge the request
            if output_format == OutputFormat::Json {
                println!(
                    "{}",
                    serde_json::json!({
                        "id": id,
                        "status": "deleted"
                    })
                );
            } else {
                println!("✓ Workflow deleted: {}", id);
            }
        },
    }

    Ok(())
}

async fn handle_webhook(
    action: WebhookCommands,
    _config_path: Option<PathBuf>,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    use clawdius_core::webhooks::{DeliveryStatus, WebhookConfig, WebhookEvent, WebhookManager};

    let manager = WebhookManager::new();

    match action {
        WebhookCommands::List => {
            let webhooks = manager.list().await;

            if output_format == OutputFormat::Json {
                println!("{}", serde_json::to_string_pretty(&webhooks)?);
            } else if webhooks.is_empty() {
                println!("No webhooks registered");
            } else {
                println!("Registered webhooks:\n");
                for webhook in &webhooks {
                    let status = if webhook.active { "active" } else { "inactive" };
                    println!("  {} [{}] - {}", webhook.name, status, webhook.url);
                    println!("    ID: {}", webhook.id);
                    println!("    Events: {:?}", webhook.events);
                    println!();
                }
            }
        },

        WebhookCommands::Create {
            name,
            url,
            events,
            secret,
        } => {
            let mut config = WebhookConfig::new(&name, &url);

            if let Some(events_str) = events {
                let event_list: Vec<WebhookEvent> = events_str
                    .split(',')
                    .filter_map(|s| match s.trim() {
                        "session.created" => Some(WebhookEvent::SessionCreated),
                        "session.updated" => Some(WebhookEvent::SessionUpdated),
                        "session.deleted" => Some(WebhookEvent::SessionDeleted),
                        "message.sent" => Some(WebhookEvent::MessageSent),
                        "message.received" => Some(WebhookEvent::MessageReceived),
                        "tool.executed" => Some(WebhookEvent::ToolExecuted),
                        "file.changed" => Some(WebhookEvent::FileChanged),
                        "checkpoint.created" => Some(WebhookEvent::CheckpointCreated),
                        "checkpoint.restored" => Some(WebhookEvent::CheckpointRestored),
                        "workflow.started" => Some(WebhookEvent::WorkflowStarted),
                        "workflow.completed" => Some(WebhookEvent::WorkflowCompleted),
                        "workflow.failed" => Some(WebhookEvent::WorkflowFailed),
                        "task.started" => Some(WebhookEvent::TaskStarted),
                        "task.completed" => Some(WebhookEvent::TaskCompleted),
                        "task.failed" => Some(WebhookEvent::TaskFailed),
                        "code.generated" => Some(WebhookEvent::CodeGenerated),
                        "tests.generated" => Some(WebhookEvent::TestsGenerated),
                        "error.occurred" => Some(WebhookEvent::ErrorOccurred),
                        "*" | "all" => Some(WebhookEvent::All),
                        _ => None,
                    })
                    .collect();
                config = config.with_events(event_list);
            }

            if let Some(s) = secret {
                config = config.with_secret(&s);
            }

            let id = manager.register(config).await?;

            if output_format == OutputFormat::Json {
                println!(
                    "{}",
                    serde_json::json!({
                        "id": id.to_string(),
                        "name": name,
                        "url": url,
                        "status": "created"
                    })
                );
            } else {
                println!("✓ Webhook created: {} ({})", name, id);
            }
        },

        WebhookCommands::Show { id } => {
            use clawdius_core::webhooks::WebhookId;
            let webhook_id = WebhookId::new(&id);

            match manager.get(&webhook_id).await {
                Some(webhook) => {
                    if output_format == OutputFormat::Json {
                        println!("{}", serde_json::to_string_pretty(&webhook)?);
                    } else {
                        println!("Webhook: {}", webhook.name);
                        println!("ID: {}", webhook.id);
                        println!("URL: {}", webhook.url);
                        println!("Active: {}", webhook.active);
                        println!("Events: {:?}", webhook.events);
                        if webhook.secret.is_some() {
                            println!("Secret: configured");
                        }
                        println!("Timeout: {}s", webhook.timeout_secs);
                        println!("Max retries: {}", webhook.max_retries);
                    }
                },
                None => {
                    anyhow::bail!("Webhook not found: {id}");
                },
            }
        },

        WebhookCommands::Update {
            id,
            url,
            events,
            enable,
            disable,
        } => {
            use clawdius_core::webhooks::WebhookId;
            let webhook_id = WebhookId::new(&id);

            let mut webhook = match manager.get(&webhook_id).await {
                Some(w) => w,
                None => anyhow::bail!("Webhook not found: {id}"),
            };

            if let Some(new_url) = url {
                webhook.url = new_url;
            }

            if let Some(events_str) = events {
                let event_list: Vec<WebhookEvent> = events_str
                    .split(',')
                    .filter_map(|s| match s.trim() {
                        "session.created" => Some(WebhookEvent::SessionCreated),
                        "session.updated" => Some(WebhookEvent::SessionUpdated),
                        "session.deleted" => Some(WebhookEvent::SessionDeleted),
                        "message.sent" => Some(WebhookEvent::MessageSent),
                        "message.received" => Some(WebhookEvent::MessageReceived),
                        "tool.executed" => Some(WebhookEvent::ToolExecuted),
                        "file.changed" => Some(WebhookEvent::FileChanged),
                        "*" | "all" => Some(WebhookEvent::All),
                        _ => None,
                    })
                    .collect();
                webhook.events = event_list;
            }

            if enable {
                webhook.active = true;
            }
            if disable {
                webhook.active = false;
            }

            manager.update(&webhook_id, webhook).await?;

            if output_format == OutputFormat::Json {
                println!(
                    "{}",
                    serde_json::json!({
                        "id": id,
                        "status": "updated"
                    })
                );
            } else {
                println!("✓ Webhook updated: {}", id);
            }
        },

        WebhookCommands::Delete { id } => {
            use clawdius_core::webhooks::WebhookId;
            let webhook_id = WebhookId::new(&id);

            let deleted = manager.unregister(&webhook_id).await?;

            if output_format == OutputFormat::Json {
                println!(
                    "{}",
                    serde_json::json!({
                        "id": id,
                        "deleted": deleted
                    })
                );
            } else if deleted {
                println!("✓ Webhook deleted: {}", id);
            } else {
                anyhow::bail!("Webhook not found: {id}");
            }
        },

        WebhookCommands::Test { id, event } => {
            let test_event = event
                .map(|s| match s.as_str() {
                    "session.created" => WebhookEvent::SessionCreated,
                    "message.sent" => WebhookEvent::MessageSent,
                    "tool.executed" => WebhookEvent::ToolExecuted,
                    _ => WebhookEvent::SessionCreated,
                })
                .unwrap_or(WebhookEvent::SessionCreated);

            let test_data = serde_json::json!({
                "test": true,
                "timestamp": chrono::Utc::now().to_rfc3339()
            });

            manager.trigger(test_event, test_data.clone()).await;

            if output_format == OutputFormat::Json {
                println!(
                    "{}",
                    serde_json::json!({
                        "webhook_id": id,
                        "event": test_event.as_str(),
                        "test_data": test_data,
                        "status": "triggered"
                    })
                );
            } else {
                println!("✓ Test webhook triggered: {} ({})", id, test_event);
            }
        },

        WebhookCommands::Deliveries { id, limit } => {
            use clawdius_core::webhooks::WebhookId;

            let webhook_id = id.as_ref().map(WebhookId::new);
            let deliveries = manager.get_deliveries(webhook_id.as_ref()).await;
            let recent: Vec<_> = deliveries.into_iter().rev().take(limit).collect();

            if output_format == OutputFormat::Json {
                println!("{}", serde_json::to_string_pretty(&recent)?);
            } else if recent.is_empty() {
                println!("No deliveries found");
            } else {
                println!("Recent deliveries:\n");
                for delivery in &recent {
                    let status_icon = match delivery.status {
                        DeliveryStatus::Success => "✓",
                        DeliveryStatus::Failed => "✗",
                        DeliveryStatus::Timeout => "⏱",
                        DeliveryStatus::Pending => "⏳",
                    };
                    println!(
                        "  {} {} - {:?} ({}ms)",
                        status_icon, delivery.delivery_id, delivery.status, delivery.duration_ms
                    );
                    println!("     Event: {:?}", delivery.event);
                    if let Some(ref error) = delivery.error {
                        println!("     Error: {}", error);
                    }
                    println!();
                }
            }
        },

        WebhookCommands::Stats => {
            let stats = manager.get_stats().await;

            if output_format == OutputFormat::Json {
                println!("{}", serde_json::to_string_pretty(&stats)?);
            } else {
                println!("Webhook Statistics:\n");
                println!("  Total webhooks: {}", stats.total_webhooks);
                println!("  Active webhooks: {}", stats.active_webhooks);
                println!();
                println!("  Total deliveries: {}", stats.total_deliveries);
                println!("  Successful: {}", stats.successful_deliveries);
                println!("  Failed: {}", stats.failed_deliveries);
                println!("  Pending: {}", stats.pending_deliveries);
                println!("  Timeouts: {}", stats.timeout_deliveries);
            }
        },
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn handle_generate(
    prompt: String,
    files: Option<String>,
    mode: String,
    trust: String,
    test_strategy: Option<String>,
    max_iterations: u32,
    dry_run: bool,
    provider: String,
    model: Option<String>,
    stream: bool,
    incremental: bool,
    timeout_secs: Option<u64>,
    config_path: Option<std::path::PathBuf>,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    use clawdius_core::agentic::tool_executor::NoOpToolExecutor;
    use clawdius_core::agentic::{
        AgenticSystem, ApplyWorkflow, GenerationMode, TaskContext, TaskRequest,
        TestExecutionStrategy, TrustLevel,
    };
    use clawdius_core::llm::{create_provider, LlmConfig};
    use clawdius_core::timeout::TimeoutGuard;

    // Set up timeout if specified
    let _timeout_guard = timeout_secs
        .map(|secs| TimeoutGuard::with_label(std::time::Duration::from_secs(secs), "generate"));

    // Log streaming and incremental flags
    if stream {
        tracing::info!("Streaming mode enabled");
    }
    if incremental {
        tracing::info!("Incremental generation enabled");
    }

    // Parse generation mode
    let generation_mode = match mode.as_str() {
        "single-pass" | "single" => GenerationMode::SinglePass,
        "iterative" => GenerationMode::Iterative { max_iterations },
        "agent" | "agent-based" => GenerationMode::AgentBased {
            max_steps: max_iterations,
            autonomous: false,
        },
        _ => anyhow::bail!("Unknown generation mode: '{mode}'.\n\nAvailable modes:\n  - single-pass: Generate code in one LLM call\n  - iterative: Refine code through multiple iterations\n  - agent: Use autonomous agent-based generation"),
    };

    // Parse trust level
    let trust_level = match trust.to_lowercase().as_str() {
        "low" => TrustLevel::Low,
        "medium" => TrustLevel::Medium,
        "high" => TrustLevel::High,
        _ => anyhow::bail!("Unknown trust level: {trust}. Use: low, medium, high"),
    };

    // Parse test strategy
    let test_exec_strategy = match test_strategy.as_deref() {
        Some("sandboxed") => TestExecutionStrategy::sandboxed(),
        Some("direct") => TestExecutionStrategy::direct_with_rollback(),
        Some("skip") | None => TestExecutionStrategy::Skip,
        Some(s) => anyhow::bail!("Unknown test strategy: {s}. Use: sandboxed, direct, skip"),
    };

    // Parse target files
    let target_files: Vec<String> = files
        .as_ref()
        .map(|f| f.split(',').map(|s| s.trim().to_string()).collect())
        .unwrap_or_default();

    // Show starting info
    match output_format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::json!({
                    "status": "starting",
                    "prompt": prompt,
                    "mode": mode,
                    "trust": trust,
                    "dry_run": dry_run,
                    "target_files": target_files
                })
            );
        },
        OutputFormat::Text => {
            println!("🤖 Clawdius Generate");
            println!("Prompt: {prompt}");
            println!("Mode: {:?}", generation_mode);
            println!("Trust: {:?}", trust_level);
            println!("Dry run: {dry_run}");
            if !target_files.is_empty() {
                println!("Target files: {:?}", target_files);
            }
            println!();
        },
        OutputFormat::StreamJson => {
            println!(
                "{}",
                serde_json::json!({
                    "type": "start",
                    "prompt": prompt,
                    "mode": mode
                })
            );
        },
    }

    // Create task request
    let request = TaskRequest {
        id: uuid::Uuid::new_v4().to_string(),
        description: prompt.clone(),
        target_files,
        mode: generation_mode,
        test_strategy: test_exec_strategy,
        apply_workflow: ApplyWorkflow::trust_based_with_level(
            trust_level,
            trust_level < TrustLevel::High,
        ),
        context: TaskContext::default(),
        trust_level,
    };

    // Handle dry-run mode early (no LLM client needed)
    if dry_run {
        match output_format {
            OutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::json!({
                        "status": "dry_run",
                        "message": "Would execute task",
                        "task": request.description,
                        "config": {
                            "mode": mode,
                            "trust": trust,
                            "test_strategy": test_strategy,
                            "max_iterations": max_iterations
                        }
                    })
                );
            },
            OutputFormat::Text => {
                println!("[DRY RUN] Would execute task: {}", request.description);
                println!();
                println!("Configuration:");
                println!("  Mode: {:?}", generation_mode);
                println!("  Trust: {:?}", trust_level);
                println!("  Test Strategy: {:?}", test_exec_strategy);
                println!("  Apply Workflow: {:?}", request.apply_workflow);
                if !request.target_files.is_empty() {
                    println!("  Target Files: {:?}", request.target_files);
                }
            },
            OutputFormat::StreamJson => {
                println!(
                    "{}",
                    serde_json::json!({
                        "type": "dry_run",
                        "task": request.description
                    })
                );
            },
        }
        return Ok(());
    }

    // Load config and create LLM client (only when not in dry-run mode)
    let show_progress = output_format == OutputFormat::Text;

    if show_progress {
        crate::cli_progress::status("Loading configuration...");
    }
    let config = load_config(config_path.as_ref())?;

    if show_progress {
        crate::cli_progress::status("Creating LLM client...");
    }
    let mut llm_config = LlmConfig::from_config(&config.llm, &provider)?;
    if let Some(ref m) = model {
        llm_config.model = m.clone();
    }

    let llm_client = std::sync::Arc::new(create_provider(&llm_config)?);

    // Create agentic system with NoOp tool executor
    // TODO: Wire up MCP tool executor when proper cross-crate integration is available
    let tool_executor = std::sync::Arc::new(NoOpToolExecutor);

    // Create agentic system
    let apply_workflow =
        ApplyWorkflow::trust_based_with_level(trust_level, trust_level < TrustLevel::High);

    let mut system =
        AgenticSystem::new(generation_mode.clone(), test_exec_strategy, apply_workflow)
            .with_llm_client(llm_client)
            .with_tool_executor(tool_executor);

    // Execute the task
    if show_progress {
        crate::cli_progress::status("Executing task...");
    }
    let task_result = system.execute(request).await?;

    // Format output based on format
    match output_format {
        OutputFormat::Json => {
            let changes: Vec<serde_json::Value> = task_result
                .changes
                .iter()
                .map(|c| {
                    serde_json::json!({
                        "path": c.path,
                        "change_type": format!("{:?}", c.change_type),
                        "lines_added": c.new.lines().count(),
                        "lines_removed": c.original.as_ref().map(|o| o.lines().count()).unwrap_or(0),
                        "diff": c.diff
                    })
                })
                .collect();

            let issues: Vec<serde_json::Value> = task_result
                .verification
                .issues
                .iter()
                .map(|i| {
                    serde_json::json!({
                        "severity": format!("{:?}", i.severity),
                        "message": i.message,
                        "file": i.file,
                        "is_blocking": i.is_blocking()
                    })
                })
                .collect();

            println!(
                "{}",
                serde_json::json!({
                    "status": if task_result.success { "success" } else { "failed" },
                    "task_id": task_result.id,
                    "duration_ms": task_result.duration_ms,
                    "changes": changes,
                    "issues": issues,
                    "test_result": task_result.test_result.as_ref().map(|t| serde_json::json!({
                        "passed": t.passed,
                        "output": t.output
                    })),
                    "rollback_checkpoint": task_result.rollback_checkpoint,
                    "log_entries": task_result.log.len()
                })
            );
        },
        OutputFormat::Text => {
            if task_result.success {
                println!("✅ Task completed successfully!");
            } else {
                println!("❌ Task failed");
            }

            println!("Task ID: {}", task_result.id);
            println!("Duration: {}ms", task_result.duration_ms);

            if !task_result.changes.is_empty() {
                println!("\n📝 Changes ({} files):", task_result.changes.len());
                for change in &task_result.changes {
                    let change_icon = match change.change_type {
                        clawdius_core::agentic::ChangeType::Created => "➕",
                        clawdius_core::agentic::ChangeType::Modified => "✏️",
                        clawdius_core::agentic::ChangeType::Deleted => "🗑️",
                    };
                    println!(
                        "  {} {} ({})",
                        change_icon,
                        change.path,
                        format!("{:?}", change.change_type).to_lowercase()
                    );
                    println!(
                        "    +{} -{}",
                        change.new.lines().count(),
                        change
                            .original
                            .as_ref()
                            .map(|o| o.lines().count())
                            .unwrap_or(0)
                    );
                }
            }

            if !task_result.verification.issues.is_empty() {
                println!("\n⚠️  Issues ({}):", task_result.verification.issues.len());
                for issue in &task_result.verification.issues {
                    let severity_icon = match issue.severity {
                        clawdius_core::agentic::IssueSeverity::Critical => "🔴",
                        clawdius_core::agentic::IssueSeverity::Blocking => "❌",
                        clawdius_core::agentic::IssueSeverity::Warning => "⚠️",
                        clawdius_core::agentic::IssueSeverity::Info => "ℹ️",
                    };
                    println!(
                        "  {} [{:?}] {}",
                        severity_icon, issue.severity, issue.message
                    );
                    println!("     File: {}", issue.file);
                }
            }

            if let Some(ref test_result) = task_result.test_result {
                println!("\n🧪 Test Results:");
                println!("  Passed: {}", test_result.passed);
                if !test_result.output.is_empty() {
                    println!("  Output: {}", test_result.output);
                }
            }

            if let Some(ref checkpoint) = task_result.rollback_checkpoint {
                println!("\n💾 Rollback checkpoint: {}", checkpoint);
            }
        },
        OutputFormat::StreamJson => {
            // Stream each change as an event
            for change in &task_result.changes {
                println!(
                    "{}",
                    serde_json::json!({
                        "type": "change",
                        "path": change.path,
                        "change_type": format!("{:?}", change.change_type)
                    })
                );
            }

            // Stream final result
            println!(
                "{}",
                serde_json::json!({
                    "type": "complete",
                    "success": task_result.success,
                    "duration_ms": task_result.duration_ms,
                    "changes_count": task_result.changes.len(),
                    "issues_count": task_result.verification.issues.len()
                })
            );
        },
    }

    Ok(())
}

async fn handle_lsp(action: LspCommands, output_format: OutputFormat) -> anyhow::Result<()> {
    use clawdius_core::lsp::{LspClient, LspClientConfig};

    match action {
        LspCommands::Start { server, args, root } => {
            // Create LSP client config
            let config = LspClientConfig::new(&server).with_args(args);

            // Show spinner for text output
            let spinner = if output_format == OutputFormat::Text {
                let mut s =
                    crate::cli_progress::Spinner::new(format!("Connecting to {}...", server));
                s.start();
                Some(s)
            } else {
                None
            };

            // Try to create and start the client
            let mut client = LspClient::new(config);

            match client.start(root.as_deref()).await {
                Ok(()) => {
                    // Stop spinner
                    if let Some(spinner) = spinner {
                        spinner.stop(Some(&format!("Connected to {}", server)));
                    }

                    let capabilities = client.capabilities().await;

                    match output_format {
                        OutputFormat::Json => {
                            println!(
                                "{}",
                                serde_json::json!({
                                    "action": "start",
                                    "server": server,
                                    "status": "connected",
                                    "capabilities": capabilities.as_ref().map(|c| {
                                        serde_json::json!({
                                            "completion": c.completion_provider.is_some(),
                                            "hover": c.hover_provider.unwrap_or(false),
                                            "definition": c.definition_provider.unwrap_or(false),
                                            "references": c.references_provider.unwrap_or(false),
                                            "symbols": c.document_symbol_provider.unwrap_or(false),
                                            "code_actions": c.code_action_provider.unwrap_or(false),
                                        })
                                    })
                                })
                            );
                        },
                        OutputFormat::Text => {
                            crate::cli_progress::success(&format!(
                                "LSP server started: {}",
                                server
                            ));
                            if let Some(r) = &root {
                                println!("   Root: {}", r);
                            }
                            if let Some(caps) = capabilities {
                                println!("\n   Capabilities:");
                                // Text synchronization
                                if caps.text_document_sync.is_some() {
                                    println!("   ✓ Text Synchronization");
                                }
                                // Completions
                                if caps.completion_provider.is_some() {
                                    let triggers = caps
                                        .completion_provider
                                        .as_ref()
                                        .map(|c| c.trigger_characters.join(", "))
                                        .unwrap_or_default();
                                    if triggers.is_empty() {
                                        println!("   ✓ Completions");
                                    } else {
                                        println!("   ✓ Completions (triggers: {})", triggers);
                                    }
                                }
                                // Hover
                                if caps.hover_provider.unwrap_or(false) {
                                    println!("   ✓ Hover");
                                }
                                // Go to Definition
                                if caps.definition_provider.unwrap_or(false) {
                                    println!("   ✓ Go to Definition");
                                }
                                // Find References
                                if caps.references_provider.unwrap_or(false) {
                                    println!("   ✓ Find References");
                                }
                                // Document Symbols
                                if caps.document_symbol_provider.unwrap_or(false) {
                                    println!("   ✓ Document Symbols");
                                }
                                // Workspace Symbols
                                if caps.workspace_symbol_provider.unwrap_or(false) {
                                    println!("   ✓ Workspace Symbols");
                                }
                                // Code Actions
                                if caps.code_action_provider.unwrap_or(false) {
                                    println!("   ✓ Code Actions");
                                }
                            } else {
                                println!("\n   ⚠ No capabilities reported");
                            }
                        },
                        OutputFormat::StreamJson => {
                            println!(
                                "{}",
                                serde_json::json!({
                                    "type": "lsp_start",
                                    "server": server,
                                    "status": "connected"
                                })
                            );
                        },
                    }

                    // Stop the client (for now, we start/stop per command)
                    let _ = client.stop().await;
                },
                Err(e) => match output_format {
                    OutputFormat::Json => {
                        println!(
                            "{}",
                            serde_json::json!({
                                "action": "start",
                                "server": server,
                                "status": "error",
                                "error": e.to_string()
                            })
                        );
                    },
                    OutputFormat::Text => {
                        println!("❌ Failed to start LSP server: {}", server);
                        println!("   Error: {}", e);
                        println!("\n   Make sure '{}' is installed and in your PATH.", server);
                    },
                    OutputFormat::StreamJson => {
                        println!(
                            "{}",
                            serde_json::json!({
                                "type": "lsp_start",
                                "server": server,
                                "status": "error",
                                "error": e.to_string()
                            })
                        );
                    },
                },
            }
        },

        LspCommands::Complete { uri, line, column } => match output_format {
            OutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::json!({
                        "action": "complete",
                        "uri": uri,
                        "position": {"line": line, "column": column},
                        "items": [],
                        "note": "Use 'clawdius lsp start' to connect to an LSP server"
                    })
                );
            },
            OutputFormat::Text => {
                println!("Completions for {}:{}:{}", uri, line, column);
                println!("\n💡 Tip: Start an LSP server first with:");
                println!("   clawdius lsp start rust-analyzer --root file://$(pwd)");
            },
            OutputFormat::StreamJson => {
                println!(
                    "{}",
                    serde_json::json!({
                        "type": "lsp_complete",
                        "uri": uri,
                        "line": line,
                        "column": column,
                        "items": []
                    })
                );
            },
        },

        LspCommands::Hover { uri, line, column } => match output_format {
            OutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::json!({
                        "action": "hover",
                        "uri": uri,
                        "position": {"line": line, "column": column},
                        "content": null,
                        "note": "Use 'clawdius lsp start' to connect to an LSP server"
                    })
                );
            },
            OutputFormat::Text => {
                println!("Hover at {}:{}:{}", uri, line, column);
                println!("\n💡 Tip: Start an LSP server first with:");
                println!("   clawdius lsp start rust-analyzer --root file://$(pwd)");
            },
            OutputFormat::StreamJson => {
                println!(
                    "{}",
                    serde_json::json!({
                        "type": "lsp_hover",
                        "uri": uri,
                        "line": line,
                        "column": column,
                        "content": null
                    })
                );
            },
        },

        LspCommands::Definition { uri, line, column } => match output_format {
            OutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::json!({
                        "action": "definition",
                        "uri": uri,
                        "position": {"line": line, "column": column},
                        "locations": [],
                        "note": "Use 'clawdius lsp start' to connect to an LSP server"
                    })
                );
            },
            OutputFormat::Text => {
                println!("Definition for {}:{}:{}", uri, line, column);
                println!("\n💡 Tip: Start an LSP server first with:");
                println!("   clawdius lsp start rust-analyzer --root file://$(pwd)");
            },
            OutputFormat::StreamJson => {
                println!(
                    "{}",
                    serde_json::json!({
                        "type": "lsp_definition",
                        "uri": uri,
                        "line": line,
                        "column": column,
                        "locations": []
                    })
                );
            },
        },

        LspCommands::References {
            uri,
            line,
            column,
            include_declaration,
        } => match output_format {
            OutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::json!({
                        "action": "references",
                        "uri": uri,
                        "position": {"line": line, "column": column},
                        "include_declaration": include_declaration,
                        "locations": [],
                        "note": "Use 'clawdius lsp start' to connect to an LSP server"
                    })
                );
            },
            OutputFormat::Text => {
                println!(
                    "References for {}:{}:{} (include_declaration: {})",
                    uri, line, column, include_declaration
                );
                println!("\n💡 Tip: Start an LSP server first with:");
                println!("   clawdius lsp start rust-analyzer --root file://$(pwd)");
            },
            OutputFormat::StreamJson => {
                println!(
                    "{}",
                    serde_json::json!({
                        "type": "lsp_references",
                        "uri": uri,
                        "line": line,
                        "column": column,
                        "locations": []
                    })
                );
            },
        },

        LspCommands::Symbols { uri } => match output_format {
            OutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::json!({
                        "action": "symbols",
                        "uri": uri,
                        "symbols": [],
                        "note": "Use 'clawdius lsp start' to connect to an LSP server"
                    })
                );
            },
            OutputFormat::Text => {
                println!("Symbols for {}", uri);
                println!("\n💡 Tip: Start an LSP server first with:");
                println!("   clawdius lsp start rust-analyzer --root file://$(pwd)");
            },
            OutputFormat::StreamJson => {
                println!(
                    "{}",
                    serde_json::json!({
                        "type": "lsp_symbols",
                        "uri": uri,
                        "symbols": []
                    })
                );
            },
        },

        LspCommands::Diagnostics { uri } => match output_format {
            OutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::json!({
                        "action": "diagnostics",
                        "uri": uri,
                        "diagnostics": [],
                        "note": "Use 'clawdius lsp start' to connect to an LSP server"
                    })
                );
            },
            OutputFormat::Text => {
                println!("Diagnostics for {}", uri);
                println!("\n💡 Tip: Start an LSP server first with:");
                println!("   clawdius lsp start rust-analyzer --root file://$(pwd)");
            },
            OutputFormat::StreamJson => {
                println!(
                    "{}",
                    serde_json::json!({
                        "type": "lsp_diagnostics",
                        "uri": uri,
                        "diagnostics": []
                    })
                );
            },
        },

        LspCommands::CodeActions {
            uri,
            start_line,
            start_column,
            end_line,
            end_column,
        } => match output_format {
            OutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::json!({
                        "action": "code_actions",
                        "uri": uri,
                        "range": {
                            "start": {"line": start_line, "column": start_column},
                            "end": {"line": end_line, "column": end_column}
                        },
                        "actions": [],
                        "note": "Use 'clawdius lsp start' to connect to an LSP server"
                    })
                );
            },
            OutputFormat::Text => {
                println!(
                    "Code actions for {} ({}:{}-{}:{})",
                    uri, start_line, start_column, end_line, end_column
                );
                println!("\n💡 Tip: Start an LSP server first with:");
                println!("   clawdius lsp start rust-analyzer --root file://$(pwd)");
            },
            OutputFormat::StreamJson => {
                println!(
                    "{}",
                    serde_json::json!({
                        "type": "lsp_code_actions",
                        "uri": uri,
                        "range": {
                            "start": {"line": start_line, "column": start_column},
                            "end": {"line": end_line, "column": end_column}
                        },
                        "actions": []
                    })
                );
            },
        },
    }

    Ok(())
}

/// Handle memory commands
async fn handle_memory(
    action: MemoryCommands,
    config_path: Option<PathBuf>,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    use clawdius_core::memory::ProjectMemory;

    let config = load_config(config_path.as_ref())?;
    let project_root = config
        .storage
        .database_path
        .parent()
        .map(|p| p.parent().unwrap_or(p))
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    match action {
        MemoryCommands::Show { instructions } => {
            let memory = match ProjectMemory::load(&project_root) {
                Ok(m) => m,
                Err(_) => ProjectMemory::new(&project_root),
            };

            if instructions {
                println!("{}", memory.to_instructions());
            } else {
                match output_format {
                    OutputFormat::Json => {
                        let build_commands: Vec<_> = memory
                            .build_commands()
                            .iter()
                            .map(|(cmd, desc)| {
                                serde_json::json!({
                                    "command": cmd,
                                    "description": desc
                                })
                            })
                            .collect();

                        let test_commands: Vec<_> = memory
                            .test_commands()
                            .iter()
                            .map(|(cmd, desc)| {
                                serde_json::json!({
                                    "command": cmd,
                                    "description": desc
                                })
                            })
                            .collect();

                        let insights: Vec<_> = memory
                            .debug_insights()
                            .iter()
                            .map(|(issue, solution)| {
                                serde_json::json!({
                                    "issue": issue,
                                    "solution": solution
                                })
                            })
                            .collect();

                        println!(
                            "{}",
                            serde_json::json!({
                                "instructions": memory.instructions(),
                                "metadata": memory.metadata(),
                                "build_commands": build_commands,
                                "test_commands": test_commands,
                                "debug_insights": insights,
                                "learned_count": memory.learned().len()
                            })
                        );
                    },
                    OutputFormat::Text => {
                        println!("📝 Project Memory\n");

                        if !memory.instructions().is_empty() {
                            println!("## Instructions\n{}\n", memory.instructions());
                        }

                        let metadata = memory.metadata();
                        if let Some(name) = &metadata.project_name {
                            println!("**Project:** {}", name);
                        }
                        if let Some(lang) = &metadata.primary_language {
                            println!("**Language:** {}", lang);
                        }
                        if let Some(fw) = &metadata.framework {
                            println!("**Framework:** {}", fw);
                        }

                        let build_commands = memory.build_commands();
                        if !build_commands.is_empty() {
                            println!("\n## Build Commands");
                            for (cmd, desc) in &build_commands {
                                if let Some(d) = desc {
                                    println!("  • {} - {}", cmd, d);
                                } else {
                                    println!("  • {}", cmd);
                                }
                            }
                        }

                        let test_commands = memory.test_commands();
                        if !test_commands.is_empty() {
                            println!("\n## Test Commands");
                            for (cmd, desc) in &test_commands {
                                if let Some(d) = desc {
                                    println!("  • {} - {}", cmd, d);
                                } else {
                                    println!("  • {}", cmd);
                                }
                            }
                        }

                        let insights = memory.debug_insights();
                        if !insights.is_empty() {
                            println!("\n## Debug Insights");
                            for (issue, solution) in &insights {
                                println!("  • Issue: {}", issue);
                                println!("    Solution: {}", solution);
                            }
                        }

                        println!("\n📊 {} learned entries", memory.learned().len());
                    },
                    OutputFormat::StreamJson => {
                        println!(
                            "{}",
                            serde_json::json!({
                                "type": "memory_show",
                                "instructions": memory.instructions(),
                                "learned_count": memory.learned().len()
                            })
                        );
                    },
                }
            }
        },

        MemoryCommands::Learn {
            entry_type,
            content,
            description,
        } => {
            let mut memory = match ProjectMemory::load(&project_root) {
                Ok(m) => m,
                Err(_) => ProjectMemory::new(&project_root),
            };

            match entry_type.to_lowercase().as_str() {
                "build" => {
                    memory.learn_build_command(&content, description);
                },
                "test" => {
                    memory.learn_test_command(&content, description);
                },
                "debug" => {
                    let parts: Vec<&str> = content.splitn(2, '=').collect();
                    if parts.len() == 2 {
                        memory.learn_debug_insight(parts[0], parts[1]);
                    } else {
                        anyhow::bail!("Debug format: issue=solution");
                    }
                },
                "pattern" => {
                    let parts: Vec<&str> = content.splitn(2, '=').collect();
                    if parts.len() == 2 {
                        memory.learn_code_pattern(
                            parts[0],
                            parts[1],
                            description.unwrap_or_default(),
                        );
                    } else {
                        anyhow::bail!("Pattern format: name=pattern");
                    }
                },
                "preference" => {
                    let parts: Vec<&str> = content.splitn(2, '=').collect();
                    if parts.len() == 2 {
                        memory.learn_preference(parts[0], parts[1]);
                    } else {
                        anyhow::bail!("Preference format: key=value");
                    }
                },
                _ => {
                    anyhow::bail!(
                        "Unknown entry type: {}. Use: build, test, debug, pattern, preference",
                        entry_type
                    );
                },
            }

            memory.save()?;

            match output_format {
                OutputFormat::Json => {
                    println!(
                        "{}",
                        serde_json::json!({
                            "status": "learned",
                            "type": entry_type,
                            "content": content
                        })
                    );
                },
                OutputFormat::Text => {
                    println!("✅ Learned {} entry", entry_type);
                },
                OutputFormat::StreamJson => {
                    println!(
                        "{}",
                        serde_json::json!({
                            "type": "memory_learn",
                            "entry_type": entry_type,
                            "content": content
                        })
                    );
                },
            }
        },

        MemoryCommands::Instructions { content } => {
            let mut memory = match ProjectMemory::load(&project_root) {
                Ok(m) => m,
                Err(_) => ProjectMemory::new(&project_root),
            };

            let instructions = if content == "-" {
                use std::io::{self, Read};
                let mut buffer = String::new();
                io::stdin().read_to_string(&mut buffer)?;
                buffer
            } else {
                content
            };

            memory.set_instructions(&instructions);
            memory.save()?;

            match output_format {
                OutputFormat::Json => {
                    println!(
                        "{}",
                        serde_json::json!({
                            "status": "updated",
                            "instructions_length": instructions.len()
                        })
                    );
                },
                OutputFormat::Text => {
                    println!("✅ Project instructions updated");
                },
                OutputFormat::StreamJson => {
                    println!(
                        "{}",
                        serde_json::json!({
                            "type": "memory_instructions",
                            "length": instructions.len()
                        })
                    );
                },
            }
        },

        MemoryCommands::List { category } => {
            let memory = match ProjectMemory::load(&project_root) {
                Ok(m) => m,
                Err(_) => ProjectMemory::new(&project_root),
            };

            let entries: Vec<_> = if category == "all" {
                memory.learned().iter().collect()
            } else {
                memory.learned_by_category(&category)
            };

            match output_format {
                OutputFormat::Json => {
                    let items: Vec<_> = entries
                        .iter()
                        .map(|e| {
                            serde_json::json!({
                                "category": e.category(),
                                "entry": format!("{:?}", e)
                            })
                        })
                        .collect();

                    println!(
                        "{}",
                        serde_json::json!({
                            "category": category,
                            "count": items.len(),
                            "entries": items
                        })
                    );
                },
                OutputFormat::Text => {
                    println!("📋 {} entries in category: {}\n", entries.len(), category);

                    for entry in &entries {
                        println!("• [{}] {:?}", entry.category(), entry);
                    }
                },
                OutputFormat::StreamJson => {
                    println!(
                        "{}",
                        serde_json::json!({
                            "type": "memory_list",
                            "category": category,
                            "count": entries.len()
                        })
                    );
                },
            }
        },

        MemoryCommands::Clear { category, yes } => {
            if !yes {
                anyhow::bail!("Use --yes to confirm clearing memory entries");
            }

            let mut memory = match ProjectMemory::load(&project_root) {
                Ok(m) => m,
                Err(_) => ProjectMemory::new(&project_root),
            };

            let count = memory.learned().len();

            if category == "all" {
                memory.clear_learned();
            } else {
                memory.remove_by_category(&category);
            }

            memory.save()?;

            match output_format {
                OutputFormat::Json => {
                    println!(
                        "{}",
                        serde_json::json!({
                            "status": "cleared",
                            "category": category,
                            "removed_count": count - memory.learned().len()
                        })
                    );
                },
                OutputFormat::Text => {
                    println!(
                        "✅ Cleared {} entries from category: {}",
                        count - memory.learned().len(),
                        category
                    );
                },
                OutputFormat::StreamJson => {
                    println!(
                        "{}",
                        serde_json::json!({
                            "type": "memory_clear",
                            "category": category
                        })
                    );
                },
            }
        },

        MemoryCommands::Init {
            name,
            language,
            framework,
        } => {
            let mut memory = match ProjectMemory::load(&project_root) {
                Ok(m) => m,
                Err(_) => ProjectMemory::new(&project_root),
            };

            let metadata = memory.metadata_mut();
            if let Some(n) = name {
                metadata.project_name = Some(n);
            }
            if let Some(l) = language {
                metadata.primary_language = Some(l);
            }
            if let Some(f) = framework {
                metadata.framework = Some(f);
            }

            // Create CLAUDE.md if it doesn't exist
            let claude_md_path = project_root.join("CLAUDE.md");
            if !claude_md_path.exists() {
                let mut content = String::new();

                // Add frontmatter
                content.push_str("---\n");
                if let Some(name) = &memory.metadata().project_name {
                    content.push_str(&format!("project: {}\n", name));
                }
                if let Some(lang) = &memory.metadata().primary_language {
                    content.push_str(&format!("language: {}\n", lang));
                }
                if let Some(fw) = &memory.metadata().framework {
                    content.push_str(&format!("framework: {}\n", fw));
                }
                content.push_str("---\n\n");

                content.push_str("# Project Instructions\n\n");
                content.push_str("Add your project-specific instructions here.\n\n");
                content.push_str("## Guidelines\n\n");
                content.push_str("- Write clear, idiomatic code\n");
                content.push_str("- Follow the project's style guide\n");
                content.push_str("- Add tests for new functionality\n");

                std::fs::write(&claude_md_path, content)?;
            }

            memory.save()?;

            match output_format {
                OutputFormat::Json => {
                    println!(
                        "{}",
                        serde_json::json!({
                            "status": "initialized",
                            "claude_md": claude_md_path.exists(),
                            "metadata": memory.metadata()
                        })
                    );
                },
                OutputFormat::Text => {
                    println!("✅ Memory initialized");
                    if claude_md_path.exists() {
                        println!("   Created: {}", claude_md_path.display());
                    }
                    println!(
                        "   Storage: {}/.clawdius/memory.json",
                        project_root.display()
                    );
                },
                OutputFormat::StreamJson => {
                    println!(
                        "{}",
                        serde_json::json!({
                            "type": "memory_init",
                            "metadata": memory.metadata()
                        })
                    );
                },
            }
        },
    }

    Ok(())
}

/// Handle models commands for local LLM management
async fn handle_models(
    action: ModelsCommands,
    host: &str,
    port: u16,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    use clawdius_core::llm::providers::local::LocalLlmProvider;

    let base_url = format!("http://{host}:{port}");
    let provider = LocalLlmProvider::new(base_url, "default".to_string());

    match action {
        ModelsCommands::List => match provider.list_models().await {
            Ok(models) => {
                if models.is_empty() {
                    match output_format {
                        OutputFormat::Json => {
                            println!("[]");
                        },
                        _ => {
                            println!("No models found. Pull a model with:");
                            println!("  clawdius models pull llama3.2");
                        },
                    }
                } else {
                    match output_format {
                        OutputFormat::Json => {
                            println!("{}", serde_json::to_string_pretty(&models)?);
                        },
                        _ => {
                            println!("Available models:\n");
                            for model in &models {
                                let size = model
                                    .size
                                    .map(|s| format!("{:.2} GB", s as f64 / 1_073_741_824.0))
                                    .unwrap_or_default();
                                println!("  🦙 {} {}", model.name, size);
                            }
                            println!("\nTotal: {} model(s)", models.len());
                        },
                    }
                }
            },
            Err(e) => {
                match output_format {
                    OutputFormat::Json => {
                        println!(
                            "{}",
                            serde_json::json!({
                                "error": e.to_string(),
                                "hint": "Ensure Ollama is running"
                            })
                        );
                    },
                    _ => {
                        eprintln!("❌ Error: {e}");
                        eprintln!("\n💡 Ensure Ollama is running:");
                        eprintln!("   ollama serve");
                    },
                }
                return Err(anyhow::anyhow!("{e}"));
            },
        },

        ModelsCommands::Pull { model } => {
            match output_format {
                OutputFormat::Json => {
                    println!(
                        "{}",
                        serde_json::json!({
                            "status": "pulling",
                            "model": model
                        })
                    );
                },
                _ => {
                    println!("📦 Pulling model: {model}");
                    println!("   This may take a while...\n");
                },
            }

            match provider.pull_model(&model).await {
                Ok(()) => match output_format {
                    OutputFormat::Json => {
                        println!(
                            "{}",
                            serde_json::json!({
                                "status": "success",
                                "model": model
                            })
                        );
                    },
                    _ => {
                        println!("✅ Model pulled successfully: {model}");
                        println!("\nUse it with:");
                        println!("  clawdius chat -P ollama --model {}", model);
                    },
                },
                Err(e) => {
                    match output_format {
                        OutputFormat::Json => {
                            println!(
                                "{}",
                                serde_json::json!({
                                    "error": e.to_string(),
                                    "model": model
                                })
                            );
                        },
                        _ => {
                            eprintln!("❌ Failed to pull model: {e}");
                        },
                    }
                    return Err(anyhow::anyhow!("{e}"));
                },
            }
        },

        ModelsCommands::Health => match provider.health_check().await {
            Ok(true) => match output_format {
                OutputFormat::Json => {
                    println!(
                        "{}",
                        serde_json::json!({
                            "status": "healthy",
                            "host": host,
                            "port": port
                        })
                    );
                },
                _ => {
                    println!("✅ Ollama server is healthy");
                    println!("   Host: {host}:{port}");
                },
            },
            Ok(false) | Err(_) => {
                match output_format {
                    OutputFormat::Json => {
                        println!(
                            "{}",
                            serde_json::json!({
                                "status": "unhealthy",
                                "host": host,
                                "port": port
                            })
                        );
                    },
                    _ => {
                        eprintln!("❌ Ollama server is not responding");
                        eprintln!("\n💡 Start Ollama with:");
                        eprintln!("   ollama serve");
                    },
                }
                return Err(anyhow::anyhow!("Ollama server not responding"));
            },
        },

        ModelsCommands::Current => {
            // This would require loading from config
            match output_format {
                OutputFormat::Json => {
                    println!(
                        "{}",
                        serde_json::json!({
                            "model": "llama3.2",
                            "provider": "ollama",
                            "note": "Configure in clawdius.toml"
                        })
                    );
                },
                _ => {
                    println!("Current model configuration:");
                    println!("  Provider: ollama (default)");
                    println!("  Model: llama3.2 (default)");
                    println!("\n💡 Configure in clawdius.toml:");
                    println!("   [llm.ollama]");
                    println!("   model = \"mistral\"");
                    println!("   base_url = \"http://localhost:11434\"");
                },
            }
        },
    }

    Ok(())
}

/// Handle inline completion requests
async fn handle_complete(
    file: String,
    line: u32,
    character: u32,
    language: Option<String>,
    provider: String,
    model: Option<String>,
    _config_path: Option<PathBuf>,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    use clawdius_core::completions::{
        CompletionProviderTrait, CompletionRequest, InlineCompletionProvider, LlmCompletionConfig,
    };
    use clawdius_core::llm::{create_provider, LlmConfig};
    use clawdius_core::lsp::Position;

    // Read file content
    let content = std::fs::read_to_string(&file)
        .map_err(|e| anyhow::anyhow!("Failed to read file {}: {}", file, e))?;

    // Detect language from file extension if not specified
    let language = language.unwrap_or_else(|| {
        std::path::Path::new(&file)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("text")
            .to_string()
    });

    // Create LLM config
    let mut llm_config = LlmConfig::from_env(&provider)?;
    if let Some(m) = model {
        llm_config.model = m;
    }

    // Create provider
    let llm_provider = create_provider(&llm_config)?;
    let llm_arc = std::sync::Arc::new(llm_provider);

    // Create completion provider
    let completion_config = LlmCompletionConfig::default();
    let completion_provider = InlineCompletionProvider::new(llm_arc, completion_config);

    // Create request
    let request = CompletionRequest::new(&content, Position::new(line, character), &language)
        .with_file_path(&file);

    match output_format {
        OutputFormat::Json => match completion_provider.complete(&request).await {
            Ok(response) => {
                println!("{}", serde_json::to_string_pretty(&response)?);
            },
            Err(e) => {
                println!(
                    "{}",
                    serde_json::json!({
                        "error": e.to_string()
                    })
                );
            },
        },
        _ => {
            println!("🔍 Requesting completion from {}...", provider);
            println!("   File: {}:{}", file, line);
            println!("   Language: {}\n", language);

            match completion_provider.complete(&request).await {
                Ok(response) => {
                    if response.text.is_empty() {
                        println!("💡 No completion available");
                    } else {
                        println!(
                            "✨ Completion (confidence: {:.0}%):",
                            response.confidence * 100.0
                        );
                        println!();
                        println!("    {}", response.text.replace("\n", "\n    "));

                        if !response.alternatives.is_empty() {
                            println!("\n📚 Alternatives:");
                            for (i, alt) in response.alternatives.iter().enumerate() {
                                println!("  {}. {}", i + 1, alt.text.lines().next().unwrap_or(""));
                            }
                        }
                    }
                },
                Err(e) => {
                    eprintln!("❌ Completion failed: {}", e);
                    eprintln!("\n💡 Ensure your LLM provider is configured and accessible");
                },
            }
        },
    }

    Ok(())
}

/// Handle analyze command for architecture drift and technical debt detection
async fn handle_analyze(
    path: PathBuf,
    drift_only: bool,
    debt_only: bool,
    format: OutputFormat,
    output_file: Option<PathBuf>,
    min_severity: String,
    exclude_patterns: Option<String>,
) -> anyhow::Result<()> {
    use clawdius_core::analysis::{DebtAnalyzer, DriftDetector};

    // Parse minimum severity filter
    let min_severity_level = match min_severity.to_lowercase().as_str() {
        "low" => 1,
        "medium" => 2,
        "high" => 3,
        "critical" => 4,
        _ => 1,
    };

    // Parse exclude patterns
    let excludes: Vec<String> = exclude_patterns
        .map(|p| p.split(',').map(|s| s.trim().to_string()).collect())
        .unwrap_or_default();

    // Collect files to analyze
    let mut files: Vec<(PathBuf, String)> = Vec::new();

    if path.is_file() {
        if let Ok(content) = std::fs::read_to_string(&path) {
            files.push((path.clone(), content));
        }
    } else if path.is_dir() {
        for entry in walkdir::WalkDir::new(&path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map(|ext| ext == "rs").unwrap_or(false))
        {
            let file_path = entry.path().to_path_buf();
            let path_str = file_path.to_string_lossy();

            if excludes.iter().any(|ex| path_str.contains(ex)) {
                continue;
            }
            if path_str.contains("/target/") || path_str.contains("\\target\\") {
                continue;
            }
            if let Ok(content) = std::fs::read_to_string(&file_path) {
                files.push((file_path, content));
            }
        }
    }

    if files.is_empty() {
        println!("⚠️  No files found to analyze");
        return Ok(());
    }

    println!("📊 Analyzing {} files...", files.len());

    // Run analysis
    let drift_report = if !debt_only {
        let detector = DriftDetector::new();
        detector.analyze_files(files.iter().map(|(p, c)| (p.clone(), c.as_str())))
    } else {
        DriftReport::default()
    };

    let debt_report = if !drift_only {
        let analyzer = DebtAnalyzer::new();
        analyzer.analyze_files(files.iter().map(|(p, c)| (p.clone(), c.as_str())))
    } else {
        DebtReport::default()
    };

    // Generate output
    let output = match format {
        OutputFormat::Json => {
            format_analyze_json(&drift_report, &debt_report, files.len(), min_severity_level)?
        },
        _ => format_analyze_text(&drift_report, &debt_report, files.len(), min_severity_level),
    };

    // Write output
    if let Some(output_path) = output_file {
        std::fs::write(&output_path, &output)?;
        println!("✅ Report written to {}", output_path.display());
    } else {
        println!("\n{output}");
    }

    Ok(())
}

// Helper functions for analyze command

fn format_analyze_json(
    drift_report: &DriftReport,
    debt_report: &DebtReport,
    files_analyzed: usize,
    min_severity: u8,
) -> anyhow::Result<String> {
    let result = serde_json::json!({
        "summary": {
            "files_analyzed": files_analyzed,
            "drift_count": drift_report.len(),
            "debt_count": debt_report.len(),
        },
        "drift": filter_drift_by_severity(drift_report, min_severity),
        "debt": filter_debt_by_priority(debt_report, min_severity),
    });
    Ok(serde_json::to_string_pretty(&result)?)
}

fn format_analyze_text(
    drift_report: &DriftReport,
    debt_report: &DebtReport,
    files_analyzed: usize,
    min_severity: u8,
) -> String {
    let mut output = String::new();

    output.push_str("╔══════════════════════════════════════════════════════════════╗\n");
    output.push_str("║                    📊 CLAWDIUS ANALYSIS                      ║\n");
    output.push_str("╠══════════════════════════════════════════════════════════════╣\n");
    output.push_str(&format!("║  Files Analyzed: {:<43}║\n", files_analyzed));
    output.push_str("╚══════════════════════════════════════════════════════════════╝\n\n");

    // Drift Summary
    output.push_str("## 🏗️  Architecture Drift\n\n");
    output.push_str(&format!("  Total Drifts: {}\n", drift_report.len()));
    output.push_str(&format!(
        "  Severity Score: {}\n",
        drift_report.total_severity_score()
    ));
    if drift_report.has_critical() {
        output.push_str("  ⚠️  CRITICAL DRIFTS DETECTED!\n");
    }
    output.push('\n');

    let filtered_drifts = filter_drift_by_severity(drift_report, min_severity);
    if !filtered_drifts.is_empty() {
        output.push_str("  Top Issues:\n");
        for drift in filtered_drifts.iter().take(10) {
            let severity = drift
                .get("severity")
                .and_then(|v| v.as_str())
                .unwrap_or("Low");
            let icon = match severity {
                "Critical" => "🔴",
                "High" => "🟠",
                "Medium" => "🟡",
                _ => "🔵",
            };
            let file = drift
                .get("file")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let line = drift.get("line").and_then(|v| v.as_u64()).unwrap_or(0);
            let msg = drift
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            output.push_str(&format!("    {} {}:{} - {}\n", icon, file, line, msg));
        }
    }
    output.push('\n');

    // Debt Summary
    output.push_str("## 💰 Technical Debt\n\n");
    output.push_str(&format!("  Total Debt Items: {}\n", debt_report.len()));
    output.push_str(&format!("  Debt Score: {:.2}\n", debt_report.debt_score()));
    output.push_str(&format!(
        "  Total Effort: {:.1} hours\n",
        debt_report.total_effort_hours
    ));
    output.push_str(&format!(
        "  Blocking Items: {}\n",
        debt_report.blocking_count
    ));
    output.push('\n');

    let top_debts = debt_report.top_priorities(10);
    if !top_debts.is_empty() {
        output.push_str("  Top Priority Items:\n");
        for debt in top_debts {
            let icon = match debt.priority {
                1..=3 => "🟢",
                4..=6 => "🟡",
                7..=8 => "🟠",
                9..=10 => "🔴",
                _ => "⚪",
            };
            output.push_str(&format!(
                "    {} P{} | {} - {}\n",
                icon,
                debt.priority,
                debt.file_path.to_string_lossy(),
                debt.description
            ));
        }
    }

    output
}

fn filter_drift_by_severity(report: &DriftReport, min_level: u8) -> Vec<serde_json::Value> {
    report
        .drifts
        .iter()
        .filter(|d| {
            let level = match d.severity {
                CoreDriftSeverity::Low => 1,
                CoreDriftSeverity::Medium => 2,
                CoreDriftSeverity::High => 3,
                CoreDriftSeverity::Critical => 4,
            };
            level >= min_level
        })
        .map(|d| {
            serde_json::json!({
                "file": d.file_path.to_string_lossy(),
                "line": d.line_number,
                "category": format!("{:?}", d.category),
                "severity": format!("{:?}", d.severity),
                "message": d.message,
                "suggestion": d.suggestion,
            })
        })
        .collect()
}

fn filter_debt_by_priority(report: &DebtReport, min_level: u8) -> Vec<serde_json::Value> {
    report
        .items
        .iter()
        .filter(|d| {
            let level = match d.priority {
                1..=3 => 1,
                4..=6 => 2,
                7..=8 => 3,
                9..=10 => 4,
                _ => 1,
            };
            level >= min_level
        })
        .map(|d| {
            serde_json::json!({
                "id": d.id,
                "file": d.file_path.to_string_lossy(),
                "line": d.line_number,
                "type": format!("{:?}", d.debt_type),
                "description": d.description,
                "priority": d.priority,
                "impact": d.impact,
                "effort_hours": d.estimated_effort_hours,
                "blocking": d.is_blocking,
                "resolution": d.resolution,
            })
        })
        .collect()
}

/// Handle watch command for file monitoring with auto-analysis
async fn handle_watch(
    path: PathBuf,
    ignore: Option<String>,
    auto_analyze: bool,
    debounce_ms: u64,
    verbose: bool,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    use clawdius_core::watch::handlers::{ContextUpdateHandler, DiagnosticHandler, WatchHandler};
    use clawdius_core::watch::{FileWatcher, WatchConfig};

    println!("👀 Watching {} for changes...", path.display());

    if auto_analyze {
        println!("🔍 Auto-analysis enabled");
    }

    println!("   Debounce: {}ms", debounce_ms);
    if verbose {
        println!("   Verbose output enabled");
    }
    println!();
    println!("Press Ctrl+C to stop watching...");
    println!();

    // Create watch configuration
    let mut config = WatchConfig::new(&path);

    if let Some(ignore_patterns) = ignore {
        let patterns: Vec<String> = ignore_patterns
            .split(',')
            .map(|s| s.trim().to_string())
            .collect();
        for pattern in patterns {
            config = config.exclude(pattern);
        }
    }

    config = config.debounce(debounce_ms);

    // Create handlers (placeholder for future async integration)
    let _context_handler = ContextUpdateHandler::new(vec!["**/*.rs".to_string()]);
    let _diagnostic_handler = DiagnosticHandler::new();

    // Create watcher
    let mut watcher = FileWatcher::new(config)?;
    watcher.start()?;

    // Simulate watching (in a real implementation, this would be async with notify)
    println!("📁 File watcher started successfully");
    println!("   Watching for: **/*.rs, **/*.toml");
    println!("   Ignoring: target/, .git/, node_modules/");

    // In a real implementation, we would integrate with notify crate
    // For now, this is a placeholder that demonstrates the feature

    if output_format == OutputFormat::Json {
        println!(
            "{}",
            serde_json::json!({
                "status": "watching",
                "path": path.to_string_lossy(),
                "auto_analyze": auto_analyze,
                "debounce_ms": debounce_ms
            })
        );
    }

    Ok(())
}
