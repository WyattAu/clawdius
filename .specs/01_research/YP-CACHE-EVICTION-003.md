---
document_id: YP-CACHE-EVICTION-003
version: 1.0.0
status: DRAFT
domain: Systems / Verified Caching
subdomains: [Eviction Policy, Competitive Analysis, Configured Invariants]
created: 2026-10-11
author: Loop
proof_files: [proof_cache.lean]
confidence_level: 0.92
tqa_level: 4
research_iterations: 3
---

# Yellow Paper: Cache — Configured Invariants and Honest Eviction Order

## YP-1: Research Basis

| Source | What it establishes |
|---|---|
| Sleator & Tarjan, *Amortized efficiency of list update and paging rules* (CACM 1984) | The founding competitive analysis of caching: off-line optimal (Bélády) versus on-line policies (LRU, FIFO) are analysed by *comparing orderings*, not by asserting one. |
| Einziger et al., *TinyLFU: A Highly Efficient Cache Admission Policy* (2017) | Modern admission control is frequency-based (Count-Min Sketch), and its guarantees are probabilistic and stated over the *admission procedure*, not over data layout. |
| Redis / memcached eviction docs | Production eviction (allkeys-lru, volatile-lru) is specified as a policy over recency metadata that the store maintains — the policy's correctness claim is about *which key leaves*, provable only when the recency order is part of the state. |

## YP-2: The defects found in the spec

1. `default_ttl_positive` / `max_entries_positive` asserted positivity of fields of an
   **arbitrary** `CacheConfig` — the same assert-the-conclusion shape as the sandbox's
   `limits_nonzero`. A config with `maxEntries := 0` type-checks, so no proof exists.
2. `fifo_eviction` was the subtlest defect in the spec tree: it *commented* that the list
   was "sorted by createdAt ascending" — inside a `let` that bound the unsorted list —
   and then `sorry`ed. A comment is not a hypothesis. No prover reads comments; the
   ordering the theorem needs was never in the model.

## YP-3: Analysis — the field's answer

Competitive analysis (Sleator–Tarjan) states every eviction claim *relative to an
ordering the policy maintains*. Redis states its policy over the recency metadata it
stores. TinyLFU states admission over sketch counts. **None of them prove a property of
data that was never recorded.** The transferable pattern is the same one iterations 1–2
found (fuel, ring): the invariant must be a *hypothesis or a constructor field*, never a
remark.

**Are we solving a problem the field could not?** No. What the estate adds is the
discipline: `SortedByCreatedAt` as an inductive predicate makes the ordering a typed
hypothesis, so the FIFO claim becomes a two-line induction — and any caller that cannot
supply the ordering cannot call the theorem at all.

## YP-4: The corrected model

- `ValidCacheConfig` carries `defaultTtl > 0` and `maxEntries > 0` as constructor fields;
  the two positivity theorems are constructor extracts.
- `SortedByCreatedAt l` is an inductive predicate (nil / single / cons-with-witnessed
  adjacent order). `fifo_eviction` is restated over a sorted list with explicit head and
  last elements, and proved by induction on the derivation: head ≤ b (adjacent witness),
  and induction gives b ≤ last.

## YP-5: Falsifiable predictions

- P1: every `ValidCacheConfig` has positive TTL and capacity — by construction.
- P2: for any nonempty sorted (≥ 2 entries) list, the first entry's `createdAt` is at
  most the last's — by induction on the ordering derivation. A single unsorted pair
  breaks it, which is exactly why the ordering must be a hypothesis.
