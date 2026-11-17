# PATTERN_ADOPTION: clawdius

Relationship to the WyattAu pattern canon
(`github.com/WyattAu/pattern-standards`), validated by
`scripts/adoption-check.py` per `references/adoption-format.md`.

| Pattern | Status | Evidence |
|---|---|---|
| T8 | adopted | `src/capability.rs` (capability model; no ambient authority in the gateway path) |
| T2 | partial | `src/fsm.rs` (runtime FSM over agent states; typestate encoding not yet applied) |
| T50 | adopted | `src/fsm.rs` + `src/capability.rs` + `src/mcp/host.rs` (capability-gated MCP gateway); invariants Lean-verified (`lean/Clawdius.lean`, 318 theorems per README) |
