//! Application state for the TUI

use super::widgets::swarm_view::AgentStatus;
use crate::error::Result;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Swarm,
    Code,
    Command,
}

#[derive(Debug, Clone)]
pub enum AppEvent {
    Quit,
    FocusNext,
    ShowHelp,
    DismissHelp,
    PhaseChange(u8),
    RigorUpdate(f64),
}

pub struct App {
    running: bool,
    focus: Focus,
    show_help: bool,
    phase: u8,
    rigor_score: f64,
    agents: Vec<AgentStatus>,
    code: String,
    code_language: String,
    command_output: String,
    command_input: String,
    frame_count: u64,
    last_frame_time: Instant,
    fps: f64,
}

impl std::fmt::Debug for App {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("App")
            .field("running", &self.running)
            .field("focus", &self.focus)
            .field("phase", &self.phase)
            .field("rigor_score", &self.rigor_score)
            .field("agents", &self.agents.len())
            .finish()
    }
}

impl App {
    pub fn new() -> Self {
        Self {
            running: true,
            focus: Focus::Swarm,
            show_help: false,
            phase: 0,
            rigor_score: 0.0,
            agents: Vec::new(),
            code: String::new(),
            code_language: "rust".to_string(),
            command_output: "Ready for commands. Type 'help' for available commands.".to_string(),
            command_input: String::new(),
            frame_count: 0,
            last_frame_time: Instant::now(),
            fps: 0.0,
        }
    }

    pub fn update(&mut self) -> Result<()> {
        self.frame_count += 1;

        let now = Instant::now();
        let elapsed = now.duration_since(self.last_frame_time);
        if elapsed.as_millis() >= 1000 {
            self.fps = self.frame_count as f64 / elapsed.as_secs_f64();
            self.frame_count = 0;
            self.last_frame_time = now;
        }

        Ok(())
    }

    pub fn running(&self) -> bool {
        self.running
    }

    pub fn quit(&mut self) {
        self.running = false;
    }

    pub fn focus(&self) -> Focus {
        self.focus
    }

    pub fn cycle_focus(&mut self) {
        self.focus = match self.focus {
            Focus::Swarm => Focus::Code,
            Focus::Code => Focus::Command,
            Focus::Command => Focus::Swarm,
        };
    }

    pub fn show_help(&mut self) {
        self.show_help = true;
    }

    pub fn dismiss_help(&mut self) {
        self.show_help = false;
    }

    pub fn is_help_visible(&self) -> bool {
        self.show_help
    }

    pub fn phase(&self) -> u8 {
        self.phase
    }

    pub fn set_phase(&mut self, phase: u8) {
        self.phase = phase;
    }

    pub fn rigor_score(&self) -> f64 {
        self.rigor_score
    }

    pub fn set_rigor_score(&mut self, score: f64) {
        self.rigor_score = score.clamp(0.0, 1.0);
    }

    pub fn agents(&self) -> &[AgentStatus] {
        &self.agents
    }

    pub fn set_agents(&mut self, agents: Vec<AgentStatus>) {
        self.agents = agents;
    }

    pub fn code(&self) -> &str {
        &self.code
    }

    pub fn code_language(&self) -> &str {
        &self.code_language
    }

    pub fn set_code(&mut self, code: &str, language: &str) {
        self.code = code.to_string();
        self.code_language = language.to_string();
    }

    pub fn command_output(&self) -> &str {
        &self.command_output
    }

    pub fn set_command_output(&mut self, output: &str) {
        self.command_output = output.to_string();
    }

    pub fn command_input(&self) -> &str {
        &self.command_input
    }

    pub fn fps(&self) -> f64 {
        self.fps
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_new() {
        let app = App::new();
        assert!(app.running());
        assert_eq!(app.phase(), 0);
        assert_eq!(app.rigor_score(), 0.0);
    }

    #[test]
    fn test_cycle_focus() {
        let mut app = App::new();
        assert_eq!(app.focus(), Focus::Swarm);

        app.cycle_focus();
        assert_eq!(app.focus(), Focus::Code);

        app.cycle_focus();
        assert_eq!(app.focus(), Focus::Command);

        app.cycle_focus();
        assert_eq!(app.focus(), Focus::Swarm);
    }

    #[test]
    fn test_rigor_score_clamping() {
        let mut app = App::new();

        app.set_rigor_score(1.5);
        assert_eq!(app.rigor_score(), 1.0);

        app.set_rigor_score(-0.5);
        assert_eq!(app.rigor_score(), 0.0);

        app.set_rigor_score(0.75);
        assert_eq!(app.rigor_score(), 0.75);
    }

    #[test]
    fn test_help_toggle() {
        let mut app = App::new();
        assert!(!app.is_help_visible());

        app.show_help();
        assert!(app.is_help_visible());

        app.dismiss_help();
        assert!(!app.is_help_visible());
    }
}
