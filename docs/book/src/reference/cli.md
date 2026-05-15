# CLI Commands

Complete reference for all Clawdius CLI commands.

## Global Flags

| Flag | Short | Description |
|------|-------|-------------|
| `--no-tui` | `-n` | Run without TUI (headless mode) |
| `--cwd` | `-w` | Working directory (default: `.`) |
| `--output-format` | `-f` | Output format: `text`, `json`, `stream-json` |
| `--quiet` | `-q` | Suppress progress indicators |
| `--config` | `-C` | Path to config file |
| `--lang` | `-L` | Output language (en, zh, ja, ko, de, fr, es, it, pt, ru) |

## Commands

### `clawdius chat` [MESSAGE]

Send a message to the LLM.

| Flag | Short | Description | Default |
|------|-------|-------------|---------|
| `--provider` | `-P` | LLM provider | `anthropic` |
| `--model` | `-m` | Model to use | Provider default |
| `--session` | `-s` | Session ID to continue | New session |
| `--mode` | `-M` | Agent mode | `code` |
| `--editor` | `-e` | Open external editor | Disabled |
| `--exit` | | Non-interactive mode | Auto |
| `--quiet` | | Suppress output except response | Disabled |
| `--auto-approve` | | Auto-approve tool executions | Disabled |

### `clawdius auto` \<TASK\>

Autonomous CI/CD mode.

| Flag | Description | Default |
|------|-------------|---------|
| `--provider` | LLM provider | `anthropic` |
| `--model` | Model | Provider default |
| `--max-iterations` | Max iterations | 50 |
| `--run-tests` | Run tests after changes | Disabled |
| `--auto-commit` | Commit automatically | Disabled |
| `--fail-on-test-failure` | Fail on test failure | Disabled |
| `--output-format` | CI output format | `text` |

### `clawdius init` [NAME]

Initialize a Clawdius project.

### `clawdius setup` [--quick] [--provider]

Interactive setup wizard.

### `clawdius sessions` [--delete ID] [--search QUERY]

List and manage sessions.

### `clawdius generate` \<PROMPT\>

Generate code with AI.

| Flag | Short | Description | Default |
|------|-------|-------------|---------|
| `--files` | `-f` | Target files (comma-separated) | None |
| `--mode` | `-M` | `single-pass`, `iterative`, `agent` | `single-pass` |
| `--trust` | `-T` | `low`, `medium`, `high` | `medium` |
| `--max-iterations` | `-i` | Max iterations | 5 |
| `--dry-run` | | Preview only | Disabled |
| `--provider` | `-P` | LLM provider | `anthropic` |
| `--model` | `-m` | Model | Provider default |

### `clawdius git` \<COMMAND\>

| Command | Description |
|---------|-------------|
| `commit [FILES...]` | Stage and commit with AI message |
| `diff` | Show working diff |
| `diff --staged` | Show staged diff |
| `status` | Show git status |

### `clawdius checkpoint` \<COMMAND\>

| Command | Description |
|---------|-------------|
| `create <desc>` | Create a checkpoint |
| `list` | List checkpoints |
| `show <id>` | Show checkpoint details |
| `restore <id>` | Restore a checkpoint |
| `compare <id1> <id2>` | Compare two checkpoints |
| `delete <id>` | Delete a checkpoint |
| `cleanup --keep N` | Keep only N most recent |

### `clawdius timeline` \<COMMAND\>

| Command | Description |
|---------|-------------|
| `create <name>` | Create timeline entry |
| `list` | List entries |
| `watch` | Auto-create on file changes |
| `rollback <id>` | Rollback to checkpoint |
| `diff <from> <to>` | Compare checkpoints |
| `history <file>` | File change history |

### `clawdius auth` \<COMMAND\> (requires `keyring` feature)

| Command | Description |
|---------|-------------|
| `set <provider>` | Store API key |
| `get <provider>` | Retrieve API key |
| `delete <provider>` | Delete API key |

### `clawdius modes` \<COMMAND\>

| Command | Description |
|---------|-------------|
| `list` | List all modes |
| `create <name>` | Create custom mode |
| `show <name>` | Show mode details |

### `clawdius models` \<COMMAND\>

| Command | Description |
|---------|-------------|
| `list` | List local models |
| `pull <model>` | Pull a model |
| `health` | Check Ollama server |
| `current` | Show current model |

### `clawdius sprint` \<TASK\>

Run a multi-phase agentic sprint.

| Flag | Description | Default |
|------|-------------|---------|
| `--max-iterations` | Max build-test iterations | 3 |
| `--real-execution` | Enable real execution | Disabled |
| `--auto-approve` | Skip confirmations | Disabled |
| `--provider` | LLM provider | `deepseek` |
| `--model` | Model | Provider default |
| `--resume` | Resume from saved state | Disabled |

### `clawdius config` \<COMMAND\>

| Command | Description |
|---------|-------------|
| `show` | Show current config (keys masked) |
| `get <key>` | Get a config value |
| `set <key> <value>` | Set a config value |
| `path` | Show config file path |
| `list` | List available keys |

### Other Commands

| Command | Description |
|---------|-------------|
| `clawdius refactor --from LANG --to LANG [PATH]` | Cross-language refactoring |
| `clawdius test <FILE>` | Generate tests |
| `clawdius doc <FILE>` | Generate documentation |
| `clawdius analyze [PATH]` | Architecture drift analysis |
| `clawdius watch [PATH]` | Watch files for changes |
| `clawdius edit` | Edit prompt in external editor |
| `clawdius memory` | Manage project memory |
| `clawdius webhook` | Manage webhooks |
| `clawdius metrics` | Show performance metrics |
| `clawdius telemetry` | Configure telemetry |
| `clawdius complete <FILE> <LINE> <COL>` | Code completions |
| `clawdius verify --proof PATH` | Lean 4 proof verification |
| `clawdius server --host H --port P` | HTTP server |
| `clawdius ship` | Pre-ship checks and commit messages |
| `clawdius skill` | List and execute skills |
