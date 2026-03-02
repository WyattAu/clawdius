# Clawdius Getting Started Guide

**Version:** 0.6.0  
**Last Updated:** 2026-03-01

---

## Quick Start

```bash
# Install
cargo install clawdius

# Initialize
cd your-project
clawd init

# Start chatting
clawd chat
```

---

## Prerequisites

### Required

| Requirement | Version | How to Install |
|-------------|---------|----------------|
| Rust | 1.85+ | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| Cargo | Included with Rust | - |

### Optional (by platform)

| Platform | Dependency | Purpose |
|----------|------------|---------|
| Linux | bubblewrap | Sandboxing |
| Linux | podman | Container execution |
| macOS | - | sandbox-exec is built-in |

### API Keys

Set at least one LLM provider API key:

```bash
# OpenAI
export OPENAI_API_KEY="sk-..."

# Anthropic
export ANTHROPIC_API_KEY="sk-ant-..."
```

---

## Installation Methods

### Method 1: Cargo (Recommended)

```bash
cargo install clawdius
```

### Method 2: Nix

```bash
nix shell github:clawdius/clawdius
```

### Method 3: From Source

```bash
git clone https://github.com/clawdius/clawdius
cd clawdius
cargo build --release
sudo cp target/release/clawdius /usr/local/bin/
```

---

## First Commands

### 1. Initialize a Project

```bash
mkdir my-project && cd my-project
clawd init
```

Output:
```
Creating .clawdius/ directory structure...
✓ .clawdius/sops/common.sop.md
✓ .clawdius/sops/rust.sop.md
✓ .clawdius/specs/
✓ .clawdius/graph/
✓ .clawdius/settings.toml

Clawdius initialized successfully.
Current phase: Context Discovery (-1)
```

### 2. Check Status

```bash
clawd status
```

Output:
```
Project: my-project
Phase: Context Discovery (-1)
Status: IN PROGRESS
Rigor Score: 0.00
Artifacts: 0

Next steps:
1. Define project domain and stakeholders
2. Identify applicable standards
3. Run 'clawd chat' to begin
```

### 3. Start Chatting

```bash
clawd chat
```

```
🦀 Clawdius v0.6.0
Phase: Context Discovery
Provider: OpenAI (gpt-4)

> What is this project?
This appears to be a new project in the Context Discovery phase...
```

---

## Configuration

### Basic Settings

Edit `.clawdius/settings.toml`:

```toml
[project]
name = "my-project"
version = "0.1.0"

[llm]
provider = "openai"
model = "gpt-4"

[sandbox]
default_tier = 2
```

### LLM Providers

| Provider | Value | API Key |
|----------|-------|---------|
| OpenAI | `openai` | `OPENAI_API_KEY` |
| Anthropic | `anthropic` | `ANTHROPIC_API_KEY` |
| DeepSeek | `deepseek` | `DEEPSEEK_API_KEY` |
| Ollama | `ollama` | None (local) |

---

## Common Workflows

### Code Analysis

```bash
# Analyze codebase structure
clawd chat

> Analyze the architecture of src/
```

### Refactoring

```bash
# Plan TypeScript to Rust migration
clawd refactor --from typescript --to rust --analyze src/

# Execute migration
clawd refactor --from typescript --to rust src/
```

### Verification

```bash
# Run all checks
clawd verify --all
```

---

## Directory Structure

```
your-project/
├── .clawdius/
│   ├── sops/               # Standard Operating Procedures
│   │   ├── common.sop.md   # General rules
│   │   └── rust.sop.md     # Rust-specific rules
│   ├── specs/              # Specifications
│   │   ├── 00_requirements/
│   │   ├── 01_research/
│   │   ├── 02_architecture/
│   │   └── ...
│   ├── graph/              # Knowledge graph
│   │   ├── ast.db          # SQLite AST index
│   │   └── vectors/        # LanceDB embeddings
│   └── settings.toml       # Configuration
├── src/                    # Your source code
└── Cargo.toml              # Your project manifest
```

---

## Troubleshooting

### "command not found: clawd"

Ensure `~/.cargo/bin` is in your PATH:
```bash
export PATH="$HOME/.cargo/bin:$PATH"
```

### "Failed to initialize"

Remove existing configuration:
```bash
rm -rf .clawdius/
clawd init
```

### "API key not found"

Set your API key:
```bash
export OPENAI_API_KEY="sk-..."
```

### Enable Debug Logging

```bash
RUST_LOG=clawdius=debug clawd chat
```

---

## Next Steps

1. Read the [User Guide](user_guide.md) for detailed commands
2. Review [Architecture Overview](architecture_overview.md) to understand the system
3. Check [API Reference](api_reference.md) for programmatic usage

---

## Getting Help

- **Issues:** https://github.com/clawdius/clawdius/issues
- **Discord:** https://discord.gg/clawdius
- **Docs:** `.docs/` directory
