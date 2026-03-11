#!/bin/bash
# Generate benchmark fixture files
# These files are NOT committed to git

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FIXTURES_DIR="$SCRIPT_DIR"

echo "Generating benchmark fixtures in $FIXTURES_DIR..."

# Small file (1KB)
echo "Creating small.txt (1KB)..."
{
  for i in {1..20}; do
    echo "Line $i: This is a test line with some content to reach approximately 1KB total."
  done
} > "$FIXTURES_DIR/small.txt"

# Medium file (100KB)
echo "Creating medium.txt (100KB)..."
{
  for i in {1..2000}; do
    echo "Line $i: This is a test line with some content to reach approximately 100KB total size."
  done
} > "$FIXTURES_DIR/medium.txt"

# Large file (1MB)
echo "Creating large.txt (1MB)..."
{
  for i in {1..20000}; do
    echo "Line $i: This is a test line with some content to reach approximately 1MB total file size."
  done
} > "$FIXTURES_DIR/large.txt"

echo "✓ Generated benchmark fixtures:"
ls -lh "$FIXTURES_DIR"/*.txt
