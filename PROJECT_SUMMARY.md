# Clawdius Project Summary

> **Last Updated:** March 2026  
> **Status:** Active Development - Monorepo Restructuring  
> **Version:** 0.2.0

---

## Executive Summary

**Clawdius** is a high-assurance Rust-native AI coding assistant being built to compete with tools like Cline, Roo Code, Claude Code, and Gemini CLI. It features unique security capabilities (Sentinel JIT sandboxing), formal verification support (Lean4), and a hybrid architecture with a TypeScript VSCode extension communicating to a Rust backend via JSON-RPC.

The current focus is restructuring the project as a monorepo with four crates and getting the build to pass.

---

## Project Goals

1. **Compare Clawdius** against competitors (Cline, Roo Code, Claude Code, Gemini CLI, OpenCode)
2. **Identify missing features** and implement them to achieve feature parity
3. **Build VSCode extension** with Rust backend (two binaries: CLI and VSCode helper)
4. **Restructure as monorepo** with shared releases

---

## Architecture Overview

### Monorepo Structure

```
clawdius/
├── Cargo.toml                 # Workspace root
├── crates/
│   ├── clawdius/             # CLI binary (standalone terminal app)
│   ├── clawdius-core/       # Shared library (all core logic)
│   ├── clawdius-code/        # VSCode helper binary (JSON-RPC server)
│   └── clawdius-webview/    # Leptos WASM webview UI
├── editors/
│   └── vscode/              # VSCode extension (TypeScript)
└── .docs/                   # Documentation
```

### Communication Protocol

- **VSCode ↔ Rust:** JSON-RPC over stdio
- **Webview:** Leptos compiled to WASM

### Technology Stack

| Component | Technology |
|-----------|------------|
| Language | Rust (2024 edition) |
| Async Runtime | Tokio |
| Database | SQLite (rusqlite) |
| Terminal UI | Ratatui |
| Webview UI | Leptos (WASM) |
| VSCode | TypeScript |
| LLM Integration | OpenAI, Anthropic, Ollama, DeepSeek, ZAI |

---

## Current Status

### ✅ Completed

| Component | Status | Details |
|-----------|--------|---------|
| Feature Gap Analysis | ✅ Complete | Identified 10 missing feature categories |
| Monorepo Structure | ✅ Complete | 4 crates configured |
| Core Library Modules | ✅ Complete | 15+ modules implemented |
| CLI Binary | ✅ Complete | Basic CLI with TUI scaffolding |
| VSCode Helper Binary | ✅ Complete | JSON-RPC server skeleton |
| VSCode Extension | ⚠️ Skeleton | TypeScript files exist, needs wiring |
| Session System | ✅ Complete | SQLite-backed persistence |
| Context/@Mentions | ✅ Complete | Mention parser and builder |
| Output System | ✅ Complete | JSON/stream output |
| RPC Protocol | ✅ Complete | Server, handlers, types |
| Tools System | ✅ Complete | File, Shell, Git, Browser tools |
| Checkpoint System | ✅ Complete | Snapshot and diff |
| Commands System | ✅ Complete | Parser, templates, executor |
| Agent Modes | ✅ Complete | Mode definitions |
| Sandbox System | ✅ Complete | Tier definitions and executor |
| Graph-RAG | ✅ Complete | AST and vector search |
| i18n | ✅ Complete | Localization framework |
| LLM Integration | ✅ Complete | Providers and message handling |

### ⚠️ In Progress

| Component | Status | Issue |
|-----------|--------|-------|
| Leptos Webview | ❌ Blocked | API errors: `mount_to_body` and `child` not found |
| Build Compilation | ❌ Blocked | Webview crate fails to compile |
| VSCode Extension Wiring | ⚠️ Pending | Extension needs to spawn `clawdius-code` binary |

---

## Key Files

### Crates

| File | Purpose |
|------|---------|
| `crates/clawdius-core/src/lib.rs` | Core library exports |
| `crates/clawdius-core/src/session/` | Session persistence (SQLite) |
| `crates/clawdius-core/src/context/` | @Mentions system |
| `crates/clawdius-core/src/output/` | JSON/stream output |
| `crates/clawdius-core/src/rpc/` | JSON-RPC protocol |
| `crates/clawdius-core/src/tools/` | Tool definitions (file, shell, git, browser) |
| `crates/clawdius-core/src/checkpoint/` | Checkpoint system |
| `crates/clawdius-core/src/commands/` | Custom commands |
| `crates/clawdius-core/src/modes.rs` | Agent modes |
| `crates/clawdius-core/src/sandbox.rs` | Sandbox tiers |
| `crates/clawdius-core/src/graph_rag.rs` | Knowledge layer |
| `crates/clawdius-core/src/i18n.rs` | Localization |
| `crates/clawdius-core/src/llm.rs` | LLM integration |
| `crates/clawdius/src/main.rs` | CLI entry point |
| `crates/clawdius/src/cli.rs` | CLI commands |
| `crates/clawdius/src/tui_app/` | Terminal UI |
| `crates/clawdius-code/src/main.rs` | VSCode helper (JSON-RPC server) |
| `crates/clawdius-webview/src/lib.rs` | Leptos WASM UI (has errors) |

### VSCode Extension

| File | Purpose |
|------|---------|
| `editors/vscode/package.json` | Extension config |
| `editors/vscode/src/extension.ts` | Main extension entry |
| `editors/vscode/src/rpc/client.ts` | JSON-RPC client |
| `editors/vscode/src/providers/chatView.ts` | Chat panel provider |
| `editors/vscode/src/providers/statusBar.ts` | Status bar provider |

### Documentation

| File | Purpose |
|------|---------|
| `.docs/feature_gap_analysis.md` | Competitor comparison |
| `.docs/implementation_roadmap.md` | Feature roadmap |
| `.docs/architecture_overview.md` | System architecture |
| `.docs/user_guide.md` | User documentation |
| `.docs/api_reference.md` | API reference |

---

## Feature Gap Analysis Summary

### Priority Features (P0)

| Feature | Status | Notes |
|---------|--------|-------|
| VSCode Extension | ⚠️ In Progress | Skeleton exists |
| Browser Automation | ⚠️ Tool exists | Needs integration |
| Session Persistence | ✅ Complete | SQLite-backed |
| @Mentions | ✅ Complete | Parser implemented |
| JSON Output | ✅ Complete | Output module |
| Auto-Compact | ✅ Complete | Session compactor |

### Secondary Features (P1)

| Feature | Status | Notes |
|---------|--------|-------|
| Diff View | ❌ Missing | Show changes before applying |
| Checkpoints | ✅ Complete | Snapshot/restore |
| Custom Commands | ✅ Complete | Parser + executor |
| External Editor | ❌ Missing | Open $EDITOR |
| GitHub Action | ❌ Missing | CI/CD integration |

### Future Features (P2)

| Feature | Status | Notes |
|---------|--------|-------|
| Agent Modes | ✅ Complete | Mode definitions |
| Web Search | ❌ Missing | Ground responses |
| Vim Keybindings | ❌ Missing | Modal editing |
| Localization | ✅ Complete | i18n framework |

---

## Build Status

### Current Error

The `clawdius-webview` crate fails with:

```
error[E0425]: cannot find function `mount_to_body` in this scope
error[E0599]: no method named `child` found for struct `leptos::html::HtmlElement`
```

**Location:** `crates/clawdius-webview/src/lib.rs`

### Root Cause

Leptos 0.7 API changes - the `csr` feature uses different APIs than server-side rendering.

### Workspace Dependencies Resolved

During setup, these dependency issues were fixed:
- ❌ `jsonrpsee` doesn't have `stdio` feature (removed, using custom implementation)
- ❌ `mimalloc` can't be optional (made required in workspace)
- ❌ Profile `lto` can't be package-specific (removed)
- ❌ `leptos_meta` doesn't have `csr` feature (needs fix)

---

## Next Steps (For Continuation)

### Immediate (Fix Build)

1. **Fix Leptos Webview** - Update `crates/clawdius-webview/src/lib.rs` to use correct Leptos 0.7 CSR APIs
2. **Run `cargo check --workspace`** - Verify all crates compile
3. **Test CLI binary** - Ensure basic functionality works

### VSCode Integration

4. **Wire Extension to Binary** - Make VSCode extension spawn `clawdius-code` process
5. **Implement RPC Methods** - Full JSON-RPC method implementation
6. **Create Chat Panel** - Webview UI for chatting

### CI/CD

7. **Create GitHub Action** - Build and release workflow
8. **Add Tests** - Unit and integration tests

### Feature Implementation

9. **Browser Automation** - Integrate `headless_chrome` tool
10. **Full TUI** - Complete ratatui implementation
11. **GitHub Integration** - Create GitHub Action for code review

---

## Configuration

### Workspace (Cargo.toml)

```toml
[workspace]
resolver = "2"
members = [
    "crates/clawdius",
    "crates/clawdius-core",
    "crates/clawdius-code",
    "crates/clawdius-webview",
]

[workspace.package]
version = "0.2.0"
edition = "2024"
rust-version = "1.85"
```

### VSCode Extension (package.json)

```json
{
    "name": "clawdius-code",
    "version": "0.2.0",
    "engines": { "vscode": "^1.85.0" },
    "categories": ["Programming Languages", "Other"],
    "extensionKind": ["workspace"]
}
```

---

## Useful Commands

```bash
# Check workspace builds
cargo check --workspace

# Build specific crate
cargo build -p clawdius
cargo build -p clawdius-code

# Run CLI
cargo run -p clawdius -- --help

# VSCode extension development
cd editors/vscode
npm install
npm run compile
```

---

## References

- [Feature Gap Analysis](./.docs/feature_gap_analysis.md)
- [Implementation Roadmap](./.docs/implementation_roadmap.md)
- [Architecture Overview](./.docs/architecture_overview.md)
- [Workspace Cargo.toml](./Cargo.toml)
- [VSCode package.json](./editors/vscode/package.json)

---

## Contact

For questions or contributions, please refer to the project repository.
