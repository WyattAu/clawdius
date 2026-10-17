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
- P4 (proved, T4/T4a–e, loop 3 it. 4): microVM placement closes the
  kernel class (`blockedWith h .microVM .KernelExploit = true`) because
  the host kernel leaves the guest TCB — but it does **not** replace guest
  hardening (T4c/T4c′: the misconfiguration verdict is unchanged), and it
  **relocates** rather than removes the boundary: a VMM enters the TCB
  (T4d). Syscall interception (gVisor) does **not** close the kernel
  class (T4a) — reduced surface, same trust domain (T4e).

## YP-6: The microVM boundary, modelled (loop 3, iteration 4)

P3 established that the kernel class needs a trust-boundary *change*, and
left P4 as a recorded-but-unasserted obligation. Iteration 4 discharged it
by reading the three real isolation technologies instead of guessing at
them — and they do not agree.

### Sources

- **NIST SP 800-190** (Application Container Security Guide, Sept 2017) —
  container isolation "is not as high as that provided by hypervisors";
  shared-kernel boundary sits below every software layer. (Basis of P3.)
- **gVisor security model** (`gvisor.dev/docs/architecture_guide/security`)
  — gVisor exists "to provide additional defense against the exploitation
  of kernel bugs by untrusted userspace code". *Additional* defense, and
  its FAQ treats VMs as a separate posture. Sentry reimplements the system
  API in user space, but memory management is still delegated to the host
  kernel: the host kernel remains in the TCB.
- **Kata Containers architecture**
  (`github.com/kata-containers/kata-containers/docs/design/architecture`)
  — hardware virtualization is "a second layer of defence" on top of the
  container. Containers still run inside the VM.
- **Firecracker jailer** (`firecracker/docs/jailer.md`) — the jailer exists
  to isolate *the Firecracker process itself* (chroot, cgroups, uid/gid,
  netns, new PID namespace; statically linked musl build required, version
  matched to the jailer).

### The three classes

| Class | Technology | Host kernel in guest TCB? | New TCB component |
|-------|-----------|--------------------------|-------------------|
| `sharedKernel` | runc/crun | yes | — |
| `intercepted` | gVisor | **yes** (memory delegated) | Sentry |
| `microVM` | Kata / Firecracker | no | VMM (jailed) |

### Findings

1. **gVisor is not a microVM** (T4a, T4e). Marketing groups it with
   sandboxing, but it shares the trust outcome with a plain container:
   `blockedWith h .intercepted .KernelExploit = false`. It reduces attack
   surface; it does not create a trust domain.
2. **A microVM closes the kernel class** (T4) — the first thing in this
   component that blocks it.
3. **A microVM does not replace hardening** (T4c/T4c′). Kata's own
   framing is a *second layer*: the misconfiguration verdict is literally
   unchanged (`rfl`), so shipping a microVM and dropping seccomp/caps would
   regress the one class that is currently closed.
4. **The boundary is relocated, not eliminated** (T4d). MicroVM removes the
   host kernel from the TCB and adds the VMM. That is a trade, not a proof
   of safety: the guarantee becomes "the VMM is small, jailed, and
   auditable" (hence the jailer). Honest framing for the run manager: the
   kernel-class risk is *relocated to a component we now must patch*, not
   solved.

### Spec defect found while proving this

T1/T1a–d stated their preconditions as `h1 : h.noNewPrivileges` — a `Bool`
*value*, not a proposition, so each precondition asserted nothing beyond
the field itself. The proofs were still sound (a `false` layer cannot
satisfy T1 — verified by probe), but the statements were opaque. Rewritten
as explicit equations (`= true` / `= false`); same proofs, readable
preconditions.
