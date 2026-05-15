# @mentions

Clawdius supports @mentions to include files, directories, and other context in your messages.

## Overview

When composing a message, use @mentions to attach files to the conversation context. The mention resolver reads the referenced files and includes their contents in the LLM's context window.

## Syntax

```
@path/to/file.rs     Include a specific file
@src/                Include a directory (all files)
@lib.rs              Relative path to a file
```

## Examples

```bash
# Reference a file in your message
clawdius chat "explain @src/main.rs"

# Reference multiple files
clawdius chat "compare @src/old.rs and @src/new.rs"

# Reference a directory
clawdius chat "review the API handlers in @src/api/"
```

## How It Works

1. The mention resolver scans your message for `@` prefixed paths
2. It resolves each path relative to the current working directory
3. File contents are read and included as context items
4. The context is sent to the LLM along with your message

## Context Items

Mentions are resolved into `ContextItem` objects:

```rust
pub struct ContextItem {
    pub path: PathBuf,
    pub content: String,
    pub item_type: ContextItemType,
}
```

## Limitations

- Large files may exceed context limits. Use specific files or functions rather than entire directories.
- Binary files cannot be included via mentions.
- Paths are resolved relative to the project root (or `--cwd` if specified).

## Combining with Editor Mode

```bash
# Compose in editor with file references
clawdius chat --editor
# Write your message with @mentions in the editor
```

## Integration with Graph-RAG

When the `vector-db` feature is enabled, @mentions are supplemented with semantic context from the Graph-RAG index, providing related code even from files not explicitly mentioned.
