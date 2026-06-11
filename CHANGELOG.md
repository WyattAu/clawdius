# Changelog

## [1.0.0] - 2026-06-11

First general availability release of Clawdius.

### Highlights

- **2,560 tests**, 0 failures across 6 workspace crates
- **284 Lean4 formal verification theorems** (39/39 lake jobs pass)
- **9 LLM providers** (Anthropic, OpenAI, Gemini, xAI, Mistral, DeepSeek, OpenRouter, Ollama, Z.AI)
- **9 messaging platform adapters** (Telegram, Discord, Slack, Matrix, Signal, Teams, WhatsApp, Rocket.Chat, Webhook)
- **5 sandbox backends** (WASM/Wasmtime, Filtered, Bubblewrap, Container, Sandbox-exec)
- **Zero clippy warnings** with `-D warnings` strict
- **Zero hardcoded secrets** in source
- **Apache 2.0** license, fully self-hosted

### Security

- CVE risk acceptance statement added to SECURITY.md for 7 transitive CVEs
- All transitive CVEs are feature-gated (discord, matrix, vector-db); default build is clean
- `[patch.crates-io]` contingency prepared for upstream resolution beyond 90 days
- SECURITY.md supported versions table updated for v1.0.0

### Documentation

- Competitive comparison matrix covering 22 AI coding agents (docs/COMPARISON_MATRIX.md)
- Custom mdBook dark theme matching project design language (Spatial Materialism / Amoebic UI)
- Blog post metrics unified across 8 posts (284 theorems, 2,560 tests, 9 providers)
- Canonical domain unified to clawdius.co.uk (31 references updated across 13 files)

### Infrastructure

- 10 CI/CD workflows with pinned action versions (47 pins across 10 workflows)
- PGO-optimized weekly builds
- CycloneDX SBOM generation in CI
- Performance regression CI gate (10% threshold)
- Nix flake with full devShell (Rust + Lean4 + cargo tools)
- GHCR Docker images (linux/amd64, linux/arm64)

### Workspace Crates

| Crate | Role |
|-------|------|
| clawdius | CLI binary + TUI (25 MB release) |
| clawdius-core | Shared library (LLM, sessions, tools, storage, RPC) |
| clawdius-gateway | Multi-platform messaging gateway (15 MB release) |
| clawdius-code | VSCode extension helper (JSON-RPC) |
| clawdius-mcp | Model Context Protocol server |
| clawdius-plugin-sdk | Plugin development SDK (WASM + native) |

## [1.0.0-rc.2] - 2026-05-20

### Added
- Shell completions for bash, zsh, and fish (via `--generate-completions`)
- xAI (Grok) provider with native genai adapter and tool calling
- Mistral AI provider with native genai adapter and tool calling
- Google Gemini tool calling support (gemini-2.5-flash default)
- LLM response cache with blake3 keying (5min TTL, 1000 entries default)
- Command autocomplete for TUI (37 commands, Tab/Shift+Tab/Enter)
- Production roadmap document (docs/ROADMAP.md)
- Competitor analysis: Claw Code, Roo Code (docs/COMPARISON.md, COMPETITOR_ANALYSIS_CLAW_CODE.md)
- Provider documentation: Google Gemini, xAI Grok, Mistral AI
- Migration guide from Claw Code
- Cloudflare Pages deployment (clawdius-docs, clawdius-landing)
- DNS configuration for clawdius.co.uk, docs.clawdius.co.uk

### Changed
- Provider count: 6 -> 9 (added xAI, Mistral, Gemini tool calling)
- Tool-calling providers: 5 -> 8
- Gateway handler now checks cache before LLM call, inserts after
- `App::draw()` signature changed from `&self` to `&mut self`
- CNAME updated to docs.clawdius.co.uk
- CI release build no longer has `continue-on-error`
- CodeQL SAST timeout increased to 30 minutes
- Fixed duplicate clap short flags (-e for events/enable in WebhookCommands)
- Workspace clap features: added `string` for clap_complete compatibility

### Infrastructure
- Cloudflare Pages projects created (clawdius-docs, clawdius-landing)
- Gateway release build passes locally
- `cargo publish --dry-run` verified for clawdius-core (253 files, 3.5MiB)

## [1.0.0-rc.1] - 2026-05-03

### Added
- 9 messaging platform adapters (Telegram, Discord, Slack, Matrix, Signal, Teams, WhatsApp, Rocket.Chat, Webhook)
- Multi-provider LLM support (Anthropic, OpenAI, OpenRouter, Ollama, Z.AI)
- Streaming TUI with vim keybindings and file browser
- 4 storage backends (SQLite, PostgreSQL, MariaDB, In-Memory)
- Billing and usage metering with SQLite persistence
- Admin REST API for multi-tenant management
- Compliance generator (SOC2, FedRAMP, HIPAA, PCI-DSS)
- Air-gapped deployment mode with encryption at rest
- Lean4 formal verification (16 verified proofs)
- CI/CD pipeline (GitHub Actions)
- Multi-stage Docker image

### Architecture
- 130K+ lines of Rust, 326 files
- 1,237+ tests across all crates
- Zero clippy warnings, zero production unsafe code
- 12 compile-time feature flags
- Modular CLI (30 subcommand modules)
- Modular storage backends (split into 6+ modules each)

### Changed
- cli.rs split from 6,995 lines to 30 focused modules
- sqlite.rs split from 2,614 lines to 6 modules
- sprint.rs split from 2,609 lines to 5 modules
- postgres.rs split from 2,358 lines to 6 modules
- 575 clippy warnings resolved to 0
- 7 broken feature flags fixed
- Gateway test coverage increased from 39 to 146+

### Security
- Zero hardcoded secrets
- Zero production unsafe blocks
- Zero known vulnerabilities in direct dependencies
