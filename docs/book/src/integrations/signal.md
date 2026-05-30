# Signal Adapter

## Overview

Connects Clawdius to Signal via a running signal-cli daemon with its REST
API enabled. Handles text message sending and receiving, group messages,
and attachment downloads.

This adapter requires a local signal-cli instance as an intermediary --
Clawdius does not connect to the Signal servers directly.

## Configuration

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `SIGNAL_ACCOUNT_NUMBER` | Yes | -- | Phone number registered with signal-cli |
| `SIGNAL_REST_URL` | No | `http://localhost:7583` | Base URL of the signal-cli REST API |

The account number is read from `api_token`. The REST URL is read from
`settings.rest_url`.

## Setup

1. Install and configure [signal-cli](https://github.com/AsamK/signal-cli):
   ```bash
   # Debian/Ubuntu
   sudo apt install signal-cli

   # Or via AUR on Arch, Homebrew on macOS, etc.
   ```
2. Register your phone number with signal-cli:
   ```bash
   signal-cli -u +1234567890 register
   ```
3. Complete verification with the SMS or voice code:
   ```bash
   signal-cli -u +1234567890 verify 123-456
   ```
4. Start the REST API daemon:
   ```bash
   signal-cli -u +1234567890 daemon --socket localhost:7583
   ```
5. Configure:
   ```toml
   [gateways.signal]
   api_token = "+1234567890"
   enabled = true

   [gateways.signal.settings]
   rest_url = "http://localhost:7583"
   ```

## Features

- Text message sending and receiving
- Group message support
- Attachment download
- Automatic retry on network errors

## Limitations

- **Signal does NOT support message editing.** When the gateway attempts
  to edit a message (e.g., during streaming), the adapter sends a new
  message with an `(edited) ` prefix instead.
- Rate limited by signal-cli's internal throttling
- Reaction support is planned but not yet implemented
- Requires a running signal-cli daemon; it is not embedded in Clawdius

## Troubleshooting

**"SIGNAL_ACCOUNT_NUMBER not set"**

Set `api_token` under `[gateways.signal]` to your registered phone number
(e.g., `+1234567890`).

**Connection refused to signal-cli**

Ensure the signal-cli daemon is running:
```bash
signal-cli -u +1234567890 daemon --socket localhost:7583
```

**Signal API errors with 4xx status**

Check that the account number matches the registered signal-cli account.
Verify signal-cli is properly registered and not in an error state.

**Streaming responses arrive as multiple messages**

This is expected behavior. Since Signal lacks message editing, streaming
edits are sent as individual messages. The adapter prefixes edits with
`(edited) ` to indicate updates.
