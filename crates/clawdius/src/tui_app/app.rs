//! TUI App state
//!
//! Supports full agentic tool-use loop: LLM responses with tool calls are
//! detected, executed, and results fed back to the LLM automatically.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use super::components;
use super::components::{ChatView, DiffView, FileList, SessionEntry, SessionPicker, Spinner, SyntaxHighlighter, WorkspaceSwitcher};
use super::theme;
use super::types::{AppMode, InputMode, LayoutMode, Message, TuiEvent};
use super::vim::VimKeymap;

use crossterm::event::KeyEvent;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};
use tokio::sync::mpsc;

use clawdius_core::{
    config::ShellSandboxConfig,
    llm::{self, ChatMessage, ChatRole},
    llm::providers::{LlmClient, Tool},
    modes::AgentMode,
    tools::file::{FileEditParams, FileListParams, FileReadParams, FileTool, FileWriteParams},
    tools::git::{GitDiffParams, GitLogParams, GitTool},
    tools::shell::{ShellParams, ShellTool},
    Config, FileDiff, Session, SessionManager,
};

/// Built-in tool executor for the TUI. Uses clawdius-core tools directly
/// (FileTool, ShellTool, GitTool) to handle tool calls from the LLM.
pub struct TuiToolExecutor {
    file_tool: Arc<FileTool>,
    shell_tool: Arc<ShellTool>,
    git_tool: Arc<GitTool>,
}

impl TuiToolExecutor {
    pub fn new(workspace_root: PathBuf) -> Self {
        let sandbox_config = ShellSandboxConfig::default();
        let file_tool = Arc::new(FileTool::with_workspace_root(&workspace_root));
        let shell_tool = Arc::new(ShellTool::new(
            sandbox_config.clone(),
            workspace_root.clone(),
        ));
        let git_tool = Arc::new(GitTool::new(sandbox_config, workspace_root));

        Self {
            file_tool,
            shell_tool,
            git_tool,
        }
    }

    /// Get tool definitions for sending to the LLM.
    pub fn tool_definitions() -> Vec<Tool> {
        vec![
            Tool::new("read_file")
                .with_description("Read file contents. Args: { path: string, offset?: number, limit?: number }"),
            Tool::new("write_file")
                .with_description("Write content to a file. Args: { path: string, content: string }"),
            Tool::new("edit_file")
                .with_description("Edit a section of a file. Args: { path: string, old_string: string, new_string: string }"),
            Tool::new("list_directory")
                .with_description("List files in a directory. Args: { path: string, recursive?: boolean }"),
            Tool::new("shell")
                .with_description("Run a shell command. Args: { command: string, timeout?: number }"),
            Tool::new("git_status")
                .with_description("Show git working tree status. Args: {}"),
            Tool::new("git_diff")
                .with_description("Show git diff. Args: { staged?: boolean, file?: string }"),
            Tool::new("git_log")
                .with_description("Show git commit log. Args: { count?: number, file?: string }"),
        ]
    }

    /// Execute a tool call by name. Returns (output, is_error).
    pub fn execute_tool(&self, name: &str, arguments: &str) -> (String, bool) {
        let args: HashMap<String, serde_json::Value> =
            serde_json::from_str(arguments).unwrap_or_default();

        match name {
            "read_file" => {
                let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let offset = args.get("offset").and_then(|v| v.as_u64()).map(|n| n as usize);
                let limit = args.get("limit").and_then(|v| v.as_u64()).map(|n| n as usize);
                let params = FileReadParams { path, offset, limit };
                match self.file_tool.read(params) {
                    Ok(content) => (content, false),
                    Err(e) => (format!("Error: {e}"), true),
                }
            },
            "write_file" => {
                let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let params = FileWriteParams { path, content };
                match self.file_tool.write(params) {
                    Ok(()) => ("File written successfully".to_string(), false),
                    Err(e) => (format!("Error: {e}"), true),
                }
            },
            "edit_file" => {
                let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let old_string = args.get("old_string").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let new_string = args.get("new_string").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let replace_all = args.get("replace_all").and_then(|v| v.as_bool()).unwrap_or(false);
                let params = FileEditParams { path, old_string, new_string, replace_all };
                match self.file_tool.edit(params) {
                    Ok(changed) => {
                        if changed {
                            ("File edited successfully".to_string(), false)
                        } else {
                            ("No changes made (old_string not found)".to_string(), true)
                        }
                    },
                    Err(e) => (format!("Error: {e}"), true),
                }
            },
            "list_directory" => {
                let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".").to_string();
                let params = FileListParams { path };
                match self.file_tool.list(params) {
                    Ok(entries) => (entries.join("\n"), false),
                    Err(e) => (format!("Error: {e}"), true),
                }
            },
            "shell" | "run_command" => {
                let command = args.get("command").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let timeout = args
                    .get("timeout")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(120_000);
                let params = ShellParams {
                    command,
                    timeout,
                    cwd: None,
                };
                match self.shell_tool.execute(params) {
                    Ok(result) => {
                        let output = if result.stdout.is_empty() {
                            result.stderr
                        } else if result.stderr.is_empty() {
                            result.stdout
                        } else {
                            format!("{}\n{}", result.stdout, result.stderr)
                        };
                        (output, result.exit_code != 0)
                    },
                    Err(e) => (format!("Error: {e}"), true),
                }
            },
            "git_status" => {
                match self.git_tool.status(None) {
                    Ok(output) => (output, false),
                    Err(e) => (format!("Error: {e}"), true),
                }
            },
            "git_diff" => {
                let staged = args.get("staged").and_then(|v| v.as_bool()).unwrap_or(false);
                let path = args.get("path").or(args.get("file")).and_then(|v| v.as_str()).map(String::from);
                let params = GitDiffParams { staged, path };
                match self.git_tool.diff(params, None) {
                    Ok(output) => (output, false),
                    Err(e) => (format!("Error: {e}"), true),
                }
            },
            "git_log" => {
                let count = args.get("count").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
                let path = args.get("path").or(args.get("file")).and_then(|v| v.as_str()).map(String::from);
                let params = GitLogParams { count, path };
                match self.git_tool.log(params, None) {
                    Ok(output) => (output, false),
                    Err(e) => (format!("Error: {e}"), true),
                }
            },
            _ => (format!("Unknown tool: {name}"), true),
        }
    }
}

/// Maximum number of agentic iterations (tool-call loops) per user message.
const MAX_ITERATIONS: usize = 50;

pub struct App {
    pub session: Option<Session>,
    pub session_manager: SessionManager,
    pub config: Config,
    pub mode: AppMode,
    pub layout_mode: LayoutMode,
    /// Secondary panel mode (only used in split layouts).
    pub secondary_mode: AppMode,
    pub agent_mode: AgentMode,
    pub input_mode: InputMode,
    pub input: String,
    pub should_quit: bool,
    pub is_loading: bool,
    pub chat_view: ChatView,
    pub file_list: FileList,
    pub diff_view: DiffView,
    pub vim: VimKeymap,
    pub spinner: Spinner,
    pub syntax: SyntaxHighlighter,
    pub error_message: Option<String>,
    /// Receiver for structured events from the agentic loop.
    pub stream_rx: Option<mpsc::Receiver<TuiEvent>>,
    /// Workspace context (repo map) prepended to user messages. None if not available.
    pub workspace_context: Option<String>,
    /// Request timeout in seconds for LLM API calls.
    pub request_timeout_secs: u64,
    /// Tool executor for handling tool calls from the LLM.
    tool_executor: Arc<TuiToolExecutor>,
    /// Conversation history accumulated across the session (for multi-turn context).
    conversation_history: Vec<ChatMessage>,
    /// Whether tools are enabled (toggled via :tools command).
    pub tools_enabled: bool,
    /// Current iteration count for the agentic loop (displayed in status).
    pub iteration_count: usize,
    /// Session picker popup.
    session_picker: SessionPicker,
    /// Workspace switcher popup.
    workspace_switcher: WorkspaceSwitcher,
    /// File watcher for live file change detection.
    file_watcher: Option<clawdius_core::watch::FileWatcher>,
    /// Receiver for file watcher events.
    file_watcher_rx:
        Option<std::sync::mpsc::Receiver<Vec<clawdius_core::watch::WatchEvent>>>,
}

impl App {
    #[allow(clippy::missing_errors_doc)]
    pub fn new() -> anyhow::Result<Self> {
        let config = Config::load_default()?;
        let session_manager = SessionManager::new(&config)?;
        let workspace_root = std::env::current_dir().unwrap_or_default();

        Ok(Self {
            session: None,
            session_manager,
            config,
            mode: AppMode::Chat,
            layout_mode: LayoutMode::Single,
            secondary_mode: AppMode::Diff,
            agent_mode: AgentMode::Code,
            input_mode: InputMode::Normal,
            input: String::new(),
            should_quit: false,
            is_loading: false,
            chat_view: ChatView::new(),
            file_list: FileList::new(),
            diff_view: DiffView::new(),
            vim: VimKeymap::new(),
            spinner: Spinner::new(),
            syntax: SyntaxHighlighter::new(),
            error_message: None,
            stream_rx: None,
            workspace_context: Self::build_workspace_context(),
            request_timeout_secs: 120,
            tool_executor: Arc::new(TuiToolExecutor::new(workspace_root)),
            conversation_history: Vec::new(),
            tools_enabled: true,
            iteration_count: 0,
            session_picker: SessionPicker::new(),
            workspace_switcher: WorkspaceSwitcher::new(),
            file_watcher: None,
            file_watcher_rx: None,
        })
    }

    /// Build workspace context from the current directory.
    /// Returns None if the current directory doesn't look like a project.
    fn build_workspace_context() -> Option<String> {
        let Ok(cwd) = std::env::current_dir() else { return None };

        // Only build context if there's a recognizable project marker
        let has_project_marker = ["Cargo.toml", "package.json", "pyproject.toml", "go.mod"]
            .iter()
            .any(|marker| cwd.join(marker).exists());

        if !has_project_marker {
            return None;
        }

        match clawdius_core::workspace::WorkspaceContextBuilder::build_single(&cwd, None) {
            Ok(ctx) if !ctx.trim().is_empty() => Some(ctx),
            _ => None,
        }
    }

    /// Create the LLM provider from current config.
    fn create_provider(&self) -> anyhow::Result<llm::LlmProvider> {
        let provider_name = self
            .config
            .llm
            .default_provider
            .as_deref()
            .unwrap_or("deepseek");

        let llm_config = llm::LlmConfig::from_config(&self.config.llm, provider_name).map_err(|e| {
            anyhow::anyhow!(
                "Failed to create LLM config: {e}. Set the appropriate API key \
                 (e.g., DEEPSEEK_API_KEY, ANTHROPIC_API_KEY, OPENAI_API_KEY)."
            )
        })?;

        llm::create_provider(&llm_config)
            .map_err(|e| anyhow::anyhow!("Failed to create provider: {e}"))
    }

    #[allow(clippy::missing_errors_doc)]
    pub async fn handle_key(&mut self, key: KeyEvent) -> anyhow::Result<()> {
        use crossterm::event::KeyCode;

        if self.error_message.is_some() {
            self.error_message = None;
            return Ok(());
        }

        // Route to session picker if visible
        if self.session_picker.visible {
            match key.code {
                KeyCode::Esc | KeyCode::Char('q') => {
                    self.session_picker.close();
                },
                KeyCode::Char('j') | KeyCode::Down => {
                    self.session_picker.move_down();
                },
                KeyCode::Char('k') | KeyCode::Up => {
                    self.session_picker.move_up();
                },
                KeyCode::Enter => {
                    if let Some(id) = self.session_picker.selected_id().map(String::from) {
                        let session_id = clawdius_core::session::SessionId(
                            id.parse::<uuid::Uuid>().unwrap_or_default(),
                        );
                        if let Ok(Some(session)) =
                            self.session_manager.load_session(&session_id)
                        {
                            self.session = Some(session);
                            self.chat_view.add_message(Message::system(format!(
                                "Switched to session {}",
                                &id[..id.len().min(8)]
                            )));
                        }
                    }
                    self.session_picker.close();
                },
                KeyCode::Backspace => {
                    self.session_picker.backspace();
                },
                KeyCode::Char(c) => {
                    self.session_picker.type_char(c);
                },
                _ => {},
            }
            return Ok(());
        }

        // Route to workspace switcher if visible
        if self.workspace_switcher.visible {
            match key.code {
                KeyCode::Esc | KeyCode::Char('q') => {
                    self.workspace_switcher.close();
                },
                KeyCode::Char('j') | KeyCode::Down => {
                    self.workspace_switcher.move_down();
                },
                KeyCode::Char('k') | KeyCode::Up => {
                    self.workspace_switcher.move_up();
                },
                KeyCode::Enter => {
                    if let Some(_id) = self.workspace_switcher.selected_id() {
                        self.chat_view
                            .add_message(Message::system("Workspace switched".to_string()));
                    }
                    self.workspace_switcher.close();
                },
                KeyCode::Backspace => {
                    self.workspace_switcher.backspace();
                },
                KeyCode::Char(c) => {
                    self.workspace_switcher.type_char(c);
                },
                _ => {},
            }
            return Ok(());
        }

        match self.mode {
            AppMode::Help => {
                match key.code {
                    KeyCode::Char('q' | '?') | KeyCode::Esc => {
                        self.mode = AppMode::Chat;
                    },
                    _ => {},
                }
                return Ok(());
            },
            AppMode::Chat => self.handle_chat_key(key).await?,
            AppMode::FileBrowser => self.handle_file_browser_key(key),
            AppMode::Diff => self.handle_diff_key(key),
        }

        Ok(())
    }

    async fn handle_chat_key(&mut self, key: crossterm::event::KeyEvent) -> anyhow::Result<()> {
        use crossterm::event::KeyCode;

        match self.input_mode {
            InputMode::Normal => match key.code {
                KeyCode::Char('i') => {
                    self.input_mode = InputMode::Insert;
                },
                KeyCode::Char(':') => {
                    self.input_mode = InputMode::Command;
                    self.input.clear();
                    self.input.push(':');
                },
                KeyCode::Char('j') | KeyCode::Down => {
                    self.chat_view.scroll_down(10);
                },
                KeyCode::Char('k') | KeyCode::Up => {
                    self.chat_view.scroll_up();
                },
                KeyCode::Char('d')
                    if key
                        .modifiers
                        .contains(crossterm::event::KeyModifiers::CONTROL) =>
                {
                    self.chat_view.scroll_page_down(10, 10);
                },
                KeyCode::Char('u')
                    if key
                        .modifiers
                        .contains(crossterm::event::KeyModifiers::CONTROL) =>
                {
                    self.chat_view.scroll_page_up(10);
                },
                KeyCode::Char('q') => {
                    self.should_quit = true;
                },
                KeyCode::Char('?') => {
                    self.mode = AppMode::Help;
                },
                KeyCode::Tab | KeyCode::Char('2') => {
                    self.mode = AppMode::FileBrowser;
                },
                KeyCode::Char('3') => {
                    self.mode = AppMode::Diff;
                },
                _ => {},
            },
            InputMode::Insert => match key.code {
                KeyCode::Esc => {
                    self.input_mode = InputMode::Normal;
                },
                KeyCode::Enter => {
                    if !self.input.is_empty() {
                        self.send_message().await?;
                    }
                },
                KeyCode::Backspace => {
                    self.input.pop();
                },
                KeyCode::Char('e')
                    if key
                        .modifiers
                        .contains(crossterm::event::KeyModifiers::CONTROL) =>
                {
                    self.open_external_editor()?;
                },
                KeyCode::Char(c) => {
                    self.input.push(c);
                },
                _ => {},
            },
            InputMode::Command => match key.code {
                KeyCode::Esc => {
                    self.input_mode = InputMode::Normal;
                    self.input.clear();
                },
                KeyCode::Enter => {
                    self.execute_command(&self.input.clone());
                    self.input_mode = InputMode::Normal;
                    self.input.clear();
                },
                KeyCode::Backspace => {
                    self.input.pop();
                    if self.input == ":" {
                        self.input_mode = InputMode::Normal;
                        self.input.clear();
                    }
                },
                KeyCode::Char(c) => {
                    self.input.push(c);
                },
                _ => {},
            },
        }

        Ok(())
    }

    fn handle_file_browser_key(&mut self, key: crossterm::event::KeyEvent) {
        use crossterm::event::KeyCode;

        match key.code {
            KeyCode::Char('q' | '1') | KeyCode::Tab => {
                self.mode = AppMode::Chat;
            },
            KeyCode::Char('j') | KeyCode::Down => {
                self.file_list.down();
            },
            KeyCode::Char('k') | KeyCode::Up => {
                self.file_list.up();
            },
            KeyCode::Enter => {
                if let Some(path) = self.file_list.enter() {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        let diff = FileDiff::compute(path, None, &content);
                        self.diff_view.set_diff(diff);
                        self.mode = AppMode::Diff;
                    }
                }
            },
            KeyCode::Char(' ') => {
                self.file_list.toggle_select();
            },
            KeyCode::Char('r') => {
                self.file_list.refresh();
            },
            KeyCode::Char('3') => {
                self.mode = AppMode::Diff;
            },
            _ => {},
        }
    }

    fn handle_diff_key(&mut self, key: crossterm::event::KeyEvent) {
        use crossterm::event::KeyCode;

        match key.code {
            KeyCode::Char('q' | '1') | KeyCode::Esc | KeyCode::Tab => {
                self.mode = AppMode::Chat;
            },
            KeyCode::Char('j') | KeyCode::Down => {
                self.diff_view.scroll_down(10);
            },
            KeyCode::Char('k') | KeyCode::Up => {
                self.diff_view.scroll_up();
            },
            KeyCode::Char('d')
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                self.diff_view.scroll_down(20);
            },
            KeyCode::Char('u')
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                for _ in 0..20 {
                    self.diff_view.scroll_up();
                }
            },
            KeyCode::Char('2') => {
                self.mode = AppMode::FileBrowser;
            },
            _ => {},
        }
    }

    fn execute_command(&mut self, cmd: &str) {
        let cmd = cmd.trim_start_matches(':').trim();
        let parts: Vec<&str> = cmd.splitn(2, ' ').collect();

        match parts[0] {
            "q" | "quit" => {
                self.should_quit = true;
            },
            "help" | "?" => {
                self.mode = AppMode::Help;
            },
            "files" | "ls" => {
                self.mode = AppMode::FileBrowser;
            },
            "diff" => {
                self.mode = AppMode::Diff;
            },
            "clear" => {
                self.chat_view = ChatView::new();
                self.conversation_history.clear();
            },
            "tools" => {
                let enabled = !self.tools_enabled;
                self.tools_enabled = enabled;
                self.chat_view.add_message(Message::system(format!(
                    "Tool execution {}",
                    if enabled { "enabled" } else { "disabled" }
                )));
            },
            "compact" => {
                // Keep only the last N messages in history to reduce context
                let keep = 20;
                let len = self.conversation_history.len();
                if len > keep {
                    let start = len - keep;
                    self.conversation_history = self.conversation_history[start..].to_vec();
                    // Keep the system prompt
                    if let Some(sys) = self
                        .conversation_history
                        .first()
                        .filter(|m| matches!(m.role, ChatRole::System))
                    {
                        let mut new_history = vec![sys.clone()];
                        new_history.extend(self.conversation_history.iter().skip(1).cloned());
                        self.conversation_history = new_history;
                    }
                    self.chat_view.add_message(Message::system(format!(
                        "Compacted conversation history to last {keep} messages"
                    )));
                } else {
                    self.chat_view
                        .add_message(Message::system("Conversation history already compact".to_string()));
                }
            },
            "sessions" => {
                match self.session_manager.list_sessions() {
                    Ok(sessions) => {
                        let entries: Vec<SessionEntry> = sessions
                            .iter()
                            .map(|s| {
                                let title_text = s
                                    .title
                                    .as_deref()
                                    .or_else(|| {
                                        s.messages.first().and_then(|m| match &m.content {
                                            clawdius_core::session::MessageContent::Text(t) => {
                                                t.lines().next()
                                            },
                                            _ => None,
                                        })
                                    })
                                    .unwrap_or("(empty)");
                                SessionEntry {
                                    id: s.id.to_string(),
                                    title: title_text.chars().take(60).collect(),
                                    message_count: s.messages.len(),
                                    last_active: s.created_at.format("%Y-%m-%d %H:%M").to_string(),
                                    tokens_used: s.total_tokens(),
                                    is_active: self
                                        .session
                                        .as_ref()
                                        .is_some_and(|cur| cur.id == s.id),
                                }
                            })
                            .collect();
                        self.session_picker.set_entries(entries);
                        self.session_picker.open();
                    },
                    Err(e) => {
                        self.error_message = Some(format!("Failed to list sessions: {e}"));
                    },
                }
            },
            "session" => {
                if parts.len() > 1 {
                    let id_str = parts[1].trim();
                    // Parse UUID from the ID string
                    let uuid = id_str.parse::<uuid::Uuid>();
                    if let Ok(uuid) = uuid {
                        let session_id = clawdius_core::session::SessionId(uuid);
                        match self.session_manager.load_session(&session_id) {
                            Ok(Some(session)) => {
                                self.session = Some(session);
                                self.chat_view.add_message(Message::system(format!(
                                    "Switched to session {}",
                                    &id_str[..id_str.len().min(8)]
                                )));
                            },
                            Ok(None) => {
                                self.error_message =
                                    Some(format!("Session '{id_str}' not found"));
                            },
                            Err(e) => {
                                self.error_message =
                                    Some(format!("Failed to load session: {e}"));
                            },
                        }
                    } else {
                        self.error_message =
                            Some(format!("Invalid session ID: '{id_str}'"));
                    }
                } else {
                    self.error_message = Some("Usage: :session <uuid>".to_string());
                }
            },
            "new" | "newsession" => {
                self.chat_view = ChatView::new();
                self.conversation_history.clear();
                self.session = None;
                self.chat_view
                    .add_message(Message::system("Started new session".to_string()));
            },
            "mode" => {
                if parts.len() > 1 {
                    let mode_name = parts[1].trim();
                    let modes_dir = std::env::current_dir()
                        .unwrap_or_default()
                        .join(".clawdius")
                        .join("modes");

                    match AgentMode::load_by_name(mode_name, &modes_dir) {
                        Ok(mode) => {
                            self.agent_mode = mode;
                            self.chat_view.add_message(Message::system(format!(
                                "Switched to {} mode",
                                self.agent_mode.name()
                            )));
                        },
                        Err(e) => {
                            self.error_message =
                                Some(format!("Failed to load mode '{mode_name}': {e}"));
                        },
                    }
                } else {
                    self.error_message = Some("Usage: :mode <mode-name>".to_string());
                }
            },
            "modes" => {
                let modes_dir = std::env::current_dir()
                    .unwrap_or_default()
                    .join(".clawdius")
                    .join("modes");

                if let Ok(modes) = AgentMode::list_all(&modes_dir) {
                    let mode_list: Vec<String> = modes
                        .iter()
                        .map(|(name, desc)| format!("  {name} - {desc}"))
                        .collect();
                    self.error_message =
                        Some(format!("Available modes:\n{}", mode_list.join("\n")));
                } else {
                    self.error_message = Some("Failed to list modes".to_string());
                }
            },
            "git" => {
                let git_cmd = parts.get(1).map(|s| *s).unwrap_or("status");
                self.run_git_command(git_cmd);
            },
            "provider" => {
                if parts.len() > 1 {
                    let provider_name = parts[1].trim();
                    self.config.llm.default_provider = Some(provider_name.to_string());
                    self.chat_view.add_message(Message::system(format!(
                        "Switched provider to {provider_name}"
                    )));
                } else {
                    let current = self
                        .config
                        .llm
                        .default_provider
                        .as_deref()
                        .unwrap_or("none");
                    self.error_message = Some(format!(
                        "Current provider: {current}\nUsage: :provider <deepseek|anthropic|openai|google|ollama|zai|openrouter>"
                    ));
                }
            },
            "timeout" => {
                if parts.len() > 1 {
                    if let Ok(secs) = parts[1].trim().parse::<u64>() {
                        self.request_timeout_secs = secs;
                        self.chat_view.add_message(Message::system(format!(
                            "Request timeout set to {secs}s"
                        )));
                    } else {
                        self.error_message = Some("Usage: :timeout <seconds>".to_string());
                    }
                } else {
                    self.error_message =
                        Some(format!("Current timeout: {}s", self.request_timeout_secs));
                }
            },
            "sprint" => {
                if parts.len() > 1 {
                    let task = parts[1..].join(" ");
                    self.start_sprint(task);
                } else {
                    self.error_message =
                        Some("Usage: :sprint <task description>".to_string());
                }
            },
            "auto" => {
                if parts.len() > 1 {
                    let task = parts[1..].join(" ");
                    self.start_auto(task);
                } else {
                    self.error_message =
                        Some("Usage: :auto <task description>".to_string());
                }
            },
            "generate" | "gen" => {
                if parts.len() > 1 {
                    let task = parts[1..].join(" ");
                    self.start_generate(task);
                } else {
                    self.error_message =
                        Some("Usage: :generate <task description>".to_string());
                }
            },
            "test" => {
                if parts.len() > 1 {
                    let file = parts[1].trim();
                    self.run_quick_command(&format!("cargo test --lib -- {file} 2>&1 | head -50"));
                } else {
                    self.run_quick_command("cargo test --lib 2>&1 | tail -30");
                }
            },
            "build" | "check" => {
                self.run_quick_command("cargo check 2>&1 | tail -20");
            },
            "doc" => {
                if parts.len() > 1 {
                    let file = parts[1].trim();
                    self.chat_view.add_message(Message::system(format!(
                        "Generating docs for {file}... (use CLI: clawdius doc {file})"
                    )));
                } else {
                    self.chat_view.add_message(Message::system(
                        "Usage: :doc <file>\nFor full doc generation, use: clawdius doc <file>".to_string(),
                    ));
                }
            },
            "verify" => {
                if parts.len() > 1 {
                    let proof = parts[1].trim();
                    self.run_quick_command(&format!("clawdius verify {proof} 2>&1"));
                } else {
                    self.chat_view.add_message(Message::system(
                        "Usage: :verify <proof-file>\nFor Lean4 proof verification, use: clawdius verify <file>".to_string(),
                    ));
                }
            },
            "checkpoint" => {
                self.run_quick_command("clawdius checkpoint list 2>&1 | tail -15");
            },
            "timeline" => {
                self.run_quick_command("clawdius timeline list 2>&1 | tail -15");
            },
            "memory" => {
                self.run_quick_command("clawdius memory show 2>&1 | tail -30");
            },
            "analyze" => {
                if parts.len() > 1 {
                    let path = parts[1].trim();
                    self.run_quick_command(&format!("clawdius analyze {path} 2>&1 | tail -40"));
                } else {
                    self.run_quick_command("clawdius analyze . 2>&1 | tail -40");
                }
            },
            "config" => {
                if parts.len() > 1 {
                    let sub = parts[1].trim();
                    match sub {
                        "show" | "s" => {
                            let provider = self.config.llm.default_provider.as_deref().unwrap_or("none");
                            self.error_message = Some(format!(
                                "Provider: {provider}\nTimeout: {}s\nTools: {}\nMode: {}",
                                self.request_timeout_secs,
                                if self.tools_enabled { "on" } else { "off" },
                                self.agent_mode.name()
                            ));
                        },
                        _ => {
                            self.error_message = Some(
                                "Usage: :config show\nSubcommands: show".to_string(),
                            );
                        },
                    }
                } else {
                    self.error_message = Some("Usage: :config show".to_string());
                }
            },
            "watch" => {
                if self.file_watcher.is_some() {
                    self.file_watcher = None;
                    self.file_watcher_rx = None;
                    self.chat_view
                        .add_message(Message::system("👀 File watcher stopped".to_string()));
                } else {
                    let cwd = std::env::current_dir().unwrap_or_default();
                    let config = clawdius_core::watch::WatchConfig::new(&cwd).debounce(300);
                    match clawdius_core::watch::FileWatcher::start_with_channel(config) {
                        Ok((watcher, rx)) => {
                            self.file_watcher_rx = Some(rx);
                            self.file_watcher = Some(watcher);
                            self.chat_view.add_message(Message::system(format!(
                                "👀 Watching {} for changes...\n  :watch to stop",
                                cwd.display()
                            )));
                        },
                        Err(e) => {
                            self.error_message = Some(format!("Failed to start watcher: {e}"));
                        },
                    }
                }
            },
            _ => {
                // Split pane commands
                if cmd == "split" || cmd == "sp" {
                    self.layout_mode = LayoutMode::SplitHorizontal;
                } else if cmd == "vsplit" || cmd == "vsp" {
                    self.layout_mode = LayoutMode::SplitVertical;
                } else if cmd == "unsplit" || cmd == "only" {
                    self.layout_mode = LayoutMode::Single;
                } else if cmd.starts_with("secondary") {
                    // :secondary diff / :secondary files
                    if parts.len() > 1 {
                        self.secondary_mode = match parts[1].trim() {
                            "files" | "f" => AppMode::FileBrowser,
                            _ => AppMode::Diff,
                        };
                    }
                } else {
                    self.error_message = Some(format!("Unknown command: {cmd}"));
                }
            },
        }
    }

    /// Run a git command and display the result in the chat.
    fn run_git_command(&mut self, cmd: &str) {
        let output = match cmd {
            "status" => std::process::Command::new("git")
                .args(["status", "--short"])
                .output(),
            "log" => std::process::Command::new("git")
                .args(["log", "--oneline", "-10"])
                .output(),
            "diff" => std::process::Command::new("git")
                .args(["diff", "--stat"])
                .output(),
            _ => {
                self.error_message = Some(format!(
                    "Unknown git command: {cmd}\nAvailable: status, log, diff"
                ));
                return;
            },
        };

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                let result = if stdout.trim().is_empty() {
                    stderr
                } else {
                    stdout
                };
                self.chat_view.add_message(Message::tool(result.trim()));
            },
            Err(e) => {
                self.error_message = Some(format!("Failed to run git {cmd}: {e}"));
            },
        }
    }

    async fn send_message(&mut self) -> anyhow::Result<()> {
        let message: String = self.input.drain(..).collect();

        // Resolve @mentions (files, URLs, etc.)
        let resolver = clawdius_core::MentionResolver::new(std::env::current_dir()?);
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

        // Add user message to chat view
        self.chat_view.add_message(Message::user(&message));

        // Get or create session
        let session = self.session_manager.get_or_create_active()?;
        self.session = Some(session);

        // Build user content with workspace context
        let user_content = match self.workspace_context.as_deref() {
            Some(ctx) if !ctx.is_empty() => {
                format!("{ctx}\n\n## Project Structure\n{context_str}")
            }
            _ => context_str,
        };

        // Add user message to conversation history
        self.conversation_history
            .push(ChatMessage { role: ChatRole::User, content: user_content });

        // Add a placeholder streaming message
        let mut stream_msg = Message::assistant("");
        stream_msg.streaming = true;
        self.chat_view.add_message(stream_msg);
        self.is_loading = true;
        self.spinner.tick();
        self.iteration_count = 0;

        // Start the agentic loop in background
        let (tx, rx) = mpsc::channel::<TuiEvent>(256);
        self.stream_rx = Some(rx);

        let provider = self.create_provider()?;
        let tools = if self.tools_enabled {
            TuiToolExecutor::tool_definitions()
        } else {
            Vec::new()
        };
        let tool_executor = Arc::clone(&self.tool_executor);
        let messages = self.conversation_history.clone();
        let system_prompt = self.agent_mode.system_prompt().to_string();
        let timeout_secs = self.request_timeout_secs;

        tokio::spawn(async move {
            run_agentic_loop(
                provider,
                messages,
                system_prompt,
                tools,
                tool_executor,
                tx,
                timeout_secs,
            )
            .await;
        });

        Ok(())
    }

    /// Poll the stream receiver for a structured event. Call this from the event loop.
    /// Returns `true` if an event was received (or stream ended).
    pub fn poll_stream(&mut self) -> bool {
        let Some(rx) = &mut self.stream_rx else { return false };

        match rx.try_recv() {
            Ok(event) => {
                match event {
                    TuiEvent::Chunk(text) => {
                        self.chat_view.append_to_last_message(&text);
                    },
                    TuiEvent::ToolCall { name, arguments } => {
                        // Finalize the current streaming message (the assistant's text before tool call)
                        self.chat_view.finish_streaming();
                        // Show the tool call as a tool message
                        let display_args = if arguments.len() > 200 {
                            format!("{}...", &arguments[..200])
                        } else {
                            arguments.clone()
                        };
                        self.chat_view
                            .add_message(Message::tool(format!("⏳ {name}({display_args})")));
                        // Start a new streaming placeholder for what comes after the tool call
                        let mut stream_msg = Message::assistant("");
                        stream_msg.streaming = true;
                        self.chat_view.add_message(stream_msg);
                    },
                    TuiEvent::ToolResult { name, output, is_error } => {
                        let prefix = if is_error { "❌" } else { "✅" };
                        let display_output = if output.len() > 500 {
                            format!("{}...\n[{} bytes total]", &output[..500], output.len())
                        } else {
                            output
                        };
                        self.chat_view
                            .add_message(Message::tool(format!("{prefix} {name}: {display_output}")));
                        // Refresh file list after file edits
                        if matches!(name.as_str(), "write_file" | "edit_file") && !is_error {
                            self.file_list.refresh();
                        }
                    },
                    TuiEvent::Phase { name, status, detail } => {
                        use super::types::PhaseStatus;
                        let icon = match status {
                            PhaseStatus::Started => "▶",
                            PhaseStatus::Progress(_) => "…",
                            PhaseStatus::Completed(_) => "✓",
                            PhaseStatus::Failed(_) => "✗",
                            PhaseStatus::Skipped => "⊘",
                        };
                        let msg = match &status {
                            PhaseStatus::Started => format!("{icon} {name}"),
                            PhaseStatus::Progress(s) => format!("{icon} {name}: {s}"),
                            PhaseStatus::Completed(s) => format!("{icon} {name}: {s}"),
                            PhaseStatus::Failed(s) => format!("{icon} {name} FAILED: {s}"),
                            PhaseStatus::Skipped => format!("{icon} {name}: skipped"),
                        };
                        self.chat_view.add_message(Message::system(msg));
                        if !detail.is_empty() && matches!(&status, PhaseStatus::Failed(_)) {
                            self.chat_view.add_message(Message::system(detail));
                        }
                    },
                    TuiEvent::Done => {
                        self.chat_view.finish_streaming();
                        self.is_loading = false;
                        self.iteration_count = 0;
                        self.stream_rx = None;
                    },
                    TuiEvent::Error(e) => {
                        self.chat_view.finish_streaming();
                        self.chat_view.append_to_last_message(&format!("\nError: {e}"));
                        self.is_loading = false;
                        self.iteration_count = 0;
                        self.stream_rx = None;
                    },
                }
                true
            },
            Err(mpsc::error::TryRecvError::Empty) => false,
            Err(mpsc::error::TryRecvError::Disconnected) => {
                // Stream ended without Done event — finalize gracefully
                self.chat_view.finish_streaming();
                self.is_loading = false;
                self.iteration_count = 0;
                self.stream_rx = None;
                true
            },
        }
    }

    /// Drain all available events from the stream receiver.
    pub fn drain_stream(&mut self) {
        // Drain up to 50 events per tick to avoid blocking the render loop
        for _ in 0..50 {
            if !self.poll_stream() {
                break;
            }
        }
    }

    /// Poll the file watcher for change events and display them in chat.
    pub fn poll_file_watcher(&mut self) {
        let rx = match &self.file_watcher_rx {
            Some(rx) => rx,
            None => return,
        };

        for _ in 0..20 {
            match rx.try_recv() {
                Ok(events) => {
                    for event in &events {
                        let icon = match event {
                            clawdius_core::watch::WatchEvent::Created { .. } => "✨",
                            clawdius_core::watch::WatchEvent::Modified { .. } => "✏️ ",
                            clawdius_core::watch::WatchEvent::Deleted { .. } => "🗑️ ",
                            clawdius_core::watch::WatchEvent::Renamed { .. } => "🔄",
                        };
                        self.chat_view.add_message(Message::system(format!(
                            "{icon} {} {}",
                            event.label(),
                            event
                                .path()
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_else(|| event.path().to_string_lossy().to_string())
                        )));
                    }
                },
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.file_watcher = None;
                    self.file_watcher_rx = None;
                    self.chat_view.add_message(Message::system(
                        "👀 File watcher disconnected".to_string(),
                    ));
                    break;
                },
            }
        }
    }

    /// Start a sprint workflow in the background.
    fn start_sprint(&mut self, task: String) {
        self.chat_view.add_message(Message::user(&task));
        self.chat_view.add_message(Message::system(format!("🚀 Starting sprint: {task}")));
        self.is_loading = true;
        self.spinner.tick();

        let (tx, rx) = mpsc::channel::<TuiEvent>(256);
        self.stream_rx = Some(rx);

        let provider_name = self
            .config
            .llm
            .default_provider
            .as_deref()
            .unwrap_or("deepseek")
            .to_string();
        let config_clone = self.config.clone();
        let cwd = std::env::current_dir().unwrap_or_default();

        tokio::spawn(async move {
            // Create provider
            let llm_config = match clawdius_core::llm::LlmConfig::from_config(
                &config_clone.llm,
                &provider_name,
            ) {
                Ok(c) => c,
                Err(e) => {
                    let _ = tx.send(TuiEvent::Error(format!("Config error: {e}"))).await;
                    let _ = tx.send(TuiEvent::Done).await;
                    return;
                },
            };
            let provider = match clawdius_core::llm::create_provider(&llm_config) {
                Ok(p) => p,
                Err(e) => {
                    let _ = tx.send(TuiEvent::Error(format!("Provider error: {e}"))).await;
                    let _ = tx.send(TuiEvent::Done).await;
                    return;
                },
            };
            let llm = std::sync::Arc::new(provider);

            // Build sprint config
            let mut sprint_config =
                clawdius_core::agentic::sprint::SprintConfig::new(&task);
            sprint_config.project_root = cwd;
            sprint_config.auto_approve = true;
            sprint_config.real_execution = true;
            sprint_config.max_iterations = 3;
            sprint_config.build_command = "cargo check 2>&1".to_string();
            sprint_config.test_command = "cargo test --lib 2>&1".to_string();

            let engine = clawdius_core::agentic::sprint::SprintEngine::new(llm);
            let result = engine.run(sprint_config).await;

            match result {
                Ok(sprint_result) => {
                    let _ = tx
                        .send(TuiEvent::Phase {
                            name: "Sprint Complete".to_string(),
                            status: super::types::PhaseStatus::Completed(
                                if sprint_result.success {
                                    "All phases passed".to_string()
                                } else {
                                    "Some phases failed".to_string()
                                },
                            ),
                            detail: sprint_result.summary,
                        })
                        .await;
                    for phase in &sprint_result.phase_results {
                        let status = match phase.status {
                            clawdius_core::agentic::sprint::PhaseStatus::Success => {
                                super::types::PhaseStatus::Completed(format!(
                                    "{} ({}ms)",
                                    phase.output.chars().take(80).collect::<String>(),
                                    phase.duration_ms
                                ))
                            },
                            clawdius_core::agentic::sprint::PhaseStatus::Failed => {
                                super::types::PhaseStatus::Failed(
                                    phase.errors.join("; ").chars().take(100).collect(),
                                )
                            },
                            clawdius_core::agentic::sprint::PhaseStatus::Skipped => {
                                super::types::PhaseStatus::Skipped
                            },
                        };
                        let _ = tx
                            .send(TuiEvent::Phase {
                                name: format!("{:?}", phase.phase),
                                status,
                                detail: String::new(),
                            })
                            .await;
                    }
                },
                Err(e) => {
                    let _ = tx.send(TuiEvent::Error(format!("Sprint failed: {e}"))).await;
                },
            }
            let _ = tx.send(TuiEvent::Done).await;
        });
    }

    /// Start an auto (single LLM call + optional test/commit) in the background.
    fn start_auto(&mut self, task: String) {
        self.chat_view.add_message(Message::user(&task));
        self.chat_view.add_message(Message::system("🤖 Auto mode: processing task..."));
        self.is_loading = true;
        self.spinner.tick();

        let (tx, rx) = mpsc::channel::<TuiEvent>(256);
        self.stream_rx = Some(rx);

        let provider_name = self
            .config
            .llm
            .default_provider
            .as_deref()
            .unwrap_or("deepseek")
            .to_string();
        let config_clone = self.config.clone();

        tokio::spawn(async move {
            let llm_config = match clawdius_core::llm::LlmConfig::from_config(
                &config_clone.llm,
                &provider_name,
            ) {
                Ok(c) => c,
                Err(e) => {
                    let _ = tx.send(TuiEvent::Error(format!("Config error: {e}"))).await;
                    let _ = tx.send(TuiEvent::Done).await;
                    return;
                },
            };
            let provider = match clawdius_core::llm::create_provider(&llm_config) {
                Ok(p) => p,
                Err(e) => {
                    let _ = tx.send(TuiEvent::Error(format!("Provider error: {e}"))).await;
                    let _ = tx.send(TuiEvent::Done).await;
                    return;
                },
            };

            // Single LLM call
            let messages = vec![
                clawdius_core::llm::ChatMessage {
                    role: clawdius_core::llm::ChatRole::System,
                    content: "You are an autonomous coding assistant. Complete the task concisely. \
                             Make real file changes using [TOOL_CALL] blocks when needed.".to_string(),
                },
                clawdius_core::llm::ChatMessage {
                    role: clawdius_core::llm::ChatRole::User,
                    content: task,
                },
            ];

            match provider.chat(messages).await {
                Ok(response) => {
                    let _ = tx.send(TuiEvent::Chunk(response)).await;
                },
                Err(e) => {
                    let _ = tx.send(TuiEvent::Error(format!("Auto failed: {e}"))).await;
                },
            }
            let _ = tx.send(TuiEvent::Done).await;
        });
    }

    /// Start a generate (AgenticSystem) workflow in the background.
    fn start_generate(&mut self, task: String) {
        self.chat_view.add_message(Message::user(&task));
        self.chat_view.add_message(Message::system("⚡ Generate mode: agentic code generation..."));
        self.is_loading = true;
        self.spinner.tick();

        let (tx, rx) = mpsc::channel::<TuiEvent>(256);
        self.stream_rx = Some(rx);

        let provider_name = self
            .config
            .llm
            .default_provider
            .as_deref()
            .unwrap_or("deepseek")
            .to_string();
        let config_clone = self.config.clone();

        tokio::spawn(async move {
            let llm_config = match clawdius_core::llm::LlmConfig::from_config(
                &config_clone.llm,
                &provider_name,
            ) {
                Ok(c) => c,
                Err(e) => {
                    let _ = tx.send(TuiEvent::Error(format!("Config error: {e}"))).await;
                    let _ = tx.send(TuiEvent::Done).await;
                    return;
                },
            };
            let provider = match clawdius_core::llm::create_provider(&llm_config) {
                Ok(p) => p,
                Err(e) => {
                    let _ = tx.send(TuiEvent::Error(format!("Provider error: {e}"))).await;
                    let _ = tx.send(TuiEvent::Done).await;
                    return;
                },
            };
            let llm = std::sync::Arc::new(provider);

            let request = clawdius_core::agentic::TaskRequest {
                                id: uuid::Uuid::new_v4().to_string(),
                description: task.clone(),
                target_files: Vec::new(),
                mode: clawdius_core::agentic::GenerationMode::iterative_with_max(3),
                test_strategy: clawdius_core::agentic::TestExecutionStrategy::Skip,
                apply_workflow: clawdius_core::agentic::ApplyWorkflow::Direct,
                context: clawdius_core::agentic::TaskContext {
                    related_files: Vec::new(),
                    conversation_history: Vec::new(),
                    project_language: None,
                    project_framework: None,
                    constraints: Vec::new(),
                },
                trust_level: clawdius_core::agentic::TrustLevel::Medium,
            };

            let mut system =
                clawdius_core::agentic::AgenticSystem::new(
                    request.mode.clone(),
                    request.test_strategy.clone(),
                    request.apply_workflow.clone(),
                )
                .with_llm_client(llm);

            match system.execute(request).await {
                Ok(result) => {
                    let _ = tx
                        .send(TuiEvent::Phase {
                            name: "Generate".to_string(),
                            status: super::types::PhaseStatus::Completed(
                                if result.success {
                                    format!(
                                        "{} files changed in {}ms",
                                        result.changes.len(),
                                        result.duration_ms
                                    )
                                } else {
                                    "Generation failed".to_string()
                                },
                            ),
                            detail: format!(
                                "Files: {}",
                                result
                                    .changes
                                    .iter()
                                    .map(|c| c.path.clone())
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            ),
                        })
                        .await;
                    if !result.changes.is_empty() {
                        for change in &result.changes {
                            let _ = tx
                                .send(TuiEvent::Chunk(format!(
                                    "  {} {}",
                                    match change.change_type {
                                        clawdius_core::agentic::ChangeType::Created => "+",
                                        clawdius_core::agentic::ChangeType::Modified => "~",
                                        clawdius_core::agentic::ChangeType::Deleted => "-",
                                    },
                                    change.path
                                )))
                                .await;
                        }
                    }
                },
                Err(e) => {
                    let _ = tx.send(TuiEvent::Error(format!("Generate failed: {e}"))).await;
                },
            }
            let _ = tx.send(TuiEvent::Done).await;
        });
    }

    /// Run a quick shell command and display the result in the chat.
    fn run_quick_command(&mut self, command: &str) {
        self.chat_view
            .add_message(Message::system(format!("Running: {command}")));
        let output = std::process::Command::new("sh")
            .arg("-c")
            .arg(command)
            .output();

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                let result = if stdout.trim().is_empty() {
                    stderr
                } else {
                    stdout
                };
                self.chat_view.add_message(Message::tool(result.trim().to_string()));
            },
            Err(e) => {
                self.error_message = Some(format!("Failed to run command: {e}"));
            },
        }
    }

    fn open_external_editor(&mut self) -> anyhow::Result<()> {
        use clawdius_core::tools::editor::ExternalEditor;

        let editor = ExternalEditor::default_editor();

        let current_input = self.input.clone();
        let edited_content = editor
            .open_and_edit(&current_input)
            .map_err(|e| anyhow::anyhow!("Editor error: {e}"))?;

        self.input = edited_content;

        Ok(())
    }

    /// Tick the spinner animation when loading
    pub fn tick(&mut self) {
        if self.is_loading {
            self.spinner.tick();
        }
    }

    /// Handle terminal resize events
    pub const fn resize(&mut self) {}

    /// Handle mouse scroll up.
    pub const fn scroll_up(&mut self) {
        match self.mode {
            AppMode::Chat => self.chat_view.scroll_up(),
            AppMode::FileBrowser => self.file_list.up(),
            AppMode::Diff => self.diff_view.scroll_up(),
            AppMode::Help => {}
        }
    }

    /// Handle mouse scroll down.
    pub fn scroll_down(&mut self) {
        match self.mode {
            AppMode::Chat => self.chat_view.scroll_down(50),
            AppMode::FileBrowser => self.file_list.down(),
            AppMode::Diff => self.diff_view.scroll_down(50),
            AppMode::Help => {}
        }
    }

    /// Draw the TUI
    pub fn draw(&self, f: &mut Frame<'_>) {
        let outer_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),   // header
                Constraint::Min(10),     // main content
                Constraint::Length(3),   // input
                Constraint::Length(1),   // status
            ])
            .split(f.area());

        self.draw_header(f, outer_chunks[0]);
        self.draw_input(f, outer_chunks[2]);
        self.draw_status(f, outer_chunks[3]);

        // Main content — respect layout mode
        match self.layout_mode {
            LayoutMode::Single => {
                self.draw_main_content(f, outer_chunks[1], self.mode);
            }
            LayoutMode::SplitHorizontal => {
                let split = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([
                        Constraint::Percentage(50),
                        Constraint::Length(1), // divider
                        Constraint::Percentage(50),
                    ])
                    .split(outer_chunks[1]);

                // Primary: always chat in split mode
                self.draw_messages(f, split[0]);

                // Divider line
                let divider = Paragraph::new("│")
                    .style(Style::default().fg(Color::DarkGray));
                f.render_widget(divider, split[1]);

                // Secondary panel
                self.draw_main_content(f, split[2], self.secondary_mode);
            }
            LayoutMode::SplitVertical => {
                let split = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Percentage(50),
                        Constraint::Length(1), // divider
                        Constraint::Percentage(50),
                    ])
                    .split(outer_chunks[1]);

                // Primary: always chat
                self.draw_messages(f, split[0]);

                // Divider
                let divider = Paragraph::new("─")
                    .style(Style::default().fg(Color::DarkGray));
                f.render_widget(divider, split[1]);

                // Secondary
                self.draw_main_content(f, split[2], self.secondary_mode);
            }
        }

        if let Some(ref error) = self.error_message {
            Self::draw_popup(f, "Error", error);
        }

        // Render session picker popup on top
        if self.session_picker.visible {
            let theme = theme::current();
            let widget = components::session_picker::SessionPickerWidget::new(&self.session_picker, theme);
            f.render_widget(widget, f.area());
        }

        // Render workspace switcher popup on top
        if self.workspace_switcher.visible {
            let theme = theme::current();
            let widget = components::workspace_switcher::WorkspaceSwitcherWidget::new(
                &self.workspace_switcher,
                theme,
            );
            f.render_widget(widget, f.area());
        }
    }

    fn draw_header(&self, f: &mut Frame<'_>, area: Rect) {
        let theme = theme::current();

        let mode_text = match self.mode {
            AppMode::Chat => "CHAT",
            AppMode::FileBrowser => "FILES",
            AppMode::Diff => "DIFF",
            AppMode::Help => "HELP",
        };

        let session_info = self.session.as_ref().map_or_else(
            || "no session".to_string(),
            |s| format!("{} msgs", s.messages.len()),
        );

        let layout_text = match self.layout_mode {
            LayoutMode::Single => "",
            LayoutMode::SplitHorizontal => " [SPLIT]",
            LayoutMode::SplitVertical => " [VSPLIT]",
        };

        let tools_indicator = if self.tools_enabled {
            "🔧"
        } else {
            ""
        };

        let iteration_text = if self.iteration_count > 0 {
            format!(" [iter {}]", self.iteration_count)
        } else {
            String::new()
        };

        let title = Line::from(vec![
            Span::styled("CLAWDIUS", theme.title()),
            Span::raw("  "),
            Span::styled(mode_text, Style::new().fg(theme.accent)),
            Span::styled(layout_text, Style::default().fg(Color::Cyan)),
            Span::raw(tools_indicator),
            Span::raw(&iteration_text),
            Span::raw("  "),
            Span::styled("|", theme.border()),
            Span::raw("  "),
            Span::styled(session_info, theme.muted()),
        ]);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme.border());

        let paragraph = Paragraph::new(title).block(block);
        f.render_widget(paragraph, area);
    }

    fn draw_main_content(&self, f: &mut Frame<'_>, area: Rect, mode: AppMode) {
        match mode {
            AppMode::Chat => self.draw_messages(f, area),
            AppMode::FileBrowser => self.draw_file_browser(f, area),
            AppMode::Diff => self.draw_diff(f, area),
            AppMode::Help => Self::draw_help(f, area),
        }
    }

    fn draw_messages(&self, f: &mut Frame<'_>, area: Rect) {
        let mut chat_view = self.chat_view.clone();
        chat_view.render(f, area);
    }

    fn draw_file_browser(&self, f: &mut Frame<'_>, area: Rect) {
        let mut file_list = self.file_list.clone();
        file_list.render(f, area);
    }

    fn draw_diff(&self, f: &mut Frame<'_>, area: Rect) {
        let mut diff_view = self.diff_view.clone();
        diff_view.render(f, area);
    }

    fn draw_help(f: &mut Frame<'_>, area: Rect) {
        let help_text = vec![
            Line::from(Span::styled(
                "Clawdius Help",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::default(),
            Line::from(Span::styled(
                "Navigation:",
                Style::default().fg(Color::Yellow),
            )),
            Line::from("  Tab        - Switch between views"),
            Line::from("  1-3        - Jump to specific view (Chat, Files, Diff)"),
            Line::from("  q/Esc      - Quit / Close popup"),
            Line::from("  ?          - Toggle this help"),
            Line::default(),
            Line::from(Span::styled(
                "Chat Mode:",
                Style::default().fg(Color::Yellow),
            )),
            Line::from("  i          - Enter insert mode"),
            Line::from("  Esc        - Return to normal mode"),
            Line::from("  Enter      - Send message (in insert mode)"),
            Line::from("  Ctrl+e     - Open external editor (in insert mode)"),
            Line::from("  j/k        - Scroll messages"),
            Line::from("  Ctrl+d/u   - Page down/up"),
            Line::from("  :          - Enter command mode"),
            Line::default(),
            Line::from(Span::styled(
                "Agentic Commands:",
                Style::default().fg(Color::Yellow),
            )),
            Line::from("  :sprint <task>  - Run multi-phase sprint (think→plan→build→test→ship)"),
            Line::from("  :auto <task>    - Single LLM call with optional test/commit"),
            Line::from("  :generate <task> - Iterative agentic code generation"),
            Line::from("  :tools     - Toggle tool execution (file edit, shell, git)"),
            Line::from("  :compact   - Compact conversation history to last 20 msgs"),
            Line::from("  :git status/log/diff - Run git commands"),
            Line::from("  :provider <name> - Switch LLM provider"),
            Line::from("  :timeout <secs>     - Set API timeout"),
            Line::default(),
            Line::from(Span::styled(
                "Quick Actions:",
                Style::default().fg(Color::Yellow),
            )),
            Line::from("  :build / :check  - Run cargo check"),
            Line::from("  :test <file>    - Run cargo test"),
            Line::from("  :analyze [path] - Analyze code for drift/debt"),
            Line::from("  :checkpoint   - Show file checkpoints"),
            Line::from("  :timeline    - Show file version history"),
            Line::from("  :memory      - Show project memory (CLAWDIUS.md)"),
            Line::from("  :config show  - Show current configuration"),
            Line::from("  :doc <file>   - Generate documentation (CLI)"),
            Line::from("  :verify <file> - Verify Lean4 proof (CLI)"),
            Line::from("  :watch     - Toggle file watcher (live change feed)"),
            Line::default(),
            Line::from(Span::styled(
                "Session Commands:",
                Style::default().fg(Color::Yellow),
            )),
            Line::from("  :sessions  - List recent sessions"),
            Line::from("  :session <id> - Switch to a session"),
            Line::from("  :new       - Start new session (clears history)"),
            Line::default(),
            Line::from(Span::styled(
                "Layout Commands:",
                Style::default().fg(Color::Yellow),
            )),
            Line::from("  :files     - Open file browser"),
            Line::from("  :diff      - Open diff view"),
            Line::from("  :clear     - Clear chat"),
            Line::from("  :split     - Horizontal split (chat + code)"),
            Line::from("  :vsplit    - Vertical split (chat + code)"),
            Line::from("  :unsplit   - Return to single pane"),
            Line::from("  :secondary <diff|files> - Set split secondary panel"),
            Line::default(),
            Line::from(Span::styled(
                "Agent Modes:",
                Style::default().fg(Color::Yellow),
            )),
            Line::from("  :mode <n>  - Switch agent mode"),
            Line::from("  :modes     - List available modes"),
            Line::from("  code       - Code generation and editing (default)"),
            Line::from("  architect  - Design and structure planning"),
            Line::from("  ask        - Quick answers and explanations"),
            Line::from("  debug      - Troubleshooting and diagnostics"),
            Line::from("  review     - Code review and analysis"),
            Line::from("  refactor   - Code improvement and refactoring"),
            Line::from("  test       - Test generation"),
        ];

        let paragraph = Paragraph::new(help_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Help (? to close)"),
            )
            .wrap(Wrap { trim: false });
        f.render_widget(paragraph, area);
    }

    fn draw_input(&self, f: &mut Frame<'_>, area: Rect) {
        let mode_text = match self.input_mode {
            InputMode::Normal => "NORMAL",
            InputMode::Insert => "INSERT",
            InputMode::Command => "COMMAND",
        };

        let mode_color = match self.input_mode {
            InputMode::Normal => Color::Blue,
            InputMode::Insert => Color::Green,
            InputMode::Command => Color::Yellow,
        };

        let title = Line::from(vec![
            Span::styled(
                format!("[{mode_text}]"),
                Style::default().fg(mode_color).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(
                match self.input_mode {
                    InputMode::Normal => "Press i to insert, : for commands",
                    InputMode::Insert => "Type your message, Enter to send, Esc to cancel",
                    InputMode::Command => "Enter command",
                },
                Style::default().fg(Color::Gray),
            ),
        ]);

        let input_text = if self.input.is_empty() && self.input_mode == InputMode::Normal {
            "~"
        } else {
            &self.input
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(Style::default().fg(mode_color));

        let paragraph = Paragraph::new(input_text).block(block);
        f.render_widget(paragraph, area);
    }

    fn draw_status(&self, f: &mut Frame<'_>, area: Rect) {
        let theme = theme::current();

        let mut status_parts = vec![
            Span::styled(self.agent_mode.name(), theme.status_highlight()),
            Span::styled(" │ ", theme.border()),
        ];

        if let Some(ref session) = self.session {
            status_parts.push(Span::styled(
                format!(
                    "{}:{}",
                    session.meta.provider.as_deref().unwrap_or("unknown"),
                    session.meta.model.as_deref().unwrap_or("unknown")
                ),
                theme.model_info(),
            ));
            status_parts.push(Span::styled(" │ ", theme.border()));
            status_parts.push(Span::styled(
                format!("{} tokens", session.total_tokens()),
                theme.token_count(),
            ));
        } else {
            status_parts.push(Span::styled("no session", theme.error()));
        }

        if self.is_loading {
            status_parts.push(Span::styled(" │ ", theme.border()));
            status_parts.push(self.spinner.render());
            if self.iteration_count > 0 {
                status_parts.push(Span::styled(
                    format!(" iter:{}", self.iteration_count),
                    theme.muted(),
                ));
            }
        }

        if self.tools_enabled {
            status_parts.push(Span::styled(" │ ", theme.border()));
            status_parts.push(Span::styled("tools:on", theme.user_message()));
        }

        status_parts.push(Span::styled(" │ ", theme.border()));
        status_parts.push(Span::styled("? help", theme.muted()));

        let status = Line::from(status_parts);
        let paragraph = Paragraph::new(status);
        f.render_widget(paragraph, area);
    }

    fn draw_popup(f: &mut Frame<'_>, title: &str, content: &str) {
        let area = centered_rect(60, 20, f.area());
        f.render_widget(Clear, area);

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Red));

        let paragraph = Paragraph::new(content)
            .block(block)
            .wrap(Wrap { trim: false });
        f.render_widget(paragraph, area);
    }
}

/// The agentic loop: send messages to LLM, detect tool calls, execute them,
/// feed results back, repeat until done or max iterations reached.
///
/// This runs as a background tokio task and sends `TuiEvent`s to the TUI
/// via the provided channel.
async fn run_agentic_loop(
    provider: llm::LlmProvider,
    mut messages: Vec<ChatMessage>,
    system_prompt: String,
    tools: Vec<Tool>,
    tool_executor: Arc<TuiToolExecutor>,
    tx: mpsc::Sender<TuiEvent>,
    timeout_secs: u64,
) {
    // Ensure system prompt is present
    if messages.is_empty() || !matches!(messages[0].role, ChatRole::System) {
        messages.insert(
            0,
            ChatMessage {
                role: ChatRole::System,
                content: system_prompt,
            },
        );
    }

    for iteration in 0..MAX_ITERATIONS {
        // Send iteration event
        let _ = tx
            .send(TuiEvent::Chunk(format!("\n[iteration {}/{}]\n", iteration + 1, MAX_ITERATIONS)))
            .await;

        // Call LLM (with tools if available)
        let response = if tools.is_empty() {
            match tokio::time::timeout(
                std::time::Duration::from_secs(timeout_secs),
                provider.chat(messages.clone()),
            )
            .await
            {
                Ok(Ok(text)) => (text, Vec::new()),
                Ok(Err(e)) => {
                    let _ = tx.send(TuiEvent::Error(e.to_string())).await;
                    return;
                },
                Err(_) => {
                    let _ = tx
                        .send(TuiEvent::Error(format!(
                            "LLM timed out after {timeout_secs}s"
                        )))
                        .await;
                    return;
                },
            }
        } else {
            match tokio::time::timeout(
                std::time::Duration::from_secs(timeout_secs),
                provider.chat_with_tools(messages.clone(), tools.clone()),
            )
            .await
            {
                Ok(Ok(result)) => (result.text, result.tool_calls),
                Ok(Err(_e)) => {
                    // Fall back to non-tool call
                    match tokio::time::timeout(
                        std::time::Duration::from_secs(timeout_secs),
                        provider.chat(messages.clone()),
                    )
                    .await
                    {
                        Ok(Ok(text)) => (text, Vec::new()),
                        Ok(Err(e2)) => {
                            let _ = tx.send(TuiEvent::Error(e2.to_string())).await;
                            return;
                        },
                        Err(_) => {
                            let _ = tx
                                .send(TuiEvent::Error(format!(
                                    "LLM timed out after {timeout_secs}s"
                                )))
                                .await;
                            return;
                        },
                    }
                },
                Err(_) => {
                    // Timeout — fall back to non-tool call
                    match tokio::time::timeout(
                        std::time::Duration::from_secs(timeout_secs),
                        provider.chat(messages.clone()),
                    )
                    .await
                    {
                        Ok(Ok(text)) => (text, Vec::new()),
                        Ok(Err(e)) => {
                            let _ = tx.send(TuiEvent::Error(e.to_string())).await;
                            return;
                        },
                        Err(_) => {
                            let _ = tx
                                .send(TuiEvent::Error(format!(
                                    "LLM timed out after {timeout_secs}s"
                                )))
                                .await;
                            return;
                        },
                    }
                },
            }
        };

        let (text, tool_calls) = response;

        // Send the text chunk
        if !text.is_empty() {
            let _ = tx.send(TuiEvent::Chunk(text.clone())).await;
        }

        // If no tool calls, we're done
        if tool_calls.is_empty() {
            // Also try parsing text-based tool calls
            let text_tool_calls = parse_text_tool_calls(&text);
            if text_tool_calls.is_empty() {
                let _ = tx.send(TuiEvent::Done).await;
                return;
            }

            // Process text-based tool calls
            for (name, arguments) in text_tool_calls {
                let _ = tx
                    .send(TuiEvent::ToolCall {
                        name: name.clone(),
                        arguments: arguments.clone(),
                    })
                    .await;

                let (output, is_error) = tool_executor.execute_tool(&name, &arguments);

                let _ = tx
                    .send(TuiEvent::ToolResult {
                        name: name.clone(),
                        output: output.clone(),
                        is_error,
                    })
                    .await;

                // Add assistant message and tool result to conversation
                messages.push(ChatMessage {
                    role: ChatRole::Assistant,
                    content: format!("[TOOL_CALL] {{\"name\": \"{}\", \"arguments\": {}}} [/TOOL_CALL]", name, arguments),
                });
                messages.push(ChatMessage {
                    role: ChatRole::User,
                    content: format!("[TOOL_RESULT] {output} [/TOOL_RESULT]"),
                });
            }
        } else {
            // Process native tool calls
            for tc in &tool_calls {
                let name = tc.fn_name.clone();
                let arguments = tc.fn_arguments.to_string();

                let _ = tx
                    .send(TuiEvent::ToolCall {
                        name: name.clone(),
                        arguments: arguments.clone(),
                    })
                    .await;

                let (output, is_error) = tool_executor.execute_tool(&name, &arguments);

                let _ = tx
                    .send(TuiEvent::ToolResult {
                        name,
                        output,
                        is_error,
                    })
                    .await;
            }

            // Add assistant message with tool calls to conversation
            let tool_calls_json: String = tool_calls
                .iter()
                .map(|tc| {
                    format!(
                        "[TOOL_CALL] {{\"name\": \"{}\", \"arguments\": {}}} [/TOOL_CALL]",
                        tc.fn_name, tc.fn_arguments
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");

            messages.push(ChatMessage {
                role: ChatRole::Assistant,
                content: if text.is_empty() {
                    tool_calls_json
                } else {
                    format!("{text}\n{tool_calls_json}")
                },
            });

            // Build tool results for conversation history
            let mut results_text: Vec<String> = Vec::new();
            for tc in &tool_calls {
                let args_str = tc.fn_arguments.to_string();
                let (output, _) = tool_executor.execute_tool(&tc.fn_name, &args_str);
                results_text.push(format!(
                    "[TOOL_RESULT] {} {} [/TOOL_RESULT]",
                    tc.fn_name, output
                ));
            }

            messages.push(ChatMessage {
                role: ChatRole::User,
                content: results_text.join("\n"),
            });
        }
    }

    // Max iterations reached
    let _ = tx
        .send(TuiEvent::Error(format!(
            "Reached maximum iterations ({MAX_ITERATIONS})"
        )))
        .await;
}

/// Parse text-based tool calls from the LLM response.
/// Supports formats:
///   [TOOL_CALL] {"name": "...", "arguments": {...}} [/TOOL_CALL]
///   ant:invoke:tool_name{"param": "value"}ant:invoke:end
fn parse_text_tool_calls(text: &str) -> Vec<(String, String)> {
    let mut results = Vec::new();

    // Format 1: [TOOL_CALL]...[/TOOL_CALL]
    for cap in text
        .split("[TOOL_CALL]")
        .skip(1)
        .filter_map(|s| s.split("[/TOOL_CALL]").next())
    {
        let trimmed = cap.trim();
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
            let name = val
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let arguments = val
                .get("arguments")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            if !name.is_empty() {
                results.push((name, arguments.to_string()));
            }
        }
    }

    if !results.is_empty() {
        return results;
    }

    // Format 2: ant:invoke:tool_name{...}ant:invoke:end
    for cap in text
        .split("ant:invoke:")
        .skip(1)
        .filter_map(|s| s.split("ant:invoke:end").next())
    {
        let trimmed = cap.trim();
        if let Some((name, args_str)) = trimmed.split_once('{') {
            let name = name.to_string();
            let args_str = format!("{{{args_str}");
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&args_str) {
                results.push((name, val.to_string()));
            } else {
                results.push((name, args_str));
            }
        }
    }

    results
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
