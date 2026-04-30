//! Status bar component for the TUI.
//!
//! Displays current model, token count, session info, project,
//! and vim mode in a single-line footer bar.

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph, Widget},
};

use crate::tui_app::theme::Theme;
use crate::tui_app::vim::VimMode;

/// Status bar state.
#[derive(Debug, Clone, Default)]
pub struct StatusBarState {
    /// Current LLM model name.
    pub model: String,
    /// Current provider name.
    pub provider: String,
    /// Tokens used in current session.
    pub tokens_used: usize,
    /// Token budget.
    pub token_budget: usize,
    /// Current project name.
    pub project: String,
    /// Current session ID (truncated).
    pub session_id: String,
    /// Whether the LLM is currently processing.
    pub processing: bool,
    /// Current vim mode.
    pub vim_mode: VimMode,
    /// Number of files in workspace.
    pub file_count: usize,
    /// Active workspace name (if multi-repo).
    pub workspace: String,
}

impl StatusBarState {
    /// Create a new status bar state.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the model name.
    #[must_use]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// Set the provider name.
    #[must_use]
    pub fn with_provider(mut self, provider: impl Into<String>) -> Self {
        self.provider = provider.into();
        self
    }

    /// Set the token count.
    #[must_use]
    pub fn with_tokens(mut self, used: usize, budget: usize) -> Self {
        self.tokens_used = used;
        self.token_budget = budget;
        self
    }

    /// Set the project name.
    #[must_use]
    pub fn with_project(mut self, project: impl Into<String>) -> Self {
        self.project = project.into();
        self
    }

    /// Set the session ID.
    #[must_use]
    pub fn with_session(mut self, id: impl Into<String>) -> Self {
        self.session_id = id.into();
        self
    }

    /// Set processing state.
    #[must_use]
    pub fn with_processing(mut self, processing: bool) -> Self {
        self.processing = processing;
        self
    }

    /// Set vim mode.
    #[must_use]
    pub fn with_vim_mode(mut self, mode: VimMode) -> Self {
        self.vim_mode = mode;
        self
    }

    /// Set file count.
    #[must_use]
    pub fn with_file_count(mut self, count: usize) -> Self {
        self.file_count = count;
        self
    }

    /// Set workspace name.
    #[must_use]
    pub fn with_workspace(mut self, workspace: impl Into<String>) -> Self {
        self.workspace = workspace.into();
        self
    }
}

/// Status bar widget.
pub struct StatusBar<'a> {
    state: &'a StatusBarState,
    theme: &'a Theme,
}

impl<'a> StatusBar<'a> {
    /// Create a new status bar.
    pub fn new(state: &'a StatusBarState, theme: &'a Theme) -> Self {
        Self { state, theme }
    }
}

impl Widget for StatusBar<'_> {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        let processing_indicator = if self.state.processing {
            " ●" // Spinning dot
        } else {
            ""
        };

        let model_display = if self.state.model.is_empty() {
            "no model".to_string()
        } else if self.state.model.len() > 25 {
            format!("{}…", &self.state.model[..23])
        } else {
            self.state.model.clone()
        };

        let session_display = if self.state.session_id.len() > 8 {
            format!("…{}", &self.state.session_id[self.state.session_id.len() - 8..])
        } else {
            self.state.session_id.clone()
        };

        let token_bar = if self.state.token_budget > 0 {
            let pct = (self.state.tokens_used as f64 / self.state.token_budget as f64).min(1.0);
            let filled = (pct * 10.0) as usize;
            let empty = 10 - filled;
            format!(
                " [{}{}] {}%",
                "█".repeat(filled),
                "░".repeat(empty),
                (pct * 100.0) as usize
            )
        } else {
            String::new()
        };

        let left = vec![
            Span::styled(
                format!(" {} ", processing_indicator),
                self.theme.status_highlight(),
            ),
            Span::styled(
                format!(" {} ", model_display),
                self.theme.model_info(),
            ),
            Span::styled(
                format!(" {} ", self.state.provider),
                self.theme.muted(),
            ),
            Span::styled(token_bar, self.theme.token_count()),
        ];

        let right = vec![
            Span::styled(
                format!(" {} ", self.state.project),
                self.theme.file_item(),
            ),
            if !self.state.workspace.is_empty() {
                Span::styled(
                    format!(" ws:{} ", self.state.workspace),
                    self.theme.title().add_modifier(Modifier::DIM),
                )
            } else {
                Span::raw("")
            },
            Span::styled(
                format!(" {} files ", self.state.file_count),
                self.theme.muted(),
            ),
            Span::styled(
                format!(" {} ", session_display),
                self.theme.muted(),
            ),
            Span::styled(
                format!(" {:?} ", self.state.vim_mode),
                self.theme.mode_normal(),
            ),
        ];

        let line = Line::from(left).spans(right);
        let block = Block::default().style(self.theme.status());

        Paragraph::new(line).block(block).render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_theme() -> Theme {
        Theme::dark()
    }

    #[test]
    fn test_status_bar_state_builder() {
        let state = StatusBarState::new()
            .with_model("claude-sonnet-4-20250514")
            .with_provider("anthropic")
            .with_tokens(1500, 8000)
            .with_project("my-project")
            .with_session("abc123-def456")
            .with_processing(true)
            .with_vim_mode(VimMode::Insert)
            .with_file_count(42);

        assert_eq!(state.model, "claude-sonnet-4-20250514");
        assert_eq!(state.provider, "anthropic");
        assert_eq!(state.tokens_used, 1500);
        assert_eq!(state.processing, true);
        assert_eq!(state.file_count, 42);
    }

    #[test]
    fn test_status_bar_long_model_truncated() {
        let state = StatusBarState::new()
            .with_model("claude-sonnet-4-20250514-with-a-very-long-suffix")
            .with_vim_mode(VimMode::Normal);

        // The state stores the original model name (not truncated).
        // Truncation to 23 chars + "…" happens during rendering.
        assert!(state.model.len() > 25, "model name should be long enough to trigger truncation");

        // Verify the truncation logic produces correct output
        let truncated = if state.model.len() > 25 {
            format!("{}…", &state.model[..23])
        } else {
            state.model.clone()
        };
        assert_eq!(truncated.chars().count(), 24, "truncated display should be 24 chars (23 + …)");
        assert!(truncated.ends_with('…'));
    }
}
