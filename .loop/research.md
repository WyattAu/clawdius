# Loop 5, Iteration 1 — Research: YP-AUDIT-013 P3 (external-signer anchoring)

## Question 1: Does the STH structure match the plan's anchor-commitment sketch?

**Yes — and it constrains the model in one important way: the anchor binds
POSITION and CONTENT together.**

- RFC 6962 §3.5 (https://www.rfc-editor.org/rfc/rfc6962, `TreeHeadSignature`):
  the log signs `{ version, signature_type = tree_hash, timestamp,
  tree_size, sha256_root_hash }`. The signed artifact is not just a root
  hash — it is (tree_size, root_hash): how many entries the head commits to
  AND what they hash to.
- RFC 9162 §4.9–4.10 (https://www.rfc-editor.org/rfc/rfc9162, current;
  obsoletes 6962): `TreeHeadDataV2 { timestamp, tree_size, root_hash }`,
  signed as an STH, produced periodically, fresh within the MMD.
- Consequence for `audit.lean`: the anchor must be modeled as a commitment
  at an explicit anchor point `n` — hypothesis of the form
  `chainOf (recorded.take n) = anchorCommit` — where `n` plays `tree_size`
  and `chainOf (take n)` plays the root hash over that prefix. Hashing a
  bare head without its position would be weaker than what CT signs.
- §4.13 (Final STH) is the same pattern in production: a frozen log
  publishes its final STH as a parameter "to ensure no new entries can be
  added without detection" — an externally-held head commitment as the
  tamper bound.

## Question 2: What does the fork (T4 witness) look like in CT's own terms?

- RFC 6962 §7.3: violation of append-only is "presenting two different,
  conflicting views of the Merkle Tree at different times and/or to
  different parties"; "As soon as two conflicting Signed Tree Heads for
  the same log are detected, this is cryptographic proof of that log's
  misbehavior."
- RFC 9162 §11.3: proof of misbehavior is "an STH with a root hash that
  does not match the one calculated from a copy of the log."
- This validates the T4 counter-witness shape: two distinct histories,
  each verifying against its OWN stored head — self-consistent forks.
  T1 cannot see them because it quantifies over ONE stored chain; the
  forger who controls storage supplies the head that matches the forged
  history. The anchor is what moves a head commitment OUTSIDE the
  forger's control; once two verifiers hold commitments, the conflicting
  heads are the public proof.

## Question 3: The trust gap, stated by the standard itself (P3 grounding)

- RFC 9162 §1 (Introduction): "The log auditing mechanisms described in
  this document can be circumvented by a misbehaving log that shows
  different, inconsistent views of itself to different clients.
  Therefore, it is necessary to treat each log as a trusted third party."
  — This is P3's "the model trusts whoever stores the head", verbatim,
  from the current standard.
- RFC 6962 §5: "All clients should gossip with each other, exchanging
  STHs at least; this is all that is required to ensure that they all
  have a consistent view."
- RFC 9162 §8.3 (Auditing): four auditable properties, including "the
  consistency of the log view presented to all query sources"; a failed
  audit leaves "the auditor ... in possession of signed proof of the
  log's misbehavior."

## Question 4: Scope line for the proof — gossip is NOT standardized

- RFC 9162 §11.3: append-only violations are "detected by multiple
  clients comparing their instances of the STHs. This technique, known
  as 'gossip', is an active area of research and not defined here."
  (6962 promised a separate gossip document; none was standardized.)
- Scope consequence: the theorem proves what a HELD anchor commitment
  gives you (prefix binding); how commitments distribute among verifiers
  is out of scope and stays recorded (P3b). The post-anchor window is
  grounded too: RFC 9162 §11.1 shows CT's own window-bounding (a
  misissued cert is usable at most ~2×MMD before it is auditable) —
  anchoring BOUNDS the forgery window, it does not close it.

## Question 5: Estate checks

- **"zatona/CT checklist" (YP-AUDIT-013 P3 wording): no resolvable
  artifact.** No `zatona` repo exists in
  `~/dev/src/github.com/WyattAu/` (directory listing checked). The new
  YP section should note the pointer is dangling and cite the RFCs
  directly instead.
- **No upstream Rust code models audit anchoring.** Grep of
  `crates/**.rs` for anchor/zatona hits only
  `crates/clawdius-core/src/tools/edit_cascade.rs`, whose "anchors" are
  edit-matching strategies (line-number/prefix-suffix anchoring) —
  unrelated. The Lean model is the first anchor model in the estate;
  the plan's no-Rust-changes scope stands.
- Existing proof assets confirmed in `audit.lean`: `ehash`/`ehash_inj`
  (justified assumption), `chainOf`, `Verify`, T1 `tamper_evident`,
  T2 `chain_append`/`verify_append`. T3/T4 extend Part 2; no new axioms.

## Model refinements adopted (plan otherwise unchanged)

1. Anchor hypothesis carries the explicit prefix length `n` (tree_size
   analog), not a bare head hash — §3.5/§4.9.
2. T4 is framed as the model-level "conflicting STHs" (§7.3/§11.3);
   its one-line comment cites that framing.
3. P3b (recorded): the distribution/gossip half is deliberately out of
   scope because the standard itself does not define it (§11.3); the
   post-anchor window is bounded, not closed (§11.1 analog).

## Sources

| Source | What it grounds | Status |
|---|---|---|
| RFC 6962 §3.5, §5, §5.3, §7.3 (Laurie/Langley/Kasper, 2013) | STH structure binds (tree_size, root_hash); gossip requirement; conflicting-STH = proof of misbehavior | obsolete by RFC 9162; cited via YP-1 already |
| RFC 9162 §1, §4.9–4.10, §4.13, §8.3, §11.1, §11.3 (Laurie/Messeri/Stradling, 2021) | trusted-third-party admission; TreeHeadDataV2; Final STH; audit properties; MMD window; gossip undefined | CURRENT — new citation for YP-AUDIT-013 YP-N |
| Russ Cox, "Transparent Logs for Skeptical Clients" (2019, research.swtch.com/tlogs) | client-side no-trust-in-operator framing | already in YP-1; re-used, not re-fetched |
| `audit.lean` (canonical tree) | existing T1/T2, `ehash_inj` assumption, Part 2 layout | read directly |

Research does NOT invalidate the plan: the model sketch is confirmed
with one refinement (anchor binds position + content). Proceeding to
prove phase with the plan's acceptance criteria intact.
