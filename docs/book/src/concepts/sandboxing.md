# Sandboxing

Clawdius provides multi-tier execution isolation to protect your system from untrusted code and LLM-generated commands.

## Overview

All tool execution in Clawdius passes through the sandbox system. Shell commands are checked against blocked patterns, subject to timeouts, and can be restricted to the project directory.

## Shell Sandbox

The shell sandbox is configured in `.clawdius/config.toml`:

```toml
[shell_sandbox]
blocked_commands = [
    "rm -rf /",
    "mkfs",
    "dd if=/dev/zero",
    "dd if=/dev/urandom",
    ":(){ :|:& };:",
    "chmod -R 777 /",
    "chown -R",
    "> /dev/sda",
    "mv /* /dev/null",
    "wget",
    "curl -X POST",
]
timeout_secs = 120
max_output_bytes = 1048576    # 1 MB
restrict_to_cwd = true
```

### Blocked Commands

Commands matching any pattern in `blocked_commands` are rejected before execution. The default list includes destructive system commands and network exfiltration attempts.

### Timeout

All shell commands are killed after `timeout_secs` seconds (default: 120). This prevents runaway processes from consuming resources.

### Output Limits

Output is truncated at `max_output_bytes` (default: 1 MB). This prevents memory exhaustion from commands that produce large output.

### Directory Restriction

When `restrict_to_cwd` is `true` (default), commands cannot access files outside the project directory.

## Sandbox Tiers

Clawdius defines four trust tiers for execution:

| Tier | Trust Level | Use Case | Technology |
|------|-------------|----------|------------|
| 1 | TrustedAudited | Rust/C++ compilation | Bubblewrap passthrough |
| 2 | Trusted | Python/Node.js scripts | Container (Podman) |
| 3 | Untrusted | LLM reasoning | WASM (Wasmtime) |
| 4 | Hardened | Unknown code | Hardened container |

## WASM Sandbox (Brain)

The Brain module runs LLM reasoning in an isolated WASM runtime (Wasmtime):

- No filesystem access
- No network access
- Fuel-based execution limits
- Memory isolation

Wasmtime is a compile-time dependency and always available. The WASI sandbox provides capability-based security.

## Platform Support

| Feature | Linux | macOS | WSL2 |
|---------|-------|-------|------|
| Shell sandbox | Yes | Yes | Yes |
| Bubblewrap | Yes (runtime dep) | N/A | Yes |
| sandbox-exec | N/A | Built-in | N/A |
| WASM (Wasmtime) | Yes | Yes | Yes |
| Containers (Podman) | Optional | Optional | Optional |

## Error Handling

When a sandbox violation occurs, Clawdius returns a `Sandbox` error:

```
Sandbox violation: blocked command pattern detected

This command was blocked for security.
Check .clawdius/config.toml for allowed commands.
```

## Security Model

The security model follows a zero-trust approach:

1. **Trust boundaries** separate the host kernel from untrusted code
2. **Capability tokens** are derived hierarchically (root -> child -> leaf)
3. **Secrets** are never exposed to sandboxed processes or WASM modules
4. **Audit logging** tracks all sandbox operations
