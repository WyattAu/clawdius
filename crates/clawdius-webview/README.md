# Clawdius WebView

WASM-based webview UI for the VSCode extension built with Leptos.

## Components

### Chat Component (`components/chat.rs`)
- Main chat interface
- Message history display
- Input handling
- VSCode extension communication

### Message Component (`components/message.rs`)
- Message display with role-based styling
- Timestamp display
- Support for User, Assistant, and System messages

### Input Component (`components/input.rs`)
- Text input field
- Send button
- Enter key handling
- Loading state management

### Sidebar Component (`components/sidebar.rs`)
- Navigation between Chat, History, and Settings
- Active tab highlighting

## Features

- **Real-time Communication**: Bidirectional messaging with VSCode extension
- **Theme Support**: VSCode theme integration with dark/light mode
- **Responsive Design**: Flexible layout adapting to webview size
- **Message Styling**: Role-based message appearance

## Requirements

- Rust 1.70+ with `wasm32-unknown-unknown` target
- `wasm-bindgen` CLI tool
- `trunk` build tool (optional, for development)

## Building

### Install WASM Target

```bash
rustup target add wasm32-unknown-unknown
```

### Build WASM

```bash
# Using cargo
cargo build --target wasm32-unknown-unknown --release

# Generate bindings
wasm-bindgen target/wasm32-unknown-unknown/release/clawdius_webview.wasm \
    --out-dir ../../dist/webview \
    --target web \
    --no-typescript

# Or use the build script
./build.sh
```

### Using Trunk (Development)

```bash
# Install trunk
cargo install trunk

# Serve with hot reload
trunk serve
```

## VSCode Integration

The webview communicates with the VSCode extension using the VSCode API:

```javascript
// Acquire VSCode API
const vscode = acquireVsCodeApi();

// Send message to extension
vscode.postMessage({
    type: 'query',
    data: { query: 'user message' }
});

// Listen for messages from extension
window.addEventListener('message', event => {
    const message = event.data;
    if (message.type === 'response') {
        // Handle response
    }
});
```

## Architecture

```
┌─────────────────┐
│   VSCode        │
│   Extension     │
└────────┬────────┘
         │ postMessage
         ▼
┌─────────────────┐
│   WebView       │
│   (WASM)        │
│                 │
│  ┌───────────┐  │
│  │  Sidebar  │  │
│  └───────────┘  │
│  ┌───────────┐  │
│  │   Chat    │  │
│  │   View    │  │
│  └───────────┘  │
└─────────────────┘
```

## Styling

The webview uses CSS custom properties to integrate with VSCode's theming:

- `--vscode-editor-background`: Main background
- `--vscode-foreground`: Text color
- `--vscode-button-background`: Accent color
- etc.

See `styles.css` for complete styling.

## Known Issues

### Build Environment

If you encounter "can't find crate for `std`" when building for WASM:

1. Ensure you're using the correct Rust toolchain (not a Nix-provided one)
2. Verify the WASM target is installed: `rustup target list | grep wasm32`
3. Try using the official rustup installation instead of system packages

### Development

For local development without VSCode:
1. Mock the VSCode API in a test HTML file
2. Use trunk for hot-reload development
3. Test communication with a mock extension

## License

Same as parent Clawdius project
