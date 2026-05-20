//! Command autocomplete for the TUI command mode.
//!
//! Provides tab-completion for `:` commands (vim-style) when in Command input mode.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState},
    Frame,
};

/// All supported `:` commands in the TUI.
const COMMANDS: &[&str] = &[
    "q",
    "quit",
    "help",
    "?",
    "files",
    "ls",
    "diff",
    "clear",
    "tools",
    "compact",
    "sessions",
    "new",
    "newsession",
    "mode",
    "modes",
    "provider",
    "timeout",
    "sprint",
    "auto",
    "generate",
    "test",
    "build",
    "check",
    "doc",
    "verify",
    "checkpoint",
    "timeline",
    "memory",
    "analyze",
    "config",
    "watch",
    "split",
    "sp",
    "vsplit",
    "vsp",
    "unsplit",
    "only",
    "secondary",
    "git",
];

/// Short descriptions for the most common commands.
fn command_description(cmd: &str) -> &'static str {
    match cmd {
        "q" | "quit" => "Quit Clawdius",
        "help" | "?" => "Show help",
        "files" | "ls" => "Browse files",
        "diff" => "View git diff",
        "clear" => "Clear chat",
        "tools" => "Toggle tools",
        "compact" => "Compact history",
        "sessions" => "List sessions",
        "new" | "newsession" => "New session",
        "mode" => "Switch agent mode",
        "modes" => "List modes",
        "provider" => "Switch LLM provider",
        "timeout" => "Set API timeout",
        "sprint" => "Start sprint",
        "auto" => "Auto mode",
        "generate" => "Generate code",
        "test" => "Run cargo test",
        "build" | "check" => "Build/check project",
        "doc" => "Generate documentation",
        "verify" => "Verify Lean4 proof",
        "checkpoint" => "Show checkpoints",
        "timeline" => "File timeline",
        "memory" => "Project memory",
        "analyze" => "Analyze code",
        "config" => "Show configuration",
        "watch" => "Toggle file watcher",
        "split" | "sp" => "Horizontal split",
        "vsplit" | "vsp" => "Vertical split",
        "unsplit" | "only" => "Single pane",
        "secondary" => "Set secondary panel",
        "git" => "Git status/log/diff",
        _ => "",
    }
}

#[derive(Default)]
pub struct CommandAutocomplete {
    suggestions: Vec<String>,
    state: ListState,
    visible: bool,
}

impl CommandAutocomplete {
    pub fn new() -> Self {
        let mut state = ListState::default();
        state.select(Some(0));
        Self {
            suggestions: Vec::new(),
            state,
            visible: false,
        }
    }

    pub const fn is_visible(&self) -> bool {
        self.visible
    }

    /// Update suggestions based on the current command input (after `:`).
    pub fn update(&mut self, input: &str) {
        let query = input.strip_prefix(':').unwrap_or(input);
        let query_lower = query.to_lowercase();

        if query.is_empty() {
            self.suggestions = COMMANDS
                .iter()
                .map(std::string::ToString::to_string)
                .collect();
            self.visible = true;
            self.state.select(Some(0));
        } else {
            self.suggestions = COMMANDS
                .iter()
                .filter(|cmd| cmd.to_lowercase().starts_with(&query_lower))
                .map(std::string::ToString::to_string)
                .collect();
            if self.suggestions.is_empty() {
                self.visible = false;
            } else {
                self.visible = true;
                self.state.select(Some(0));
            }
        }
    }

    pub fn hide(&mut self) {
        self.visible = false;
        self.suggestions.clear();
    }

    #[allow(dead_code, clippy::missing_const_for_fn)]
    pub fn next(&mut self) {
        if !self.suggestions.is_empty() {
            let i = match self.state.selected() {
                Some(i) if i >= self.suggestions.len() - 1 => 0,
                Some(i) => i + 1,
                None => 0,
            };
            self.state.select(Some(i));
        }
    }

    #[allow(dead_code, clippy::missing_const_for_fn)]
    pub fn previous(&mut self) {
        if !self.suggestions.is_empty() {
            let i = match self.state.selected() {
                Some(0) => self.suggestions.len() - 1,
                Some(i) => i - 1,
                None => 0,
            };
            self.state.select(Some(i));
        }
    }

    /// Returns the currently selected command (with `:` prefix).
    pub fn selected(&self) -> Option<String> {
        self.state
            .selected()
            .and_then(|i| self.suggestions.get(i))
            .map(|s| format!(":{s}"))
    }

    #[allow(clippy::cast_possible_truncation)]
    pub fn render(&mut self, f: &mut Frame<'_>, area: Rect) {
        if !self.visible || self.suggestions.is_empty() {
            return;
        }

        let count = self.suggestions.len().min(10) as u16;
        let popup_area = Rect {
            x: area.x,
            y: area.y.saturating_sub(count + 2),
            width: area.width.min(50),
            height: count + 2,
        };

        f.render_widget(Clear, popup_area);

        let items: Vec<ListItem<'_>> = self
            .suggestions
            .iter()
            .map(|cmd| {
                let style = Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD);

                let desc = command_description(cmd);
                let content = if desc.is_empty() {
                    Line::from(Span::styled(format!(":{cmd}"), style))
                } else {
                    Line::from(vec![
                        Span::styled(format!(":{cmd}"), style),
                        Span::raw(" "),
                        Span::styled(desc, Style::default().fg(Color::Gray)),
                    ])
                };

                ListItem::new(content)
            })
            .collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Commands"))
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(">> ");

        f.render_stateful_widget(list, popup_area, &mut self.state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_query_shows_all() {
        let mut ac = CommandAutocomplete::new();
        ac.update(":");
        assert!(ac.is_visible());
        assert_eq!(ac.suggestions.len(), COMMANDS.len());
    }

    #[test]
    fn test_partial_match() {
        let mut ac = CommandAutocomplete::new();
        ac.update(":mo");
        assert!(ac.is_visible());
        assert!(ac.suggestions.iter().any(|s| s == "mode"));
        assert!(ac.suggestions.iter().any(|s| s == "modes"));
        assert!(!ac.suggestions.iter().any(|s| s == "quit"));
    }

    #[test]
    fn test_no_match_hides() {
        let mut ac = CommandAutocomplete::new();
        ac.update(":zzzzz");
        assert!(!ac.is_visible());
    }

    #[test]
    fn test_navigation() {
        let mut ac = CommandAutocomplete::new();
        ac.update(":");
        let first = ac.selected().unwrap();
        ac.next();
        let second = ac.selected().unwrap();
        assert_ne!(first, second);
    }

    #[test]
    fn test_selected_has_prefix() {
        let mut ac = CommandAutocomplete::new();
        ac.update(":q");
        let sel = ac.selected().unwrap();
        assert!(sel.starts_with(':'));
    }
}
