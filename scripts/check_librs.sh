#!/bin/bash
set -euo pipefail

# lib.rs integrity check for CI
# This file is the single source of truth for the lib.rs integrity check.
# The same check also runs unconditionally in pre-commit and pre-push hooks.

LIBRS="crates/clawdius-core/src/lib.rs"

if [ ! -f "$LIBRS" ]; then
  echo "FAIL: $LIBRS does not exist"
  exit 1
fi

LINE_COUNT=$(wc -l < "$LIBRS")
MODS=$(grep -c "^pub mod " "$LIBRS" || true)

if [ "$LINE_COUNT" -lt 100 ]; then
  echo "FAIL: lib.rs has only $LINE_COUNT lines (expected >=100)"
  echo "lib.rs may have been corrupted by a git stash/merge operation."
  exit 1
fi

if [ "$MODS" -lt 10 ]; then
  echo "FAIL: lib.rs has only $MODS pub mod declarations (expected >=10)"
  echo "lib.rs may have been accidentally overwritten -- check for cargo init stub"
  exit 1
fi

echo "OK: lib.rs has $LINE_COUNT lines, $MODS pub mod declarations"
