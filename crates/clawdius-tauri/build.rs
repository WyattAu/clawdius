fn main() {
    // `tauri_build::build()` requires the `tauri` crate in the dependency
    // graph (it consumes the `cargo:dev` instruction tauri emits), which is
    // only pulled in by the `desktop` feature. Build scripts cannot use
    // `cfg(feature)`, so gate on the CARGO_FEATURE_* env instead.
    if std::env::var_os("CARGO_FEATURE_DESKTOP").is_some() {
        tauri_build::build();
    }
}
