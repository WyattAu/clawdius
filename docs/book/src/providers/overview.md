# LLM Providers Overview

Clawdius supports multiple LLM providers through a unified interface powered by the `genai` crate.

## Supported Providers

| Provider | Identifier | Default Model | API Key Required |
|----------|------------|---------------|------------------|
| Anthropic | `anthropic` | claude-sonnet-4-20250514 | Yes |
| OpenAI | `openai` | gpt-4o | Yes |
| DeepSeek | `deepseek` | deepseek-coder | Yes |
| Ollama | `ollama` | llama3.2 | No (local) |
| ZAI | `zai` | zai-default | Yes |
| Google Gemini | `google` | gemini-pro | Yes |

## Selecting a Provider

### Via CLI Flag

```bash
clawdius chat "Hello" --provider anthropic
clawdius chat "Hello" --provider openai --model gpt-4o
```

### Via Configuration

```toml
[llm]
default_provider = "anthropic"
```

### Per-Provider Settings

```toml
[llm.anthropic]
model = "claude-sonnet-4-20250514"
api_key_env = "ANTHROPIC_API_KEY"

[llm.openai]
model = "gpt-4o"
api_key_env = "OPENAI_API_KEY"
```

## API Key Priority

Keys are resolved in this order:

1. Environment variable (e.g., `ANTHROPIC_API_KEY`)
2. System keyring (via `clawdius auth set`)
3. Config file `api_key` field (not recommended)

## Retry Logic

All providers share a common retry system:

```toml
[llm.retry]
max_retries = 3
initial_delay_ms = 1000
max_delay_ms = 30000
exponential_base = 2.0
retry_on = ["rate_limit", "timeout", "server_error", "network_error"]
```

Retries use exponential backoff with jitter. Rate limit errors (HTTP 429) are automatically retried.

## Provider-Specific Pages

- [Anthropic Claude](./anthropic.md) - Claude setup and configuration
- [OpenAI](./openai.md) - GPT models setup
- [Ollama](./ollama.md) - Local model setup
- [Custom Providers](./custom.md) - Custom endpoints and proxies

## Switching Providers

You can switch providers per-command:

```bash
# Quick question with OpenAI
clawdius chat "explain this regex" --provider openai

# Deep analysis with Claude
clawdius chat "review architecture" --provider anthropic

# Local development with Ollama
clawdius chat "write tests" --provider ollama --model llama3.2
```
