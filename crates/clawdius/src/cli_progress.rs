//! CLI Progress Indicators
//!
//! Simple text-based progress indicators for CLI operations.
//!
//! ## Thread Safety
//!
//! All components are designed to work in a single-threaded context.
//! For multi-threaded use, wrap them in Arc<Mutex<...>>.

use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// Spinner frames (Unicode Braille patterns)
const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Simple CLI spinner for long-running operations.
///
/// # Thread Safety
///
/// The spinner runs in a separate thread and communicates via atomic flags.
/// It's thread-safe when used behind an `Arc<Mutex<>>`.
pub struct Spinner {
    message: String,
    running: Arc<AtomicBool>,
    frame: Arc<AtomicUsize>,
    handle: Option<JoinHandle<()>>,
}

impl Spinner {
    /// Create a new spinner with a message.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            running: Arc::new(AtomicBool::new(false)),
            frame: Arc::new(AtomicUsize::new(0)),
            handle: None,
        }
    }

    /// Start the spinner animation.
    pub fn start(&mut self) {
        if self.running.load(Ordering::SeqCst) {
            return;
        }

        self.running.store(true, Ordering::SeqCst);
        let running = Arc::clone(&self.running);
        let frame = Arc::clone(&self.frame);
        let message = self.message.clone();

        self.handle = Some(thread::spawn(move || {
            let stdout = io::stdout();
            let mut handle = stdout.lock();

            while running.load(Ordering::SeqCst) {
                let current_frame = frame.load(Ordering::SeqCst);
                let spinner_char = SPINNER_FRAMES[current_frame % SPINNER_FRAMES.len()];

                // Clear line and print spinner with message
                let _ = write!(handle, "\r\x1B[K{spinner_char} {message}\x1B[0m");
                let _ = handle.flush();

                frame.store(current_frame + 1, Ordering::SeqCst);
                thread::sleep(Duration::from_millis(80));
            }

            // Clear the spinner line when done
            let _ = write!(handle, "\r\x1B[2K\r");
            let _ = handle.flush();
        }));
    }

    /// Stop the spinner and optionally show a completion message.
    pub fn stop(mut self, completion_message: Option<&str>) {
        self.running.store(false, Ordering::SeqCst);

        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }

        if let Some(msg) = completion_message {
            println!("✅ {msg}");
        }
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// Simple status message for one-time operations.
pub fn status(message: &str) {
    println!("⟳ {message}");
}

/// Success message.
pub fn success(message: &str) {
    println!("✅ {message}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spinner_creation() {
        let spinner = Spinner::new("Loading...");
        assert!(!spinner.running.load(Ordering::SeqCst));
    }
}
