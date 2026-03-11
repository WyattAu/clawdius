# Clawdius User Guide

**Version:** 0.7.0  
**Last Updated:** 2026-03-06

---

## Table of Contents

1. [Introduction](#1-introduction)
2. [Installation](#2-installation)
3. [Configuration](#3-configuration)
4. [API Key Setup](#4-api-key-setup)
5. [Commands](#5-commands)
6. [File Timeline](#6-file-timeline)
7. [JSON Output](#7-json-output)
8. [Enhanced Completions](#8-enhanced-completions)
9. [Workflows](#9-workflows)
10. [Troubleshooting](#10-troubleshooting)

---

## 1. Introduction

Clawdius is a high-assurance AI engineering engine that enforces rigorous development practices through the 24-phase Nexus R&D Lifecycle. It provides deterministic behavior, secure sandboxing, and comprehensive knowledge management.

### 1.1 Key Features

- **24-Phase Nexus Lifecycle:** Structured development process with quality gates
- **Graph-RAG Intelligence:** AST-based code understanding with vector search
- **Sentinel Sandboxing:** Multi-tier execution isolation
- **SOP Enforcement:** Automated standard operating procedure compliance
- **HFT-Ready:** Sub-millisecond latency for financial applications
- **Monorepo Architecture:** Single repository for all components

### 1.2 System Requirements

| Requirement | Minimum | Recommended |
|-------------|---------|-------------|
| OS | Linux, macOS, WSL2 | Linux (Ubuntu 24.04) |
| Rust | 1.85+ | 1.85+ |
| RAM | 64MB | 256MB |
| Disk | 50MB | 500MB |

### 1.3 Monorepo Components

Clawdius is distributed as a monorepo with multiple components:

| Component | Type | Description |
|-----------|------|-------------|
| **clawdius** | CLI Binary | Command-line interface |
| **clawdius-core** | Library | Core functionality |
| **clawdius-code** | Binary | VSCode extension helper |
| **clawdius-webview** | WASM | Browser-based UI |
| **VSCode Extension** | Extension | Editor integration |

---

## 2. Installation

### 2.1 Via Cargo (Recommended)

Install the CLI binary directly:

```bash
cargo install clawdius
```

This installs the `clawdius` binary to `~/.cargo/bin/`.

### 2.2 Via Nix

```bash
nix shell github:clawdius/clawdius
```

### 2.3 From Source (Monorepo)

Build from the monorepo to get all components:

```bash
# Clone the monorepo
git clone https://github.com/clawdius/clawdius
cd clawdius

# Build all crates
cargo build --release

# CLI binary location
./target/release/clawdius

# Optional: Install to system
cargo install --path crates/clawdius
```

### 2.4 VSCode Extension

Install the VSCode extension for editor integration:

**Option 1: From VSIX**
```bash
# Build extension
cd clawdius/editors/vscode
pnpm install
pnpm run compile
vsce package

# Install in VSCode
code --install-extension clawdius-0.7.0.vsix
```

**Option 2: Development Mode**
```bash
cd clawdius/editors/vscode
code .
# Press F5 to launch Extension Development Host
```

### 2.5 Verification

```bash
clawdius --version
# Output: clawdius 0.7.0

# Check all components
clawdius --version          # CLI
clawdius-code --version     # VSCode helper (if built)
```

---

## 3. Configuration

### 3.1 Initialization

Initialize Clawdius in your project:

```bash
clawdius init
```

This creates the `.clawdius/` directory structure:

```
.clawdius/
├── config.toml      # Main configuration file
├── sops/            # Standard Operating Procedures
│   ├── common.sop.md
│   └── rust.sop.md
├── specs/           # Specifications and papers
│   ├── 00_requirements/
│   ├── 01_research/
│   ├── 02_architecture/
│   └── ...
├── graph/           # Knowledge graph storage
│   ├── index.db     # SQLite AST database
│   └── vectors.lance # LanceDB vector store
├── sessions.db      # Session storage
└── commands/        # Custom commands
```

### 3.2 Configuration File

The `.clawdius/config.toml` file configures all aspects of Clawdius:

```toml
[project]
name = "my-project"
rigor_level = "high"           # low, medium, high
lifecycle_phase = "context_discovery"

[storage]
database_path = ".clawdius/graph/index.db"
vector_path = ".clawdius/graph/vectors.lance"
sessions_path = ".clawdius/sessions.db"

[llm]
default_provider = "anthropic"  # anthropic, openai, ollama, zai
max_tokens = 4096

[llm.anthropic]
model = "claude-3-5-sonnet-20241022"
# api_key_env = "ANTHROPIC_API_KEY"  # Optional: custom env var name

[llm.openai]
model = "gpt-4o"
# base_url = "https://api.openai.com/v1"  # Optional: custom endpoint

[llm.ollama]
model = "llama3.2"
base_url = "http://localhost:11434"

[llm.zai]
model = "zai-default"

[llm.retry]
max_retries = 3
initial_delay_ms = 1000
max_delay_ms = 30000
exponential_base = 2.0
retry_on = ["rate_limit", "timeout", "server_error", "network_error"]

[session]
compact_threshold = 0.85       # Auto-compact at 85% context
keep_recent = 4                # Keep last 4 messages when compacting
min_messages = 10              # Minimum messages before compacting
auto_save = true

[output]
format = "text"                # text, json, stream-json
show_progress = true

[shell_sandbox]
blocked_commands = [
    "rm -rf /",
    "mkfs",
    "dd if=/dev/zero",
    "wget",
    "curl -X POST"
]
timeout_secs = 120
max_output_bytes = 1048576     # 1MB
restrict_to_cwd = true
```

### 3.3 LLM Provider Selection

Clawdius supports multiple LLM providers:

| Provider | Models | API Key Required |
|----------|--------|------------------|
| **Anthropic** | claude-3-5-sonnet, claude-3-opus | Yes |
| **OpenAI** | gpt-4o, gpt-4-turbo, gpt-3.5-turbo | Yes |
| **Ollama** | llama3.2, mistral, codellama | No (local) |
| **ZAI** | zai-default | Yes |

Select a provider via CLI:

```bash
clawd chat --provider anthropic --model claude-3-5-sonnet-20241022
clawd chat --provider openai --model gpt-4o
clawd chat --provider ollama --model llama3.2
```

### 3.4 Retry Configuration

Configure automatic retry behavior for transient errors:

```toml
[llm.retry]
max_retries = 3              # Maximum retry attempts
initial_delay_ms = 1000      # Initial delay before retry
max_delay_ms = 30000         # Maximum delay cap
exponential_base = 2.0       # Backoff multiplier
retry_on = [
    "rate_limit",            # HTTP 429 errors
    "timeout",               # Request timeouts
    "server_error",          # HTTP 5xx errors
    "network_error"          # Connection failures
]
```

### 3.5 Shell Sandboxing

Clawdius protects your system by sandboxing shell commands:

```toml
[shell_sandbox]
# Commands matching these patterns are blocked
blocked_commands = [
    "rm -rf /",
    "mkfs",
    "dd if=/dev/zero",
    "dd if=/dev/urandom",
    ":(){ :|:& };:",
    "chmod -R 777 /",
    "wget",
    "curl -X POST"
]

# Maximum execution time
timeout_secs = 120

# Maximum output size (bytes)
max_output_bytes = 1048576

# Restrict commands to project directory
restrict_to_cwd = true
```

---

## 4. API Key Setup

### 4.1 Environment Variables (Recommended)

Set API keys via environment variables:

```bash
# Anthropic
export ANTHROPIC_API_KEY="sk-ant-..."

# OpenAI
export OPENAI_API_KEY="sk-..."

# ZAI
export ZAI_API_KEY="..."

# Ollama (optional, for remote server)
export OLLAMA_BASE_URL="http://localhost:11434"
```

Add to your shell profile for persistence:

```bash
# ~/.bashrc or ~/.zshrc
export ANTHROPIC_API_KEY="sk-ant-..."
export OPENAI_API_KEY="sk-..."
```

### 4.2 System Keyring Storage

Clawdius can securely store API keys in your system's keyring:

```bash
# Store an API key (prompts for key input)
clawd auth set-key anthropic
clawd auth set-key openai
clawd auth set-key zai

# Retrieve a stored key (shows first 8 characters)
clawd auth get-key anthropic

# Delete a stored key
clawd auth delete-key anthropic
```

**Key Priority Order:**
1. Environment variable (e.g., `ANTHROPIC_API_KEY`)
2. System keyring (via `clawd auth set-key`)
3. Config file `api_key` field (not recommended)

### 4.3 Config File (Not Recommended)

You can also specify keys in `config.toml` (less secure):

```toml
[llm.anthropic]
api_key = "sk-ant-..."  # WARNING: Visible in file, git history
```

---

## 5. Commands

### 5.1 `clawd init`

Initialize Clawdius in the current directory.

```bash
clawd init [PATH]
```

| Argument | Description | Default |
|----------|-------------|---------|
| `PATH` | Project directory | `.` |

**Example:**
```bash
clawd init                    # Initialize in current directory
clawd init ~/projects/my-app  # Initialize in specific directory
```

### 5.2 `clawd chat`

Send a message to the LLM or start an interactive session.

```bash
clawd chat MESSAGE [OPTIONS]
```

| Flag | Description | Default |
|------|-------------|---------|
| `--provider`, `-P` | LLM provider | `anthropic` |
| `--model`, `-m` | Model to use | Provider default |
| `--session`, `-s` | Continue from session ID | New session |

**Examples:**
```bash
# Simple message
clawd chat "Explain this code"

# Specify provider and model
clawd chat "Write tests" --provider openai --model gpt-4o

# Continue previous session
clawd chat "Continue" --session abc123

# Use local Ollama
clawd chat "Hello" --provider ollama --model llama3.2
```

### 5.3 `clawd sessions`

List and manage conversation sessions.

```bash
clawd sessions [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--delete`, `-d` | Delete a session by ID |
| `--search`, `-s` | Search session messages |

**Examples:**
```bash
# List all sessions
clawd sessions

# Delete a session
clawd sessions --delete abc123

# Search sessions
clawd sessions --search "error handling"
```

### 5.4 `clawd auth`

Manage API keys in system keyring.

```bash
clawd auth <COMMAND>
```

| Command | Description |
|---------|-------------|
| `set-key <provider>` | Store API key for provider |
| `get-key <provider>` | Retrieve stored API key |
| `delete-key <provider>` | Delete stored API key |

**Supported Providers:** `anthropic`, `openai`, `zai`

**Examples:**
```bash
# Store API key (prompts securely)
clawd auth set-key anthropic

# Verify key is stored
clawd auth get-key anthropic

# Remove key
clawd auth delete-key anthropic
```

### 5.5 `clawd refactor`

Plan and execute cross-language refactoring.

```bash
clawd refactor --from LANG --to LANG [PATH] [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--from`, `-f` | Source language |
| `--to`, `-t` | Target language |
| `--dry-run` | Preview without applying |

**Example:**
```bash
clawd refactor --from typescript --to rust src/ --dry-run
```

### 5.6 `clawd broker`

Activate financial monitoring and trading signals.

```bash
clawd broker [--config CONFIG] [--paper-trade]
```

| Flag | Description |
|------|-------------|
| `--config`, `-c` | Broker configuration file |
| `--paper-trade` | Simulate without executing trades |

**Example:**
```bash
clawd broker --config broker.toml --paper-trade
```

### 5.7 `clawd verify`

Run Lean 4 proofs and SOP compliance checks.

```bash
clawd verify --proof PATH [--lean-path PATH]
```

| Flag | Description |
|------|-------------|
| `--proof`, `-p` | Path to .lean proof file or directory |
| `--lean-path` | Path to lean binary |

**Example:**
```bash
clawd verify --proof proofs/
```

### 5.8 `clawd compliance`

Generate compliance matrix.

```bash
clawd compliance --standards STANDARDS [OPTIONS]
```

| Flag | Description | Default |
|------|-------------|---------|
| `--standards`, `-s` | Standards (comma-separated) | Required |
| `--path`, `-p` | Project root | `.` |
| `--format`, `-f` | Output format | `markdown` |
| `--output`, `-o` | Output file | stdout |

**Example:**
```bash
clawd compliance --standards iso26262,do178c --format markdown
```

### 5.9 `clawd research`

Multi-lingual research synthesis.

```bash
clawd research QUERY [OPTIONS]
```

| Flag | Description | Default |
|------|-------------|---------|
| `--languages`, `-l` | Languages (comma-separated) | All |
| `--tqa-level`, `-L` | Minimum TQA level (1-5) | `3` |
| `--max-results`, `-m` | Max results per language | `10` |

**Example:**
```bash
clawd research "distributed systems consensus" --languages en,zh,ru
```

---

## 6. File Timeline

The file timeline system provides complete change tracking and rollback capability.

### 6.1 Creating Checkpoints

Create a checkpoint to save the current state of your project:

```bash
clawd timeline create <name> [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--description`, `-d` | Description of the checkpoint |
| `--tag`, `-t` | Tag for categorization |

**Examples:**
```bash
# Simple checkpoint
clawd timeline create "before-refactor"

# With description and tag
clawd timeline create "v1.0-rc1" --description "Release candidate 1" --tag release
```

### 6.2 Listing Checkpoints

View all timeline checkpoints:

```bash
clawd timeline list [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--format`, `-f` | Output format (text, json) |
| `--limit`, `-l` | Limit number of results |

**Examples:**
```bash
# List all checkpoints
clawd timeline list

# List in JSON format
clawd timeline list --format json

# List last 10 checkpoints
clawd timeline list --limit 10
```

### 6.3 Rolling Back

Restore project to a previous checkpoint:

```bash
clawd timeline rollback <checkpoint-id> [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--dry-run` | Preview changes without applying |
| `--force`, `-f` | Force rollback without confirmation |

**Examples:**
```bash
# Rollback to checkpoint
clawd timeline rollback abc123

# Preview rollback
clawd timeline rollback abc123 --dry-run

# Force rollback
clawd timeline rollback abc123 --force
```

### 6.4 Viewing Diff

Compare changes between checkpoints:

```bash
clawd timeline diff <from-id> <to-id> [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--format`, `-f` | Diff format (unified, json) |
| `--stat` | Show diff statistics only |

**Examples:**
```bash
# View diff between checkpoints
clawd timeline diff abc123 def456

# Show statistics only
clawd timeline diff abc123 def456 --stat

# JSON output
clawd timeline diff abc123 def456 --format json
```

### 6.5 File History

View change history for a specific file:

```bash
clawd timeline history <file-path> [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--limit`, `-l` | Limit number of entries |
| `--format`, `-f` | Output format (text, json) |

**Examples:**
```bash
# View file history
clawd timeline history src/main.rs

# Limited history
clawd timeline history src/main.rs --limit 20

# JSON output
clawd timeline history src/main.rs --format json
```

---

## 7. JSON Output

All CLI commands support JSON output for programmatic consumption and integration.

### 7.1 Usage

Add `--format json` to any command:

```bash
clawd <command> --format json
```

### 7.2 Examples

**Init Command:**
```bash
clawd init . --format json
# Output:
# {
#   "status": "success",
#   "path": "/path/to/project/.clawdius",
#   "message": "Initialized Clawdius in /path/to/project"
# }
```

**Chat Command:**
```bash
clawd chat "Explain this code" --format json
# Output:
# {
#   "response": "This code implements...",
#   "provider": "anthropic",
#   "model": "claude-3-5-sonnet-20241022",
#   "tokens_used": 1234
# }
```

**Timeline List:**
```bash
clawd timeline list --format json
# Output:
# {
#   "checkpoints": [
#     {
#       "id": "abc123",
#       "name": "before-refactor",
#       "description": "Pre-refactor checkpoint",
#       "timestamp": "2026-03-06T10:30:00Z",
#       "files_changed": 15
#     }
#   ]
# }
```

**Metrics Command:**
```bash
clawd metrics --format json
# Output:
# {
#   "files_indexed": 234,
#   "ast_nodes": 12456,
#   "vectors": 8901,
#   "sessions": 12,
#   "timeline_checkpoints": 5
# }
```

### 7.3 JSON Output Structure

All JSON output follows this structure:

```json
{
  "status": "success" | "error",
  "data": { ... },
  "error": null | { "code": "...", "message": "..." },
  "metadata": {
    "version": "0.7.0",
    "timestamp": "2026-03-06T10:30:00Z"
  }
}
```

---

## 8. Enhanced Completions

The completion system now features improved performance and reliability.

### 8.1 Features

- **LRU Caching:** Faster responses through intelligent caching
- **Smart Fallbacks:** Language-specific fallback strategies
- **Timeout Handling:** Reliable completion generation
- **Multi-Language Support:** Rust, Python, JavaScript/TypeScript, Go

### 8.2 Usage

Completions are automatically invoked during chat sessions:

```bash
clawd chat "Complete this function: @file:src/main.rs"
```

### 8.3 Configuration

Configure completion behavior in `.clawdius/config.toml`:

```toml
[completions]
cache_size = 100          # LRU cache size
timeout_ms = 5000         # Completion timeout
fallback_enabled = true   # Enable language fallbacks

[completions.languages]
rust = { enabled = true, max_tokens = 2048 }
python = { enabled = true, max_tokens = 2048 }
javascript = { enabled = true, max_tokens = 2048 }
go = { enabled = true, max_tokens = 2048 }
```

### 8.4 Supported Languages

| Language | Status | Features |
|----------|--------|----------|
| Rust | ✅ Full | Full syntax support, type inference |
| Python | ✅ Full | Full syntax support, type hints |
| JavaScript | ✅ Full | ES6+, TypeScript |
| TypeScript | ✅ Full | Full type system support |
| Go | ✅ Full | Full syntax support |

---

## 9. Workflows

### 9.1 Starting a New Project

```bash
# 1. Initialize
clawd init

# 2. Set up API keys
export ANTHROPIC_API_KEY="sk-ant-..."
# Or use keyring:
clawd auth set-key anthropic

# 3. Edit configuration
vim .clawdius/config.toml

# 4. Create initial checkpoint
clawd timeline create "initial" --description "Initial project state"

# 5. Start a chat
clawd chat "Help me understand this codebase"

# 6. View sessions
clawd sessions
```

### 9.2 Code Refactoring with Timeline

```bash
# 1. Create checkpoint before refactoring
clawd timeline create "before-refactor" --description "Pre-refactor state"

# 2. Analyze current codebase
clawd refactor --from typescript --to rust src/ --dry-run

# 3. Review migration plan
cat .clawdius/specs/migration_plan.md

# 4. Execute migration
clawd refactor --from typescript --to rust src/

# 5. If something goes wrong, rollback
clawd timeline rollback <checkpoint-id>

# 6. Verify results
clawd verify --proof proofs/
```

### 9.3 Using Multiple Providers

```bash
# Quick question with OpenAI
clawd chat "Explain this regex" --provider openai

# Deep analysis with Claude
clawd chat "Review architecture" --provider anthropic --model claude-3-5-sonnet-20241022

# Local development with Ollama
clawd chat "Write tests" --provider ollama --model llama3.2

# JSON output for scripting
clawd chat "Analyze code" --format json | jq '.response'
```

### 9.4 HFT Mode Setup

```bash
# 1. Enable HFT in settings
vim .clawdius/config.toml
# Set [hft].enabled = true

# 2. Configure risk parameters
clawd broker --config hft_config.toml --paper-trade

# 3. Start broker mode
clawd broker --config hft_config.toml
```

### 9.5 Timeline-Based Development

```bash
# 1. Create feature branch checkpoint
clawd timeline create "feature-auth-start" --tag feature

# 2. Make changes and create intermediate checkpoints
clawd timeline create "auth-models-done" --tag milestone
clawd timeline create "auth-routes-done" --tag milestone

# 3. Review changes
clawd timeline diff "feature-auth-start" "auth-routes-done"

# 4. If needed, rollback to intermediate state
clawd timeline rollback "auth-models-done"
```

---

## 10. Troubleshooting

### 10.1 Common Issues

#### "Failed to initialize state machine"

**Cause:** Corrupted `.clawdius/` directory

**Solution:**
```bash
rm -rf .clawdius/
clawd init
```

#### "Sandbox creation failed"

**Cause:** Missing sandboxing tools

**Solution (Linux):**
```bash
sudo apt install bubblewrap
```

**Solution (macOS):**
```bash
# sandbox-exec is built-in
# Ensure SIP is enabled
csrutil status
```

#### "LLM provider error" / "API key not set"

**Cause:** Invalid or missing API key

**Solution:**
```bash
# Check environment variable
echo $ANTHROPIC_API_KEY

# Or set via keyring
clawd auth set-key anthropic

# Test connection
clawd chat "Hello" --provider anthropic
```

#### "Retry exhausted" errors

**Cause:** Transient network/server issues after multiple retries

**Solution:**
```toml
# Increase retry settings in config.toml
[llm.retry]
max_retries = 5
initial_delay_ms = 2000
max_delay_ms = 60000
```

#### "Blocked command pattern detected"

**Cause:** Shell command matches blocked pattern

**Solution:**
```bash
# Review blocked commands in config
# .clawdius/config.toml [shell_sandbox].blocked_commands

# For trusted operations, modify config (use caution)
```

#### "Binary not found"

**Cause:** clawdius not in PATH

**Solution:**
```bash
# Add Cargo bin to PATH
export PATH="$HOME/.cargo/bin:$PATH"

# Or install from source
cargo install --path crates/clawdius
```

#### "VSCode extension not working"

**Cause:** Helper binary not built or wrong path

**Solution:**
```bash
# Build helper binary
cargo build --release -p clawdius-code

# Check VSCode setting
# "clawdius.binaryPath": "./target/release/clawdius-code"
```

### 10.2 Debug Mode

Enable verbose logging:

```bash
RUST_LOG=clawdius=debug clawd chat "Hello"
```

### 10.3 Getting Help

- **Documentation:** `.docs/` directory
- **Issues:** https://github.com/clawdius/clawdius/issues
- **Discord:** https://discord.gg/clawdius

---

## 8. Monorepo Features

### 7.1 Building from Source

The monorepo allows building all components from a single repository:

```bash
# Clone
git clone https://github.com/clawdius/clawdius
cd clawdius

# Build all crates
cargo build --release

# Build specific crate
cargo build -p clawdius-core

# Build with features
cargo build --features hft-mode

# Run tests
cargo test --all
```

### 7.2 Development Workflow

```bash
# Check all crates
cargo check --all

# Run clippy on all crates
cargo clippy --all-targets --all-features

# Format all code
cargo fmt --all

# Run benchmarks
cargo bench --all
```

### 7.3 Cross-Crate Testing

The monorepo enables integration testing across crates:

```bash
# Run integration tests
cargo test --test '*'

# Test specific integration
cargo test --test cli_core_integration
```

### 7.4 Feature Flags

Features propagate across crates:

```bash
# Build CLI with HFT mode
cargo build -p clawdius --features hft-mode

# This automatically enables hft-mode in clawdius-core
```

---

## Appendix A: Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `RUST_LOG` | Log level | `info` |
| `ANTHROPIC_API_KEY` | Anthropic API key | - |
| `OPENAI_API_KEY` | OpenAI API key | - |
| `ZAI_API_KEY` | Z.AI API key | - |
| `OLLAMA_BASE_URL` | Ollama server URL | `http://localhost:11434` |

## Appendix B: Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error |
| 2 | Configuration error |
| 3 | Sandbox error |
| 4 | LLM error |
| 5 | Authentication error |
| 6 | Retry exhausted |

## Appendix C: VSCode Extension Commands

| Command | Description |
|---------|-------------|
| `Clawdius: Ask a question` | Open chat input |
| `Clawdius: Chat with selection` | Chat about selected code |
| `Clawdius: Add file to context` | Add file to conversation |
| `Clawdius: Add current file to context` | Add active editor file |
| `Clawdius: Create checkpoint` | Save current state |
| `Clawdius: Open chat view` | Open sidebar chat |
