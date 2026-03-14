//! Chat view component with modern styling

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, Borders, Padding, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
    },
    Frame,
};

use crate::tui_app::theme;
use crate::tui_app::types::{Message, MessageRole};

/// Chat message view with scroll support
#[derive(Clone)]
pub struct ChatView {
    messages: Vec<Message>,
    scroll: usize,
    scroll_state: ScrollbarState,
}

impl ChatView {
    /// Create a new empty chat view
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            scroll: 0,
            scroll_state: ScrollbarState::default(),
        }
    }

    /// Add a message to the chat
    pub fn add_message(&mut self, msg: Message) {
        self.messages.push(msg);
    }

    /// Get all messages
    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    /// Scroll up by one line
    pub fn scroll_up(&mut self) {
        self.scroll = self.scroll.saturating_sub(1);
    }

    /// Scroll down by one line
    pub fn scroll_down(&mut self, max_visible: usize) {
        let total = self.calculate_total_lines();
        if self.scroll + max_visible < total {
            self.scroll += 1;
        }
    }

    /// Scroll up by a page
    pub fn scroll_page_up(&mut self, page_size: usize) {
        self.scroll = self.scroll.saturating_sub(page_size);
    }

    /// Scroll down by a page
    pub fn scroll_page_down(&mut self, page_size: usize, max_visible: usize) {
        let total = self.calculate_total_lines();
        self.scroll = (self.scroll + page_size).min(total.saturating_sub(max_visible));
    }

    /// Scroll to the bottom of the chat
    pub fn scroll_to_bottom(&mut self, max_visible: usize) {
        let total = self.calculate_total_lines();
        self.scroll = total.saturating_sub(max_visible);
    }

    /// Calculate total lines needed to display all messages
    fn calculate_total_lines(&self) -> usize {
        self.messages
            .iter()
            .map(|m| m.content.lines().count().max(1) + 2) // +2 for header and spacing
            .sum()
    }

    /// Render the chat view
    pub fn render(&mut self, f: &mut Frame<'_>, area: Rect) {
        let theme = theme::current();
        let mut lines: Vec<Line<'_>> = Vec::new();

        for msg in &self.messages {
            // Role header with styling
            let (role_style, role_name, indicator) = match msg.role {
                MessageRole::User => (theme.user_message(), "You", ">"),
                MessageRole::Assistant => (theme.assistant_message(), "Clawdius", "<"),
                MessageRole::System => (theme.system_message(), "System", "#"),
                MessageRole::Tool => (theme.tool_message(), "Tool", "@"),
            };

            // Role header line
            lines.push(Line::from(vec![
                Span::styled(indicator, role_style),
                Span::raw(" "),
                Span::styled(role_name, role_style),
                Span::raw(" "),
                Span::styled(
                    format!("[{}]", msg.timestamp.format("%H:%M")),
                    theme.muted(),
                ),
            ]));

            // Message content with proper indentation
            for line in msg.content.lines() {
                lines.push(Line::from(vec![
                    Span::raw("  "), // Indent for readability
                    Span::styled(line, Style::new().fg(theme.text)),
                ]));
            }

            // Spacing between messages
            lines.push(Line::default());
        }

        // Handle empty state
        if lines.is_empty() {
            lines.push(Line::default());
            lines.push(Line::from(vec![Span::styled(
                "  No messages yet",
                theme.muted(),
            )]));
            lines.push(Line::from(vec![
                Span::styled("  Press ", theme.muted()),
                Span::styled("i", theme.mode_insert()),
                Span::styled(" to start typing", theme.muted()),
            ]));
        }

        let total_lines = lines.len();
        let visible_lines = area.height.saturating_sub(2) as usize;
        let scroll_offset = self.scroll.min(total_lines.saturating_sub(visible_lines));

        // Create block with modern styling
        let block = Block::default()
            .borders(Borders::LEFT | Borders::RIGHT)
            .border_style(theme.border())
            .padding(Padding::horizontal(1));

        let paragraph = Paragraph::new(Text::from(lines))
            .block(block)
            .scroll((scroll_offset as u16, 0));

        f.render_widget(paragraph, area);

        // Render scrollbar if needed
        if total_lines > visible_lines {
            self.scroll_state = self
                .scroll_state
                .content_length(total_lines)
                .viewport_content_length(visible_lines)
                .position(scroll_offset);

            let scrollbar = Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None)
                .track_symbol(Some(" "))
                .thumb_symbol("│");

            let scrollbar_area = Rect {
                x: area.right().saturating_sub(1),
                y: area.top() + 1,
                width: 1,
                height: area.height.saturating_sub(2),
            };

            f.render_stateful_widget(scrollbar, scrollbar_area, &mut self.scroll_state);
        }
    }
}

impl Default for ChatView {
    fn default() -> Self {
        Self::new()
    }
}
