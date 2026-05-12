#!/bin/bash
set -euo pipefail

LIBRS="crates/clawdius-core/src/lib.rs"

if [ ! -f "$LIBRS" ]; then
  echo "FAIL: $LIBRS does not exist"
  exit 1
fi

MODS=$(grep -c "^pub mod " "$LIBRS")
if [ "$MODS" -lt 10 ]; then
  echo "FAIL: lib.rs has only $MODS pub mod declarations (expected >= 10)"
  echo "lib.rs may have been accidentally overwritten — check for cargo init stub"
  exit 1
fi

echo "OK: lib.rs has $MODS pub mod declarations"
