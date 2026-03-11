# Quality Gates

This document describes the quality gates implemented in the Clawdius project to prevent compilation errors and maintain code quality.

## Overview

Quality gates are automated checks that run at different stages of development to catch issues early. Clawdius implements quality gates at three levels:

1. **Pre-commit hooks** - Run locally before each commit
2. **Make targets** - Manual quality checks
3. **CI/CD pipeline** - Comprehensive automated checks on every push/PR

## Quality Checks

### 1. Compilation Check

**What it does:** Verifies that all code compiles successfully across all targets and features.

**Command:** 
```bash
cargo check --all-targets --all-features
```

**When it runs:**
- Pre-commit hook (every commit)
- CI pipeline (before tests)
- Manual: `make check-compile`

**Why it matters:** Catches type errors, missing imports, syntax issues, and other compilation failures before they reach the repository.

### 2. Formatting Check

**What it does:** Ensures code follows Rust formatting conventions.

**Command:**
```bash
cargo fmt --all -- --check
```

**When it runs:**
- Pre-commit hook (every commit)
- CI lint job (every push/PR)
- Manual: `make fmt-check`

**How to fix:**
```bash
cargo fmt --all
```

**Why it matters:** Maintains consistent code style across the codebase, improving readability and reducing merge conflicts.

### 3. Clippy Linting

**What it does:** Runs the Clippy linter to catch common mistakes, improve code quality, and enforce best practices.

**Command:**
```bash
# Pre-commit (warnings allowed)
cargo clippy --all-targets --all-features

# CI (strict mode - all warnings are errors)
cargo clippy --all-targets --all-features -- \
  -D warnings \
  -D clippy::all \
  -D clippy::pedantic \
  -D clippy::unwrap_used \
  -D clippy::expect_used \
  -D clippy::panic \
  -D clippy::todo
```

**When it runs:**
- Pre-commit hook (errors only, warnings allowed)
- CI lint job (strict - all warnings are errors)
- Manual: `make lint`

**Why it matters:** Catches potential bugs, performance issues, and code that doesn't follow Rust idioms.

## Pre-commit Hook

### Installation

The pre-commit hook is automatically created at `.git/hooks/pre-commit`. It runs automatically before every commit.

### What It Checks

1. **Compilation** - All code must compile
2. **Formatting** - Code must be properly formatted
3. **Clippy** - No clippy errors (warnings allowed)

### How to Bypass (Emergency Only)

⚠️ **WARNING:** Only bypass quality gates in emergencies. Bypassing can introduce broken code.

```bash
# Method 1: Environment variable
SKIP_PRE_COMMIT=1 git commit -m "emergency fix"

# Method 2: Git flag (bypasses all hooks)
git commit --no-verify -m "emergency fix"
```

### Troubleshooting

**Issue:** Hook is too slow
- **Solution:** The hook uses `--quiet` flags to minimize output. Compilation is cached by Cargo, so subsequent runs are faster.

**Issue:** Hook fails on formatting
- **Solution:** Run `cargo fmt --all` and commit the changes.

**Issue:** Hook fails on clippy warnings
- **Solution:** Pre-commit allows warnings, only errors fail. Fix errors or use `SKIP_PRE_COMMIT=1` temporarily.

## CI/CD Pipeline

The GitHub Actions workflow (`.github/workflows/ci.yml`) runs comprehensive quality checks:

### Lint Job
- Formatting check
- Clippy (strict mode)
- Dependency checking (cargo-deny)
- Unused dependency check (cargo-machete)

### Test Job
- Compilation check (before tests)
- Test build
- Test execution (sharded across 4 parallel jobs)

### Additional Jobs
- Coverage reporting (≥85% required)
- Security scanning
- Fuzz testing
- Mutation testing (PRs only)
- Build verification (multiple platforms)

### Quality Gate

The pipeline includes a final "Quality Gate" job that verifies all checks passed:
- ✅ Lint
- ✅ Tests
- ✅ Coverage
- ✅ VSCode Extension

If any check fails, the entire pipeline fails.

## Make Targets

### Quick Checks

```bash
# Check compilation only (fastest)
make check-compile

# Check compilation with quick mode
make check-quick

# Run all pre-commit checks manually
make pre-commit
```

### Full Checks

```bash
# Full CI check (formatting + clippy + tests)
make check

# Formatting check only
make fmt-check

# Auto-format code
make fmt

# Clippy only
make lint
```

### Development Workflow

```bash
# 1. Make your changes
vim src/main.rs

# 2. Format code
make fmt

# 3. Run quick checks
make check-compile

# 4. Run full checks
make check

# 5. Commit (hook runs automatically)
git commit -m "feat: add new feature"
```

## Common Issues and Fixes

### Compilation Errors

**Symptom:** `cargo check` fails

**Common causes:**
- Type mismatches
- Missing imports
- Syntax errors
- Missing trait implementations

**Fix:**
```bash
# Run cargo check to see errors
cargo check --all-targets --all-features

# Fix errors in your editor
# Re-run check
cargo check --all-targets --all-features
```

### Formatting Issues

**Symptom:** `cargo fmt -- --check` fails

**Fix:**
```bash
# Auto-format all code
cargo fmt --all

# Commit formatting changes
git add .
git commit -m "style: format code"
```

### Clippy Warnings

**Symptom:** Clippy reports warnings or errors

**Common warnings:**
- `unwrap_used` - Use `.ok_or()` or `?` operator
- `expect_used` - Use `.ok_or()` with descriptive error
- `panic` - Return `Result` instead
- `todo` - Implement or use `unimplemented!()`

**Fix:**
```bash
# Run clippy to see warnings
cargo clippy --all-targets --all-features

# Fix warnings following Clippy suggestions
# Re-run clippy
cargo clippy --all-targets --all-features
```

### Pre-commit Hook Not Running

**Symptom:** Commits succeed without checks

**Possible causes:**
- Hook not executable: `chmod +x .git/hooks/pre-commit`
- Hook missing: Re-create the hook file
- Bypassed: Check for `SKIP_PRE_COMMIT=1` in environment

## Best Practices

### 1. Run Checks Locally First

Before pushing, run:
```bash
make pre-commit
```

This catches issues early and avoids CI failures.

### 2. Fix Issues Immediately

When quality gates fail, fix them immediately rather than bypassing. Technical debt accumulates quickly.

### 3. Keep CI Green

Maintain a green CI pipeline. If CI fails, prioritize fixing it over new features.

### 4. Use `make check-compile` Frequently

During development, run `make check-compile` often to catch compilation errors early. It's faster than full builds.

### 5. Format Code Regularly

Run `cargo fmt` frequently during development, not just before commits.

## Performance Optimization

### Caching

Cargo caches compilation artifacts, so subsequent checks are faster:
- First run: ~30-60 seconds
- Subsequent runs: ~5-15 seconds

### Incremental Builds

The project uses incremental compilation (`CARGO_INCREMENTAL=1` in CI) for faster builds.

### Parallel Jobs

CI runs tests in 4 parallel shards to reduce total pipeline time.

## Dependencies

The quality gates require these tools:

### Required (installed via rustup)
- `rustc` - Rust compiler
- `cargo` - Package manager
- `rustfmt` - Code formatter
- `clippy` - Linter

### Optional (installed via cargo install)
- `cargo-deny` - Dependency checker
- `cargo-machete` - Unused dependency finder
- `cargo-nextest` - Test runner
- `cargo-llvm-cov` - Coverage tool

All required tools are installed automatically by the pre-commit hook or CI pipeline.

## Additional Resources

- [Rust formatting guidelines](https://rust-lang.github.io/rustfmt/)
- [Clippy lints](https://rust-lang.github.io/rust-clippy/master/)
- [Cargo book](https://doc.rust-lang.org/cargo/)
- [GitHub Actions workflow](../.github/workflows/ci.yml)

## Contributing

When contributing to Clawdius:

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Run `make pre-commit` locally
5. Push and create a PR
6. Wait for CI to pass
7. Request review

The quality gates ensure your contribution maintains code quality standards.

---

**Note:** Quality gates are here to help, not hinder. They catch issues early when they're cheapest to fix. If you find a gate too restrictive, open an issue to discuss adjustments rather than bypassing it.
