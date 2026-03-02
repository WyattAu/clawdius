//! Phase View widget - Current phase progress indicator

use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

const PHASE_NAMES: [&str; 13] = [
    "Init", "Analysis", "Spec", "Plan", "Design", "Impl-1", "Impl-2", "Impl-3", "Impl-4", "Impl-5",
    "Review", "Deploy", "Complete",
];

pub struct PhaseView;

impl PhaseView {
    pub fn new() -> Self {
        Self
    }

    pub fn render(&self, phase: u8) -> Paragraph<'static> {
        let phase_name = PHASE_NAMES.get(phase as usize).unwrap_or(&"Unknown");

        let phase_color = if phase >= 11 {
            Color::Green
        } else if phase >= 6 {
            Color::Yellow
        } else if phase >= 1 {
            Color::Cyan
        } else {
            Color::DarkGray
        };

        let progress_dots: String = (0..12)
            .map(|i| if i < phase as usize { "●" } else { "○" })
            .collect();

        Paragraph::new(Line::from(vec![
            Span::styled("Phase: ", Style::default().fg(Color::White)),
            Span::styled(
                format!("{}", phase),
                Style::default().fg(phase_color).bold(),
            ),
            Span::raw(" "),
            Span::styled(*phase_name, Style::default().fg(phase_color)),
            Span::raw("  "),
            Span::styled(progress_dots, Style::default().fg(Color::DarkGray)),
        ]))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Progress")
                .title_style(Style::default().fg(Color::White)),
        )
    }
}

impl Default for PhaseView {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_phase_view_new() {
        let view = PhaseView::new();
        let _ = view;
    }

    #[test]
    fn test_phase_names() {
        assert_eq!(PHASE_NAMES[0], "Init");
        assert_eq!(PHASE_NAMES[12], "Complete");
        assert_eq!(PHASE_NAMES.len(), 13);
    }
}
