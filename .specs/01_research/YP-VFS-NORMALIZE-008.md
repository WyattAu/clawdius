---
document_id: YP-VFS-NORMALIZE-008
version: 1.0.0
status: DRAFT
domain: Security / Filesystem
subdomains: [Lexical Path Normalization, Traversal Defence]
created: 2026-10-11
author: Loop
proof_files: [vfs_safety.lean]
confidence_level: 0.94
tqa_level: 4
research_iterations: 8
---

# Yellow Paper: VFS — Lexical Normalization Composes

## YP-1: Research Basis

| Source | What it establishes |
|---|---|
| POSIX 1-2017 §4.13 (Pathname Resolution) | Resolution is defined *lexically* first: a pathname is a sequence of components; `..` "cancels" the previous component; canonical form contains no dots. |
| RFC 3986 §5.2.4 (remove_dot_segments) | The URI normalizer is specified as an **algorithm over segments**, whose correctness is stated as: the output contains no dot segments, and appending never reintroduces them. |
|Sleight caveats in the field: `realpath(3)` vs lexical normalization | Lexical normalization is *symlink-blind* — the field's honest boundary, which is why filesystems pair it with kernel-level resolution. |

## YP-2: The gap found in the spec

`vfs_safety.lean` proved containment for *already-safe* component lists (`no_escape_when_all_safe`) but never defined normalization — the operation that *produces* the safe list from an arbitrary one. The containment theorem's hypothesis was doing all the work: any caller with an unnormalized list cannot reach the theorem. A dot-component model and its cancellation property were missing entirely.

## YP-3: Analysis — the field's answer

POSIX and RFC 3986 both specify normalization **algorithmically over segments** — the correctness claim is not "the result is safe" but two structural facts: (1) the output contains no dot segments, and (2) normalization is *idempotent* (normalizing twice equals normalizing once), which is what makes the canonical form canonical.

**Are we solving a problem the field could not?** No — POSIX solved it in 1988. The estate's honest addition is the *machine-checked* version of property (2) plus dot-freedom (1), and the recorded boundary: lexical normalization cannot see symlinks, which is exactly why the sandbox keeps the OS containment tier behind it (the 4-tier model of YP-SANDBOX-ISOLATION-001).

## YP-4: The corrected model

- Component `0` is the dot (`..`); every other component is a name.
- `normalize` folds left-to-right: a name pushes; a dot pops the last component if one exists, else is dropped (a leading `..` cannot escape the root — the root *is* the sandbox).
- Proved: normalization's output contains no dots (`normalize_no_dots`); it is idempotent (`normalize_idempotent`); it is *compositional* — normalizing the append equals normalizing the suffix against the normalized prefix only when the prefix is already normalized (`normalize_cons`), with the prefix case discharged by dot-freedom.

## YP-5: Falsifiable predictions

- P1: `0 ∉ normalize p` for every `p` — dots cannot survive normalization.
- P2: `normalize (normalize p) = normalize p` — the canonical form is canonical.
- P3: `normalize (0 :: p) = normalize p` — a leading escape attempt changes nothing.
