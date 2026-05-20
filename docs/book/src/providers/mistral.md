# Mistral AI

Mistral provides high-quality open-weight models with competitive performance.

## Setup

### Environment Variable

```bash
export MISTRAL_API_KEY="..."
```

### Configuration File

```toml
[llm]
default_provider = "mistral"

[llm.mistral]
model = "mistral-large-latest"
api_key_env = "MISTRAL_API_KEY"
```

## Available Models

| Model | ID | Best For |
|-------|----|----------|
| Mistral Large | `mistral-large-latest` | General coding (default) |
| Codestral | `codestral-latest` | Code generation |
| Mistral Small | `mistral-small-latest` | Fast responses |

## Usage

```bash
clawdius chat "explain this code" --provider mistral
clawdius chat "write a parser" --provider mistral --model codestral-latest
```

## Tool Calling

Mistral Large supports function/tool calling. Enable in config:

```toml
[modes.code]
tools = true
```
