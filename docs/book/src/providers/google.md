# Google Gemini

Google's Gemini models offer large context windows and strong multimodal capabilities.

## Setup

### Environment Variable

```bash
export GOOGLE_API_KEY="..."
```

Add to your shell profile for persistence:

```bash
# ~/.bashrc or ~/.zshrc
export GOOGLE_API_KEY="..."
```

### Configuration File

```toml
[llm]
default_provider = "google"

[llm.google]
model = "gemini-2.5-flash"
api_key_env = "GOOGLE_API_KEY"
```

## Available Models

| Model | ID | Best For |
|-------|----|----------|
| Gemini 2.5 Flash | `gemini-2.5-flash` | Fast coding (default) |
| Gemini 2.5 Pro | `gemini-2.5-pro` | Complex reasoning |
| Gemini 2.0 Flash | `gemini-2.0-flash` | Balanced speed/quality |

## Usage

```bash
# Default (Gemini 2.5 Flash)
clawdius chat "explain this code"

# Specific model
clawdius chat "review architecture" --provider google --model gemini-2.5-pro
```

## Configuration Reference

```toml
[llm.google]
model = "gemini-2.5-flash"                   # Model to use
api_key_env = "GOOGLE_API_KEY"                # Env var for API key
# base_url = "https://generativelanguage.googleapis.com" # Custom endpoint
```

## Tool Calling

Gemini 2.5 Flash and Pro support function/tool calling. Enable tools in your config:

```toml
[modes.code]
tools = true
```

## Troubleshooting

**"API key not set"**

```bash
echo $GOOGLE_API_KEY       # Check env var
clawdius setup --provider google  # Re-run setup
```
