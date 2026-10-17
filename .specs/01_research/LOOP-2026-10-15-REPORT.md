# Research Loop Report — 2026-10-15 (Iterations 1–10)

Second ten research→analysis→spec→proof iterations. Every iteration:
pulled real sources, compared with how other systems solve the problem,
wrote or extended the spec (Yellow Paper), and adjusted the Lean proof.
`lake build`: 43 targets, **0 sorries, 0 errors** throughout.

## Outcomes

| Iter | Domain | Research anchor | Spec artifact | Proof outcome |
|---|---|---|---|---|
| 1 | VFS normalization | POSIX §4.13; RFC 3986 §5.2.4 (carried obligation) | YP-VFS-NORMALIZE-008 → APPROVED v1.1.0 | pop-semantics model proven: dot-freedom, idempotence, leading-dot (3 theorems) |
| 2 | Capability system | Capability Myths Demolished; Oxford non-delegable authorities; seL4 capDL | YP-CAPABILITY-010 (new) | grantable-bit model: T1 non_grantable_child, T2 grantability_monotone, T3 transitive — anti-proliferation proven; legacy proof migrated from .clawdius tree |
| 3 | Security policy | Snyk Zip Slip (2018); Android/Sonar mitigations | YP-SECURITY-POLICY-011 (new) | T0 unsoundness WITNESS proven (startsWith bypass); T1 no-escape for floor-clamped normalizeWithin |
| 4 | Sanitizer | OWASP allowlist-vs-denylist; Greshake 2023; GenAI LLM01:2025; 2026 survey | YP-SANITIZER-012 (new) | T1 deletion sound, T2 idempotent, T3 blocklist_incomplete (pigeonhole witness against ANY finite B) |
| 5 | Audit | RFC 6962 CT; Crosby-Wallach 2009; Russ Cox tlog; Stebila ESORICS 2016 | YP-AUDIT-013 (new) | vacuous integrity stubs replaced: ehash chain, T0 chainOf_inj, T1 tamper_evident, T2 append-only consistency |
| 6 | Container | NIST SP 800-190 (+§3.5.2 boundary); escape-vector taxonomy; CVE-2019-5736 | YP-CONTAINER-014 (new) | T1 defense-in-depth composition, T1a-d per-layer necessity, T2 kernel-class unblockable (honest boundary), T3 defaults; legacy proof migrated |
| 7 | Host lifecycle | Candea & Fox, Crash-Only Software (HotOS IX 2003); Dijkstra self-stabilization 1974 | YP-HOST-KERNEL-015 (new) | stopped_is_terminal REVERSED → crash-only restart; T1 self-stabilization (running reachable from every state, kernel + component); T2 no-shortcut |
| 8 | FSM recovery | Harel statecharts 1987; Drusinsky runtime recovery; retry-budget patterns | YP-FSM-016 (new) | failure recovery transitions: deadlock-freedom under failure unconditional; recovery reaches archive within bounded steps; unbounded_retry_livelock PROVEN (retry budget mandatory) |
| 9 | Proof-carrying edits | Necula-Lee PCC (POPL 1997); Yale FLINT; 2026 survey | YP-PROOF-CARRYING-017 (new) | verification-method algebra: trust ordering typed (formal > tool > test > review), T1-T3 order properties, T4 downgrade safety, T5 review_never_machine |
| 10 | Repo map scoring | Aider repo map; Brin-Page 1998; Page-Brin P3 selection soundness | YP-REPO-MAP-018 (new) | T1 accumulation monotone, T2 order-independent, T3 selection soundness (top-1 dominates) |

## Verification state after iteration 10

- Lean: `lake build` — 43 targets, **0 sorries**, **0 errors**.
- Registry: 26 yellow papers.
- 9 of the 21 original Lean proof files now live in the canonical
  `.specs` tree (capability, audit, container, host, fsm migrated out
  of the legacy `.clawdius` tree); legacy tree holds only
  ring_buffer/sandbox remnants (sandbox has an extended canonical
  twin already).

## Defects found and FIXED in this loop (loop 1's carried obligations)

1. **VFS pop-semantics** (loop 1's recorded obligation): implemented and
   proven — YP-VFS-NORMALIZE-008 APPROVED.
2. **Legacy integrity vacuity** (audit): computeChecksum = "" /
   verifyIntegrity = true replaced by a position-bound hash chain with
   proven tamper-evidence.

## Defects found that are NOT fixed in this loop (recorded, not hidden)

1. **Container kernel-class risk**: no container-layer hardening blocks
   kernel-exploit escapes (T2) — mitigated by trust-boundary change
   (microVM), recorded as YP-CONTAINER-014 P4.
2. **FSM retry budget**: unbounded failures livelock
   (unbounded_retry_livelock) — the budget is mandatory and now
   formally justified; the run manager must enforce it.
3. **Component-identity closure** (security policy) and **rank-damping
   monotonicity** (repo map): stated as predictions, not asserted —
   next loop's work.
4. **Attainment** (repo map): the maximum is attained by a real symbol —
   recorded, not asserted.
5. **ehash_inj / postulate_signature_unforgeable**: both remain
   justified cryptographic assumptions (documented class), one per
   security-critical component.

## Method note (loop 2)

The loop's thesis held seven times and reversed once more (iteration 7):
the legacy `stopped_is_terminal` was asserted as a safety property but
was in fact the system's largest liveness defect. Iteration 3 added a
new reversal class — the **unsoundness witness**
(`raw_prefix_bypass_exists`): a spec model proven broken by a
machine-checked counterexample rather than by citation. The verifier
finds what is true; the delta is the spec improvement.
