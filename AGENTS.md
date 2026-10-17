# AGENTS.md — instructions for AI coding agents

Project: **clawdius** — a Rust product (crates/) with a formal proof
estate (.specs/). The two halves have different rules; know which half
you are in before you edit.

Start with the `spec-research-loop` skill (`.opencode/skills/`) when
running research iterations; this file is the always-on orientation.

## The proof estate (`.specs/`)

`.specs/02_architecture/proofs/` is the ONLY proof tree. The legacy
`.clawdius/specs/02_architecture/proofs/` was retired in loop 3 — never
recreate it, never import from it.

- One Lean file per domain (`host.lean`, `container.lean`,
  `security_policy.lean`, `repo_map_scoring.lean`, `fsm.lean`,
  `sandbox_policy.lean`, `tier_isolation.lean`, `invariant_ledger.lean`,
  …).
- **New file = new `[[lean_lib]]` + `defaultTargets` entry** in
  lakefile.toml. A proof file no lib builds is only hand-checked — that
  is how stale proofs survive. (ProofHost pointed at the LEGACY tree for
  a full loop before anyone noticed.)
- The files are deliberately import-free. Cross-file concepts are
  restated and tied by enumeration theorems (see tier_isolation.lean's
  T4). Duplication + a tie theorem beats hidden coupling.

## Yellow Papers (`.specs/01_research/YP-*.md`)

House format: frontmatter (document_id, version, status, proof_files,
confidence_level, tqa_level) + YP-1 research basis / YP-2 gap / YP-3
field analysis / YP-4 corrected model / YP-5 falsifiable predictions /
YP-6+ later findings.

- YP-5 items are `proved` or `recorded, not asserted`. Recorded items
  are the backlog — the next iterations pick them up.
- **Superseded approaches stay in the text.** The record of being wrong
  is more valuable than the record of being right (see CONTAINER-014's
  Bool-precondition defect, HOST-015's table-vs-state claim).
- The registry (`yellow_paper_registry.toml`) must parse (check with
  tomllib after editing). Bump version on extension; append theorem IDs;
  keep IDs unique. **Anchor registry edits at the paper's ID LINE, then
  search forward** — the ID appears in other entries' arrays, and
  anchoring at the first textual occurrence corrupts a different paper.

## Verification ladder

```
lake env lean .specs/02_architecture/proofs/<file>.lean   # fast
lake build                                                # 45 targets, 0 sorries
python3 -c "import tomllib; tomllib.load(open('.specs/01_research/yellow_paper_registry.toml','rb'))"
git push
```

## Proof rules (each one was a real defect)

1. **Invariant records carry PROOFS, not `True`s** — see
   invariant_ledger.lean for the ledger pattern (named dischargers,
   totality + exhaustiveness proven).
2. **Preconditions are propositions** (`h1 : x.enabled = true`), never
   Bool values (`h1 : x.enabled` asserts nothing).
3. **decide/native_decide need CLOSED goals** — unfold defs with `show`
   or restate with literals first.
4. **`cases` with a `_` wildcard leaves the variable abstract** —
   enumerate constructors or use `<;>` chains.
5. **Compute a concrete example before stating a list theorem** — two
   first-draft identities here were backwards (push prepends ⇒ walk
   result is `p.reverse ++ acc`).
6. **Ship the counter-witness with the property**
   (`raw_prefix_bypass_exists`, `unclamped_prefix_escapes`) — properties
   that cannot drift into "true because we said so".
7. **State-level vs table-level**: check which one your claim is on
   (HOST-015: `initialized → running` is a valid table entry; the
   no-shortcut claim is about the post-restart state).
8. **A probe of the negation is part of the proof**: try the claim with
   the hypothesis deliberately false; if it compiles, the theorem was
   vacuous.

## Test vectors

`.specs/01_research/test_vectors/*.toml` — the policy surfaces as data.
Generate tables programmatically (never hand-type 21 rows), and
re-check the generated data against the constraint classes
independently. See test_vectors_tier_isolation.toml.

## Tooling

- `mcp-doctor` (WyattAu/mcp-doctor) — MCP fleet diagnostics; the
  codebase-memory MCP indexes THIS repo (30k+ nodes): `search_graph` /
  `get_architecture` before grepping.
- The `spec-research-loop` skill automates the iteration protocol.

## Refusals

Refuse to: leave a `sorry`, lower a threshold to make a proof pass,
delete a failing test, or mark a YP item `proved` without the theorem in
this tree.
