---
document_id: YP-RPC-DISPATCH-003
version: 1.0.0
status: DRAFT
domain: Distributed Systems
subdomains: [RPC Semantics, Delivery Guarantees, Relational Specification]
created: 2026-10-11
author: Loop
proof_files: [proof_rpc.lean]
confidence_level: 0.94
tqa_level: 4
research_iterations: 4
---

# Yellow Paper: RPC — Responses are Related to Requests, Not Merely Similar

## YP-1: Research Basis

| Source | What it establishes |
|---|---|
| CMU 15-440 (Distributed Systems), RPC semantics lectures | The taxonomy: exactly-once is **impossible in practice**; at-least-once is safe only for idempotent operations; at-most-once gives "zero, don't know, or once". |
| Tyler Treat, *You Cannot Have Exactly-Once Delivery* (2015), and the Stripe idempotency-key pattern | What production builds instead: at-least-once delivery + idempotent processing = effectively exactly-once. The sender cannot distinguish a lost message from a lost acknowledgement. |
| LogPlayer (arXiv 1911.11286) | TLA+ model-checks exactly-once *processing* for a specific protocol — the guarantee is always relative to a stated specification, never absolute. |
| JSON-RPC 2.0 specification | Notifications carry no ID and are never answered — the one response that must *not* be matched to a request. |

## YP-2: The defects found in the spec

`response_id_matches_request` claimed, for an **arbitrary** response and an arbitrary
request, that the response's ID equals the request's ID. Two responses to two different
requests are both well-typed values of `RpcResponse`, so the statement is false — and
unprovable, hence `sorry`. `batch_ordering` repeated it positionally: equal lengths do
not make index-𝑖 responses answers to index-𝑖 requests.

The model had no notion of *which request a response answers*. That linkage is the only
content the theorem ever needed, and it was absent from the model entirely.

## YP-3: Analysis — the field's answer

Distributed-systems verification (TLA+/PlusCal specs, IronFleet, Verdi) specifies RPC as
a **relation between requests and responses** — a "next" or "reply" action whose
parameters bind the response to its request. The impossibility results (at-most-once vs
at-least-once, FLP-adjacent failure detection limits) are all stated over such relations.
The transferable pattern: make the dispatch relation an inductive predicate whose
constructors carry the linkage, and let correctness be a case analysis over it.

**Are we solving a problem the field could not?** No — the field was explicit that the
*unqualified* claim is impossible. The estate's addition is small but real: the dispatch
relation is machine-checked rather than prose, and notifications are ruled out by having
no constructor.

## YP-4: The corrected model

- `ResponseFor req resp` — an inductive relation with two constructors: a success reply
  carrying `req.id` with a value, and an error reply carrying `req.id` with a code and
  message. Notifications have no constructor: the model *cannot* relate a notification to
  a request, which is the JSON-RPC 2.0 rule made type-level.
- `response_id_matches_request (h : ResponseFor req resp)` — case analysis; each case is
  the constructor's own field.
- `batch_ordering` is restated over a dispatch hypothesis per index: `∀ i, ResponseFor
  (reqs.get i) (resps.get i)` — positional alignment plus the relation, and the proof is
  a case analysis per index.

## YP-5: Falsifiable predictions

- P1: no `ResponseFor` derivation relates a notification to any request — by cases.
- P2: any derivable response's ID equals its request's — by cases on the two constructors.
- P3 (the honest boundary): the model claims nothing about responses outside the relation;
  a response manufactured with a foreign ID is well-typed and simply has no derivation.
