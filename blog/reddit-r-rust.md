---
name: Reddit r/rust Post
about: Pre-written post for r/rust community
title: "Show r/rust: Clawdius v1.2.0 - High-assurance AI coding assistant in Rust"
labels: ""
assignees: ""
---

## Post Content

Hey r/rust!

I'm excited to share **Clawdius v1.2.0** - a high-assurance AI coding assistant built in Rust.

## What is Clawdius?

Clawdius is an AI coding assistant that runs natively in Rust (no Node.js runtime), designed for developers who need more than just code suggestions:

- **<20ms cold boot** - Native binary, no Electron
- **5 sandbox backends (+ 2 planned)** - WASM, gVisor, Firecracker, etc.
- **104 formal verification proofs** - Lean4 theorems
- **Zero vulnerabilities** - Clean security audit
- **Local LLM support** - 100% private via Ollama

## What's New in v1.2.0

**Interactive Setup Wizard:**
```bash
clawdius setup
```

Guides you through:
- Provider selection (Anthropic, OpenAI, Ollama, Zhipu AI)
- API key configuration (secure keyring storage)
- Settings presets (Balanced, Security, Performance, Development)

**Security Fixes:**
- 4 CVEs patched
- All dependencies updated
- Zero vulnerabilities in `cargo audit`

## Quick Demo

```bash
# Install
cargo install clawdius

# Setup (new!)
clawdius setup

# Chat
clawdius chat

# Generate code
clawdius generate --mode agent "Create a REST API endpoint"

# Use local LLMs (100% private)
clawdius chat --provider ollama --model llama3
```

## Stats

| Metric | Value |
|--------|-------|
| Rust LOC | 65,834 |
| Tests | 1,002+ passing |
| Lean4 Proofs | 104 theorems |
| Sandbox Backends | 5 (+ 2 planned) |
| LLM Providers | 5 |

## Links

- **GitHub:** https://github.com/WyattAu/clawdius
- **Releases:** https://github.com/WyattAu/clawdius/releases/tag/v1.2.0
- **Docs:** https://docs.clawdius.dev (coming soon)
- **Discord:** https://discord.gg/clawdius

Happy to answer any questions! What features would you like to see next?

---

**Note to poster:** This template is ready to use. Just copy the content above the horizontal line and post to r/rust.
