# Clawdius

**The High-Assurance AI Engineering Engine.**
*Native Rust. Formal Proofs. Multi-Platform Gateway.*

[![Version](https://img.shields.io/badge/version-1.0.0--rc.1-blue.svg)](https://github.com/WyattAu/clawdius/releases)
[![Rust](https://img.shields.io/badge/language-Rust-orange.svg)](https://www.rust-lang.org)
[![Tests](https://img.shields.io/badge/tests-1%2C240-passing-brightgreen.svg)]()
[![Clippy](https://img.shields.io/badge/clippy-0%20warnings-success.svg)]()
[![Lean4](https://img.shields.io/badge/Lean4-16%2F16%20proofs-blue.svg)]()
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache_2.0-yellow.svg)](LICENSE)

Clawdius is a next-generation AI coding engine built in Rust. It provides a terminal UI (TUI), CLI, and multi-platform messaging gateway for agentic coding — connecting chat platforms (Telegram, Discord, Slack, Matrix, and more) to a formal-verification-backed code generation engine.

## Highlights

- **1,240 tests, 0 failures** across 5 workspace crates
- **0 clippy warnings** (strictest `-W clippy::all`)
- **16/16 Lean4 formal proofs** compiled and verified
- **12/12 feature flags** compile independently
- **0 unsafe blocks** in production code (8 in SIMD module only)
- **25 MB + 15 MB** release binaries (LTO fat, stripped)

## Features

- **Multi-Provider LLM** — DeepSeek, Anthropic, OpenAI, OpenRouter, Ollama, ZAI with automatic retry
- **Terminal UI** — 60 FPS ratatui TUI with 25+ commands, streaming responses, file watching
- **Agentic Engine** — Sprint execution, auto-fix, code generation with tool use (file, shell, git)
- **Messaging Gateway** — 9 platform adapters (Telegram, Discord, Slack, Matrix, Signal, Teams, WhatsApp, Rocket.Chat, Webhook)
- **Formal Verification** — Lean4 proofs for sandboxing, concurrency, security, and data structures
- **JIT Sandboxing** — Wasmtime WASM sandbox, command blocking, timeout limits, directory restrictions
- **Session Management** — Persistent conversations with auto-compact, per-chat isolation
- **File Watching** — Real-time file monitoring with debounced drift and debt analysis
- **Code Analysis** — Architecture drift detection, technical debt scoring, graph-RAG intelligence

## Workspace Structure

```
clawdius/
├── crates/
│   ├── clawdius/              # TUI + CLI binary (25 MB release)
│   ├── clawdius-core/         # Core library (101K lines)
│   ├── clawdius-gateway/      # Messaging gateway binary (15 MB release)
│   ├── clawdius-code/         # VSCode extension helper (JSON-RPC)
│   └── clawdius-mcp/          # Model Context Protocol server
├── .specs/                    # Yellow Papers, Blue Papers, Lean4 proofs
├── SECURITY.md                # Dependency risk inventory
└── Cargo.toml                 # Workspace configuration
```

## Quick Start

### 1. Install

```bash
# From source (requires Rust 1.93+)
git clone https://github.com/WyattAu/clawdius
cd clawdius
cargo build --release

# Binaries: target/release/clawdius (25 MB)
#           target/release/clawdius-gateway (15 MB)
```

### 2. Configure

```bash
# Set your LLM provider API key
export DEEPSEEK_API_KEY="sk-your-key"
# Or: export ANTHROPIC_API_KEY="sk-ant-..."
# Or: export OPENAI_API_KEY="sk-..."

clawdius --version
```

### 3. Chat (TUI)

```bash
clawdius                    # Launch interactive TUI
clawdius chat "hello"       # Quick message from terminal
```

### 4. Agentic Commands

```bash
clawdius sprint "build a REST API"    # Execute full sprint lifecycle
clawdius auto "fix the tests"         # Auto-fix with tool use
clawdius generate "add logging" -o .  # Generate code into directory
clawdius analyze                      # Run drift + debt analysis
```

### 5. Messaging Gateway

```bash
# Start gateway with Telegram
export TELEGRAM_BOT_TOKEN="123456:abc..."
clawdius-gateway --platform telegram --provider deepseek

# Health check
curl http://localhost:8081/api/gateway/health
```

## TUI Commands

| Command | Description |
|---------|-------------|
| `:sprint <query>` | Execute sprint lifecycle with LLM |
| `:auto <query>` | Auto-fix with tool use |
| `:generate <query>` | Generate code into directory |
| `:build` | Run `cargo build` |
| `:test` | Run `cargo test` |
| `:doc` | Generate documentation |
| `:verify` | Run Lean4 proof verification |
| `:checkpoint` | Create file checkpoint |
| `:timeline` | View file timeline |
| `:memory` | Manage CLAWDIUS.md |
| `:analyze` | Run drift + debt analysis |
| `:config show` | Display current config |
| `:watch` | Toggle file watcher |
| `:sessions` | List/manage sessions |
| `:workspace` | Switch workspace |
| `:quit` | Exit TUI |

## CLI Commands

```bash
clawdius chat "message"           # Send message to LLM
clawdius sprint "task"            # Sprint execution
clawdius auto "task"              # Auto-fix
clawdius generate "task" -o dir   # Generate code
clawdius analyze                  # Code analysis
clawdius test                     # Run tests
clawdius doc                      # Generate docs
clawdius verify                   # Lean4 verification
clawdius timeline                 # File timeline
clawdius memory                   # Project memory
clawdius sessions                 # Session management
clawdius watch                    # File watching
clawdius index                    # Build code index
```

## Feature Flags

| Feature | Description | Default |
|---------|-------------|---------|
| `keyring` | OS keyring for API keys | On |
| `crash-reporting` | Sentry crash reporting | Off |
| `browser` | Browser automation | Off |
| `embeddings` | ML embeddings (candle) | Off |
| `local-llm` | Local LLM support | Off |
| `vector-db` | Vector database (lancedb) | Off |
| `orchestrator` | Multi-agent orchestration | Off |
| `redis-queue` | Redis job queue | Off |
| `postgres` | PostgreSQL storage | Off |
| `mariadb` | MariaDB storage | Off |
| `stripe` | Stripe billing | Off |
| `telegram` | Telegram gateway adapter | Off |
| `discord` | Discord gateway adapter | Off |
| `slack` | Slack gateway adapter | Off |
| `matrix` | Matrix gateway adapter | Off |

```bash
# Minimal build
cargo build --release

# With gateway platforms
cargo build --release --features telegram,discord

# Full build
cargo build --release --features "keyring,embeddings,vector-db,postgres"
```

### Binary Size Comparison

| Configuration | Dependencies | Binary Size |
|--------------|--------------|-------------|
| Minimal | ~350 | clawdius 25MB, clawdius-gateway 15MB |
| +embeddings | ~400 | ~35MB |
| +vector-db | ~450 | ~30MB |
| Full | ~696 | ~40MB |

---

## Quick Start

### 1. Set Up (New in v1.2.0!)
```bash
# Interactive setup wizard (recommended)
clawdius setup

# Or quick setup with pre-selected provider
clawdius setup --quick --provider anthropic
```

The wizard will:
- Guide you through provider selection (Anthropic, OpenAI, Ollama, Zhipu AI)
- Securely store your API key using system keyring
- Apply a settings preset (Balanced, Security, Performance, Development)
- Verify connectivity for local LLMs (Ollama)

### 2. Start Chatting

```bash
clawdius chat
```

### 3. Generate Code

```bash
clawdius generate --mode agent "Create a REST API endpoint"
```

## Configuration

```toml
# ~/.clawdius/config.toml

[llm]
default_provider = "deepseek"
max_tokens = 4096

[llm.deepseek]
model = "deepseek-chat"

[llm.retry]
max_retries = 3
retry_on = ["rate_limit", "timeout", "server_error"]

[shell_sandbox]
timeout_secs = 120
restrict_to_cwd = true
blocked_commands = ["rm -rf /", "mkfs", ":(){ :|:& };:"]
```

## Messaging Gateway

Connect Clawdius to chat platforms for remote coding:

```bash
# Telegram
clawdius-gateway --platform telegram --provider deepseek

# Multiple platforms
clawdius-gateway -p telegram -p discord -p slack

# Admin API on :8081, webhook on :8080
clawdius-gateway -p telegram --port 8080 --admin-port 8081
```

| Platform | Polling | Edit | Auth |
|----------|---------|------|------|
| Telegram | Long-poll | Yes | Bot API token |
| Discord | Gateway | Yes | Bot token |
| Slack | Socket Mode | Yes | Bot token + app token |
| Matrix | Sync | Yes | Homeserver + token |
| Signal | REST poll | No | Account number |
| Teams | REST | No | App ID + password |
| WhatsApp | Cloud API | Yes | Access token |
| Rocket.Chat | REST | Yes | Auth token |
| Webhook | HTTP POST | No | URL + secret |

## Development

```bash
# Run all tests (1,240 tests)
cargo test --workspace

# Clippy (0 warnings)
cargo clippy --workspace -- -W clippy::all

# Security audit
cargo audit

# Lean4 proofs (16 proofs)
lean .specs/02_architecture/proofs/proof_*.lean

# Check specific feature flag
cargo check -p clawdius-core --features postgres
```

## Quality Metrics

| Metric | Value |
|--------|-------|
| Test suite | 1,240 tests, 0 failures |
| Clippy warnings | 0 code warnings |
| Lean4 proofs | 16/16 verified |
| Feature flags | 12/12 compile |
| Unsafe blocks | 0 in production |
| Codebase | 344 files, 132,861 lines |
| Release binaries | 25 MB + 15 MB |

## License

Apache 2.0 — see [LICENSE](LICENSE) for details.

---

> "Build like an Engineer. Verify like a Mathematician. Deploy like an Operator."
