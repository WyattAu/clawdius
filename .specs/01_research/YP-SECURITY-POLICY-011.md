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
- P3 (recorded, not asserted): the *first* `floor` components of the
  normalized result equal the workspace components exactly
  (component-identity closure — stronger than the length form of T1;
  next iteration's work).
