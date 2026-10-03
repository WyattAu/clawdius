//! Build script for the clawdius-tauri desktop shell.
//!
//! Runs `tauri_build::build()` only when the `desktop` feature is enabled;
//! the `main` body explains why the gate reads `CARGO_FEATURE_DESKTOP`.

fn main() {
    // `tauri_build::build()` requires the `tauri` crate in the dependency
    // graph (it consumes the `cargo:dev` instruction tauri emits), which is
    // only pulled in by the `desktop` feature. Build scripts cannot use
    // `cfg(feature)`, so gate on the CARGO_FEATURE_* env instead.
    if std::env::var_os("CARGO_FEATURE_DESKTOP").is_some() {
        tauri_build::build();
    }
}
