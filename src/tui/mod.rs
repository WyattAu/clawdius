//! Clawdius-Pit TUI - Terminal User Interface
//!
//! Provides 60fps rendering with real-time rigor score visualization
//! and multi-agent swarm view using ratatui.

mod app;
mod render;
pub mod widgets;

pub use app::App;
pub use render::Renderer;

use crate::component::{Component, ComponentId, ComponentState};
use crate::error::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

const FRAME_TIME_60FPS: Duration = Duration::from_micros(16_666);

pub struct Tui {
    app: App,
    renderer: Renderer,
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    running: Arc<AtomicBool>,
    state: ComponentState,
}

impl std::fmt::Debug for Tui {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tui")
            .field("app", &self.app)
            .field("running", &self.running.load(Ordering::Relaxed))
            .field("state", &self.state)
            .finish()
    }
}

impl Tui {
    pub fn new() -> Result<Self> {
        enable_raw_mode()?;

        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        let app = App::new();
        let renderer = Renderer::new();

        Ok(Self {
            app,
            renderer,
            terminal,
            running: Arc::new(AtomicBool::new(false)),
            state: ComponentState::Uninitialized,
        })
    }

    pub fn run(&mut self) -> Result<()> {
        self.running.store(true, Ordering::Relaxed);
        self.state = ComponentState::Running;

        while self.running.load(Ordering::Relaxed) {
            let frame_start = Instant::now();

            self.handle_events()?;

            self.app.update()?;

            self.terminal.draw(|f| {
                self.renderer.render(f, &self.app);
            })?;

            let elapsed = frame_start.elapsed();
            if elapsed < FRAME_TIME_60FPS {
                std::thread::sleep(FRAME_TIME_60FPS - elapsed);
            }
        }

        self.state = ComponentState::Stopped;
        Ok(())
    }

    fn handle_events(&mut self) -> Result<()> {
        let timeout = Duration::from_millis(1);
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                match (key.modifiers, key.code) {
                    (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
                        self.running.store(false, Ordering::Relaxed);
                    }
                    (KeyModifiers::NONE, KeyCode::Char('q')) => {
                        self.running.store(false, Ordering::Relaxed);
                    }
                    (KeyModifiers::NONE, KeyCode::Tab) => {
                        self.app.cycle_focus();
                    }
                    (KeyModifiers::NONE, KeyCode::Char('h')) => {
                        self.app.show_help();
                    }
                    (KeyModifiers::NONE, KeyCode::Esc) => {
                        self.app.dismiss_help();
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }

    pub fn cleanup(&mut self) -> Result<()> {
        disable_raw_mode()?;

        execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;

        self.terminal.show_cursor()?;

        Ok(())
    }

    pub fn update_rigor_score(&mut self, score: f64) {
        self.app.set_rigor_score(score);
    }

    pub fn update_phase(&mut self, phase: u8) {
        self.app.set_phase(phase);
    }

    pub fn update_agents(&mut self, agents: Vec<widgets::swarm_view::AgentStatus>) {
        self.app.set_agents(agents);
    }

    pub fn update_code(&mut self, code: &str, language: &str) {
        self.app.set_code(code, language);
    }

    pub fn update_command_output(&mut self, output: &str) {
        self.app.set_command_output(output);
    }
}

impl Drop for Tui {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

impl Component for Tui {
    fn id(&self) -> ComponentId {
        ComponentId(0x0007)
    }

    fn name(&self) -> &'static str {
        "Tui"
    }

    fn state(&self) -> ComponentState {
        self.state
    }

    fn initialize(&mut self) -> Result<()> {
        self.state = ComponentState::Initialized;
        Ok(())
    }

    fn start(&mut self) -> Result<()> {
        self.state = ComponentState::Running;
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        self.running.store(false, Ordering::Relaxed);
        self.state = ComponentState::Stopped;
        self.cleanup()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::float_cmp, clippy::unnecessary_unwrap)]
mod tests {
    use super::*;

    #[test]
    fn test_tui_creation_requires_terminal() {
        let tui = Tui::new();
        if tui.is_ok() {
            let tui = tui.expect("Tui creation failed");
            assert_eq!(tui.name(), "Tui");
            assert_eq!(tui.state(), ComponentState::Uninitialized);
        }
    }

    #[test]
    fn test_app_default() {
        let app = App::new();
        assert_eq!(app.phase(), 0);
        assert_eq!(app.rigor_score(), 0.0);
    }
}
