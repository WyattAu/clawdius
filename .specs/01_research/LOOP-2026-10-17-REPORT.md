# Research Loop Report — 2026-10-17 (Iterations 1–9)

Third research loop: deepening the existing 18 yellow papers rather than
minting new ones. Every iteration: pulled real sources, extended the spec,
adjusted the Lean proof, kept `lake build` green. Final state: **41
targets, 0 sorries, 0 errors**. Registry: 26 yellow papers, five bumped to
v1.1.0. **Both loop-1/loop-2 carried defects are now discharged**, and the
legacy proof tree is retired — `.specs/` is the single source of proof
truth.

## Outcomes

| Iter | Domain | Research anchor | Spec artifact | Proof outcome |
|---|---|---|---|---|
| 1 | Repo map attainment | (carried P4) | YP-REPO-MAP-018 v1.0→(it.9 v1.1.0) | `best_attained`: the maximum is attained by a real symbol — selection cannot return a phantom |
| 2 | Core doctests | (loop 1 defect #1, carried) | crates/clawdius-core README = crate docs | 8 stale doctests repaired; doc suite compiles again |
| 3 | OpenCode MCP toolchain | fleet-side (chezmoi `3d92e62`); official github-mcp-server v2.0.2 | chezmoi repo, not clawdius | github MCP remote (Copilot-entitlement-gated) replaced with the official server + gh-keyring token; fleet 11 servers, sys-check extended |
| 4 | Container microVM boundary | gVisor security model (user-space Sentry, memory still delegated); Kata architecture ("second layer of defence"); Firecracker jailer | YP-CONTAINER-014 v1.1.0 | P4 discharged: `Isolation` (sharedKernel/intercepted/microVM); microVM closes the kernel class; **gVisor does not** (host kernel stays in the TCB); boundary RELOCATED into a jailed VMM, not removed. Spec defect fixed: T1/T1a–d preconditions were `Bool` values, not propositions |
| 5 | Host registry restart | (carried P4) | YP-HOST-KERNEL-015 v1.1.0 | P4 discharged: restart loses no component (T-P4a), resets every entry (T-P4b), invents none (T-P4c), no shortcut (T-P4d), full init sequence valid for all 6 components (T-P4e). Finding: the reset is half the guarantee — the no-shortcut transition relation is the other half |
| 6 | Security component-identity closure | (carried P3) | YP-SECURITY-POLICY-011 v1.1.0 | P3 discharged: root invariant (workspace suffix of the stack invariant under any walk) ⇒ `normalized_prefix_is_workspace`; counter-witness `unclamped_prefix_escapes` proves the clamp IS the content. Finding: the accumulator orientation (deepest-first stack) is the whole proof |
| 7+8 | Legacy tree retirement | (loop 1 defect #2, carried) | lakefile.toml; sandbox_policy.lean (new) | `.clawdius/specs/02_architecture/proofs/` removed from the build; ProofHost repointed at the canonical host.lean (first build of it.5's P4); ProofRingBuffer/ProofSandbox retired (superseded by the *Extended reconstructions); the genuinely-missing halves (tier policy, secret detection) migrated to sandbox_policy.lean. First cut wrongly deleted the whole `.clawdius` agent tree — corrected before commit: ADRs, memory, knowledge graph, and config are working material, not proof residue |
| 9 | Repo-map damping | PageRank non-monotonicity in edges | YP-REPO-MAP-018 v1.1.0 | P4 closed as P4a (attainment, proved) + P4b (damping RECORDED with rationale): damping is non-monotone by design — a new out-edge strictly shrinks every old neighbor's share — so the honest prediction is that generalized monotonicity FAILS. A future damping YP must lead with that witness |

## Verification state after iteration 9

- Lean: `lake build` — 41 targets, **0 sorries**, **0 errors**.
- Registry: 26 yellow papers; five at v1.1.0 (CONTAINER-014,
  HOST-KERNEL-015, SECURITY-POLICY-011, REPO-MAP-018, VFS-NORMALIZE-008).
- **Defect #2 discharged**: the legacy proof tree
  (`.clawdius/specs/02_architecture/proofs/`) is out of the build and out
  of the repo; `.specs/02_architecture/proofs/` is the only proof tree.
- New canonical file: `sandbox_policy.lean` (tier policy + secret
  detection — the halves no canonical file carried).

## Defects found by the proofs in this loop

1. **Container T1/T1a–d preconditions were `Bool` values**
   (`h1 : h.noNewPrivileges`), asserting nothing beyond the field itself.
   Probe-verified the proofs were still sound, then rewrote the
   preconditions as explicit equations. (it. 4)
2. **Host T-P4d draft asserted the claim at the transition-table level**;
   `decide` rejected it — `initialized → running` is valid (the normal
   startup path). The property lives on the post-restart STATE. (it. 5)
3. **Ring-buffer orientation false start**: `acc ++ p.reverse` killed by
   concrete evaluation — push prepends, so the walk result is
   `p.reverse ++ acc`. (it. 6, all_push)
4. **The clamp is the content**: P3 without the floor clamp is provably
   false — `unclamped_prefix_escapes` ships in the same commit as P3 so
   the property cannot drift into "true because we said so". (it. 6)

## Fleet-side outcome (iteration 3)

The github MCP server on every fleet host was silently non-functional
(the `api.githubcopilot.com/mcp/` remote needs a Copilot entitlement).
Replaced with the official `github-mcp-server` (v2.0.2) in local stdio
mode, token sourced from the gh keyring at spawn time — no secret in
dotfiles. Verified live (45 tools; `list_pull_requests` on this very
repo). Plus a new `filesystem` MCP sandboxed to `~/dev`, and `sys-check`
now verifies the server binary AND `gh auth status`. chezmoi `3d92e62`.

## Next-loop candidates

- **Damping non-monotonicity witness** (from YP-REPO-MAP-018 YP-7): the
  machine-checked counter-example that any damped model sacrifices
  `accumulate_mono_append` — small, Nat-only, and it sharpens the
  boundary the additive model sits on.
- **gVisor-class isolation as a first-class tier** (from
  YP-CONTAINER-014 YP-6): the `intercepted` class currently shares the
  trust outcome with `sharedKernel`; a sandbox-tier policy could route
  LLM toolchains through it for surface reduction without claiming a
  trust boundary.
- **Registry YP-version sweep**: five papers at v1.1.0 — a pass to
  re-read every v1.0.0 paper for the same `Bool`-valued-precondition
  defect class found in CONTAINER-014.
