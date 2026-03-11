use clawdius_core::{diff::DiffLine, FileDiff};
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
    Frame,
};

#[derive(Clone)]
pub struct DiffView {
    diff: Option<FileDiff>,
    scroll: usize,
    scroll_state: ScrollbarState,
}

impl DiffView {
    pub fn new() -> Self {
        Self {
            diff: None,
            scroll: 0,
            scroll_state: ScrollbarState::default(),
        }
    }

    pub fn set_diff(&mut self, diff: FileDiff) {
        self.diff = Some(diff);
        self.scroll = 0;
    }

    pub fn clear(&mut self) {
        self.diff = None;
        self.scroll = 0;
    }

    pub fn has_diff(&self) -> bool {
        self.diff.is_some()
    }

    pub fn scroll_up(&mut self) {
        if self.scroll > 0 {
            self.scroll -= 1;
        }
    }

    pub fn scroll_down(&mut self, max_visible: usize) {
        if let Some(diff) = &self.diff {
            let total = self.calculate_total_lines(diff);
            if self.scroll + max_visible < total {
                self.scroll += 1;
            }
        }
    }

    fn calculate_total_lines(&self, diff: &FileDiff) -> usize {
        diff.hunks.iter().map(|h| h.lines.len() + 1).sum()
    }

    pub fn render(&mut self, f: &mut Frame<'_>, area: Rect) {
        if let Some(diff) = &self.diff {
            let mut lines: Vec<Line<'_>> = Vec::new();

            let path_str = diff.path.to_string_lossy();
            lines.push(Line::from(vec![
                Span::styled("--- ", Style::default().fg(Color::Red)),
                Span::styled(path_str.as_ref(), Style::default().fg(Color::Yellow)),
            ]));
            lines.push(Line::from(vec![
                Span::styled("+++ ", Style::default().fg(Color::Green)),
                Span::styled(path_str.as_ref(), Style::default().fg(Color::Yellow)),
            ]));

            for hunk in &diff.hunks {
                lines.push(Line::from(vec![Span::styled(
                    format!(
                        "@@ -{},{} +{},{} @@",
                        hunk.old_start, hunk.old_lines, hunk.new_start, hunk.new_lines
                    ),
                    Style::default().fg(Color::Cyan),
                )]));

                for line in &hunk.lines {
                    let line_span = match line {
                        DiffLine::Context(l) => {
                            Span::styled(format!(" {}", l), Style::default().fg(Color::Gray))
                        }
                        DiffLine::Added(l) => {
                            Span::styled(format!("+{}", l), Style::default().fg(Color::Green))
                        }
                        DiffLine::Removed(l) => {
                            Span::styled(format!("-{}", l), Style::default().fg(Color::Red))
                        }
                    };
                    lines.push(Line::from(line_span));
                }
            }

            let total_lines = lines.len();
            let visible_lines = area.height.saturating_sub(2) as usize;

            let scroll_offset = self.scroll.min(total_lines.saturating_sub(visible_lines));

            let title = format!(
                "Diff: {} (+{} -{})",
                path_str,
                diff.stats().additions,
                diff.stats().deletions
            );

            let paragraph = Paragraph::new(lines)
                .block(Block::default().borders(Borders::ALL).title(title))
                .scroll((scroll_offset as u16, 0));

            f.render_widget(paragraph, area);

            if total_lines > visible_lines {
                self.scroll_state = self
                    .scroll_state
                    .content_length(total_lines)
                    .viewport_content_length(visible_lines)
                    .position(scroll_offset);

                let scrollbar =
                    Scrollbar::default().orientation(ScrollbarOrientation::VerticalRight);

                let scrollbar_area = Rect {
                    x: area.right().saturating_sub(1),
                    y: area.top() + 1,
                    width: 1,
                    height: area.height.saturating_sub(2),
                };

                f.render_stateful_widget(scrollbar, scrollbar_area, &mut self.scroll_state);
            }
        } else {
            let paragraph = Paragraph::new("No diff to display")
                .block(Block::default().borders(Borders::ALL).title("Diff"));
            f.render_widget(paragraph, area);
        }
    }
}

impl Default for DiffView {
    fn default() -> Self {
        Self::new()
    }
}
