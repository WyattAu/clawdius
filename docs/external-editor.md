# External Editor Support

Clawdius supports opening prompts in your preferred external editor for composing complex messages.

## Features

- **Editor Detection**: Automatically detects `$EDITOR` or `$VISUAL` environment variables
- **Temporary File Workflow**: Creates `.md` files for editing
- **Content Preservation**: Saves draft content before opening editor
- **Cleanup**: Automatically removes temporary files after editing

## CLI Usage

### Using the --editor flag

```bash
# Open editor to compose a new message
clawdius chat --editor

# Open editor with initial draft content
clawdius chat --editor --message "Initial draft..."
```

### Workflow

1. Clawdius creates a temporary file with `.md` extension
2. Opens the file in your configured editor
3. Waits for you to save and close the editor
4. Reads the edited content
5. Cleans up the temporary file
6. Sends the message to the LLM

## TUI Usage

### Keyboard Shortcut

In the TUI, press `Ctrl+E` while in insert mode to open the external editor.

1. Press `i` to enter insert mode
2. Type any initial content (optional)
3. Press `Ctrl+E` to open external editor
4. Edit your message in the editor
5. Save and close the editor
6. The edited content is loaded back into the input field
7. Press `Enter` to send

### Help

Press `?` in the TUI to see all keyboard shortcuts including the editor shortcut.

## Configuration

### Setting Your Editor

Set the `EDITOR` or `VISUAL` environment variable:

```bash
# For vim
export EDITOR=vim

# For nano
export EDITOR=nano

# For VS Code
export EDITOR=code

# For Neovim
export EDITOR=nvim

# For Emacs
export EDITOR=emacs
```

### Common Editors

| Editor | Command | Notes |
|--------|---------|-------|
| Vim | `vim` | Modal editor, great for power users |
| Neovim | `nvim` | Modern Vim fork |
| Nano | `nano` | Simple, beginner-friendly |
| VS Code | `code` | Microsoft's editor, wait flag handled automatically |
| Emacs | `emacs` | Extensible, customizable editor |
| Sublime Text | `subl` | Fast, feature-rich editor |
| Atom | `atom` | GitHub's editor (deprecated) |

## API Usage

### Rust API

```rust
use clawdius_core::tools::editor::ExternalEditor;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut editor = ExternalEditor::new()?;
    
    // Open with initial content
    let content = editor.open_and_edit("Initial draft content")?;
    
    println!("Edited content: {}", content);
    Ok(())
}
```

### Custom Editor

```rust
use clawdius_core::tools::editor::ExternalEditor;

let editor = ExternalEditor::with_editor("code".to_string())?;
```

## Error Handling

The editor module handles various error conditions:

- **No editor found**: Set `$EDITOR` or `$VISUAL`
- **Temp file creation failed**: Check disk space and permissions
- **Editor launch failed**: Verify editor is installed and in PATH
- **Editor exit error**: Editor exited with non-zero status
- **Read error**: Failed to read edited content
- **Cleanup error**: Failed to remove temporary file

## Testing

Run the editor module tests:

```bash
cargo test -p clawdius-core --lib editor
```

## Examples

### Example 1: Quick Chat with Editor

```bash
export EDITOR=vim
clawdius chat --editor
```

### Example 2: Multi-line Prompt

```bash
# Start with a draft
clawdius chat --editor --message "Please review this code:"
# Editor opens, add your multi-line content
```

### Example 3: TUI Workflow

```
1. Launch TUI: clawdius
2. Press 'i' for insert mode
3. Press Ctrl+E to open editor
4. Compose complex message in editor
5. Save and close editor
6. Press Enter to send
```

## Implementation Details

### File Location

- **Module**: `crates/clawdius-core/src/tools/editor.rs`
- **CLI Integration**: `crates/clawdius/src/cli.rs`
- **TUI Integration**: `crates/clawdius/src/tui_app/app.rs`

### Temporary Files

- **Location**: System temp directory (via `std::env::temp_dir()`)
- **Naming**: `clawdius_prompt_<timestamp>.md`
- **Extension**: `.md` (Markdown)
- **Cleanup**: Automatic on completion or error

### Process Flow

```
1. Detect Editor (EDITOR || VISUAL)
2. Create temp file with .md extension
3. Write initial content (if provided)
4. Launch editor as subprocess
5. Wait for editor to exit
6. Read edited content
7. Remove temp file
8. Return content
```

## Troubleshooting

### Editor Not Found

```bash
# Check if editor is set
echo $EDITOR

# Set it if needed
export EDITOR=vim
```

### Editor Hangs

Some editors (like VS Code) may return immediately. The integration handles this, but if you experience issues:

```bash
# Use a blocking editor
export EDITOR=vim
```

### Permission Denied

Ensure you have write permissions to the temp directory:

```bash
# Check temp directory
ls -la /tmp

# Or use a different temp directory
export TMPDIR=~/tmp
```

## See Also

- [CLI Reference](../docs/cli.md)
- [TUI Guide](../docs/tui.md)
- [Configuration](../docs/config.md)
