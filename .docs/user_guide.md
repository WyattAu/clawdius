# Clawdius User Guide

**Version:** 0.6.0  
**Last Updated:** 2026-03-01

---

## Table of Contents

1. [Introduction](#1-introduction)
2. [Installation](#2-installation)
3. [Configuration](#3-configuration)
4. [Commands](#4-commands)
5. [Workflows](#5-workflows)
6. [Troubleshooting](#6-troubleshooting)

---

## 1. Introduction

Clawdius is a high-assurance AI engineering engine that enforces rigorous development practices through the 24-phase Nexus R&D Lifecycle. It provides deterministic behavior, secure sandboxing, and comprehensive knowledge management.

### 1.1 Key Features

- **24-Phase Nexus Lifecycle:** Structured development process with quality gates
- **Graph-RAG Intelligence:** AST-based code understanding with vector search
- **Sentinel Sandboxing:** Multi-tier execution isolation
- **SOP Enforcement:** Automated standard operating procedure compliance
- **HFT-Ready:** Sub-millisecond latency for financial applications

### 1.2 System Requirements

| Requirement | Minimum | Recommended |
|-------------|---------|-------------|
| OS | Linux, macOS, WSL2 | Linux (Ubuntu 24.04) |
| Rust | 1.85+ | 1.85+ |
| RAM | 64MB | 256MB |
| Disk | 50MB | 500MB |

---

## 2. Installation

### 2.1 Via Cargo

```bash
cargo install clawdius
```

### 2.2 Via Nix

```bash
nix shell github:clawdius/clawdius
```

### 2.3 From Source

```bash
git clone https://github.com/clawdius/clawdius
cd clawdius
cargo build --release
```

The binary will be at `target/release/clawdius`.

### 2.4 Verification

```bash
clawdius --version
# Output: clawdius 0.6.0
```

---

## 3. Configuration

### 3.1 Initialization

Initialize Clawdius in your project:

```bash
clawdius init
```

This creates the `.clawdius/` directory structure:

```
.clawdius/
├── sops/           # Standard Operating Procedures
│   ├── common.sop.md
│   └── rust.sop.md
├── specs/          # Specifications and papers
│   ├── 00_requirements/
│   ├── 01_research/
│   ├── 02_architecture/
│   └── ...
├── graph/          # Knowledge graph storage
│   ├── ast.db      # SQLite AST database
│   └── vectors/    # LanceDB vector store
└── settings.toml   # Project configuration
```

### 3.2 Settings File

The `.clawdius/settings.toml` file configures project behavior:

```toml
[project]
name = "my-project"
version = "0.1.0"

[llm]
provider = "openai"  # openai, anthropic, deepseek, ollama
model = "gpt-4"

[sandbox]
default_tier = 2  # 1=Native, 2=Container, 3=WASM, 4=Hardened

[hft]
enabled = false
position_limit = 1000000
daily_drawdown_limit = 50000
```

### 3.3 SOP Configuration

SOPs define coding standards and are enforced during code generation:

```markdown
<!-- .clawdius/sops/rust.sop.md -->
# Rust SOP

## Rules
- Use `thiserror` for error types
- Deny `unwrap()` and `expect()`
- Require documentation on public APIs
```

---

## 4. Commands

### 4.1 `clawd init`

Initialize Clawdius in the current directory.

```bash
clawd init [--force]
```

| Flag | Description |
|------|-------------|
| `--force` | Overwrite existing `.clawdius/` directory |

### 4.2 `clawd chat`

Start an interactive high-assurance session.

```bash
clawd chat [--provider PROVIDER] [--model MODEL]
```

| Flag | Description | Default |
|------|-------------|---------|
| `--provider` | LLM provider | `openai` |
| `--model` | Model to use | `gpt-4` |

**Example:**
```bash
clawd chat --provider anthropic --model claude-3-opus
```

### 4.3 `clawd refactor`

Plan and execute cross-language refactoring.

```bash
clawd refactor --from LANG --to LANG [PATH]
```

| Flag | Description |
|------|-------------|
| `--from` | Source language |
| `--to` | Target language |
| `PATH` | Files to refactor |

**Example:**
```bash
clawd refactor --from typescript --to rust src/
```

### 4.4 `clawd broker`

Activate financial monitoring and trading signals.

```bash
clawd broker [--config CONFIG] [--dry-run]
```

| Flag | Description |
|------|-------------|
| `--config` | Broker configuration file |
| `--dry-run` | Simulate without executing trades |

**Example:**
```bash
clawd broker --config broker.toml --dry-run
```

### 4.5 `clawd verify`

Run Lean 4 proofs and SOP compliance checks.

```bash
clawd verify [--lean4] [--sop] [--all]
```

| Flag | Description |
|------|-------------|
| `--lean4` | Run formal verification proofs |
| `--sop` | Check SOP compliance |
| `--all` | Run all verification |

**Example:**
```bash
clawd verify --all
```

### 4.6 `clawd status`

Show current project status and phase.

```bash
clawd status
```

Output:
```
Project: my-project
Version: 0.1.0
Phase: 6.5 (Documentation Verification)
Status: IN PROGRESS
Rigor Score: 0.92
```

---

## 5. Workflows

### 5.1 Starting a New Project

```bash
# 1. Initialize
clawd init

# 2. Edit settings
vim .clawdius/settings.toml

# 3. Start interactive session
clawd chat

# 4. Check status
clawd status
```

### 5.2 Code Refactoring

```bash
# 1. Analyze current codebase
clawd refactor --from typescript --to rust --analyze src/

# 2. Review migration plan
cat .clawdius/specs/migration_plan.md

# 3. Execute migration
clawd refactor --from typescript --to rust src/

# 4. Verify results
clawd verify --all
```

### 5.3 HFT Mode Setup

```bash
# 1. Enable HFT in settings
vim .clawdius/settings.toml
# Set [hft].enabled = true

# 2. Configure risk parameters
clawd broker --config hft_config.toml --dry-run

# 3. Start broker mode
clawd broker --config hft_config.toml
```

---

## 6. Troubleshooting

### 6.1 Common Issues

#### "Failed to initialize state machine"

**Cause:** Corrupted `.clawdius/` directory

**Solution:**
```bash
rm -rf .clawdius/
clawd init
```

#### "Sandbox creation failed"

**Cause:** Missing sandboxing tools

**Solution (Linux):**
```bash
sudo apt install bubblewrap
```

**Solution (macOS):**
```bash
# sandbox-exec is built-in
# Ensure SIP is enabled
csrutil status
```

#### "LLM provider error"

**Cause:** Invalid API key or network issue

**Solution:**
```bash
# Check API key
echo $OPENAI_API_KEY

# Test connection
curl https://api.openai.com/v1/models \
  -H "Authorization: Bearer $OPENAI_API_KEY"
```

### 6.2 Debug Mode

Enable verbose logging:

```bash
RUST_LOG=clawdius=debug clawd chat
```

### 6.3 Getting Help

- **Documentation:** `.docs/` directory
- **Issues:** https://github.com/clawdius/clawdius/issues
- **Discord:** https://discord.gg/clawdius

---

## Appendix A: Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `RUST_LOG` | Log level | `info` |
| `CLAWDIUS_CONFIG` | Config path | `.clawdius/` |
| `OPENAI_API_KEY` | OpenAI API key | - |
| `ANTHROPIC_API_KEY` | Anthropic API key | - |

## Appendix B: Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error |
| 2 | Configuration error |
| 3 | Sandbox error |
| 4 | LLM error |
| 5 | Verification failed |
