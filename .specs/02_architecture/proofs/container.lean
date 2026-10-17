/-
  Lean4 Proof: Container Isolation Safety Properties
  Component: COMP-CONTAINER-001
  Blue Paper: BP-CONTAINER-SANDBOX-001
  Yellow Paper: YP-CONTAINER-014

  Migrated from the legacy tree (.clawdius/specs/02_architecture/proofs/)
  in loop 2, iteration 6, and extended with a layered defense-in-depth
  model (NIST SP 800-190): the misconfiguration escape class is blocked
  iff every hardening layer is set (T1, with per-layer necessity T1a-d),
  while the kernel-exploit and runtime-bug classes are NOT blockable by
  any container layer (T2 — the shared-kernel boundary sits below every
  software layer; mitigating it requires a different trust boundary:
  microVM/hypervisor). All legacy theorems are re-verified.
-/

inductive ContainerState where
  | Created : ContainerState
  | Running : ContainerState
  | Paused : ContainerState
  | Stopped : ContainerState
  | Removed : ContainerState
  deriving Repr, DecidableEq, BEq

structure ResourceLimits where
  memory_bytes : Nat
  cpu_quota : Nat
  pids_limit : Nat
  timeout_secs : Nat
  deriving Repr

def defaultLimits : ResourceLimits := {
  memory_bytes := 536870912, cpu_quota := 100000, pids_limit := 1024, timeout_secs := 300
}

structure SecurityOptions where
  no_new_privileges : Bool
  read_only_root : Bool
  drop_all_capabilities : Bool
  seccomp_profile : Option String
  apparmor_profile : Option String
  deriving Repr

def defaultSecurity : SecurityOptions := {
  no_new_privileges := true, read_only_root := true, drop_all_capabilities := true,
  seccomp_profile := some "default", apparmor_profile := some "docker-default"
}

def containerKilled (usage limit : Nat) : Prop := usage > limit
def cpuThrottled (used quota : Nat) : Prop := used > quota
def forkDenied (current limit : Nat) : Prop := current ≥ limit
def containerStopped (elapsed timeout : Nat) : Prop := elapsed > timeout
def canReach (_url : String) : Bool := false
def canEscalatePrivileges (opts : SecurityOptions) : Bool := !opts.no_new_privileges
def canWriteToFile (opts : SecurityOptions) (_path : String) : Bool := !opts.read_only_root
def hasCapability (opts : SecurityOptions) (_cap : String) : Bool := !opts.drop_all_capabilities
def resourcesReleased (state : ContainerState) : Bool := state == ContainerState.Removed

theorem memory_limit_enforcement (limit usage : Nat) : usage > limit → containerKilled usage limit := id
theorem cpu_quota_enforcement (quota used : Nat) : used > quota → cpuThrottled used quota := id
theorem pid_limit_enforcement (limit current : Nat) : current ≥ limit → forkDenied current limit := id
theorem timeout_enforcement (elapsed timeout : Nat) : elapsed > timeout → containerStopped elapsed timeout := id

theorem network_isolation (networkEnabled : Bool) (url : String) :
    networkEnabled = false → canReach url = false := by intro _; rfl

theorem no_new_privileges_effective (opts : SecurityOptions) :
    opts.no_new_privileges = true → canEscalatePrivileges opts = false := by
  intro h; rw [canEscalatePrivileges, h]; rfl

theorem read_only_root_effective (opts : SecurityOptions) (path : String) :
    opts.read_only_root = true → canWriteToFile opts path = false := by
  intro h; rw [canWriteToFile, h]; rfl

theorem capability_dropping (opts : SecurityOptions) :
    opts.drop_all_capabilities = true → hasCapability opts "CAP_SYS_ADMIN" = false := by
  intro h; rw [hasCapability, h]; rfl

def validNextState : ContainerState → Option ContainerState
  | ContainerState.Created => some ContainerState.Running
  | ContainerState.Running => some ContainerState.Paused
  | ContainerState.Paused => some ContainerState.Running
  | ContainerState.Stopped => some ContainerState.Removed
  | ContainerState.Removed => none

theorem no_invalid_transitions :
    validNextState ContainerState.Created ≠ some ContainerState.Stopped := by
  decide

theorem resource_cleanup (state : ContainerState) :
    state = ContainerState.Removed → resourcesReleased state = true := by
  intro h; rw [resourcesReleased, h]; rfl

-- ---------------------------------------------------------------------------
-- Layered defense-in-depth model (YP-CONTAINER-014, loop 2 it. 6)
-- ---------------------------------------------------------------------------

/-! NIST SP 800-190 layers as one hardening bundle: privilege (no-new-
privileges), filesystem (read-only root), capabilities (all dropped),
syscall surface (seccomp set). The escape-vector taxonomy splits into
three classes: misconfiguration (blocked by the layers), kernel
exploit and runtime bug (crossing the shared-kernel boundary itself —
NOT blockable by any container layer; NIST 800-190 §3.5.2). -/

structure Hardening where
  noNewPrivileges : Bool
  readOnlyRoot : Bool
  capsDropped : Bool
  seccompSet : Bool

inductive EscapeVector where
  | Misconfig : EscapeVector
  | KernelExploit : EscapeVector
  | RuntimeBug : EscapeVector

/-- Which escape vectors a configuration blocks. The misconfiguration
class requires ALL layers; the kernel-exploit and runtime-bug classes
are never blocked by container-layer hardening — they cross the
shared-kernel boundary below it. -/
def blocked (h : Hardening) (v : EscapeVector) : Bool :=
  match v with
  | .Misconfig => h.noNewPrivileges && h.readOnlyRoot && h.capsDropped && h.seccompSet
  | .KernelExploit => false
  | .RuntimeBug => false

/-- T1 (defense-in-depth): the hardened default blocks the entire
misconfiguration escape class. -/
theorem hardened_blocks_misconfig (h : Hardening)
    (h1 : h.noNewPrivileges = true) (h2 : h.readOnlyRoot = true)
    (h3 : h.capsDropped = true) (h4 : h.seccompSet = true) :
    blocked h .Misconfig = true := by
  simp [blocked, h1, h2, h3, h4]

/-- T1a-d (per-layer necessity): removing ANY single layer reopens the
misconfiguration class — the layers compose, none is redundant. -/
theorem layer_noNewPrivileges_necessary (h : Hardening) (hne : h.noNewPrivileges = false)
    (h2 : h.readOnlyRoot = true) (h3 : h.capsDropped = true)
    (h4 : h.seccompSet = true) :
    blocked h .Misconfig = false := by
  simp [blocked, hne]

theorem layer_readOnlyRoot_necessary (h : Hardening) (h1 : h.noNewPrivileges = true)
    (hne : h.readOnlyRoot = false) (h3 : h.capsDropped = true)
    (h4 : h.seccompSet = true) :
    blocked h .Misconfig = false := by
  simp [blocked, h1, hne]

theorem layer_capsDropped_necessary (h : Hardening) (h1 : h.noNewPrivileges = true)
    (h2 : h.readOnlyRoot = true) (hne : h.capsDropped = false)
    (h4 : h.seccompSet = true) :
    blocked h .Misconfig = false := by
  simp [blocked, h1, h2, hne]

theorem layer_seccomp_necessary (h : Hardening) (h1 : h.noNewPrivileges = true)
    (h2 : h.readOnlyRoot = true) (h3 : h.capsDropped = true)
    (hne : h.seccompSet = false) :
    blocked h .Misconfig = false := by
  simp [blocked, h1, h2, h3, hne]

/-- T2 (the honest boundary): NO container-layer hardening blocks the
kernel-exploit or runtime-bug escape classes — those cross the
shared-kernel boundary below every software layer (NIST SP 800-190
§3.5.2: container isolation "is not as high as that provided by
hypervisors"). The mitigation class is a different trust boundary:
microVM / hypervisor (Kata, Firecracker, gVisor). The default hardened
configuration is recorded for reference. -/
theorem kernel_class_unblockable (h : Hardening) :
    blocked h .KernelExploit = false ∧ blocked h .RuntimeBug = false := by
  simp [blocked]

/-- T3: the shipped default hardening blocks the misconfiguration class. -/
theorem default_security_blocks_misconfig :
    blocked
      { noNewPrivileges := true, readOnlyRoot := true,
        capsDropped := true, seccompSet := true }
      .Misconfig = true := by
  simp [blocked]


-- ---------------------------------------------------------------------------
-- Isolation classes and the microVM trust boundary
-- (YP-CONTAINER-014 P4, loop 3 it. 4)
-- ---------------------------------------------------------------------------

/-! P3 proved that NO container-layer hardening closes the kernel class: it
needs a trust-boundary change. P4 models that change honestly instead of
waving at it, and the three real isolation technologies land in three
DIFFERENT classes:

- `sharedKernel` - runc/crun. The host kernel is shared, so a kernel
  exploit crosses straight out (T2 above).
- `intercepted` - gVisor. gvisor.dev/docs/architecture_guide/security states
  gVisor exists "to provide additional defense against the exploitation of
  kernel bugs" and its FAQ treats VMs as a separate posture. Sentry
  reimplements the system API in user space, but memory management stays
  delegated to the host kernel, so the host kernel REMAINS in the guest's
  trusted computing base: less surface, not a new trust domain.
- `microVM` - Kata Containers / Firecracker. Kata's architecture doc calls
  hardware virtualization "a second layer of defence" on top of the
  container, so guest layers still apply; and Firecracker's own jailer
  (docs/jailer.md) exists to isolate the Firecracker process itself (chroot,
  cgroups, uid/gid, netns, new PID namespace, statically linked musl build
  required). The mitigation RELOCATES the boundary into a small hardened
  component instead of eliminating it.

The provable consequences are deliberately modest: a microVM removes the
host kernel from the guest TCB and closes the kernel class; it does not
replace guest hardening; and it adds one component that is now trusted. -/

/-- Where a guest's isolation boundary sits. -/
inductive Isolation where
  /-- runc/crun: one kernel shared by host and guests. -/
  | sharedKernel : Isolation
  /-- gVisor: user-space system API, host kernel still delegated. -/
  | intercepted : Isolation
  /-- Kata/Firecracker: guest kernel behind a VMM boundary. -/
  | microVM : Isolation
  deriving DecidableEq, Repr

/-- Is the host kernel part of this guest's trusted computing base? -/
def hostKernelInTCB : Isolation -> Bool
  | .sharedKernel => true
  | .intercepted => true
  | .microVM => false

/-- Does a VMM process become part of the guest's TCB? -/
def vmmInTCB : Isolation -> Bool
  | .sharedKernel => false
  | .intercepted => false
  | .microVM => true

/-- Escape vectors judged against an isolation class: the
misconfiguration class is still decided by the guest layers, and the
kernel class only stops where the host kernel is not part of the TCB. -/
def blockedWith (h : Hardening) (i : Isolation) (v : EscapeVector) : Bool :=
  match v with
  | .Misconfig => blocked h v
  | .KernelExploit => !(hostKernelInTCB i)
  | .RuntimeBug => !(hostKernelInTCB i)

/-- T4 (P4 discharged): a microVM closes the kernel class - the host
kernel is no longer in the guest TCB, so neither a kernel exploit nor a
runtime bug crosses the boundary. -/
theorem microvm_closes_kernel_class (h : Hardening) :
    blockedWith h .microVM .KernelExploit = true /\
      blockedWith h .microVM .RuntimeBug = true := by
  simp [blockedWith, hostKernelInTCB]

/-- T4a (the honest nuance, gVisor): syscall interception is NOT the same
boundary. The host kernel stays in the TCB, so the kernel class remains
open - gVisor buys reduced surface, not a new trust domain. -/
theorem gvisor_does_not_close_kernel_class (h : Hardening) :
    blockedWith h .intercepted .KernelExploit = false /\
      blockedWith h .intercepted .RuntimeBug = false := by
  simp [blockedWith, hostKernelInTCB]

/-- T4b (P3 survives the new model): no hardening closes the kernel class
while the host kernel is shared. -/
theorem shared_kernel_still_open (h : Hardening) :
    blockedWith h .sharedKernel .KernelExploit = false := by
  simp [blockedWith, hostKernelInTCB]

/-- T4c (Kata's "second layer of defence"): a microVM does NOT replace
guest hardening - the misconfiguration verdict is unchanged. -/
theorem microvm_still_needs_layers (h : Hardening) :
    blockedWith h .microVM .Misconfig = blocked h .Misconfig := by
  rfl

/-- T4c' (the composed default): a hardened guest keeps blocking the
misconfiguration class inside a microVM - the layers are independent and
neither substitutes for the other. -/
theorem hardened_microvm_blocks_misconfig (h : Hardening)
    (h1 : h.noNewPrivileges = true) (h2 : h.readOnlyRoot = true)
    (h3 : h.capsDropped = true) (h4 : h.seccompSet = true) :
    blockedWith h .microVM .Misconfig = true := by
  simp [blockedWith, blocked, h1, h2, h3, h4]

/-- T4d (the boundary is relocated, not eliminated): moving the guest off
the host kernel removes one TCB component and adds one - the VMM, which
Firecracker's jailer exists to harden. -/
theorem boundary_relocated_not_removed :
    hostKernelInTCB .microVM = false /\ vmmInTCB .microVM = true /\
      (vmmInTCB .sharedKernel = false /\ vmmInTCB .intercepted = false) := by
  simp [hostKernelInTCB, vmmInTCB]

/-- T4e: interception and sharing agree on the kernel-class outcome -
reduced surface is not a new trust boundary. -/
theorem interception_shares_kernel_outcome (h : Hardening) :
    blockedWith h .sharedKernel .KernelExploit =
      blockedWith h .intercepted .KernelExploit := by
  simp [blockedWith, hostKernelInTCB]
