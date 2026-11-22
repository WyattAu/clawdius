# AGENTS.md — instructions for AI coding agents

Project: **clawdius** — a Rust product (crates/) with a formal proof
estate (.specs/). The two halves have different rules; know which half
you are in before you edit.

Start with the `spec-research-loop` skill (`.opencode/skills/`) when
running research iterations; this file is the always-on orientation.

## The proof estate (`.specs/`)

Proof FILES live in [`WyattAu/clawdius-spec`](https://github.com/WyattAu/clawdius-spec)
— the shared Lean proof package (see `.specs/02_architecture/proofs/README.md`).
This repo consumes them at a pinned tag via `lakefile.toml`; the
`lean4-proofs` CI job builds the pinned package and greps the log for
sorries.

- Proof changes: PR against `clawdius-spec`. Never recreate `.lean`
  proof files here, and never resurrect the legacy
  `.clawdius/specs/02_architecture/proofs/` tree.
- To bump the proof pin: merge the release in `clawdius-spec`, move the
  tag, PR the `rev` in `lakefile.toml`. Tags are claim pins — bump only
  on green CI there.
- YP files (`proof_files` frontmatter) reference modules by their
  `clawdius-spec` file names.

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
