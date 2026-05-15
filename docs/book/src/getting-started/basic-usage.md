# Basic Usage

This page covers common CLI patterns for day-to-day use of Clawdius.

## Chatting with Clawdius

### Interactive Chat

Start an interactive session with the default provider (Anthropic):

```bash
clawdius chat
```

The interactive mode supports multi-turn conversation. Type your message and press Enter. Use `Ctrl+C` to exit.

### Single-Shot Messages

Pass a message directly on the command line:

```bash
clawdius chat "Explain this codebase"
clawdius chat "Write tests for src/lib.rs"
```

### Composing Long Prompts

Open your system editor to compose a message:

```bash
clawdius chat --editor
clawdius chat --editor --extension rs
```

### Piping from Stdin

```bash
echo "Explain this function" | clawdius chat -
cat error.log | clawdius chat "What caused this error?"
```

## Selecting Providers and Models

Override the default provider and model for any command:

```bash
clawdius chat "Hello" --provider openai --model gpt-4o
clawdius chat "Hello" --provider anthropic --model claude-sonnet-4-20250514
clawdius chat "Hello" --provider ollama --model llama3.2
clawdius chat "Hello" --provider deepseek
clawdius chat "Hello" --provider zai
```

## Agent Modes

Clawdius ships with built-in agent modes that change the system prompt and tool behavior:

| Mode | Purpose |
|------|---------|
| `code` | Default coding assistant (default) |
| `architect` | High-level system design |
| `ask` | Q&A without code execution |
| `debug` | Debugging and error diagnosis |
| `review` | Code review |
| `refactor` | Refactoring guidance |
| `test` | Test generation |
| `auto` | Fully autonomous execution |

```bash
clawdius chat --mode architect "Design a microservice for auth"
clawdius chat --mode debug "Why is this test failing?"
clawdius chat --mode review "Review src/main.rs"
clawdius chat --mode auto "Fix the failing CI pipeline"
```

## Session Management

### Continuing a Session

Resume a previous conversation by session ID:

```bash
clawdius chat "Continue where we left off" --session abc123
```

### Listing Sessions

```bash
clawdius sessions
clawdius sessions --search "error handling"
```

### Deleting a Session

```bash
clawdius sessions --delete abc123
```

## Autonomous Mode

Run tasks without human interaction:

```bash
clawdius auto "fix failing tests" --run-tests --fail-on-test-failure
clawdius auto "implement user auth" --auto-commit
clawdius auto "resolve lint errors" --output-format json
```

The `--auto-approve` flag on `chat` enables autonomous tool execution within interactive mode:

```bash
clawdius chat --auto-approve "Refactor this module"
```

## Code Generation

Generate code with different strategies:

```bash
# Single-pass generation (fastest)
clawdius generate "create REST API handlers"

# Iterative generation with refinement
clawdius generate "add validation" --mode iterative --max-iterations 5

# Agent mode (plan, execute, verify)
clawdius generate "implement feature X" --mode agent

# Target specific files
clawdius generate "add error handling" --files src/api.rs,src/lib.rs

# Dry run (preview only)
clawdius generate "refactor database layer" --dry-run
```

## Git Integration

### AI-Generated Commits

```bash
# Stage all and generate commit message
clawdius git commit

# Stage specific files
clawdius git commit src/lib.rs src/config.rs

# Provide a message hint
clawdius git commit -m "fix: resolve null pointer in parser"
```

### Viewing Diffs

```bash
clawdius git diff
clawdius git diff --staged
clawdius git diff --file src/lib.rs
```

### Git Status

```bash
clawdius git status
```

## Agentic Sprint

Run a multi-phase sprint (think, plan, build, review, test, ship, reflect):

```bash
clawdius sprint "implement user authentication" --max-iterations 3
clawdius sprint "add search feature" --real-execution --auto-approve
```

## Output Formats

All commands support JSON output for scripting:

```bash
clawdius chat "Analyze this code" -f json
clawdius sessions -f json
clawdius metrics -f json -o metrics.json
```

Stream JSON for real-time processing:

```bash
clawdius chat -f stream-json
```

## Global Flags

| Flag | Short | Description |
|------|-------|-------------|
| `--no-tui` | `-n` | Run without TUI (headless mode) |
| `--cwd` | `-w` | Working directory |
| `--output-format` | `-f` | Output format: `text`, `json`, `stream-json` |
| `--quiet` | `-q` | Suppress progress indicators |
| `--config` | `-C` | Path to config file |
| `--lang` | `-L` | Output language (en, zh, ja, ko, de, fr, es, it, pt, ru) |

## Quick Reference

```bash
# Daily workflow
clawdius chat                              # Start interactive session
clawdius chat "question" --provider openai  # Quick question
clawdius git commit                        # Commit with AI message
clawdius sessions                          # List sessions
clawdius checkpoint create "milestone"     # Save state
clawdius auto "task" --run-tests           # Autonomous task
clawdius metrics                           # Check performance
```
