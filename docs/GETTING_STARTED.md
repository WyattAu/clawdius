# Getting Started with Clawdius

**Time to First Chat: ~10 minutes**  
**Full Setup: ~30 minutes**

---

## Prerequisites

| Requirement | Minimum | Recommended |
|-------------|---------|-------------|
| Rust | 1.75+ | 1.85+ |
| OS | Linux, macOS, Windows | Linux (Ubuntu 22.04+) |
| Memory | 512MB | 2GB+ |
| Disk | 100MB | 500MB+ |

### Optional Dependencies

| Tool | Purpose | Installation |
|------|---------|--------------|
| `bubblewrap` | Linux sandboxing | `sudo apt install bubblewrap` |
| `sandbox-exec` | macOS sandboxing | Included with macOS |
| `podman` | Container sandboxing | `sudo apt install podman` |

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

## Step 1: Configure Your LLM Provider (5 minutes)

Clawdius supports multiple LLM providers. Choose one:

### Anthropic Claude (Recommended)

```bash
# Set your API key securely
clawdius config set provider anthropic
clawdius config set api-key anthropic

# Or use environment variable
export ANTHROPIC_API_KEY="sk-ant-..."
```

### OpenAI

```bash
clawdius config set provider openai
clawdius config set api-key openai
# Or: export OPENAI_API_KEY="sk-..."
```

### Ollama (Local, Free)

```bash
# Install Ollama first
curl -fsSL https://ollama.com/install.sh | sh
ollama pull llama3.2

# Configure Clawdius
clawdius config set provider ollama
clawdius config set ollama-base-url http://localhost:11434
clawdius config set ollama-model llama3.2
```

### Verify Configuration

```bash
clawdius config show
```

---

## Step 2: Start Your First Chat (2 minutes)

### Basic Chat

```bash
clawdius chat
```

```
🤖 Clawdius v1.0.0 | Provider: anthropic | Model: claude-sonnet-4-20250514

You: Hello! Can you help me write a Rust function?

Claude: Of course! I'd be happy to help you write a Rust function. What would 
you like the function to do?

You: /exit
```

### Chat with File Context

```bash
# Start chat with file context
clawdius chat --with src/main.rs

# Or reference files with @mentions
You: @src/main.rs Can you explain what this code does?
```

### Chat with Session Persistence

```bash
# Name your session
clawdius chat --session my-project

# Resume later
clawdius chat --session my-project
```

---

## Step 3: Enable Tools (3 minutes)

Clawdius can interact with your codebase through tools:

### Enable File Operations

```bash
# In your chat session
You: /tools enable file

# Now you can ask for file operations
You: Read the file @src/lib.rs and suggest improvements
```

### Enable Shell Commands

```bash
You: /tools enable shell

# Run commands safely
You: Run `cargo test` and summarize the results
```

### Enable Git Operations

```bash
You: /tools enable git

# Git operations
You: What files changed in the last commit?
```

### Check Tool Status

```bash
You: /tools status

┌─────────────────┬─────────┬─────────────────────┐
│ Tool            │ Status  │ Sandbox             │
├─────────────────┼─────────┼─────────────────────┤
│ file            │ enabled │ filtered            │
│ shell           │ enabled │ bubblewrap          │
│ git             │ enabled │ filtered            │
│ web_search      │ disabled│ -                   │
│ browser         │ disabled│ -                   │
│ keyring         │ enabled │ direct              │
└─────────────────┴─────────┴─────────────────────┘
```

---

## Step 4: Use Sandboxing (5 minutes)

Clawdius protects your system with multi-tier sandboxing:

### Check Available Backends

```bash
clawdius sandbox status
```

```
Sandbox Backends:
  ✅ wasm        - WASM runtime (always available)
  ✅ filtered    - Command filtering (always available)
  ✅ bubblewrap  - Linux namespace sandbox
  ⬜ sandbox-exec - macOS sandbox (macOS only)
  ✅ container   - Docker/Podman containers
  ⬜ gvisor      - gVisor runsc (requires runsc)
  ⬜ firecracker - Firecracker microVM (requires firecracker)

Current Default: bubblewrap
```

### Set Sandbox Tier

```bash
# Maximum security (slower)
clawdius config set sandbox-tier hardened

# Balanced (default)
clawdius config set sandbox-tier standard

# Trusted code only
clawdius config set sandbox-tier trusted
```

### Sandbox Tiers Explained

| Tier | Use Case | Isolation | Performance |
|------|----------|-----------|-------------|
| `hardened` | Untrusted code | Container/gVisor | ~80% |
| `standard` | General use | Bubblewrap/filtered | ~95% |
| `trusted` | Your own code | Filtered commands | ~100% |
| `direct` | audited code only | None | 100% |

---

## Step 5: VSCode Integration (5 minutes)

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

### Start the Language Server

```bash
# Terminal 1: Start Clawdius server
clawdius serve --port 9527

# Terminal 2: Open VSCode
code .
```

### VSCode Features

- **Chat Panel:** `Ctrl+Shift+P` → "Clawdius: Open Chat"
- **Code Actions:** Right-click → "Ask Clawdius"
- **@mentions:** Type `@` in chat to reference files
- **Inline Completions:** Start typing, get AI suggestions

---

## Step 6: Explore Advanced Features (10 minutes)

### Sessions with Checkpoints

```bash
# Create a checkpoint
You: /checkpoint save "before-refactor"

# Make changes with AI...

# Rollback if needed
You: /checkpoint restore "before-refactor"

# View history
You: /checkpoint list
```

### Graph-RAG Context

```bash
# Index your codebase
clawdius index .

# Ask with semantic context
You: Where is the authentication logic in this codebase?

# View indexed symbols
clawdius index show
```

### Custom Modes

```bash
# Use built-in modes
clawdius chat --mode architect
clawdius chat --mode code
clawdius chat --mode ask

# Create custom mode
cat > .clawdius/modes.toml << EOF
[modes.security-review]
description = "Security-focused code review"
system_prompt = "You are a security expert..."
tools = ["file", "git"]
temperature = 0.3
EOF

clawdius chat --mode security-review
```

### Enterprise SSO (if applicable)

```bash
# Configure SSO
clawdius sso configure --provider okta --domain your-company.okta.com

# Login
clawdius sso login

# Verify
clawdius sso status
```

---

## Common Workflows

### Code Review

```bash
clawdius chat --mode architect --with src/

You: Review the code in @src/auth/ for security issues
```

### Refactoring

```bash
clawdius chat --mode code

You: /checkpoint save "before-refactor"
You: Refactor @src/database.rs to use async/await
You: Apply the changes
You: Run tests with `cargo test`
```

### Documentation

```bash
clawdius chat --mode ask

You: Generate documentation for the public API in @src/lib.rs
```

### Debugging

```bash
clawdius chat --tools file,shell,git

You: I'm getting a panic in @src/parser.rs at line 42. Help me debug it.
```

---

## Troubleshooting

### "No provider configured"

```bash
clawdius config set provider anthropic
clawdius config set api-key anthropic
```

### "Sandbox not available"

```bash
# Linux: Install bubblewrap
sudo apt install bubblewrap

# Or use filtered backend
clawdius config set sandbox-backend filtered
```

### "API key not found"

```bash
# Check keyring
clawdius keyring list

# Re-set the key
clawdius config set api-key anthropic
```

### "Session not found"

```bash
# List sessions
clawdius session list

# Or start fresh
clawdius chat
```

---

## Next Steps

1. **Read the User Guide:** [docs/user_guide.md](./user_guide.md)
2. **Explore the API:** [docs/api_reference.md](./api_reference.md)
3. **Join the Community:** [Discord](https://discord.gg/clawdius)
4. **Contribute:** [CONTRIBUTING.md](../CONTRIBUTING.md)

---

## Quick Reference

| Command | Description |
|---------|-------------|
| `clawdius chat` | Start interactive chat |
| `clawdius chat --session NAME` | Resume named session |
| `clawdius chat --mode MODE` | Use specific mode |
| `clawdius config show` | Show configuration |
| `clawdius config set KEY VALUE` | Set configuration |
| `clawdius index PATH` | Index codebase for RAG |
| `clawdius sandbox status` | Check sandbox backends |
| `clawdius session list` | List saved sessions |
| `clawdius --help` | Show all commands |

### Chat Commands

| Command | Description |
|---------|-------------|
| `/help` | Show available commands |
| `/tools enable NAME` | Enable a tool |
| `/tools disable NAME` | Disable a tool |
| `/tools status` | Show tool status |
| `/checkpoint save NAME` | Save checkpoint |
| `/checkpoint restore NAME` | Restore checkpoint |
| `/checkpoint list` | List checkpoints |
| `/exit` | Exit chat |
| `/clear` | Clear conversation |

---

*Need help? Join our [Discord](https://discord.gg/clawdius) or open a [GitHub Discussion](https://github.com/clawdius/clawdius/discussions).*
