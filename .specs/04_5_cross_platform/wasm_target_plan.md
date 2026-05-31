# WASM Target Compilation Plan

> Groundwork for v1.0.0-rc.4 / v1.1.0 WASM browser-based agent support.
> Status: PLANNING | Last updated: 2026-05-30

## 1. Current State

`clawdius-core` does NOT compile to `wasm32-unknown-unknown`. Full dependency and source audit completed.

## 2. Dependency Blockers

| Dependency | Issue | Resolution |
|------------|-------|------------|
| `getrandom` (v0.2 + v0.3) | No WASM support without `js` or `custom` feature | Add `features = ["js"]` for WASM target |
| `tokio` | `net`, `fs`, `process`, `signal` modules | Gate behind `cfg(not(wasm32))`; use `tokio` with `rt`, `sync`, `macros` only |
| `rusqlite` | C library, no WASM build | Replace with WASM SQLite (sql.js) or remote DB via IPC |
| `reqwest` | Needs WASM-compatible HTTP | Switch to `rustls-tls`; use `web-sys` fetch for browser |
| `tree-sitter` (+ 8 grammars) | C library | Use `tree-sitter/wasm` builds |
| `tiktoken-rs` | Links C/C++ tokenizer | Use pure-Rust tokenizer or WASM build |
| `wasmtime` + `wasmtime-wasi` | Cannot embed WASM runtime inside WASM | Gate entirely behind `cfg(not(wasm32))` |
| `notify` | OS file-watching APIs | Gate behind `cfg(not(wasm32))` |
| `tempfile` | Uses `std::fs` | Gate or replace |
| `sysinfo` | OS system info | Gate behind `cfg(not(wasm32))` |
| `which` | Filesystem executable lookup | Gate behind `cfg(not(wasm32))` |

## 3. Source-Level Blocking Modules

### Tier 1 -- Exclude Wholesale (cfg gate entire subtree)
- `crates/clawdius-core/src/sandbox/` -- process spawning, filesystem sandboxing, wasmtime embedding, firewall
- `crates/clawdius-core/src/tools/shell.rs` -- command execution
- `crates/clawdius-core/src/tools/editor.rs` -- filesystem editing
- `crates/clawdius-core/src/lsp/client.rs` -- TCP client
- `crates/clawdius-core/src/proof/verifier.rs` -- process-based verification
- `crates/clawdius-core/src/mcp/sandboxed_executor.rs` -- filesystem + process

### Tier 2 -- Need WASM-compatible Replacements
- `config.rs`, `encryption.rs`, `i18n.rs` -- `std::fs` for file I/O; needs JS IndexedDB/fetch or WASI
- `session/store.rs`, `timeline/store.rs`, `memory/mod.rs` -- SQLite persistence; needs WASM SQLite or remote DB
- `workspace/context.rs`, `workspace/manager.rs` -- filesystem workspace access
- `context/aggregator.rs`, `graph_rag/` -- filesystem traversal

### Tier 3 -- Feature-Flag in Cargo.toml
- `getrandom`, `tokio`, `reqwest`, `wasmtime`, `rusqlite`, `tree-sitter`, `tiktoken-rs`, `notify`, `sysinfo`, `which`, `tempfile`

## 4. Phased Approach

### Phase A: Feature Flag Infrastructure
1. Add `#[cfg(target_arch = "wasm32")]` gates to Tier 1 modules
2. Add `[target.'cfg(target_arch = "wasm32")'.dependencies]` in Cargo.toml
3. Create `crates/clawdius-core/src/wasm_compat/` for browser abstractions

### Phase B: Minimal WASM Build
1. Compile a minimal `clawdius-core` subset (LLM API, tokenization, session in-memory)
2. Replace `getrandom` with `js` feature
3. Replace `rusqlite` with in-memory or IndexedDB backend
4. Verify `wasm-pack build --target web` succeeds

### Phase C: Browser Integration
1. Create `crates/clawdius-wasm` with `wasm-bindgen` exports
2. Implement JS interop for LLM streaming, session management
3. Build and test in browser environment

## 5. Estimated Effort

| Phase | Scope | Estimated Time |
|-------|-------|----------------|
| Phase A | Feature flags + cfg gates | 2-3 days |
| Phase B | Minimal WASM build | 3-5 days |
| Phase C | Browser integration | 5-7 days |

## 6. Dependencies

- `wasm-bindgen`, `wasm-pack`, `js-sys`, `web-sys`
- `sql.js` (WASM SQLite) or remote DB via HTTP
- Pure-Rust tokenizer alternative to `tiktoken-rs`
