//! Swarm View widget - Multi-agent DAG visualization

use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
};

#[derive(Debug, Clone)]
pub struct AgentStatus {
    pub name: String,
    pub status: AgentState,
    pub progress: f64,
    pub tasks_completed: u32,
    pub tasks_total: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentState {
    Idle,
    Running,
    Complete,
    Error,
}

impl AgentState {
    pub fn color(&self) -> Color {
        match self {
            Self::Idle => Color::DarkGray,
            Self::Running => Color::Yellow,
            Self::Complete => Color::Green,
            Self::Error => Color::Red,
        }
    }

    pub fn symbol(&self) -> &'static str {
        match self {
            Self::Idle => "○",
            Self::Running => "◐",
            Self::Complete => "●",
            Self::Error => "✗",
        }
    }
}

pub struct SwarmView {
    bar_width: usize,
}

impl SwarmView {
    pub fn new() -> Self {
        Self { bar_width: 12 }
    }

    pub fn render(&self, agents: &[AgentStatus], focused: bool) -> List<'static> {
        let border_style = if focused {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        };

        let items: Vec<ListItem<'_>> = agents
            .iter()
            .map(|agent| {
                let filled = (agent.progress * self.bar_width as f64).round() as usize;
                let filled = filled.min(self.bar_width);
                let bar: String = "█".repeat(filled) + &"░".repeat(self.bar_width - filled);

                let status_text = if agent.tasks_total > 0 {
                    format!(" {}/{}", agent.tasks_completed, agent.tasks_total)
                } else {
                    String::new()
                };

                ListItem::new(Line::from(vec![
                    Span::styled(
                        agent.status.symbol(),
                        Style::default().fg(agent.status.color()),
                    ),
                    Span::raw(" "),
                    Span::styled(
                        format!("{:12}", agent.name),
                        Style::default().fg(Color::White),
                    ),
                    Span::raw(" "),
                    Span::styled(bar, Style::default().fg(agent.status.color())),
                    Span::styled(status_text, Style::default().fg(Color::DarkGray)),
                ]))
            })
            .collect();

        List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Swarm")
                .title_style(Style::default().fg(Color::White))
                .border_style(border_style),
        )
    }

    pub fn set_bar_width(&mut self, width: usize) {
        self.bar_width = width;
    }
}

impl Default for SwarmView {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_state_colors() {
        assert_eq!(AgentState::Idle.color(), Color::DarkGray);
        assert_eq!(AgentState::Running.color(), Color::Yellow);
        assert_eq!(AgentState::Complete.color(), Color::Green);
        assert_eq!(AgentState::Error.color(), Color::Red);
    }

    #[test]
    fn test_agent_state_symbols() {
        assert_eq!(AgentState::Idle.symbol(), "○");
        assert_eq!(AgentState::Running.symbol(), "◐");
        assert_eq!(AgentState::Complete.symbol(), "●");
        assert_eq!(AgentState::Error.symbol(), "✗");
    }

    #[test]
    fn test_swarm_view_new() {
        let view = SwarmView::new();
        assert_eq!(view.bar_width, 12);
    }
}
