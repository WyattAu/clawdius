#!/bin/bash

set -e

echo "Building Clawdius WebView for WASM..."

# Check if wasm target is installed
if ! rustup target list | grep -q "wasm32-unknown-unknown (installed)"; then
    echo "Installing wasm32-unknown-unknown target..."
    rustup target add wasm32-unknown-unknown
fi

# Build the project
echo "Compiling to WASM..."
cargo build --target wasm32-unknown-unknown --release

# Generate bindings
echo "Generating WASM bindings..."
wasm-bindgen target/wasm32-unknown-unknown/release/clawdius_webview.wasm \
    --out-dir ../../dist/webview \
    --target web \
    --no-typescript

echo "Build complete! Output in dist/webview/"
