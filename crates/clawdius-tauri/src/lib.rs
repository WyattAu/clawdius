//! Clawdius Tauri -- Desktop Application
//!
//! Tauri v2 desktop application using the clawdius-ui shared component library.
//! The `desktop` feature flag gates the actual Tauri runtime; without it the
//! crate compiles but prints a message at launch.

pub mod commands;
pub mod frontend;

/// Entry point that launches the Tauri runtime when the `desktop` feature is
/// enabled, or prints a diagnostic message otherwise.
pub fn run() {
    #[cfg(feature = "desktop")]
    {
        crate::commands::run_tauri_app();
    }
    #[cfg(not(feature = "desktop"))]
    {
        eprintln!("clawdius-tauri: desktop feature not enabled, skipping tauri launch");
    }
}
