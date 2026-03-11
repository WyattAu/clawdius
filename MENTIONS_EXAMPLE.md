# @Mentions System - Complete Guide

The @mentions system allows you to include rich context in your messages to Clawdius.

## Overview

@mentions are parsed from your message text and resolved to include relevant context before sending to the LLM. This allows you to reference files, folders, URLs, git state, and more.

## Supported Mention Types

### 📄 @file:path

Include the contents of a file:

```bash
clawd chat "Explain this code @file:src/main.rs"
clawd chat "Compare @file:src/a.rs with @file:src/b.rs"
```

**Context Added:**
- Full file contents
- Language identifier (for syntax highlighting)
- File path

---

### 📁 @folder:path

List the contents of a folder:

```bash
clawd chat "What files are in @folder:src/components?"
clawd chat "Describe the structure @folder:crates/clawdius/src"
```

**Context Added:**
- Folder path
- List of files (non-hidden files only)

---

### 🌐 @url:https://...

Fetch and include content from a URL:

```bash
clawd chat "Summarize @url:https://doc.rust-lang.org/book/ch01-00-getting-started.html"
clawd chat "Review this documentation @url:https://example.com/api-docs"
```

**Context Added:**
- URL
- Fetched content (converted to markdown)
- Page title (if available)

---

### 🔧 @problems[:severity]

Include workspace diagnostics and errors:

```bash
clawd chat "Fix these issues @problems"
clawd chat "Show only errors @problems:error"
clawd chat "What warnings do we have? @problems:warning"
```

**Context Added:**
- List of diagnostics
- File paths, line numbers, severity, messages

**Note:** Requires LSP integration (coming soon)

---

### 🌳 @git:diff or @git:staged

Include git diff in your context:

```bash
clawd chat "Review my changes @git:diff"
clawd chat "Review staged changes @git:staged"
```

**Context Added:**
- Full git diff output
- Whether staged or unstaged

---

### 📝 @git:log:N

Include recent commit history:

```bash
clawd chat "What changed recently? @git:log:5"
clawd chat "Summarize the last 10 commits @git:log:10"
```

**Context Added:**
- Commit hashes
- Authors
- Commit messages
- Timestamps

---

### 🔍 @symbol:name

Include a symbol definition:

```bash
clawd chat "Explain this function @symbol:parse_function"
clawd chat "Where is @symbol:Config defined?"
```

**Context Added:**
- Symbol name
- Location (file:line)
- Definition content

**Note:** Requires tree-sitter parsing (coming soon)

---

### 🔎 @search:"query" or @search:query

Search the codebase:

```bash
clawd chat "Find all uses of @search:\"error handling\""
clawd chat "Where is @search:authenticate used?"
```

**Context Added:**
- Search query
- Matching files and lines
- Relevance scores

## Examples

### Code Review

```bash
clawd chat "Review this PR: @git:diff and @git:log:3"
```

This includes both the changes and recent commits for context.

---

### Debugging

```bash
clawd chat "Why is this failing? @file:src/parser.rs @problems:error"
```

Includes the problematic file and any error diagnostics.

---

### Documentation

```bash
clawd chat "Document the API in @file:src/api.rs based on @url:https://example.com/api-spec"
```

Includes the code file and external API specification.

---

### Architecture Understanding

```bash
clawd chat "Explain the structure of @folder:src/tui_app and its main @symbol:App"
```

Combines folder listing with symbol definition.

---

## Implementation Details

### Parsing

@mentions are parsed using regex patterns in `clawdius-core/src/context/mentions.rs`:

- `@file:([^\s]+)` - Matches file paths
- `@folder:([^\s]+)` - Matches folder paths
- `@url:(https?://[^\s]+)` - Matches URLs
- `@problems(?::(\w+))?` - Matches problems with optional severity
- `@git:(diff|staged|log)(?::(\d+))?` - Matches git commands
- `@symbol:([^\s]+)` - Matches symbol names
- `@search:"([^"]+)"` - Matches quoted search queries
- `@search:(?!")([^\s]+)` - Matches unquoted search queries

### Resolution

The `MentionResolver` resolves mentions to `ContextItem` objects:

1. **File**: Reads file contents, detects language
2. **Folder**: Lists directory contents
3. **Url**: Fetches URL, converts HTML to markdown
4. **Problems**: Queries LSP for diagnostics (placeholder)
5. **Git**: Executes git commands
6. **Symbol**: Queries tree-sitter/LSP (placeholder)
7. **Search**: Uses ripgrep for text search

### Integration

@mentions work in:

- ✅ CLI `chat` command (`clawd chat "message with @mentions"`)
- ✅ TUI chat mode (press `i` to insert, type message with @mentions)
- ✅ Headless mode (stdin with @mentions)
- ✅ Autocomplete suggestions (TUI)
- ✅ Syntax highlighting (TUI)

### Testing

Run the @mentions test suite:

```bash
cargo test --lib -p clawdius-core mentions
```

All tests pass:
- File mention parsing
- Folder mention parsing
- URL mention parsing
- Problems mention parsing
- Git diff/log parsing
- Symbol mention parsing
- Search query parsing (quoted and unquoted)
- Multiple mentions in one message
- Mention position tracking
- No mentions (edge case)
- **File resolution (integration)**
- **Folder resolution (integration)**
- **Multiple mentions resolution (integration)**

## Future Enhancements

- [ ] Recursive folder support (`@folder:src/**`)
- [ ] LSP integration for `@problems` and `@symbol`
- [ ] Semantic search with LanceDB for `@search`
- [ ] Mention preview in TUI
- [ ] Mention validation and error messages
- [ ] Mention caching for repeated references
- [x] Mention autocomplete in TUI ✅
- [x] Mention syntax highlighting in TUI ✅

---

## Contributing

To add a new mention type:

1. Add regex pattern to `Mention::parse()` in `mentions.rs`
2. Add variant to `Mention` enum
3. Implement resolution in `MentionResolver::resolve()`
4. Add `ContextItem` variant if needed
5. Add tests to the test module
6. Update this documentation

---

**Version:** 1.0.0  
**Last Updated:** 2026-03-05
