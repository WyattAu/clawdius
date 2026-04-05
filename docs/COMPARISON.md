# Clawdius vs Competitors

A comprehensive comparison of Clawdius with other AI coding assistants.

---

## Quick Summary

| Feature | Clawdius | Claude Code | Cursor | Aider | OpenClaw | Roo Code |
|---------|----------|-------------|--------|-------|----------|----------|
| **Runtime** | Rust (Native) | Node.js | Electron | Python | Node.js | VSCode |
| **Boot Time** | <20ms | ~500ms | ~2s | ~300ms | ~500ms | ~100ms |
| **Memory** | ~100MB | ~200MB | ~500MB | ~150MB | ~200MB | ~300MB |
| **Sandboxing** | 5 backends (+ 2 planned) | None | Limited | None | None | None |
| **Formal Proofs** | 104 theorems | None | None | None | None | None |
| **Enterprise SSO** | SAML/OIDC | Limited | Limited | None | None | None |
| **Plugin System** | WASM | None | Limited | None | None | Limited |
| **Offline/LAN** | Full | Partial | Partial | Full | Partial | Partial |
| **Open Source** | Apache 2.0 | Proprietary | Proprietary | Apache 2.0 | MIT | MIT |

---

## Detailed Comparison

### 1. Security & Sandboxing

#### Clawdius ✅
- **5 sandbox backends (+ 2 planned):** WASM, Filtered, Bubblewrap, Sandbox-exec, Container, gVisor [v1.7.0], Firecracker [v1.7.0]
- **Capability tokens:** Fine-grained permission system
- **Keyring storage:** OS-native secure credential storage
- **Command filtering:** Blocks dangerous commands (rm -rf /, fork bombs, etc.)
- **Network isolation:** Optional network disable per execution

```rust
// Example: Clawdius sandbox configuration
SandboxConfig {
    tier: SandboxTier::Hardened,  // Uses container/gVisor
    network: false,                // No network access
    mounts: vec![MountPoint::readonly("/src")],
    memory_limit: Some(512 * 1024 * 1024),  // 512MB
}
```

#### Claude Code ⚠️
- No sandboxing by default
- Raw shell access
- API keys in environment variables
- No execution isolation

#### Cursor ⚠️
- Limited file system sandboxing
- VSCode extension model
- No command isolation

#### Aider ❌
- No sandboxing
- Direct shell execution
- Environment variable credentials

### 2. Performance

#### Clawdius ✅
```
Benchmark: Cold Start
Clawdius:    18ms   ████████░░
Claude Code: 487ms  █████████████████████████████████████████████
Cursor:      2.1s   ████████████████████████████████████████████████████████████████████████████████
Aider:       312ms ████████████████████████████

Benchmark: Memory Usage (Idle)
Clawdius:    98MB   ████████
Claude Code: 187MB  ███████████████████
Cursor:      512MB  ████████████████████████████████████████████████████████████████████████████████
Aider:       156MB  ████████████████
```

### 3. Formal Verification

#### Clawdius ✅ (Unique Feature)
- **104 Lean4 theorems** covering core algorithms
- Mathematical proofs for:
  - Plugin system safety
  - Container isolation properties
  - Audit logging completeness
  - SSO authentication security
  - Sandbox boundary integrity

```lean
-- Example: Plugin Safety Theorem
theorem plugin_memory_isolation (p : Plugin) (s : SystemState) :
  p.executes_in_sandbox → 
  ¬p.can_access_host_memory s :=
  by
    intro h_sandbox h_access
    exact sandbox_isolation p s h_sandbox h_access
```

#### All Others ❌
- No formal verification
- Reliance on testing only
- No mathematical guarantees

### 4. Enterprise Features

| Feature | Clawdius | Claude Code | Cursor |
|---------|----------|-------------|--------|
| SSO (SAML 2.0) | ✅ | ⚠️ Enterprise | ⚠️ Enterprise |
| SSO (OIDC) | ✅ | ⚠️ Enterprise | ⚠️ Enterprise |
| Okta Integration | ✅ | ⚠️ | ⚠️ |
| Azure AD | ✅ | ⚠️ | ⚠️ |
| Audit Logging | ✅ Multi-backend | ⚠️ Basic | ⚠️ Basic |
| SOC 2 Template | ✅ | ❌ | ❌ |
| HIPAA Template | ✅ | ❌ | ❌ |
| GDPR Template | ✅ | ❌ | ❌ |
| Team Permissions | ✅ 23 permissions | ⚠️ Basic | ⚠️ Basic |
| Self-Hosted | ✅ Full | ❌ | ❌ |

### 5. LLM Provider Support

| Provider | Clawdius | Claude Code | Cursor | Aider |
|----------|----------|-------------|--------|-------|
| Anthropic | ✅ | ✅ (Only) | ✅ | ✅ |
| OpenAI | ✅ | ❌ | ✅ | ✅ |
| Ollama (Local) | ✅ | ❌ | ⚠️ | ✅ |
| Z.AI | ✅ | ❌ | ❌ | ❌ |
| Custom/Open | ✅ | ❌ | ⚠️ | ✅ |

### 6. Code Intelligence (Graph-RAG)

#### Clawdius ✅
- **SQLite + LanceDB:** Hybrid graph-vector storage
- **Tree-sitter:** 5 language parsers (Rust, Python, JS, TS, Go)
- **Symbol extraction:** Functions, classes, imports, relationships
- **Semantic search:** Vector similarity for documentation
- **Multi-lingual research:** 16 language support (EN/ZH/RU/JP/etc.)

```bash
# Index your codebase
clawdius index .

# Query with semantic understanding
clawdius chat
You: Where is the authentication middleware defined?
```

#### Claude Code ⚠️
- Basic context window
- No persistent code graph
- Limited semantic understanding

#### Cursor ⚠️
- Codebase indexing
- Vector similarity search
- No structural graph

### 7. Plugin System

#### Clawdius ✅
- **WASM runtime:** Sandboxed plugin execution
- **26 hook types:** before_edit, after_commit, on_startup, etc.
- **Marketplace:** Plugin discovery and installation
- **Manifest validation:** TOML-based plugin metadata

```toml
# plugin.toml
[plugin]
id = "my-plugin@1.0.0"
name = "My Plugin"
hooks = ["before_edit", "after_commit"]
wasm = "plugin.wasm"
```

#### Cursor ⚠️
- VSCode extension model
- Limited API surface

#### Others ❌
- No plugin system

### 8. IDE Integration

| IDE | Clawdius | Claude Code | Cursor | Aider |
|-----|----------|-------------|--------|-------|
| VSCode | ✅ Extension | ✅ Native | ✅ Native | ✅ Terminal |
| JetBrains | 📋 Planned | ❌ | ❌ | ❌ |
| Vim/Neovim | ✅ CLI | ✅ CLI | ❌ | ✅ CLI |
| Emacs | ✅ CLI | ✅ CLI | ❌ | ✅ CLI |

### 9. Licensing & Pricing

| Aspect | Clawdius | Claude Code | Cursor | Aider |
|--------|----------|-------------|--------|-------|
| License | Apache 2.0 | Proprietary | Proprietary | Apache 2.0 |
| Price | Free | $20/mo | $20/mo | Free |
| Self-Hosted | ✅ | ❌ | ❌ | ✅ |
| Enterprise | ✅ | Custom | Custom | N/A |

---

## When to Choose Clawdius

### ✅ Choose Clawdius If:

1. **Security is paramount** - You can't afford AI running unrestricted commands
2. **You need formal guarantees** - Mathematical proofs matter for your use case
3. **Enterprise deployment** - SSO, audit logging, compliance templates
4. **Performance matters** - Sub-20ms startup, <100MB memory
5. **You want extensibility** - WASM plugin system
6. **Offline/local-first** - Full functionality without internet
7. **Open source required** - Apache 2.0, fully auditable

### ⚠️ Consider Alternatives If:

1. **Claude-native workflow** - Claude Code for tight Anthropic integration
2. **VSCode-native** - Cursor for integrated IDE experience
3. **Simple scripting** - Aider for lightweight terminal use

---

## Migration Guides

### From Claude Code

```bash
# Export your config
claude-code config export > claude-config.json

# Convert to Clawdius format
clawdius migrate --from claude-code --config claude-config.json

# Start using Clawdius
clawdius chat
```

### From Aider

```bash
# Aider uses .aider.conf.yml
clawdius migrate --from aider --config .aider.conf.yml

# Your context files are preserved
clawdius chat --with src/
```

### From Cursor

```bash
# Export Cursor settings
# (In Cursor: Settings → Export)

# Import to Clawdius
clawdius migrate --from cursor --settings cursor-settings.json
```

---

## Benchmark Sources

All benchmarks conducted on:
- **Hardware:** AMD Ryzen 9 5950X, 64GB RAM, NVMe SSD
- **OS:** Ubuntu 22.04 LTS
- **Date:** 2026-03-11
- **Methodology:** 10 runs, averaged, cold start after reboot

Full benchmark data: [BENCHMARKS.md](./BENCHMARKS.md)

---

*Last updated: 2026-03-11 | Clawdius v1.0.0-rc.1*
