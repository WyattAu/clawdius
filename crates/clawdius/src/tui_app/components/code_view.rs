//! Code view component — displays source code with line numbers and syntax highlighting.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use super::syntax::SyntaxHighlighter;

/// A code viewer with line numbers, syntax highlighting, and scroll support.
pub struct CodeView {
    /// File path being viewed.
    pub file_path: Option<String>,
    /// Source code content.
    pub content: String,
    /// Language for syntax highlighting.
    pub language: String,
    /// Current scroll offset (line number, 0-indexed).
    pub scroll_offset: usize,
    /// Cursor line (0-indexed).
    pub cursor_line: usize,
    /// Whether to show line numbers.
    pub show_line_numbers: bool,
    /// Whether to highlight the current line.
    pub highlight_current_line: bool,
}

impl CodeView {
    /// Create a new empty code view.
    #[must_use]
    pub fn new() -> Self {
        Self {
            file_path: None,
            content: String::new(),
            language: String::new(),
            scroll_offset: 0,
            cursor_line: 0,
            show_line_numbers: true,
            highlight_current_line: true,
        }
    }

    /// Create a code view with content.
    #[must_use]
    pub fn with_content(content: impl Into<String>, language: impl Into<String>) -> Self {
        let mut view = Self::new();
        view.content = content.into();
        view.language = language.into();
        view
    }

    /// Set the file path.
    #[must_use]
    pub fn file_path(mut self, path: impl Into<String>) -> Self {
        self.file_path = Some(path.into());
        self
    }

    /// Set the content.
    pub fn set_content(&mut self, content: impl Into<String>, language: impl Into<String>) {
        self.content = content.into();
        self.language = language.into();
        self.scroll_offset = 0;
        self.cursor_line = 0;
    }

    /// Scroll up by one line.
    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }

    /// Scroll down by one line.
    pub fn scroll_down(&mut self, visible_lines: usize) {
        let max_scroll = self.line_count().saturating_sub(visible_lines);
        self.scroll_offset = (self.scroll_offset + 1).min(max_scroll);
    }

    /// Scroll to a specific line.
    pub fn scroll_to_line(&mut self, line: usize) {
        self.scroll_offset = line;
        self.cursor_line = line;
    }

    /// Scroll to make the cursor visible.
    pub fn ensure_cursor_visible(&mut self, visible_lines: usize) {
        if self.cursor_line < self.scroll_offset {
            self.scroll_offset = self.cursor_line;
        } else if self.cursor_line >= self.scroll_offset + visible_lines {
            self.scroll_offset = self.cursor_line.saturating_sub(visible_lines.saturating_sub(1));
        }
    }

    /// Get the total number of lines.
    #[must_use]
    pub fn line_count(&self) -> usize {
        if self.content.is_empty() {
            0
        } else {
            self.content.lines().count()
        }
    }

    /// Get the current line number (1-indexed for display).
    #[must_use]
    pub fn current_line_number(&self) -> usize {
        self.cursor_line + 1
    }

    /// Get the total column count of the current line.
    #[must_use]
    pub fn current_line_length(&self) -> usize {
        self.content
            .lines()
            .nth(self.cursor_line)
            .map(|l| l.len())
            .unwrap_or(0)
    }

    /// Render the code view.
    pub fn render(&mut self, f: &mut Frame<'_>, area: Rect, syntax: &SyntaxHighlighter) {
        let title = self
            .file_path
            .as_deref()
            .unwrap_or("(no file)")
            .rsplit('/')
            .next()
            .unwrap_or("(no file)");

        let block = Block::default()
            .title(format!(" {} ", title))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));

        let inner = block.inner(area);
        f.render_widget(block, area);

        let visible_height = inner.height as usize;
        if visible_height == 0 || self.content.is_empty() {
            return;
        }

        let lines: Vec<&str> = self.content.lines().collect();
        let start = self.scroll_offset.min(lines.len());
        let end = (start + visible_height).min(lines.len());

        let mut text_lines: Vec<Line<'_>> = Vec::with_capacity(visible_height);

        for i in start..end {
            let line_num = i + 1;
            let is_current = i == self.cursor_line && self.highlight_current_line;

            let mut spans: Vec<Span<'_>> = Vec::new();

            // Line number
            if self.show_line_numbers {
                let line_num_str = format!("{line_num:>4} │ ");
                let line_num_style = if is_current {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::DarkGray)
                };
                spans.push(Span::styled(line_num_str, line_num_style));
            }

            // Code content (plain text — syntax highlighting can be added per-line)
            let code_line = lines[i];
            if is_current {
                spans.push(Span::styled(
                    code_line.to_string(),
                    Style::default().fg(Color::White),
                ));
            } else {
                spans.push(Span::styled(
                    code_line.to_string(),
                    Style::default().fg(Color::Gray),
                ));
            }

            text_lines.push(Line::from(spans));
        }

        let paragraph = Paragraph::new(text_lines).wrap(Wrap { trim: false });
        f.render_widget(paragraph, inner);
    }

    /// Render with syntax highlighting.
    pub fn render_highlighted(&mut self, f: &mut Frame<'_>, area: Rect, syntax: &SyntaxHighlighter) {
        let title = self
            .file_path
            .as_deref()
            .unwrap_or("(no file)")
            .rsplit('/')
            .next()
            .unwrap_or("(no file)");

        let lang_label = if self.language.is_empty() {
            String::new()
        } else {
            format!(" [{}]", self.language)
        };

        let block = Block::default()
            .title(format!(" {}{} ", title, lang_label))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));

        let inner = block.inner(area);
        f.render_widget(block, area);

        let visible_height = inner.height as usize;
        if visible_height == 0 || self.content.is_empty() {
            return;
        }

        // Use syntax highlighting
        let highlighted = syntax.highlight(&self.content, &self.language);
        let lines: Vec<&str> = self.content.lines().collect();
        let start = self.scroll_offset.min(lines.len());
        let end = (start + visible_height).min(lines.len());

        // Build highlighted text per line
        let mut text_lines: Vec<Line<'_>> = Vec::with_capacity(visible_height);
        let mut hl_idx = 0;

        // Group highlighted spans by line
        let mut current_line_spans: Vec<Span<'_>> = Vec::new();
        let mut current_line_num = 0;

        for (text, color) in &highlighted {
            // Count newlines in text
            for ch in text.chars() {
                if ch == '\n' {
                    // Emit line
                    if current_line_num >= start && current_line_num < end {
                        let is_current =
                            current_line_num == self.cursor_line && self.highlight_current_line;

                        let mut spans: Vec<Span<'_>> = Vec::new();

                        if self.show_line_numbers {
                            let line_num_str = format!("{:>4} │ ", current_line_num + 1);
                            let line_num_style = if is_current {
                                Style::default()
                                    .fg(Color::Yellow)
                                    .add_modifier(Modifier::BOLD)
                            } else {
                                Style::default().fg(Color::DarkGray)
                            };
                            spans.push(Span::styled(line_num_str, line_num_style));
                        }

                        spans.extend(current_line_spans.drain(..));

                        if is_current {
                            // Could add background highlight here
                        }

                        text_lines.push(Line::from(spans));
                    }
                    current_line_spans.clear();
                    current_line_num += 1;
                } else if current_line_num >= start && current_line_num < end {
                    let style = match color {
                        Some(c) => Style::default().fg(*c),
                        None => Style::default().fg(Color::Gray),
                    };
                    current_line_spans.push(Span::styled(ch.to_string(), style));
                } else {
                    // Skip lines outside visible range
                    current_line_num += 0;
                }
            }
        }

        let paragraph = Paragraph::new(text_lines).wrap(Wrap { trim: false });
        f.render_widget(paragraph, inner);
    }
}

impl Default for CodeView {
    fn default() -> Self {
        Self::new()
    }
}

// ─────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_code_view_creation() {
        let view = CodeView::new();
        assert!(view.content.is_empty());
        assert_eq!(view.line_count(), 0);
    }

    #[test]
    fn test_code_view_with_content() {
        let view = CodeView::with_content("line1\nline2\nline3", "rust");
        assert_eq!(view.line_count(), 3);
    }

    #[test]
    fn test_scroll() {
        let mut view = CodeView::with_content(
            (0..100).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n"),
            "rust",
        );
        assert_eq!(view.scroll_offset, 0);
        view.scroll_down(10);
        assert_eq!(view.scroll_offset, 1);
        view.scroll_up();
        assert_eq!(view.scroll_offset, 0);
    }

    #[test]
    fn test_scroll_clamp() {
        let mut view = CodeView::with_content("a\nb\nc", "rust");
        view.scroll_up(); // should stay at 0
        assert_eq!(view.scroll_offset, 0);
        view.scroll_down(100); // should clamp
        assert_eq!(view.scroll_offset, 0); // 3 lines - 100 visible = 0
    }

    #[test]
    fn test_set_content_resets_scroll() {
        let mut view = CodeView::with_content("a\nb\nc", "rust");
        view.scroll_down(2);
        view.set_content("x\ny", "py");
        assert_eq!(view.scroll_offset, 0);
        assert_eq!(view.line_count(), 2);
    }

    #[test]
    fn test_file_path_builder() {
        let view = CodeView::with_content("fn main() {}", "rust")
            .file_path("/src/main.rs");
        assert_eq!(view.file_path.as_deref(), Some("/src/main.rs"));
    }

    #[test]
    fn test_current_line_info() {
        let view = CodeView::with_content("short\na very long line here\nshort", "rust");
        assert_eq!(view.current_line_number(), 1);
        assert_eq!(view.current_line_length(), 5);
    }

    #[test]
    fn test_ensure_cursor_visible() {
        let mut view = CodeView::with_content(
            (0..50).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n"),
            "rust",
        );
        view.cursor_line = 40;
        view.ensure_cursor_visible(10);
        assert!(view.scroll_offset >= 31); // 40 - 10 + 1
    }
}
