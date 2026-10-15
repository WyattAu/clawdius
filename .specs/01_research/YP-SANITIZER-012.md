---
document_id: YP-SANITIZER-012
version: 1.0.0
status: APPROVED
domain: Security / Input Sanitization
subdomains: [Deletion Sanitization, Blocklist Incompleteness, Metacharacter Model]
created: 2026-10-15
author: Loop
proof_files: [sanitizer_soundness.lean]
confidence_level: 0.94
tqa_level: 4
research_iterations: 3
---

# Yellow Paper: Sanitizer — Deletion Is Provable, Enumeration Is Provably Incomplete

## YP-1: Research Basis

| Source | What it establishes |
|---|---|
| OWASP Input Validation Cheat Sheet | Denylisting "relies on knowing all attacks that exist"; blocking individual characters "rejects legitimate names without making anything safe". Allowlist/deletion is the preferred discipline. |
| MDN Web Docs, Input Validation | Allowlists "default to denying values that the author didn't consider" — the defaults-deny framing. |
| Greshake et al. 2023 (arXiv 2302.12173) | Indirect prompt injection compromises LLM-integrated applications; "effective mitigations of these emerging threats are currently lacking"; obfuscation and payload splitting bypass safety filters. |
| OWASP Gen AI, LLM01:2025 | "It is unclear if there are fool-proof methods of prevention for prompt injection" — the honest boundary for detection-based layers. |
| 2026 Survey (ScienceDirect) | Quantified: input filtering suffers 15–30% false positives and is evadable "through simple obfuscation"; prompt-engineering defenses show 40–60% bypass rates. |

## YP-2: The gap found in the spec

`sanitizer_soundness.lean` proved the blocklist is *large* (`blocklistSize
= 57`, range theorems over IDs) and that path depths are bounded — but
nothing connected the blocklist to soundness. Size is not a security
property: the spec could not distinguish "we block 57 commands" from "we
block the right things", and had no answer to the generative attack
space — commands *not yet invented* that the 57 entries cannot cover.

## YP-3: Analysis — the field's answer

The field's consensus is structural, not enumerative: OWASP and MDN both
mandate allowlist/deletion discipline because denylists are incomplete
against a generative space by construction. Greshake et al. and the 2026
survey extend this to prompt injection: filtering layers are evadable,
detection is heuristic, and no sound filter exists for natural-language
injection. The formally sound core is narrow and mechanical:
**deletion of a defined metacharacter set is verifiable**; enumeration
is not.

**Are we solving a problem the field could not?** No — the field states
the discipline informally. The estate's contribution is the *machine-
checked* pairing: (T1/T2) deletion sanitization is sound and idempotent;
(T3) ANY finite enumerated blocklist is provably incomplete — a theorem,
with the miss constructed, not asserted.

## YP-4: The corrected model

- Commands are component lists; component `0` is the metacharacter
  (shell control byte) — consistent with the path model of
  YP-VFS-NORMALIZE-008.
- `sanitize`: deletion of every `0`. Asserted (T1):
  `sanitize_strips_metachars` — no metacharacter survives. Asserted
  (T2): `sanitize_idempotent` — the clean form is canonical.
- `blocklist_incomplete` (T3): for ANY finite blocklist `B`, there
  exists a command containing the metacharacter that `B` does not list
  (witness: a metachar-prefixed command longer than every blocklisted
  command — a length-based pigeonhole).
- Honest boundary (recorded, not asserted): prompt-injection detection
  is a heuristic layer with measured evasion rates (2026 survey); this
  YP scopes soundness claims to the mechanical shell/path layers only.

## YP-5: Falsifiable predictions

- P1 (proved, T1): `0 ∉ sanitize cs` for every `cs`.
- P2 (proved, T2): `sanitize (sanitize cs) = sanitize cs`.
- P3 (proved, T3): for every finite `B`, a dangerous unlisted command
  exists — adding entries to a blocklist never makes the theorem false;
  it stays incomplete for every finite `B` whatsoever.
