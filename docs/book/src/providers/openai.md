# OpenAI

Clawdius supports OpenAI's GPT models as an alternative LLM provider.

## Setup

### Environment Variable

```bash
export OPENAI_API_KEY="sk-..."
```

### System Keyring

```bash
clawdius auth set openai
```

### Configuration File

```toml
[llm]
default_provider = "openai"

[llm.openai]
model = "gpt-4o"
api_key_env = "OPENAI_API_KEY"
```

## Available Models

| Model | ID | Best For |
|-------|----|----------|
| GPT-4o | `gpt-4o` | General coding (default) |
| GPT-4 Turbo | `gpt-4-turbo` | Long context |
| GPT-3.5 Turbo | `gpt-3.5-turbo` | Fast, cost-effective |
| o1 | `o1` | Complex reasoning |
| o3-mini | `o3-mini` | Reasoning, cost-effective |

## Usage

```bash
# Default (GPT-4o)
clawdius chat "explain this code" --provider openai

# Specific model
clawdius chat "review architecture" --provider openai --model o1

# With explicit flags
clawdius chat "write tests" -P openai -m gpt-4o
```

## Configuration Reference

```toml
[llm.openai]
model = "gpt-4o"                            # Model to use
api_key_env = "OPENAI_API_KEY"               # Env var for API key
# api_key = "sk-..."                       # Inline key (not recommended)
# base_url = "https://custom-proxy.example" # Custom endpoint or proxy
```

## Using a Proxy

If you need to route requests through a proxy or use a compatible API:

```toml
[llm.openai]
model = "gpt-4o"
base_url = "https://your-proxy.example.com/v1"
api_key_env = "PROXY_API_KEY"
```

## Rate Limiting

OpenAI rate limits vary by model and tier. The retry system handles HTTP 429 automatically:

```toml
[llm.retry]
max_retries = 3
initial_delay_ms = 1000
max_delay_ms = 30000
retry_on = ["rate_limit", "timeout", "server_error", "network_error"]
```

## Troubleshooting

**"API key not set"**

```bash
echo $OPENAI_API_KEY
clawdius auth get openai
```

**"Model not available"**

Verify the model name matches OpenAI's current API. Check available models at OpenAI's documentation.
