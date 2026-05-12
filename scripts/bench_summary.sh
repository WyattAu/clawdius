#!/bin/bash
echo "## Benchmark Results"
echo ""
echo "Run: cargo bench --workspace"
echo ""
cargo bench --workspace -- --list 2>&1 | head -50
