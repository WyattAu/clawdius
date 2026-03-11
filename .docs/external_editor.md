# External Editor Support

Clawdius supports editing long prompts in your preferred external text editor. This is useful for composing complex queries, multi-line code snippets, or detailed instructions.

## Quick Start

```bash
# Open editor with default template
clawdius edit

# Open editor with initial content
clawdius edit --initial "Start typing here..."

# Use a specific editor
clawdius edit --editor code

# Use a specific file extension for syntax highlighting
clawdius edit -x rs
```

## Configuration

### Setting Your Editor

Clawdius detects your editor from environment variables in this order:

1. `$EDITOR`
2. `$VISUAL`
3. Platform default (vim on Unix, notepad on Windows)

To set your preferred editor:

```bash
# Bash/Zsh (add to ~/.bashrc or ~/.zshrc)
export EDITOR=vim

# Or for VS Code
export EDITOR="code --wait"

# Or for Neovim
export EDITOR=nvim
```

### Editor-Specific Setup

#### VS Code
```bash
export EDITOR="code --wait"
```

#### Neovim
```bash
export EDITOR=nvim
```

#### Sublime Text
```bash
export EDITOR="subl --wait"
```

#### Emacs
```bash
export EDITOR=emacs
```

#### Nano
```bash
export EDITOR=nano
```

#### JetBrains IDEs (IntelliJ, PyCharm, etc.)
```bash
# Use the built-in terminal editor or
export EDITOR="idea --wait"
```

## Features

### Comment Stripping

When using `clawdius edit` without specifying an extension, comment lines (starting with `#`) are automatically stripped:

```
# This is a comment and will be removed
This is your actual prompt
# Another comment
More prompt content
```

Result:
```
This is your actual prompt
More prompt content
```

### Syntax Highlighting

Use the `-x` (or `--extension`) flag to get appropriate syntax highlighting:

```bash
# For Rust code
clawdius edit -x rs

# For Python code
clawdius edit -x py

# For TypeScript
clawdius edit -x ts
```

### Integration with Chat

The `--editor` flag is also available in the `chat` command:

```bash
# Open editor to compose chat message
clawdius chat --editor

# With a specific mode
clawdius chat --editor --mode architect
```

## Workflow Examples

### Composing a Complex Refactor Request

```bash
clawdius edit --extension rs
```

In your editor:
```rust
// Refactor the following function to use async/await:

fn fetch_data(url: &str) -> Result<String, Error> {
    // ... existing code ...
}

// Requirements:
// 1. Make it async
// 2. Add timeout support
// 3. Improve error handling
```

### Writing Multi-File Instructions

```bash
clawdius edit
```

In your editor:
```markdown
# Task: Update API Endpoints

## Files to modify:
1. src/api/users.rs - Add pagination
2. src/api/posts.rs - Add filtering
3. src/api/comments.rs - Add rate limiting

## Requirements:
- Maintain backward compatibility
- Add unit tests
- Update documentation
```

### Code Review Notes

```bash
clawdius edit --initial "# Code review notes for PR #123"
```

## Troubleshooting

### Editor Not Found

If you see "No editor found", set the `EDITOR` environment variable:

```bash
export EDITOR=vim
```

### Editor Opens but Doesn't Wait

Some editors (like VS Code) return immediately. Use the `--wait` flag:

```bash
export EDITOR="code --wait"
```

### Permission Denied

Ensure your editor is in your PATH and executable:

```bash
which vim
# /usr/bin/vim

chmod +x /path/to/your/editor
```

### Temporary Files

Clawdius creates temporary files in your system's temp directory. These are automatically cleaned up after editing.

## Platform Notes

### Linux
- Default editor: `vim`
- Temp directory: `/tmp`

### macOS
- Default editor: `vim`
- Temp directory: `/var/folders/...`

### Windows
- Default editor: `notepad`
- Temp directory: `%TEMP%`

## Advanced Usage

### Using with Pipes

```bash
# Edit a file's contents before sending to chat
cat src/main.rs | clawdius edit --extension rs
```

### Custom Templates

Create a template file and use it:

```bash
# Create template
cat > ~/.config/clawdius/templates/review.md << 'EOF'
# Code Review

## Summary
[Brief summary of changes]

## Issues Found
- [ ] Issue 1
- [ ] Issue 2

## Suggestions
1. ...
2. ...
EOF

# Use template
clawdius edit --initial "$(cat ~/.config/clawdius/templates/review.md)"
```

### Aliases

Add convenient aliases to your shell:

```bash
# Bash/Zsh
alias ce='clawdius edit'
alias ce-rs='clawdius edit -x rs'
alias ce-py='clawdius edit -x py'
alias ce-ts='clawdius edit -x ts'
```
