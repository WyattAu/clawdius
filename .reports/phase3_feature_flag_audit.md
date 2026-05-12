# Feature Flag Audit Report

**Workspace:** clawdius
**Date:** 2026-05-12
**Command:** manual audit of `[features]` in each crate Cargo.toml

## Summary

| Crate | Feature Count | Default Features | Notes |
|-------|--------------|------------------|-------|
| clawdius | 7 | `mimalloc` | Passthrough to clawdius-core |
| clawdius-core | 11 | none | Core feature definitions |
| clawdius-code | 1 | `mimalloc` | Minimal |
| clawdius-mcp | 0 | none | No features |
| clawdius-gateway | 5 | none | Platform adapter flags |

## Feature Map

### clawdius (CLI binary)

| Feature | Type | Dependencies |
|---------|------|-------------|
| `default` | meta | `mimalloc` |
| `mimalloc` | implicit | Global allocator |
| `keyring` | passthrough | `clawdius-core/keyring`, `dep:keyring` |
| `crash-reporting` | passthrough | `clawdius-core/crash-reporting` |
| `vector-db` | passthrough | `clawdius-core/vector-db` |
| `browser` | passthrough | `clawdius-core/browser` |
| `embeddings` | passthrough | `clawdius-core/embeddings` |
| `local-llm` | passthrough | `clawdius-core/local-llm` |

### clawdius-core (library)

| Feature | Type | Dependencies |
|---------|------|-------------|
| `default` | none | Empty |
| `keyring` | optional | `dep:keyring` |
| `crash-reporting` | optional | `dep:sentry` |
| `browser` | optional | `dep:chromiumoxide` |
| `embeddings` | optional | `dep:tokenizers`, `dep:hf-hub` |
| `local-llm` | composite | `embeddings`, `dep:candle-core`, `dep:candle-nn`, `dep:candle-transformers` |
| `vector-db` | optional | `dep:lancedb`, `dep:arrow`, `dep:arrow-array`, `dep:arrow-schema` |
| `orchestrator` | flag | No deps (enables orchestrator module) |
| `redis-queue` | composite | `orchestrator`, `dep:redis` |
| `postgres` | optional | `dep:tokio-postgres`, `dep:deadpool-postgres` |
| `mariadb` | optional | `dep:mysql_async` |
| `stripe` | optional | `dep:stripe` |

### clawdius-gateway (library + binary)

| Feature | Type | Dependencies |
|---------|------|-------------|
| `default` | none | Empty |
| `telegram` | optional | `dep:teloxide` |
| `discord` | optional | `dep:serenity` |
| `slack` | optional | `dep:slack-morphism` |
| `matrix` | optional | `dep:matrix-sdk` |
| `all-platforms` | composite | `telegram`, `discord`, `slack`, `matrix` |

## OOM Root Cause

`cargo build --workspace --all-features` causes OOM because it activates all features simultaneously:

1. **WASM-heavy:** `local-llm` pulls `candle-core`, `candle-nn`, `candle-transformers` (large ML crates)
2. **Database-heavy:** `vector-db` pulls `lancedb` + `arrow` ecosystem; `postgres` + `mariadb` add connection pools
3. **Network-heavy:** `all-platforms` pulls `teloxide`, `serenity`, `slack-morphism`, `matrix-sdk` (4 large async frameworks)
4. **Combined:** Compiling all of these in a single `cargo check` with all features exceeds typical CI memory (8-16 GB)

## Supported Feature Sets

| Set | Features | Use Case |
|-----|----------|----------|
| Minimal | default | Basic CLI without optional deps |
| Full client | default + keyring + crash-reporting | Production CLI with all user-facing features |
| AI-complete | default + embeddings + local-llm + vector-db | Full AI pipeline (requires ~8 GB RAM to compile) |
| Gateway-full | all-platforms | All chat platform adapters |
| Database | postgres + mariadb + redis-queue + vector-db | Full backend with persistence |

## Conflicting Combinations

No feature conflicts detected. All features are additive. The only issue is memory consumption when compiling all features simultaneously.

## Recommendations

1. **CI feature matrix:** Instead of `--all-features`, run separate CI jobs per feature set:
   - Job 1: `default` (fast, low memory)
   - Job 2: `--features keyring,crash-reporting` (production client)
   - Job 3: `--features vector-db,embeddings` (AI features)
   - Job 4: `--features postgres,mariadb,redis-queue` (database features)
   - Job 5 (gateway): `--features all-platforms` (platform adapters)
2. **Feature gate documentation:** Add a `FEATURES.md` file documenting which features enable which capabilities and their compile-time cost.
3. **Consider splitting `local-llm`:** The `candle` crate family is extremely large. Consider making it a separate workspace crate to reduce incremental compile times.
