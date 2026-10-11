---
document_id: YP-BUDGET-MONOTONE-006
version: 1.0.0
status: APPROVED (refutation)
domain: LLM Agent Systems / Verified Resource Allocation
subdomains: [Token Budgeting, Greedy Selection, Monotonicity]
created: 2026-10-11
author: Loop
proof_files: [token_budget_scan.lean]
confidence_level: 0.97
tqa_level: 4
research_iterations: 6
---

# Yellow Paper: Budget Monotonicity — Refuted Inside the Proof Assistant

## YP-1: Research Basis (2026 frontier)

| Source | What it establishes |
|---|---|
| *Optimal Skill Selection for LLM Agents with Provable Bicriteria Guarantees* (arXiv 2608.19993) | Greedy packing of skills under a token budget has **no quality guarantee**; the regularized submodular problem admits no approximation better than 1 − 1/e unless P = NP. Their BPS algorithm guarantees ≈63% of optimal benefit. |
| *Token-Budget-Aware LLM Reasoning* (TALE, ACL Findings 2025) | Introduces the **"implicit monotonicity assumption"**: correctness at budget β implies correctness above β — satisfied by 90.91% of sampled cases. An *empirical* assumption about model behaviour, never proved. |
| *Beyond Compaction: Structured Context Eviction* (CWL, 2026) | Budget-aware eviction as a deterministic, LLM-free policy over typed structure — the closest production analogue to this crate's scanner. |
| *A Stackelberg Framework for Resource-Aware LLM Agents* (2026) | Token allocation as sequential decision under explicit budgets; conditional guarantees only. |

## YP-2: What iteration 6 set out to prove — and why it cannot be

The planned theorem: *a larger budget never selects fewer tags*. The proof attempt
survived two inductions before the mathematics refused:

**The selection-count monotonicity of this greedy stream scanner is false, even from the
initial state.** Verified witness (compiled and `decide`-checked in
`token_budget_scan.lean::budget_monotonicity_fails`): budgets **4 ≤ 12**, tags
`[12, 1, 1, 1, 1]` — the small budget selects **4** tags, the large budget selects **1**.

Mechanism: the expensive tag consumes the larger budget whole; the cheap tags never fit
the remainder. The smaller budget skips the expensive tag and harvests the cheap ones.
This is the knapsack greedy failure the selection literature knows (BPS's own motivation),
now *machine-checked for this allocator* rather than cited.

## YP-3: Are we solving a problem the field could not?

**Yes — the reverse direction of the usual one.** TALE *assumes* monotonicity
empirically; BPS proves approximation ratios for a *different* algorithm (seed
enumeration + density-greedy chains). Neither states the streaming greedy scanner's
monotonicity as a property at all, because the field evaluates by benchmarks. The estate's
contribution is small but sharp: the assumption is decidable, and here it is *decided
false*, with the counterexample as a compiled artifact any caller can query.

The corrected claim the scanner *does* carry (proved in the same file):

- **Accounting**: cumulative cost ≤ starting cost + total tag cost — the scanner never
  invents cost (`scan_cost_le_total_add`).
- **Capacity**: selection count ≤ tag count — one tag, at most one selection
  (`scanStep_count_nonneg` family).
- **Budget safety**: cumulative cost ≤ budget at every step (`scanStep_preserves`).

## YP-4: Spec consequence

Any caller that needs "more budget never loses selections" must sort by cost (density
greedy, BPS-style) — stream order is not enough, and this YP is the citation for that
requirement. The scanner's honest contract is the accounting bound, not monotonicity.

## YP-5: Method note

Five of this loop's first six iterations closed `sorry`s whose statements were true;
this iteration's planned statement was **false**, and the proof assistant said so in one
build. That asymmetry is the argument for the loop: research finds what the field
believes, the verifier finds what is actually true, and the delta is the spec
improvement.
