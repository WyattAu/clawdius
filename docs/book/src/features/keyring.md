# Keyring Storage

Clawdius can securely store API keys in your operating system's native keyring, keeping them out of config files and environment variables.

## Overview

The keyring feature uses the `keyring` crate to interface with platform-specific secret storage:

| Platform | Backend |
|----------|---------|
| Linux | libsecret / Secret Service (GNOME Keyring, KDE Wallet) |
| macOS | Keychain |
| Windows | Credential Manager |

## Enabling Keyring

The `keyring` feature must be enabled at compile time:

```bash
cargo install clawdius --features keyring
```

Or when building from source:

```bash
cargo build --features keyring
```

## CLI Commands

### Store an API Key

```bash
clawdius auth set anthropic
# Prompts securely for the key (input is hidden)
```

### Retrieve a Stored Key

```bash
clawdius auth get anthropic
# Shows first 8 characters: sk-ant-a...
```

### Delete a Stored Key

```bash
clawdius auth delete anthropic
```

### Supported Providers

| Provider | Key Name |
|----------|----------|
| `anthropic` | Anthropic API key |
| `openai` | OpenAI API key |
| `zai` | Z.AI API key |

## API Key Priority

When Clawdius resolves an API key, it checks sources in order:

1. **Environment variable** (e.g., `ANTHROPIC_API_KEY`)
2. **System keyring** (via `clawdius auth set`)
3. **Config file** `api_key` field (least secure)

If a key is found in an earlier source, later sources are not checked.

## Programmatic API

```rust
use clawdius_core::config::KeyringStorage;

let storage = KeyringStorage::global();

// Store
storage.set_api_key("anthropic", "sk-ant-...")?;

// Retrieve
if let Some(key) = storage.get_api_key("anthropic")? {
    println!("Key found: {}***", &key[..8]);
}

// Delete
storage.delete_api_key("anthropic")?;
```

The `KeyringStorage::global()` method returns a lazily-initialized singleton, so you can call it from anywhere without managing lifetimes.

## Security Notes

- Keys are stored in the OS keyring, which is encrypted at rest
- Keys are never written to log files or traces
- Keys are never exposed to sandboxed processes or WASM modules
- The `clawdius auth get` command masks the key, showing only the prefix

## Troubleshooting

**"Failed to access keyring"**

On Linux, ensure a secret service is running:

```bash
# GNOME
gnome-keyring-daemon --start

# KDE
# Wallet should auto-start; check kwalletmanager5
```

On headless Linux servers without a desktop environment, you may need:

```bash
# Install and configure gnome-keyring
sudo apt install gnome-keyring
```

**Feature not available**

Ensure the `keyring` feature is compiled in:

```bash
clawdius auth set anthropic
# If this fails with "unknown command", rebuild with --features keyring
```
