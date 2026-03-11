# Dependency Reduction Plan

## Current State

- **Total Dependencies**: 1003 unique crates
- **Total Dependency Lines**: 2980
- **Target**: Reduce to <700 dependencies (30% reduction)

## Heaviest Dependency Chains

| Count | Dependency | Impact |
|-------|------------|--------|
| 77 | serde | Core serialization - cannot remove |
| 66 | quote | Proc-macro dependency |
| 62 | proc-macro2 | Proc-macro dependency |
| 52 | syn | Proc-macro dependency |
| 52 | log | Pervasive logging |
| 48 | tokio | Async runtime |
| 48 | libc | System bindings |
| 44 | tracing | Observability |
| 44 | bytes | Byte handling |
| 41 | serde_json | JSON serialization |
| 31 | arrow-schema | Vector DB dependency |
| 30 | arrow-array | Vector DB dependency |
| 23 | futures | Async utilities |
| 23 | chrono | Date/time |

## Major Dependency Groups

### 1. ML/Embeddings Stack (~80+ crates)
- `candle-core`, `candle-nn`, `candle-transformers`
- `tokenizers`, `hf-hub`
- `arrow`, `arrow-array`, `arrow-schema`, `arrow-*` (10+ crates)
- `lancedb`, `lance-arrow`

### 2. WASM Runtime (~40+ crates)
- `wasmtime` and all internal crates

### 3. Browser Automation (~30+ crates)
- `chromiumoxide` + `headless_chrome` (duplicated functionality)
- Both pull in websocket, TLS, and HTTP stacks

### 4. Tree-sitter Languages (~25+ crates)
- 6 language parsers, each with C library compilation

### 5. Async Runtime Duplication (~20+ crates)
- Both `tokio` AND `monoio` runtimes

## Duplicate Version Analysis

| Crate | Versions | Issue |
|-------|----------|-------|
| base64 | 0.13, 0.21, 0.22 | 3 versions |
| bitflags | 1.3, 2.11 | 2 major versions |
| darling | 0.14, 0.21, 0.23 | 3 versions |
| dirs | 5.0, 6.0 | 2 versions |
| getrandom | 0.2, 0.3, 0.4 | 3 versions |
| h2 | 0.3, 0.4 | 2 versions |
| thiserror | 1.0, 2.0 | 2 versions |
| lru | 0.12, 0.16 | 2 versions |

## Reduction Strategy

### Priority 1: Quick Wins (No Code Changes)

#### 1.1 Remove Duplicate Browser Automation
```toml
# REMOVE ONE:
- chromiumoxide  # Keep - tokio-native
- headless_chrome # Remove - pulls async-std
```
**Estimated savings**: 15-20 crates

#### 1.2 Consolidate Async Runtime
```toml
# Choose ONE runtime:
- tokio (recommended - ecosystem standard)
- monoio (remove unless io_uring is critical)
```
**Estimated savings**: 10-15 crates

#### 1.3 Update Duplicate Versions
Add to `Cargo.toml` to force single versions:
```toml
[patch.crates-io]
# Force specific versions to deduplicate
```

### Priority 2: Feature Reduction

#### 2.1 Tokio Features
```toml
# Current: ["rt", "rt-multi-thread", "macros", "sync", "time", "process", "io-util", "signal"]
# Reduce to essentials:
tokio = { version = "1", features = ["rt", "rt-multi-thread", "macros", "sync", "time"] }
# Remove "process", "signal" if not used in core
```
**Estimated savings**: 5-10 crates

#### 2.2 Serde Features
```toml
# Current: features = ["derive"]
# Already minimal - keep as is
```

#### 2.3 Reqwest Features
```toml
# Current: ["json", "stream"]
# If streaming not used:
reqwest = { version = "0.12", features = ["json"] }
```
**Estimated savings**: 5 crates

#### 2.4 Tracing Features
```toml
# Current: ["env-filter", "json"]
# If JSON logging not used:
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```
**Estimated savings**: 3-5 crates

### Priority 3: Optional Heavy Dependencies

#### 3.1 Make ML Stack Optional
```toml
[features]
default = []
embeddings = ["candle-core", "candle-nn", "candle-transformers", "tokenizers", "hf-hub"]
```
**Estimated savings**: 50-60 crates (when disabled)

#### 3.2 Make Vector DB Optional
```toml
[features]
vector-db = ["lancedb", "arrow", "arrow-array", "arrow-schema"]
```
**Estimated savings**: 40-50 crates (when disabled)

#### 3.3 Make WASM Runtime Optional
```toml
[features]
wasm-sandbox = ["wasmtime"]
```
**Estimated savings**: 30-40 crates (when disabled)

#### 3.4 Make Browser Automation Optional
```toml
[features]
browser = ["chromiumoxide"]  # Remove headless_chrome
```
**Estimated savings**: 20-30 crates (when disabled)

### Priority 4: Replace with Std/Minimal Alternatives

#### 4.1 Replace chrono with time
```toml
# chrono is heavy (23 dependents)
# time is lighter and more modern
time = "0.3"
```
**Estimated savings**: 10-15 crates

#### 4.2 Replace once_cell with std::sync::OnceLock
```toml
# once_cell is now in std
# Replace: once_cell::sync::Lazy with std::sync::OnceLock
```
**Estimated savings**: 1 crate (but modernizes code)

#### 4.3 Replace glob with glob_match or std::fs
```toml
# For simple patterns, use std::fs with filtering
# For complex patterns, glob_match is lighter
```
**Estimated savings**: 1-2 crates

### Priority 5: Architecture Changes

#### 5.1 Separate CLI from Core
Move heavy dependencies to CLI crate only:
- `chromiumoxide` → CLI only
- `headless_chrome` → Remove entirely
- `syntect` → CLI only (syntax highlighting)

#### 5.2 Feature-gate Language Support
```toml
[features]
default = []
lang-rust = ["tree-sitter-rust"]
lang-python = ["tree-sitter-python"]
lang-javascript = ["tree-sitter-javascript", "tree-sitter-typescript"]
lang-go = ["tree-sitter-go"]
```

## Implementation Order

1. **Phase 1** (Immediate, No Code Changes)
   - Remove `headless_chrome` (keep `chromiumoxide`)
   - Remove `monoio` if not critical
   - Pin duplicate versions

2. **Phase 2** (Feature Flags)
   - Add `embeddings` feature flag
   - Add `vector-db` feature flag
   - Add `wasm-sandbox` feature flag

3. **Phase 3** (Refactoring)
   - Replace `chrono` with `time`
   - Replace `once_cell` with std
   - Feature-gate tree-sitter languages

4. **Phase 4** (Architecture)
   - Split CLI-specific dependencies
   - Create minimal core crate

## Estimated Impact

| Change | Crates Removed | Difficulty |
|--------|---------------|------------|
| Remove headless_chrome | 15-20 | Easy |
| Remove monoio | 10-15 | Easy |
| Feature-gate embeddings | 50-60 | Medium |
| Feature-gate vector-db | 40-50 | Medium |
| Feature-gate wasm | 30-40 | Medium |
| Reduce tokio features | 5-10 | Easy |
| Deduplicate versions | 10-20 | Easy |
| **Total Potential** | **160-215** | - |

## Quick Win Commands

```bash
# 1. Remove headless_chrome (keep chromiumoxide)
# Edit Cargo.toml to remove headless_chrome line

# 2. Check if monoio is actually used
rg "monoio" --type rust

# 3. Find unused workspace dependencies
cargo +nightly udeps --all-targets

# 4. Update to resolve duplicates
cargo update
```

## Monitoring

After each change:
```bash
cargo tree --prefix none | grep -E "^[a-z]" | sort -u | wc -l
```

Target: <700 dependencies
