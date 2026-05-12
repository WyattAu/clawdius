# Transitive Dependency Audit Report

**Workspace:** clawdius  
**Date:** 2025-05-12  
**Command:** `cargo tree --workspace --duplicates`

## Summary

| Metric | Value |
|--------|-------|
| Total unique duplicate crates | 91 |
| High priority (semver-incompatible) | 29 |
| Low priority (dev-only) | 4 |
| Low priority (acceptable/expected) | 58 |

Note: Several entries flagged by `--duplicates` are false positives -- same version appearing in multiple duplicate-tree blocks, or same-version features/features2 deduplication. After removing false positives (bitflags, log, and single-version feature duplicates), **29 distinct high-priority duplicates** and **4 dev-only duplicates** remain actionable.

---

## Category 1: High Priority -- Semver-Incompatible Duplicates

These represent different major/minor versions of the same crate coexisting in the dependency graph. Each increases binary size and compile time, and some (http, hyper) can cause type mismatches at crate boundaries.

| Crate | Versions | Pulled by (older) | Pulled by (newer) | Action |
|-------|----------|-------------------|-------------------|--------|
| `http` | 0.2.12 vs 1.4.0 | `hyper 0.14` (httpmock dev-dep) | `axum`, `reqwest`, `jsonrpsee`, `tower-http` | Eliminate httpmock or replace with lighter mock; no fix needed for prod deps (they already agree on v1) |
| `http-body` | 0.4.6 vs 1.0.1 | `hyper 0.14` (httpmock dev-dep) | `axum`, `reqwest`, `hyper-util`, `tower-http` | Same as http -- driven by httpmock |
| `hyper` | 0.14.32 vs 1.9.0 | `httpmock 0.7` (dev-dep) | `axum`, `reqwest`, `jsonrpsee`, `tower-http` | Dev-only; prod deps all use v1 |
| `thiserror` | 1.0.69 vs 2.0.18 | `jsonrpsee`, `witx` | `clawdius`, `clawdius-core`, `clawdius-gateway`, `syntect`, `wasmtime` | Wait for jsonrpsee to adopt thiserror v2, or patch |
| `thiserror-impl` | 1.0.69 vs 2.0.18 | `jsonrpsee` proc-macro chain | `clawdius` workspace, `wasmtime` | Follows thiserror above |
| `itertools` | 0.10.5 / 0.11.0 vs 0.14.0 | 0.10: `criterion` (dev); 0.11: `lalrpop` (httpmock build-dep) | 0.14: `ratatui`, `wasmtime-cranelift` | 0.10/0.11 are dev-only; no prod conflict |
| `petgraph` | 0.6.5 vs 0.7.1 | `lalrpop`, `wasm-compose` (both via transitive deps) | `clawdius-core` (direct dep) | Unify to v0.7 if upstream deps allow; otherwise accept |
| `async-channel` | 1.9.0 vs 2.5.0 | `async-std` (httpmock tree) | `async-global-executor`, `async-process` | Dev-only; no prod impact |
| `getrandom` | 0.2.17 / 0.3.4 vs 0.4.2 | 0.2: `ring`, `rand 0.8`; 0.3: `rand 0.9` (proptest dev) | 0.4: `tempfile`, `uuid` (prod) | Accept; driven by rand ecosystem split |
| `rand` | 0.8.6 vs 0.9.4 | 0.8: `cap-rand` (wasmtime-wasi), `jsonrpsee-core`, `soketto` | 0.9: `proptest` (dev-dep) | Dev-only for v0.9; prod all on v0.8 |
| `rand_core` | 0.6.4 vs 0.9.5 | 0.6: `rand 0.8` ecosystem | 0.9: `proptest` (dev-dep) | Follows rand above |
| `rand_chacha` | 0.3.1 vs 0.9.0 | 0.3: `rand 0.8` | 0.9: `proptest` (dev-dep) | Follows rand above |
| `base64` | 0.21.7 vs 0.22.1 | 0.21: `httpmock` (dev), `tiktoken-rs` | 0.22: `clawdius-core`, `genai`, `hyper-util`, `reqwest`, `wasmtime` | `tiktoken-rs` is the only prod dep on old version; check if newer tiktoken-rs exists |
| `bit-set` | 0.5.3 vs 0.8.0 | 0.5: `fancy-regex`, `lalrpop` (httpmock tree) | 0.8: `proptest` (dev-dep) | Both paths are dev-only in practice |
| `bit-vec` | 0.6.3 vs 0.8.0 | 0.6: `bit-set 0.5` (lalrpop/httpmock) | 0.8: `proptest` (dev-dep) | Follows bit-set |
| `event-listener` | 2.5.3 vs 5.4.1 | 2.5: `async-channel 1` (httpmock tree) | 5.4: `async-lock`, `async-channel 2` | Dev-only for old version |
| `fixedbitset` | 0.4.2 vs 0.5.7 | 0.4: `petgraph 0.6` (lalrpop, wasm-compose) | 0.5: `petgraph 0.7` (clawdius-core) | Follows petgraph |
| `hashbrown` | 0.16.1 vs 0.17.0 | 0.16: `cranelift`, `wasmtime-environ`, `wit-parser`, `rusqlite` (hashlink), `ratatui`, `lru` | 0.17: `indexmap 2.14` | Wasmtime internal split; accept |
| `linux-raw-sys` | 0.4.15 vs 0.12.1 | 0.4: `rustix 0.38` (wasmtime-wasi) | 0.12: `rustix 1.1` (async-std, wasmtime-wasi, crossterm, tempfile) | Follows rustix split |
| `rustix` | 0.38.44 vs 1.1.4 | 0.38: `system-interface` (wasmtime-wasi) | 1.1: `async-io`, `async-std`, `crossterm`, `tempfile`, `wasmtime` | Wasmtime internal split between wasmtime-wasi and cap-std; accept |
| `rustc-hash` | 1.1.0 vs 2.1.2 | v1: `tiktoken-rs` | v2: `cranelift`, `fxprof`, `jsonrpsee-core`, `regalloc2` | `tiktoken-rs` is only prod consumer of v1; check for update |
| `socket2` | 0.5.10 vs 0.6.3 | 0.5: `hyper 0.14` (httpmock dev) | 0.6: `hyper-util`, `tokio` | Dev-only for old version |
| `syn` | 1.0.109 vs 2.0.117 | v1: `async-attributes` (async-std / httpmock tree) | v2: virtually all prod proc-macros | Dev-only for v1 |
| `toml` | 0.9 vs 1.1 | 0.9: `wasmtime-internal-cache` | 1.1: `clawdius`, `clawdius-core` | Different spec variants but same semver target; accept |
| `toml_datetime` | 0.7 vs 1.1 | 0.7: `toml 0.9` (wasmtime) | 1.1: `toml 1.1` (clawdius workspace) | Follows toml |
| `winnow` | 0.7.15 vs 1.0.2 | 0.7: `toml 0.9` (wasmtime) | 1.0: `toml 1.1` (clawdius workspace) | Follows toml |
| `tower` | 0.4.13 vs 0.5.3 | 0.4: `jsonrpsee-http-client`, `jsonrpsee-server` | 0.5: `axum`, `clawdius`, `clawdius-core`, `reqwest`, `tower-http` | Wait for jsonrpsee to adopt tower v0.5 |
| `wast` | 35.0.2 vs 248.0.0 | 35: `witx` (wiggle-generate) | 248: `wat` (wasmtime) | Wasmtime internal; the v35 is an ancient forked version. Accept |
| `wasm-encoder` | 0.246.2 vs 0.248.0 | 0.246: `wasmtime`, `wasm-compose` | 0.248: `wast 248` | Wasmtime internal version skew; accept |
| `reqwest` | 0.12.28 vs 0.13.3 | 0.12: `clawdius-core`, `clawdius-gateway`, `jsonrpsee`, `tower-http` | 0.13: `genai` | Migrate workspace to reqwest 0.13 when jsonrpsee supports it |
| `rustls-platform-verifier` | 0.5.3 vs 0.7.0 | 0.5: `jsonrpsee-client-transport`, `jsonrpsee-http-client` | 0.7: `reqwest 0.13` (genai) | Follows reqwest split; wait for jsonrpsee |
| `cpufeatures` | 0.2.17 vs 0.3.0 | 0.2: `aes`/`aes-gcm`/`sha1`/`sha2`/`soketto` | 0.3: `blake3` | Accept; both are prod paths but different crypto implementations |

---

## Category 2: Dev-Only Duplicates (Low Priority)

These duplicates only appear in the build graph via `[dev-dependencies]` or `[build-dependencies]` and do not affect production binaries.

| Crate | Versions | Only via | Note |
|-------|----------|----------|------|
| `lalrpop-util` | 0.20.2 (x2) | `httpmock -> basic-cookies -> lalrpop` (build-dep) | Entire chain is dev-only |
| `async-channel` | 1.9.0 | `httpmock -> async-std -> async-object-pool` | Dev-only |
| `hyper 0.14` | 0.14.32 | `httpmock` | Dev-only; prod uses hyper 1.x |
| `http 0.2` | 0.2.12 | `httpmock -> hyper 0.14` | Dev-only |

---

## Category 3: Acceptable Duplicates (Low Priority)

These are expected and harmless:

- **Same-version feature duplicates:** `aho-corasick`, `bitflags`, `bumpalo`, `crypto-common`, `digest`, `either`, `gimli`, `log`, `memchr`, `object`, `postcard`, `pulley-interpreter`, `regex`, `regex-automata`, `regex-syntax`, `semver`, `serde`, `serde_core`, `serde_json`, `sha2`, `smallvec`, `wasmtime-environ`, `wasmtime-internal-core` -- same version compiled with different feature sets.
- **Proc-macro deduplication:** `serde_core`, `thiserror-impl` -- expected artifact of proc-macro compilation.
- **Wasmtime internal:** `wasmtime-environ`, `wasmtime-internal-core` -- single crate version, split across feature sets.

---

## Recommended Next Steps

1. **reqwest unification (medium effort, high impact):** The biggest real duplicate. `jsonrpsee 0.24` pins reqwest 0.12 while `genai` pulls reqwest 0.13. This also cascades into `tower`, `http`, `hyper`, `rustls-platform-verifier` splits. Monitor jsonrpsee for reqwest 0.13 support.
2. **httpmock removal (low effort, medium impact):** Replacing `httpmock` with `wiremock` or `mockito` (which use http v1) would eliminate 5+ duplicate crates from the dev tree (`http 0.2`, `http-body 0.4`, `hyper 0.14`, `socket2 0.5`, `async-channel 1`, `syn 1`, `event-listener 2`).
3. **tiktoken-rs update (low effort, low impact):** Check if a newer tiktoken-rs uses base64 v0.22 and rustc-hash v2.
4. **thiserror v2 migration (low effort, medium impact):** Patch `jsonrpsee` or wait for upstream. v1 vs v2 is a widespread ecosystem split.
5. **Wasmtime version skew (no action needed):** The `rustix 0.38 vs 1.1`, `hashbrown 0.16 vs 0.17`, `wast 35 vs 248`, `wasm-encoder 0.246 vs 0.248` splits are all internal to wasmtime 44. These will resolve when upgrading to a single wasmtime version.
6. **toml 0.9 vs 1.1 (no action needed):** Both use `spec-1.1.0`. This is a feature-flag split, not a real version conflict.
