#!/bin/bash
# Test JSON output format

set -e

echo "Testing JSON output format..."
echo "=============================="
echo

echo "1. Testing --output-format help"
cargo run --package clawdius -- --help 2>&1 | grep -A 2 "output-format" || echo "Help text check failed"
echo

echo "2. Testing JSON format for chat command (will fail without API key, but should show JSON error)"
timeout 5 cargo run --package clawdius -- chat "test message" --output-format json 2>&1 | head -20 || true
echo

echo "3. Testing stream-json format for chat command"
timeout 5 cargo run --package clawdius -- chat "test message" --output-format stream-json 2>&1 | head -20 || true
echo

echo "4. Testing text format (default)"
timeout 5 cargo run --package clawdius -- chat "test message" --output-format text 2>&1 | head -20 || true
echo

echo "5. Testing sessions with JSON format"
cargo run --package clawdius -- sessions --output-format json 2>&1 | head -20 || true
echo

echo "=============================="
echo "Test complete!"
