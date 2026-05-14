# Dependency Audit — Clawdius Workspace

**Date:** 2026-05-13
**Workspace version:** 1.0.0-rc.1
**Crates:** clawdius, clawdius-core, clawdius-gateway, clawdius-code, clawdius-mcp

---

## Executive Summary

| Metric | Value |
|---|---|
| Total unique dependencies (transitive) | 497 |
| Total duplicate dependency names | 31 |
| Total duplicate version pairs | 39 |
| Direct workspace dependencies | 91 |
| Semver-compatible duplicates | 0 |
| Semver-incompatible duplicates | 31 |
| Critical findings | 3 |

### Critical Findings

1. **reqwest dual-version (0.12 + 0.13):** `genai 0.5` pulls `reqwest 0.13` while the workspace pins `0.12`. This doubles the HTTP stack (hyper 0.14 + 1.x, tower 0.4 + 0.5, socket2 0.5 + 0.6, etc.) — the single largest source of duplication.
2. **Wasmtime ecosystem bloat:** `wasmtime 44` pulls ~50 crates including full Cranelift JIT. Only used for WASI sandboxing. Consider `wasmtime-wasi` component-only builds or a lighter sandbox.
3. **httpmock 0.7 pulls async-std:** The test-only dependency `httpmock` drags in `async-std` and `hyper 0.14`, creating secondary duplication chains (async-channel, event-listener, socket2).

---

## Duplicate Dependencies

### Direct/High-Impact Duplicates

| Dependency | Versions | Who Uses Which | Semver Compatible | Actionable |
|---|---|---|---|---|
| `reqwest` | 0.12.28, 0.13.3 | Workspace (0.12), `genai` (0.13) | No (major bump) | Yes — see recommendations |
| `tower` | 0.4.13, 0.5.3 | `jsonrpsee` (0.4), workspace (0.5) | No (major bump) | Yes — jsonrpsee upstream |
| `tower-http` | 0.6.8 | Unified | — | OK |
| `hyper` | 0.14.32, 1.9.0 | `httpmock` (0.14), workspace (1.x) | No (major bump) | Yes — httpmock upgrade |
| `http` | 0.2.12, 1.4.0 | `httpmock`/hyper 0.14 (0.2), workspace (1.x) | No (major bump) | Indirect — follows hyper |
| `http-body` | 0.4.6, 1.0.1 | `httpmock`/hyper 0.14 (0.4), workspace (1.x) | No (major bump) | Indirect — follows hyper |
| `thiserror` | 1.0.69, 2.0.18 | `httpmock`/jsonrpsee (1.0), workspace (2.0) | No (major bump) | Yes — httpmock/jsonrpsee upgrade |
| `base64` | 0.21.7, 0.22.1 | `httpmock`/tiktoken-rs (0.21), workspace (0.22) | No (major bump) | Low — test-only + minor |
| `rustls-platform-verifier` | 0.5.3, 0.7.0 | `jsonrpsee` (0.5), `reqwest` 0.13 (0.7) | No (major bump) | Indirect — follows reqwest |
| `getrandom` | 0.2.17, 0.3.4, 0.4.2 | Workspace/rand 0.8 (0.2), rand 0.9 (0.3), uuid (0.4) | No | Low — trait difference |
| `rand` | 0.8.6, 0.9.4 | `jsonrpsee`/criterion (0.8), `proptest` (0.9) | No (major bump) | Low — test-only |
| `toml` | 0.9.12, 1.1.2 | Wasmtime (0.9), workspace (1.x) | No (major bump) | No — wasmtime internal |

### Transitive/Low-Impact Duplicates

| Dependency | Versions | Source | Semver Compatible | Notes |
|---|---|---|---|---|
| `hashbrown` | 0.16.1, 0.17.0 | wasmtime (0.16), indexmap (0.17) | No | Internal hashmap impl |
| `petgraph` | 0.6.5, 0.7.1 | wasmtime (0.6), workspace (0.7) | No | wasmtime internal |
| `itertools` | 0.10.5, 0.11.0, 0.14.0 | criterion (0.10), lalrpop (0.11), wasmtime/ratatui (0.14) | No | 3 versions, all test/transitive |
| `rustix` | 0.38.44, 1.1.4 | wasmtime (0.38), tokio/tempfile (1.x) | No | Major semver split |
| `linux-raw-sys` | 0.4.15, 0.12.1 | rustix 0.38 (0.4), rustix 1.x (0.12) | No | Follows rustix |
| `socket2` | 0.5.10, 0.6.3 | hyper 0.14 (0.5), tokio (0.6) | No | Follows hyper |
| `event-listener` | 2.5.3, 5.4.1 | async-channel 1.x (2.x), async-channel 2.x (5.x) | No | async-std chain |
| `async-channel` | 1.9.0, 2.5.0 | async-std (1.x), async-std internals (2.x) | No | async-std chain |
| `syn` | 1.0.109, 2.0.117 | async-attributes (1.x), workspace (2.x) | No | async-std chain |
| `bitflags` | 2.11.1 (x2) | wasmtime vs crossterm/ratatui | Same version | False positive — feature sets differ |
| `bit-set` | 0.5.3, 0.8.0 | fancy-regex/lalrpop (0.5), proptest (0.8) | No | Test-only |
| `bit-vec` | 0.6.3, 0.8.0 | bit-set 0.5 (0.6), bit-set 0.8 (0.8) | No | Follows bit-set |
| `fixedbitset` | 0.4.2, 0.5.7 | petgraph 0.6 (0.4), petgraph 0.7 (0.5) | No | Follows petgraph |
| `rustc-hash` | 1.1.0, 2.1.2 | tiktoken-rs (1.x), cranelift/jsonrpsee (2.x) | No | Trait impl split |
| `cpufeatures` | 0.2.17, 0.3.0 | aes/sha (0.2), blake3 (0.3) | No | Different APIs |
| `wast` | 35.0.2, 248.0.0 | witx (35.x), wasmtime (248.x) | No | wasmtime internal |
| `wasm-encoder` | 0.246.2, 0.248.0 | wasmtime (246.x), wast (248.x) | No | wasmtime internal |
| `winnow` | 0.7.15, 1.0.2 | toml 0.9 (0.7), toml 1.x (1.x) | No | wasmtime chain |
| `toml_datetime` | 0.7.5, 1.1.1 | toml 0.9 (0.7), toml 1.x (1.1) | No | wasmtime chain |
| `rand_core` | 0.6.4, 0.9.5 | rand 0.8 (0.6), rand 0.9 (0.9) | No | Follows rand |
| `rand_chacha` | 0.3.1, 0.9.0 | rand 0.8 (0.3), rand 0.9 (0.9) | No | Follows rand |
| `thiserror-impl` | 1.0.69, 2.0.18 | Follows thiserror | No | Proc-macro follows thiserror |

---

## Unused Optional Dependencies

Analysis of workspace `[workspace.dependencies]` that are declared but never referenced from any crate's `[dependencies]`:

| Dependency | Status | Notes |
|---|---|---|
| `tokio-postgres` | Used conditionally via `clawdius-core` `postgres` feature | OK |
| `deadpool-postgres` | Used conditionally via `clawdius-core` `postgres` feature | OK |
| `mysql_async` | Used conditionally via `clawdius-core` `mariadb` feature | OK |
| `redis` | Used conditionally via `clawdius-core` `redis-queue` feature | OK |
| `stripe` | Used conditionally via `clawdius-core` `stripe` feature | OK |
| `keyring` | Used in `clawdius` + `clawdius-core` via feature | OK |
| `chromiumoxide` | Used in `clawdius-core` via `browser` feature | OK |
| `sentry` | Used in `clawdius-core` via `crash-reporting` feature | OK |
| `lancedb` / `arrow-*` | Used in `clawdius-core` via `vector-db` feature | OK |
| `candle-*` / `tokenizers` / `hf-hub` | Used in `clawdius-core` via `local-llm` feature | OK |

No unused optional dependencies found. All workspace-declared optionals are referenced by at least one crate.

---

## Recommendations

### 1. Eliminate `reqwest` Dual-Version (HIGH priority)

`genai 0.5` depends on `reqwest 0.13`, but the workspace pins `0.12`. This cascades into:
- 2x hyper (0.14 + 1.x)
- 2x tower (0.4 + 0.5)
- 2x http/http-body
- 2x rustls-platform-verifier (0.5 + 0.7)
- 2x socket2

**Options:**
- (a) **Upgrade workspace reqwest to 0.13** and add `[patch]` for jsonrpsee 0.24 if needed (jsonrpsee 0.24 uses reqwest 0.12). This is the cleanest path if genai is critical.
- (b) **Pin genai's reqwest** via `[patch]` to compile against 0.12 — risky, may break genai.
- (c) **Wait for jsonrpsee 0.25+** which may support reqwest 0.13, then upgrade both.

### 2. Replace or Upgrade `httpmock` (MEDIUM priority)

`httpmock 0.7` is outdated and pulls `hyper 0.14`, `async-std`, and `thiserror 1.x`.

**Options:**
- (a) Replace with `wiremock 0.6` which uses `hyper 1.x` natively.
- (b) Use `axum::test` / `tower::ServiceExt` for in-process HTTP testing (no network stack needed).

### 3. Evaluate Wasmtime Weight (MEDIUM priority)

Wasmtime contributes ~50 crates (Cranelift JIT, Wiggle, Wasi, etc.) for WASI sandboxing.

**Options:**
- (a) **Use `wasmtime-wasi` component model only** — skip Cranelift by using precompiled WASM or the component model.
- (b) **Feature-gate wasmtime** so it's only compiled when the sandbox feature is enabled (it already appears non-optional in clawdius-core — consider making it optional).
- (c) **Consider `wasmer` or `wasm3`** as lighter alternatives if full JIT is not needed.

### 4. Document Acceptable Duplicates (LOW priority)

The following duplicates are purely transitive from wasmtime internals and should be documented as accepted:

- `wast`, `wasm-encoder`, `toml`/`winnow`/`toml_datetime` — wasmtime vs workspace
- `petgraph`, `hashbrown`, `fixedbitset`, `rustc-hash` — wasmtime vs workspace
- `rustix`, `linux-raw-sys` — wasmtime vs tokio

The workspace Cargo.toml already has a partial list at lines 226-245. Update it to include the newly identified duplicates (`cpufeatures`, `rustc-hash`, `rustls-platform-verifier`, `event-listener`, `async-channel`, `bit-set`, `bit-vec`, `fixedbitset`, `wast`, `wasm-encoder`, `winnow`, `toml_datetime`).

### 5. No `[patch.crates-io]` Additions Recommended

None of the duplicate versions are semver-compatible (same major version, different minor). All duplicates are true semver incompatibilities from different upstream crates with different MSRV/version requirements. Patching would risk breaking upstream compatibility. The existing `half` patch should remain as-is.

---

## Dependency Count by Crate (depth-1 direct deps)

| Crate | Direct deps | Dev deps |
|---|---|---|
| clawdius | 24 | 2 |
| clawdius-core | 42 | 6 |
| clawdius-code | 10 | 1 |
| clawdius-gateway | 14 | 1 |
| clawdius-mcp | 4 | 1 |

**clawdius-core** is the dependency hotspot — it pulls wasmtime, genai, reqwest, tree-sitter, tiktoken-rs, and all optional feature deps. Any optimization here has the largest impact.
