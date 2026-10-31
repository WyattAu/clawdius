# Loop 5, Iteration 1 — Plan: Discharge YP-AUDIT-013 P3 (external-signer anchoring)

## The unit of work (one sentence)

Discharge the last `recorded, not asserted` audit obligation — YP-AUDIT-013
P3, "an external signer on the chain head" — by proving an anchored
prefix-binding theorem in `audit.lean`, shipping the no-anchor fork
witness that shows why the unanchored T1 is insufficient, and flipping P3
to `proved` in the paper + registry.

## Why this, and not the loop-3 "next-loop candidates"

Verified against the tree on 2026-10-29:

| Candidate | Status | Evidence |
|---|---|---|
| Damping non-monotonicity witness | **Done in loop 4** (it. 1+4) | `damped_add_lowers_old_neighbor`, `damped_append_lowers`, `damped_truncation_breaks_conservation` in `repo_map_scoring.lean`; YP-REPO-MAP-018 v1.1.0 YP-7 says "Machine-checked (loop 4, iterations 1 + 4)" |
| gVisor `intercepted` as first-class tier | **Done in loop 4** (it. 2) | `tier_isolation.lean` bridges tiers × isolation classes with T1–T4 incl. the spec-implementation tie to `SandboxPolicy.selectTier`; YP-SECURITY-TIER-019 v1.0.0, all P-items proved |
| Bool-precondition defect-class sweep | Deprioritized | Grep for the known shapes (`.enabled`, `.noNewPrivileges`, … as bare hypothesis Bools) finds no hits; CONTAINER-014's instance was fixed in loop 3 it. 4. Fall back to this only if P3 stalls in prove phase. |
| **YP-AUDIT-013 P3 — head anchoring** | **Open — chosen** | YP-AUDIT-013 v1.0.0 line: "P3 (recorded, not asserted) … Next iteration's work". Its sibling (SECURITY-POLICY-011 P3 closure) was discharged in loop 3 it. 6; this is the lone survivor. |

The skill's rule decides it: recorded items are THE backlog; an audit for
a defect class that greps clean is the fallback. P3 is also the highest-
value hole in the estate's integrity story: T1 `tamper_evident` binds two
histories only under ONE stored chain — a storage-level attacker forges
the presented events AND the stored head together and T1 never fires.
P3 names the fix (periodic external anchoring, RFC 6962 STH style) and
the estate has never machine-checked it.

## Model sketch (Nat/list-only, no Floats — house policy)

Reuse the existing `AuditEvent`, `ehash`, `ehash_inj`, `chainOf`,
`Verify`. Add, in `namespace` of audit.lean (no new file, no lakefile
change):

- `anchorCommit : Nat` — the external signer's commitment, defined as
  `chainOf (recorded.take n)` for an anchor point `n` (the signer sees
  the prefix at anchor time).
- Theorem T3 (`anchored_prefix_tamper_evident`): if `recorded` and
  `presented` verify under the same stored chain AND
  `chainOf (recorded.take n) = anchorCommit` AND
  `chainOf (presented.take n) = anchorCommit`, then
  `recorded.take n = presented.take n`. Proof: `chainOf_inj` restricted
  to the prefix (via `chainOf (take n)` equality + injectivity).
  No new axioms — only the existing justified `ehash`/`ehash_inj`.
- Witness T4 (`fork_without_anchor_exists`): two CONCRETE distinct
  single-event histories, each verifying against its own stored head —
  i.e., without an anchor commitment the forger controls `stored` and
  T1 binds nothing. `decide`/`rfl`-level; ship it next to T3 so the
  property cannot drift into "true because we said so"
  (house rule 6, same pattern as `unclamped_prefix_escapes`).
- Recorded boundary (P3b, recorded with rationale like the damping
  decision): anchoring bounds the forgery window to the post-anchor
  suffix — the anchored prefix is bound, the tail is only as trustworthy
  as the head-holder until the next anchor. State it in the YP; do NOT
  pretend the theorem covers it (state-level vs table-level honesty,
  house rule 7).

## Phase acceptance criteria

### 1. Research (real sources, never training memory alone)

- Pull RFC 6962 §2.1.1 (Signed Tree Head = the anchor structure) and
  RFC 9162 (CT gossip / how anchor commitments distribute) — confirm the
  model sketch matches how CT binds clients to log history.
- Cross-check Russ Cox "Transparent Logs for Skeptical Clients" (already
  in YP-1) for the client-side no-trust-in-operator framing.
- AC: YP-AUDIT-013 gains the next sequential `YP-N` section citing
  ≥1 NEW source (RFC 9162 at minimum) with one line on what each source
  grounds; the section states the post-anchor-window boundary explicitly
  as a non-claim.

### 2. Prove (`audit.lean`, no `sorry`, ever)

- T3 and T4 as above, in the existing Part 2 section, numbered T3/T4
  continuing the file's T1/T2 convention.
- Negation probe (house rule 8): restate T3 without the anchor
  hypotheses and confirm it is exactly the T4 counter-witness situation
  — the probe result recorded in a one-line comment next to T4.
- Pitfalls to observe: `decide` only on closed goals (unfold with `show`
  / literals first — `anchorCommit` mentions defs); no `cases _`
  wildcards; hypotheses are propositions (`h : chainOf … = anchorCommit`),
  never Bool fields.
- AC: `lake env lean .specs/02_architecture/proofs/audit.lean` exits 0;
  `grep -c sorry audit.lean` = 0; T4 constructs its witnesses with
  literal `AuditEvent` values (no opaque exists-statement).

### 3. Implement (spec artifacts; NO Rust changes this iteration)

- YP-AUDIT-013: version 1.0.0 → 1.1.0, `research_iterations` 3 → 4,
  P3 re-stated as proved (T3/T4), P3b recorded with rationale; first
  drafts that fail stay in the text (the record of being wrong is an
  asset).
- Registry: edit `yellow_paper_registry.toml` ANCHORED AT THE
  `id = "YP-AUDIT-013"` LINE, searching forward from there (house rule —
  the ID string appears in other entries' arrays); append
  `THM-AUD-T3`, `THM-AUD-T4` to its `theorems`; bump its `version`.
- AC: `python3 -c "import tomllib; tomllib.load(open('.specs/01_research/yellow_paper_registry.toml','rb'))"`
  exits 0; no theorem-ID collisions (grep THM-AUD-T3/T4 count = 1 entry
  each); no `recorded, not asserted` remains for P3 in the YP.

### 4. Test / 5. Validate (the verification ladder, in order)

- `lake env lean .specs/02_architecture/proofs/audit.lean` (fast)
- `lake build` — all targets, 0 sorries, 0 errors
- tomllib parse (above)
- Write `LOOP-2026-10-29-REPORT.md` (this loop's report; loop 4 left no
  report — note that gap in it, one paragraph, not a work item)
- Commit + push; commit message states the FINDING (storage-level fork
  was unbound under T1; anchored prefix-binding closes it; post-anchor
  window remains, recorded), not "update proofs".
- AC: all four ladder steps exit 0; `git status` clean after push.

## Out of scope (explicit)

- No Rust-side changes to the audit sink/emitter (`crates/`) — the
  shipped policy spec tie is Lean-side only this iteration.
- No new proof file / lakefile target (audit.lean extends in place).
- No Float/rational damping-style machinery — anchor is a Nat hash.
- Do not touch the other recorded items (YP-SANITIZER-012 injection
  boundary) or the v1.0.0 sweep — they stay on the backlog.
- Do NOT recreate the legacy proof tree; `.specs/02_architecture/proofs/`
  is the only one.

## Definition of done

P3 is nowhere marked `recorded, not asserted`; T3 + the T4 counter-
witness compile in the canonical tree with 0 sorries; YP-AUDIT-013 at
v1.1.0 with the new section; registry parses with THM-AUD-T3/T4 appended;
full `lake build` green; loop report written; pushed to origin. If the
prove phase stalls on P3 after exhausting the pitfall checklist, the
documented fallback is the Bool-precondition sweep (candidates table
above) — but P3 discharging cleanly is the expected path.
