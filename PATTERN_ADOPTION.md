# PATTERN_ADOPTION

Relationship to the WyattAu pattern canon (references/adoption-format.md).
Rows list patterns with code evidence; patterns without code and without a
scheduled plan are omitted (omission gate: repo ADR per PATTERN-CORE).

| Pattern | Status | Evidence |
|---|---|---|
| T48 | planned | crates/clawdius-core/src/agentic/llm_generator.rs:142 (tool-call decoding is post-hoc today; grammar-constrained decoding targeted 2027-Q1) |
| T49 | adopted | crates/clawdius-core/src/agentic/verifier_agent.rs:17 (verifier in the sprint loop; build/test retry cycle at crates/clawdius-core/src/agentic/sprint/engine.rs:382) |
| T50 | adopted | crates/clawdius-core/src/sandbox/executor.rs:46 (8 isolation backends, refuse-by-default; egress firewall at crates/clawdius-core/src/sandbox/firewall.rs:1) |
| P31 | partial | crates/clawdius-core/src/sandbox/safety.rs:1 (Sanitizer detects and neutralizes injection patterns at the boundary; no DataRef-style opaque-handle quarantine for untrusted content) |
| P33 | adopted | crates/clawdius-core/src/tools/loop_guard.rs:1 (semantic state-hash loop breaker; a repeated state hash forces stop or pivot) |
| P36 | partial | crates/clawdius-core/src/agentic/apply_workflow.rs:39 (diff preview with trust vs rollback apply modes; per-sprint worktree isolation at crates/clawdius-core/src/agentic/worktree.rs:1; no ephemeral CoW sandbox with policy commit) |
| P37 | partial | crates/clawdius-core/src/tools/edit_cascade.rs:1 (5-strategy deterministic repair cascade for edit arguments; no AST-guided repair of malformed LLM output) |
| P39 | partial | crates/clawdius-core/src/llm/cache.rs:9 (blake3-keyed response cache with hit counts; in-memory, key lacks modelId and seed, no durable replay store) |
| P40 | partial | crates/clawdius-core/src/api/agent_loop.rs:25 (typed TokenBudgetExhausted and TimeBudgetExhausted faults; caps at crates/clawdius-core/src/api/resource_budget.rs:6; budgets are counters, not attenuable macaroon leases) |
| P119 | adopted | crates/clawdius-core/src/api/resource_budget.rs:6 (per-tenant max_sessions and max_messages_per_session caps with typed limit errors) |
