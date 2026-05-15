# Configuration Schema

The Clawdius configuration file is a TOML document with the following schema.

## File Locations

Clawdius looks for configuration in this order (later overrides earlier):

1. `.clawdius/config.toml` (default)
2. `clawdius.toml` (in current directory)
3. Path specified by `--config` / `-C` flag

## Top-Level Schema

```toml
[project]
name = "my-project"
rigor_level = "high"           # low | medium | high
lifecycle_phase = "context_discovery"

[workspace]
name = "default"
storage = "sqlite"              # sqlite | postgres | mariadb
database_path = ".clawdius/workspace.db"
postgres_url = ""               # For postgres backend
mariadb_url = ""                # For mariadb backend
per_project_tokens = 2000
max_total_tokens = 8000

[storage]
database_path = ".clawdius/graph/index.db"
vector_path = ".clawdius/graph/vectors.lance"
sessions_path = ".clawdius/sessions.db"

[llm]
default_provider = "anthropic"  # anthropic | openai | ollama | zai | deepseek | google
max_tokens = 4096

[llm.anthropic]
model = "claude-sonnet-4-20250514"
api_key_env = "ANTHROPIC_API_KEY"
api_key = ""                    # Not recommended
base_url = ""                   # Custom endpoint

[llm.openai]
model = "gpt-4o"
api_key_env = "OPENAI_API_KEY"

[llm.ollama]
model = "llama3.2"
base_url = "http://localhost:11434"

[llm.retry]
max_retries = 3
initial_delay_ms = 1000
max_delay_ms = 30000
exponential_base = 2.0
retry_on = ["rate_limit", "timeout", "server_error", "network_error"]

[session]
compact_threshold = 0.85
keep_recent = 4
min_messages = 10
auto_save = true

[output]
show_progress = true
format = "text"                # text | json | stream-json

[shell_sandbox]
blocked_commands = [
    "rm -rf /",
    "mkfs",
    "dd if=/dev/zero",
]
timeout_secs = 120
max_output_bytes = 1048576
restrict_to_cwd = true
```

## Section Reference

### `[project]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `name` | string | `"clawdius"` | Project name |
| `rigor_level` | string | `"high"` | Rigor level: `low`, `medium`, `high` |
| `lifecycle_phase` | string | `"context_discovery"` | Current Nexus FSM phase |

### `[storage]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `database_path` | path | `.clawdius/graph/index.db` | SQLite database path |
| `vector_path` | path | `.clawdius/graph/vectors.lance` | LanceDB vector store path |
| `sessions_path` | path | `.clawdius/sessions.db` | Sessions database path |

### `[llm]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `default_provider` | string | (none) | Default LLM provider |
| `max_tokens` | integer | `4096` | Maximum response tokens |

### `[session]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `compact_threshold` | float | `0.85` | Auto-compact at this context fraction |
| `keep_recent` | integer | `4` | Messages to keep when compacting |
| `min_messages` | integer | `10` | Minimum messages before compacting |
| `auto_save` | boolean | `true` | Auto-save sessions |

### `[shell_sandbox]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `blocked_commands` | array of strings | (see defaults) | Blocked command patterns |
| `timeout_secs` | integer | `120` | Command timeout in seconds |
| `max_output_bytes` | integer | `1048576` | Maximum output size in bytes |
| `restrict_to_cwd` | boolean | `true` | Restrict to project directory |

### `[llm.retry]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `max_retries` | integer | `3` | Maximum retry attempts |
| `initial_delay_ms` | integer | `1000` | Initial delay in ms |
| `max_delay_ms` | integer | `30000` | Maximum delay cap in ms |
| `exponential_base` | float | `2.0` | Backoff multiplier |
| `retry_on` | array of strings | (all) | Conditions: `rate_limit`, `timeout`, `server_error`, `network_error` |

## Viewing Configuration

```bash
clawdius config show       # Show current config (API keys masked)
clawdius config path       # Show config file path
clawdius config list       # List available keys
clawdius config get llm.default_provider
clawdius config set llm.default_provider openai
```

## Environment Variable Overrides

API keys can be set via environment variables (highest priority):

| Variable | Provider |
|----------|----------|
| `ANTHROPIC_API_KEY` | Anthropic |
| `OPENAI_API_KEY` | OpenAI |
| `DEEPSEEK_API_KEY` | DeepSeek |
| `ZAI_API_KEY` | Z.AI |
| `OLLAMA_BASE_URL` | Ollama server URL |
