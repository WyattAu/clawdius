# Security Audit Report — Clawdius v1.6.0

**Date:** 2026-04-05
**Scanner:** Manual audit + cargo audit + cargo deny + ripgrep static analysis
**Scope:** `crates/` (clawdius, clawdius-core, clawdius-server, clawdius-code, clawdius-webview)

---

## Executive Summary

| # | Category | Status | Highest Risk |
|---|----------|--------|-------------|
| 1 | Dependency advisories (cargo audit) | **WARNING** | Medium |
| 2 | Dependency policy (cargo deny) | **PASS** | — |
| 3 | Hardcoded secrets | **FAIL** | High |
| 4 | Unsafe code usage | **PASS** (test-only) | Low |
| 5 | Command injection vectors | **WARNING** | Medium |
| 6 | SQL injection vectors | **PASS** | Low |
| 7 | Path traversal vectors | **WARNING** | Medium |
| 8 | Risky dependency tree | **WARNING** | Medium |
| 9 | Debug/test code in release | **PASS** | — |
| 10 | Environment variable usage | **PASS** | Low |

**Overall: 3 PASS, 5 WARNING, 2 FAIL**

---

## 1. Dependency Advisories (cargo audit)

**Status:** WARNING — 9 warnings, 0 vulnerabilities

### Unmaintained Crates (7)

| Crate | Version | Advisory | Ingested Via |
|-------|---------|----------|-------------|
| `async-std` | 1.13.2 | RUSTSEC-2025-0052 (discontinued) | httpmock (dev-dep) |
| `bincode` | 1.3.3 | RUSTSEC-2025-0141 (unmaintained) | syntect |
| `fxhash` | 0.2.1 | RUSTSEC-2025-0057 (unmaintained) | monoio |
| `number_prefix` | 0.4.0 | RUSTSEC-2025-0119 (unmaintained) | indicatif |
| `paste` | 1.0.15 | RUSTSEC-2024-0436 (unmaintained) | tokenizers, leptos, gemm |
| `proc-macro-error` | 1.0.4 | RUSTSEC-2024-0370 (unmaintained) | syn_derive / rstml / leptos |
| `yaml-rust` | 0.4.5 | RUSTSEC-2024-0320 (unmaintained) | syntect |

### Unsound Crates (2)

| Crate | Version | Advisory | Ingested Via |
|-------|---------|----------|-------------|
| `lexical-core` | 0.8.5 | RUSTSEC-2023-0086 (soundness issues) | arrow (arrow-json, arrow-csv, arrow-cast) |
| `lru` | 0.12.5 | RUSTSEC-2026-0002 (IterMut Stacked Borrows violation) | tantivy → lance-index → lancedb |

### Risk Rating: **Medium**

Most unmaintained crates are leaf dependencies in `syntect`, `monoio`, and the `leptos`/`tokenizers` ecosystems. The `async-std` finding is limited to `httpmock` (a dev-dependency). The two **unsound** advisories (`lexical-core`, `lru`) are transitive through `arrow` and `tantivy`/`lancedb` and may be exploitable in edge cases involving malformed data.

### Recommendations
- `yaml-rust` → migrate to `yaml-rust2` (drop-in replacement) or press `syntect` upstream.
- `bincode` → monitor for `bincode` v2 adoption by `syntect`.
- `lexical-core` / `lru` → track upstream arrow/lancedb updates; these are deep transitive deps requiring ecosystem-level fixes.
- `async-std` → no action needed (dev-only via httpmock).

---

## 2. Dependency Policy (cargo deny)

**Status:** PASS

```
advisories ok, bans ok, licenses ok, sources ok
```

All deny checks pass cleanly. No duplicate crates, no banned licenses, all sources are trusted registries.

### Risk Rating: **N/A — PASS**

---

## 3. Hardcoded Secrets

**Status:** FAIL — 13 hardcoded secret values found

### Findings

#### 3a. Test/example config secrets (Medium)

**File:** `crates/clawdius-core/src/messaging/config_builder.rs` (lines 349-377)

Contains example TOML config with hardcoded placeholder tokens:
```
secret_token = "my-tg-secret"
bot_token = "123456:ABC-DEF"
discord_bot_token = "discord-bot-token"
access_token = "syt_abc123"
signing_secret = "slack-secret"
slack_bot_token = "xoxb-slack-bot"
token = "rc-token"
verification_token = "signal-verify"
verify_token = "wa-verify"
app_secret = "wa-secret"
whatsapp_access_token = "wa-token"
```

These appear to be example/fixture data for config builder tests, but could be mistaken for real credentials.

#### 3b. Hardcoded webhook secret (Medium)

**File:** `crates/clawdius-core/src/webhooks/types.rs:399`
```rust
let secret = "my-secret";
```

Used in test/fixture context but the literal `"my-secret"` should use a well-known test sentinel.

#### 3c. Hardcoded password (Low)

**File:** `crates/clawdius-core/src/analysis/debt.rs:892`
```rust
let password = "secret123";
```

Appears in a test fixture. Low risk but bad hygiene.

### Risk Rating: **High** (reputation + supply-chain risk if examples are copy-pasted to production)

### Recommendations
- Replace all hardcoded secret literals with `"<PLACEHOLDER>"` or `env!("CARGO_TEST_SECRET")` patterns.
- Add a CI check (e.g., `detect-secrets` or `gitleaks`) to prevent future commits of secret-like strings.
- Tag config_builder examples with `// DO NOT USE IN PRODUCTION` comments.

---

## 4. Unsafe Code Usage

**Status:** PASS — All `unsafe` blocks are test-only env var manipulation

### Findings

All 14 instances of `unsafe` are in `crates/clawdius/tests/integration_tests.rs`, exclusively for:
```rust
unsafe { std::env::set_var("ANTHROPIC_API_KEY", "test-anthropic-key"); }
unsafe { std::env::remove_var("ANTHROPIC_API_KEY"); }
```

This is the standard Rust pattern for modifying process environment in tests (required since `std::env::set_var` is `unsafe` in recent Rust editions).

No `unsafe` blocks exist in production code.

### Risk Rating: **Low** — Test-only, idiomatic usage.

---

## 5. Command Injection Vectors

**Status:** WARNING — Shell tool accepts arbitrary commands

### Findings

#### 5a. Shell tool — arbitrary command execution (High)

**File:** `crates/clawdius-core/src/tools/shell.rs`
```rust
let mut command = Command::new(shell);
command.arg(flag).arg(&params.command);
```

The shell tool directly passes user-provided `params.command` as an argument to a shell binary. This is by design (it IS a shell tool), but represents the highest-risk attack surface.

#### 5b. CLI git commands — user-influenced arguments (Low)

**File:** `crates/clawdius/src/cli.rs`

Multiple `Command::new("git")` calls where user-provided file paths and commit messages are passed as arguments (not via shell). Using `.args()` (not `.arg(format!(...))`) mitigates shell injection, but:
- `commit_message` at line ~2458 is user-generated and passed to `git commit -m`.
- `files_to_stage` are user-specified.

#### 5c. Sandbox backends (Medium)

**Files:** `crates/clawdius-core/src/sandbox/backends/` (gvisor, firecracker, bubblewrap, sandbox-exec, container, direct, filtered)

Sandbox backends construct commands with user-provided arguments. The `direct.rs` and `filtered.rs` backends are particularly risky as they pass `command` and `args` strings directly to `Command::new(command).args(args)`.

#### 5d. LSP client (Low)

**File:** `crates/clawdius-core/src/lsp/client.rs`
```rust
let mut cmd = Command::new(&self.config.command);
cmd.args(&self.config.args);
```

LSP command and args come from configuration, not direct user input.

### Risk Rating: **Medium** (shell tool is intentional; sandbox backends need review)

### Recommendations
- Shell tool: Ensure this is only accessible after explicit user authorization / confirmation. Add rate limiting and command allowlisting if possible.
- Sandbox backends: Validate `command` and `args` against an allowlist before execution. The `filtered.rs` backend should implement stricter command validation.
- CLI git commands: Sanitize commit messages (strip shell metacharacters) even though `.args()` provides some protection.

---

## 6. SQL Injection Vectors

**Status:** PASS — All queries use parameterized statements

### Findings

**File:** `crates/clawdius-core/src/messaging/state_store.rs`

All SQL queries use `format!` only for table name interpolation with quoted identifiers:
```rust
format!("DELETE FROM \"{table}\" WHERE expires_at IS NOT NULL AND expires_at <= ?")
format!("SELECT value, expires_at FROM \"{table}\" WHERE key = ?")
format!("INSERT OR REPLACE INTO \"{table}\" (key, value, expires_at) VALUES (?, ?, ?)")
```

User values are passed via `?` parameterized placeholders (Rusqlite bind parameters), not string interpolation.

### Residual Risk: **Low**

Table names are interpolated via `format!`. If table names are ever derived from untrusted user input, this could be exploitable. Currently table names appear to be static/config-defined values.

### Recommendations
- Validate table names against a regex (`^[a-zA-Z_][a-zA-Z0-9_]*$`) before interpolation.
- Consider using an enum or const for allowed table names.

---

## 7. Path Traversal Vectors

**Status:** WARNING — File tools accept user-provided paths

### Findings

#### 7a. File tools — no path validation (Medium)

**File:** `crates/clawdius-core/src/tools/file.rs`
```rust
let path = Path::new(&params.path);
```

`read`, `write`, `edit`, and `list` methods all accept user-provided paths without canonicalization or sandbox boundary checks. An LLM-generated path like `../../etc/shadow` or `C:\Windows\System32\config` could be accessed.

#### 7b. CLI path handling (Low)

**File:** `crates/clawdius/src/cli.rs`
```rust
let clawdius_dir = workspace_path.join(".clawdius");
```

Workspace-relative paths are used, which is safe.

#### 7c. Server path handling (Low)

**File:** `crates/clawdius-server/src/main.rs`
```rust
Config::load(std::path::Path::new(path))?
SessionStore::open(std::path::Path::new(path))?
```

Paths come from CLI arguments / config, not from untrusted network input.

### Risk Rating: **Medium** — File tools are the primary concern.

### Recommendations
- Implement path canonicalization and boundary checks in file tools (ensure resolved path is within the workspace directory).
- Reject paths containing `..` components or symlinks pointing outside the workspace.
- Add a configuration option for allowed root directories.

---

## 8. Risky Dependency Tree

**Status:** WARNING — 71 direct+transitive dependencies, several heavy crates

### Notable Direct Dependencies

| Crate | Notes |
|-------|-------|
| `monoio` 0.2.4 | io_uring-based async runtime (Linux-only, requires `fxhash` — unmaintained) |
| `syntect` 5.3.0 | Syntax highlighting (pulls in `bincode`, `yaml-rust` — both unmaintained) |
| `lancedb` 0.27.1 | Vector DB (pulls in `datafusion`, `tantivy`, `lru` — unsound) |
| `candle-core` 0.4.1 | ML inference (pulls in `gemm` ecosystem with `paste`) |
| `leptos` 0.6.15 | WASM frontend framework (pulls in `paste`, `proc-macro-error`) |
| `reqwest` 0.13.2 | HTTP client (well-maintained, rustls-based) |
| `rpassword` 7.4.0 | Password prompt (uses `rtoolbox` — low risk) |

### Risk Rating: **Medium** — Large dependency surface but no known exploited vulnerabilities.

### Recommendations
- Consider whether `monoio` is necessary alongside `tokio` (dual async runtime complexity).
- Monitor `lancedb` and `arrow` for updates that resolve `lexical-core` and `lru` advisories.
- Lock file is present — continue pinning exact versions.

---

## 9. Debug/Test Code in Release

**Status:** PASS — All test/debug-gated code is properly attributed

### Findings

~140 files contain `#[cfg(test)]` or `#[cfg(debug_assertions)]` annotations. All are in test modules or debug-only code paths that will not be compiled in release builds. This is normal Rust practice.

### Risk Rating: **N/A — PASS**

---

## 10. Environment Variable Usage

**Status:** PASS — Proper patterns observed

### Findings

| Variable | File | Usage |
|----------|------|-------|
| `ANTHROPIC_API_KEY` | `clawdius-core/src/llm.rs` | API key resolution |
| `OPENAI_API_KEY` | `clawdius-core/src/llm.rs` | API key resolution |
| `OLLAMA_BASE_URL` / `OLLAMA_HOST` | `clawdius-core/src/llm.rs`, `clawdius/src/cli.rs` | Local LLM config |
| `CLAWDIUS_JWT_SECRET` | `clawdius-server/src/main.rs` | JWT signing key |
| `SENTRY_DSN` | `clawdius-core/src/telemetry/crash.rs` | Error reporting |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | `clawdius-server/src/otel.rs` | Observability |
| `LANG` / `LC_ALL` / `LC_MESSAGES` | `clawdius-core/src/i18n.rs` | Locale detection |

All secrets are read via `std::env::var()` (never hardcoded). The `CLAWDIUS_JWT_SECRET` falls back to config file with a warning to use the env var in production.

### Risk Rating: **Low** — Good patterns. The empty-string fallback for JWT secret is the only concern.

### Recommendations
- Consider refusing to start the server if `CLAWDIUS_JWT_SECRET` is empty rather than falling back to an empty config value.
- Document all expected environment variables in a single location.

---

## Summary of Recommendations (Priority Order)

| Priority | Action |
|----------|--------|
| **P0** | Add path traversal protection to file tools (`crates/clawdius-core/src/tools/file.rs`) |
| **P0** | Validate shell tool authorization and add rate limiting |
| **P1** | Replace hardcoded secrets in config_builder examples with obvious placeholders |
| **P1** | Add `gitleaks` or `detect-secrets` to CI pipeline |
| **P1** | Reject empty JWT secret at server startup |
| **P2** | Validate table names in state_store.rs before SQL interpolation |
| **P2** | Track `lexical-core` and `lru` unsound advisories via upstream updates |
| **P2** | Validate command strings in sandbox `filtered.rs` and `direct.rs` backends |
| **P3** | Press upstream to replace `yaml-rust` with `yaml-rust2` in syntect |
| **P3** | Evaluate necessity of `monoio` alongside `tokio` |
