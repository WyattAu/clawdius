# Production Unwrap Audit Report

**Workspace:** clawdius
**Date:** 2026-05-12
**Scope:** All `.rs` files under `crates/`, excluding test and benchmark files

## Summary

| Metric | Value |
|--------|-------|
| Total `.unwrap()` calls in production code | **1,773** |
| Files containing `.unwrap()` | **121** |
| Roadmap target | < 30 files |
| Current workspace lint | `unwrap_used = "warn"` |
| clawdius-core lint | `#![deny(clippy::unwrap_used)]` (since this audit) |

## Top 10 Files by Unwrap Count

| File | Unwraps | Lines | Density |
|------|---------|-------|---------|
| `timeline/store.rs` | 136 | 1,609 | 8.5% |
| `timeline/mod.rs` | 113 | 708 | 16.0% |
| `workspace/manager.rs` | 98 | 811 | 12.1% |
| `storage/sqlite/mod.rs` | 67 | 290 | 23.1% |
| `storage/postgres/mod.rs` | 67 | 539 | 12.4% |
| `storage/mariadb.rs` | 67 | 2,271 | 3.0% |
| `mcp/handler.rs` | 66 | 1,103 | 6.0% |
| `session/storage/vfs.rs` | 49 | 516 | 9.5% |
| `session/membership.rs` | 41 | 396 | 10.4% |
| `storage/in_memory.rs` | 33 | 285 | 11.6% |

## Pattern Classification

### Infallible (safe to unwrap -- document with expect)

| Pattern | Count | Safety Argument |
|---------|-------|-----------------|
| `Ok(x)` constructors | ~1,880 | Not unwraps; just Result construction |
| Lock operations (`.lock().unwrap()`) | 20 | Poisons only on panic-while-holding; acceptable in single-threaded contexts |
| `.to_str().unwrap()` on known-ASCII paths | 9 | File paths are UTF-8 on Linux/macOS |

### Fallible (should use `?` or `expect`)

| Pattern | Estimated Count | Risk |
|---------|----------------|------|
| Database query results | ~400 | SQLite/Postgres/MariaDB queries can fail |
| HashMap `.get().unwrap()` on computed keys | ~19 | Key may not exist if logic changes |
| `serde_json::from_str().unwrap()` | ~50 | User-controlled input can be malformed |
| `std::fs::read/write` chains | ~30 | File I/O can fail |
| Configuration parsing | ~40 | Missing/invalid config fields |
| Network operations | ~100 | Timeouts, connection failures |

### Already Protected

| Pattern | Status |
|---------|--------|
| `clawdius-core/src/lib.rs` | `#![deny(clippy::unwrap_used)]` -- 0 violations |
| Workspace root `Cargo.toml` | `unwrap_used = "warn"` -- warns in all crates |

## Strategy

### Phase 1: High-Impact Modules (Week 1-2)

Target the highest-density files first. Each file should be audited in isolation:

1. `storage/sqlite/mod.rs` (23.1% density, 67 unwraps)
2. `timeline/mod.rs` (16.0% density, 113 unwraps)
3. `workspace/manager.rs` (12.1% density, 98 unwraps)
4. `storage/in_memory.rs` (11.6% density, 33 unwraps)
5. `session/membership.rs` (10.4% density, 41 unwraps)
6. `storage/postgres/mod.rs` (12.4% density, 67 unwraps)

**Approach per file:**
1. Add `#![deny(clippy::unwrap_used)]` at file top
2. Fix each compile error: replace with `?`, `expect("invariant: ...")`, or `ok()?`
3. Run `cargo test -p clawdius-core` to verify no regressions
4. Commit per file

### Phase 2: Medium-Impact Modules (Week 2-3)

Files with 20-60 unwraps: handler.rs, vfs.rs, encryption.rs, api/rest.rs, api/vscode.rs, sandbox/wasi.rs, etc.

### Phase 3: Low-Impact Modules (Week 3-4)

Files with <20 unwraps: CLI commands, adapter modules, formatter, etc.

### Phase 4: Workspace-Level Enforcement

After all files are fixed, escalate workspace lint from `warn` to `deny`:

```toml
[workspace.lints.clippy]
unwrap_used = "deny"
```

## Tracking

Files remaining after this audit: **121**

Progress will be tracked by counting files with `.unwrap()` after each batch.
