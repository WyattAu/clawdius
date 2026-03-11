#!/bin/bash
# Quick test for JSON output format feature

echo "=== Testing JSON Output Format Feature ==="
echo

echo "1. Checking if --help shows the output-format flag..."
cargo run --quiet --package clawdius -- --help 2>&1 | grep -E "(-f, --output-format|output.format)" && echo "✓ Flag found" || echo "✗ Flag not found"
echo

echo "2. Testing that output-format values are recognized..."
echo "   Testing 'json' format..."
cargo run --quiet --package clawdius -- sessions --output-format json 2>&1 | head -5
echo

echo "   Testing 'text' format..."
cargo run --quiet --package clawdius -- sessions --output-format text 2>&1 | head -5
echo

echo "   Testing 'stream-json' format..."
cargo run --quiet --package clawdius -- sessions --output-format stream-json 2>&1 | head -5
echo

echo "=== Test Complete ==="
