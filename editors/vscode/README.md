# Clawdius VSCode Extension

Visual Studio Code extension for Clawdius - The High-Assurance Engineering Engine.

---

## Overview

This extension provides deep integration between VSCode and Clawdius, enabling:

- **Inline Chat:** AI assistance directly in your editor
- **Code Generation:** Generate code from natural language
- **Refactoring:** Intelligent cross-language refactoring
- **Documentation:** Auto-generate documentation
- **Graph-RAG Integration:** Context-aware suggestions

---

## Installation

### From VSIX

```bash
# Build the extension
cd editors/vscode
pnpm install
pnpm run compile
vsce package

# Install in VSCode
code --install-extension clawdius-0.2.0.vsix
```

### From Source

```bash
# Clone monorepo
git clone https://github.com/clawdius/clawdius
cd clawdius

# Build VSCode extension
cd editors/vscode
pnpm install
pnpm run compile

# Open in development mode
code .
# Press F5 to launch Extension Development Host
```

### Prerequisites

1. **Clawdius Binary:** Install the CLI tool first
   ```bash
   cargo install clawdius
   ```

2. **VSCode:** Version 1.85 or higher

3. **Node.js:** Version 18 or higher (for pnpm)

---

## Configuration

### Extension Settings

Open VSCode settings (`Ctrl+,` / `Cmd+,`) and search for "Clawdius":

```json
{
  "clawdius.binaryPath": "clawdius",
  "clawdius.provider": "openai",
  "clawdius.model": "gpt-4",
  "clawdius.enableTelemetry": true,
  "clawdius.autoIndex": true,
  "clawdius.indexIgnore": [
    "**/node_modules/**",
    "**/target/**",
    "**/.git/**"
  ]
}
```

### Settings Reference

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `clawdius.binaryPath` | string | `"clawdius"` | Path to clawdius binary |
| `clawdius.provider` | enum | `"openai"` | LLM provider (openai, anthropic, deepseek, ollama) |
| `clawdius.model` | string | `"gpt-4"` | Model to use |
| `clawdius.enableTelemetry` | boolean | `true` | Enable usage telemetry |
| `clawdius.autoIndex` | boolean | `true` | Auto-index project on open |
| `clawdius.indexIgnore` | array | `[...]` | Glob patterns to ignore during indexing |

### API Keys

Configure API keys in your environment:

```bash
# Add to ~/.bashrc or ~/.zshrc
export OPENAI_API_KEY="sk-..."
export ANTHROPIC_API_KEY="sk-ant-..."
export DEEPSEEK_API_KEY="..."
```

Or use VSCode's integrated terminal to set them temporarily.

---

## Commands

All commands are accessible via the Command Palette (`Ctrl+Shift+P` / `Cmd+Shift+P`):

### `Clawdius: Initialize Project`

Initialize Clawdius in the current workspace.

**Shortcut:** None

**Usage:**
1. Open Command Palette
2. Type "Clawdius: Initialize Project"
3. Press Enter

**Creates:**
- `.clawdius/` directory structure
- Default SOPs
- Configuration file

---

### `Clawdius: Open Chat`

Open the Clawdius chat panel.

**Shortcut:** `Ctrl+Shift+C` / `Cmd+Shift+C`

**Features:**
- Context-aware responses
- Code suggestions
- Project-specific knowledge

**Usage:**
1. Open Command Palette or use shortcut
2. Type your question in the chat panel
3. View AI response with code suggestions
4. Click "Insert" to add code to your file

---

### `Clawdius: Generate Code`

Generate code from natural language description.

**Shortcut:** `Ctrl+Shift+G` / `Cmd+Shift+G`

**Usage:**
1. Select text or place cursor where you want code
2. Open Command Palette or use shortcut
3. Enter description: "Create a function that validates email addresses"
4. Review generated code
5. Accept or reject changes

**Example:**
```typescript
// Select this comment and run "Generate Code"
// Create a function that fetches user data from an API with error handling

// Generated code:
async function fetchUserData(userId: string): Promise<User> {
  try {
    const response = await fetch(`/api/users/${userId}`);
    if (!response.ok) {
      throw new Error(`HTTP error! status: ${response.status}`);
    }
    return await response.json();
  } catch (error) {
    console.error('Failed to fetch user data:', error);
    throw error;
  }
}
```

---

### `Clawdius: Refactor Code`

Intelligent code refactoring.

**Shortcut:** `Ctrl+Shift+R` / `Cmd+Shift+R`

**Options:**
- Extract function
- Extract variable
- Inline variable
- Convert to async/await
- Cross-language migration

**Usage:**
1. Select code to refactor
2. Open Command Palette or use shortcut
3. Choose refactoring type
4. Preview changes
5. Apply or cancel

---

### `Clawdius: Generate Documentation`

Auto-generate documentation for functions and classes.

**Shortcut:** `Ctrl+Shift+D` / `Cmd+Shift+D`

**Usage:**
1. Place cursor on function/class
2. Open Command Palette or use shortcut
3. Review generated documentation
4. Accept or edit

**Example:**
```rust
// Before:
fn calculate_interest(principal: f64, rate: f64, years: u32) -> f64 {
    principal * (1.0 + rate).powi(years as i32)
}

// After:
/// Calculates compound interest over a given number of years.
///
/// # Arguments
///
/// * `principal` - The initial investment amount
/// * `rate` - The annual interest rate (as a decimal, e.g., 0.05 for 5%)
/// * `years` - The number of years to compound
///
/// # Returns
///
/// The total value after compounding
///
/// # Example
///
/// ```
/// let total = calculate_interest(1000.0, 0.05, 10);
/// assert!(total > 1000.0);
/// ```
fn calculate_interest(principal: f64, rate: f64, years: u32) -> f64 {
    principal * (1.0 + rate).powi(years as i32)
}
```

---

### `Clawdius: Explain Code`

Get an explanation of selected code.

**Shortcut:** `Ctrl+Shift+E` / `Cmd+Shift+E`

**Usage:**
1. Select code to explain
2. Open Command Palette or use shortcut
3. View explanation in output panel

---

### `Clawdius: Find Bugs`

Analyze code for potential bugs.

**Shortcut:** None

**Usage:**
1. Select code or open file
2. Open Command Palette
3. Type "Clawdius: Find Bugs"
4. Review findings in Problems panel

---

### `Clawdius: Index Project`

Manually trigger project indexing for Graph-RAG.

**Shortcut:** None

**Usage:**
1. Open Command Palette
2. Type "Clawdius: Index Project"
3. Wait for indexing to complete (progress shown in status bar)

---

### `Clawdius: Show Status`

Show current project status and phase.

**Shortcut:** None

**Output:**
```
Project: my-project
Phase: 6.5 (Documentation Verification)
Graph-RAG: 234 files indexed
Last indexed: 2 minutes ago
```

---

## Features

### Inline Chat

Chat with Clawdius directly in your editor:

1. Type `// ask ` followed by your question
2. Press Enter
3. Clawdius responds inline

**Example:**
```rust
// ask How do I handle errors in Rust?

// Clawdius responds:
// Use Result<T, E> for recoverable errors and panic! for unrecoverable errors.
// Example:
fn divide(a: f64, b: f64) -> Result<f64, String> {
    if b == 0.0 {
        Err("Division by zero".to_string())
    } else {
        Ok(a / b)
    }
}
```

### Code Actions

Quick fixes and suggestions appear in the lightbulb menu:

- **Generate implementation:** Create function body from signature
- **Add documentation:** Generate doc comments
- **Fix error:** Suggest fixes for compiler errors
- **Optimize imports:** Remove unused imports

### Status Bar

The status bar shows:

- Current project phase
- Graph-RAG status (indexed files count)
- LLM provider and model
- Active session indicator

### Terminal Integration

Use Clawdius in the integrated terminal:

```bash
# Commands work directly
clawdius chat
clawdius refactor --from ts --to rust src/
```

---

## Architecture

### Extension Structure

```
editors/vscode/
├── src/
│   ├── extension.ts        # Extension entry point
│   ├── client.ts           # JSON-RPC client
│   ├── providers/
│   │   ├── chat.ts         # Chat panel provider
│   │   ├── completion.ts   # Code completion provider
│   │   └── codeAction.ts   # Code actions provider
│   ├── commands/
│   │   ├── init.ts         # Initialize command
│   │   ├── chat.ts         # Chat commands
│   │   ├── refactor.ts     # Refactoring commands
│   │   └── generate.ts     # Code generation
│   └── utils/
│       ├── config.ts       # Configuration helpers
│       └── logger.ts       # Logging utilities
├── package.json            # Extension manifest
├── tsconfig.json           # TypeScript config
└── webview/                # Webview UI components
```

### Communication Flow

```
VSCode Extension (TypeScript)
         │
         │ JSON-RPC
         ▼
clawdius-code Binary (Rust)
         │
         │ Direct calls
         ▼
clawdius-core Library
         │
         ├─► LLM APIs
         ├─► Graph-RAG
         └─► Sandboxing
```

---

## Troubleshooting

### Extension Not Activating

**Cause:** Clawdius binary not found

**Solution:**
1. Ensure `clawdius` is installed: `which clawdius`
2. Check `clawdius.binaryPath` setting
3. Restart VSCode

### "Failed to connect to Clawdius server"

**Cause:** JSON-RPC server not running

**Solution:**
1. Check Output panel → "Clawdius"
2. Try manual start: `clawdius-code`
3. Check logs for errors

### Chat Not Responding

**Cause:** Invalid API key or network issue

**Solution:**
1. Verify API key: `echo $OPENAI_API_KEY`
2. Test connection:
   ```bash
   curl https://api.openai.com/v1/models \
     -H "Authorization: Bearer $OPENAI_API_KEY"
   ```
3. Check network connectivity

### Indexing Slow or Stuck

**Cause:** Large codebase or insufficient resources

**Solution:**
1. Add patterns to `clawdius.indexIgnore`
2. Reduce indexed files
3. Check available memory

### Performance Issues

**Cause:** High CPU/memory usage

**Solution:**
1. Disable `clawdius.autoIndex`
2. Reduce indexing frequency
3. Exclude large directories

---

## Development

### Build Extension

```bash
cd editors/vscode
pnpm install
pnpm run compile
```

### Watch Mode

```bash
pnpm run watch
```

### Run Tests

```bash
pnpm run test
```

### Package Extension

```bash
vsce package
```

### Debug Extension

1. Open `editors/vscode/` in VSCode
2. Press `F5` to launch Extension Development Host
3. Set breakpoints in TypeScript code
4. Use Debug Console for output

---

## Keyboard Shortcuts

| Command | Linux/Windows | macOS |
|---------|---------------|-------|
| Open Chat | `Ctrl+Shift+C` | `Cmd+Shift+C` |
| Generate Code | `Ctrl+Shift+G` | `Cmd+Shift+G` |
| Refactor Code | `Ctrl+Shift+R` | `Cmd+Shift+R` |
| Generate Docs | `Ctrl+Shift+D` | `Cmd+Shift+D` |
| Explain Code | `Ctrl+Shift+E` | `Cmd+Shift+E` |

Customize in: File → Preferences → Keyboard Shortcuts

---

## Contributing

See [CONTRIBUTING.md](../../CONTRIBUTING.md) for guidelines.

### Extension-Specific Guidelines

- Follow TypeScript best practices
- Use VSCode API conventions
- Add tests for new features
- Update package.json for new commands

---

## Known Issues

1. **Large files:** Performance may degrade with files >1MB
2. **Network latency:** First request may be slow due to cold start
3. **Memory usage:** Indexing large projects may consume significant memory

---

## Roadmap

- [ ] Inline diff view for code suggestions
- [ ] Multi-file refactoring
- [ ] Test generation
- [ ] Code review integration
- [ ] Custom SOP editor
- [ ] Graph-RAG visualization

---

## Support

- **Documentation:** [User Guide](../../.docs/user_guide.md)
- **Issues:** https://github.com/clawdius/clawdius/issues
- **Discord:** https://discord.gg/clawdius

---

## License

Apache 2.0 - See [LICENSE](../../LICENSE) for details.
