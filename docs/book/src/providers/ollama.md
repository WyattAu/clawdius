# Ollama (Local)

Ollama provides fully local LLM inference with no API key required and no data leaving your machine.

## Prerequisites

Install Ollama:

```bash
# macOS / Linux
curl -fsSL https://ollama.ai/install.sh | sh

# Start the server
ollama serve
```

## Setup

### Default Configuration

No API key is needed. The default Ollama URL is `http://localhost:11434`.

### Configuration File

```toml
[llm]
default_provider = "ollama"

[llm.ollama]
model = "llama3.2"
base_url = "http://localhost:11434"
```

### Environment Variable (Remote Server)

```bash
export OLLAMA_BASE_URL="http://your-server:11434"
```

## Available Models

Pull models from the Ollama registry:

```bash
ollama pull llama3.2
ollama pull codellama
ollama pull mistral
ollama pull deepseek-coder
```

| Model | ID | Size | Best For |
|-------|----|------|----------|
| Llama 3.2 | `llama3.2` | 3.2B | General (default) |
| Code Llama | `codellama` | 13B | Code generation |
| Mistral | `mistral` | 7B | Balanced |
| DeepSeek Coder | `deepseek-coder` | 6.7B | Code |

## Usage

```bash
# Default (llama3.2)
clawdius chat "explain this code" --provider ollama

# Specific model
clawdius chat "write tests" --provider ollama --model codellama

# Manage models via CLI
clawdius models list
clawdius models pull llama3.2
clawdius models health
clawdius models current
```

### Managing Models

```bash
# List available local models
clawdius models list

# Pull a new model
clawdius models pull mistral

# Check server health
clawdius models health

# Custom server
clawdius models list --host 192.168.1.100 --port 11434
```

## Configuration Reference

```toml
[llm.ollama]
model = "llama3.2"                       # Model to use
base_url = "http://localhost:11434"       # Ollama server URL
```

## Connectivity Check

The setup wizard checks Ollama connectivity:

```bash
clawdius setup --provider ollama
```

If the server is unreachable, verify:

```bash
curl http://localhost:11434/api/tags
```

## Performance Tips

- Use models with fewer parameters for faster responses
- Ensure sufficient RAM (at least 8GB for 7B models)
- Use `llama3.2` for the best speed/quality tradeoff on consumer hardware
- Consider GPU acceleration for larger models

## Troubleshooting

**"Connection refused"**

```bash
# Verify Ollama is running
ollama serve
curl http://localhost:11434/api/tags
```

**"Model not found"**

```bash
ollama pull llama3.2
clawdius models list
```
