# Clawdius Examples

This directory contains example projects demonstrating Clawdius features.

## Examples

### 1. Hello World (`hello-world/`)

Basic example showing:
- Interactive chat
- Code generation
- Session management

```bash
cd hello-world
clawdius chat
```

### 2. REST API (`rest-api/`)

Demonstrates:
- Generate REST API endpoints
- Test generation
- Documentation generation

```bash
cd rest-api
clawdius generate --mode agent "Create a REST API for user management"
```

### 3. Local LLM (`local-llm/`)

Shows how to use Clawdius with local LLMs (Ollama):
- 100% private operation
- No API keys required
- Multiple model support

```bash
# Start Ollama
ollama serve

# Use with Clawdius
clawdius chat --provider ollama --model llama3
```

### 4. Code Review (`code-review/`)

Demonstrates:
- Analyze existing code
- Generate review comments
- Suggest improvements

```bash
cd code-review
clawdius analyze
```

### 5. Plugin Development (`plugin-dev/`)

Shows how to:
- Create a Clawdius plugin
- Use the plugin API
- Test plugins locally

```bash
cd plugin-dev
clawdius plugin build
clawdius plugin test
```

## Quick Start Examples

### Generate a Function

```bash
clawdius generate "Create a function that validates email addresses"
```

### Generate with Streaming

```bash
clawdius generate --stream "Create a REST API endpoint for user authentication"
```

### Generate Tests

```bash
clawdius test --file src/auth.rs --function validate_email
```

### Generate Documentation

```bash
clawdius doc --file src/auth.rs --format rustdoc
```

### Analyze Code

```bash
clawdius analyze --path ./src
```

### Watch for Changes

```bash
clawdius watch --path ./src --on-change analyze
```

## Mode Examples

### Single-Pass Mode

Generate code in one LLM call:

```bash
clawdius generate --mode single-pass "Create a sorting function"
```

### Iterative Mode

Refine code through multiple iterations:

```bash
clawdius generate --mode iterative --max-iterations 5 "Create an optimized search algorithm"
```

### Agent-Based Mode

Use autonomous agent for complex tasks:

```bash
clawdius generate --mode agent "Create a complete authentication system with JWT tokens"
```

## Provider Examples

### Anthropic (Default)

```bash
clawdius chat --provider anthropic --model claude-sonnet-4-20250514
```

### OpenAI

```bash
clawdius chat --provider openai --model gpt-4o
```

### Ollama (Local)

```bash
# List available models
ollama list

# Chat with local model
clawdius chat --provider ollama --model llama3
clawdius chat --provider ollama --model codellama
clawdius chat --provider ollama --model mistral
```

## Configuration Examples

### Project Configuration

Create `.clawdius/config.toml` in your project:

```toml
[general]
provider = "anthropic"
model = "claude-sonnet-4-20250514"

[session]
auto_save = true
max_history = 500

[sandbox]
default_tier = "standard"
allowed_paths = ["./src", "./tests"]

[graph_rag]
enabled = true
max_files = 5000
```

### Custom Provider

```toml
[providers.custom]
name = "my-company-llm"
base_url = "https://llm.company.com/v1"
api_key_env = "COMPANY_LLM_API_KEY"
model = "company-model-v1"
```

## Integration Examples

### VSCode

1. Install Clawdius extension
2. Open Command Palette: `Clawdius: Start Chat`
3. Or use `Ctrl+Shift+P` → `Clawdius: Generate Code`

### Vim/Neovim

```vim
" Add to init.vim
Plug 'clawdius/vim-clawdius'

" Commands
:ClawdiusChat
:ClawdiusGenerate
:ClawdiusAnalyze
```

### Emacs

```elisp
;; Add to init.el
(use-package clawdius
  :ensure t
  :bind (("C-c c c" . clawdius-chat)
         ("C-c c g" . clawdius-generate)))
```

## Enterprise Examples

### SSO Integration

```toml
[enterprise.sso]
enabled = true
provider = "okta"

[enterprise.sso.okta]
domain = "company.okta.com"
client_id = "your-client-id"
```

### Audit Logging

```toml
[enterprise.audit]
enabled = true
backend = "elasticsearch"

[enterprise.audit.elasticsearch]
url = "https://elasticsearch.company.com"
index = "clawdius-audit"
api_key_env = "ES_API_KEY"
```

## Troubleshooting

### Common Issues

**API Key Not Found:**
```bash
# Set via environment
export ANTHROPIC_API_KEY=sk-ant-xxxxx

# Or use keyring
clawdius auth login
```

**Ollama Connection Failed:**
```bash
# Check Ollama is running
curl http://localhost:11434/api/tags

# Start Ollama
ollama serve
```

**Sandbox Permission Denied:**
```bash
# Check allowed paths
clawdius config get sandbox.allowed_paths

# Add path
clawdius config set sandbox.allowed_paths '["./src", "./workspace"]'
```

## Getting Help

- **Documentation:** https://docs.clawdius.dev
- **GitHub Issues:** https://github.com/WyattAu/clawdius/issues
- **Discord:** https://discord.gg/clawdius
- **Discussions:** https://github.com/WyattAu/clawdius/discussions
