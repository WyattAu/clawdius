# Configuration

Clawdius is highly configurable. This guide covers the essential configuration options.

## Interactive Setup (Recommended)

New in v1.2.0: Use the interactive setup wizard for first-time configuration:

```bash
clawdius setup
```

The wizard provides:
- **Guided provider selection** with descriptions
- **Secure API key storage** using your system keyring
- **Preset configurations** for common use cases
- **Connectivity verification** for local LLMs (Ollama)
- **Quick start examples** after setup completes

### Setup Presets

| Preset | Use Case | Key Settings |
|--------|----------|--------------|
| **Balanced** | General development | Standard sandboxing, moderate caching |
| **Security** | Production/sensitive code | Maximum sandboxing, full audit logging |
| **Performance** | Speed-critical workflows | Aggressive caching, streaming enabled |
| **Development** | Plugin/core development | Verbose logging, debug features |

## Configuration File Location
Clawdius looks for configuration in the following locations (in order):
1. `CLAWDIUS_CONFIG` environment variable (if set)
2. `./.clawdius/config.toml` (project-local)
3. `~/.config/clawdius/config.toml` (user-level)
4. `/etc/clawdius/config.toml` (system-wide)
## Quick Setup
### Set API Key
```bash
# Using CLI
clawdius config set api_key sk-ant-xxxxx
# Or set environment variable
export ANTHROPIC_API_KEY=sk-ant-xxxxx
```
### Set Default Provider
```bash
clawdius config set provider anthropic
```

### Set Default Provider

```bash
clawdius config set provider anthropic
```

## Configuration File

Create `~/.config/clawdius/config.toml`:

```toml
# General Settings
[general]
provider = "anthropic"
model = "claude-sonnet-4-20250514"
max_tokens = 4096
temperature = 0.7

# API Keys (stored securely in keyring)
# Use: clawdius config set api_key YOUR_KEY

# Session Settings
[session]
auto_save = true
max_history = 1000
compaction_threshold = 50000

# Sandbox Settings
[sandbox]
default_tier = "standard"
allow_network = false
allowed_paths = ["~/projects"]

# Output Settings
[output]
format = "streaming"
theme = "dark"
show_token_count = true

# Graph-RAG Settings
[graph_rag]
enabled = true
max_files = 10000
embedding_model = "text-embedding-3-small"

# Plugin Settings
[plugins]
enabled = true
auto_update = false
trusted_sources = ["https://plugins.clawdius.dev"]

# Telemetry
[telemetry]
enabled = false
# Set to true to help improve Clawdius
```

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `ANTHROPIC_API_KEY` | Anthropic API key | - |
| `OPENAI_API_KEY` | OpenAI API key | - |
| `CLAWDIUS_CONFIG` | Custom config path | - |
| `CLAWDIUS_LOG` | Log level (debug, info, warn) | info |
| `CLAWDIUS_CACHE_DIR` | Cache directory | `~/.cache/clawdius` |
| `CLAWDIUS_DATA_DIR` | Data directory | `~/.local/share/clawdius` |

## Provider Configuration

### Anthropic Claude

```toml
[providers.anthropic]
api_key_env = "ANTHROPIC_API_KEY"
model = "claude-sonnet-4-20250514"
base_url = "https://api.anthropic.com"

[providers.anthropic.options]
max_tokens = 4096
temperature = 0.7
```

### OpenAI

```toml
[providers.openai]
api_key_env = "OPENAI_API_KEY"
model = "gpt-4o"

[providers.openai.options]
max_tokens = 4096
temperature = 0.7
```

### Ollama (Local)

```toml
[providers.ollama]
base_url = "http://localhost:11434"
model = "llama3.2"

[providers.ollama.options]
num_ctx = 4096
temperature = 0.7
```

### Custom Provider

```toml
[providers.custom]
name = "my-provider"
base_url = "https://api.example.com/v1"
api_key_env = "MY_API_KEY"
model = "my-model"

[providers.custom.headers]
X-Custom-Header = "value"
```

## Sandbox Configuration

### Sandbox Tiers

| Tier | Isolation | Use Case |
|------|-----------|----------|
| `minimal` | None | Trusted code only |
| `standard` | WASM + seccomp | Normal development |
| `hardened` | Bubblewrap | Untrusted code |
| `container` | Docker/Podman | Strong isolation |
| `gvisor` | gVisor runsc | Strong isolation |
| `firecracker` | MicroVM | Maximum isolation |

```toml
[sandbox]
default_tier = "standard"

# Per-tier configuration
[sandbox.tiers.standard]
backend = "wasm"
memory_limit_mb = 512
cpu_limit_percent = 50

[sandbox.tiers.hardened]
backend = "bubblewrap"
network = false
filesystem = "readonly"

[sandbox.tiers.container]
backend = "docker"
image = "clawdius/sandbox:latest"
```

### Path Allowlisting

```toml
[sandbox]
allowed_paths = [
    "~/projects",
    "~/src",
    "/tmp/clawdius"
]

# Read-only paths
read_only_paths = [
    "/usr/include",
    "~/.cargo/registry"
]

# Denied paths (always blocked)
denied_paths = [
    "~/.ssh",
    "~/.gnupg",
    "~/.config/clawdius/keys"
]
```

## Session Configuration

```toml
[session]
# Auto-save sessions
auto_save = true
auto_save_interval_secs = 60

# History limits
max_history = 1000
max_context_tokens = 100000

# Compaction settings
compaction_threshold = 50000
compaction_strategy = "sliding_window"  # or "summarize"
```

## Output Configuration

```toml
[output]
# Output format: streaming, batch, json
format = "streaming"

# Theme: dark, light, ansi
theme = "dark"

# Display options
show_token_count = true
show_timing = false
show_model = true

# Markdown rendering
markdown = true
code_highlighting = true
```

## Plugin Configuration

```toml
[plugins]
enabled = true
directory = "~/.local/share/clawdius/plugins"
auto_update = false

# Trusted plugin sources
trusted_sources = [
    "https://plugins.clawdius.dev"
]

# Disabled plugins (by ID)
disabled = []

# Plugin-specific settings
[plugins.settings."my-plugin"]
option1 = "value1"
option2 = true
```

## Enterprise Configuration

### SSO

```toml
[enterprise.sso]
enabled = true
provider = "okta"  # okta, azure, github, custom

[enterprise.sso.okta]
domain = "your-company.okta.com"
client_id = "your-client-id"

[enterprise.sso.azure]
tenant_id = "your-tenant-id"
client_id = "your-client-id"
```

### Audit Logging

```toml
[enterprise.audit]
enabled = true
backend = "sqlite"  # sqlite, elasticsearch, webhook

[enterprise.audit.sqlite]
path = "/var/log/clawdius/audit.db"

[enterprise.audit.elasticsearch]
url = "https://elasticsearch.example.com"
index = "clawdius-audit"
```

## CLI Commands

```bash
# View current configuration
clawdius config list

# Set a value
clawdius config set <key> <value>

# Get a value
clawdius config get <key>

# Edit configuration file
clawdius config edit

# Reset to defaults
clawdius config reset
```

## Next Steps

- [First Chat](./first-chat.md) - Start your first conversation
- [Sandboxing](../concepts/sandboxing.md) - Learn about security tiers
- [Enterprise SSO](../enterprise/sso.md) - Configure enterprise features
