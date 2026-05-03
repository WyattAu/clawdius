# Changelog

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
