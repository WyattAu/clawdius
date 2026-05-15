# API Stability

Clawdius follows semantic versioning for its public APIs.

## Versioning

The current version is `1.0.0-rc.1`. The crate versions are managed at the workspace level in `Cargo.toml`:

```toml
[workspace.package]
version = "1.0.0-rc.1"
```

## Stability Guarantees

### Public API Surface

The following types and modules are considered part of the stable public API:

| Module | Exports |
|--------|---------|
| `clawdius_core::Config` | Configuration loading and saving |
| `clawdius_core::SessionManager` | Session lifecycle management |
| `clawdius_core::error::Error` | Unified error type |
| `clawdius_core::error::Result` | Result type alias |
| `clawdius_core::llm` | LLM client and configuration |
| `clawdius_core::session` | Session types and storage |
| `clawdius_core::context` | Context building and mentions |
| `clawdius_core::output::OutputFormat` | Output format enum |
| `clawdius_core::diff` | Diff types and rendering |
| `clawdius_core::timeline` | Timeline and checkpoints |
| `clawdius_core::completions` | Code completion handler |
| `clawdius_core::config::KeyringStorage` | Keyring storage (feature-gated) |

### Breaking Changes

Breaking changes to the public API will only occur with a major version bump (following SemVer). The project is currently at `rc.1`, indicating the API is nearing stability but may still change.

## Unsafe Code Policy

```toml
[workspace.lints.rust]
unsafe_code = "deny"
```

All unsafe code is denied at the workspace level. This is a hard constraint across all crates.

## Lint Policy

```toml
[workspace.lints.rust]
missing_docs = "warn"
rust_2018_idioms = { level = "warn", priority = -1 }

[workspace.lints.clippy]
pedantic = { level = "warn", priority = -1 }
unwrap_used = "warn"
expect_used = "warn"
panic = "warn"
todo = "warn"
```

CI enforces stricter checks than the baseline workspace lints.

## Feature Flags

Feature flags are considered part of the API contract. Adding a new feature flag is not a breaking change. Removing or changing the behavior of an existing flag requires a major version bump.

See the [Rust API](./rust.md) page for the full feature flag reference.

## Error Type Stability

The `Error` enum is extended with new variants as non-breaking additions. Existing variants and their `Display` messages are stable.

## CLI Stability

CLI command names, flags, and argument formats are part of the public API. Changes follow SemVer conventions.

## Deprecation Policy

Deprecated APIs will emit warnings for at least one minor version before removal. Deprecated items are documented in the changelog.
