//! CLI argument parsing and command handling

#![allow(
    clippy::too_many_lines,
    clippy::too_many_arguments,
    clippy::future_not_send,
    clippy::fn_params_excessive_bools,
    clippy::items_after_statements,
    clippy::ptr_arg,
    elided_lifetimes_in_paths,
)]

use clap::{Parser, Subcommand, ValueEnum};
use std::path::{Path, PathBuf};

use clawdius_core::output::OutputFormat as CoreOutputFormat;
use clawdius_core::{Config, MentionResolver, Onboarding, SessionManager};

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
            OutputFormat::Text => Self::Text,
            OutputFormat::Json => Self::Json,
            OutputFormat::StreamJson => Self::StreamJson,
        }
    }
}

impl ValueEnum for OutputFormat {
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::Text, Self::Json, Self::StreamJson]
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        match self {
            Self::Text => Some(clap::builder::PossibleValue::new("text")),
            Self::Json => Some(clap::builder::PossibleValue::new("json")),
            Self::StreamJson => Some(clap::builder::PossibleValue::new("stream-json")),
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
#[derive(Debug, Subcommand)]
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

    #[command(about = "Initialize a new Clawdius project in the current directory")]
    Init {
        /// Project name (defaults to directory name)
        name: Option<String>,
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

        #[arg(short, long)]
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

        #[arg(short = 'R', long)]
        #[arg(help = "Timeout in seconds for LLM operations")]
        timeout_secs: Option<u64>,
    },

    #[command(about = "Language Server Protocol operations")]
    Lsp {
        #[command(subcommand)]
        action: LspCommands,
    },

    #[command(about = "Manage project memory (CLAWDIUS.md)")]
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

    #[command(about = "Git workflow operations")]
    Git {
        #[command(subcommand)]
        action: GitCommands,
    },

    #[command(
        about = "Run an agentic sprint (think -> plan -> build -> review -> test -> ship -> reflect)"
    )]
    Sprint {
        /// Task description for the sprint
        task: String,

        #[arg(short = 'n', long, default_value = "3")]
        /// Maximum build-test iterations
        max_iterations: usize,

        #[arg(long)]
        /// Enable real command execution (build, test)
        real_execution: bool,

        #[arg(long)]
        /// Auto-approve all phases without confirmation
        auto_approve: bool,

        #[arg(short = 'P', long, default_value = "openrouter")]
        /// LLM provider
        provider: String,

        #[arg(short, long)]
        /// Specific model to use
        model: Option<String>,

        #[arg(long)]
        /// URL for browser-based QA in the Test phase
        browser_qa_url: Option<String>,

        #[arg(long)]
        /// Resume from the most recent saved sprint state
        resume: bool,

        #[arg(long, value_name = "COMMAND")]
        /// LSP server command for code intelligence (e.g., --lsp "rust-analyzer")
        lsp: Option<String>,
    },

    #[command(about = "Run pre-ship checks or generate a commit message")]
    Ship {
        /// Subcommand
        #[command(subcommand)]
        action: ShipAction,
    },

    #[command(about = "List and execute markdown skills")]
    Skill {
        /// Subcommand
        #[command(subcommand)]
        action: SkillAction,
    },

    #[command(about = "Start the Clawdius HTTP server")]
    Server {
        #[arg(long, default_value = "0.0.0.0")]
        host: String,
        #[arg(short, long, default_value = "8080")]
        port: u16,
    },

    #[command(about = "View and manage configuration")]
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Debug, Subcommand)]
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

#[derive(Debug, Subcommand)]
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

#[derive(Debug, Subcommand)]
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

#[derive(Debug, Subcommand)]
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

#[derive(Debug, Subcommand)]
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

#[derive(Debug, Subcommand)]
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

#[derive(Debug, Subcommand)]
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

#[derive(Debug, Subcommand)]
pub enum MemoryCommands {
    #[command(about = "Show project memory (CLAWDIUS.md + learned entries)")]
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

    #[command(about = "Create or update CLAWDIUS.md file")]
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

#[derive(Debug, Subcommand)]
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

#[derive(Debug, Subcommand)]
pub enum ShipAction {
    /// Run pre-ship quality checks
    Checks {
        /// Branch name (default: current branch)
        #[arg(short, long, default_value = "main")]
        branch: String,
        /// Changed files to check
        #[arg(short, long)]
        files: Vec<String>,
    },
    /// Generate a conventional commit message
    CommitMessage {
        /// Changed files
        #[arg(short, long)]
        files: Vec<String>,
        /// Description of the changes
        #[arg(short, long)]
        description: String,
        /// Commit scope (e.g. "core", "api")
        #[arg(short, long)]
        scope: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
pub enum SkillAction {
    /// List available skills
    List,
    /// Execute a skill by name
    Run {
        /// Skill name
        name: String,
        /// Arguments to pass to the skill
        #[arg(default_value = "")]
        arguments: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum ConfigAction {
    /// Show current configuration (masks API keys)
    Show,
    /// Get a specific config value
    Get {
        /// Config key (e.g. `llm.default_provider`, llm.anthropic.model)
        key: String,
    },
    /// Set a specific config value
    Set {
        /// Config key (e.g. `llm.default_provider`, llm.anthropic.model, `llm.anthropic.api_key`)
        key: String,
        /// Value to set
        value: String,
    },
    /// Show the path to the config file
    Path,
    /// List available config keys
    List,
}

#[derive(Debug, Subcommand)]
pub enum GitCommands {
    /// Stage files and create a commit with an LLM-generated message
    Commit {
        /// Files to stage (default: all modified)
        files: Vec<String>,
        /// Commit message hint (optional)
        #[arg(short, long)]
        message: Option<String>,
    },
    /// Show a diff of staged or modified files
    Diff {
        /// Show staged diff instead of working diff
        #[arg(short = 's', long)]
        staged: bool,
        /// Specific file to diff
        file: Option<String>,
    },
    /// Show git status summary
    Status,
}

pub mod action;
pub mod analyze;
pub mod auth;
pub mod auto;
pub mod chat;
pub mod checkpoint;
pub mod complete;
pub mod config_cmd;
pub mod doc;
pub mod edit;
pub mod generate;
pub mod git;
pub mod index;
pub mod lang;
pub mod lsp;
pub mod memory;
pub mod metrics;
pub mod modes;
pub mod models;
pub mod server;
pub mod sessions;
pub mod setup;
pub mod ship;
pub mod skill;
pub mod sprint;
pub mod test_cmd;
pub mod timeline;
pub mod verify;
pub mod webhook;

/// Handle a command
#[allow(clippy::missing_errors_doc)]
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
            chat::handle_chat(
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
            auto::handle_auto(
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
        Commands::Init { name } => setup::handle_init(name).await,
        Commands::Setup { quick, provider } => setup::handle_setup(quick, provider, output_format),
        Commands::Sessions { delete, search } => {
            sessions::handle_sessions(delete.as_deref(), search.as_deref(), config_path.as_ref(), output_format)
        },
        Commands::Refactor {
            from,
            to,
            path,
            dry_run,
        } => action::handle_refactor(from, to, path, dry_run, output_format),
        Commands::Action {
            action,
            file,
            line,
            column,
            end_line,
            end_column,
        } => {
            action::handle_action(
                &action,
                &file,
                line,
                column,
                end_line,
                end_column,
                output_format,
            )
        },
        Commands::Test {
            file,
            function,
            output,
        } => test_cmd::handle_test(file, function, output, output_format).await,
        Commands::Doc {
            file,
            element,
            format,
            output,
            inline,
        } => doc::handle_doc(file, element, format, output, inline, output_format).await,
        Commands::Verify { proof, lean_path } => {
            verify::handle_verify(&proof, lean_path, output_format)
        },
        #[cfg(feature = "keyring")]
        Commands::Auth { action } => auth::handle_auth(action).await,
        Commands::Metrics {
            format,
            output,
            reset,
            watch,
        } => metrics::handle_metrics(format, output, reset, watch, output_format).await,
        Commands::Telemetry {
            enable,
            disable,
            enable_metrics,
            enable_crash_reporting,
        } => {
            metrics::handle_telemetry(
                enable,
                disable,
                enable_metrics,
                enable_crash_reporting,
                config_path,
                output_format,
            )
        },
        #[cfg(feature = "vector-db")]
        Commands::Index { path, watch } => index::handle_index(path, watch, output_format).await,
        #[cfg(feature = "vector-db")]
        Commands::Context { query, max_tokens } => {
            index::handle_context(query, max_tokens, output_format).await
        },
        Commands::Checkpoint { action } => {
            checkpoint::handle_checkpoint(action, config_path, output_format).await
        },
        Commands::Timeline { action } => timeline::handle_timeline(action, config_path, output_format).await,
        Commands::Modes { action } => modes::handle_modes(action, config_path, output_format).await,
        Commands::Lang { action } => lang::handle_lang(action, config_path, output_format),
        Commands::Edit {
            initial,
            editor,
            extension,
        } => edit::handle_edit(initial, editor, extension, output_format).await,
        Commands::Webhook { action } => webhook::handle_webhook(action, config_path, output_format).await,
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
            timeout_secs,
        } => {
            generate::handle_generate(
                prompt,
                files,
                mode,
                trust,
                test_strategy,
                max_iterations,
                dry_run,
                provider,
                model,
                timeout_secs,
                config_path,
                output_format,
            )
            .await
        },
        Commands::Lsp { action } => lsp::handle_lsp(action, output_format).await,
        Commands::Memory { action } => memory::handle_memory(action, config_path.as_ref(), output_format),
        Commands::Models { action, host, port } => {
            models::handle_models(action, &host, port, output_format).await
        },
        Commands::Complete {
            file,
            line,
            character,
            language,
            provider,
            model,
        } => {
            complete::handle_complete(
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
        } => analyze::handle_analyze(&path, drift, debt, analyze_format, output, &severity, exclude),
        Commands::Watch {
            path,
            ignore,
            auto_analyze,
            debounce_ms,
            verbose,
        } => {
            analyze::handle_watch(
                &path,
                ignore,
                auto_analyze,
                debounce_ms,
                verbose,
                output_format,
            )
        },
        Commands::Git { action } => git::handle_git(action, config_path).await,
        Commands::Server { host, port } => server::handle_server(&host, port).await,
        Commands::Sprint {
            task,
            max_iterations,
            real_execution,
            auto_approve,
            provider,
            model,
            browser_qa_url,
            resume,
            lsp,
        } => {
            sprint::handle_sprint(
                task,
                max_iterations,
                real_execution,
                auto_approve,
                provider,
                model,
                browser_qa_url,
                resume,
                lsp,
                config_path,
                output_format,
            )
            .await
        },
        Commands::Ship { action } => ship::handle_ship(action, output_format).await,
        Commands::Skill { action } => skill::handle_skill(action, output_format).await,
        Commands::Config { action } => config_cmd::handle_config(action, config_path, output_format),
    }
}

#[allow(elided_lifetimes_in_paths, clippy::missing_errors_doc)]
pub fn load_config(config_path: Option<&Path>) -> anyhow::Result<Config> {
    config_path.map_or_else(
        || Config::load_default().map_err(|e| anyhow::anyhow!("Failed to load default config: {e}")),
        |path| Config::load(path).map_err(|e| anyhow::anyhow!("Failed to load config from {}: {}", path.display(), e)),
    )
}

/// Run in headless mode (read from stdin)
#[allow(clippy::missing_errors_doc)]
pub async fn run_headless(config_path: Option<PathBuf>) -> anyhow::Result<()> {
    use std::io::{self, BufRead};

    let config = load_config(config_path.as_deref())?;
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

        let resolver = MentionResolver::new(std::env::current_dir()?);
        let _context_items = resolver.resolve_all(&line).await?;

        println!("Echo: {line}");

        let msg = clawdius_core::session::Message::user(&line);
        session_manager.add_message(&mut session, msg).await?;
    }

    Ok(())
}

/// First-run experience for new users
pub fn first_run_experience() {
    clawdius_core::onboarding::print_welcome_message();

    let status = Onboarding::check_environment();
    clawdius_core::onboarding::print_onboarding_status(&status);
}
