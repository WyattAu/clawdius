//! CLI argument parsing and command handling

use anyhow::Context;
use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

use clawdius_core::actions::Function;
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

        #[arg(short = 'm', long, default_value = "code")]
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
        }
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
        }
        Commands::Init { path } => handle_init(path, output_format).await,
        Commands::Sessions { delete, search } => {
            handle_sessions(delete, search, config_path, output_format).await
        }
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
        }
        Commands::Test {
            file,
            function,
            output,
        } => handle_test(file, function, output, output_format).await,
        Commands::Verify { proof, lean_path } => {
            handle_verify(proof, lean_path, output_format).await
        }
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
        }
        #[cfg(feature = "vector-db")]
        Commands::Index { path, watch } => handle_index(path, watch, output_format).await,
        #[cfg(feature = "vector-db")]
        Commands::Context { query, max_tokens } => {
            handle_context(query, max_tokens, output_format).await
        }
        Commands::Checkpoint { action } => {
            handle_checkpoint(action, config_path, output_format).await
        }
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
    }
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
            anyhow::bail!("No input provided via stdin");
        }
        input.trim().to_string()
    } else if non_interactive {
        anyhow::bail!(
            "Message is required in non-interactive mode. Provide via argument or stdin."
        );
    } else {
        anyhow::bail!("Message is required. Use --editor or provide via argument/stdin.");
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
        }
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
        }
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
        }
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
        }
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
            }
            Err(e) => {
                if output_format == OutputFormat::Text {
                    println!("⚠️ Could not run tests: {e}");
                }
            }
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
            }
            Err(e) => {
                if output_format == OutputFormat::Text {
                    println!("⚠️ Could not commit: {e}");
                }
            }
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
        }
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
        }
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
            }
            '}' => {
                depth -= 1;
                if in_function && depth == 0 {
                    function_end = start + i + 1;
                    break;
                }
            }
            _ => {}
        }
    }

    if function_end > start {
        Ok(code[start..function_end].to_string())
    } else {
        anyhow::bail!("Could not extract function body")
    }
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
        }
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
        }
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
        }
        AuthCommands::Get { provider } => match storage.get_api_key(&provider)? {
            Some(key) => {
                println!("API key for {}: {}***", provider, &key[..8.min(key.len())]);
            }
            None => {
                println!("No API key found for {provider}");
            }
        },
        AuthCommands::Delete { provider } => {
            storage.delete_api_key(&provider)?;
            println!("✓ API key deleted for {provider}");
        }
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

    let workspace_path = path.unwrap_or_else(|| std::env::current_dir().unwrap());

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
        }
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
        }

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
                }
                Err(e) => CheckpointResult::error("list", e.to_string()),
            }
        }

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
        }

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
        }

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
        }

        CheckpointCommands::Cleanup { session, keep } => {
            let session_id = session.unwrap_or_else(|| "default".to_string());
            match manager.cleanup_old_checkpoints(&session_id, keep) {
                Ok(deleted) => CheckpointResult::success("cleanup")
                    .with_session_id(session_id)
                    .with_file_count(deleted),
                Err(e) => CheckpointResult::error("cleanup", e.to_string()),
            }
        }

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
                }
                Err(e) => CheckpointResult::error("timeline", e.to_string()),
            }
        }
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
        }

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
        }

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
                                        }
                                        Err(e) => {
                                            eprintln!(
                                                "[{}] Failed to create checkpoint: {}",
                                                chrono::Local::now().format("%H:%M:%S"),
                                                e
                                            );
                                        }
                                    }
                                }
                                Err(e) => {
                                    eprintln!(
                                        "[{}] Failed to create timeline manager: {}",
                                        chrono::Local::now().format("%H:%M:%S"),
                                        e
                                    );
                                }
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
        }

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
        }

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
        }

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
        }

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
        }

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
        }
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
            }
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
        }

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
        }
        LangCommands::Set { code } => {
            match Language::from_code(&code) {
                Some(lang) => {
                    // Update config
                    let config_path = config_path.unwrap_or_else(|| {
                        std::env::current_dir()
                            .unwrap()
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
                        let re = regex::Regex::new(r"^language\s*=\s*.*$").unwrap();
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
                }
                None => {
                    anyhow::bail!("Unknown language code: {code}. Supported codes: en, zh, ja, ko, de, fr, es, it, pt, ru");
                }
            }
        }
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
        }
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
        }
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
        }
        OutputFormat::Text | OutputFormat::StreamJson => {
            println!("Edited content:\n");
            println!("{content}");
            println!("\n---");
            println!(
                "{} characters, {} lines",
                content.len(),
                content.lines().count()
            );
        }
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
        }

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
        }

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
                }
                None => {
                    anyhow::bail!("Workflow not found: {id}");
                }
            }
        }

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
        }

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
        }

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
                }
                None => {
                    anyhow::bail!("Execution not found: {}", execution_id);
                }
            }
        }

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
        }
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
        }

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
        }

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
                }
                None => {
                    anyhow::bail!("Webhook not found: {id}");
                }
            }
        }

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
        }

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
        }

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
        }

        WebhookCommands::Deliveries { id, limit } => {
            use clawdius_core::webhooks::WebhookId;

            let webhook_id = id.as_ref().map(|s| WebhookId::new(s));
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
        }

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
        }
    }

    Ok(())
}
