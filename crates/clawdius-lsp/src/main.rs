//! Main entry point for the clawdius-lsp binary.

use clawdius_lsp::run_stdio;

#[tokio::main]
async fn main() {
    if let Err(e) = run_stdio().await {
        eprintln!("clawdius-lsp error: {e}");
        std::process::exit(1);
    }
}
