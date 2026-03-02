//! Rendering logic for the TUI

use super::app::{App, Focus};
use super::widgets::{
    code_view::CodeView, phase_view::PhaseView, rigor_score::RigorScore, swarm_view::SwarmView,
};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

const VERSION: &str = env!("CARGO_PKG_VERSION");

pub struct Renderer {
    rigor_widget: RigorScore,
    swarm_widget: SwarmView,
    code_widget: CodeView,
    phase_widget: PhaseView,
}

impl Renderer {
    pub fn new() -> Self {
        Self {
            rigor_widget: RigorScore::new(),
            swarm_widget: SwarmView::new(),
            code_widget: CodeView::new(),
            phase_widget: PhaseView::new(),
        }
    }

    pub fn render(&mut self, frame: &mut Frame<'_>, app: &App) {
        let size = frame.area();

        if app.is_help_visible() {
            self.render_help_overlay(frame, size);
            return;
        }

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(10),
                Constraint::Length(3),
            ])
            .split(size);

        self.render_header(frame, app, chunks[0]);
        self.render_main_area(frame, app, chunks[1]);
        self.render_command_bar(frame, app, chunks[2]);
    }

    fn render_header(&mut self, frame: &mut Frame<'_>, app: &App, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(25),
                Constraint::Length(20),
                Constraint::Min(20),
            ])
            .split(area);

        let title = Paragraph::new(Line::from(vec![
            Span::styled("Clawdius", Style::default().fg(Color::Cyan).bold()),
            Span::raw(" v"),
            Span::raw(VERSION),
        ]))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .style(Style::default()),
        );

        frame.render_widget(title, chunks[0]);

        let phase_text = self.phase_widget.render(app.phase());
        frame.render_widget(phase_text, chunks[1]);

        let rigor_text = self.rigor_widget.render(app.rigor_score(), app.fps());
        frame.render_widget(rigor_text, chunks[2]);
    }

    fn render_main_area(&mut self, frame: &mut Frame<'_>, app: &App, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(area);

        let swarm_focus = app.focus() == Focus::Swarm;
        let swarm = self.swarm_widget.render(app.agents(), swarm_focus);
        frame.render_widget(swarm, chunks[0]);

        let code_focus = app.focus() == Focus::Code;
        let code = self
            .code_widget
            .render(app.code(), app.code_language(), code_focus);
        frame.render_widget(code, chunks[1]);
    }

    fn render_command_bar(&self, frame: &mut Frame<'_>, app: &App, area: Rect) {
        let style = if app.focus() == Focus::Command {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        };

        let command = Paragraph::new(Line::from(vec![
            Span::styled("> ", Style::default().fg(Color::Green)),
            Span::styled(app.command_output().to_string(), style),
        ]))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Command")
                .title_style(Style::default().fg(Color::White)),
        );

        frame.render_widget(command, area);
    }

    fn render_help_overlay(&self, frame: &mut Frame<'_>, area: Rect) {
        let help_area = Rect::new(
            area.x + area.width.saturating_sub(60) / 2,
            area.y + area.height.saturating_sub(14) / 2,
            60.min(area.width),
            14.min(area.height),
        );

        frame.render_widget(Clear, help_area);

        let help_text = vec![
            Line::from(Span::styled(
                "Keyboard Shortcuts",
                Style::default().fg(Color::Cyan).bold(),
            )),
            Line::raw(""),
            Line::from(vec![
                Span::styled("  Tab     ", Style::default().fg(Color::Yellow)),
                Span::raw("Cycle focus between panels"),
            ]),
            Line::from(vec![
                Span::styled("  h       ", Style::default().fg(Color::Yellow)),
                Span::raw("Show this help"),
            ]),
            Line::from(vec![
                Span::styled("  q/Esc   ", Style::default().fg(Color::Yellow)),
                Span::raw("Quit / Dismiss help"),
            ]),
            Line::from(vec![
                Span::styled("  Ctrl+C  ", Style::default().fg(Color::Yellow)),
                Span::raw("Force quit"),
            ]),
            Line::raw(""),
            Line::styled(
                "  Press any key to close",
                Style::default().fg(Color::DarkGray),
            ),
        ];

        let help_widget = Paragraph::new(help_text).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Help")
                .title_style(Style::default().fg(Color::White))
                .style(Style::default().bg(Color::Black)),
        );

        frame.render_widget(help_widget, help_area);
    }
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_renderer_new() {
        let renderer = Renderer::new();
        let _ = &renderer;
    }
}
