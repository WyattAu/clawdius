#!/bin/bash

# Test script for timeline feature

echo "=== Building timeline module ==="
cargo build --package clawdius-core --lib 2>&1 | grep -E "(Compiling|error|warning:.*timeline)" | head -20

echo ""
echo "=== Testing timeline commands ==="

# Initialize a test project
TEST_DIR=$(mktemp -d)
cd "$TEST_DIR"

echo "Test directory: $TEST_DIR"

# Create some test files
echo "Creating test files..."
mkdir -p src
echo "fn main() {}" > src/main.rs
echo "root = true" > .editorconfig

# Initialize clawdius
echo ""
echo "Initializing clawdius..."
cargo run --package clawdius -- init . 2>&1 | head -10

# Create first checkpoint
echo ""
echo "Creating first checkpoint..."
cargo run --package clawdius -- timeline create "initial" --description "Initial state" 2>&1 | head -10

# Modify files
echo ""
echo "Modifying files..."
echo "fn main() { println!(\"Hello\"); }" > src/main.rs
echo "test" > test.txt

# Create second checkpoint
echo ""
echo "Creating second checkpoint..."
cargo run --package clawdius -- timeline create "after-modification" 2>&1 | head -10

# List checkpoints
echo ""
echo "Listing checkpoints..."
cargo run --package clawdius -- timeline list 2>&1 | head -20

# Cleanup
cd /
rm -rf "$TEST_DIR"

echo ""
echo "=== Test complete ==="
