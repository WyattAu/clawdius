---
document_id: YP-SANDBOX-FUEL-002
version: 1.0.0
status: DRAFT
domain: Security / Verified Runtimes
subdomains: [Fuel Accounting, Resource Metering, Verified Interpreters]
applicable_standards: [WebAssembly Core 2.0 (spec), NIST SP 800-53 Rev 5 (CM-4)]
created: 2026-10-11
author: Loop
supersedes_portion_of: YP-SECURITY-CAPABILITY-001
proof_files: [proof_sandbox_extended.lean]
confidence_level: 0.93
tqa_level: 4
research_iterations: 1
---

# Yellow Paper: Fuel Accounting — Outcome-Typed Metering

## YP-1: Research Basis

### Sources pulled

| Source | What it establishes | Why it matters here |
|---|---|---|
| Watt, *Mechanising and Verifying the WebAssembly Specification* (CPP 2018) | The first mechanised Wasm semantics (Isabelle/HOL) with a verified interpreter and a type-soundness proof; found errors in the drafted spec. | The pattern this paper's proofs follow: semantics as an inductive relation, properties proved as *preservation lemmas over transitions* — never as assertions about arbitrary terminal states. |
| Watt, Rao, Pichon-Pharabod, Bodin, Gardner, *Two Mechanisations of WebAssembly 1.0* (ITP 2021) — WasmCert-Isabelle, WasmCert-Coq | Two complete mechanisations of Wasm 1.0; WasmCert-Isabelle proves its executable interpreter's soundness and removes Watt's unverified OCaml harness. | Confirms the estate's model is achievable in a proof assistant, and that the harness (our Rust `WasiSandbox::execute`) is the untrusted part unless its outputs are typed. |
| Wasmtime `Store` documentation (bytecodealliance, 49.x) | Fuel is a store-level counter: `set_fuel`/`get_fuel`; most instructions cost 1 unit, some cost 0; **"when fuel is entirely consumed it will cause wasm to trap"**; epoch interruption is the deliberately *non-deterministic* alternative. | The engineering ground truth for what a fuel invariant can honestly claim: on exhaustion the counter is zero and the run traps — it does *not* satisfy an unconditional `consumed + remaining = initial`. |
| wasmi (wasmi-labs) — built-in fuel metering; audited twice (SRLabs 0.31; Runtime Verification 0.36–0.38) | Fuel metering as a first-class interpreter feature; audits are the industry's trust mechanism absent proofs. | Shows what the estate is competing with: audits rather than theorems. Closing the theorems is the differentiator. |

### The defect found in the spec

`proof_sandbox_extended.lean` asserted, over an **unconstrained** `WasiOutput` structure:

```
theorem fuel_conservation (limits : ResourceLimits) (output : WasiOutput) :
    output.fuelConsumed + output.fuelRemaining = limits.fuelUnits
```

This is **false as a universal statement** — the structure can be built with
`fuelConsumed := 5, fuelRemaining := 5, fuelUnits := 100`. No honest proof exists, which
is why it was `sorry`. The defect is not in the prover work; it is in the *model*: the
theorem asserts a property of the runtime's *outputs* while the runtime's guarantee is a
property of its *execution*.

## YP-2: Analysis — how the field models this

| System | Model | What is proved |
|---|---|---|
| WasmCert-Isabelle/Coq | Semantics as an inductive reduction relation; fuel-free (determinism is the goal) | Preservation/progress over steps; soundness of the type system |
| Wasmtime | Store-level saturating fuel counter + trap on exhaustion; epoch preemption as the coarse non-deterministic alternative | Behavioural (spec tests, audits); no theorems |
| wasmi | Per-instruction fuel charges; two third-party audits | Audits, not theorems |
| Ethereum EVM (gas) | Gas is charged *within* each transition rule; an out-of-gas transition is a distinct semantic rule with its own outcome | Conservation holds *by construction of the rules* — the pattern to adopt |
| **Clawdius (before this YP)** | `WasiOutput` as an unconstrained record; conservation asserted over all values | Nothing — 6 `sorry`s |

**The transferable idea** (from the EVM's rule-level gas and from every verified
interpreter): an outcome is *typed by what held when it was produced*. Do not assert
properties of arbitrary results; make the constructors carry the invariant, and let the
theorems be case analyses over constructors.

**Are we solving a problem the field could not?** No — and the old spec was pretending we
were. The field solved this decades ago by construction; our defect was asserting the
conclusion without the construction. What the estate *adds* is the bridging claim the
papers leave to harnesses: WasmCert-Isabelle explicitly left an *unverified OCaml
harness*; the estate's equivalent gap is the Rust sandbox driver, and outcome-typing is
how the harness's results become checkable.

## YP-3: The corrected model

An execution is typed by how it terminated:

- `FuelExecution.completed consumed remaining h` — the run finished; `consumed + remaining = initial` holds **as a constructor field**, and `remaining` is what the runtime reports left.
- `FuelExecution.exhausted` — the counter hit zero, the trap fired, exit code −1; `remaining = 0` definitionally.

Consequences, all proved by case analysis:

1. *Conservation where it is meaningful:* a completed run conserves fuel (by constructor).
2. *Bounds universally:* every outcome satisfies `consumed ≤ initial` — completed by its
   hypothesis, exhausted because `initial ≥ 0`.
3. *Exhaustion semantics:* exhaustion implies remaining zero and the trap exit code.
4. *Non-negativity everywhere:* by `Nat` and the constructors.

The runtime is then trusted for exactly one thing — that it builds values of
`FuelExecution` — rather than for a theorem about arithmetic it may violate.

## YP-4: What changed in the code

- `proof_sandbox_extended.lean`: `WasiOutput` replaced by `FuelExecution` (outcome-typed);
  `charge` models the saturating step (charge ≤ remaining consumes; otherwise exhausts)
  with a preservation lemma proving `charge` preserves `invariant: consumed + remaining = initial`;
  the six `sorry`s are closed as case analyses over the constructors. Remaining theorems
  (`PathValid`, workspace prefix) are proved from `String` lemmas or retracted where they
  were tautologies in theorem's clothing.

## YP-5: Falsifiable predictions

- P1: any `FuelExecution.completed` value satisfies conservation — by construction, not by trust.
- P2: a saturated charge returns `exhausted`, never a negative or `initial + n` state.
- P3: `N` charges from `initial ≥ N` preserve the invariant at every step (proved by induction).
