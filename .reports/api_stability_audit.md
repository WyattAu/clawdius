# Clawdius Public API Stability Audit

**Date:** 2026-05-14
**Workspace version:** `1.0.0-rc.1`

---

## Summary

| Crate | Type | `pub fn` | `pub struct` | `pub enum` | `pub trait` | `pub type` | `pub const` | `pub mod` | Assessment |
|---|---|---|---|---|---|---|---|---|---|
| `clawdius-core` | library | 1879 | 664 | 141 | 21 | 8 | 83 | 161 | **Unstable** |
| `clawdius-gateway` | library | 78 | 34 | 3 | 2 | 1 | 4 | 18 | **Provisional** |
| `clawdius` | binary | 105 | 27 | 29 | 0 | 0 | 61 | 41 | N/A (bin) |
| `clawdius-mcp` | binary | 2 | 0 | 0 | 0 | 0 | 0 | 0 | N/A (bin) |
| `clawdius-code` | binary | 2 | 0 | 0 | 0 | 0 | 0 | 0 | N/A (bin) |

---

## Per-Crate Analysis

### `clawdius-core` — Core Library

**Role:** Central library consumed by all other crates.

**Public modules (visible):** `actions`, `agentic`, `agents`, `analysis`, `api`, `capability`, `checkpoint`, `commands`, `completions`, `config`, `context`, `diff`, `encryption`, `error`, `graph_rag`, `llm`, `lsp`, `mcp`, `memory`, `modes`, `orchestrator`, `output`, `retry`, `session`, `simd`, `skills`, `storage`, `timeline`, `timeout`, `tokenize`, `tools`, `workspace` (33 modules)

**Re-export surface** (from `lib.rs`):
- `agents`: `AgentError`, `AgentMessage`, `AgentRole`, `AgentStatus`, `AgentTeam`, `TeamConfig`, `TeamResult`
- `api`: `ApiConfig`, `ApiGateway`, `ChatRequest`, `ChatResponse`, `HealthResponse`
- `config`: `Config`
- `context`: `AggregatedContext`, `ContextAggregator` *(feature: `vector-db`)*, `CompactResult`, `Context`, `ContextCompactor`, `ContextCompactorConfig`, `ContextItem`, `ContextWindowManager`, `FileInfo`, `Mention`, `MentionResolver`, `ProviderTokenLimits`
- `diff`: `DiffPreview`, `DiffRenderer`, `DiffStats`, `DiffTheme`, `FileDiff`
- `error`: `EnhancedError`, `Error`, `ErrorHelpers`, `Result`
- `memory`: `MemoryEntry`, `MemoryMetadata`, `ProjectMemory`
- `output`: `OutputFormat`
- `retry`: `with_retry_and_circuit`, `CircuitBreaker`, `CircuitState`
- `session`: `Session`, `SessionManager`, `SessionStore`
- `skills`: `Skill`, `SkillContext`, `SkillError`, `SkillMeta`, `SkillRegistry`, `SkillResult`
- `storage`: `GraphRepository`, `InMemoryBackend`, `SessionRepository`, `SqliteBackend`, `StorageBackend`, `TimelineRepository`
- `telemetry`: `CrashReporter`, `TelemetryConfig`
- `timeline`: `CheckpointId`, `TimelineManager`
- `workspace`: `IndexStats`, `WorkspaceIndexer` *(feature: `vector-db`)*
- `agentic`: `AgenticState`, `AgenticSystem`, `ApplyWorkflow`, `ChangeType`, `FileChange`, `GenerationMode`, `GenerationOptions`, `GenerationResult`, `LogEntry`, `LogLevel`, `TaskContext`, `TaskRequest`, `TaskResult`, `TestExecutionStrategy`, `TestResult`, `TrustLevel`, `WorkflowResult`, `ExecutorAgent`, `StepResult`, `IssueSeverity`, `VerificationIssue`, `VerificationResult`, `VerifierAgent`, `PlannerAgent`, `RiskAssessment`, `StepAction`, `TaskPlan`, `TaskStep`
- `analysis`: `AnalysisError`, `AnalysisResult`, `ArchitectureDrift`, `DebtAnalyzer`, `DebtItem`, `DebtReport`, `DebtRule`, `DebtType`, `DriftCategory`, `DriftDetector`, `DriftReport`, `DriftRule`, `DriftSeverity`
- Constants: `VERSION`, `CRATE_NAME`

**`#[doc(hidden)]` modules** (14 modules — intentionally excluded from docs):

| Module | Also re-exported at crate root? |
|---|---|
| `airgap` | No |
| `audit` | No |
| `billing` | No |
| `compliance` | No |
| `i18n` | No |
| `invoice` | No |
| `onboarding` | **Yes** — `Onboarding`, `OnboardingStatus` |
| `proof` | **Yes** — `LeanVerifier`, `ProofDefinition`, `ProofTemplate` |
| `rpc` | No |
| `sandbox` | No |
| `telemetry` | **Yes** — `CrashReporter`, `TelemetryConfig` |
| `usage` | No |
| `watch` | No |
| `webhooks` | No |

**Assessment: Unstable.** At `1.0.0-rc.1` with 1879 public functions and 664 public structs, this is a very large API surface. The `#![allow(missing_docs)]` crate-level attribute means most items lack documentation. 14 modules are `#[doc(hidden)]` but 3 of those (`onboarding`, `proof`, `telemetry`) leak items via `pub use` re-exports — these should either be made fully public or the re-exports should also be `#[doc(hidden)]`.

---

### `clawdius-gateway` — Messaging Gateway Library

**Role:** Platform adapter layer for chat integrations.

**Public modules:** `adapter`, `adapters`, `admin`, `error`, `formatter`, `gateway`, `handler`, `rate_limit`

**Re-export surface:**
- `IncomingMessage`, `OutgoingMessage`, `PlatformAdapter`, `PlatformConfig`
- `GatewayError`
- `ResponseFormatter`
- `MessageGateway`
- `ClawdiusHandler`
- `RateLimiter`

**`#[doc(hidden)]` items:** None
**`#[deprecated]` items:** None

**Assessment: Provisional.** Small, focused API surface. Feature-gated platform adapters (`telegram`, `discord`, `slack`, `matrix`) behind `all-platforms` feature. The gateway depends on `clawdius-core` at `1.0.0-rc.1`, so it inherits core's instability.

---

### `clawdius` — CLI Binary

**Role:** End-user CLI binary. Not a library.

**Public modules:** `cli`, `cli_progress`, `tool_executor`

**Assessment: N/A.** Binary crate — no semver guarantees needed for downstream Rust consumers. The `pub mod` declarations exist for internal organization but are not published as a library.

---

### `clawdius-mcp` — MCP Server Binary

**Role:** MCP stdio server for Claude Desktop interop. Not a library.

**Public API:** 2 functions only — `parse_request()`, `format_response()`

**Assessment: N/A (binary).** The lib.rs provides testable core logic but the crate is structured as a binary. The 2-function API is trivially small and stable.

---

### `clawdius-code` — VSCode Extension Binary

**Role:** JSON-RPC server binary for the VS Code extension. Not a library.

**Public API:** 2 functions only — `parse_request()`, `format_response()`

**Assessment: N/A (binary).** Same pattern as `clawdius-mcp`. 2-function API, trivially stable.

---

## `#[deprecated]` Items

**None found across all crates.**

---

## Key Findings

### 1. Inconsistent `doc(hidden)` Usage
Three `#[doc(hidden)]` modules (`onboarding`, `proof`, `telemetry`) re-export items at the crate root without `#[doc(hidden)]`. This is a leak — consumers can depend on these items despite their parent module being hidden.

### 2. `#![allow(missing_docs)]` on Both Library Crates
Both `clawdius-core` and `clawdius` blanket-suppress missing documentation warnings. For a crate at `1.0.0-rc.1` with library consumers, this is a quality gap.

### 3. No `#[deprecated]` Annotations
Zero deprecated items. Either the API has never changed, or changes were made without deprecation cycles — both are concerns at rc.1.

### 4. Large API Surface in Core
With 1879 public functions and 664 structs, `clawdius-core` has a very broad surface for semver guarantees. A single breaking change to any of these items triggers a major version bump.

### 5. Feature-Gated Re-exports
`vector-db` feature gates `AggregatedContext`, `ContextAggregator`, `IndexStats`, `WorkspaceIndexer`. This is correct practice — optional features should not appear in the public API when disabled.

---

## Recommendations

### Semver Guarantees

| Recommendation | Priority | Rationale |
|---|---|---|
| **Do not publish 1.0.0 yet.** Stay on `1.0.0-rc.x`. | Critical | `missing_docs` allowed, 14 hidden modules, no deprecation history. |
| **Add `#[doc(hidden)]` to leaking re-exports** in `onboarding`, `proof`, `telemetry` | High | Prevents consumers from depending on unstable items |
| **Remove `#![allow(missing_docs)]`** from `clawdius-core` and `clawdius` | High | Documentation coverage is required for stable API |
| **Audit `clawdius-core` pub surface** — many `pub fn`/`pub struct` are likely `pub(crate)` candidates | High | Reduces semver blast radius |
| **Adopt `#[deprecated]`** for any future API changes | Medium | Enables semver-compliant deprecation cycles |
| **Mark gateway traits (`PlatformAdapter`, `RateLimiter`) with stability annotations** | Medium | These are integration points — consumers need guarantees |
| **Consider `#[non_exhaustive]`** on core enums and structs | Medium | Allows adding fields/variants without semver breaks |
| **Publish a CHANGELOG** per-crate for every rc release | Low | Enables downstream consumers to track API evolution |
