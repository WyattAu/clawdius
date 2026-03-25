# Clawdius Quick Reference

## Installation

```bash
cargo install clawdius
```

## First-Time Setup

```bash
clawdius setup
```

## Essential Commands

| Command | Description |
|---------|-------------|
| `clawdius setup` | Interactive setup wizard |
| `clawdius chat` | Start interactive chat |
| `clawdius generate` | Generate code |
| `clawdius analyze` | Analyze codebase |
| `clawdius test` | Generate tests |
| `clawdius doc` | Generate documentation |

## Chat Options

```bash
clawdius chat                              # Interactive chat
clawdius chat --provider anthropic         # Use Anthropic
clawdius chat --provider openai            # Use OpenAI
clawdius chat --provider ollama            # Use local LLM
clawdius chat --message "Hello!"           # Single message
clawdius chat --model claude-sonnet-4      # Specific model
```

## Generate Options

```bash
clawdius generate "Create a function"      # Basic generation
clawdius generate --mode single-pass       # Single LLM call
clawdius generate --mode iterative         # Multi-iteration refinement
clawdius generate --mode agent             # Autonomous agent
clawdius generate --stream                 # Stream output
clawdius generate --files src/main.rs      # Target files
clawdius generate --trust high             # Trust level
clawdius generate --dry-run                # Preview without execution
```

## Providers

| Provider | Flag | Models |
|----------|------|--------|
| Anthropic | `--provider anthropic` | claude-sonnet-4, claude-opus-4 |
| OpenAI | `--provider openai` | gpt-4o, gpt-4-turbo |
| Ollama | `--provider ollama` | llama3, codellama, mistral |
| Zhipu AI | `--provider zhipu` | glm-4 |

## Configuration

```bash
clawdius config list                       # Show all settings
clawdius config set api_key YOUR_KEY       # Set API key
clawdius config set provider anthropic     # Set provider
clawdius config get model                  # Get current model
clawdius config edit                       # Open config file
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `ANTHROPIC_API_KEY` | Anthropic API key |
| `OPENAI_API_KEY` | OpenAI API key |
| `OLLAMA_HOST` | Ollama server URL |
| `CLAWDIUS_CONFIG` | Custom config path |
| `CLAWDIUS_LOG` | Log level (debug/info/warn) |

## Sandbox Tiers

| Tier | Isolation | Use Case |
|------|-----------|----------|
| minimal | None | Trusted code |
| standard | WASM + seccomp | Development |
| hardened | Bubblewrap | Untrusted code |
| container | Docker/Podman | Strong isolation |
| gvisor | gVisor runsc | Strong isolation |
| firecracker | MicroVM | Maximum isolation |

## Output Formats

```bash
clawdius chat --format text     # Human-readable (default)
clawdius chat --format json     # JSON output
clawdius chat --format stream   # Streaming JSON
```

## Trust Levels

| Level | Description |
|-------|-------------|
| `low` | Maximum sandboxing, confirm all changes |
| `medium` | Standard sandboxing, confirm risky changes |
| `high` | Minimal sandboxing, auto-apply changes |

## Test Strategies

```bash
clawdius generate --test sandboxed   # Run tests in sandbox
clawdius generate --test direct      # Run tests with rollback
clawdius generate --test skip        # Skip tests
```

## Keyboard Shortcuts (Chat)

| Key | Action |
|-----|--------|
| `Ctrl+C` | Cancel current input |
| `Ctrl+D` | Exit chat |
| `Ctrl+L` | Clear screen |
| `↑/↓` | Command history |
| `Tab` | Autocomplete |

## Special Commands (Chat)

| Command | Description |
|---------|-------------|
| `/help` | Show help |
| `/clear` | Clear conversation |
| `/save` | Save session |
| `/load` | Load session |
| `/export` | Export to file |
| `/model` | Change model |
| `/provider` | Change provider |
| `/compact` | Compact context |
| `/tokens` | Show token count |
| `/exit` | Exit chat |

## Files & Directories

| Path | Description |
|------|-------------|
| `~/.config/clawdius/` | Configuration |
| `~/.local/share/clawdius/` | Data (sessions, cache) |
| `~/.cache/clawdius/` | Temporary cache |
| `.clawdius/config.toml` | Project config |
| `.clawdius/session.json` | Session data |

## Shell Completion

```bash
# Bash
clawdius completion bash > /etc/bash_completion.d/clawdius

# Zsh
clawdius completion zsh > "${fpath[1]}/_clawdius"

# Fish
clawdius completion fish > ~/.config/fish/completions/clawdius.fish

# PowerShell
clawdius completion powershell | Out-String | Invoke-Expression
```

## Diagnostics

```bash
clawdius doctor                    # Run diagnostics
clawdius --version                 # Show version
clawdius config list               # Show configuration
cargo audit                        # Security audit
```

## Getting Help

```bash
clawdius --help                    # General help
clawdius chat --help               # Chat help
clawdius generate --help           # Generate help
clawdius <command> --help          # Command-specific help
```

## Resources

- 📖 Documentation: https://docs.clawdius.dev
- 🐙 GitHub: https://github.com/WyattAu/clawdius
- 💬 Discord: https://discord.gg/clawdius
- 🐛 Issues: https://github.com/WyattAu/clawdius/issues
- 💡 Discussions: https://github.com/WyattAu/clawdius/discussions

---

**Version:** 1.2.0 | **Updated:** 2026-03-25
