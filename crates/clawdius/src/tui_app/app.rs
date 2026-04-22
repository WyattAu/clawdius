//! TUI App state

use super::components::{ChatView, DiffView, FileList, Spinner, SyntaxHighlighter};
use super::theme;
use super::types::{AppMode, InputMode, Message};
use super::vim::VimKeymap;

use crossterm::event::KeyEvent;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use clawdius_core::{
    llm::{self, ChatMessage, ChatRole},
    llm::providers::LlmClient,
    modes::AgentMode,
    Config, FileDiff, Session, SessionManager,
};

pub struct App {
    pub session: Option<Session>,
    pub session_manager: SessionManager,
    pub config: Config,
    pub mode: AppMode,
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
}

impl App {
    pub fn new() -> anyhow::Result<Self> {
        let config = Config::load_default()?;
        let session_manager = SessionManager::new(&config)?;

        Ok(Self {
            session: None,
            session_manager,
            config,
            mode: AppMode::Chat,
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
        })
    }

    pub async fn handle_key(&mut self, key: KeyEvent) -> anyhow::Result<()> {
        use crossterm::event::KeyCode;

        if self.error_message.is_some() {
            self.error_message = None;
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
            AppMode::FileBrowser => self.handle_file_browser_key(key).await?,
            AppMode::Diff => self.handle_diff_key(key).await?,
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
                KeyCode::Tab => {
                    self.mode = AppMode::FileBrowser;
                },
                KeyCode::Char('2') => {
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
                    self.open_external_editor().await?;
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

    async fn handle_file_browser_key(
        &mut self,
        key: crossterm::event::KeyEvent,
    ) -> anyhow::Result<()> {
        use crossterm::event::KeyCode;

        match key.code {
            KeyCode::Char('q') => {
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
                        let diff = FileDiff::compute(path.clone(), None, &content);
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
            KeyCode::Tab => {
                self.mode = AppMode::Chat;
            },
            KeyCode::Char('1') => {
                self.mode = AppMode::Chat;
            },
            KeyCode::Char('3') => {
                self.mode = AppMode::Diff;
            },
            _ => {},
        }

        Ok(())
    }

    async fn handle_diff_key(&mut self, key: crossterm::event::KeyEvent) -> anyhow::Result<()> {
        use crossterm::event::KeyCode;

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
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
            KeyCode::Tab => {
                self.mode = AppMode::Chat;
            },
            KeyCode::Char('1') => {
                self.mode = AppMode::Chat;
            },
            KeyCode::Char('2') => {
                self.mode = AppMode::FileBrowser;
            },
            _ => {},
        }

        Ok(())
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
            _ => {
                self.error_message = Some(format!("Unknown command: {cmd}"));
            },
        }
    }

    async fn send_message(&mut self) -> anyhow::Result<()> {
        let message: String = self.input.drain(..).collect();

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

        self.chat_view.add_message(Message::user(&message));

        let session = self.session_manager.get_or_create_active()?;
        self.session = Some(session);

        self.is_loading = true;
        self.spinner.tick();

        let response = match self.call_llm(&context_str).await {
            Ok(resp) => resp,
            Err(e) => format!("Error: {e}"),
        };

        self.chat_view.add_message(Message::assistant(&response));
        self.is_loading = false;

        Ok(())
    }

    async fn call_llm(&self, message: &str) -> anyhow::Result<String> {
        let provider_name = self
            .config
            .llm
            .default_provider
            .as_deref()
            .unwrap_or("anthropic");

        let llm_config = llm::LlmConfig::from_config(&self.config.llm, provider_name)
            .map_err(|e| anyhow::anyhow!("Failed to create LLM config: {e}. Make sure the appropriate API key is set (e.g., ANTHROPIC_API_KEY, OPENAI_API_KEY, or OLLAMA_BASE_URL)."))?;

        let provider = llm::create_provider(&llm_config)
            .map_err(|e| anyhow::anyhow!("Failed to create provider: {e}"))?;

        let messages = vec![
            ChatMessage {
                role: ChatRole::System,
                content: self.agent_mode.system_prompt().to_string(),
            },
            ChatMessage {
                role: ChatRole::User,
                content: message.to_string(),
            },
        ];

        let response = provider
            .chat(messages)
            .await
            .map_err(|e| anyhow::anyhow!("LLM API call failed: {e}"))?;

        Ok(response)
    }

    async fn open_external_editor(&mut self) -> anyhow::Result<()> {
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
    pub fn resize(&mut self) {}

    /// Draw the TUI
    pub fn draw(&self, f: &mut Frame<'_>) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(10),
                Constraint::Length(3),
                Constraint::Length(1),
            ])
            .split(f.area());

        self.draw_header(f, chunks[0]);
        self.draw_main_content(f, chunks[1]);
        self.draw_input(f, chunks[2]);
        self.draw_status(f, chunks[3]);

        if let Some(ref error) = self.error_message {
            self.draw_popup(f, "Error", error);
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

        let session_info = if let Some(session) = &self.session {
            format!("{} msgs", session.messages.len())
        } else {
            "no session".to_string()
        };

        let title = Line::from(vec![
            Span::styled("CLAWDIUS", theme.title()),
            Span::raw("  "),
            Span::styled(mode_text, Style::new().fg(theme.accent)),
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

    fn draw_main_content(&self, f: &mut Frame<'_>, area: Rect) {
        match self.mode {
            AppMode::Chat => self.draw_messages(f, area),
            AppMode::FileBrowser => self.draw_file_browser(f, area),
            AppMode::Diff => self.draw_diff(f, area),
            AppMode::Help => self.draw_help(f, area),
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

    fn draw_help(&self, f: &mut Frame<'_>, area: Rect) {
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
                "File Browser:",
                Style::default().fg(Color::Yellow),
            )),
            Line::from("  j/k        - Move up/down"),
            Line::from("  Enter      - Enter directory / View file"),
            Line::from("  Space      - Toggle file selection"),
            Line::from("  r          - Refresh directory"),
            Line::default(),
            Line::from(Span::styled(
                "Diff View:",
                Style::default().fg(Color::Yellow),
            )),
            Line::from("  j/k        - Scroll diff"),
            Line::from("  Ctrl+d/u   - Page down/up"),
            Line::default(),
            Line::from(Span::styled(
                "Commands:",
                Style::default().fg(Color::Yellow),
            )),
            Line::from("  :q         - Quit"),
            Line::from("  :help      - Show help"),
            Line::from("  :files     - Open file browser"),
            Line::from("  :diff      - Open diff view"),
            Line::from("  :clear     - Clear chat"),
            Line::from("  :mode <n>  - Switch agent mode (code, architect, debug, etc.)"),
            Line::from("  :modes     - List available modes"),
            Line::default(),
            Line::from(Span::styled(
                "Agent Modes:",
                Style::default().fg(Color::Yellow),
            )),
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
        }

        status_parts.push(Span::styled(" │ ", theme.border()));
        status_parts.push(Span::styled("? help", theme.muted()));

        let status = Line::from(status_parts);
        let paragraph = Paragraph::new(status);
        f.render_widget(paragraph, area);
    }

    fn draw_popup(&self, f: &mut Frame<'_>, title: &str, content: &str) {
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
