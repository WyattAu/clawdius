# Clawdius CLI

The command-line interface for Clawdius - The High-Assurance Engineering Engine.

---

## Overview

This crate provides the `clawdius` binary, a terminal-based interface for the Clawdius AI engineering engine. It features:

- **Terminal UI (TUI):** 60FPS responsive interface built with Ratatui
- **Multi-runtime Support:** monoio on Linux, tokio on macOS/WSL2
- **Interactive Chat:** High-assurance AI sessions with context awareness
- **Project Management:** Initialize and manage `.clawdius/` directories

---

## Installation

### From Source

```bash
# In the monorepo root
cargo build --release -p clawdius

# Binary location
./target/release/clawdius
```

### Via Cargo

```bash
cargo install clawdius
```

---

## Commands

### `clawdius init`

Initialize Clawdius in the current directory.

```bash
clawdius init [OPTIONS]
```

**Options:**
- `--force` - Overwrite existing `.clawdius/` directory

**Creates:**
```
.clawdius/
├── sops/           # Standard Operating Procedures
│   ├── common.sop.md
│   └── rust.sop.md
├── specs/          # Specifications and papers
├── graph/          # Knowledge graph storage
│   ├── ast.db
│   └── vectors/
└── settings.toml   # Project configuration
```

**Example:**
```bash
clawdius init
# Output: Initialized Clawdius in /path/to/project
```

---

### `clawdius chat`

Start an interactive high-assurance chat session.

```bash
clawdius chat [OPTIONS]
```

**Options:**
- `--provider <PROVIDER>` - LLM provider (openai, anthropic, deepseek, ollama) [default: openai]
- `--model <MODEL>` - Model to use [default: gpt-4]
- `--no-tui` - Disable TUI, use simple REPL

**Examples:**
```bash
# Default session
clawdius chat

# Use Anthropic Claude
clawdius chat --provider anthropic --model claude-3-opus

# Local model with Ollama
clawdius chat --provider ollama --model llama2

# Simple REPL mode
clawdius chat --no-tui
```

**TUI Controls:**
- `Ctrl+C` - Exit
- `Ctrl+L` - Clear screen
- `↑/↓` - Scroll history
- `Tab` - Autocomplete

---

### `clawdius refactor`

Plan and execute cross-language refactoring.

```bash
clawdius refactor --from <LANG> --to <LANG> [PATH]
```

**Options:**
- `--from <LANG>` - Source language (typescript, javascript, python, etc.)
- `--to <LANG>` - Target language (rust, go, etc.)
- `--analyze` - Only analyze, don't execute
- `--dry-run` - Show plan without executing

**Examples:**
```bash
# Analyze TypeScript to Rust migration
clawdius refactor --from typescript --to rust --analyze src/

# Execute migration
clawdius refactor --from typescript --to rust src/

# Dry run with preview
clawdius refactor --from python --to rust --dry-run api/
```

**Output:**
- Generates migration plan in `.clawdius/specs/migration_plan.md`
- Shows file-by-file breakdown
- Estimates complexity and time

---

### `clawdius broker`

Activate financial monitoring and trading signals (HFT mode).

```bash
clawdius broker [OPTIONS]
```

**Options:**
- `--config <FILE>` - Broker configuration file
- `--dry-run` - Simulate without executing trades

**Prerequisites:**
- Build with `--features broker-mode`
- Configure risk parameters in `settings.toml`

**Examples:**
```bash
# Build with broker support
cargo build --features broker-mode

# Run with config
clawdius broker --config broker.toml

# Dry run mode
clawdius broker --config broker.toml --dry-run
```

**Configuration (`broker.toml`):**
```toml
[broker]
exchange = "binance"
api_key_env = "BINANCE_API_KEY"
api_secret_env = "BINANCE_API_SECRET"

[risk]
position_limit = 1000000
daily_drawdown_limit = 50000
max_leverage = 3

[notifications]
matrix_room = "!room:matrix.org"
whatsapp_number = "+1234567890"
```

---

### `clawdius timeline`

Manage file timeline checkpoints and rollback.

#### `clawdius timeline create`

Create a new checkpoint.

```bash
clawdius timeline create <NAME> [OPTIONS]
```

**Options:**
- `--description <DESC>` - Description of the checkpoint
- `--tag <TAG>` - Tag for categorization

**Examples:**
```bash
# Simple checkpoint
clawdius timeline create "before-refactor"

# With description
clawdius timeline create "v1.0-rc1" --description "Release candidate 1"

# With tag
clawdius timeline create "feature-auth" --tag feature
```

#### `clawdius timeline list`

List all checkpoints.

```bash
clawdius timeline list [OPTIONS]
```

**Options:**
- `--format <FORMAT>` - Output format (text, json) [default: text]
- `--limit <N>` - Limit number of results

**Examples:**
```bash
# List all checkpoints
clawdius timeline list

# JSON output
clawdius timeline list --format json

# Limit results
clawdius timeline list --limit 10
```

#### `clawdius timeline rollback`

Rollback to a specific checkpoint.

```bash
clawdius timeline rollback <CHECKPOINT-ID> [OPTIONS]
```

**Options:**
- `--dry-run` - Preview changes without applying
- `--force` - Force rollback without confirmation

**Examples:**
```bash
# Rollback to checkpoint
clawdius timeline rollback abc123

# Preview rollback
clawdius timeline rollback abc123 --dry-run

# Force rollback
clawdius timeline rollback abc123 --force
```

#### `clawdius timeline diff`

View diff between checkpoints.

```bash
clawdius timeline diff <FROM-ID> <TO-ID> [OPTIONS]
```

**Options:**
- `--format <FORMAT>` - Diff format (unified, json) [default: unified]
- `--stat` - Show statistics only

**Examples:**
```bash
# View diff
clawdius timeline diff abc123 def456

# Statistics only
clawdius timeline diff abc123 def456 --stat

# JSON format
clawdius timeline diff abc123 def456 --format json
```

#### `clawdius timeline history`

View file change history.

```bash
clawdius timeline history <FILE-PATH> [OPTIONS]
```

**Options:**
- `--limit <N>` - Limit number of entries
- `--format <FORMAT>` - Output format (text, json) [default: text]

**Examples:**
```bash
# View file history
clawdius timeline history src/main.rs

# Limited history
clawdius timeline history src/main.rs --limit 20

# JSON output
clawdius timeline history src/main.rs --format json
```

---

### `clawdius metrics`

Show project metrics and statistics.

```bash
clawdius metrics [OPTIONS]
```

**Options:**
- `--format <FORMAT>` - Output format (text, json) [default: text]

**Examples:**
```bash
# Show metrics
clawdius metrics

# JSON output
clawdius metrics --format json
```

**Output:**
```
Project Metrics:
  Files indexed: 234
  AST nodes: 12,456
  Vectors: 8,901
  Sessions: 12
  Timeline checkpoints: 5
  
Code Completion Stats:
  Cache hit rate: 87%
  Avg response time: 234ms
  Languages: rust, python, javascript
```

---

### `clawdius verify`

Run Lean 4 proofs and SOP compliance checks.

```bash
clawdius verify [OPTIONS]
```

**Options:**
- `--lean4` - Run formal verification proofs
- `--sop` - Check SOP compliance
- `--all` - Run all verification

**Examples:**
```bash
# Verify SOP compliance
clawdius verify --sop

# Run Lean 4 proofs
clawdius verify --lean4

# Full verification
clawdius verify --all
```

---

### `clawdius status`

Show current project status and phase.

```bash
clawdius status
```

**Output:**
```
Project: my-project
Version: 0.1.0
Phase: 6.5 (Documentation Verification)
Status: IN PROGRESS
Rigor Score: 0.92

Graph-RAG:
  Files indexed: 234
  AST nodes: 12,456
  Vectors: 8,901

Recent Activity:
  - Phase 6 completed (Security Engineering)
  - SOP compliance: 98%
  - 3 pending actions
```

---

## Configuration

### Settings File (`.clawdius/settings.toml`)

```toml
[project]
name = "my-project"
version = "0.1.0"

[llm]
provider = "openai"
model = "gpt-4"
temperature = 0.7
max_tokens = 4096

[sandbox]
default_tier = 2
# 1 = Native (bubblewrap/sandbox-exec)
# 2 = Container (Podman)
# 3 = WASM (Wasmtime)
# 4 = Hardened (restrictive container)

[graph_rag]
ast_index = true
vector_index = true
embedding_model = "text-embedding-ada-002"

[hft]
enabled = false
position_limit = 1000000
daily_drawdown_limit = 50000

[ui]
theme = "dark"
fps = 60
```

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `RUST_LOG` | Log level | `info` |
| `CLAWDIUS_CONFIG` | Config directory | `.clawdius/` |
| `OPENAI_API_KEY` | OpenAI API key | - |
| `ANTHROPIC_API_KEY` | Anthropic API key | - |
| `DEEPSEEK_API_KEY` | DeepSeek API key | - |

---

## Features

### Build Features

```bash
# Standard build
cargo build -p clawdius

# With HFT mode (high-frequency trading)
cargo build -p clawdius --features hft-mode

# With broker mode
cargo build -p clawdius --features broker-mode

# All features
cargo build -p clawdius --all-features
```

### Feature Flags

| Feature | Description | Dependencies |
|---------|-------------|--------------|
| `default` | Standard features + mimalloc | mimalloc |
| `hft-mode` | High-frequency trading mode | clawdius-core/hft-mode |
| `broker-mode` | Financial broker integration | clawdius-core/broker-mode |

---

## Architecture

### Binary Structure

```
clawdius binary
├── main.rs           # Entry point
├── cli.rs            # CLI argument parsing (clap)
├── tui/              # Terminal UI (ratatui)
│   ├── app.rs        # Application state
│   ├── ui.rs         # Rendering
│   └── event.rs      # Event handling
└── commands/         # Command implementations
    ├── init.rs
    ├── chat.rs
    ├── refactor.rs
    └── ...
```

### Runtime Selection

The CLI automatically selects the optimal runtime:

```rust
// Linux with io_uring
#[cfg(all(target_os = "linux", feature = "io-uring"))]
type Runtime = monoio::Runtime;

// macOS, WSL2, or without io_uring
#[cfg(any(target_os = "macos", not(feature = "io-uring")))]
type Runtime = tokio::runtime::Runtime;
```

---

## Development

### Running Tests

```bash
# Unit tests
cargo test -p clawdius

# Integration tests
cargo test -p clawdius --test '*'

# Benchmarks
cargo bench -p clawdius
```

### Debugging

```bash
# Enable debug logging
RUST_LOG=clawdius=debug clawdius chat

# Enable trace logging
RUST_LOG=clawdius=trace clawdius chat

# Log to file
RUST_LOG=debug clawdius chat 2> clawdius.log
```

---

## Troubleshooting

### "Failed to initialize state machine"

**Cause:** Corrupted `.clawdius/` directory

**Solution:**
```bash
rm -rf .clawdius/
clawdius init
```

### "Sandbox creation failed"

**Linux:**
```bash
sudo apt install bubblewrap
```

**macOS:**
```bash
# sandbox-exec is built-in, ensure SIP is enabled
csrutil status
```

### "LLM provider error"

**Cause:** Invalid API key or network issue

**Solution:**
```bash
# Check API key
echo $OPENAI_API_KEY

# Test connection
curl https://api.openai.com/v1/models \
  -H "Authorization: Bearer $OPENAI_API_KEY"
```

### TUI Not Displaying Correctly

**Cause:** Terminal doesn't support required features

**Solution:**
```bash
# Use simple REPL mode
clawdius chat --no-tui

# Or update terminal emulator
```

---

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error |
| 2 | Configuration error |
| 3 | Sandbox error |
| 4 | LLM error |
| 5 | Verification failed |
| 6 | Network error |
| 7 | Authentication error |

---

## See Also

- [Core Library Documentation](../clawdius-core/README.md)
- [Architecture Overview](../../.docs/architecture_overview.md)
- [User Guide](../../.docs/user_guide.md)

---

## License

Apache 2.0 - See [LICENSE](../../LICENSE) for details.
