# Clawdius VSCode Extension

VSCode extension for the Clawdius AI coding engine.

## Prerequisites

- [Clawdius](https://github.com/WyattAu/clawdius) built from source (v1.0.0+)
- `clawdius-code` binary available in `$PATH`

## Install from Source

```bash
cd extensions/clawdius
npm install
npm run compile
```

Then in VSCode:

1. Open the `extensions/clawdius` folder
2. Press F5 to launch Extension Development Host
3. Or package with `npx vsce package` and install the `.vsix`

## Commands

| Command | Description |
|---------|-------------|
| `Clawdius: Start Server` | Start the clawdius-code JSON-RPC server |
| `Clawdius: Stop Server` | Stop the server |
| `Clawdius: Open Chat` | Send a message to the LLM |
| `Clawdius: Execute Sprint` | Run an agentic sprint task |
| `Clawdius: Analyze Code` | Run drift and debt analysis |
| `Clawdius: Verify Proofs` | Run Lean4 proof verification |
| `Clawdius: Create Checkpoint` | Create a file checkpoint |

## Configuration

| Setting | Default | Description |
|---------|---------|-------------|
| `clawdius.binaryPath` | `clawdius-code` | Path to clawdius-code binary |
| `clawdius.provider` | `anthropic` | Default LLM provider |
| `clawdius.sandboxTier` | `filtered` | Sandbox isolation level |
| `clawdius.autoStart` | `false` | Auto-start server on workspace open |

## Architecture

```
VSCode Extension (TypeScript)
    |
    | JSON-RPC over stdio
    v
clawdius-code binary (Rust)
    |
    | clawdius-core library
    v
LLM Providers / Sandbox / Sessions / Tools
```

The extension is a thin shim. All logic runs in the Rust binary.

## License

Apache 2.0
