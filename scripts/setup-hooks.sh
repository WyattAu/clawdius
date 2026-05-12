#!/bin/bash
# Clawdius git hooks installer
#
# Installs pre-commit and pre-push hooks into .git/hooks/.
# Run this script once after cloning the repository.
#
# Usage: bash scripts/setup-hooks.sh

set -euo pipefail

HOOK_DIR="$(git rev-parse --git-dir)/hooks"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Source hook templates
PRECOMMIT_SRC="$SCRIPT_DIR/../.git/hooks/pre-commit"
PREPUSH_SRC="$SCRIPT_DIR/../.git/hooks/pre-push"

if [ ! -f "$PRECOMMIT_SRC" ]; then
    echo "ERROR: pre-commit hook template not found at $PRECOMMIT_SRC"
    exit 1
fi

cp "$PRECOMMIT_SRC" "$HOOK_DIR/pre-commit"
chmod +x "$HOOK_DIR/pre-commit"
echo "Installed: $HOOK_DIR/pre-commit"

if [ -f "$PREPUSH_SRC" ]; then
    cp "$PREPUSH_SRC" "$HOOK_DIR/pre-push"
    chmod +x "$HOOK_DIR/pre-push"
    echo "Installed: $HOOK_DIR/pre-push"
fi

echo "Git hooks installed successfully."
