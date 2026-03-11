# API Stability Guarantee

**Version:** 1.0.0  
**Effective Date:** 2026-03-11  
**Status:** Binding Commitment

---

## Summary

Clawdius v1.0.0 and all subsequent minor releases (1.x.x) commit to **Semantic Versioning (SemVer 2.0)** for all public APIs. Breaking changes will only be introduced in major version releases (2.0.0, 3.0.0, etc.).

---

## What Is Covered

### ✅ Stable APIs (SemVer Guaranteed)

The following are covered by our stability guarantee:

#### 1. Public Rust API (`clawdius-core`)

All items marked `pub` in the following modules:

| Module | Stability | Notes |
|--------|-----------|-------|
| `llm::*` | ✅ Stable | Provider traits, message types, streaming |
| `session::*` | ✅ Stable | Session management, storage, compaction |
| `tools::*` | ✅ Stable | Tool traits, execution, results |
| `sandbox::*` | ✅ Stable | Backend traits, configuration, execution |
| `config::*` | ✅ Stable | Configuration structures, loading |
| `error::*` | ✅ Stable | Error types and conversions |
| `checkpoint::*` | ✅ Stable | Timeline, checkpoints, rollback |
| `graph_rag::*` | ✅ Stable | Symbol extraction, relationships, search |
| `enterprise::*` | ✅ Stable | SSO, audit, compliance, teams |
| `plugin::*` | ✅ Stable | Plugin API, hooks, marketplace |

#### 2. Configuration File Schemas

All TOML configuration files:

| File | Stability |
|------|-----------|
| `.clawdius/config.toml` | ✅ Stable |
| `.clawdius/providers.toml` | ✅ Stable |
| `.clawdius/modes.toml` | ✅ Stable |
| `.clawdius/plugins/plugin.toml` | ✅ Stable |

#### 3. CLI Interface

All command-line arguments and output formats:

| Interface | Stability |
|-----------|-----------|
| `clawdius chat` | ✅ Stable |
| `clawdius config` | ✅ Stable |
| `clawdius --json` | ✅ Stable |
| Exit codes (0, 1, 2) | ✅ Stable |
| JSON output schema | ✅ Stable |

#### 4. RPC Protocol

JSON-RPC interface for editor integration:

| Component | Stability |
|-----------|-----------|
| Method names | ✅ Stable |
| Request/Response schemas | ✅ Stable |
| Notification events | ✅ Stable |

#### 5. Plugin API

WASM plugin interface:

| Component | Stability |
|-----------|-----------|
| `Plugin` trait | ✅ Stable |
| `HookType` enum | ✅ Stable |
| `HookContext` struct | ✅ Stable |
| `PluginManifest` schema | ✅ Stable |

---

## What Is NOT Covered

### ⚠️ Unstable APIs (No Guarantees)

The following may change without notice:

| Category | Examples |
|----------|----------|
| **Internal modules** | Any `pub` item in `src/internal/` or marked `#[doc(hidden)]` |
| **Private fields** | Struct fields not marked `pub` |
| **Test utilities** | Anything in `#[cfg(test)]` modules |
| **Build internals** | `build.rs` outputs, generated code |
| **Debug output** | Log formats, trace messages |
| **Experimental features** | Features behind `experimental-*` flags |

### 🔬 Experimental Features

Features in preview may change:

```toml
# Cargo.toml
[features]
experimental-mcp = []        # Model Context Protocol (preview)
experimental-wasm-webview = [] # WASM webview UI (preview)
```

These features are excluded from stability guarantees until promoted to stable.

---

## Version Numbering

Clawdius follows [SemVer 2.0.0](https://semver.org/):

```
MAJOR.MINOR.PATCH

MAJOR: Breaking changes (incompatible API changes)
MINOR: New features (backward-compatible)
PATCH: Bug fixes (backward-compatible)
```

### Examples

| Change Type | Version Bump | Example |
|-------------|--------------|---------|
| Add new LLM provider | MINOR | 1.0.0 → 1.1.0 |
| Add new config field | MINOR | 1.1.0 → 1.2.0 |
| Fix bug in session save | PATCH | 1.2.0 → 1.2.1 |
| Remove deprecated API | MAJOR | 1.2.1 → 2.0.0 |
| Change function signature | MAJOR | 1.2.1 → 2.0.0 |
| Add optional parameter | MINOR | 1.2.0 → 1.3.0 |

---

## Deprecation Policy

When we need to remove or change an API:

### Timeline

| Phase | Duration | Action |
|-------|----------|--------|
| **Announcement** | Immediate | Add `#[deprecated]` attribute |
| **Warning Period** | 2 minor releases | API still works, emits warning |
| **Removal** | Next major release | API removed |

### Example

```rust
// v1.2.0 - Deprecation announced
#[deprecated(
    since = "1.2.0",
    note = "Use `Session::new_with_config()` instead",
    suggestion = "Session::new_with_config"
)]
pub fn new() -> Self { ... }

// v1.3.0 - Still available with warning
// v2.0.0 - Removed
```

### Deprecation Tracking

All deprecations are tracked in:
- [CHANGELOG.md](../CHANGELOG.md) - Each release lists deprecations
- [MIGRATION.md](./MIGRATION.md) - Migration guides between versions

---

## Breaking Change Process

For changes that require a major version bump:

1. **Proposal** - Open a "Breaking Change Proposal" issue
2. **Discussion** - Minimum 30-day public comment period
3. **Decision** - Maintainer approval required
4. **Documentation** - Update MIGRATION.md with guide
5. **Release** - Include in major version release notes

---

## Stability Exceptions

### Security Fixes

Security vulnerabilities may require immediate breaking changes:

- Critical security issues bypass the normal process
- Changes will be documented in security advisories
- Migration path provided when possible

### Bug Fixes That Change Behavior

Some bug fixes may technically be breaking:

- Behavior that contradicts documented API is considered a bug
- Fixing such bugs is not considered a breaking change
- Changes will be clearly documented in release notes

---

## Commitment

The Clawdius maintainers commit to:

1. **No surprise breakage** - Breaking changes only in major releases
2. **Clear migration paths** - Every breaking change documented
3. **Reasonable deprecation windows** - Minimum 2 minor releases
4. **Transparent process** - All changes discussed publicly

---

## Contact

For questions about API stability:

- **GitHub Issues:** [github.com/clawdius/clawdius/issues](https://github.com/clawdius/clawdius/issues)
- **GitHub Discussions:** [github.com/clawdius/clawdius/discussions](https://github.com/clawdius/clawdius/discussions)
- **Discord:** [discord.gg/clawdius](https://discord.gg/clawdius)

---

*This document is part of the Clawdius v1.0.0 release and represents a binding commitment to our users.*
