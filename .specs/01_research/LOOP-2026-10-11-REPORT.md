# Research Loop Report — 2026-10-11 (Iterations 1–10)

Ten research→analysis→spec→proof iterations, as specified. Every iteration: pulled real
sources, compared with how other systems solve the problem, wrote the spec (Yellow
Paper), and adjusted the Lean proof.

## Outcomes

| Iter | Domain | Research anchor | Spec artifact | Proof outcome |
|---|---|---|---|---|
| 1 | Sandbox fuel | WasmCert (CPP 2018, ITP 2021); wasmtime Store docs; EVM rule-level gas | YP-SANDBOX-FUEL-002 | 6 sorries → 0 (outcome-typed conservation) |
| 2 | Ring buffer | Linux kfifo; BPF ringbuf docs; Rust SPSC crates | YP-RING-FREE-RUNNING-002 | 5 sorries → 0 (free-running counters; slot injectivity) |
| 3 | Cache | Sleator–Tarjan (CACM 1984); TinyLFU (2017); Redis eviction docs | YP-CACHE-EVICTION-003 | 3 sorries → 0 (ValidCacheConfig; SortedByCreatedAt) |
| 4 | RPC | CMU 15-440 taxonomy; Tyler Treat (2015); LogPlayer/TLA+ (2019) | YP-RPC-DISPATCH-003 | 2 sorries → 0 (ResponseFor dispatch relation) |
| 5 | Symbol index | Self-adjusting computation (ICFP 2014); LSP 3.17; salsa notes | YP-SYMBOL-INDEX-002 | 2 sorries → 0 |
| 6 | Token budget | TALE (ACL 2025); BPS (arXiv 2608.19993); CWL (2026); Stackelberg agents (2026) | YP-BUDGET-MONOTONE-006 | **Monotonicity REFUTED** — verified counterexample (budgets 4 ≤ 12 over [12,1,1,1,1] select 4 vs 1) |
| 7 | Concurrency | Herlihy & Wing (TOPLAS 1990); JoinSet docs | YP-CONCURRENT-PLACEMENT-007 | +3 theorems (placement, self, no-cross-talk) |
| 8 | VFS | POSIX §4.13; RFC 3986 §5.2.4; realpath boundary | YP-VFS-NORMALIZE-008 | model + 2 rfl theorems; pop-semantics proofs pending (recorded, not asserted) |
| 9 | Agent loop | ReAct (ICLR 2023) | YP-AGENT-THINK-ONLY-009 | +1 theorem (thought-only chains never terminate) |
| 10 | Registry/traceability | this report | yellow_paper_registry.toml (8 → 16 papers) | full-build verification below |

## Verification state after iteration 10

- Lean: `lake build` — 43 targets, **0 sorries**, **0 errors** (was 18 sorries).
- Registry: 16 yellow papers registered with theorems, consumers and proof files.

## Defects found that are NOT fixed in this loop (recorded, not hidden)

1. **The Rust doctest suite on `main` is red**: 8 doctests in `clawdius-core`
   (`lib.rs` ×6, `tools.rs` ×2) reference API that no longer exists
   (`llm::LlmConfig`, a `SandboxConfig` initializer missing `allow_unisolated`).
   Pre-existing — this loop touched only `.specs`. The README's "3,044 tests
   passing" badge is therefore stale as stated. This loop also has **no CI** on
   its pushes (see estate-wide CI state), which is why the drift went unnoticed.
2. **`.clawdius/specs/02_architecture/proofs/`** holds a second, older proof tree
   (7 Lean files) that the lakefile does not build. Either archive it or fold its
   content into `.specs`; two proof trees is one too many.
3. **VFS pop-semantics** proofs (idempotence, dot-freedom for the POSIX model) are
   the next iteration's work — YP-VFS-NORMALIZE-008 states the obligations; they
   are not asserted here.

## Method note

The loop's thesis held six times and reversed once: research finds what the field
believes, the verifier finds what is true, and the delta is the spec improvement.
Iteration 6 is the reversal — the planned monotonicity theorem was *false*, and the
verified counterexample (`budget_monotonicity_fails`) is a stronger artifact than the
theorem would have been.
