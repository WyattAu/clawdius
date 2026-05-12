# clawdius-core Public API Stability Audit

**Date:** 2026-05-11
**Scope:** `crates/clawdius-core/src/`

---

## Summary Metrics

| Metric | Count |
|---|---|
| Total `pub` items (struct/enum/fn/trait/type/const/static/mod) | **235** |
| Files containing public items | **200** |
| Items with `#[doc(hidden)]` | **0** |
| Re-exports in `lib.rs` | **23** |
| Public modules declared in `lib.rs` | **46** |
| Files with `#[deprecated]` / `#[non_exhaustive]` / `#[cfg(test)]` | **144** (includes internal test cfgs) |

---

## Current Re-export Surface (lib.rs)

The following items are deliberately re-exported at the crate root:

```
agents::{...}, api::{ApiConfig, ApiGateway, ChatRequest, ChatResponse, HealthResponse},
config::Config, context::{AggregatedContext, ContextAggregator, ...},
diff::{DiffPreview, DiffRenderer, DiffStats, DiffTheme, FileDiff},
error::{EnhancedError, Error, ErrorHelpers, Result},
memory::{MemoryEntry, MemoryMetadata, ProjectMemory},
onboarding::{Onboarding, OnboardingStatus}, output::OutputFormat,
proof::{LeanVerifier, ProofDefinition, ProofTemplate},
retry::{with_retry_and_circuit, CircuitBreaker, CircuitState},
session::{Session, SessionManager, SessionStore},
skills::{Skill, SkillContext, SkillError, SkillMeta, SkillRegistry, SkillResult},
storage::{...}, telemetry::{CrashReporter, TelemetryConfig},
timeline::{CheckpointId, TimelineManager}, workspace::{IndexStats, WorkspaceIndexer},
agentic::{...}, analysis::{...}
```

---

## Critical Issues

### 1. Zero `#[doc(hidden)]` usage (Severity: HIGH)

Not a single item uses `#[doc(hidden)]`. This means all 235 public items are part of the semver-stable API surface, including internal implementation details.

### 2. Dead/test code exposed publicly (Severity: HIGH)

| Item | Location | Issue |
|---|---|---|
| `pub fn calculate(x: i32, y: i32)` | `graph_rag/repo_map.rs:490` | Dead test code |
| `pub fn main()` | `graph_rag/repo_map.rs:495` | Binary entry point in library crate |
| `pub fn add(a: i32, b: i32)` | `workspace/context.rs:232` | Dead test code |
| `pub fn normalize(vec: &mut [f32])` | `graph_rag/embedding/openai_api.rs:200` | Internal utility, should be `pub(crate)` or `#[doc(hidden)]` |
| `pub fn levenshtein(a: &str, b: &str)` | `tools/edit_cascade.rs:689` | Internal helper |
| `pub fn format_results_for_llm(...)` | `tools/web_search.rs:457` | Internal formatting helper |
| `pub fn format_grounded_response(...)` | `tools/web_search.rs:474` | Internal formatting helper |
| `pub fn ensure_tenant_columns(...)` | `session/isolation.rs:242` | Migration helper, should be `pub(crate)` |

### 3. No `#[non_exhaustive]` on public structs/enums (Severity: MEDIUM)

None of the 235 public items use `#[non_exhaustive]`, meaning adding fields or variants to any public type is a breaking change.

### 4. Internal types exposed publicly (Severity: MEDIUM)

Types that are implementation details but publicly accessible:

- `graph_rag::repo_map::utils` and `graph_rag::repo_map::helpers` — submodules of repo_map
- `ConsoleLog`, `DialogInfo` in `tools/browser.rs` — browser-internal state
- `DiffCalculator` in `timeline/change_tracker.rs` — internal unit struct
- `LegacyMetricsSnapshot` in `telemetry/metrics.rs` — deprecated, should be hidden
- `proof::types::LeanError`, `LeanErrorSeverity` — verifier-internal diagnostics
- `proof::types::TemplateError` — template loading error, internal detail
- `mcp::handler::handle_mcp_request` — handler function, should be called through the protocol layer
- `output::stream::ChangeType` — duplicates `output::format::ChangeType`
- `output::stream::TokenUsageFinal` — streaming-internal type
- `config::keyring_storage` — submodule of config, implementation detail

### 5. Duplicate type names (Severity: LOW)

| Name | Locations |
|---|---|
| `FileDiff` | `diff/mod.rs:20`, `timeline/mod.rs:95` |
| `Diff` | `timeline/mod.rs:82`, `diff/mod.rs` (module) |
| `ChangeType` | `output/format.rs:194`, `output/stream.rs:111`, `timeline/change_tracker.rs:39` |
| `ContextAggregator` | `context/aggregator.rs:42`, `workspace/aggregator.rs:9` |
| `AggregatedContext` | `context/aggregator.rs:16`, `workspace/aggregator.rs:62` |
| `FileInfo` | `context/window_manager.rs:21`, `graph_rag/ast.rs:47` |
| `SymbolKind` | `graph_rag/ast.rs:60`, `lsp/protocol.rs:429` |
| `Diagnostic` | `context/mod.rs:197`, `lsp/protocol.rs:167` |
| `SearchResult` | `context/mod.rs:227`, `graph_rag/vector.rs:35`, `tools/web_search.rs:47` |
| `Quota` | `usage.rs:132`, `orchestrator/resource_governor.rs:14` |
| `SessionRepository` trait | `session/repository.rs:5`, `storage/backend.rs:27` |
| `TimelineCheckpoint` | `telemetry/structured.rs:452`, `timeline/store.rs:63` |

### 6. Massive public module tree (Severity: MEDIUM)

`lib.rs` exports **46 public modules**, many of which are internal subsystems that should not be part of the stable API:

- `messaging` (discord, matrix, telegram) — messaging platform implementations
- `rpc` — internal RPC server
- `api` — HTTP gateway implementation
- `sandbox::backends` — sandbox implementation details
- `llm::providers` — individual provider implementations (8 providers)
- `storage` — concrete storage backends (in-memory, sqlite, postgres, mariadb)
- `billing`, `invoice` — billing subsystem
- `airgap` — air-gap mode implementation
- `watch` — file watcher implementation

---

## Recommendations

### Immediate (Pre-0.1)

1. **Add `#[doc(hidden)]` to all dead/test code:**
   - `graph_rag::repo_map::calculate`, `graph_rag::repo_map::main`
   - `workspace::context::add`
   - Internal helpers: `normalize`, `levenshtein`, `format_results_for_llm`, `format_grounded_response`, `ensure_tenant_columns`

2. **Add `#[doc(hidden)]` to implementation-detail submodules:**
   - `graph_rag::repo_map::utils`, `graph_rag::repo_map::helpers`
   - `config::keyring_storage`
   - `sandbox::backends` (keep `sandbox::backends::mod` detection functions public)

3. **Add `#[non_exhaustive]` to all public structs and enums** that consumers might construct. At minimum:
   - All config structs (`Config`, `ProjectConfig`, `LlmConfig`, etc.)
   - All result/output structs
   - All event enums (`McpMessage`, `StreamEvent`, `WebhookEvent`, etc.)

4. **Consider `pub(crate)` for:** browser internal types, `DiffCalculator`, `LegacyMetricsSnapshot`

### Short-term (0.1 release)

5. **Reduce public module tree.** Mark internal modules `#[doc(hidden)]` or make them `pub(crate)`:
   - `messaging`, `rpc`, `api`, `billing`, `invoice`, `airgap`, `watch`, `completions`, `capability`

6. **Resolve duplicate type names** by namespacing or consolidating:
   - `context::FileDiff` vs `timeline::FileDiff` — rename one
   - `context::ContextAggregator` vs `workspace::ContextAggregator` — consolidate
   - `session::SessionRepository` vs `storage::SessionRepository` — remove duplicate trait

7. **Add `#[deprecated]` to `LegacyMetricsSnapshot`** with migration guidance.

### Medium-term

8. **Define a `prelude` module** re-exporting only the stable surface.
9. **Consider sealed traits** for `LlmClient`, `SandboxBackend`, `TaskQueue`, `Skill`, `Vfs`, `WatchHandler`, `StorageBackend` — these should not be implemented by downstream crates without explicit opt-in.
10. **Audit `#[derive]` attributes** — many public types derive `Serialize`/`Deserialize`, which are part of the API contract.

---

## Stability Readiness Score

| Category | Score | Notes |
|---|---|---|
| `#[doc(hidden)]` hygiene | 0/10 | Zero usage |
| `#[non_exhaustive]` coverage | 0/10 | Zero usage |
| Dead code in public API | 2/10 | Multiple dead functions |
| Re-export discipline | 6/10 | Good selection, but leaks internals |
| Naming collisions | 3/10 | 12+ duplicate names |
| Module tree scope | 3/10 | 46 public modules, most are internals |
| **Overall** | **2.3/10** | **Not ready for stable release** |

---

*Generated by automated API surface audit.*
