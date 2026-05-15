# Architecture Overview

Clawdius is a Rust monorepo with multiple crates, organized around a zero-trust security model and a 24-phase R&D lifecycle engine.

## Monorepo Structure

```
clawdius/
├── crates/
│   ├── clawdius/              # CLI binary
│   ├── clawdius-core/         # Core library
│   ├── clawdius-code/         # VSCode extension helper
│   ├── clawdius-gateway/      # HTTP gateway
│   └── clawdius-mcp/          # Model Context Protocol
├── editors/vscode/            # VSCode extension (TypeScript)
├── .docs/                     # Internal documentation
└── Cargo.toml                 # Workspace root
```

## Crate Responsibilities

| Crate | Type | Purpose |
|-------|------|---------|
| `clawdius` | Binary | CLI, TUI (ratatui), command orchestration |
| `clawdius-core` | Library | LLM, sessions, tools, sandboxing, Graph-RAG |
| `clawdius-code` | Binary | JSON-RPC server for VSCode extension |
| `clawdius-gateway` | Binary | HTTP API server |
| `clawdius-mcp` | Binary | Model Context Protocol server |

### Dependency Graph

```
clawdius (CLI) ──────────┐
                         ├──> clawdius-core
clawdius-code (VSCode) ──┘         │
                                  (external deps)
clawdius-gateway ──────────────────┘
```

`clawdius-core` is the single shared library. All binaries depend on it, and it depends only on external crates.

## System Architecture

```
┌───────────────────────────────────────────────────────────┐
│                       Clawdius CLI                        │
│  ┌─────────────┐ ┌─────────────┐ ┌────────────────────┐  │
│  │ Session Mgr │ │ Context     │ │ Timeline           │  │
│  │             │ │ Builder     │ │ & Checkpoints      │  │
│  └─────────────┘ └─────────────┘ └────────────────────┘  │
│  ┌─────────────┐ ┌─────────────┐ ┌────────────────────┐  │
│  │ LLM         │ │ Graph-RAG   │ │ Plugin System      │  │
│  │ Providers   │ │             │ │                    │  │
│  └─────────────┘ └─────────────┘ └────────────────────┘  │
│  ┌─────────────┐ ┌─────────────┐ ┌────────────────────┐  │
│  │ Sandbox     │ │ Tool Runner │ │ Enterprise Features │  │
│  │ Executors   │ │             │ │                    │  │
│  └─────────────┘ └─────────────┘ └────────────────────┘  │
└───────────────────────────────────────────────────────────┘
```

## Core Design Principles

| Principle | Description |
|-----------|-------------|
| **Zero Trust** | All external code runs in sandboxes |
| **Typestate Safety** | Invalid states unrepresentable at compile time |
| **Defense in Depth** | Multiple isolation layers |
| **Local First** | Full functionality without internet |

## Nexus FSM

The Nexus FSM is a 24-phase state machine that enforces structured development through quality gates. Each phase must pass its quality gates before transitioning to the next.

Phases include Context Discovery, Requirements Engineering, Security Engineering, Performance Engineering, CI/CD Engineering, and more. See the [sessions](./sessions.md) page for lifecycle details.

## Cross-Crate Communication

### CLI to Core

Direct Rust function calls:

```rust
use clawdius_core::{Config, SessionManager};

let config = Config::load_default()?;
let manager = SessionManager::new(&config)?;
```

### VSCode to Core

JSON-RPC over stdio via `clawdius-code`:

```
VSCode Extension (TypeScript)
    │ JSON-RPC (stdio)
    ▼
clawdius-code Binary
    │ Direct calls
    ▼
clawdius-core Library
```

## Performance

| Component | Target |
|-----------|--------|
| Cold boot | < 20ms |
| FSM transition | < 1ms |
| Graph-RAG query | < 50ms |

The release profile uses fat LTO, single codegen unit, and symbol stripping for optimal binary size and performance.

## Feature Flags

Feature flags propagate across crates. Key flags:

| Feature | Description |
|---------|-------------|
| `keyring` | System keyring for API keys |
| `vector-db` | LanceDB vector search (Graph-RAG) |
| `embeddings` | Local ML embeddings |
| `local-llm` | Local LLM inference with Candle |
| `browser` | Browser automation with chromiumoxide |
| `postgres` | PostgreSQL session storage |
| `redis-queue` | Redis-backed orchestrator queue |
| `crash-reporting` | Sentry crash reporting |
