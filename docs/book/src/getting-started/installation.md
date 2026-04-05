# Installation

Clawdius can be installed in several ways depending on your platform and preferences.

## Quick Install

### Linux and macOS

```bash
curl -fsSL https://clawdius.dev/install.sh | sh
```

### Windows (PowerShell)

```powershell
irm https://clawdius.dev/install.ps1 | iex
```

## Package Managers

### Homebrew (macOS/Linux)

```bash
brew tap clawdius/tap
brew install clawdius
```

### Cargo (All Platforms)

```bash
cargo install clawdius
```

### Arch Linux (AUR)

```bash
yay -S clawdius-bin
# Or build from source:
yay -S clawdius-git
```

### Nix

```nix
# Using nixpkgs
nix-env -iA nixpkgs.clawdius

# Using flake
nix profile install github:WyattAu/clawdius
```

## Pre-built Binaries

Download pre-built binaries from [GitHub Releases](https://github.com/WyattAu/clawdius/releases):

| Platform | Architecture | Download |
|----------|--------------|----------|
| Linux | x86_64 | `clawdius-linux-x86_64.tar.gz` |
| Linux | aarch64 | `clawdius-linux-aarch64.tar.gz` |
| macOS | x86_64 | `clawdius-darwin-x86_64.tar.gz` |
| macOS | aarch64 | `clawdius-darwin-aarch64.tar.gz` |
| Windows | x86_64 | `clawdius-windows-x86_64.zip` |

### Manual Installation

```bash
# Download and extract
curl -LO https://github.com/WyattAu/clawdius/releases/download/v1.0.0/clawdius-linux-x86_64.tar.gz
tar xzf clawdius-linux-x86_64.tar.gz

# Move to PATH
sudo mv clawdius /usr/local/bin/

# Verify installation
clawdius --version
```

## Build from Source

### Prerequisites

- **Rust 1.75+** (recommended: latest stable)
- **C compiler** (gcc, clang, or MSVC)
- **pkg-config** (Linux)

### Build Steps

```bash
# Clone the repository
git clone https://github.com/WyattAu/clawdius.git
cd clawdius

# Build release binary
cargo build --release

# Install locally
cargo install --path crates/clawdius

# Or copy binary manually
cp target/release/clawdius /usr/local/bin/
```

### Build Features

Clawdius supports optional features:

```bash
# Enable all features
cargo build --release --all-features

# Enable only enterprise features
cargo build --release --features enterprise

# Minimal build (no plugins, basic sandbox)
cargo build --release --no-default-features
```

| Feature | Description |
|---------|-------------|
| `default` | Core features, WASM sandbox, basic tools |
| `enterprise` | SSO, audit logging, compliance |
| `plugins` | Plugin system with WASM runtime |
| `all-sandboxes` | All available sandbox backends |
| `self-hosted` | Self-hosted LLM support |

## Docker

```bash
# Pull official image
docker pull clawdius/clawdius:latest

# Run with current directory mounted
docker run -it --rm \
  -v $(pwd):/workspace \
  -v ~/.config/clawdius:/root/.config/clawdius \
  clawdius/clawdius:latest chat
```

### Docker Compose

```yaml
version: '3.8'
services:
  clawdius:
    image: clawdius/clawdius:latest
    volumes:
      - ./:/workspace
      - clawdius-config:/root/.config/clawdius
    environment:
      - ANTHROPIC_API_KEY=${ANTHROPIC_API_KEY}
volumes:
  clawdius-config:
```

## Verification

After installation, verify everything works:

```bash
# Check version
clawdius --version

# Run the interactive setup wizard (recommended for first-time users)
clawdius setup

# Or run diagnostics
clawdius doctor

# Quick test (requires API key)
clawdius chat --message "Hello, Clawdius!"
```

## First-Time Setup

New in v1.2.0: Use the interactive setup wizard to configure Clawdius:

```bash
clawdius setup
```

The wizard will guide you through:
1. **Provider Selection** - Choose your LLM provider (Anthropic, OpenAI, Ollama, Zhipu AI)
2. **API Key Configuration** - Enter and securely store your API key
3. **Settings Preset** - Choose a configuration preset:
   - **Balanced**: Good defaults for most users
   - **Security**: Maximum sandboxing and audit logging
   - **Performance**: Optimized for speed
   - **Development**: Verbose logging and debugging features

### Quick Setup Options

```bash
# Skip welcome screen
clawdius setup --quick

# Pre-select provider
clawdius setup --provider anthropic
clawdius setup --provider ollama  # For local LLMs
```

## Shell Completion

Enable shell completion for your shell:

### Bash

```bash
clawdius completion bash > /etc/bash_completion.d/clawdius
# Or for user-only:
clawdius completion bash > ~/.local/share/bash-completion/completions/clawdius
```

### Zsh

```bash
clawdius completion zsh > "${fpath[1]}/_clawdius"
```

### Fish

```bash
clawdius completion fish > ~/.config/fish/completions/clawdius.fish
```

### PowerShell

```powershell
clawdius completion powershell | Out-String | Invoke-Expression
```

## Next Steps

- [Configuration](./configuration.md) - Set up your API keys and preferences
- [First Chat](./first-chat.md) - Your first conversation with Clawdius
