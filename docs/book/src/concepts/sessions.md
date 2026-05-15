# Sessions

Clawdius manages conversation state through a session system backed by SQLite.

## Overview

Every chat interaction belongs to a session. Sessions store the full conversation history, metadata, and can be resumed across invocations.

## Session Storage

Sessions are stored in SQLite at `.clawdius/sessions.db` (configurable via `storage.sessions_path`).

```toml
[storage]
sessions_path = ".clawdius/sessions.db"
```

## Session Lifecycle

### Creating a Session

A session is created automatically when you start chatting:

```bash
clawdius chat
# Session ID is printed on start
```

### Continuing a Session

Resume a previous session with `--session`:

```bash
clawdius chat "continue the refactoring" --session abc123
```

### Listing Sessions

```bash
clawdius sessions
```

Output includes session IDs, timestamps, and message counts.

### Searching Sessions

```bash
clawdius sessions --search "error handling"
```

### Deleting a Session

```bash
clawdius sessions --delete abc123
```

## Session Configuration

```toml
[session]
compact_threshold = 0.85    # Auto-compact at 85% of context limit
keep_recent = 4             # Keep last 4 messages when compacting
min_messages = 10           # Minimum messages before compacting
auto_save = true            # Automatically save after each message
```

### Context Compaction

When a session approaches the model's context limit (controlled by `compact_threshold`), Clawdius automatically compacts the conversation. The `keep_recent` setting determines how many recent messages are preserved in full.

### Manual Compaction

Start a fresh context while preserving the session metadata:

```bash
clawdius chat "compact" --session abc123
```

## Active Session

When you start `clawdius chat` without specifying a session, the most recently active session is resumed. If no sessions exist, a new one is created.

## Multi-Provider Sessions

You can switch providers within a session:

```bash
clawdius chat "quick question" --provider openai --session abc123
clawdius chat "deep analysis" --provider anthropic --session abc123
```

## Headless Mode

For programmatic usage, Clawdius can read from stdin in headless mode:

```bash
clawdius --no-tui
# Type messages, press Enter
# Ctrl+D to exit
```

Or pipe messages:

```bash
echo "analyze src/lib.rs" | clawdius --no-tui
```

## Session Internals

Each session contains:

- **Messages**: Full conversation history (user, assistant, system roles)
- **Metadata**: Provider, model, creation time, update time
- **Context items**: Files and mentions attached to the conversation
- **Checkpoints**: Session-level file snapshots

The `SessionManager` in `clawdius-core` provides the API:

```rust
use clawdius_core::SessionManager;

let manager = SessionManager::new(&config)?;
let session = manager.get_or_create_active()?;
manager.add_message(&mut session, message).await?;
```
