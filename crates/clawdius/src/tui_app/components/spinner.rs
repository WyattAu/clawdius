//! Animated spinner for loading states

use ratatui::{
    style::{Modifier, Style},
    text::Span,
};
use std::time::Instant;

use crate::tui_app::theme;

/// Modern spinner animation frames (Braille patterns)
const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Animated spinner component
#[derive(Clone)]
pub struct Spinner {
    frame: usize,
    last_tick: Instant,
}

impl Spinner {
    /// Create a new spinner
    pub fn new() -> Self {
        Self {
            frame: 0,
            last_tick: Instant::now(),
        }
    }

    /// Advance the spinner animation
    pub fn tick(&mut self) {
        let elapsed = self.last_tick.elapsed().as_millis();
        if elapsed >= 80 {
            self.frame = (self.frame + 1) % SPINNER_FRAMES.len();
            self.last_tick = Instant::now();
        }
    }

    /// Render the spinner as a styled span
    pub fn render(&self) -> Span<'static> {
        let theme = theme::current();
        Span::styled(
            SPINNER_FRAMES[self.frame].to_string(),
            Style::new().fg(theme.accent).add_modifier(Modifier::BOLD),
        )
    }

    /// Get the current frame as a string
    pub fn frame(&self) -> &'static str {
        SPINNER_FRAMES[self.frame]
    }
}

impl Default for Spinner {
    fn default() -> Self {
        Self::new()
    }
}
