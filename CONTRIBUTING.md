# Contributing to Clawdius

Thank you for your interest in contributing to Clawdius! This document provides guidelines and instructions for contributing.

---

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Development Setup](#development-setup)
- [Project Structure](#project-structure)
- [Code Style Guidelines](#code-style-guidelines)
- [Testing Requirements](#testing-requirements)
- [Pull Request Process](#pull-request-process)
- [Commit Guidelines](#commit-guidelines)

---

## Code of Conduct

This project follows the [Rust Code of Conduct](https://www.rust-lang.org/policies/code-of-conduct). By participating, you are expected to uphold this code. Please report unacceptable behavior to the maintainers.

---

## Development Setup

### Prerequisites

| Tool | Version | Purpose |
|------|---------|---------|
| Rust | 1.85+ | Core language |
| Cargo | Included with Rust | Build system |
| pnpm | Latest | VSCode extension |
| git | Latest | Version control |

### Initial Setup

```bash
# 1. Fork and clone the repository
git clone https://github.com/YOUR_USERNAME/clawdius.git
cd clawdius

# 2. Install Rust toolchain
rustup toolchain install stable
rustup default stable

# 3. Add required components
rustup component add clippy rustfmt rust-src

# 4. Build all crates
cargo build

# 5. Run tests
cargo test

# 6. (Optional) Setup VSCode extension
cd editors/vscode
pnpm install
cd ../..
```

### IDE Setup

**Recommended:** VSCode with rust-analyzer

```json
// .vscode/settings.json
{
  "rust-analyzer.cargo.features": "all",
  "rust-analyzer.checkOnSave.command": "clippy",
  "editor.formatOnSave": true
}
```

---

## Project Structure

### Monorepo Layout

```
clawdius/
├── crates/
│   ├── clawdius/              # CLI application
│   ├── clawdius-core/         # Core library
│   ├── clawdius-code/         # VSCode helper
│   └── clawdius-webview/      # WASM webview
├── editors/
│   └── vscode/                # VSCode extension
├── .docs/                     # Documentation
├── tests/                     # Integration tests
└── benches/                   # Benchmarks
```

### Crate Responsibilities

| Crate | Purpose | Dependencies |
|-------|---------|--------------|
| `clawdius` | CLI binary, TUI | clawdius-core |
| `clawdius-core` | LLM, sessions, tools, sandboxing | External libs only |
| `clawdius-code` | JSON-RPC for VSCode | clawdius-core |
| `clawdius-webview` | Browser UI | clawdius-core |

---

## Code Style Guidelines

### Rust Code

We follow the standard Rust style guidelines with additional constraints:

#### Linting Rules

Our workspace enforces strict linting (see `Cargo.toml`):

```toml
[workspace.lints.rust]
unsafe_code = "deny"
missing_docs = "warn"

[workspace.lints.clippy]
all = "deny"
pedantic = "warn"
unwrap_used = "allow"
expect_used = "allow"
```

#### Documentation

- **All public APIs must have documentation comments**
- Use `///` for doc comments (not `//`)
- Include examples in doc comments when applicable

```rust
/// Performs a semantic search on the codebase.
///
/// # Arguments
///
/// * `query` - The search query string
/// * `limit` - Maximum number of results to return
///
/// # Returns
///
/// A vector of search results ranked by relevance.
///
/// # Example
///
/// ```
/// let results = graph_rag.semantic_search("error handling", 10)?;
/// ```
pub fn semantic_search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
    // ...
}
```

#### Error Handling

- Use `thiserror` for library errors
- Use `anyhow` for application errors
- Never panic in library code
- Use `Result<T, E>` for fallible operations

```rust
// Good: Library error type
#[derive(Debug, thiserror::Error)]
pub enum GraphRagError {
    #[error("Database connection failed: {0}")]
    DatabaseError(#[from] rusqlite::Error),
    
    #[error("Vector search failed: {0}")]
    VectorSearchError(String),
}

// Good: Application error handling
use anyhow::Context;

fn load_config() -> Result<Config> {
    let content = std::fs::read_to_string("config.toml")
        .context("Failed to read config file")?;
    toml::from_str(&content)
        .context("Failed to parse config")
}
```

#### Code Organization

- One module per file
- Group related functionality in submodules
- Keep public API minimal
- Use `#[cfg(test)]` for unit tests in same file

```rust
// src/graph_rag/mod.rs
mod ast_index;
mod vector_store;
mod search;

pub use search::{SearchResult, SemanticSearch};
```

### TypeScript Code (VSCode Extension)

- Use strict mode
- Follow ESLint configuration
- Use async/await over promises
- Document public functions

```typescript
/**
 * Sends a request to the Clawdius JSON-RPC server.
 * @param method - The RPC method name
 * @param params - Method parameters
 * @returns Promise resolving to the result
 */
export async function sendRequest<T>(
  method: string,
  params: unknown
): Promise<T> {
  // ...
}
```

---

## Testing Requirements

### Unit Tests

All new code must include unit tests:

```bash
# Run all tests
cargo test

# Run tests for specific crate
cargo test -p clawdius-core

# Run specific test
cargo test test_semantic_search

# Run tests with output
cargo test -- --nocapture

# Run tests with coverage
cargo tarpaulin
```

### Test Guidelines

1. **Coverage:** Aim for >80% coverage on new code
2. **Naming:** Use `test_<function>_<scenario>` pattern
3. **Arrange-Act-Assert:** Structure tests clearly
4. **Property testing:** Use `proptest` for complex logic

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_semantic_search_returns_relevant_results() {
        // Arrange
        let graph_rag = GraphRag::new_test_instance();
        graph_rag.index("fn hello() { println!(\"hello\"); }");
        
        // Act
        let results = graph_rag.semantic_search("hello function", 10)
            .unwrap();
        
        // Assert
        assert!(!results.is_empty());
        assert!(results[0].score > 0.8);
    }
    
    #[test]
    fn test_semantic_search_handles_empty_query() {
        let graph_rag = GraphRag::new_test_instance();
        let result = graph_rag.semantic_search("", 10);
        
        assert!(result.is_err());
    }
}
```

### Integration Tests

Place integration tests in `tests/` directory:

```rust
// tests/integration_test.rs
use clawdius_core::GraphRag;

#[test]
fn test_full_workflow() {
    let temp_dir = tempfile::tempdir().unwrap();
    let graph_rag = GraphRag::new(temp_dir.path()).unwrap();
    
    graph_rag.index_project("src/").unwrap();
    let results = graph_rag.semantic_search("main function", 5).unwrap();
    
    assert!(!results.is_empty());
}
```

### Benchmarks

Benchmarks are located in `benches/` directories:

```bash
# Run all benchmarks
cargo bench

# Run benchmarks for specific crate
cargo bench -p clawdius-core

# Run specific benchmark
cargo bench --bench llm_benchmark

# Save baseline for comparison
cargo bench -- --save-baseline main

# Compare against baseline
cargo bench -- --baseline main
```

#### Available Benchmarks

| Benchmark | Location | Description |
|-----------|----------|-------------|
| `llm_benchmark` | `crates/clawdius-core/benches/` | LLM provider performance |
| `tools_benchmark` | `crates/clawdius-core/benches/` | Tool execution performance |
| `session_benchmark` | `crates/clawdius-core/benches/` | Session management performance |
| `core_bench` | `crates/clawdius-core/benches/` | Core operations |
| `cli_bench` | `crates/clawdius/benches/` | CLI operations |

#### Writing Benchmarks

Add benchmarks for performance-critical code:

```rust
// benches/core_bench.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use clawdius_core::llm::{LlmConfig, create_provider};

fn bench_llm_chat(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let config = LlmConfig::from_env("anthropic").unwrap();
    let provider = create_provider(&config).unwrap();
    
    c.bench_function("llm_chat", |b| {
        b.to_async(&rt).iter(|| async {
            provider.chat(vec![]).await
        })
    });
}

criterion_group!(benches, bench_llm_chat);
criterion_main!(benches);
```

#### Benchmark Guidelines

1. Use `black_box()` to prevent compiler optimizations
2. Include setup/teardown in separate measurements
3. Test with realistic data sizes
4. Compare against baselines for regressions

---

## Pull Request Process

### Before Submitting

1. **Update from main:**
   ```bash
   git fetch origin
   git rebase origin/main
   ```

2. **Run all checks:**
   ```bash
   cargo fmt --check
   cargo clippy --all-targets --all-features
   cargo test --all
   cargo build --release
   ```

3. **Run feature-specific tests:**
   ```bash
   # Test with keyring feature
   cargo test --features keyring
   
   # Test all features
   cargo test --all-features
   ```

4. **Update documentation** if needed

5. **Add changelog entry** in `CHANGELOG.md` (if exists)

### PR Checklist

- [ ] Code compiles without warnings
- [ ] All tests pass
- [ ] New code has documentation
- [ ] New code has tests
- [ ] Clippy passes
- [ ] Format is correct
- [ ] CHANGELOG updated (if applicable)

### PR Template

```markdown
## Description
Brief description of changes

## Type of Change
- [ ] Bug fix
- [ ] New feature
- [ ] Breaking change
- [ ] Documentation update

## Testing
Describe tests added/modified

## Checklist
- [ ] Tests pass
- [ ] Documentation updated
- [ ] Clippy clean
```

### Review Process

1. Submit PR
2. Automated CI checks run
3. At least one maintainer review required
4. Address review feedback
5. Squash and merge when approved

---

## Commit Guidelines

### Commit Message Format

```
<type>(<scope>): <subject>

<body>

<footer>
```

### Types

| Type | Description |
|------|-------------|
| `feat` | New feature |
| `fix` | Bug fix |
| `docs` | Documentation only |
| `style` | Formatting, no code change |
| `refactor` | Code refactoring |
| `test` | Adding tests |
| `chore` | Maintenance tasks |

### Examples

```bash
# Feature
feat(graph-rag): add semantic search caching

# Bug fix
fix(sandbox): correct bubblewrap permission flags

# Documentation
docs(README): update installation instructions

# Breaking change
feat(api)!: change GraphRag::new signature

BREAKING CHANGE: GraphRag::new now requires a PathBuf
instead of &Path
```

---

## Development Workflow

### Branch Naming

- `feat/feature-name` - New features
- `fix/bug-name` - Bug fixes
- `docs/topic` - Documentation
- `refactor/component` - Refactoring

### Example Workflow

```bash
# 1. Create feature branch
git checkout -b feat/semantic-caching

# 2. Make changes and commit
git add .
git commit -m "feat(graph-rag): add semantic search caching"

# 3. Push to fork
git push origin feat/semantic-caching

# 4. Create PR on GitHub

# 5. After approval, squash merge
```

---

## Feature Flags

### Available Features

| Feature | Crates | Description |
|---------|--------|-------------|
| `keyring` | clawdius, clawdius-core | System keyring for secure API key storage |
| `hft-mode` | clawdius, clawdius-core | High-frequency trading optimizations |
| `broker-mode` | clawdius | Financial trading features |

### Building with Features

```bash
# Build with keyring support
cargo build --features keyring

# Test with specific features
cargo test --features keyring

# Build all features
cargo build --all-features

# Test all features
cargo test --all-features
```

---

## Getting Help

### Community Channels

| Channel | Purpose | Link |
|---------|---------|------|
| **GitHub Discussions** | Q&A, ideas, showcases | [discussions](https://github.com/clawdius/clawdius/discussions) |
| **Discord** | Real-time chat, community | [discord.gg/clawdius](https://discord.gg/clawdius) |
| **GitHub Issues** | Bug reports, feature requests | [issues](https://github.com/clawdius/clawdius/issues) |
| **Email** | Private/security matters | maintainers@clawdius.dev |

### Discord Channels

| Channel | Purpose |
|---------|---------|
| `#general` | General discussion |
| `#help` | Get help with Clawdius |
| `#development` | Development discussions |
| `#showcase` | Share your projects |
| `#announcements` | Project updates |

### GitHub Discussions Categories

| Category | Use For |
|----------|---------|
| **Q&A** | Questions about using Clawdius |
| **Ideas** | Feature suggestions and discussions |
| **Show and Tell** | Share projects built with Clawdius |
| **General** | Other discussions |

### Documentation

- **API Reference:** `.docs/api/` directory
- **Architecture:** `.docs/architecture/` directory
- **Competitor Analysis:** `.docs/competitor-analysis/` directory
- **Examples:** `.docs/examples/` directory

### Reporting Issues

When reporting issues, please include:

1. **Environment:**
   ```bash
   clawdius --version
   rustc --version
   cargo --version
   uname -a  # or system info on Windows
   ```

2. **Steps to reproduce**

3. **Expected vs actual behavior**

4. **Logs** (with `RUST_LOG=debug` if relevant)

5. **Minimal reproduction** (if possible)

---

## License

By contributing, you agree that your contributions will be licensed under the Apache 2.0 License.

Thank you for contributing to Clawdius!
