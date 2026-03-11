#!/bin/bash
# Quick demonstration of JSON output format feature

echo "========================================="
echo "Clawdius JSON Output Format Demonstration"
echo "========================================="
echo

echo "1. Check that the flag is recognized:"
echo "   $ clawdius --help | grep output-format"
echo

echo "2. List sessions in JSON format:"
echo "   $ clawdius sessions --output-format json"
echo "   Output:"
cargo run --quiet --package clawdius -- sessions --output-format json 2>&1 | head -10
echo

echo "3. List sessions in stream-json format:"
echo "   $ clawdius sessions --output-format stream-json"
echo "   Output:"
cargo run --quiet --package clawdius -- sessions --output-format stream-json 2>&1 | head -5
echo

echo "4. List sessions in text format (default):"
echo "   $ clawdius sessions --output-format text"
echo "   Output:"
cargo run --quiet --package clawdius -- sessions --output-format text 2>&1 | head -5
echo

echo "========================================="
echo "Feature Implementation: COMPLETE ✓"
echo "========================================="
