# Security Policy

## Reporting Vulnerabilities

If you discover a security vulnerability in Clawdius, please report it responsibly:

1. **Email**: security@clawdius.dev
2. **GitHub**: Create a [Security Advisory](https://github.com/WyattAu/clawdius/security/advisories/new)

Please do NOT open public issues for security vulnerabilities.

## Supported Versions

| Version | Supported |
|---------|-----------|
| main branch | ✅ |
| v1.0.0-rc.1 | ✅ |
| < v1.0.0 | ❌ |

## Transitive Dependency Risks

Clawdius has **zero vulnerabilities in its direct dependencies**. All known CVEs are in transitive (indirect) dependencies. Below is the full inventory with mitigations.

### Resolved

| Advisory | Crate | Status | Mitigation |
|----------|-------|--------|------------|
| RUSTSEC-2026-0114 | wasmtime | ✅ Fixed | Upgraded to wasmtime 44.x |

### Pending (Upstream Blocked)

| Advisory | Crate | Severity | Chain | Mitigation |
|----------|-------|----------|-------|------------|
| RUSTSEC-2026-0049 | rustls-webpki | Medium | serenity → tokio-tungstenite 0.21 → rustls 0.22 → rustls-webpki 0.102 | Optional `discord` feature only; requires serenity 0.13 (unreleased) |
| RUSTSEC-2026-0098 | rustls-webpki | Medium | Same chain | Same |
| RUSTSEC-2026-0099 | rustls-webpki | Medium | Same chain | Same |
| RUSTSEC-2026-0104 | rustls-webpki | Medium | Same chain | Same |
| RUSTSEC-2025-0065 | matrix-sdk-base | Medium | matrix-sdk 0.10 (optional `matrix` feature) | Optional `matrix` feature only; requires matrix-sdk 0.16+ |
| RUSTSEC-2025-0135 | matrix-sdk-base | Medium | Same chain | Same |
| RUSTSEC-2026-0002 | lru | Low | tantivy → lru 0.12 (optional `vector-db` feature) | Optional `vector-db` feature only; `IterMut` unsound, not triggered in Clawdius usage |

### Unmaintained (Informational)

| Crate | Note | Impact |
|-------|------|--------|
| async-std | Discontinued | Test-only dependency via httpmock |
| backoff | Unmaintained | Transitive via matrix-sdk |
| bincode | Unmaintained | Transitive via syntect |
| paste | Unmaintained | Transitive via tokenizers, candle |
| yaml-rust | Unmaintained | Transitive via syntect |
| rustls-pemfile | Unmaintained | Transitive via mysql_async |
| number_prefix | Unmaintained | Transitive via indicatif |
| instant | Unmaintained | Transitive via backoff |

### Default Install Risk

The **default build** (`cargo build --release -p clawdius`) does NOT include any of the affected transitive dependencies. The vulnerable crates are only pulled in when optional features are enabled:

- `discord` feature → rustls-webpki 0.102 (4 CVEs)
- `matrix` feature → matrix-sdk-base 0.10 (2 CVEs)
- `vector-db` feature → lru 0.12 (1 unsound)

Users who do not enable these features are not affected.

## Security Features

- **`#![deny(unsafe_code)]`** — Zero unsafe code in production (all 8 blocks isolated in `simd.rs`)
- **Shell sandboxing** — Blocked command patterns, timeout limits, directory restrictions
- **No hardcoded secrets** — All API keys loaded from environment variables or OS keychain
- **No telemetry** — Zero data sent to external servers without explicit user consent
- **WASM isolation** — Brain execution sandboxed via wasmtime (feature-gated)
- **TLS everywhere** — All network connections use TLS via rustls
