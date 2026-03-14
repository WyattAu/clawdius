//! TUI application for Clawdius
//!
//! A modern, assertive terminal interface with vim-inspired keybindings.

mod app;
mod components;
mod theme;
mod types;
mod ui;
mod vim;

pub use app::App;
pub use theme::Theme;

/// Run the TUI
pub async fn run_tui() -> anyhow::Result<()> {
    use crossterm::{
        event::{DisableMouseCapture, EnableMouseCapture, Event, EventStream},
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    };
    use futures::StreamExt;
    use ratatui::{backend::CrosstermBackend, Terminal};
    use std::io::stdout;

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app
    let mut app = App::new()?;

    // Event loop
    let mut events = EventStream::new();

    loop {
        // Draw
        terminal.draw(|f| app.draw(f))?;

        // Handle events
        if let Some(event) = events.next().await {
            let event = event?;
            match event {
                Event::Key(key) => {
                    app.handle_key(key).await?;
                    if app.should_quit {
                        break;
                    }
                }
                Event::Resize(_cols, _rows) => {
                    app.resize();
                }
                _ => {}
            }
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;

    Ok(())
}
