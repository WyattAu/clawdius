---
document_id: YP-SECURITY-POLICY-011
version: 1.0.0
status: APPROVED
domain: Security / Sandboxed Execution
subdomains: [Path Containment, Workspace Scoping, Sibling-Directory Bypass]
created: 2026-10-14
author: Loop
proof_files: [security_policy.lean]
confidence_level: 0.92
tqa_level: 4
research_iterations: 3
---

# Yellow Paper: Security Policy — String Prefixes Are Not Path Containment

## YP-1: Research Basis

| Source | What it establishes |
|---|---|
| Snyk, *Zip Slip* (2018) | The canonical exploitation of character-prefix path checks; the fix everywhere is *normalize, then compare on path segments*. |
| Android Developers, *Zip Path Traversal* | Canonical mitigation pattern: `canonicalPath.startsWith(targetPath.canonicalPath + File.separator)` — the separator boundary is load-bearing. |
| Sonar Source, *OpenRefine Zip Slip* (2023) | Documents the pitfall chain: `getCanonicalPath().startsWith(dir)` still fails on sibling dirs; the patched form uses `Path.normalize().startsWith()` — segment comparison. |
| RFC 3986 §5.2.4 (loop 1) | Normalization correctness = output is dot-free + idempotent; appends never reintroduce escape. |

## YP-2: The gap found in the spec

`security_policy.lean`'s `validatePath` models containment as
`filePath.startsWith workspace` — a *character* prefix. The field has
documented this exact check as the vulnerability itself: any sibling
directory whose name extends the workspace name (`/srv/ws` vs
`/srv/ws-evildir`) passes the check while being fully outside. The spec
also contained a false positive: `filePath.contains ".."` rejects
legitimate names like `a..b` while being blind to separators.

## YP-3: Analysis — the field's answer

Every published mitigation converges on the same two-step discipline:
**(1) normalize, (2) compare segment-wise with a separator boundary.**
The estate already owns step (1) — loop 1's `normalize`
(YP-VFS-NORMALIZE-008) plus loop 2 iteration 1's pop-semantics proofs.
Step (2) needed a component-wise prefix relation and — the estate's
honest addition — a **machine-checked witness that the old model is
unsound** (`raw_prefix_bypass_exists`), not merely a citation.

**Are we solving a problem the field could not?** No — the field solved
it; the estate had re-created the problem. The machine-checked witness
and the floor-clamped normalizer (`normalizeWithin`, chroot semantics
expressed lexically) are the new artifacts.

## YP-4: The corrected model

- `normalizeWithin floor p`: loop-1 normalization with pops **clamped at
  `floor`** — the first `floor` components (the workspace root) cannot
  be popped (chroot semantics, lexically).
- Containment = `isComponentPrefix (normalizeWithin |d| f) (normalizeWithin |d| d)`;
  component-wise, normalized, floor-clamped.
- Asserted (T1): **no-escape** — a path inside the workspace, followed
  by anything at all, normalizes back to a path with at least as many
  components as the workspace (`component_prefix_no_escape`).
- Asserted (T0): `raw_prefix_bypass_exists` — the falsifiable witness
  that the predecessor model was unsound.

## YP-5: Falsifiable predictions

- P1 (proved, T0): there exist workspace/path string pairs where the
  character-prefix check passes and containment fails.
- P2 (proved, T1): floor-clamped normalization is closed under appends —
  no suffix can pop a contained path out of the workspace.
- P3 (proved, `normalized_prefix_is_workspace` + the root invariant,
  loop 3 it. 6): for a file inside the workspace — `f = d ++ rest` — the
  first `|d|` components of the normalized path are EXACTLY `d`
  (component-identity closure). Witnessed from below:
  `unclamped_prefix_escapes` — at `floor = 0` the same walk eats into the
  workspace prefix; the clamp is the entire content of P3.

## YP-6: Component-identity closure, modelled (loop 3, iteration 6)

P1 was the falsifiable witness (the character-prefix model is unsound); T1
was the length form of the fix. P3 — recorded last loop — is the identity
form: not "at least |workspace| components" but "THE workspace components,
in order".

### The lemma chain (each one forced by the one before it)

1. `normAuxWithin_composition` — walking `p ++ q` is walking `p` then `q`
   with the stack threaded through. Needed to split the file path into
   workspace + remainder.
2. `normAuxWithin_all_push` — a walk with no `..` is a pure push sequence:
   the result is `p.reverse ++ acc`. (First draft had the orientation
   backwards — `acc ++ p.reverse` — and a concrete evaluation killed it:
   push PREPENDS, so the deepest component ends up at the head.)
3. `normAuxWithin_root_invariant` — THE heart: for any walk and any
   starting stack with `floor ≤ |acc|`, the last `floor` stack elements
   are unchanged. Pops are clamped above the root; pushes grow below it.
   This is chroot semantics stated as an invariant instead of a promise.
4. `normalized_prefix_is_workspace` (P3) — combine 1–3: the normalized
   file's first `|d|` components equal `d`.
5. `unclamped_prefix_escapes` — the counter-witness: at `floor = 0` the
   same walk removes a workspace component. P3 without the clamp is FALSE,
   so the clamp is the entire content of the property.

### Findings

1. **The accumulator orientation is the whole proof.** `normAuxWithin`
   stores the stack deepest-first (push prepends); the workspace prefix
   therefore lives at the stack's *tail*, and the invariant is a
   `take`-on-the-reversed-stack statement. The first draft of the
   invariant used `take` directly and failed against the pop case
   (`acc.tail` removes the head — the deepest component — never the
   workspace root).
2. **CI-grade honesty on the witness**: P3 is stated WITH its
   counter-witness in the same commit (`unclamped_prefix_escapes`), so the
   property cannot quietly drift into "true because we said so" — remove
   the clamp and a machine-checked escape exists.
3. `List.take_append_of_le_left` changed names across core releases; the
   needed lemma is restated locally as `take_append_last` with its own
   proof, so this file no longer depends on which core release supplied
   which name.
