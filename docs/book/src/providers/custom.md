# Custom Providers

Clawdius supports custom LLM endpoints through the `base_url` configuration.

## OpenAI-Compatible Endpoints

Any server implementing the OpenAI chat completions API can be used by setting `base_url`:

```toml
[llm.openai]
model = "your-model-name"
base_url = "https://your-endpoint.example.com/v1"
api_key_env = "YOUR_API_KEY"
```

This works with:

- Local inference servers (vLLM, llama.cpp, text-generation-webui)
- Cloud proxies (LiteLLM, Helicone)
- Enterprise gateways

## DeepSeek

```toml
[llm]
default_provider = "deepseek"

[llm.deepseek]
model = "deepseek-coder"
api_key_env = "DEEPSEEK_API_KEY"
```

```bash
export DEEPSEEK_API_KEY="your-key"
clawdius chat "write code" --provider deepseek
```

## ZAI (Zhipu AI)

```toml
[llm]
default_provider = "zai"

[llm.zai]
model = "zai-default"
api_key_env = "ZAI_API_KEY"
```

```bash
export ZAI_API_KEY="your-key"
clawdius chat "write code" --provider zai
```

## Google Gemini

```toml
[llm]
default_provider = "google"

[llm.google]
model = "gemini-pro"
api_key_env = "GOOGLE_API_KEY"
```

## OpenRouter

OpenRouter provides multi-provider routing. Configure it as an OpenAI-compatible endpoint:

```toml
[llm.openai]
model = "anthropic/claude-sonnet-4-20250514"
base_url = "https://openrouter.ai/api/v1"
api_key_env = "OPENROUTER_API_KEY"
```

```bash
export OPENROUTER_API_KEY="your-key"
clawdius chat "Hello" --provider openai --model anthropic/claude-sonnet-4-20250514
```

## Local Inference Servers

### vLLM

```toml
[llm.openai]
model = "your-model"
base_url = "http://localhost:8000/v1"
```

### llama.cpp Server

```toml
[llm.openai]
model = "your-model"
base_url = "http://localhost:8080/v1"
```

## Custom Provider via Environment

You can override any provider's base URL via environment variables. The provider identifier must match a known provider name in the CLI:

```bash
clawdius chat "Hello" --provider openai --model custom-model
```

Combined with `base_url` in the config, this routes requests to your custom endpoint while using OpenAI's message format.

## Retry Configuration

Custom endpoints benefit from the same retry system:

```toml
[llm.retry]
max_retries = 5
initial_delay_ms = 2000
max_delay_ms = 60000
exponential_base = 2.0
retry_on = ["rate_limit", "timeout", "server_error", "network_error"]
```
