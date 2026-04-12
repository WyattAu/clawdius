# Hacker News Show HN Post - Clawdius v2.0.0

## Title

Show HN: Clawdius – Open-source coding assistant with Lean4 proofs and WASM sandboxing

## Body

Clawdius is a Rust CLI for coding with LLMs. Generated code runs in sandboxed environments instead of raw shell, and core invariants are formally verified with 142 Lean4 proofs.

```
$ clawdius chat                            # Multi-provider LLM chat
$ clawdius generate "a REST API with auth"  # Agent-mode code generation
$ clawdius chat --provider ollama         # 100% local, no data leaves your machine
```

Sandboxing

Generated code runs through sandboxed backends, not bare shell. Three are functional:

- **Container** — Docker/Podman, full process isolation
- **Bubblewrap** — Linux namespace isolation
- **sandbox-exec** — macOS native sandbox

The executor auto-selects the most restrictive backend available. Four more backends exist as stubs (WASM, Filtered, gVisor, Firecracker).

Formal Verification

142 Lean4 theorems prove properties of the core data structures. Example — the ring buffer index masking invariant used in the lock-free SPSC queue:

```lean
-- x % n = x &&& (n - 1) for power-of-2 n, x < 2n
theorem pow2_mod_eq_mask (n x : Nat)
    (hpow : isPowerOfTwo n) (hbound : x < 2 * n) :
    x % n = Nat.land x (n - 1) := by
  have ⟨_, ⟨k, hk⟩⟩ := hpow
  subst hk
  exact (Nat.and_two_pow_sub_one_eq_mod x k).symm
```

All 142 theorems proven, zero `sorry`. One axiom remains:

```lean
axiom postulate_signature_unforgeable (t1 t2 : CapabilityToken) :
    t1.signature ≠ t2.signature → t1.id ≠ t2.id ∨ t1.resource ≠ t2.resource
```

A cryptographic assumption about Ed25519 capability token unforgeability — the same assumption every signature-based security system relies on. Down from 42 axioms.

Architecture

6 crates, 6 protocol layers (JSON-RPC, LSP, MCP, DAP, GraphQL, REST), 4 IDE plugins (VSCode, JetBrains, Neovim, Emacs), plus `clawdius-mcp` for Claude Desktop interop. 3 working LLM providers (Anthropic, OpenAI, Ollama). ~1,200 `unwrap()` calls remain in production code (tracked for remediation). Zero compiler warnings.

Limitations

- No Aider-style autonomous loops (generate, apply, test, retry)
- Plugin marketplace backend exists (7 REST endpoints) but no UI
- gVisor/Firecracker sandboxes are stubs
- IDE inline completions exist but aren't wired to editor plugins

CI runs tests on every push with coverage, clippy, security scanning (cargo-deny, cargo-audit), and Lean4 proof compilation. Release pipeline builds signed binaries for 4 platforms.

```
cargo install clawdius
clawdius setup
clawdius chat --provider ollama --model llama3
```

GitHub: https://github.com/WyattAu/clawdius
