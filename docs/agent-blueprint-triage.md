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
3. ✅ **Metamorphic oracle synthesis** (50) — `tools/metamorphic.rs`, 5 MR classes + stub synthesis
4. ⚠️ PARTIAL — **Lyapunov termination Lean proof (12)** shipped (`agent_loop.lean`, 5 theorems); entropy-gated rollback (5) remains
5. ⚠️ PARTIAL — **Test impact analysis (2)** shipped (`tools/test_impact.rs`); flaky bisection (17) remains


## Addendum — 100 production failure-mode items (2026-11-07, second agent discussion)

Two more lists from the same discussion: 50 ground-level production
failure modes (terminal/git/test/observability/DB/network/security/
multi-repo/ergonomics/cost/deploy) + 50 low-level systems items
(kernel bypass, CUDA, storage engines, allocators, streaming, compilers,
PKI, multi-tenant, micro-arch). Cluster-level triage; standout items
marked ★ (feasible in clawdius now).

### Ground-level production (50)

| Cluster | Status | ★ Standouts / notes |
|---|---|---|
| I. Terminal & runtime (1–9) | PARTIAL | ★3 virtual token pager (our compact_output is the seed — needs scratch-file + line-slice API); ★6 non-interactive PTY enforcer (shell tool sets env, lacks stdin-null); 1 daemon supervisor; 2 port remap; ★7 .env.example structural diff; 8 loopback tunneling; 9 PID-namespace reaping |
| II. Git safety (10–14) | PARTIAL | ★10 worktree isolation (fleet uses worktrees — productize); ★11 append-only git interceptor (block reset --hard/force-push at command validation); 12 AST replacers (tree-sitter); 13 conflict-marker ingestion (our own repo hit this — committed markers!); 14 atomic micro-commits |
| III. Test fraud (15–19) | PARTIAL | ★15 immutable test oracle mask (reject diffs that weaken assertions); 16 mutation score gate; 17 taint-tracked target isolation (mock-of-SUT = invalid); ★18 suppression-diff quota (no net-new @ts-ignore/noqa); 19 heap-delta leak detection |
| IV. Cross-file (20–24) | PARTIAL | 20 LSP rename propagation (clawdius-lsp); 21 acyclic module check; 22 native-first import audit; 23 hallucinated-API type gate; 24 runtime-target syntax lint |
| V. DB & schema (25–29) | PARTIAL | 25 squawk migration linter; 26 auto-rollback test transactions; 27 N+1 query-count alerts; 28 CoW seed branching; 29 zero-raw-SQL lint |
| VI. Network & API (30–34) | PARTIAL | ★30 VCR/WireMock recording (biggest agent-test enabler); 31 resilience contracts (timeout+backoff required); 32 semantic payload assertions; 33 SSO/HTML interception shield; 34 TSAN/-race gates |
| VII. Hygiene (35–39) | PARTIAL | ★35 in-tree formatter invocation post-edit; 36 debug-statement scrubber; 37 unused-symbol pruner; ★38 pre-commit secret detection (gitleaks — estate has it); 39 cyclomatic complexity cap |
| VIII. Frontend (40–43) | MISSING | 40–43 Playwright visual/console/a11y/CSS-scope gates (clawdius has web/tauri crates — applicable) |
| IX. Cost & latency (44–47) | PARTIAL | 44 cache-pinned prompt prefixes; 45 dual-stream silent execution (auto --quiet partially); ★46 semantic state-hash loop detection; ★47 impacted-only test filtering (TIA module shipped) |
| X. Deploy & PR (48–50) | PARTIAL | 48 reviewer-centric intent diffs; 49 self-rebasing branch bots; 50 canary rollback triggers (gateway has health primitives) |

### Low-level systems (50)

| Cluster | Status | Notes |
|---|---|---|
| I. Kernel bypass (1–5) | N/A-MOST | TCP/epoll/SO_REUSEPORT/UDP — applies to gateway crate only; ★5 TCP FIN half-close test harness is portable |
| II. CUDA/GPU (6–11) | N/A | clawdius-core has candle (ML feature) — defer until GPU features active |
| III. Storage engines (12–16) | N/A-MOST | Applies to storage-backed features; ★13 dm-flakey crash-consistency harness pairs with our journal (power-cut semantics) |
| IV. Allocators (17–20) | PARTIAL | 17 Go safepoints N/A (Rust); 18 jemalloc decay clawdius uses mimalloc; ★19 Tarjan SCC retain-cycle detection applicable to Rust Rc graphs in tests; 20 TLS dlopen audit N/A |
| V. Streaming (21–24) | PARTIAL | 21–22 Kafka/Flink N/A yet; 23 key-entropy skew analyzer portable; 24 backpressure invariants apply to gateway mpsc channels |
| VI. Compilers/linkers (25–29) | PARTIAL | 25 ThinLTO budgeting; 26 static-init fiasco (Rust: const-fn discipline, mostly moot); ★29 hermetic cache-reproducibility (sccache in CI — direct hit) |
| VII. PKI/auth (30–33) | PARTIAL | ★31 idempotent token-refresh mutex — DIRECT match for the OAuth design (refresh-on-401 must coalesce concurrent refreshes); 30 mTLS rotation (gateway); 32 STS auto-refresh proxies; 33 lockfile strictness (cargo --locked already) |
| VIII. Sagas (34–37) | PARTIAL | 34 saga chaos reversal; 35 enriched DLQ envelopes; ★36 deep dependency-aware health checks (gateway /healthz upgrade); 37 sliding-window circuit breaker (matches our resilience roadmap) |
| IX. Multi-tenant (38–40) | PARTIAL | 38 RLS query validators (auth crate + postgres feature); 39 read-replica routing; 40 DRR fair-share schedulers |
| X. IDE/micro-arch (41–50) | PARTIAL | 41 known_hosts injection; 42 POSIX binary normalization (Nix!); ★43 pre-commit dry-run emulation (fleet: sys-save gate precedent); 44 compat matrix solver; 45–50 SIMD/cache-line items → clawdius-unsafe crate territory |

### Live CI verification (2026-11-06/07)

Dogfood runs 13-15 exercised the loop-detection + bounce + retry stack
against a real spinning model: attempts that spun were fast-failed by
the detector (cheap), the job-level retry gave fresh attempts, and run
15 completed the repair ("Dogfood PASS", run 42620338543). Refinement
noted: the state hash currently covers response-text + tool-names;
adding tool-RESULT hashing would reduce false positives on legitimately
repeated identical invocations.

### Cross-program synthesis (both 100-item sets)

Six items recur across all three lists and form the convergent core:
1. **Output pager + delta distillation** (done: output_filter)
2. **Impacted-only test selection** (done: test_impact)
3. **Append-only test oracle protection + suppression quotas** (feasible)
4. **Deterministic capture/replay for network + time** (feasible: VCR harness)
5. **Idempotent token-refresh coalescing** (OAuth design §6 — add mutex)
6. **Semantic state-hash loop detection** (feasible: hash errors+AST per iteration)

