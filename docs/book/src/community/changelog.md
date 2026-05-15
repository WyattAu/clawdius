# Changelog

All notable changes to the Clawdius project are documented here.

## [1.0.0-rc.1] - Current

### Added

- Interactive setup wizard (`clawdius setup`)
- Agentic sprint system (`clawdius sprint`)
- Ship command for pre-ship checks and commit messages
- Skill system for markdown-based skills
- HTTP server (`clawdius server`)
- Config management commands (`clawdius config`)
- Project memory system (`clawdius memory`)
- Webhook management (`clawdius webhook`)
- Multi-language output support (en, zh, ja, ko, de, fr, es, it, pt, ru)
- Messaging gateway with multi-platform support (Telegram, Discord, Slack, Matrix, Signal, WhatsApp, Rocket.Chat)
- Multi-tenant configuration
- Audit logging with multiple backends (file, SQLite, Elasticsearch, webhook)
- PII redaction in logs
- State store encryption at rest (AES-256-GCM)
- API key rotation with JWT auth
- IP allowlisting for webhook requests
- Redis-backed orchestrator queue
- PostgreSQL and MariaDB session storage backends
- Stripe billing integration
- Browser automation (chromiumoxide)
- Enhanced code completions with LRU caching
- File watching with auto-analysis
- LSP client integration

### Changed

- Migrated to `deny(unsafe_code)` workspace-wide
- Updated minimum Rust version to 1.88
- Improved retry system with jitter and dead letter queues
- Enhanced error messages with user-friendly suggestions

### Security

- Fixed RUSTSEC-2026-0037 (quinn-proto via reqwest chain)
- Fixed RUSTSEC-2026-0044, RUSTSEC-2026-0048, RUSTSEC-2026-0049 (lancedb)
- Fixed RUSTSEC-2025-0052 (async-std via chromiumoxide)
- Fixed RUSTSEC-2026-0002 (unsound IterMut)

## [0.7.0]

### Added

- Nexus 24-phase FSM engine
- Quality gate system
- Artifact tracking with SQLite
- Graph-RAG with AST index and vector store
- Multi-tier sandboxing (WASM, bubblewrap, containers)
- Timeline and checkpoint system
- Session management with auto-compaction
- Shell tool with command blocking
- File and git tools
- JSON output format for all commands
- Keyring-based API key storage

## [0.6.0]

### Added

- Initial CLI with `chat`, `init`, `status` commands
- Anthropic Claude support
- OpenAI support
- Ollama local model support
- Basic configuration via TOML
- tree-sitter code analysis (Rust, Python, JS, TS, Go)

## Earlier Versions

See git history for detailed changes before version 0.6.0.
