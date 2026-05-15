# Streaming

Clawdius supports real-time streaming of LLM responses for immediate feedback.

## Overview

Instead of waiting for the full response, streaming delivers tokens as they are generated. This provides a responsive experience, especially for long-running responses.

## Stream JSON Mode

Use the `stream-json` output format for real-time processing:

```bash
clawdius chat -f stream-json
```

Each line in the output is a self-contained JSON object containing a token or status update. This is suitable for piping to other tools or processing in real time.

## Output Formats

| Format | Flag | Description |
|--------|------|-------------|
| Text | `text` | Standard formatted output (default) |
| JSON | `json` | Complete JSON response after processing |
| Stream JSON | `stream-json` | Real-time JSON token stream |

## Usage Examples

### Standard Streaming

```bash
clawdius chat "explain this code"
# Response streams token by token in the TUI
```

### JSON Output

```bash
clawdius chat "explain this code" -f json
# Complete response as JSON after generation finishes
```

### Stream JSON for Scripts

```bash
clawdius chat "analyze code" -f stream-json | jq '.content'
# Process tokens in real time
```

### Programmatic Consumption

```bash
# Pipe to a file
clawdius chat "generate docs" -f json -o response.json

# Watch mode for continuous output
clawdius chat "monitor" -f stream-json
```

## Performance

Streaming uses zero-copy patterns for efficient data transfer:

- Tokens are emitted as soon as they arrive from the LLM
- No buffering delay between the provider and the terminal
- The TUI renders at 60 FPS using ratatui

## Headless Streaming

In headless mode (no TUI), streaming output goes to stdout:

```bash
clawdius --no-tui
# Type a message, see tokens stream to stdout
```

## Metrics Output

Metrics also support streaming/watch mode:

```bash
clawdius metrics --watch
clawdius metrics -f json --watch
```
