# Agent Blueprint — 50+50 breakthrough triage

**Source:** joint analysis with another AI agent (2026-11-07), two lists:
50 research-grade (I. state-space/control/formal) + 50 practical
(monorepo/sandbox/test/observability/DB/security/multi-repo/ergonomics/cost).
**Purpose:** durable triage vs clawdius's actual state; drives implementation
order. Status: HAVE / PARTIAL / FEASIBLE-NOW / RESEARCH (novel science).

## Practical list (50) — triage

| # | Item | clawdius state | Action |
|---|---|---|---|
| 1 | SCIP/LSIP graph index vs vector RAG | graph-RAG exists; SCIP missing | FEASIBLE (scip CLI) |
| 2 | Test impact analysis (changed lines → tests) | missing | FEASIBLE (cargo test --list + coverage map) |
| 3 | AST-level semantic diffs (Tree-sitter) | tree-sitter dep exists; AST diffs missing | FEASIBLE-NOW |
| 4 | Internal utility discovery | graph-RAG partial | PARTIAL |
| 5 | Build-graph query (Bazel/Nix) | Nix flake exists; agent query missing | FEASIBLE |
| 6 | Git blame/intent attribution | git_log tool exists; intent parsing missing | FEASIBLE |
| 7 | FUSE/VFS monorepo mounting | missing | RESEARCH (heavy) |
| 8 | Type-inference hydration (Pyright/Mypy daemon) | LSP crate exists | PARTIAL (clawdius-lsp) |
| 9 | MicroVM fork <5ms (Firecracker) | Firecracker backend exists (8 backends) | HAVE |
| 10 | PTY auto-kill for interactive prompts | shell tool: timeout only | FEASIBLE-NOW |
| 11 | eBPF egress guardrails | missing | FEASIBLE (bpftrace; sandbox-adjacent) |
| 12 | CoW database branching | missing | FEASIBLE (Neon/SQLite VFS) |
| 13 | Deterministic mock clock + RNG injection | missing | RESEARCH-adjacent (LD_PRELOAD) |
| 14 | Output throttling + adaptive summarization | TUI truncates 500ch; engine-level missing | **FEASIBLE-NOW → implemented (output_filter)** |
| 15 | Filesystem transactional journaling | missing (overlayfs snapshots) | FEASIBLE (Linux-only) |
| 16 | Mutation-testing score gate | missing | FEASIBLE (cargo-mutants) |
| 17 | Flaky-test bisection loop | missing | FEASIBLE |
| 18 | Property-based test synthesis | proptest dep exists | FEASIBLE |
| 19 | Snapshot auto-update with diffs | missing | PARTIAL |
| 20 | Profiler-guided perf regression gate | missing | FEASIBLE (perf) |
| 21 | Hermetic build enforcement | docker + lockfiles exist | PARTIAL |
| 22 | OpenTelemetry trace ingestion | missing | FEASIBLE |
| 23 | Core dump / DWARF unwinding | missing | FEASIBLE (lldb headless) |
| 24 | Flamegraph ingestion | missing | FEASIBLE |
| 25 | Log clustering (Drain) | missing | FEASIBLE |
| 26 | eBPF live probe injection | missing | FEASIBLE (deferred) |
| 27 | Expand-and-contract migrations | db-migration skill doc'd | PARTIAL |
| 28 | Lock-contention analyzer | missing | FEASIBLE |
| 29 | Shadow-traffic replay | missing | FEASIBLE |
| 30 | API contract break detection | missing | FEASIBLE (openapi-diff) |
| 31 | Kafka schema-compat check | missing | FEASIBLE |
| 32 | Secret scrubbing on output | audit logging exists; scrubbing missing | **FEASIBLE-NOW → implemented (output_filter)** |
| 33 | Least-privilege execution tokens | RBAC crate exists; scoped agent creds missing | PARTIAL |
| 34 | SBOM + CVE vetting on dep add | supply-chain/ + cargo-deny exist | HAVE |
| 35 | Taint-analysis SAST patching | security module exists | PARTIAL |
| 36 | HSM/Sigstore signed agent commits | missing | FEASIBLE (cosign) |
| 37 | Prompt-injection-proof tool I/O | structured args (JSON) already; untrusted-string quarantine missing | PARTIAL |
| 38 | Cross-repo multi-PR orchestration | fleet manager v1 (single repo) | FEASIBLE (fleet roadmap) |
| 39 | Arch boundary enforcement | analysis/drift exists | PARTIAL |
| 40 | Strangler-fig decoupling automation | missing | RESEARCH (large) |
| 41 | IaC drift reconciliation | terraform in deploy/ | PARTIAL |
| 42 | Intent diffs (why-not-what) | ActionResult edits only | FEASIBLE |
| 43 | IDE checkpoint timeline | checkpoint module + clawdius-code exist | PARTIAL |
| 44 | Team idiom lint extraction | missing | FEASIBLE |
| 45 | Semantic rebase engine | missing | RESEARCH |
| 46 | KV-cache prefix reuse | inference-side | OUT (provider concern) |
| 47 | Local speculative decoding | inference-side | OUT |
| 48 | Compiler-error delta distillation | missing | **FEASIBLE-NOW → implemented (output_filter delta)** |
| 49 | Zero-token lint pre-filter | missing | **FEASIBLE-NOW → implemented (post-edit rustfmt hook)** |
| 50 | Complexity-tiered model routing | model_router.rs exists (7 task types) | HAVE |

## Research list (50) — triage (compressed)

| Cluster | Items | clawdius state | Action |
|---|---|---|---|
| I. State-space/POMDP (1–8) | particle-filter belief, active sensing, symplectic world models, Fisher exploration, entropy rollback, Kolmogorov compression, Dec-POMDP co-design, causal DAG induction | none | RESEARCH; **entropy-gated rollback (5)** and **causal failure DAGs (8)** are the two implementable seeds |
| II. Optimal control (9–14) | CBFs, path-integral control, MPC codegen, Lyapunov loop termination, Pontryagin token budget, CMA-ES | MAX_ITERATIONS; proofs adjacent | **Lyapunov termination proof (12)** maps to Lean4 estate — FEASIBLE as a formal theorem; CBF action projection (9) FEASIBLE as a sandbox policy |
| III. Formal (15–22) | PCC, Hoare embedding, separation logic, abstract interpretation, SMT-guided decoding, bidirectional typing, Craig interpolation, refinement types | **318 Lean4 theorems; proof-carrying direction already the estate's thesis** | HAVE (directionally); refinement-type tool schemas (22) FEASIBLE-NOW |
| IV. Representation (23–28) | SSA GNN, sheaf repos, spectral call-graphs, equivariant AST nets, Riemannian editing, IR/ASM duality | none | RESEARCH |
| V. Test-time search (29–34) | Pareto-MCTS, HMC, sequential halving, bidirectional A\*, conformal values, MH mutations | none | RESEARCH (MCTS groundwork = fleet DAG planner adjacency) |
| VI. Runtime (35–40) | deterministic microVM, CoW snapshots, Merkle-DAG memory, eBPF traces, reverse debugging, typed ABI tools | Firecracker/WASM backends; checkpoint module | PARTIAL (35/36 = sandbox roadmap; 40 = typed tool ABI FEASIBLE) |
| VII. Continual learning (41–45) | null-space meta-learning, fast weights, Hopfield memory, online PPO, TD(λ) traces | none | RESEARCH (weight updates = provider concern) |
| VIII. Concurrency (46–50) | actor model, Raft mutation consensus, vector clocks, STM speculative runs, metamorphic oracles | distributed/ module exists | PARTIAL (46/47 FEASIBLE for fleet; **50 metamorphic oracles FEASIBLE-NOW**) |

## Implementation order (this blueprint)

1. ✅ **output_filter.rs** — secret scrub + repetition-collapse + delta (14/32/48)
2. ✅ **Post-edit rustfmt hook** (49)
3. Metamorphic oracle synthesis (50) — property stubs in test synthesis
4. Entropy-gated rollback (5) + Lyapunov termination Lean proof (12)
5. Test impact analysis (2) + flaky bisection (17)
