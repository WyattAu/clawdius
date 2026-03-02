//! Rigor Score visualization widget
//!
//! Displays the SOP compliance score (0.0-1.0) as a progress bar with color coding.

use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

pub struct RigorScore {
    bar_width: usize,
}

impl RigorScore {
    pub fn new() -> Self {
        Self { bar_width: 10 }
    }

    pub fn render(&self, score: f64, fps: f64) -> Paragraph<'static> {
        let filled = (score * self.bar_width as f64).round() as usize;
        let filled = filled.min(self.bar_width);

        let (color, label) = if score >= 0.9 {
            (Color::Green, "EXCELLENT")
        } else if score >= 0.7 {
            (Color::Yellow, "GOOD")
        } else if score >= 0.5 {
            (Color::Rgb(255, 165, 0), "MODERATE")
        } else {
            (Color::Red, "LOW")
        };

        let bar: String = "█".repeat(filled) + &"░".repeat(self.bar_width - filled);

        Paragraph::new(Line::from(vec![
            Span::styled("Rigor: ", Style::default().fg(Color::White)),
            Span::styled(format!("{:.2} ", score), Style::default().fg(color).bold()),
            Span::styled(bar, Style::default().fg(color)),
            Span::raw(" "),
            Span::styled(label, Style::default().fg(color)),
            Span::raw("  "),
            Span::styled(
                format!("{:.0} fps", fps),
                Style::default().fg(Color::DarkGray),
            ),
        ]))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Metrics")
                .title_style(Style::default().fg(Color::White)),
        )
    }

    pub fn set_bar_width(&mut self, width: usize) {
        self.bar_width = width;
    }
}

impl Default for RigorScore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rigor_score_new() {
        let widget = RigorScore::new();
        assert_eq!(widget.bar_width, 10);
    }

    #[test]
    fn test_bar_width() {
        let mut widget = RigorScore::new();
        widget.set_bar_width(20);
        assert_eq!(widget.bar_width, 20);
    }
}
