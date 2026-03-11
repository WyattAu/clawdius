//! UI components

use super::vim::VimMode;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Paragraph, Widget},
};

#[allow(dead_code)]
pub struct ModeIndicator {
    mode: VimMode,
}

#[allow(dead_code)]
impl ModeIndicator {
    pub fn new(mode: VimMode) -> Self {
        Self { mode }
    }

    fn mode_text(&self) -> (&'static str, Style) {
        match self.mode {
            VimMode::Normal => (
                "-- NORMAL --",
                Style::default()
                    .fg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            ),
            VimMode::Insert => (
                "-- INSERT --",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            VimMode::Visual => (
                "-- VISUAL --",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            VimMode::Command => (
                "-- COMMAND --",
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ),
        }
    }
}

impl Widget for ModeIndicator {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        let (text, style) = self.mode_text();
        Paragraph::new(text).style(style).render(area, buf);
    }
}

#[allow(dead_code)]
pub fn get_cursor_shape(mode: VimMode) -> &'static str {
    match mode {
        VimMode::Normal => "block",
        VimMode::Insert => "line",
        VimMode::Visual => "block",
        VimMode::Command => "underscore",
    }
}
