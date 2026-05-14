# Clawdius Feature Flag Matrix

Generated: 2026-05-13

## Feature Matrix

### clawdius-core (core library)

| Feature | Enables Deps | Default |
|---------|-------------|---------|
| `keyring` | `keyring` | No |
| `crash-reporting` | `sentry 0.32` | No |
| `browser` | `chromiumoxide 0.7` | No |
| `embeddings` | `tokenizers 0.15`, `hf-hub 0.3` | No |
| `local-llm` | `embeddings` + `candle-core`, `candle-nn`, `candle-transformers 0.4` | No |
| `vector-db` | `lancedb 0.27`, `arrow 57`, `arrow-array 57`, `arrow-schema 57` | No |
| `orchestrator` | _(no deps)_ | No |
| `redis-queue` | `orchestrator` + `redis 0.27` | No |
| `postgres` | `tokio-postgres 0.7`, `deadpool-postgres 0.14` | No |
| `mariadb` | `mysql_async 0.35` | No |
| `stripe` | `async-stripe 1.0.0-rc.5` | No |

Default: `[]` (empty — all features opt-in)

### clawdius (CLI binary)

| Feature | Enables | Default |
|---------|---------|---------|
| `mimalloc` | `dep:mimalloc` | **Yes** |
| `keyring` | `clawdius-core/keyring`, `dep:keyring` | No |
| `crash-reporting` | `clawdius-core/crash-reporting` | No |
| `vector-db` | `clawdius-core/vector-db` | No |
| `browser` | `clawdius-core/browser` | No |
| `embeddings` | `clawdius-core/embeddings` | No |
| `local-llm` | `clawdius-core/local-llm` | No |

Default: `["mimalloc"]`

### clawdius-gateway (messaging gateway)

| Feature | Enables Deps | Default |
|---------|-------------|---------|
| `telegram` | `teloxide 0.14` | No |
| `discord` | `serenity 0.12` | No |
| `slack` | `slack-morphism 1` | No |
| `matrix` | `matrix-sdk 0.10` | No |
| `all-platforms` | telegram + discord + slack + matrix | No |

Default: `[]` (empty)

### clawdius-code (VSCode helper)

| Feature | Enables Deps | Default |
|---------|-------------|---------|
| `mimalloc` | `dep:mimalloc` | **Yes** |

Default: `["mimalloc"]`

### clawdius-mcp (MCP server)

No features defined. No optional dependencies.

Default: `[]` (empty)

## Feature Dependency Graph

```
clawdius-core
├── embeddings → tokenizers, hf-hub
├── local-llm → embeddings + candle-core, candle-nn, candle-transformers
├── redis-queue → orchestrator + redis
└── (all others are leaf features)

clawdius (CLI)
├── keyring → clawdius-core/keyring
├── crash-reporting → clawdius-core/crash-reporting
├── vector-db → clawdius-core/vector-db
├── browser → clawdius-core/browser
├── embeddings → clawdius-core/embeddings
└── local-llm → clawdius-core/local-llm

clawdius-gateway
└── all-platforms → telegram, discord, slack, matrix
```

## `--all-features` Compilation Results

| Crate | Compiles | Status |
|-------|----------|--------|
| clawdius-core | Yes | Pass |
| clawdius-mcp | Yes | Pass |
| clawdius-code | Yes | Pass |
| clawdius | **No** | FAIL — `IndexStats` not in scope (`src/cli/index.rs:91`) |
| clawdius-gateway | **No** | FAIL — 8 errors (see below) |

**Full workspace `--all-features`: FAIL**

### clawdius errors

- `E0425`: `IndexStats` not found in `src/cli/index.rs:91` — likely gated behind `vector-db` feature but missing the import/feature gate in the CLI crate.

### clawdius-gateway errors (8 total)

| Error | Location | Description |
|-------|----------|-------------|
| E0433 | telegram.rs | Unresolved `tokio_util` crate/module |
| E0308 | telegram.rs | Mismatched types |
| E0599 | telegram.rs | `allowed_update` not found on `JsonRequest<GetUpdates>` |
| E0599 | telegram.rs | `RetryAfter` variant not found on `teloxide::ApiError` |
| E0599 | telegram.rs | `Mutex::lock().clone()` — `Mutex` doesn't impl `Clone` |
| E0599 | telegram.rs | `AtomicBool` — `clone()` not found |
| E0282 | telegram.rs | Type annotations needed (x2) |

Root cause: The `telegram` adapter code has compatibility issues with `teloxide 0.14` — likely written for an older version. The `all-platforms` feature activates this broken code path.

## Known Conflicts & Limitations

1. **clawdius `vector-db` proxy bug**: The CLI's `src/cli/index.rs` references `IndexStats` only available when `clawdius-core/vector-db` is enabled, but the CLI feature `vector-db` doesn't properly gate the usage.
2. **clawdius-gateway `telegram` broken**: `teloxide 0.14` API changes broke the telegram adapter. The `all-platforms` feature is unusable until this is fixed.
3. **Feature cascade via CLI**: Enabling `clawdius/local-llm` pulls in `candle-*`, `tokenizers`, `hf-hub`, `lancedb`, and `arrow-*` — significantly increasing compile time and binary size.
4. **No workspace-level features**: The root workspace has no `[workspace.features]` — each crate manages features independently. Feature consistency across crates is manual.

## Recommended CI Test Combinations

### Tier 1: Fast smoke tests (~2 min each)

```bash
# Minimal (no optional deps)
cargo check -p clawdius-core
cargo check -p clawdius --no-default-features
cargo check -p clawdius-gateway
cargo check -p clawdius-code --no-default-features
cargo check -p clawdius-mcp
```

### Tier 2: Individual feature coverage

```bash
# Core features (isolated)
cargo check -p clawdius-core --features keyring
cargo check -p clawdius-core --features postgres
cargo check -p clawdius-core --features mariadb
cargo check -p clawdius-core --features redis-queue
cargo check -p clawdius-core --features vector-db
cargo check -p clawdius-core --features embeddings
cargo check -p clawdius-core --features stripe
cargo check -p clawdius-core --features browser

# CLI feature passthrough
cargo check -p clawdius --features keyring
cargo check -p clawdius --features crash-reporting

# Gateway platforms (test each independently)
cargo check -p clawdius-gateway --features telegram   # EXPECTED FAIL until fixed
cargo check -p clawdius-gateway --features discord
cargo check -p clawdius-gateway --features slack
cargo check -p clawdius-gateway --features matrix
```

### Tier 3: Heavyweight (long compile)

```bash
# Local LLM stack (pulls candle + tokenizers + hf-hub)
cargo check -p clawdius-core --features local-llm

# vector-db + local-llm combined
cargo check -p clawdius-core --features "vector-db,local-llm"
```

### Tier 4: Full matrix (fix before enabling)

```bash
# These should pass but currently FAIL:
# cargo check -p clawdius --all-features
# cargo check -p clawdius-gateway --all-features
# cargo check --workspace --all-features
```

## Summary

| Metric | Value |
|--------|-------|
| Total crates | 5 |
| Crates with features | 4 |
| Total features | 22 |
| Total optional deps | 21 |
| `--all-features` workspace | **FAIL** (2 crates broken) |
| Blocking issues | 2 (IndexStats import, teloxide compat) |
