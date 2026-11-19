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
| RUSTSEC-2026-0316 | wasmtime | [OK] Fixed | Upgraded to wasmtime 49.0.2 (dynamic record lifting fuel) |
| RUSTSEC-2026-0314 | wasmtime-wasi | [OK] Fixed | Same (guest filesystem datetime overflow panic) |
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
| anymap2 | Unmaintained | Transitive via matrix-sdk 0.16 (optional `matrix` feature) |
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
| RUSTSEC-2026-0346/0349 (rustls 0.22.4), RUSTSEC-2026-0382 (rustls-webpki 0.102.8) | Same `discord` chain as the row above (serenity 0.12 -> tokio-tungstenite 0.21). The active graph uses rustls 0.23.46; 0.22.4 exists only under the optional feature. Blocked on serenity 0.13. |
| RUSTSEC-2026-0292 (imbl-sized-chunks), RUSTSEC-2026-0451/0510 (vodozemac), RUSTSEC-2026-0247 (bitmaps) | `matrix` feature only (matrix-sdk 0.16 -> eyeball-im/imbl/vodozemac 0.9). Not compiled into default or release builds. Blocked on matrix-sdk 0.17 (its reqwest `rustls-tls` -> `rustls` feature rename cascades). |
| RUSTSEC-2026-0367/0368/0369/0429 + 0545/0546/0547/0548 (zip 7.4) | `candle-core` (optional ML stack) pins zip ^7; fixes ship in zip 9.0+. zip is not used by Clawdius code. Blocked on a candle-core release that allows zip 9. Same chain: RUSTSEC-2026-0555 (chrono-tz) and RUSTSEC-2026-0542 (humantime unmaintained) ride the lance/arrow/datafusion vector-db stack. |
| RUSTSEC-2026-0543 (chrono unmaintained) | Direct workspace dependency (0.4.45, serde) used in every crate for timestamps. Advisory is maintenance-status only — no vulnerability in 0.4.x to date. Migration to jiff is a cross-crate project (serde formats, parsing, TZ data); tracked as a standing task rather than a drive-by refactor. |
| RUSTSEC-2026-0002/0253 (lru) | Affect `vector-db`/`mariadb` features only (tantivy / mysql_async -> lru 0.12; the direct workspace dependency is lru 0.18). Unsoundness not triggered in Clawdius usage patterns. Low severity. |

**Review cadence:** Weekly via Dependabot alerts. Re-assessment upon upstream patch releases.
**Contingency:** `[patch.crates-io]` overrides prepared in root Cargo.toml (commented). Activate if upstream does not patch within 90 days of this assessment (2026-06-11).
