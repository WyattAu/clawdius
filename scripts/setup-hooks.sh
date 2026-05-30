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

# Prefer core.hooksPath if set, otherwise fall back to .githooks/
CUSTOM_HOOKS_PATH="$(git config --get core.hooksPath 2>/dev/null || true)"
if [ -n "$CUSTOM_HOOKS_PATH" ]; then
    echo "core.hooksPath is set to: $CUSTOM_HOOKS_PATH"
    echo "Hooks are already configured. Making them executable..."
    chmod +x "$CUSTOM_HOOKS_PATH/pre-commit" 2>/dev/null || true
    chmod +x "$CUSTOM_HOOKS_PATH/pre-push" 2>/dev/null || true
    echo "Git hooks ready."
    exit 0
fi

# Source hook templates from .githooks/
PRECOMMIT_SRC="$SCRIPT_DIR/../.githooks/pre-commit"
PREPUSH_SRC="$SCRIPT_DIR/../.githooks/pre-push"

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
echo ""
echo "To bypass hooks in emergencies: CLAWDIUS_SKIP_HOOKS=1 git commit/push"
