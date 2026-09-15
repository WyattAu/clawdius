# Security Policy

## Reporting Vulnerabilities

If you discover a security vulnerability in Clawdius, please report it responsibly:

1. **Email**: security@clawdius.dev
2. **GitHub**: Create a [Security Advisory](https://github.com/WyattAu/clawdius/security/advisories/new)

Please do NOT open public issues for security vulnerabilities.

## Supported Versions

| Version | Supported |
|---------|-----------|
| main branch | [OK] |
| v1.0.0 | [OK] |
| v1.0.0-rc.2 | [OK] |
| v1.0.0-rc.1 | [OK] |
| < v1.0.0-rc.1 | [FAIL] |

## Transitive Dependency Risks

Clawdius has **zero vulnerabilities in its direct dependencies**. All known CVEs are in transitive (indirect) dependencies. Below is the full inventory with mitigations.

### Resolved

| Advisory | Crate | Status | Mitigation |
|----------|-------|--------|------------|
| RUSTSEC-2026-0114 | wasmtime | [OK] Fixed | Upgraded to wasmtime 44.x |
| RUSTSEC-2025-0065 | matrix-sdk-base | [OK] Fixed | Upgraded matrix-sdk 0.10 → 0.16 (matrix-sdk-base 0.16.1); `matrix` feature |
| RUSTSEC-2025-0135 | matrix-sdk-base | [OK] Fixed | Same |

### Pending (Upstream Blocked)

| Advisory | Crate | Severity | Chain | Mitigation |
|----------|-------|----------|-------|------------|
| RUSTSEC-2026-0049 | rustls-webpki | Medium | serenity → tokio-tungstenite 0.21 → rustls 0.22 → rustls-webpki 0.102 | Optional `discord` feature only; requires serenity 0.13 (unreleased) |
| RUSTSEC-2026-0098 | rustls-webpki | Medium | Same chain | Same |
| RUSTSEC-2026-0099 | rustls-webpki | Medium | Same chain | Same |
| RUSTSEC-2026-0104 | rustls-webpki | Medium | Same chain | Same |
| RUSTSEC-2026-0002 | lru | Low | tantivy / mysql_async → lru 0.12 (optional `vector-db`/`mariadb` features) | Optional features only; `IterMut` unsound, not triggered in Clawdius usage |
| RUSTSEC-2026-0253 | lru | Low | Same chain | Optional features only; `pop()` panic-safety unsoundness |

### Unmaintained (Informational)

| Crate | Note | Impact |
|-------|------|--------|
| async-std | Discontinued | Test-only dependency via httpmock |
| bincode | Unmaintained | Transitive via syntect |
| paste | Unmaintained | Transitive via tokenizers, candle |
| yaml-rust | Unmaintained | Transitive via syntect |
| rustls-pemfile | Unmaintained | Transitive via mysql_async |
| number_prefix | Unmaintained | Transitive via indicatif |
| bitmaps | Unmaintained | Transitive via imbl (matrix-sdk 0.16) |
| proc-macro-error / proc-macro-error2 | Unmaintained | Transitive via gtk/glib-macros (clawdius-tauri linux GUI) |
| unic-* family | Unmaintained | Transitive via urlpattern |

### Default Install Risk

The **default build** (`cargo build --release -p clawdius`) does NOT include any of the affected transitive dependencies. The vulnerable crates are only pulled in when optional features are enabled:

- `discord` feature → rustls-webpki 0.102 (4 CVEs)
- `vector-db` / `mariadb` features → lru 0.12 (2 unsound)

Users who do not enable these features are not affected.

## Security Features

- **`#![deny(unsafe_code)]`** — ~11 unsafe blocks across 3 files (simd.rs, proof/templates.rs, analysis/drift.rs)
- **Shell sandboxing** — Blocked command patterns, timeout limits, directory restrictions
- **Refuse-by-default sandbox floor** — When no real isolation backend (bubblewrap, Docker/Podman,
  gVisor, sandbox-exec) is available, sandboxed command execution fails with
  `Error::SandboxUnavailable` and remediation guidance instead of silently degrading to the
  `filtered` backend (a command blocklist that is trivially bypassed via flag reordering,
  interpreter eval, or string-embedded payloads). Unisolated execution requires an explicit
  `allow_unisolated = true` opt-in under `[shell_sandbox]` in `clawdius.toml` — treat that flag
  as dangerous and never enable it for untrusted, LLM-proposed commands.
- **No hardcoded secrets** — All API keys loaded from environment variables or OS keychain
- **No telemetry** — Zero data sent to external servers without explicit user consent
- **WASM isolation** — Brain execution sandboxed via wasmtime (feature-gated)
- **TLS everywhere** — All network connections use TLS via rustls

## Risk Acceptance Statement

The following transitive CVEs are accepted under documented risk assessment:

| Risk | Acceptance Rationale |
|------|---------------------|
| RUSTSEC-2026-0049/0098/0099/0104 (rustls-webpki) | Affects `discord` feature only (serenity -> tokio-tungstenite -> rustls -> rustls-webpki). Default build is unaffected. Certificate validation edge cases; not exploitable in Clawdius's Discord bot token authentication flow. Blocked on serenity 0.13 (unreleased). |
| RUSTSEC-2026-0002/0253 (lru) | Affect `vector-db`/`mariadb` features only (tantivy / mysql_async -> lru 0.12; the direct workspace dependency is lru 0.18). Unsoundness not triggered in Clawdius usage patterns. Low severity. |

**Review cadence:** Weekly via Dependabot alerts. Re-assessment upon upstream patch releases.
**Contingency:** `[patch.crates-io]` overrides prepared in root Cargo.toml (commented). Activate if upstream does not patch within 90 days of this assessment (2026-06-11).
