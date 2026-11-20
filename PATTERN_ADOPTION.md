# PATTERN_ADOPTION: clawdius

Relationship to the WyattAu pattern canon
(`github.com/WyattAu/pattern-standards`), validated by
`scripts/adoption-check.py` per `references/adoption-format.md`.

| Pattern | Status | Evidence |
|---|---|---|
| T8 | adopted | `../clawdius/src/capability.rs` (capability model; no ambient authority in the gateway path) |
| T2 | partial | `../clawdius/src/fsm.rs` (runtime FSM over agent states; typestate encoding not yet applied) |
| T50 | adopted | `../clawdius/src/fsm.rs` + `../clawdius/src/capability.rs` + `../clawdius/src/mcp/host.rs` (capability-gated MCP gateway); invariants Lean-verified (`.specs/02_architecture/proofs/` (Lean 4 suite, 318 theorems per README)) |
| P23 | n/a | canon baseline line names this repo; code shows no implementation of P23: no abort-scope teardown exists, `../clawdius/src/mcp/types.rs:12` CancellationToken is an MCP wire notification, not a scope token, and event subscribers (`../clawdius/src/fsm.rs:608`) have no removal path |
| T48 | n/a | canon baseline line names this repo; code shows no implementation of T48: LLM calls (`../clawdius/src/llm.rs`) send raw completions with no schema or grammar constraint, and tool arguments (`../clawdius/src/mcp/server.rs:457`) are deserialized without schema validation |
