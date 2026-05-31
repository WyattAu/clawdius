# WASM Target Compilation Plan

> Groundwork for v1.0.0-rc.4 / v1.1.0 WASM browser-based agent support.
> Status: PHASE A COMPLETE | Last updated: 2026-05-31

## 1. Current State

`clawdius-core` compiles to `wasm32-unknown-unknown` (Phase A complete).
`cargo check -p clawdius-core --target wasm32-unknown-unknown` passes with zero errors.

### Error Count

| Metric | Before | After |
|--------|--------|-------|
| Blocking errors | 1 (getrandom compile_error) | 0 |
| Failing crates | 1 (getrandom) | 0 |

Note: The initial run revealed only `getrandom` as a hard compile_error. Gating it
revealed `mio` (via tokio net features), which required gating 18 additional deps.

## 2. Dependency Blockers -- Resolution Status

### Fixed (Phase A -- cfg-gated behind `not(target_arch = "wasm32")`)

| Dependency | Issue | Resolution |
|------------|-------|------------|
| `getrandom` v0.2 | No WASM support without `js` feature | `[target.'cfg(target_arch = "wasm32")'.dependencies]` with `features = ["js"]` |
| `tokio` | `net`, `fs`, `process`, `signal` pull `mio` | Gated behind `cfg(not(wasm32))` -- WASM gets no tokio runtime (will need `wasm-bindgen` futures) |
| `reqwest` | Needs tokio `net` features | Gated behind `cfg(not(wasm32))` -- WASM needs `web-sys` fetch or `reqwest` with `wasm-bindgen` |
| `genai` | Depends on tokio with net features | Gated behind `cfg(not(wasm32))` |
| `axum` | HTTP server needs tokio net | Gated behind `cfg(not(wasm32))` -- browser has no HTTP server |
| `tower` / `tower-http` | HTTP middleware needs tokio net | Gated behind `cfg(not(wasm32))` |
| `rusqlite` | C library, no WASM build | Gated behind `cfg(not(wasm32))` -- needs sql.js or remote DB |
| `tree-sitter` (+ 8 grammars) | C library | Gated behind `cfg(not(wasm32))` -- needs tree-sitter/wasm |
| `tiktoken-rs` | Links C/C++ tokenizer | Gated behind `cfg(not(wasm32))` -- needs pure-Rust or WASM build |
| `wasmtime` + `wasmtime-wasi` | Cannot embed WASM runtime inside WASM | Gated behind `cfg(not(wasm32))` |
| `notify` | OS file-watching APIs | Gated behind `cfg(not(wasm32))` |
| `tempfile` | Uses `std::fs` | Gated behind `cfg(not(wasm32))` |
| `sysinfo` | OS system info | Gated behind `cfg(not(wasm32))` |
| `which` | Filesystem executable lookup | Gated behind `cfg(not(wasm32))` |
| `walkdir` / `glob-match` | Filesystem traversal | Gated behind `cfg(not(wasm32))` |
| `tracing-subscriber` / `tracing-appender` | File I/O for logging | Gated behind `cfg(not(wasm32))` |
| `tokio-stream` | Depends on tokio net features | Gated behind `cfg(not(wasm32))` |

### Remaining (require WASM-compatible replacements)

| Dependency | Needed For | WASM Replacement |
|------------|------------|------------------|
| `tokio` (runtime) | Async runtime | `wasm-bindgen-futures` for browser |
| `reqwest` | LLM API calls | `web-sys` fetch or `reqwest` wasm backend |
| `genai` | LLM abstraction | Browser fetch-based adapter |
| `rusqlite` | Persistence | sql.js (WASM SQLite) or IndexedDB |
| `tree-sitter` | Code parsing | tree-sitter/wasm builds |
| `tiktoken-rs` | Token counting | Pure-Rust tokenizer |

### Dependencies that work on WASM (no changes needed)

`serde`, `serde_json`, `toml`, `uuid`, `thiserror`, `anyhow`, `tracing`, `chrono`,
`regex`, `aho-corasick`, `petgraph`, `lru`, `url`, `parking_lot`, `similar`,
`base64`, `hex`, `urlencoding`, `aes-gcm`, `hmac`, `sha2`, `sha3`, `futures`,
`async-trait`, `async-stream`

## 3. Source-Level Blocking Modules

### Tier 1 -- Exclude Wholesale (cfg gate entire subtree)
- `crates/clawdius-core/src/sandbox.rs` -- process spawning, filesystem sandboxing, wasmtime embedding
- `crates/clawdius-core/src/tools.rs` -- command execution, filesystem tools
- `crates/clawdius-core/src/llm.rs` -- LLM integration (needs reqwest)
- `crates/clawdius-core/src/session.rs` -- session persistence (needs rusqlite)
- `crates/clawdius-core/src/config.rs` -- filesystem config I/O
- `crates/clawdius-core/src/encryption.rs` -- filesystem key management
- `crates/clawdius-core/src/i18n.rs` -- filesystem i18n loading
- `crates/clawdius-core/src/storage/` -- all database backends
- `crates/clawdius-core/src/agentic/` -- agent system (uses tokio, tools, LLM)
- `crates/clawdius-core/src/api/` -- HTTP server (axum)
- `crates/clawdius-core/src/orchestrator/` -- uses tokio, wasmtime
- `crates/clawdius-core/src/audit/` -- filesystem logging
- `crates/clawdius-core/src/analysis/` -- may use tree-sitter

Note: Currently `lib.rs` only exports a `greet()` function, so no source-level
cfg gates are needed yet. Source-level gates will be required as modules are
added back to the public API.

## 4. Phased Approach

### Phase A: Feature Flag Infrastructure (DONE)
1. Add `[target.'cfg(target_arch = "wasm32")'.dependencies]` in Cargo.toml with getrandom js feature
2. Gate 18 WASM-incompatible deps behind `[target.'cfg(not(target_arch = "wasm32"))'.dependencies]`
3. Add workspace-level `getrandom-wasm` alias with js feature

### Phase B: Minimal WASM Build
1. Replace tokio runtime with `wasm-bindgen-futures`
2. Replace reqwest with `web-sys` fetch for LLM API calls
3. Replace rusqlite with in-memory store or IndexedDB
4. Replace tree-sitter with WASM builds
5. Replace tiktoken-rs with pure-Rust tokenizer
6. Verify `wasm-pack build --target web` succeeds

### Phase C: Browser Integration
1. Create `crates/clawdius-wasm` with `wasm-bindgen` exports
2. Implement JS interop for LLM streaming, session management
3. Build and test in browser environment

## 5. Estimated Effort

| Phase | Scope | Status |
|-------|-------|--------|
| Phase A | Feature flags + cfg gates | DONE |
| Phase B | Minimal WASM build | TODO |
| Phase C | Browser integration | TODO |

## 6. Other Workspace Crates

The following workspace crates still fail WASM compilation due to tokio net (mio):
`clawdius`, `clawdius-gateway`, `clawdius-code`, `clawdius-mcp`.
Only `clawdius-plugin-sdk` has no tokio dependency.

These are binary/server crates and are not expected to compile to WASM.
Fix them if WASM CLI or gateway is ever needed.

## 7. Dependencies

- `wasm-bindgen`, `wasm-pack`, `js-sys`, `web-sys`
- `sql.js` (WASM SQLite) or remote DB via HTTP
- Pure-Rust tokenizer alternative to `tiktoken-rs`
