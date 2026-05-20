# xAI Grok

xAI's Grok models provide fast inference with strong coding capabilities.

## Setup

### Environment Variable

```bash
export XAI_API_KEY="xai-..."
```

### Configuration File

```toml
[llm]
default_provider = "xai"

[llm.xai]
model = "grok-3"
api_key_env = "XAI_API_KEY"
```

## Available Models

| Model | ID | Best For |
|-------|----|----------|
| Grok 3 | `grok-3` | General coding (default) |
| Grok 3 Mini | `grok-3-mini` | Fast responses |

## Usage

```bash
clawdius chat "explain this code" --provider xai
clawdius chat "review" --provider xai --model grok-3-mini
```

## Tool Calling

Grok models support function/tool calling. Enable in config:

```toml
[modes.code]
tools = true
```
