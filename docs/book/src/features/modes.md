# Modes

Agent modes change the LLM's system prompt and behavior to specialize for different tasks.

## Built-in Modes

| Mode | Description |
|------|-------------|
| `code` | General-purpose coding assistant (default) |
| `architect` | System design and high-level architecture |
| `ask` | Q&A without executing tools |
| `debug` | Debugging and error diagnosis |
| `review` | Code review and quality analysis |
| `refactor` | Refactoring guidance and execution |
| `test` | Test generation |
| `auto` | Fully autonomous execution |

## Usage

```bash
clawdius chat --mode architect "Design a payment system"
clawdius chat --mode debug "Why does this test fail?"
clawdius chat --mode review "Review src/main.rs"
clawdius chat --mode test "Write tests for the auth module"
clawdius chat --mode auto "Fix the CI pipeline"
```

## Listing Modes

```bash
clawdius modes list
```

## Showing Mode Details

```bash
clawdius modes show architect
```

## Custom Modes

Create your own agent mode:

```bash
clawdius modes create security-review
```

This creates a mode configuration file that you can customize with:
- System prompt
- Allowed tools
- Temperature and other model parameters
- Custom instructions

Save to a specific path:

```bash
clawdius modes create security-review --output .clawdius/modes/
```

## Mode Behavior

Each mode configures:

- **System prompt**: Instructions that guide the LLM's behavior
- **Tool access**: Which tools are available
- **Auto-approve**: Whether tools execute without confirmation
- **Output style**: How responses are formatted

The `code` mode is the default and provides full tool access with confirmation prompts. The `auto` mode enables auto-approve for all tool executions. The `ask` mode disables tool execution entirely.

## Using Custom Modes

Once created, use a custom mode like a built-in one:

```bash
clawdius chat --mode security-review "Audit the authentication flow"
```
