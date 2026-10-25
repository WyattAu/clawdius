---
name: spec-research-loop
description: Run a research→spec→proof iteration on this repo — pull real sources, extend the Yellow Paper, keep lake build green, update the registry
compatibility: opencode
metadata:
  audience: agents
  workflow: research-loop
  opencode/slash: "true"
  opencode/autoinvoke: "true"
---

## What I do

Run one research iteration end-to-end:

1. **Pick the obligation.** Every YP's "Falsifiable predictions" section
   marks items `proved` / `recorded, not asserted`. Recorded items are the
   backlog. If nothing is pending, read the proofs for the weakest claim
   instead — this loop's best finds were defects the proofs caught
   (Bool-valued preconditions, wrong orientations, table-level vs
   state-level claims).
2. **Research real sources.** Specs, RFCs, NIST, vendor docs, upstream
   design docs — never training-data memory alone. The YP's YP-1 section
   cites what each source grounds.
3. **Compare with the field.** YP-3: how do established systems solve
   it? "Evaluated and rejected" is a first-class outcome.
4. **Extend the Yellow Paper** (`.specs/01_research/YP-<DOMAIN>-<NNN>.md`):
   house format is frontmatter (document_id, version, status, domain,
   proof_files, confidence_level, tqa_level) + sections YP-1 research
   basis / YP-2 gap / YP-3 field analysis / YP-4 corrected model /
   YP-5 falsifiable predictions. Later findings go in YP-6, YP-7…
5. **Prove it** in `.specs/02_architecture/proofs/` — no `sorry`, ever.
6. **Update the registry** (`.specs/01_research/yellow_paper_registry.toml`):
   bump the paper's version, append theorem IDs, keep IDs unique.
7. **Commit + push.** Commit message = the iteration's findings, not just
   "update proofs".

## Verification ladder (in order)

```
lake env lean .specs/02_architecture/proofs/<file>.lean   # fast single-file
lake build                                                # 41+ targets, 0 sorries
python3 -c "import tomllib; tomllib.load(open('.specs/01_research/yellow_paper_registry.toml','rb'))"
git push
```

## Proof pitfalls this estate already paid for

- **decide/native_decide need CLOSED goals.** A goal mentioning a `def`
  (e.g. `refWeight`) is not closed — unfold with `show`/`have … := rfl`
  or restate with literals.
- **`cases` with a `_` wildcard leaves the variable ABSTRACT** — `decide`
  and `rfl` then fail for no reason. Enumerate all constructors, or use
  `cases p <;> simp [...] <;> …` chains.
- **Compute a concrete example BEFORE stating a list theorem.** Two of
  this estate's first-draft identities were backwards (push prepends ⇒
  the walk result is `p.reverse ++ acc`, not `acc ++ p.reverse`).
- **State invariants over ARBITRARY contents as list inductions**, not
  claims about a snapshot (registry = association list; "all components"
  = induction over entries).
- **Statements live on STATES, not tables.** `initialized → running` is a
  valid table entry; "no shortcut after restart" is about the
  post-restart STATE. The proof assistant will find the difference for
  you — after you wasted a draft.
- **Probe the negation before trusting a proof**: try to prove the claim
  with the hypothesis deliberately FALSE. If it compiles, the theorem is
  vacuous.
- **Ship the counter-witness with the property** (see
  `unclamped_prefix_escapes` next to `normalized_prefix_is_workspace`) —
  properties that cannot drift into "true because we said so".
- **New file = new `[[lean_lib]]` + defaultTargets entry** in
  lakefile.toml. A proof file that no lib builds is only hand-checked —
  that is how stale proofs survive.

## Registry etiquette

- One entry per paper; bump `version` when extending (1.0.0 → 1.1.0).
- After editing, re-parse with tomllib — a broken registry fails CI.
- `proof_files` names files in `.specs/02_architecture/proofs/`.
- Superseded approaches stay in the YP text (the record of being wrong is
  more valuable than the record of being right).

## Layout

- `.specs/01_research/` — YPs + `yellow_paper_registry.toml` + loop
  reports (`LOOP-<date>-REPORT.md`)
- `.specs/02_architecture/proofs/` — the only proof tree (the legacy
  `.clawdius/specs/02_architecture/proofs/` was retired in loop 3)
- `.specs/02_architecture/archive/` — loop-0 era verification reports
