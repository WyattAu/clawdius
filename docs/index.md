# Clawdius Documentation

Welcome to the Clawdius documentation hub.

## Documentation

| Resource | Description |
|----------|-------------|
| [Getting Started Guide](book/) | Full mdBook documentation site with installation, configuration, and usage guides |
| [API Reference](book/src/api/rust.html) | Rust API documentation (built via `cargo doc`) |
| [Architecture Overview](book/src/concepts/architecture.html) | System design, sandboxing, and plugin architecture |
| [Contributing Guide](book/src/community/contributing.html) | How to contribute to Clawdius |

## Additional Resources

| Resource | Location |
|----------|----------|
| Getting Started (standalone) | [.docs/getting_started.md](.docs/getting_started.md) |
| Architecture Overview (standalone) | [.docs/architecture_overview.md](.docs/architecture_overview.md) |
| API Reference (standalone) | [.docs/api_reference.md](.docs/api_reference.md) |
| User Guide | [.docs/user_guide.md](.docs/user_guide.md) |
| Benchmarks | [.docs/benchmarks.md](.docs/benchmarks.md) |
| Quality Gates | [.docs/quality_gates.md](.docs/quality_gates.md) |

## Building the Documentation

The documentation site uses [mdBook](https://rust-lang.github.io/mdBook/). To build locally:

```bash
# Install mdbook
cargo install mdbook

# Build the book
mdbook build docs/book

# Serve locally
mdbook serve docs/book --open
```

The built output goes to `docs/book/book/`.
