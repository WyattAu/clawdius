use ratatui::{
    style::{Color, Style},
    text::Span,
};

const FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub struct Spinner {
    frame: usize,
}

impl Spinner {
    pub fn new() -> Self {
        Self { frame: 0 }
    }

    pub fn tick(&mut self) {
        self.frame = (self.frame + 1) % FRAMES.len();
    }

    pub fn render(&self) -> Span<'static> {
        Span::styled(
            FRAMES[self.frame].to_string(),
            Style::default().fg(Color::Cyan),
        )
    }
}

impl Default for Spinner {
    fn default() -> Self {
        Self::new()
    }
}
