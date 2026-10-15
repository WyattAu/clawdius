---
document_id: YP-CONTAINER-014
version: 1.0.0
status: APPROVED
domain: Security / Sandboxed Execution
subdomains: [Container Isolation, Defense-in-Depth, Shared-Kernel Boundary]
created: 2026-10-15
author: Loop
proof_files: [container.lean]
confidence_level: 0.92
tqa_level: 4
research_iterations: 3
---

# Yellow Paper: Container — Defense-in-Depth Blocks Misconfiguration, Not the Kernel

## YP-1: Research Basis

| Source | What it establishes |
|---|---|
| NIST SP 800-190, *Application Container Security Guide* | The layer taxonomy (namespace isolation, cgroups, capabilities, filesystem, seccomp/LSM) AND the honest boundary: §3.5.2 — shared-kernel container isolation "is not as high as that provided by hypervisors". |
| Raven.io, *Container Runtime Security* | The escape-vector taxonomy: misconfiguration, Linux kernel vulnerabilities, container runtime bugs. "No single mechanism solves every escape path." |
| Northflank / Varsity analyses | The boundary-count argument: VM escape = guest kernel + hypervisor (3 boundaries); container escape = 1 boundary (the shared kernel's enforcement). |
| CVE-2019-5736 (runc overwrite) | Worked example: a runtime-bug escape that bypasses namespace/cgroup/seccomp layers by attacking the runtime below them. |

## YP-2: The gap found in the spec

`proof_container.lean` (legacy tree) proved resource-limit enforcement
and security-option effects — but every "enforcement" theorem was
definitional (`usage > limit → containerKilled usage limit := id`):
the theorem restated its own definition. The spec contained **no
composition claim** (do the layers work together?), **no necessity
claim** (is any layer redundant?), and **no honest boundary** (what do
the layers NOT stop?).

## YP-3: Analysis — the field's answer

NIST 800-190's layering is compositional by intent: the misconfiguration
escape class requires privilege escalation AND filesystem write AND
retained capabilities AND unfiltered syscalls — so setting all four
layers closes the class. The necessity direction matters equally:
dropping any single layer reopens it (per-layer theorems, T1a–d).

The boundary: a shared-kernel exploit bypasses namespaces, cgroups and
seccomp *simultaneously* — they are one enforcement point below the
workload. NIST: hypervisor isolation is stronger; Northflank: "escaping
to the host via kernel exploit: not prevented". Mitigation for that
class is a different trust boundary: microVM/hypervisor (Kata,
Firecracker, gVisor) or confidential computing.

**Are we solving a problem the field could not?** No — NIST states the
taxonomy. The machine-checked contributions: layer composition (T1),
per-layer necessity (T1a–d), and the recorded unblockability of the
kernel class (T2) — turning the survey's prose into checked facts that
fail loudly if the model drifts.

## YP-4: The corrected model

- `Hardening`: the four-layer bundle (noNewPrivileges, readOnlyRoot,
  capsDropped, seccompSet).
- `blocked h v`: misconfiguration blocked iff all four layers set;
  kernel-exploit and runtime-bug never blocked.
- Asserted (T1): `hardened_blocks_misconfig` — all layers ⇒ misconfig
  class closed. Asserted (T1a–d): per-layer necessity — each layer's
  removal reopens the class.
- Asserted (T2): `kernel_class_unblockable` — no hardening value blocks
  the kernel/runtime classes.
- Asserted (T3): `default_security_blocks_misconfig` — the shipped
  default blocks the misconfiguration class.
- All legacy theorems re-verified under the migrated model.

## YP-5: Falsifiable predictions

- P1 (proved, T1): `blocked h .Misconfig = true` for the fully hardened
  configuration.
- P2 (proved, T1a–d): for each layer, a configuration without it has
  `blocked h .Misconfig = false`.
- P3 (proved, T2): for every hardening value `h`, `blocked h
  .KernelExploit = false` — kernel-class risk requires a trust-boundary
  change (microVM), not more container options.
- P4 (recorded, not asserted): microVM placement (Kata/Firecracker)
  restores the 3-boundary structure; modeling that boundary is future
  work (component COMP-SANDBOX-002).
