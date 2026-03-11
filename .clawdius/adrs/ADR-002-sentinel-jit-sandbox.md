# ADR-002: Sentinel JIT Sandbox Architecture

## Status
Accepted

## Context
Clawdius executes code from multiple sources with varying trust levels:
- **User-provided code**: Potentially malicious or buggy
- **LLM-generated code**: Subject to "Brain-Leaking" attacks where compromised responses attempt privilege escalation
- **Third-party dependencies**: Supply chain attack vectors
- **Repository configurations**: Settings.toml files could contain RCE payloads

Traditional approaches have significant limitations:
- **Docker-only**: High overhead (~100ms startup) for short-lived operations
- **SELinux/AppArmor**: Complex configuration, OS-specific, difficult to audit
- **seccomp-bpf**: Too low-level, error-prone, insufficient granularity

The system requires defense-in-depth with:
1. Just-In-Time sandbox selection based on trust level and toolchain
2. Capability-based access control with unforgeable tokens
3. Secret isolation preventing credential exfiltration
4. Sub-50ms sandbox startup for responsive UX

## Decision
Implement a **4-tier JIT sandboxing architecture** with capability-based security:

### Tier Selection Matrix
| Tier | Name | Technology | Trust Level | Toolchains |
|------|------|------------|-------------|------------|
| 1 | Native Passthrough | None | TrustedAudited | Rust, C++, Vulkan |
| 2 | OS Container | bubblewrap/sandbox-exec | Trusted | Node.js, Python, Ruby |
| 3 | WASM Sandbox | wasmtime | Untrusted | LLM Reasoning (Brain) |
| 4 | Hardened Container | gVisor/Kata | Untrusted | Any |

### Capability System
- HMAC-signed tokens with permission bitmap
- Attenuation-only derivation (permissions can only decrease)
- Resource scoping (paths, hosts, environment variables)
- Optional expiration timestamps

### Secret Isolation
- Host acts as sole network proxy for authenticated requests
- Secrets stored in OS keychain (libsecret/Keychain/Credential Manager)
- Secrets never cross sandbox boundary
- Secure memory zeroing after use

## Consequences

### Positive
- **Defense-in-depth**: Multiple isolation layers provide fail-safe behavior
- **Performance**: Native execution for trusted code; WASM for Brain with ~10ms startup
- **Portability**: Platform-specific backends via HAL (bubblewrap on Linux, sandbox-exec on macOS)
- **Auditability**: Capability tokens are signed and traceable
- **Compliance**: Meets SEC 15c3-5, NIST SP 800-53 requirements

### Negative
- **Complexity**: 4-tier system requires careful configuration and testing
- **Platform dependencies**: bubblewrap, sandbox-exec, gVisor availability varies
- **Debugging difficulty**: Sandboxed execution harder to introspect
- **WASM limitations**: Brain component restricted to WASM-compatible operations

## Alternatives Considered

### Docker-only
**Rejected**: 100ms+ startup time violates UX requirements; complex configuration attack surface; resource overhead unacceptable for short tasks.

### SELinux Policies
**Rejected**: Complex policy authoring; OS-specific; difficult to verify correctness; requires root privileges.

### seccomp-bpf Only
**Rejected**: Too low-level; syscall-level granularity insufficient; error-prone filter programming; no filesystem isolation.

### Firecracker MicroVMs
**Rejected**: Higher overhead than WASM for Brain component; requires KVM access; not available on macOS.

### gVisor for All Sandboxing
**Rejected**: Unnecessary overhead for trusted code; Tier 1 native execution provides better performance for audited Rust/C++.

## Related Standards
- **NIST SP 800-53**: AC-3 (Access Enforcement), SC-3 (Security Function Isolation)
- **OWASP ASVS**: V1.5 (TCB Minimization), V5.3 (Sandboxing)
- **SLSA**: Build L3 (Hermetic Builds)
- **SEC Rule 15c3-5**: Pre-trade Risk Controls (for Broker mode)

## Related ADRs
- ADR-003: WASM Runtime Selection (Wasmtime)
- ADR-001: Rust Native Implementation

## Date
2026-03-08

## Author
Construct (Systems Architect Agent)
