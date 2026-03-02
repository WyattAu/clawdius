# Clawdius Dependency Analysis

**Generated:** 2026-03-01
**Project:** Clawdius v0.1.0

## Executive Summary

| Metric | Value |
|--------|-------|
| Total Dependencies | 2932 |
| Direct Dependencies | 21 |
| Dev Dependencies | 4 |
| Build Dependencies | ~50 |
| Transitive Dependencies | 2907 |
| Max Dependency Depth | 11 |

## Dependency Tree Overview

```
clawdius v0.1.0
├── Async Runtime
│   └── monoio v0.2.4 (io_uring, thread-per-core)
├── WASM Runtime
│   └── wasmtime v42.0.1
├── Database
│   ├── rusqlite v0.38.0
│   └── lancedb v0.26.2
│       └── lance v2.0.0
│           └── datafusion v51.0.0
├── LLM Integration
│   ├── genai v0.5.3
│   └── async-openai v0.33.0
├── Code Analysis
│   ├── tree-sitter v0.26.0
│   └── syntect v5.3.0
├── Terminal UI
│   ├── ratatui v0.30.0
│   └── crossterm v0.29.0
├── Serialization
│   ├── serde v1.0.228
│   ├── serde_json v1.0.149
│   ├── toml v1.0.3+spec-1.1.0
│   └── rkyv v0.8.10
├── Security
│   └── keyring v3.6.2
├── Utilities
│   ├── uuid v1.21.0
│   ├── thiserror v2.0.18
│   ├── tracing v0.1.44
│   └── tracing-subscriber v0.3.20
└── Allocator (optional)
    └── mimalloc v0.1.46
```

## Critical Dependencies Analysis

### 1. monoio v0.2.4 - Async Runtime

| Attribute | Assessment |
|-----------|------------|
| **Purpose** | Thread-per-core async runtime with io_uring |
| **Risk Level** | MEDIUM |
| **Unsafe Code** | Yes (io_uring bindings) |
| **Maintenance** | Active |
| **Audit Priority** | HIGH |

**Notes:**
- Uses io_uring for zero-copy I/O
- Thread-per-core model eliminates lock contention
- Contains unsafe code for FFI bindings
- Per HFT SOP, this is the preferred runtime for high-throughput scenarios

**Transitive Risks:**
- `fxhash` - Unmaintained (RUSTSEC-2025-0057)

---

### 2. wasmtime v42.0.1 - WASM Runtime

| Attribute | Assessment |
|-----------|------------|
| **Purpose** | WASM sandbox for Brain isolation |
| **Risk Level** | LOW |
| **Unsafe Code** | Yes (JIT compilation) |
| **Maintenance** | Active (Bytecode Alliance) |
| **Audit Priority** | HIGH |

**Notes:**
- Maintained by Bytecode Alliance with strong security focus
- Used for sandboxing untrusted code execution
- Implements capability-based security model
- Regular security audits by Mozilla/Bytecode Alliance

**Transitive Dependencies:** ~200 crates

---

### 3. rusqlite v0.38.0 - SQLite Database

| Attribute | Assessment |
|-----------|------------|
| **Purpose** | Embedded SQL database |
| **Risk Level** | LOW |
| **Unsafe Code** | Yes (SQLite FFI) |
| **Maintenance** | Active |
| **Audit Priority** | MEDIUM |

**Notes:**
- Bundled SQLite for reproducibility
- Well-maintained Rust bindings
- Critical for persistent storage

---

### 4. lancedb v0.26.2 - Vector Database

| Attribute | Assessment |
|-----------|------------|
| **Purpose** | Vector database for embeddings |
| **Risk Level** | MEDIUM |
| **Unsafe Code** | Yes (Arrow FFI) |
| **Maintenance** | Active |
| **Audit Priority** | HIGH |

**Notes:**
- Large dependency tree (~1000 transitive deps)
- Brings in Apache Arrow ecosystem
- Uses datafusion for query execution

**Transitive Risks:**
- `paste` - Unmaintained (RUSTSEC-2024-0436)

---

### 5. genai v0.5.3 - LLM Integration

| Attribute | Assessment |
|-----------|------------|
| **Purpose** | Multi-provider LLM client |
| **Risk Level** | LOW |
| **Unsafe Code** | Minimal |
| **Maintenance** | Active |
| **Audit Priority** | LOW |

**Notes:**
- Abstracts multiple LLM providers
- Uses async Rust patterns
- HTTP client for API calls

---

### 6. tree-sitter v0.26.0 - AST Parsing

| Attribute | Assessment |
|-----------|------------|
| **Purpose** | Incremental parsing for code analysis |
| **Risk Level** | LOW |
| **Unsafe Code** | Yes (C FFI) |
| **Maintenance** | Active |
| **Audit Priority** | MEDIUM |

**Notes:**
- Used for code understanding
- Well-maintained by tree-sitter project
- C library bindings

---

### 7. ratatui v0.30.0 - Terminal UI

| Attribute | Assessment |
|-----------|------------|
| **Purpose** | Terminal user interface |
| **Risk Level** | LOW |
| **Unsafe Code** | Minimal |
| **Maintenance** | Active |
| **Audit Priority** | LOW |

**Notes:**
- Pure Rust implementation
- Active community
- Good documentation

---

## Unused Dependencies (cargo-machete)

The following dependencies were flagged as potentially unused:

| Crate | Reason | Action |
|-------|--------|--------|
| async-openai | Not yet implemented | Keep for Phase 2 |
| crossterm | Not yet implemented | Keep for Phase 2 |
| genai | Not yet implemented | Keep for Phase 2 |
| keyring | Not yet implemented | Keep for Phase 2 |
| lancedb | Not yet implemented | Keep for Phase 2 |
| monoio | Not yet implemented | Keep for Phase 2 |
| ratatui | Not yet implemented | Keep for Phase 2 |
| rkyv | Not yet implemented | Keep for Phase 2 |
| rusqlite | Not yet implemented | Keep for Phase 2 |
| serde | Not yet implemented | Keep for Phase 2 |
| serde_json | Not yet implemented | Keep for Phase 2 |
| syntect | Not yet implemented | Keep for Phase 2 |
| toml | Not yet implemented | Keep for Phase 2 |
| tree-sitter | Not yet implemented | Keep for Phase 2 |
| uuid | Not yet implemented | Keep for Phase 2 |
| wasmtime | Not yet implemented | Keep for Phase 2 |

**Assessment:** All flagged dependencies are intentional for future phases. No removal recommended.

## Duplicate Version Analysis

### High-Impact Duplicates

| Crate | Versions | Impact | Recommendation |
|-------|----------|--------|----------------|
| hashbrown | 0.14.5, 0.15.5, 0.16.1 | Compile time | Monitor |
| syn | 1.0.117, 2.0.117 | Compile time | Monitor |
| bitflags | 1.3.2, 2.11.0 | Binary size | Low priority |

### Root Causes

1. **lancedb/datafusion** - Brings older versions of many crates
2. **monoio** - Uses older nix and bitflags
3. **wasmtime** - Self-contained dependency tree

## Risk Assessment Summary

| Risk Level | Count | Crates |
|------------|-------|--------|
| HIGH | 3 | monoio, wasmtime, lancedb |
| MEDIUM | 2 | rusqlite, tree-sitter |
| LOW | 16 | All others |

## Recommendations

### Immediate Actions

1. **cargo-vet audits** - Prioritize HIGH risk crates
2. **Monitor unmaintained** - Track for updates or alternatives

### Short-term Actions

1. Consider `rustc-hash` for internal use per HFT SOP
2. Evaluate syntect alternatives to reduce unmaintained deps
3. Profile duplicate versions impact on compile time

### Long-term Actions

1. Pin critical dependency versions in Cargo.toml
2. Establish dependency update policy (weekly/monthly)
3. Create automated vulnerability scanning in CI

## Dependency Update Policy

| Category | Update Frequency | Approval Required |
|----------|------------------|-------------------|
| Security patches | Immediate | Auto-merge |
| Minor versions | Weekly | 1 reviewer |
| Major versions | Monthly | 2 reviewers |
| Unmaintained crates | As needed | Security review |

## Conclusion

The dependency tree is well-structured for the project's requirements. The 2932 total dependencies are primarily from the lancedb/datafusion ecosystem. Four unmaintained transitive dependencies have been identified and documented with no immediate security impact.

**Next Steps:**
1. Run cargo-vet audits on HIGH risk crates
2. Begin Phase 2 implementation using defined dependencies
3. Monitor RUSTSEC advisories for all dependencies
