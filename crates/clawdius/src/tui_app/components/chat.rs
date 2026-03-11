use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
    Frame,
};

use crate::tui_app::types::Message;

#[derive(Clone)]
pub struct ChatView {
    messages: Vec<Message>,
    scroll: usize,
    scroll_state: ScrollbarState,
}

impl ChatView {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            scroll: 0,
            scroll_state: ScrollbarState::default(),
        }
    }

    pub fn add_message(&mut self, msg: Message) {
        self.messages.push(msg);
    }

    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    pub fn scroll_up(&mut self) {
        if self.scroll > 0 {
            self.scroll -= 1;
        }
    }

    pub fn scroll_down(&mut self, max_visible: usize) {
        let total = self.calculate_total_lines();
        if self.scroll + max_visible < total {
            self.scroll += 1;
        }
    }

    pub fn scroll_page_up(&mut self, page_size: usize) {
        self.scroll = self.scroll.saturating_sub(page_size);
    }

    pub fn scroll_page_down(&mut self, page_size: usize, max_visible: usize) {
        let total = self.calculate_total_lines();
        self.scroll = (self.scroll + page_size).min(total.saturating_sub(max_visible));
    }

    pub fn scroll_to_bottom(&mut self, max_visible: usize) {
        let total = self.calculate_total_lines();
        self.scroll = total.saturating_sub(max_visible);
    }

    fn calculate_total_lines(&self) -> usize {
        self.messages
            .iter()
            .map(|m| m.content.lines().count().max(1))
            .sum()
    }

    pub fn render(&mut self, f: &mut Frame<'_>, area: Rect) {
        let mut lines: Vec<Line<'_>> = Vec::new();

        for msg in &self.messages {
            let role_style = match msg.role {
                crate::tui_app::types::MessageRole::User => Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
                crate::tui_app::types::MessageRole::Assistant => Style::default()
                    .fg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
                crate::tui_app::types::MessageRole::System => Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
                crate::tui_app::types::MessageRole::Tool => Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            };

            let role_name = match msg.role {
                crate::tui_app::types::MessageRole::User => "You",
                crate::tui_app::types::MessageRole::Assistant => "Clawdius",
                crate::tui_app::types::MessageRole::System => "System",
                crate::tui_app::types::MessageRole::Tool => "Tool",
            };

            lines.push(Line::from(vec![Span::styled(
                format!("{}: ", role_name),
                role_style,
            )]));

            for line in msg.content.lines() {
                lines.push(Line::from(line.to_string()));
            }
            lines.push(Line::default());
        }

        let total_lines = lines.len();
        let visible_lines = area.height.saturating_sub(2) as usize;

        let scroll_offset = self.scroll.min(total_lines.saturating_sub(visible_lines));

        let paragraph = Paragraph::new(Text::from(lines))
            .block(Block::default().borders(Borders::ALL).title("Chat"))
            .scroll((scroll_offset as u16, 0));

        f.render_widget(paragraph, area);

        if total_lines > visible_lines {
            self.scroll_state = self
                .scroll_state
                .content_length(total_lines)
                .viewport_content_length(visible_lines)
                .position(scroll_offset);

            let scrollbar = Scrollbar::default().orientation(ScrollbarOrientation::VerticalRight);

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
