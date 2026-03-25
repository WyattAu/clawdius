# Twitter/X Thread - Clawdius v1.2.0 Launch

## Tweet 1/6

🦀 Excited to announce Clawdius v1.2.0!

The high-assurance AI coding assistant built in Rust just got:
• Interactive onboarding wizard
• 4 security fixes
• Better error messages
• Zero vulnerabilities

github.com/WyattAu/clawdius/r…

## Tweet 2/6

🆕 New: `clawdius setup` wizard

First-time setup in seconds:
- Choose your LLM provider (Anthropic, OpenAI, Ollama, Zhipu AI)
- Secure API key storage via keyring
- Smart presets for different workflows
- Automatic Ollama connectivity check

## Tweet 3/6

🔒 Security hardening in v1.2.0:

- Fixed RUSTSEC-2026-0044 (X.509 bypass)
- Fixed RUSTSEC-2026-0048 (CRL scope check)
- Fixed RUSTSEC-2026-0049 (CRL authority)
- Fixed RUSTSEC-2026-0041 (lz4_flex memory leak)

Zero vulnerabilities in cargo audit ✅

## Tweet 4/6

📊 Clawdius by the numbers:

- 65,834 lines of Rust
- 1,002+ test functions
- 104 Lean4 formal proofs
- 5 LLM providers
- 7 sandbox backends

Built for developers who can't afford hallucinations.

## Tweet 5/6

🚀 Coming in v2.0.0:

- Agentic code generation (single-pass, iterative, agent-based)
- Automatic test generation with sandboxed execution
- Multi-format documentation generation

The future of AI-assisted development is here.

## Tweet 6/6

📥 Get started now:

```bash
cargo install clawdius
clawdius setup
clawdius chat
```

Or try local LLMs for 100% privacy:
```bash
clawdius chat --provider ollama --model llama3
```

⭐ Star us on GitHub: github.com/WyattAu/clawdius

#Rust #AI #LLM #OpenSource #DeveloperTools
