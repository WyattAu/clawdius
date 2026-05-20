# Migrating from Claw Code

Clawdius provides a compatible CLI experience with significantly stronger security, formal verification, and multi-provider support.

## Why Migrate

| Feature | Claw Code | Clawdius |
|---------|-----------|----------|
| LLM Providers | 2 (Anthropic, xAI) | 9 (Anthropic, OpenAI, Gemini, xAI, Mistral, DeepSeek, Ollama, ZAI, OpenRouter) |
| Sandboxing | Container detection | 5 backends (WASM, filtered, bubblewrap, container, sandbox-exec) |
| Formal Verification | None | 209 Lean4 theorems |
| Open Source | MIT | Apache 2.0 |
| Enterprise | None | SSO, audit logging, compliance |
| Messaging | None | 9 platform adapters |
| Tool Calling | Limited | 8 providers with tool support |

## Quick Start

### 1. Install Clawdius

```bash
cargo install --locked clawdius
```

### 2. Copy Your Configuration

Claw Code uses `~/.claude/` for settings. Clawdius uses `~/.config/clawdius/`:

```bash
# Your existing API keys work with Clawdius
export ANTHROPIC_API_KEY="sk-ant-..."   # Claude Code's key works directly
export OPENAI_API_KEY="sk-..."          # If you added OpenAI
export XAI_API_KEY="xai-..."            # Claw Code's key works directly
```

### 3. Configure Clawdius

```bash
# Interactive setup wizard
clawdius setup

# Or create config manually
mkdir -p ~/.config/clawdius
cat > ~/.config/clawdius/config.toml << 'EOF'
[llm]
default_provider = "anthropic"

[llm.anthropic]
model = "claude-sonnet-4-20250514"
api_key_env = "ANTHROPIC_API_KEY"
EOF
```

### 4. Verify

```bash
clawdius chat "Hello from Clawdius" --provider anthropic
```

## Command Mapping

| Claw Code | Clawdius | Notes |
|-----------|----------|-------|
| `claude "prompt"` | `clawdius chat "prompt"` | Different subcommand |
| `claude --model MODEL` | `clawdius chat -m MODEL` | Same model selection |
| `claude --allowedTools` | `clawdius chat --tools` | Tools enabled by default |
| `claude config` | `clawdius config` | Different config format (TOML) |
| `claude mcp` | `clawdius mcp` | Both support MCP |

## Configuration Differences

### Provider Configuration

**Claw Code:**
```json
// ~/.claude/settings.json
{
  "model": "claude-sonnet-4-20250514",
  "apiKeyHelper": "environment"
}
```

**Clawdius:**
```toml
# ~/.config/clawdius/config.toml
[llm]
default_provider = "anthropic"

[llm.anthropic]
model = "claude-sonnet-4-20250514"
api_key_env = "ANTHROPIC_API_KEY"
```

### Multi-Provider (Clawdius Advantage)

```toml
[llm]
default_provider = "anthropic"

[llm.openai]
model = "gpt-4o"
api_key_env = "OPENAI_API_KEY"

[llm.google]
model = "gemini-2.5-flash"
api_key_env = "GOOGLE_API_KEY"

[llm.xai]
model = "grok-3"
api_key_env = "XAI_API_KEY"

[llm.ollama]
model = "llama3.2"
base_url = "http://localhost:11434"
```

## Tool Calling

Clawdius supports tool calling across 8 providers. Enable in your mode config:

```toml
[modes.code]
tools = true
```

Tools work with any provider that supports function calling (Anthropic, OpenAI, Google, xAI, Mistral).

## What You Keep

- All API keys (environment variables work identically)
- Your project files (Clawdius reads from the same working directory)
- Git integration (Clawdius has full git support)
- Markdown formatting (both use markdown for code blocks)

## What Changes

- Config file format: JSON -> TOML
- Config location: `~/.claude/` -> `~/.config/clawdius/`
- CLI entry point: `claude` -> `clawdius chat`
- Additional security features are enabled by default (sandboxing, command filtering)

## Troubleshooting

**"Config not found"**

```bash
# Clawdius uses a different config path
ls ~/.config/clawdius/config.toml
clawdius config --show
```

**"Provider not found"**

Clawdius uses different provider identifiers. Check available providers:

```bash
clawdius config --show
```

**"Tools not working"**

Ensure tools are enabled in your mode:

```bash
clawdius modes
clawdius mode code --tools on
```
