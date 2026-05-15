# Anthropic Claude

Anthropic's Claude models are the default provider in Clawdius.

## Setup

### Environment Variable

```bash
export ANTHROPIC_API_KEY="sk-ant-..."
```

Add to your shell profile for persistence:

```bash
# ~/.bashrc or ~/.zshrc
export ANTHROPIC_API_KEY="sk-ant-..."
```

### System Keyring

```bash
clawdius auth set anthropic
# Prompts for key input securely
```

### Configuration File

```toml
[llm]
default_provider = "anthropic"

[llm.anthropic]
model = "claude-sonnet-4-20250514"
api_key_env = "ANTHROPIC_API_KEY"
```

## Available Models

| Model | ID | Best For |
|-------|----|----------|
| Claude Sonnet 4 | `claude-sonnet-4-20250514` | General coding (default) |
| Claude 3.5 Sonnet | `claude-3-5-sonnet-20241022` | Balanced speed/quality |
| Claude 3 Opus | `claude-3-opus-20240229` | Complex reasoning |

## Usage

```bash
# Default (Claude Sonnet 4)
clawdius chat "explain this code"

# Specific model
clawdius chat "review architecture" --provider anthropic --model claude-3-opus-20240229

# With explicit provider
clawdius chat "write tests" -P anthropic -m claude-sonnet-4-20250514
```

## Configuration Reference

```toml
[llm.anthropic]
model = "claude-sonnet-4-20250514"          # Model to use
api_key_env = "ANTHROPIC_API_KEY"            # Env var for API key
# api_key = "sk-ant-..."                    # Inline key (not recommended)
# base_url = "https://custom-proxy.example" # Custom endpoint
```

## Rate Limiting

Anthropic enforces rate limits per API key. Clawdius handles HTTP 429 responses automatically through the retry system:

```toml
[llm.retry]
max_retries = 3
initial_delay_ms = 1000
max_delay_ms = 30000
retry_on = ["rate_limit", "timeout", "server_error", "network_error"]
```

If you hit rate limits frequently, consider:

- Increasing `max_delay_ms`
- Using `clawdius auth get anthropic` to verify your key
- Switching to a different model tier

## Troubleshooting

**"API key not set"**

```bash
echo $ANTHROPIC_API_KEY    # Check env var
clawdius auth get anthropic  # Check keyring
clawdius setup --provider anthropic  # Re-run setup
```

**"Retry exhausted"**

Increase retry settings in config.toml:

```toml
[llm.retry]
max_retries = 5
initial_delay_ms = 2000
max_delay_ms = 60000
```
