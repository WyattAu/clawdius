# Clawdius User Guide

**Version:** 2.0.0  
**Last Updated:** 2026-04-07

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
- **Multi-Agent Modes:** Built-in and custom agent modes (code, architect, ask, debug, review, refactor, test, auto)
- **Autonomous CI/CD:** Run tasks without interaction with auto-commit and test execution
- **Agentic Code Generation:** Single-pass, iterative, and agent generation modes
- **Sentinel Sandboxing:** Multi-tier execution isolation
- **SOP Enforcement:** Automated standard operating procedure compliance
- **HFT-Ready:** Sub-millisecond latency for financial applications
- **Monorepo Architecture:** Single repository for all components
- **Project Memory:** CLAUDE.md-based persistent project knowledge
- **Git Integration:** AI-generated commit messages, diffs, and status
- **Webhook Support:** Event notifications via webhooks
- **Multi-Language Output:** i18n support for 10 languages

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
code --install-extension clawdius-2.0.0.vsix
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
# Output: clawdius 2.0.0

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

Or use the interactive setup wizard:

```bash
clawdius setup
clawdius setup --provider anthropic  # Pre-select provider
clawdius setup --quick               # Skip welcome screen
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
default_provider = "anthropic"  # anthropic, openai, ollama, zai, deepseek, openrouter
max_tokens = 4096

[llm.anthropic]
model = "claude-sonnet-4-20250514"

[llm.openai]
model = "gpt-4o"

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
| **Anthropic** | claude-sonnet-4, claude-3-opus | Yes |
| **OpenAI** | gpt-4o, gpt-4-turbo, gpt-3.5-turbo | Yes |
| **Ollama** | llama3.2, mistral, codellama | No (local) |
| **DeepSeek** | deepseek-coder | Yes |
| **ZAI** | zai-default | Yes |
| **OpenRouter** | Various (multi-provider routing) | Yes |

Select a provider via CLI:

```bash
clawdius chat --provider anthropic --model claude-sonnet-4-20250514
clawdius chat --provider openai --model gpt-4o
clawdius chat --provider ollama --model llama3.2
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

Requires the `keyring` feature. Clawdius can securely store API keys in your system's keyring:

```bash
# Store an API key (prompts for key input)
clawdius auth set anthropic
clawdius auth set openai
clawdius auth set zai

# Retrieve a stored key (shows first 8 characters)
clawdius auth get anthropic

# Delete a stored key
clawdius auth delete anthropic
```

**Key Priority Order:**
1. Environment variable (e.g., `ANTHROPIC_API_KEY`)
2. System keyring (via `clawdius auth set`)
3. Config file `api_key` field (not recommended)

### 4.3 Config File (Not Recommended)

You can also specify keys in `config.toml` (less secure):

```toml
[llm.anthropic]
api_key = "sk-ant-..."  # WARNING: Visible in file, git history
```

---

## 5. Commands

### 5.1 `clawdius init`

Initialize Clawdius in the current directory.

```bash
clawdius init [NAME]
```

| Argument | Description | Default |
|----------|-------------|---------|
| `NAME` | Project name | Directory name |

**Example:**
```bash
clawdius init                    # Initialize with directory name
clawdius init my-app            # Initialize with specific name
```

### 5.2 `clawdius setup`

Interactive setup wizard for first-time configuration.

```bash
clawdius setup [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--quick`, `-q` | Skip welcome screen |
| `--provider`, `-P` | Pre-select provider |

**Example:**
```bash
clawdius setup                        # Full interactive setup
clawdius setup --provider anthropic   # Pre-select Anthropic
clawdius setup --quick                # Quick setup
```

### 5.3 `clawdius chat`

Send a message to the LLM or start an interactive session.

```bash
clawdius chat [MESSAGE] [OPTIONS]
```

| Flag | Description | Default |
|------|-------------|---------|
| `--provider`, `-P` | LLM provider | `anthropic` |
| `--model`, `-m` | Model to use | Provider default |
| `--session`, `-s` | Continue from session ID | New session |
| `--mode`, `-M` | Agent mode | `code` |
| `--editor`, `-e` | Open external editor to compose | Disabled |
| `--exit` | Non-interactive mode (auto with prompt) | Disabled |
| `--quiet` | Suppress all output except response | Disabled |
| `--auto-approve` | Auto-approve all tool executions | Disabled |

**Built-in Modes:** `code`, `architect`, `ask`, `debug`, `review`, `refactor`, `test`, `auto`

**Examples:**
```bash
# Interactive chat
clawdius chat

# Single-shot message
clawdius chat "Explain this code"

# Specify provider and model
clawdius chat "Write tests" --provider openai --model gpt-4o

# Continue previous session
clawdius chat "Continue" --session abc123

# Use specific agent mode
clawdius chat --mode architect

# Use local Ollama
clawdius chat "Hello" --provider ollama --model llama3.2

# Compose in editor
clawdius chat --editor
```

### 5.4 `clawdius auto`

Autonomous CI/CD mode - run tasks without interaction.

```bash
clawdius auto <TASK> [OPTIONS]
```

| Flag | Description | Default |
|------|-------------|---------|
| `--provider`, `-P` | LLM provider | `anthropic` |
| `--model`, `-m` | Model to use | Provider default |
| `--max-iterations` | Max iterations before stopping | 50 |
| `--run-tests` | Run tests after changes | Disabled |
| `--auto-commit` | Commit changes automatically | Disabled |
| `--fail-on-test-failure` | Fail if tests fail after changes | Disabled |
| `--output-format` | CI output format (text, json, github-actions) | text |

**Examples:**
```bash
clawdius auto "fix failing tests" --run-tests --fail-on-test-failure
clawdius auto "implement user auth" --auto-commit
clawdius auto "refactor module" --output-format json
```

### 5.5 `clawdius sessions`

List and manage conversation sessions.

```bash
clawdius sessions [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--delete`, `-d` | Delete a session by ID |
| `--search`, `-s` | Search session messages |

**Examples:**
```bash
# List all sessions
clawdius sessions

# Delete a session
clawdius sessions --delete abc123

# Search sessions
clawdius sessions --search "error handling"
```

### 5.6 `clawdius auth`

Manage API keys in system keyring (requires `keyring` feature).

```bash
clawdius auth <COMMAND>
```

| Command | Description |
|---------|-------------|
| `set <provider>` | Store API key for provider |
| `get <provider>` | Retrieve stored API key |
| `delete <provider>` | Delete stored API key |

**Supported Providers:** `anthropic`, `openai`, `zai`

**Examples:**
```bash
clawdius auth set anthropic
clawdius auth get anthropic
clawdius auth delete anthropic
```

### 5.7 `clawdius generate`

Generate code using agentic AI.

```bash
clawdius generate <PROMPT> [OPTIONS]
```

| Flag | Description | Default |
|------|-------------|---------|
| `--files`, `-f` | Target files (comma-separated) | None |
| `--mode`, `-M` | Generation mode: `single-pass`, `iterative`, `agent` | `single-pass` |
| `--trust`, `-T` | Trust level: `low`, `medium`, `high` | `medium` |
| `--test-strategy` | Test execution: `sandboxed`, `direct`, `skip` | None |
| `--max-iterations`, `-i` | Max iterations for iterative/agent mode | 5 |
| `--dry-run` | Preview without applying | Disabled |
| `--provider`, `-P` | LLM provider | `anthropic` |
| `--model`, `-m` | Model to use | Provider default |
| `--stream` | Enable streaming output | Disabled |
| `--incremental` | Enable incremental (diff-based) generation | Disabled |
| `--timeout-secs`, `-R` | Timeout for LLM operations | None |

**Examples:**
```bash
clawdius generate "create REST API handlers"
clawdius generate "add validation" --files src/api.rs --dry-run
clawdius generate "implement feature" --mode agent --max-iterations 10
```

### 5.8 `clawdius refactor`

Plan and execute cross-language refactoring.

```bash
clawdius refactor --from LANG --to LANG [PATH] [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--from`, `-f` | Source language |
| `--to`, `-t` | Target language |
| `--path`, `-p` | Path to file or directory |
| `--dry-run` | Preview without applying |

**Example:**
```bash
clawdius refactor --from typescript --to rust src/ --dry-run
```

### 5.9 `clawdius test`

Generate tests for code.

```bash
clawdius test <FILE> [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--function` | Function to generate tests for (all if omitted) |
| `--output`, `-o` | Output file path |

**Example:**
```bash
clawdius test src/lib.rs
clawdius test src/lib.rs --function my_function
clawdius test src/lib.rs -o tests/lib_test.rs
```

### 5.10 `clawdius doc`

Generate documentation for code.

```bash
clawdius doc <FILE> [OPTIONS]
```

| Flag | Description | Default |
|------|-------------|---------|
| `--element` | Element to document (function, struct, module) | None |
| `--format`, `-f` | Doc format: `auto`, `rustdoc`, `jsdoc`, `pydoc`, `markdown` | `auto` |
| `--output`, `-o` | Output file path (stdout if omitted) | stdout |
| `--inline` | Include inline comments | Disabled |

**Example:**
```bash
clawdius doc src/lib.rs
clawdius doc src/lib.rs --element MyStruct --format rustdoc
clawdius doc src/lib.rs --inline -o docs/api.md
```

### 5.11 `clawdius analyze`

Analyze codebase for architecture drift and technical debt.

```bash
clawdius analyze [PATH] [OPTIONS]
```

| Flag | Description | Default |
|------|-------------|---------|
| `--drift` | Analyze for architecture drift only | Disabled |
| `--debt` | Analyze for technical debt only | Disabled |
| `--format`, `-f` | Output format: `text`, `json` | `text` |
| `--output`, `-o` | Output file path | stdout |
| `--severity` | Minimum severity: `low`, `medium`, `high`, `critical` | `low` |
| `--exclude` | Exclude patterns (comma-separated) | None |

**Examples:**
```bash
clawdius analyze .
clawdius analyze . --drift --severity high
clawdius analyze . --debt -f json -o report.json
```

### 5.12 `clawdius action`

Apply a code action.

```bash
clawdius action <ACTION> <FILE> [OPTIONS]
```

| Argument | Description |
|----------|-------------|
| `ACTION` | Action: `extract-function`, `extract-variable`, `inline-variable`, `rename`, `move-module`, `generate-tests` |
| `FILE` | File path |

| Flag | Description |
|------|-------------|
| `--line`, `-l` | Line number |
| `--column`, `-c` | Column number |
| `--end-line`, `-s` | End line for selection |
| `--end-column`, `-e` | End column for selection |

### 5.13 `clawdius broker`

Activate financial monitoring and trading signals.

```bash
clawdius broker [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--config`, `-c` | Broker configuration file |
| `--paper-trade` | Simulate without executing trades |

**Example:**
```bash
clawdius broker --config broker.toml --paper-trade
```

### 5.14 `clawdius verify`

Run Lean 4 proofs and SOP compliance checks.

```bash
clawdius verify --proof PATH [--lean-path PATH]
```

| Flag | Description |
|------|-------------|
| `--proof`, `-p` | Path to .lean proof file or directory |
| `--lean-path` | Path to lean binary |

**Example:**
```bash
clawdius verify --proof proofs/
```

### 5.15 `clawdius compliance`

Generate compliance matrix.

```bash
clawdius compliance --standards STANDARDS [OPTIONS]
```

| Flag | Description | Default |
|------|-------------|---------|
| `--standards`, `-s` | Standards (comma-separated) | Required |
| `--path`, `-p` | Project root | `.` |
| `--format`, `-f` | Output format | `markdown` |
| `--output`, `-o` | Output file | stdout |

**Example:**
```bash
clawdius compliance --standards iso26262,do178c --format markdown
```

### 5.16 `clawdius research`

Multi-lingual research synthesis.

```bash
clawdius research <QUERY> [OPTIONS]
```

| Flag | Description | Default |
|------|-------------|---------|
| `--languages`, `-l` | Languages (comma-separated) | All |
| `--tqa-level`, `-L` | Minimum TQA level (1-5) | `3` |
| `--max-results`, `-m` | Max results per language | `10` |

**Example:**
```bash
clawdius research "distributed systems consensus" --languages en,zh,ru
```

### 5.17 `clawdius git`

Git workflow operations with AI-generated commit messages.

```bash
clawdius git <COMMAND>
```

| Command | Description |
|---------|-------------|
| `commit [FILES...]` | Stage files and create AI-generated commit |
| `diff` | Show working diff |
| `diff --staged` | Show staged diff |
| `status` | Show git status summary |

**Examples:**
```bash
clawdius git commit                   # Stage all, AI-generated message
clawdius git commit src/lib.rs        # Stage specific files
clawdius git commit -m "fix typo"     # Use provided message
clawdius git diff
clawdius git status
```

### 5.18 `clawdius memory`

Manage project memory (CLAUDE.md).

```bash
clawdius memory <COMMAND>
```

| Command | Description |
|---------|-------------|
| `show` | Show project memory |
| `show --instructions` | Show as LLM-ready instructions |
| `learn <type> <content>` | Learn a new entry (build, test, debug, pattern, preference) |
| `instructions <content>` | Set project instructions |
| `list [category]` | List entries by category |
| `clear [category]` | Clear entries |
| `init` | Create/update CLAUDE.md file |

**Examples:**
```bash
clawdius memory show
clawdius memory learn pattern "always use Result<T, E>"
clawdius memory instructions "This project uses async Rust"
clawdius memory init --name my-project --language rust --framework actix-web
```

### 5.19 `clawdius workflow`

Manage agentic workflows.

```bash
clawdius workflow <COMMAND>
```

| Command | Description |
|---------|-------------|
| `list` | List all workflows |
| `create <name>` | Create a new workflow |
| `show <id>` | Show workflow details |
| `run <id>` | Execute a workflow |
| `cancel <execution-id>` | Cancel a running workflow |
| `status <execution-id>` | Show execution status |
| `delete <id>` | Delete a workflow |

**Examples:**
```bash
clawdius workflow list
clawdius workflow create "code-review" --description "Automated review pipeline"
clawdius workflow run <workflow-id> --provider anthropic
```

### 5.20 `clawdius modes`

Manage agent modes.

```bash
clawdius modes <COMMAND>
```

| Command | Description |
|---------|-------------|
| `list` | List all available modes |
| `create <name>` | Create a custom mode |
| `show <name>` | Show mode details |

**Examples:**
```bash
clawdius modes list
clawdius modes create security-review
clawdius modes show architect
```

### 5.21 `clawdius models`

Manage local LLM models (Ollama).

```bash
clawdius models <COMMAND> [OPTIONS]
```

| Command | Description |
|---------|-------------|
| `list` | List available local models |
| `pull <model>` | Pull a model from registry |
| `health` | Check Ollama server health |
| `current` | Show current model |

| Flag | Description | Default |
|------|-------------|---------|
| `--host`, `-H` | Ollama host | `localhost` |
| `--port`, `-p` | Ollama port | `11434` |

**Examples:**
```bash
clawdius models list
clawdius models pull llama3.2
clawdius models health
```

### 5.22 `clawdius nexus`

Run the Nexus 24-phase FSM engine.

```bash
clawdius nexus start [PATH]
```

| Flag | Description | Default |
|------|-------------|---------|
| `--path`, `-p` | Project root path | `.` |

**Example:**
```bash
clawdius nexus start .
```

### 5.23 `clawdius webhook`

Manage webhooks for event notifications.

```bash
clawdius webhook <COMMAND>
```

| Command | Description |
|---------|-------------|
| `list` | List all webhooks |
| `create <name> <url>` | Create a webhook |
| `show <id>` | Show webhook details |
| `update <id>` | Update a webhook |
| `delete <id>` | Delete a webhook |
| `test <id>` | Test a webhook |
| `deliveries [id]` | Show delivery history |
| `stats` | Show webhook statistics |

### 5.24 `clawdius metrics`

Show performance metrics.

```bash
clawdius metrics [OPTIONS]
```

| Flag | Description | Default |
|------|-------------|---------|
| `--format`, `-f` | Output format: `text`, `json`, `html` | `text` |
| `--output`, `-o` | Output file path | stdout |
| `--reset`, `-r` | Reset metrics after displaying | Disabled |
| `--watch`, `-w` | Watch mode - continuously display | Disabled |

### 5.25 `clawdius telemetry`

Configure telemetry settings.

```bash
clawdius telemetry [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--enable` | Enable telemetry |
| `--disable` | Disable telemetry |
| `--enable-metrics` | Enable metrics collection |
| `--enable-crash-reporting` | Enable crash reporting |

### 5.26 `clawdius watch`

Watch files for changes and trigger auto-analysis.

```bash
clawdius watch [PATH] [OPTIONS]
```

| Flag | Description | Default |
|------|-------------|---------|
| `--ignore` | Patterns to ignore (comma-separated) | None |
| `--auto-analyze` | Enable auto-analysis on changes | Disabled |
| `--debounce-ms` | Debounce interval in milliseconds | `500` |
| `--verbose`, `-v` | Enable verbose output | Disabled |

### 5.27 `clawdius complete`

Get inline code completions from LLM.

```bash
clawdius complete <FILE> <LINE> <CHARACTER> [OPTIONS]
```

| Flag | Description | Default |
|------|-------------|---------|
| `--language` | Programming language | Auto-detected |
| `--provider`, `-P` | LLM provider | `ollama` |
| `--model`, `-m` | Model name | None |

---

## 6. File Timeline

The file timeline system provides complete change tracking and rollback capability.

### 6.1 Creating Checkpoints

Create a checkpoint to save the current state of your project:

```bash
clawdius timeline create <name> [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--description`, `-d` | Description of the checkpoint |

**Examples:**
```bash
clawdius timeline create "before-refactor"
clawdius timeline create "v1.0-rc1" --description "Release candidate 1"
```

### 6.2 Listing Checkpoints

View all timeline checkpoints:

```bash
clawdius timeline list
```

### 6.3 Rolling Back

Restore project to a previous checkpoint:

```bash
clawdius timeline rollback <checkpoint-id>
```

### 6.4 Viewing Diff

Compare changes between checkpoints:

```bash
clawdius timeline diff <from-id> <to-id>
```

### 6.5 File History

View change history for a specific file:

```bash
clawdius timeline history <file-path>
```

### 6.6 Session Checkpoints

Session-level checkpoints provide per-session save/restore:

```bash
clawdius checkpoint create "before-refactor"
clawdius checkpoint list
clawdius checkpoint show <checkpoint-id>
clawdius checkpoint restore <checkpoint-id>
clawdius checkpoint compare <id1> <id2>
clawdius checkpoint delete <checkpoint-id>
clawdius checkpoint cleanup --keep 10
```

### 6.7 Timeline Watch Mode

Automatically create checkpoints when files change:

```bash
clawdius timeline watch
```

| Flag | Description | Default |
|------|-------------|---------|
| `--debounce-secs`, `-d` | Debounce interval in seconds | `30` |
| `--ignore`, `-i` | Patterns to ignore (repeatable) | None |
| `--max-per-hour`, `-m` | Maximum checkpoints per hour | `120` |

---

## 7. JSON Output

All CLI commands support JSON output via the global `--output-format` flag for programmatic consumption.

### 7.1 Usage

Add `-f json` (or `--output-format json`) to any command:

```bash
clawdius <command> -f json
```

### 7.2 Examples

**Chat Command:**
```bash
clawdius chat "Explain this code" -f json
```

**Metrics Command:**
```bash
clawdius metrics -f json
```

**Analyze Command:**
```bash
clawdius analyze . -f json -o report.json
```

### 7.3 Stream JSON Mode

For real-time streaming output:

```bash
clawdius chat -f stream-json
```

---

## 8. Enhanced Completions

The completion system features improved performance and reliability.

### 8.1 Features

- **LRU Caching:** Faster responses through intelligent caching
- **Smart Fallbacks:** Language-specific fallback strategies
- **Timeout Handling:** Reliable completion generation
- **Multi-Language Support:** Rust, Python, JavaScript/TypeScript, Go

### 8.2 Usage

Get inline completions:

```bash
clawdius complete src/main.rs 42 10 --language rust --provider ollama
```

### 8.3 Supported Languages

| Language | Status | Features |
|----------|--------|----------|
| Rust | Supported | Full syntax support, type inference |
| Python | Supported | Full syntax support, type hints |
| JavaScript | Supported | ES6+, TypeScript |
| TypeScript | Supported | Full type system support |
| Go | Supported | Full syntax support |

---

## 9. Workflows

### 9.1 Starting a New Project

```bash
# 1. Initialize
clawdius init

# 2. Set up API keys
export ANTHROPIC_API_KEY="sk-ant-..."
# Or use keyring:
clawdius auth set anthropic

# 3. Edit configuration
vim .clawdius/config.toml

# 4. Create initial checkpoint
clawdius timeline create "initial" --description "Initial project state"

# 5. Start a chat
clawdius chat "Help me understand this codebase"

# 6. View sessions
clawdius sessions
```

### 9.2 Autonomous Development

```bash
# Fix a bug autonomously
clawdius auto "fix the failing test in src/auth.rs" --run-tests --fail-on-test-failure

# Implement a feature
clawdius auto "implement user authentication" --auto-commit --run-tests

# CI integration with JSON output
clawdius auto "resolve lint errors" --output-format json
```

### 9.3 Code Refactoring with Timeline

```bash
# 1. Create checkpoint before refactoring
clawdius timeline create "before-refactor" --description "Pre-refactor state"

# 2. Analyze current codebase
clawdius analyze . --drift

# 3. Preview migration
clawdius refactor --from typescript --to rust src/ --dry-run

# 4. Execute migration
clawdius refactor --from typescript --to rust src/

# 5. If something goes wrong, rollback
clawdius timeline rollback <checkpoint-id>

# 6. Verify results
clawdius verify --proof proofs/
```

### 9.4 Using Multiple Providers

```bash
# Quick question with OpenAI
clawdius chat "Explain this regex" --provider openai

# Deep analysis with Claude
clawdius chat "Review architecture" --provider anthropic

# Local development with Ollama
clawdius chat "Write tests" --provider ollama --model llama3.2

# JSON output for scripting
clawdius chat "Analyze code" -f json | jq '.response'
```

### 9.5 HFT Mode Setup

```bash
# 1. Configure risk parameters
clawdius broker --config hft_config.toml --paper-trade

# 2. Start broker mode
clawdius broker --config hft_config.toml
```

### 9.6 Timeline-Based Development

```bash
# 1. Create feature branch checkpoint
clawdius timeline create "feature-auth-start"

# 2. Make changes and create intermediate checkpoints
clawdius timeline create "auth-models-done"
clawdius timeline create "auth-routes-done"

# 3. Review changes
clawdius timeline diff "feature-auth-start" "auth-routes-done"

# 4. If needed, rollback to intermediate state
clawdius timeline rollback "auth-models-done"
```

### 9.7 Project Memory Workflow

```bash
# Initialize project memory
clawdius memory init --name my-project --language rust

# Set project instructions
clawdius memory instructions "Use tower for middleware, sqlx for database"

# Learn patterns from development
clawdius memory learn build "cargo build --release"
clawdius memory learn test "cargo test --all"

# Review accumulated knowledge
clawdius memory show
clawdius memory list patterns
```

---

## 10. Troubleshooting

### 10.1 Common Issues

#### "Failed to initialize state machine"

**Cause:** Corrupted `.clawdius/` directory

**Solution:**
```bash
rm -rf .clawdius/
clawdius init
```

#### "LLM provider error" / "API key not set"

**Cause:** Invalid or missing API key

**Solution:**
```bash
# Check environment variable
echo $ANTHROPIC_API_KEY

# Or set via keyring
clawdius auth set anthropic

# Or run setup wizard
clawdius setup

# Test connection
clawdius chat "Hello" --provider anthropic
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
RUST_LOG=clawdius=debug clawdius chat "Hello"
```

### 10.3 Getting Help

- **Documentation:** `.docs/` directory
- **Issues:** https://github.com/clawdius/clawdius/issues
- **Discord:** https://discord.gg/clawdius

---

## Appendix A: Monorepo Development

### A.1 Building from Source

The monorepo allows building all components from a single repository:

```bash
# Clone
git clone https://github.com/clawdius/clawdius
cd clawdius

# Build all crates
cargo build --release

# Build specific crate
cargo build -p clawdius-core

# Run tests
cargo test --all
```

### A.2 Development Workflow

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

### A.3 Feature Flags

Features propagate across crates:

```bash
# Build CLI with keyring support
cargo build -p clawdius --features keyring

# Build CLI with vector-db support
cargo build -p clawdius --features vector-db
```

---

## Appendix B: Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `RUST_LOG` | Log level | `info` |
| `ANTHROPIC_API_KEY` | Anthropic API key | - |
| `OPENAI_API_KEY` | OpenAI API key | - |
| `ZAI_API_KEY` | Z.AI API key | - |
| `OLLAMA_BASE_URL` | Ollama server URL | `http://localhost:11434` |

## Appendix C: Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error |

## Appendix D: VSCode Extension Commands

| Command | Description |
|---------|-------------|
| `Clawdius: Ask a question` | Open chat input |
| `Clawdius: Chat with selection` | Chat about selected code |
| `Clawdius: Add file to context` | Add file to conversation |
| `Clawdius: Add current file to context` | Add active editor file |
| `Clawdius: Create checkpoint` | Save current state |
| `Clawdius: Open chat view` | Open sidebar chat |

## Appendix E: Global Flags

All commands accept these global flags:

| Flag | Short | Description |
|------|-------|-------------|
| `--no-tui` | `-n` | Run without TUI (headless mode) |
| `--cwd` | `-w` | Working directory |
| `--output-format` | `-f` | Output format: `text`, `json`, `stream-json` |
| `--quiet` | `-q` | Quiet mode (no progress indicators) |
| `--config` | `-C` | Path to config file |
| `--lang` | `-L` | Output language (en, zh, ja, ko, de, fr, es, it, pt, ru) |
