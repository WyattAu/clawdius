# Clawdius

**The High-Assurance Engineering Engine.**  
*Powered by Rust. Governed by SOPs. Verified by Nexus.*

[![Version](https://img.shields.io/badge/version-1.0.0--rc.1-blue.svg)](https://github.com/clawdius/clawdius/releases/tag/v1.0.0-rc.1)
[![Rust](https://img.shields.io/badge/language-Rust-orange.svg)](https://www.rust-lang.org)
[![Security: Sentinel](https://img.shields.io/badge/Security-Sentinel_JIT-blue.svg)](#-the-sentinel-jit-sandboxing)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache_2.0-yellow.svg)](LICENSE)

**Clawdius** is a next-generation AI agentic engine built for developers who can't afford hallucinations and traders who can't afford latency. While other "claws" run on bloated Node.js runtimes with raw shell access, Clawdius is a native Rust binary that enforces a formal R&D lifecycle and executes code in strictly isolated, just-in-time sandboxes.

---

## Features

- **Multi-Provider LLM Support** - Anthropic, OpenAI, Ollama, ZAI with automatic retry
- **File Timeline System** - Complete change tracking with checkpoints and rollback capability
- **JSON Output** - All commands support JSON output for programmatic consumption
- **Enhanced Completions** - LRU-cached, language-specific code completions with smart fallbacks
- **Secure Shell Sandboxing** - Blocked command patterns, timeout limits, directory restrictions
- **System Keyring Storage** - Securely store API keys in OS keychain
- **Session Management** - Persistent conversations with auto-compact
- **Tool Execution** - File, shell, and git operations with safety controls
- **VSCode Extension** - Full IDE integration with chat and context
- **Streaming Responses** - Real-time LLM response streaming
- **Configuration File** - TOML-based project and provider configuration

---

## Monorepo Structure

Clawdius is organized as a Rust workspace with multiple crates:

```
clawdius/
├── crates/
│   ├── clawdius/              # CLI application
│   │   └── src/main.rs        # Binary entry point
│   ├── clawdius-core/         # Core library
│   │   └── src/lib.rs         # LLM, sessions, tools, sandboxing
│   ├── clawdius-code/         # VSCode extension helper
│   │   └── src/lib.rs         # JSON-RPC server for VSCode
│   └── clawdius-webview/      # Leptos WASM webview UI
│       └── src/lib.rs         # Browser-based interface
├── editors/
│   └── vscode/                # VSCode extension
│       ├── src/extension.ts   # TypeScript extension
│       └── package.json       # Extension manifest
├── .docs/                     # Documentation
│   ├── architecture_overview.md
│   ├── user_guide.md
│   └── api_reference.md
└── Cargo.toml                 # Workspace configuration
```

### Crates

| Crate | Description | Binary/Library |
|-------|-------------|----------------|
| **clawdius** | CLI tool | Binary |
| **clawdius-core** | Core library (LLM, sessions, tools) | Library |
| **clawdius-code** | VSCode extension helper | Binary |
| **clawdius-webview** | Web-based UI | WASM Library |

---

## Why Clawdius?

| Feature      | Clawdius                           | Claude Code / OpenClaw             |
| :----------- | :--------------------------------- | :--------------------------------- |
| **Runtime**  | **Rust** (Zero GC, <20ms boot)     | Node.js (Heavy, Garbage Collected) |
| **Security** | **Sentinel JIT Sandboxing**        | Raw Shell / Local OS Access        |
| **Rigor**    | **Nexus Lifecycle** (Formal Specs) | Stochastic (Guess & Check)         |
| **Context**  | **Graph-RAG** (AST + Vector)       | Simple Vector / RAG                |
| **Trading**  | **Broker Mode** (Sub-ms latency)   | Not Supported / High Latency       |

---

## Core Pillars

### The Sentinel (JIT Sandboxing)
Stop letting AI agents run `rm -rf /` on your machine. Clawdius analyzes your project and dynamically spawns the most restrictive environment needed:
- **Tier 1 (Systems):** Bubblewrap/sandbox-exec passthrough for high-performance C++/Rust.
- **Tier 2 (Scripts):** Rootless Podman containers for untrusted Node.js/Python code.
- **Privacy:** Your API keys and SSH secrets stay in the Host memory; they are never visible to the agent.

### Graph-RAG Intelligence
Clawdius doesn't just "read" your files; it understands them.
- **Structural:** Uses `tree-sitter` to build a local SQLite graph of your codebase (Who calls whom? What defines what?).
- **Semantic:** LanceDB vector indexing for high-speed retrieval of documentation and intent.
- **Multi-Lingual:** Research SOTA findings across 16 languages (EN/ZH/RU/JP/etc.) with integrated TQA (Translation Quality Assurance).

### The Nexus Lifecycle
Clawdius enforces the **Nexus R&D Lifecycle**—a 24-phase transition from Context Discovery to Knowledge Transfer.
- **Yellow Papers:** Theoretical ground truth and mathematical proofs.
- **Blue Papers:** IEEE 1016-compliant architectural specifications.
- **SOPs:** Active Standard Operating Procedures that Clawdius "signs off" on before every commit.

### The Broker (Financial Guard)
Deploy Clawdius as a 24/7 financial assistant on your server or Mac Mini.
- **Low Latency:** Zero garbage collection pauses for real-time market ingestion.
- **Wallet Guard:** A hard-coded safety interlock that rejects any trade violating your pre-defined risk parameters.
- **Bridge:** Instant reports via Matrix or WhatsApp when a signal is triggered.

---

## Installation

### Pre-built Binary

```bash
# Via Cargo
cargo install clawdius

# Or via Nix
nix shell github:clawdius/clawdius
```

### From Source

```bash
# Clone the monorepo
git clone https://github.com/clawdius/clawdius
cd clawdius

# Build all crates
cargo build --release

# The CLI binary will be at:
# target/release/clawdius

# Optional: Build VSCode extension
cd editors/vscode
pnpm install
pnpm run compile
```

### Verify Installation

```bash
clawdius --version
# Output: clawdius 0.7.0
```

---

## Feature Flags

Clawdius supports optional features to reduce binary size and dependencies:

| Feature | Description | Default | Dependencies Added |
|---------|-------------|---------|-------------------|
| `embeddings` | ML embeddings (candle, tokenizers) | Off | ~50-60 crates |
| `vector-db` | Vector database (lancedb, arrow) | Off | ~40-50 crates |
| `crash-reporting` | Sentry crash reporting | Off | sentry crates |

### Build Examples

```bash
# Minimal build (recommended for most users)
cargo build --release

# With ML embeddings support
cargo build --release --features embeddings

# With vector database support
cargo build --release --features vector-db

# Full featured build
cargo build --release --features "embeddings,vector-db"
```

### Binary Size Comparison

| Configuration | Dependencies | Binary Size |
|--------------|--------------|-------------|
| Minimal | ~350 | ~40MB |
| +embeddings | ~400 | ~55MB |
| +vector-db | ~450 | ~50MB |
| Full | ~696 | ~59MB |

---

## Quick Start

### 1. Set Up API Keys

```bash
# Option A: Environment variables (recommended)
export ANTHROPIC_API_KEY="sk-ant-..."
export OPENAI_API_KEY="sk-..."

# Option B: System keyring (secure)
clawd auth set-key anthropic
clawd auth set-key openai
```

### 2. Initialize a Project

```bash
clawdius init
```

This creates the `.clawdius/` directory:
- `config.toml` - Main configuration
- `sops/` - Standard Operating Procedures
- `specs/` - Specifications and papers
- `graph/` - AST and vector stores
- `sessions.db` - Conversation storage

### 3. Configure Providers (Optional)

Edit `.clawdius/config.toml`:

```toml
[llm]
default_provider = "anthropic"
max_tokens = 4096

[llm.anthropic]
model = "claude-3-5-sonnet-20241022"

[llm.openai]
model = "gpt-4o"

[llm.ollama]
model = "llama3.2"
base_url = "http://localhost:11434"

[llm.retry]
max_retries = 3
retry_on = ["rate_limit", "timeout", "server_error"]

[shell_sandbox]
timeout_secs = 120
restrict_to_cwd = true
```

### 4. Start Chatting

```bash
# Quick message
clawd chat "Explain this code"

# Specify provider and model
clawd chat "Write tests" --provider openai --model gpt-4o

# Use local Ollama
clawd chat "Hello" --provider ollama --model llama3.2
```

### 5. Manage Sessions

```bash
# List sessions
clawd sessions

# Search sessions
clawd sessions --search "error handling"

# Delete a session
clawd sessions --delete <session-id>
```

### 6. Use @Mentions for Context

Clawdius supports @mentions to include context in your messages:

```bash
# Include a file
clawd chat "Explain this @file:src/main.rs"

# Include multiple files
clawd chat "Compare @file:src/a.rs with @file:src/b.rs"

# Include folder listing
clawd chat "What's in @folder:src/components?"

# Fetch URL content
clawd chat "Summarize @url:https://example.com/doc"

# Include git diff
clawd chat "Review @git:diff"
clawd chat "Review staged changes @git:staged"

# Show recent commits
clawd chat "What changed? @git:log:5"

# Search codebase
clawd chat "Find @search:\"error handling\""

# Include workspace problems (requires LSP)
clawd chat "Fix @problems"
clawd chat "Fix errors @problems:error"
```

@mentions work in both CLI chat and TUI modes. Multiple mentions are resolved and included as context.

---

## Configuration Example

```toml
# .clawdius/config.toml

[project]
name = "my-project"
rigor_level = "high"

[llm]
default_provider = "anthropic"
max_tokens = 4096

[llm.anthropic]
model = "claude-3-5-sonnet-20241022"

[llm.retry]
max_retries = 3
initial_delay_ms = 1000
max_delay_ms = 30000
exponential_base = 2.0
retry_on = ["rate_limit", "timeout", "server_error", "network_error"]

[session]
compact_threshold = 0.85
keep_recent = 4
auto_save = true

[shell_sandbox]
blocked_commands = ["rm -rf /", "mkfs", "wget"]
timeout_secs = 120
max_output_bytes = 1048576
restrict_to_cwd = true
```

---

## File Timeline

The file timeline system provides complete change tracking and rollback capability.

### Creating Checkpoints

```bash
clawdius timeline create "before-refactor" --description "Pre-refactor checkpoint"
```

### Listing Checkpoints

```bash
clawdius timeline list
clawdius timeline list --format json
```

### Rolling Back

```bash
clawdius timeline rollback <checkpoint-id>
```

### Viewing Diff

```bash
clawdius timeline diff <from-id> <to-id>
```

### File History

```bash
clawdius timeline history src/main.rs
```

---

## JSON Output

All CLI commands support JSON output for programmatic consumption.

### Usage

```bash
clawdius <command> --format json
```

### Examples

```bash
# Init with JSON output
clawdius init . --format json

# Metrics with JSON output
clawdius metrics --format json

# Timeline list with JSON output
clawdius timeline list --format json

# Chat with JSON output
clawdius chat "Explain this" --format json
```

---

## Commands

| Command | Description |
|---------|-------------|
| `clawd init` | Initialize Clawdius in current directory |
| `clawd chat` | Send a message to the LLM |
| `clawd sessions` | List and manage conversation sessions |
| `clawd timeline create` | Create a file timeline checkpoint |
| `clawd timeline list` | List all timeline checkpoints |
| `clawd timeline rollback` | Rollback to a specific checkpoint |
| `clawd timeline diff` | View diff between checkpoints |
| `clawd timeline history` | View file change history |
| `clawd auth set-key` | Store API key in system keyring |
| `clawd auth get-key` | Retrieve stored API key |
| `clawd auth delete-key` | Delete stored API key |
| `clawd refactor` | Plan and execute cross-language refactoring |
| `clawd broker` | Activate financial monitoring and trading signals |
| `clawd verify` | Run Lean 4 proofs and SOP compliance checks |
| `clawd compliance` | Generate compliance matrix |
| `clawd research` | Multi-lingual research synthesis |

### CLI Options

```bash
clawd chat "message" [OPTIONS]

Options:
  -P, --provider <PROVIDER>  LLM provider (anthropic, openai, ollama, zai)
  -m, --model <MODEL>        Model to use
  -s, --session <ID>         Continue from session ID
  -f, --format <FORMAT>      Output format (text, json, stream-json)
  -C, --config <PATH>        Path to config file
  --no-tui                   Run without TUI (headless mode)
  --quiet                    Quiet mode (no progress indicators)
```

See the [User Guide](.docs/user_guide.md) for detailed command documentation.

---

## Architecture

Clawdius is built with a modular architecture:

- **Engine:** Rust (Tokio/monoio runtime)
- **Logic:** Wasmtime (Brain isolation)
- **Database:** SQLite (Structural) + LanceDB (Vector)
- **UI:** Ratatui (60FPS Terminal UI) + Leptos (WASM Webview)
- **Protocols:** MCP (Model Context Protocol), Matrix, LSP

For detailed architecture information, see the [Architecture Overview](.docs/architecture_overview.md).

---

## Development

### Prerequisites

- Rust 1.85+
- Cargo
- pnpm (for VSCode extension)

### Building

```bash
# Build all crates
cargo build

# Build specific crate
cargo build -p clawdius

# Build with features
cargo build --features hft-mode
```

### Feature Flags

See the [Feature Flags](#feature-flags) section for detailed information on optional features and build configurations.

Additional development-only features:

| Feature | Description | Dependencies |
|---------|-------------|--------------|
| `hft-mode` | High-frequency trading mode | - |
| `broker-mode` | Financial broker integration | - |
| `keyring` | OS keyring for secure storage | keyring |

```bash
# With high-frequency trading mode
cargo build --features hft-mode

# With keyring support
cargo build --features keyring
```

### Testing

```bash
# Run all tests
cargo test

# Run tests for specific crate
cargo test -p clawdius-core
```

### Linting

```bash
# Run Clippy on all crates
cargo clippy --all-targets --all-features

# Check formatting
cargo fmt --check
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for detailed development guidelines.

---

## Documentation

- [User Guide](.docs/user_guide.md) - Installation, configuration, and commands
- [Architecture Overview](.docs/architecture_overview.md) - System design and components
- [API Reference](.docs/api_reference.md) - Core library API documentation
- [CLI Reference](crates/clawdius/README.md) - CLI-specific documentation
- [Core Library](crates/clawdius-core/README.md) - Library API and features
- [VSCode Extension](editors/vscode/README.md) - Extension setup and usage

---

## License

Clawdius is released under the Apache 2.0 License. See [LICENSE](LICENSE) for details.

---

## Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for:
- Development setup
- Code style guidelines
- PR process
- Testing requirements

---

## API Stability & Deprecation Policy

Clawdius follows [Semantic Versioning 2.0](https://semver.org/). 

### Versioning

```
MAJOR.MINOR.PATCH

MAJOR: Breaking API changes
MINOR: New features (backward-compatible)
PATCH: Bug fixes (backward-compatible)
```

### Deprecation Timeline

| Phase | Duration | What Happens |
|-------|----------|--------------|
| Announcement | Immediate | `#[deprecated]` attribute added |
| Warning Period | 2 minor releases | API works but emits compiler warning |
| Removal | Next major release | API removed |

### Example

```rust
// v1.2.0 - Deprecated
#[deprecated(since = "1.2.0", note = "Use new_method()")]
pub fn old_method() { ... }

// v2.0.0 - Removed
// old_method() no longer exists
```

For full details, see [API Stability Guarantee](docs/API_STABILITY.md).

---

## Community

Clawdius is built by and for developers who value security, performance, and rigor.

### Get Help
- **Documentation:** [docs.clawdius.dev](https://docs.clawdius.dev)
- **GitHub Discussions:** [github.com/clawdius/clawdius/discussions](https://github.com/clawdius/clawdius/discussions)
- **Discord:** [discord.gg/clawdius](https://discord.gg/clawdius)
- **Issues:** [github.com/clawdius/clawdius/issues](https://github.com/clawdius/clawdius/issues)

### Contribute
- See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines
- Check [Good First Issues](https://github.com/clawdius/clawdius/labels/good%20first%20issue)

### Stay Updated
- Watch releases on GitHub
- Follow [@clawdius_dev](https://twitter.com/clawdius_dev) on Twitter/X

---

> **"Clawdius: Build like an Emperor. Protect like a Sentinel."**
