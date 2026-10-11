---
document_id: YP-SYMBOL-INDEX-002
version: 1.0.0
status: DRAFT
domain: Language Tooling
subdomains: [Incremental Indexing, LSP, Pigeonhole Counting]
created: 2026-10-11
author: Loop
proof_files: [proof_symbol_index.lean]
confidence_level: 0.93
tqa_level: 4
research_iterations: 5
---

# Yellow Paper: Symbol Index — Insertion, Independence, and Counting

## YP-1: Research Basis

| Source | What it establishes |
|---|---|
| Cai, Giarrusso, Rendel, Ostermann, *The Theory and Practice of Self-Adjusting Computation* (ICFP 2014) | Incremental maintenance is correct when a change propagates through recorded *dependencies* — per-document independence is the dependency boundary that makes incremental indexing sound. |
| Microsoft LSP specification (Language Server Protocol 3.17) | Document identity is the URI; symbol queries are per-document. URI inequality is what makes two documents independent. |
| Lambermont, *Applied Incidence Calculus* / the salsa (rust-analyzer) design notes | Salsa's durability/revisions model: a query keyed by a URI is invalidated only by writes to that URI. Same boundary, engineering form. |

## YP-2: The defects found in the spec

1. `independent_uris` was stated with the needed hypothesis — `uri1 != uri2` — and then
   `sorry`ed, with the comment "requires string inequality reasoning". The reasoning
   exists: string equality is decidable (` LawfulBEq`), and the insertion filters the
   foreign document out, so the lookup finds only the original entry. The rewrite needs
   the hypothesis in a form simp can use; the statement was fine, the proof needed the
   decidable-inequality discipline.
2. `kind_counts_bounded` summed three kind counts against `3 * symbols.length` and
   `sorry`ed after one `simp`. The three counts are each bounded by the total (already
   proved, one theorem up: `kind_count_le_total`); the sum bound is then arithmetic.

## YP-3: Analysis — the field's answer

rust-analyzer's salsa and the self-adjusting-computation literature make per-key
independence the *load-bearing* property: everything incremental is built on "a write to
key A does not change the answer for key B". The field states independence exactly where
this spec needed it — as a hypothesis about key inequality — and proves consistency by
induction over the operations.

**Are we solving a problem the field could not?** No. The field's difficulty is
*incremental* propagation through dependencies; the estate's property is the leaf of
that tree. What the estate adds is machine-checking of the leaf for its own index shape.

## YP-4: The corrected model

- `independent_uris` takes the inequality as decidable-boolean evidence
  (`(uri1 == uri2) = false`), which is what the insertion filter consumes, and the proof
  is a simp run through the filter and the find.
- `kind_counts_bounded` composes three instances of the already-proved
  `kind_count_le_total` with `omega`.

## YP-5: Falsifiable predictions

- P1: with `uri1 == uri2` false, inserting `uri2`'s document never changes the symbols
  returned for `uri1`.
- P2: the three kind counts sum to at most three times the symbol count — each is at most
  the total, and the total is the same list.
