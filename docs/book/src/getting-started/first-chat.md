# First Chat

Now that Clawdius is installed and configured, let's start your first conversation.

## Starting a Chat Session

### Interactive Mode

```bash
clawdius chat
```

This opens an interactive REPL where you can have a conversation:

```
╭─────────────────────────────────────────────────────────────────╮
│  Clawdius v1.0.0 - High-Assurance AI Coding Assistant          │
│  Provider: Anthropic (claude-sonnet-4-20250514)                 │
│  Sandbox: standard (WASM)                                       │
╰─────────────────────────────────────────────────────────────────╯

You: Hello! Can you help me write a Rust function to parse JSON?

Clawdius: I'd be happy to help! Here's a function that parses JSON using 
serde_json:

```rust
use serde_json::Value;

fn parse_json(input: &str) -> Result<Value, serde_json::Error> {
    serde_json::from_str(input)
}
```

You can use it like this:
...
```

### One-shot Mode

For quick questions:

```bash
clawdius chat --message "What is the difference between Vec and VecDeque in Rust?"
```

### With File Context

```bash
# Include files for context
clawdius chat --file src/main.rs --file src/lib.rs

# Or include entire directory
clawdius chat --dir src/
```

## Basic Commands

Inside the chat session:

| Command | Description |
|---------|-------------|
| `/help` | Show available commands |
| `/clear` | Clear conversation history |
| `/save [name]` | Save current session |
| `/load <name>` | Load a saved session |
| `/model <name>` | Switch model |
| `/mode <mode>` | Switch mode (code, architect, review) |
| `/checkpoint` | Create a checkpoint |
| `/undo` | Undo last action |
| `/redo` | Redo last undone action |
| `/exit` | Exit the session |

## Using @mentions

Clawdius supports @mentions to reference files, directories, or previous context:

```bash
You: Please review @src/main.rs and suggest improvements

You: What patterns are used in @src/ directory?

You: Based on our @previous discussion, implement the feature

You: Use @commit:abc123 as reference
```

### Available Mentions

| Mention | Description |
|---------|-------------|
| `@file.rs` | Reference a file |
| `@directory/` | Reference all files in directory |
| `@previous` | Reference previous context |
| `@commit:hash` | Reference a git commit |
| `@issue:123` | Reference a GitHub issue |
| `@pr:456` | Reference a GitHub PR |
| `@url:https://...` | Reference a URL |

## Modes

Clawdius has different modes optimized for different tasks:

### Code Mode (default)

Optimized for writing and editing code:

```bash
clawdius chat --mode code
```

### Architect Mode

High-level system design and planning:

```bash
clawdius chat --mode architect
```

### Review Mode

Code review and quality analysis:

```bash
clawdius chat --mode review
```

### Debug Mode

Debugging and troubleshooting:

```bash
clawdius chat --mode debug
```

## File Operations

Clawdius can read, create, edit, and delete files:

```
You: Read the file src/main.rs and explain what it does

Clawdius: I'll read the file...

[Reads src/main.rs]

This file contains the main entry point for your application...

You: Create a new file src/utils.rs with helper functions

Clawdius: I'll create src/utils.rs with some utility functions...

[Creates src/utils.rs]

Created src/utils.rs with the following helper functions:
- `truncate_string` - Truncates a string to max length
- `format_bytes` - Formats bytes as human-readable string
...

You: Add a function to validate email addresses in src/utils.rs

Clawdius: I'll add an email validation function...

[Edits src/utils.rs]

Added `validate_email` function at line 45.
```

## Git Operations

Clawdius can interact with git:

```
You: What changes have been made since the last commit?

Clawdius: I'll check the git status...

[Runs git status and git diff]

Modified files:
- src/main.rs (3 additions, 1 deletion)
- src/utils.rs (15 additions)

You: Commit these changes with a descriptive message

Clawdius: I'll commit these changes...

[Runs git add and git commit]

Committed: "Add utility functions and update main"
Commit hash: abc1234
```

## Checkpoints and Undo

Clawdius tracks all changes and allows undo/redo:

```
You: /checkpoint

Clawdius: Created checkpoint "before-refactor" at 2024-01-15 10:30:00

You: Refactor the authentication module

Clawdius: I'll refactor the authentication module...

[Refactors auth module]

You: Actually, let me undo that

Clawdius: Undid the refactoring changes. Restored to checkpoint "before-refactor"

You: /redo

Clawdius: Redid the refactoring changes.
```

## Session Management

### Save Session

```
You: /save auth-feature

Clawdius: Session saved as "auth-feature"
```

### Load Session

```
You: /load auth-feature

Clawdius: Loaded session "auth-feature" from 2024-01-15 10:30:00
Restored context: 5 files, 12 messages
```

### List Sessions

```bash
clawdius session list
```

### Export Session

```bash
clawdius session export auth-feature --format markdown
```

## Tips for Effective Chats

1. **Be specific**: Provide context and clarify your requirements
2. **Use @mentions**: Reference relevant files and context
3. **Use checkpoints**: Create checkpoints before major changes
4. **Review changes**: Always review code changes before applying
5. **Iterate**: Start simple and refine through conversation

## Next Steps

- [Basic Usage](./basic-usage.md) - Learn more about daily workflows
- [Tools](../concepts/tools.md) - Understand available tools
- [Modes](../features/modes.md) - Deep dive into different modes
