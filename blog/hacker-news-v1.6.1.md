# Hacker News Post - Clawdius v1.6.1 Technical Preview

## Title
Show HN: Clawdius – Formally verified AI coding engine in Rust

## Body

Hi HN,

I'm sharing the Technical Preview of Clawdius (https://github.com/WyattAu/clawdius), a Rust-native AI coding engine with formal verification and JIT sandboxing.

The elevator pitch: 122K lines of Rust, 142 Lean4 theorems proven correct, zero panics, zero compiler warnings, zero CVEs. Everything an AI coding assistant generates runs in isolated sandboxes — not raw shell access.

## Technical Highlights

- **Formal verification:** 142 Lean4 theorems covering ring buffers (2ns push, 1ns pop), wallet guards (16ns), and sandbox isolation. Zero `sorry` — 39 axioms only for external I/O/syscall boundaries.
- **5 sandbox backends:** WASM, Filtered, Bubblewrap, sandbox-exec, Container. Automatically selects the most restrictive tier for the code being executed.
- **CI rigor:** 11 quality gates including mutation testing (≥85%), 85% coverage enforcement, ASan, fuzzing, and Lean4 proof checking on every PR.
- **cargo-vet audited:** All 8 direct `unsafe` dependencies are audited. 0 CVEs.

## Why not just use Claude Code?

Claude Code and Cursor are great tools. Clawdius is built for a different threat model: environments where you can't afford hallucinated code touching your filesystem, or where latency matters (HFT broker mode with sub-ms wallet guard validation). It's also fully usable with local LLMs via Ollama — your code never leaves your machine.

## What Works Now

CLI chat, agent code generation, VSCode extension, multi-platform messaging gateway (Telegram/Discord/Slack/Matrix/Signal/WhatsApp), HFT broker mode, formal proofs. This is a Technical Preview — the core is solid, but some UX edges are still rough.

## What's Planned

gVisor/Firecracker sandboxes (v1.7.0), plugin marketplace, Neovim/Emacs plugins, MCP server mode (v1.8.0).

GitHub: https://github.com/WyattAu/clawdius
