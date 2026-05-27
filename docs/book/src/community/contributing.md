# Contributing

Contributions to Clawdius are welcome. This guide covers the development workflow.

## Getting Started

### Prerequisites

- Rust 1.92+ (see `rust-version` in `Cargo.toml`)
- A C compiler (for `bundled` SQLite)
- Git

### Building from Source

```bash
git clone https://github.com/WyattAu/clawdius
cd clawdius
cargo build --release
```

### Running Tests

```bash
# All tests
cargo test --all

# Specific crate
cargo test -p clawdius-core

# With verbose output
cargo test --all -- --nocapture
```

### Development Commands

```bash
# Check all crates compile
cargo check --all

# Run clippy (warnings allowed in dev)
cargo clippy --all-targets --all-features

# Format code
cargo fmt --all

# Check formatting
cargo fmt --all -- --check

# Run benchmarks
cargo bench --all
```

## Code Style

The project enforces strict linting rules:

| Lint | Level |
|------|-------|
| `unsafe_code` | deny |
| `missing_docs` | warn |
| `pedantic` | warn |
| `unwrap_used` | warn |
| `expect_used` | warn |
| `panic` | warn |
| `todo` | warn |

CI runs with stricter flags (`-D warnings`).

## Commit Messages

Follow Conventional Commits format:

```
type(scope): description

feat(core): add retry logic for rate limits
fix(cli): handle missing session ID gracefully
docs(api): update error codes reference
refactor(sandbox): extract sandbox config into struct
```

## Pull Request Process

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Run `cargo test --all` and `cargo clippy --all-targets --all-features`
5. Run `cargo fmt --all`
6. Open a pull request

## Project Structure

```
clawdius/
├── crates/
│   ├── clawdius/          # CLI binary
│   ├── clawdius-core/     # Core library
│   ├── clawdius-code/     # VSCode helper
│   ├── clawdius-gateway/  # HTTP gateway
│   └── clawdius-mcp/      # MCP server
├── editors/vscode/        # VSCode extension
├── docs/book/src/         # mdBook documentation
├── .docs/                 # Internal docs
└── Cargo.toml             # Workspace root
```

## Adding a New Crate

```bash
mkdir crates/clawdius-newfeature
cd crates/clawdius-newfeature
cargo init --lib
```

Then add to workspace `Cargo.toml`:

```toml
members = [
    # ...existing...
    "crates/clawdius-newfeature",
]
```

## Feature Flags

When adding new functionality, consider whether it should be behind a feature flag. See `crates/clawdius-core/Cargo.toml` for existing patterns.

## Reporting Issues

Use the GitHub issue tracker at https://github.com/WyattAu/clawdius/issues.

Include:

- Rust version (`rustc --version`)
- Clawdius version (`clawdius --version`)
- Operating system
- Steps to reproduce
- Expected vs actual behavior
- Relevant log output (`RUST_LOG=clawdius=debug`)
