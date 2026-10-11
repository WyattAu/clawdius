---
document_id: YP-RING-FREE-RUNNING-002
version: 1.0.0
status: DRAFT
domain: Concurrent Data Structures
subdomains: [Ring Buffers, FIFO Ordering, Lock-free Indices]
created: 2026-10-11
author: Loop
supersedes_portion_of: YP-PERFORMANCE-RING-BUFFER-001
proof_files: [proof_ring_buffer_extended.lean]
confidence_level: 0.94
tqa_level: 4
research_iterations: 2
---

# Yellow Paper: Ring Buffers — Free-Running Counters, Definitional Occupancy

## YP-1: Research Basis

| Source | What it establishes |
|---|---|
| Linux BPF ring buffer docs (kernel.org) | The ring is "a power-of-2 sized circular buffer with two logical and **ever-increasing counters**" — producer and consumer counters never wrap; only the *index* is taken modulo. |
| Linux `kfifo.h` | `kfifo_len = in - out`; `kfifo_is_empty ↔ in == out`; `kfifo_is_full ↔ in - out == size`. Occupancy is **arithmetic on two counters**, not a third stored field. |
| subms-spsc-ring-buffer (Rust) | Power-of-two sizing so "index modulo is a bitmask, not a %"; counters on separate cache lines. |
| Watt, *Mechanising the WebAssembly Specification* (CPP 2018) and the WasmCert line | The meta-lesson: proofs get easy when the *representation* is right; mechanisation routinely reveals spec defects before implementation. |

## YP-2: The defects found in the spec

`proof_ring_buffer_extended.lean` modelled the buffer as
`RingBufferState { head, tail, len }` — **three stored fields with no stated
relationship**. Five theorems were `sorry` because no proof can exist over that
model:

1. `len_within_capacity` — false: `len := 999` with capacity 4 is a well-typed value.
2. `head_tail_bounded` — false for the same reason.
3. `empty_iff_zero_len` — asserts `state.len = 0` for **every** state: a theorem
   that all buffers are empty. Not merely unproven; false.
4. `capacity_positive` — false for an arbitrary config (capacity 0 type-checks).
5. `fifo_ordering` — unstatable over three unrelated counters: "oldest" is a
   property of *push order*, which the model never recorded.

## YP-3: Analysis — the field's answer

Every production lock-free ring stores **two free-running counters and derives
occupancy by subtraction**. BPF's ringbuf says the counters are "logical and
ever-increasing"; kfifo computes length as `in - out`; the Rust SPSC crate
indexes by mask against free-running counters. None of them store a length
field, because a length that can disagree with the counters is a corruption
waiting for a scheduler.

**Are we solving a problem the field could not?** The opposite: the field's
representation *is* the fix. The lesson is Lamport's — a specification is an
assumption about behaviour — and the concrete failure mode is storing
redundant state. Verified queue work (Verus-verified SPSC queues, Iris-style
protocols) reaches the same shape: the invariant is `head ≤ tail ≤ head + capacity`,
and everything else is arithmetic.

## YP-4: The corrected model

- `Ring` carries `config` (capacity positive, by constructor), free-running
  `head ≤ tail`, and `tail ≤ head + capacity`. Two fields, two constructor
  proofs — the whole invariant, in the value.
- `len r := r.tail - r.head`, a *definition*. Empty ↔ `head = tail`;
  full ↔ `tail = head + capacity`; `len ≤ capacity` — all arithmetic.
- Physical index of logical position `i` is `(head + i) % capacity`, proved
  in-bounds from the config's positivity.
- **Slot injectivity**: two logical positions inside `len` map to distinct
  physical slots — this is the theorem that says a push cannot overwrite a
  live element, and it is where the power-of-two/positive-capacity
  requirement earns its place.

## YP-5: Falsifiable predictions

- P1: no well-formed `Ring` has `len` exceeding capacity — it is `tail - head` under `tail ≤ head + capacity`.
- P2: pushing when full is impossible to even *state* as a state transition; `push` on a full ring is a distinct outcome, and the non-full push preserves `tail ≤ head + capacity`.
- P3: distinct live positions occupy distinct physical slots (`injective_window`), so a push never collides with an unconsumed element.
