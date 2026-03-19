//! CLI Progress Indicators
//!
//! Simple text-based progress indicators for CLI operations.

//!
//! ## Usage
//!
//! ```rust
//! use crate::cli_progress::{ProgressBar, Spinner};
//!
//! // Create a spinner for long operations
//! let mut spinner = Spinner::new("Loading...");
//! spinner.start();
//!
//! // Do some work...
//! std::thread::sleep(std::time::Duration::from_secs(2));
//!
//! // Stop with success message
//! spinner.stop(Some("Done!"));
//!
//! // Create a progress bar for multi-step operations
//! let mut progress = ProgressBar::new(10, "Processing items");
//! for i in 0..10 {
//!     progress.inc();
//!     // Do work...
//! }
//! progress.finish("Complete!");
//! ```

//!
//! ## Output Format Support
//!
//! Both component supports JSON output via the `--format json` flag.
//! In JSON mode, the spinner will output JSON progress events.

//!
//! ## Thread Safety
//!
//! All components are designed to work in a single-threaded context.
//! For multi-threaded use, wrap them in Arc<Mutex<...>>.

//!
//! ## Example: Integration with LSP
//!
//! ```rust
//! use crate::cli_progress::Spinner;
//!
//! async fn connect_to_lsp(server: &str) -> Result<()> {
//!     let mut spinner = Spinner::new(format!("Connecting to {}...", server));
//!     spinner.start();
//!
//!     // Attempt connection
//!     let result = attempt_connection().await;
//!
//!     match result {
//!         Ok(_) => {
//!             spinner.stop(Some(&format!("Connected to {}", server)));
//!             Ok(())
//!         }
//!         Err(e) => {
//!             spinner.stop_with_error(&format!("Failed to connect: {}", e));
//!             Err(e)
//!         }
//!     }
//! }
//! ```

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
                let _ = write!(handle, "\r\x1B[K{} {}\x1B[0m", spinner_char, message);
                let _ = handle.flush();

                frame.store(current_frame + 1, Ordering::SeqCst);
                thread::sleep(Duration::from_millis(80));
            }

            // Clear the spinner line when done
            let _ = write!(handle, "\r\x1B[2K\r");
            let _ = handle.flush();
        }));
    }

    /// Update the spinner message.
    pub fn set_message(&mut self, message: impl Into<String>) {
        self.message = message.into();
    }

    /// Stop the spinner and optionally show a completion message.
    pub fn stop(mut self, completion_message: Option<&str>) {
        self.running.store(false, Ordering::SeqCst);

        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }

        if let Some(msg) = completion_message {
            println!("✅ {}", msg);
        }
    }

    /// Stop the spinner with an error message.
    pub fn stop_with_error(mut self, error_message: &str) {
        self.running.store(false, Ordering::SeqCst);

        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }

        println!("❌ {}", error_message);
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

/// Progress bar for tracking multi-step operations.
pub struct ProgressBar {
    current: usize,
    total: usize,
    message: String,
    width: usize,
}

impl ProgressBar {
    /// Create a new progress bar.
    pub fn new(total: usize, message: impl Into<String>) -> Self {
        Self {
            current: 0,
            total,
            message: message.into(),
            width: 40,
        }
    }

    /// Advance the progress by one step.
    pub fn inc(&mut self) {
        if self.current < self.total {
            self.current += 1;
        }
        self.render();
    }

    /// Set the current progress.
    pub fn set(&mut self, current: usize) {
        self.current = current.min(self.total);
        self.render();
    }

    /// Render the progress bar.
    fn render(&self) {
        let percent = if self.total > 0 {
            (self.current as f64 / self.total as f64 * 100.0) as usize
        } else {
            100
        };

        let filled = if self.total > 0 {
            (self.current * self.width) / self.total
        } else {
            self.width
        };

        let bar: String = "█".repeat(filled) + &"░".repeat(self.width - filled);

        print!(
            "\r\x1B[K{} [{}] {}% ({}/{})",
            self.message, bar, percent, self.current, self.total
        );
    }

    /// Complete the progress bar with a message.
    pub fn finish(mut self, message: &str) {
        self.current = self.total;
        let bar: String = "█".repeat(self.width);
        println!("\r\x1B[K{} [{}] 100% - {}", self.message, bar, message);
    }
}

/// Simple status message for one-time operations.
pub fn status(message: &str) {
    println!("⟳ {}", message);
}

/// Success message.
pub fn success(message: &str) {
    println!("✅ {}", message);
}

/// Error message with optional suggestion.
pub fn error(message: &str, suggestion: Option<&str>) {
    println!("❌ {}", message);
    if let Some(suggestion) = suggestion {
        println!("   💡 {}", suggestion);
    }
}

/// Warning message.
pub fn warning(message: &str) {
    println!("⚠️  {}", message);
}

/// Info message.
pub fn info(message: &str) {
    println!("ℹ️  {}", message);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spinner_creation() {
        let spinner = Spinner::new("Loading...");
        assert!(!spinner.running.load(Ordering::SeqCst));
    }

    #[test]
    fn test_progress_bar_creation() {
        let bar = ProgressBar::new(10, "Processing");
        assert_eq!(bar.current, 0);
        assert_eq!(bar.total, 10);
    }
}
