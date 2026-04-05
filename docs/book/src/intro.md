# Introduction to Clawdius

**Clawdius** is a high-assurance AI coding assistant built in Rust. It combines the power of large language models with formal verification, secure sandboxing, and enterprise-grade features to provide a trustworthy development companion.

## Why Clawdius?

### 🛡️ Security First

Clawdius was designed from the ground up with security as a primary concern:

- **5 Sandbox Backends (+ 2 Planned)**: From lightweight WASM to hardware-isolated Firecracker microVMs
- **104 Formal Verification Theorems**: Mathematically proven correctness for critical operations
- **Enterprise SSO**: SAML 2.0, OIDC, Okta, Azure AD, and GitHub integration
- **Comprehensive Audit Logging**: SQLite, Elasticsearch, and webhook backends

### ⚡ Native Performance

Built in Rust for maximum performance:

- **<20ms cold boot**: Faster than any competitor
- **Zero-copy streaming**: Efficient real-time output
- **Memory-efficient**: Minimal resource footprint
- **Cross-platform**: Linux, macOS, Windows support

### 🔧 Extensible Architecture

- **Plugin System**: WASM-based plugins with 26 hook types
- **Multiple LLM Providers**: Anthropic, OpenAI, Ollama, and custom endpoints
- **Graph-RAG**: Enhanced context through code graph analysis
- **Timeline & Checkpoints**: Full session history and rollback

## Feature Comparison

| Feature | Clawdius | Competitors |
|---------|----------|-------------|
| Sandbox Backends | 5 (+ 2 planned) | 1-3 |
| Formal Verification | 104 theorems | None |
| Cold Boot Time | <20ms | 100-500ms |
| Plugin System | WASM + 26 hooks | Limited or None |
| Enterprise SSO | Full (SAML, OIDC) | Limited |
| Audit Logging | 4 backends | Basic or None |
| Graph-RAG | Built-in | External add-on |
| Self-Hosted | Full support | Limited |

## Quick Start

```bash
# Install from crates.io
cargo install clawdius

# Run the interactive setup wizard (new in v1.2.0!)
clawdius setup

# Or manually set your API key
clawdius config set api_key YOUR_ANTHROPIC_API_KEY

# Start chatting
clawdius chat
```

### New: Interactive Setup Wizard

Version 1.2.0 introduces an interactive setup wizard that guides you through:

- **Provider Selection**: Choose from Anthropic, OpenAI, Ollama (local), or Zhipu AI
- **API Key Configuration**: Secure storage using your system keyring
- **Settings Presets**: Balanced, Security-focused, Performance-optimized, or Development mode
- **Ollama Connectivity Check**: Automatic TCP verification for local LLMs

```bash
# First-time setup
clawdius setup

# Quick setup with pre-selected provider
clawdius setup --quick --provider anthropic
```

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                        Clawdius CLI                          │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │   Session   │  │   Context   │  │      Timeline       │  │
│  │   Manager   │  │   Builder   │  │    & Checkpoints    │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │     LLM     │  │   Graph-    │  │      Plugin         │  │
│  │  Providers  │  │    RAG      │  │      System         │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │   Sandbox   │  │   Tool      │  │     Enterprise      │  │
│  │  Executors  │  │   Runner    │  │     Features        │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

## What's Next?

- [Installation Guide](./getting-started/installation.md) - Get Clawdius running on your system
- [Configuration](./getting-started/configuration.md) - Customize Clawdius for your workflow
- [First Chat](./getting-started/first-chat.md) - Your first conversation with Clawdius
- [Architecture Overview](./concepts/architecture.md) - Understand how Clawdius works

## Getting Help

- **Documentation**: [docs.clawdius.dev](https://docs.clawdius.dev)
- **GitHub**: [github.com/WyattAu/clawdius](https://github.com/WyattAu/clawdius)
- **Discord**: [Join our community](https://discord.gg/clawdius)
- **GitHub Discussions**: For Q&A and feature requests

## License

Clawdius is licensed under the [MIT License](https://github.com/WyattAu/clawdius/blob/main/LICENSE).
