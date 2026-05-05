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
    use std::time::Duration;

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
    // A short tick interval so we can poll the LLM stream frequently
    let mut tick_interval = tokio::time::interval(Duration::from_millis(50));

    loop {
        // Draw
        terminal.draw(|f| app.draw(f))?;

        // Drain any available streaming chunks
        app.drain_stream();

        // Poll file watcher for change events
        app.poll_file_watcher();

        // Use select to handle either terminal events or tick timeouts
        tokio::select! {
            // Terminal events (keyboard, mouse, resize)
            maybe_event = events.next() => {
                if let Some(event) = maybe_event {
                    let event = event?;
                    match event {
                        Event::Key(key) => {
                            app.handle_key(key).await?;
                            if app.should_quit {
                                break;
                            }
                        },
                        Event::Resize(_cols, _rows) => {
                            app.resize();
                        },
                        Event::Mouse(mouse) => {
                            use crossterm::event::MouseEventKind;
                            match mouse.kind {
                                MouseEventKind::ScrollUp => {
                                    app.scroll_up();
                                },
                                MouseEventKind::ScrollDown => {
                                    app.scroll_down();
                                },
                                MouseEventKind::Down(_button) => {
                                    // Future: click-to-focus, context menus
                                },
                                _ => {},
                            }
                        },
                        _ => {},
                    }
                }
            },
            // Periodic tick — ensures we keep draining stream even without events
            _ = tick_interval.tick() => {
                // The drain_stream() call above handles this
            },
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
