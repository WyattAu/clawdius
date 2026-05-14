# Clawdius Workspace — crates.io Publishing Readiness Report

**Date:** 2026-05-14
**Workspace version:** 1.0.0-rc.1
**Workspace resolver:** 2

---

## 1. Workspace-level Settings

| Setting | Value |
|---|---|
| `workspace.package.version` | 1.0.0-rc.1 |
| `workspace.package.license` | Apache-2.0 |
| `workspace.package.repository` | https://github.com/clawdius/clawdius |
| `workspace.package.homepage` | https://clawdius.dev |
| `workspace.package.documentation` | https://docs.clawdius.dev |
| `workspace.package.keywords` | ai, llm, coding-assistant, agent, sandbox |
| `workspace.package.categories` | development-tools, command-line-utilities, api-bindings |
| `workspace.package.readme` | README.md |
| `workspace.package.rust-version` | 1.88 |
| `workspace.package.authors` | Clawdius Team <team@clawdius.dev> |
| `publish.workspace` | **NOT SET** (defaults to true — all crates publishable) |

---

## 2. Per-Crate Metadata Checklist

### clawdius-core

| Field | Status | Value |
|---|---|---|
| `publish` | Not set (publishable) | — |
| `description` | OK | "Core library for Clawdius - High-assurance AI coding assistant with multi-tier sandboxing" |
| `license` | OK (workspace) | Apache-2.0 |
| `categories` | OK | development-tools, api-bindings, asynchronous |
| `keywords` | OK | ai, llm, coding-assistant, sandbox, formal-verification |
| `repository` | OK (workspace) | https://github.com/clawdius/clawdius |
| `homepage` | OK (workspace) | https://clawdius.dev |
| `documentation` | OK | https://docs.rs/clawdius-core |
| `readme` | OK | README.md (exists) |
| `rust-version` | **MISSING** | Not set (workspace has 1.88 but crate doesn't inherit) |
| Path deps | None (leaf crate) | — |
| `exclude` | OK | tests, examples, .github, fuzz, *.log |

### clawdius-code

| Field | Status | Value |
|---|---|---|
| `publish` | Not set (publishable) | — |
| `description` | OK | "Clawdius Code - VSCode extension helper binary" |
| `license` | OK (workspace) | Apache-2.0 |
| `categories` | OK | development-tools, command-line-utilities |
| `keywords` | OK | ai, llm, coding-assistant, vscode, editor |
| `repository` | OK (workspace) | https://github.com/clawdius/clawdius |
| `homepage` | OK (workspace) | https://clawdius.dev |
| `documentation` | OK | https://docs.rs/clawdius-code |
| `readme` | OK | README.md (exists) |
| `rust-version` | **MISSING** | Not set |
| Path deps | clawdius-core | `{ path = "../clawdius-core", version = "1.0.0-rc.1" }` |

### clawdius-mcp

| Field | Status | Value |
|---|---|---|
| `publish` | Not set (publishable) | — |
| `description` | OK | "MCP stdio server for Claude Desktop interop" |
| `license` | OK (workspace) | Apache-2.0 |
| `categories` | OK | development-tools, command-line-utilities |
| `keywords` | OK | ai, llm, mcp, claude, coding-assistant |
| `repository` | OK (workspace) | https://github.com/clawdius/clawdius |
| `homepage` | OK (workspace) | https://clawdius.dev |
| `documentation` | OK | https://docs.rs/clawdius-mcp |
| `readme` | OK | README.md (exists) |
| `rust-version` | **MISSING** | Not set |
| Path deps | clawdius-core | `{ path = "../clawdius-core", version = "1.0.0-rc.1" }` |

### clawdius-gateway

| Field | Status | Value |
|---|---|---|
| `publish` | Not set (publishable) | — |
| `description` | OK | "Messaging gateway for Clawdius — routes chat platform messages to the agent" |
| `license` | OK (workspace) | Apache-2.0 |
| `categories` | OK (workspace) | development-tools, command-line-utilities, api-bindings |
| `keywords` | OK (workspace) | ai, llm, coding-assistant, agent, sandbox |
| `repository` | OK (workspace) | https://github.com/clawdius/clawdius |
| `homepage` | OK (workspace) | https://clawdius.dev |
| `documentation` | OK (workspace) | https://docs.clawdius.dev |
| `readme` | OK (workspace) | README.md |
| `rust-version` | **MISSING** | Not set |
| `readme.md` | **MISSING FILE** | No README.md in crates/clawdius-gateway/ |
| Path deps | clawdius-core | `{ path = "../clawdius-core", version = "1.0.0-rc.1" }` |

### clawdius (CLI)

| Field | Status | Value |
|---|---|---|
| `publish` | Not set (publishable) | — |
| `description` | OK | "Clawdius CLI - High-Assurance Rust-Native Engineering Engine" |
| `license` | OK (workspace) | Apache-2.0 |
| `categories` | OK | command-line-utilities, development-tools, asynchronous |
| `keywords` | OK | ai, llm, coding-assistant, cli, tui |
| `repository` | OK (workspace) | https://github.com/clawdius/clawdius |
| `homepage` | OK (workspace) | https://clawdius.dev |
| `documentation` | OK | https://docs.rs/clawdius |
| `readme` | OK | README.md (exists) |
| `rust-version` | **MISSING** | Not set |
| Path deps | clawdius-core, clawdius-gateway | Both `{ path = "..", version = "1.0.0-rc.1" }` |

---

## 3. Dry-Run Results

| Crate | Result | Details |
|---|---|---|
| **clawdius-core** | **PASS** | Packaged 251 files, 3.5 MiB (668.8 KiB compressed). Compiled successfully. 11 test files excluded (expected, via `exclude` list). |
| **clawdius-code** | **FAIL** | `no matching package named clawdius-core found` — clawdius-core must be published first. |
| **clawdius-mcp** | **FAIL** | `no matching package named clawdius-core found` — clawdius-core must be published first. |
| **clawdius-gateway** | **FAIL** | `no matching package named clawdius-core found` — clawdius-core must be published first. |
| **clawdius** | **FAIL** | `no matching package named clawdius-core found` — clawdius-core must be published first. |

**Note:** clawdius also depends on clawdius-gateway, so the publish order matters: core → gateway → (code, mcp, clawdius).

---

## 4. Blockers

### Critical (must fix before publishing)

1. **Publish order dependency** — All 4 non-core crates depend on `clawdius-core` via path deps. `clawdius-core` must be published to crates.io before any other crate can be published. The `clawdius` CLI also depends on `clawdius-gateway`, so gateway must be published before the CLI.
   - Required order: `clawdius-core` → `clawdius-gateway` → `clawdius-code`, `clawdius-mcp`, `clawdius` (last two independent).

2. **`rust-version` not inherited by any crate** — The workspace sets `rust-version = "1.88"` in `[workspace.package]`, but none of the 5 crates use `rust-version.workspace = true`. This means crates.io will not display a MSRV, and users on older Rust compilers will get cryptic compile errors instead of a clear version mismatch message.

3. **Missing README.md for clawdius-gateway** — The crate declares `readme.workspace = true` which resolves to `README.md`, but no `README.md` file exists in `crates/clawdius-gateway/`. This will cause a packaging error at publish time.

### Warnings (non-blocking but recommended)

4. **No `publish` field on any crate** — All crates default to `publish = true`. This is fine if intentional, but consider explicitly setting `publish = true` or `publish.workspace = true` to make intent clear.

5. **`[patch.crates-io]` in root Cargo.toml** — The workspace patches `half` to a vendored local path. This is not included in published crate metadata, but if `clawdius-core` transitively depends on `half`, the published version will resolve from crates.io rather than the vendored version. Verify this doesn't cause build failures for consumers.

6. **Workspace documentation URL differs from crate-level** — The workspace sets `documentation = "https://docs.clawdius.dev"`, but individual crates override with `https://docs.rs/clawdius-*`. This inconsistency is likely intentional (docs.rs is standard for crate-level docs), but worth noting.

---

## 5. Recommendations

1. Add `rust-version.workspace = true` to every crate's `[package]` section.
2. Create `crates/clawdius-gateway/README.md`.
3. Publish in order: `clawdius-core` → `clawdius-gateway` → `clawdius-code` / `clawdius-mcp` / `clawdius` (any order for the last three).
4. Verify the `half` patch doesn't cause issues for downstream consumers after publishing.
5. Consider adding `publish.workspace = true` or explicit `publish` fields for clarity.
6. Run `cargo publish --dry-run -p clawdius-gateway` and `cargo publish --dry-run -p clawdius` after fixing the above to confirm end-to-end.
