# Hacker News Post - Clawdius v1.2.0

## Title
Show HN: Clawdius – High-assurance AI coding assistant in Rust with zero vulnerabilities

## Body

Hi HN,

I've been working on Clawdius (https://github.com/WyattAu/clawdius), a Rust-native AI coding assistant designed for developers who need more than just code suggestions.

## Why another AI coding assistant?

After using various AI coding tools, I kept running into the same problems:
- **Security concerns**: Most tools run code directly or in opaque containers
- **Hallucinations**: No formal verification of generated code
- **Privacy**: API keys and code sent to third parties
- **Performance**: Node.js/Electron runtimes add latency

## What makes Clawdius different?

**1. Security-first architecture**
- 5 sandbox backends (+ 2 planned) (WASM, gVisor, Firecracker, Bubblewrap, etc.)
- Your API keys stay in host memory - never visible to agents
- Code runs in isolated, just-in-time sandboxes

**2. Formal verification**
- 104 Lean4 theorems proving core behavior
- Yellow Papers establish theoretical ground truth
- Blue Papers provide IEEE 1016-compliant specs

**3. Native performance**
- Pure Rust, <20ms cold boot
- No Electron, no Node.js runtime
- HFT-grade SPSC ring buffers for streaming

**4. Multi-LLM support**
- Anthropic, OpenAI, Ollama (local), Zhipu AI
- 100% private with local LLMs
- Streaming generation with real-time diffs

## What's new in v1.2.0

- **Interactive setup wizard**: `clawdius setup` guides first-time users
- **4 security fixes**: All CVEs patched, zero vulnerabilities
- **Better error messages**: Helpful suggestions instead of cryptic errors
- **Dependency updates**: lancedb 0.27.x, lance 3.0.x

## Quick demo

```bash
# Install
cargo install clawdius

# First-time setup
clawdius setup

# Start coding
clawdius generate --mode agent "Create a REST API with JWT auth"

# Use locally (100% private)
clawdius chat --provider ollama --model llama3
```

## Stats

- 65,834 lines of Rust
- 1,002+ tests passing
- 5 LLM providers
- 5 sandbox backends (+ 2 planned)
- Zero cargo audit vulnerabilities

## What's next

Working on v2.0.0 with:
- Full LLM integration for code/test/doc generation
- Agent-based autonomous workflows
- Multi-file refactoring

Would love feedback from the HN community. What features would you want to see? What's missing?

GitHub: https://github.com/WyattAu/clawdius
Releases: https://github.com/WyattAu/clawdius/releases/tag/v1.2.0
Docs: https://docs.clawdius.dev (coming soon)
