# Getting Started with Clawdius

**Time to First Chat: ~5 minutes**  
**Full Setup: ~30 minutes**

---

## Prerequisites

| Requirement | Minimum | Recommended |
|-------------|---------|-------------|
| Rust | 1.75+ | 1.85+ |
| OS | Linux, macOS, Windows | Linux (Ubuntu 22.04+) |
| Memory | 512MB | 2GB+ |
| Disk | 100MB | 500MB+ |

---

## Quick Start (5 minutes)

### Option 1: Install from Source

```bash
# Clone and build
git clone https://github.com/clawdius/clawdius
cd clawdius
cargo build --release

# Run
./target/release/clawdius chat
```

### Option 2: Install with Cargo

```bash
cargo install clawdius
clawdius chat
```

### Option 3: Install with Nix

```bash
nix shell github:clawdius/clawdius
clawdius chat
```

---

## Step 1: Configure Your LLM Provider (2 minutes)

Clawdius supports multiple LLM providers. Choose one:

### Anthropic Claude (Recommended)

```bash
# Set your API key via environment variable
export ANTHROPIC_API_KEY="sk-ant-..."
```

### OpenAI

```bash
export OPENAI_API_KEY="sk-..."
```

### Ollama (Local, Free)

```bash
# Install Ollama first
curl -fsSL https://ollama.com/install.sh | sh
ollama pull llama3.2

# No API key needed - configure in .clawdius/config.toml:
# [llm]
# default_provider = "ollama"
# [llm.ollama]
# model = "llama3.2"
# base_url = "http://localhost:11434"
```

### Using the Setup Wizard

```bash
# Interactive first-time setup
clawdius setup

# Quick setup with pre-selected provider
clawdius setup --provider anthropic
```

### Storing Keys in System Keyring

Requires the `keyring` feature. Store keys securely without env vars:

```bash
clawdius auth set anthropic    # Prompts for key input
clawdius auth get anthropic    # Verify key is stored
clawdius auth delete anthropic # Remove stored key
```

### Verify Configuration

```bash
# View your config file directly
cat .clawdius/config.toml

# Or test with a simple chat
clawdius chat "Hello"
```

---

## Step 2: Start Your First Chat (1 minute)

### Basic Chat

```bash
clawdius chat
```

### Chat with Provider/Model Selection

```bash
# Use specific provider
clawdius chat --provider openai --model gpt-4o

# Use local Ollama
clawdius chat --provider ollama

# Single-shot (non-interactive)
clawdius chat "Explain this function"
```

### Chat with Session Persistence

```bash
# Start a session (clawdius auto-assigns an ID)
clawdius chat

# Resume a session by ID
clawdius chat --session <session-id>

# List and search sessions
clawdius sessions
clawdius sessions --search "error handling"
```

### Chat with Agent Modes

```bash
clawdius chat --mode architect
clawdius chat --mode code
clawdius chat --mode debug
clawdius chat --mode review
clawdius chat --mode test
clawdius chat --mode refactor
```

---

## Step 3: Autonomous CI/CD Mode

Run tasks without interaction, with optional test execution and auto-commit:

```bash
# Fix failing tests autonomously
clawdius auto "fix failing tests" --run-tests --fail-on-test-failure

# Implement a feature
clawdius auto "implement user authentication" --auto-commit

# CI mode with JSON output
clawdius auto "refactor error handling" --output-format json
```

---

## Step 4: Code Generation and Analysis

### Generate Code

```bash
# Generate code with agentic AI
clawdius generate "create a REST API handler for user management"

# Preview changes without applying
clawdius generate "add input validation" --dry-run

# Specify target files
clawdius generate "add unit tests" --files src/lib.rs,src/models.rs

# Iterative generation with more passes
clawdius generate "implement feature X" --mode iterative --max-iterations 10
```

### Generate Tests

```bash
clawdius test src/lib.rs
clawdius test src/lib.rs --function my_function
```

### Generate Documentation

```bash
clawdius doc src/lib.rs
clawdius doc src/lib.rs --element MyStruct --format rustdoc
```

### Analyze Codebase

```bash
# General analysis
clawdius analyze .

# Architecture drift only
clawdius analyze . --drift

# Technical debt only
clawdius analyze . --debt --severity high

# JSON output
clawdius analyze . -f json -o report.json
```

### Watch for Changes

```bash
# Watch files and auto-analyze on changes
clawdius watch . --auto-analyze
```

---

## Step 5: Checkpoint and Timeline System

### Session Checkpoints

```bash
# Create a checkpoint
clawdius checkpoint create "before-refactor"

# List checkpoints
clawdius checkpoint list

# Restore a checkpoint
clawdius checkpoint restore <checkpoint-id>

# Compare checkpoints
clawdius checkpoint compare <id1> <id2>
```

### File Timeline

```bash
# Create a timeline checkpoint
clawdius timeline create "v1.0-rc1" --description "Release candidate 1"

# List timeline checkpoints
clawdius timeline list

# Rollback to a checkpoint
clawdius timeline rollback <checkpoint-id>

# View diff between checkpoints
clawdius timeline diff <from-id> <to-id>

# View file history
clawdius timeline history src/main.rs
```

---

## Step 6: VSCode Integration (5 minutes)

### Install the Extension

1. Open VSCode
2. Go to Extensions (Ctrl+Shift+X)
3. Search for "Clawdius"
4. Click Install

### Or Install from Source

```bash
cd editors/vscode
pnpm install
pnpm run compile
# Then: Extensions → ... → Install from VSIX
```

### VSCode Features

- **Chat Panel:** `Ctrl+Shift+P` → "Clawdius: Open Chat"
- **Code Actions:** Right-click → "Ask Clawdius"
- **@mentions:** Type `@` in chat to reference files
- **Inline Completions:** Start typing, get AI suggestions

---

## Step 7: Advanced Features

### Nexus FSM Engine

Run the 24-phase Nexus R&D lifecycle:

```bash
clawdius nexus start .
```

### Git Workflow

```bash
# AI-generated commit messages
clawdius git commit

# View diffs
clawdius git diff
clawdius git diff --staged

# Status summary
clawdius git status
```

### Project Memory (CLAUDE.md)

```bash
# Show project memory
clawdius memory show

# Learn a new pattern
clawdius memory learn pattern "always use Result<T, E> for fallible operations"

# Set project instructions
clawdius memory instructions "This project uses async Rust throughout"

# Initialize CLAUDE.md
clawdius memory init --name my-project --language rust --framework actix-web
```

### Workflows

```bash
# List workflows
clawdius workflow list

# Create a workflow
clawdius workflow create "code-review" --description "Automated code review pipeline"

# Execute a workflow
clawdius workflow run <workflow-id>

# Check status
clawdius workflow status <execution-id>
```

### Custom Agent Modes

```bash
# List available modes
clawdius modes list

# Show mode details
clawdius modes show architect

# Create a custom mode
clawdius modes create security-review
```

### Local LLM Management

```bash
# List available Ollama models
clawdius models list

# Pull a new model
clawdius models pull llama3.2

# Check Ollama health
clawdius models health
```

### Metrics and Telemetry

```bash
# Show performance metrics
clawdius metrics

# JSON metrics output
clawdius metrics -f json -o metrics.json

# Configure telemetry
clawdius telemetry --enable --enable-metrics
```

---

## Common Workflows

### Code Review

```bash
clawdius chat --mode review
```

### Refactoring

```bash
# Cross-language refactor
clawdius refactor --from typescript --to rust src/ --dry-run

# Or use chat with refactor mode
clawdius chat --mode refactor
```

### Debugging

```bash
clawdius chat --mode debug
```

### Autonomous Development

```bash
clawdius auto "implement feature X" --run-tests --auto-commit
```

---

## Troubleshooting

### "No provider configured"

```bash
# Set environment variable
export ANTHROPIC_API_KEY="sk-ant-..."

# Or run setup wizard
clawdius setup

# Or store in keyring
clawdius auth set anthropic
```

### "API key not found"

```bash
# Check environment variable
echo $ANTHROPIC_API_KEY

# Check keyring
clawdius auth get anthropic

# Re-set the key
clawdius auth set anthropic
```

### "Session not found"

```bash
# List sessions
clawdius sessions

# Search sessions
clawdius sessions --search "keyword"

# Start fresh
clawdius chat
```

---

## Next Steps

1. **Read the User Guide:** [.docs/user_guide.md](../.docs/user_guide.md)
2. **Explore the API:** [docs/api_reference.md](./api_reference.md)
3. **Join the Community:** [Discord](https://discord.gg/clawdius)
4. **Contribute:** [CONTRIBUTING.md](../CONTRIBUTING.md)

---

## Quick Reference

| Command | Description |
|---------|-------------|
| `clawdius chat` | Start interactive chat |
| `clawdius chat -M MODE` | Use specific agent mode |
| `clawdius chat -P PROVIDER` | Use specific LLM provider |
| `clawdius auto TASK` | Run task autonomously |
| `clawdius generate PROMPT` | Generate code with AI |
| `clawdius init` | Initialize project |
| `clawdius setup` | Interactive setup wizard |
| `clawdius sessions` | List/manage sessions |
| `clawdius auth set PROVIDER` | Store API key in keyring |
| `clawdius analyze PATH` | Analyze codebase for drift/debt |
| `clawdius test FILE` | Generate tests for code |
| `clawdius doc FILE` | Generate documentation |
| `clawdius refactor --from LANG --to LANG` | Cross-language refactor |
| `clawdius checkpoint create DESC` | Create a checkpoint |
| `clawdius timeline create NAME` | Create timeline checkpoint |
| `clawdius git commit` | AI-generated commit messages |
| `clawdius memory show` | Show project memory |
| `clawdius models list` | List local Ollama models |
| `clawdius workflow list` | List workflows |
| `clawdius nexus start .` | Run Nexus 24-phase engine |
| `clawdius --help` | Show all commands |

### Global Flags

| Flag | Description |
|------|-------------|
| `-f`, `--output-format` | Output format: `text`, `json`, `stream-json` |
| `-C`, `--config` | Path to config file |
| `-L`, `--lang` | Output language |
| `-n`, `--no-tui` | Run without TUI (headless mode) |
| `-q`, `--quiet` | Quiet mode |

---

*Need help? Join our [Discord](https://discord.gg/clawdius) or open a [GitHub Discussion](https://github.com/clawdius/clawdius/discussions).*
