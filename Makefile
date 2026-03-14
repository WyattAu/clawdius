.PHONY: all build test run clean docs lint fmt fmt-check check release install dev help \
	tui test-% wasm vscode bench fuzz audit update completions \
	test-int test-integration test-llm coverage coverage-html \
	docker-build docker-run run-release docs-open check-quick pre-commit \
	jetbrains vscode-extension vscode-package \
	clippy clippy-fix watch benchmark \
	install-local install-templates \
	dev-server mcp-server \
	test-core test-cli test-integration-single \
	check-deps security-audit \
	help

# ==============================================================================
# Configuration
# ==============================================================================

CARGO := cargo
BINARY_NAME := clawdius
INSTALL_DIR := $(HOME)/.local/bin
VERSION := $(shell grep '^version =' Cargo.toml | head -1 | sed 's/.*= "//' | sed 's/"//')

# ==============================================================================
# Core Build Targets
# ==============================================================================

all: build

build:
	@echo "🔨 Building workspace..."
	$(CARGO) build --workspace

release:
	@echo "🚀 Building release..."
	$(CARGO) build --workspace --release

check-quick:
	@echo "🔍 Quick check..."
	$(CARGO) check --workspace --all-targets

check-compile:
	@echo "🔍 Checking compilation..."
	$(CARGO) check --all-targets --all-features

# ==============================================================================
# Testing Targets
# ==============================================================================

test:
	@echo "🧪 Running all tests..."
	$(CARGO) test --workspace

test-core:
	@echo "🧪 Running core library tests..."
	$(CARGO) test -p clawdius-core

test-cli:
	@echo "🧪 Running CLI tests..."
	$(CARGO) test -p clawdius

test-%:
	$(CARGO) test --workspace $*

test-int:
	@echo "🧪 Running integration tests..."
	$(CARGO) test --workspace --ignored

test-integration: test-int

test-integration-single:
	@echo "🧪 Running integration tests (single thread)..."
	$(CARGO) test -p clawdius --test integration_tests -- --test-threads=1

test-llm:
	@echo "🧪 Running LLM integration tests..."
	$(CARGO) test --workspace --test llm_integration -- --ignored

coverage:
	@echo "📊 Generating coverage report..."
	$(CARGO) llvm-cov --all-features --workspace --lcov --output-path lcov.info

coverage-html:
	@echo "📊 Generating HTML coverage report..."
	$(CARGO) llvm-cov --all-features --workspace --html

# ==============================================================================
# Code Quality Targets
# ==============================================================================

lint:
	@echo "🔎 Running clippy..."
	$(CARGO) clippy --workspace --all-targets --all-features -- -D warnings

clippy: lint

clippy-fix:
	@echo "🔧 Auto-fixing clippy warnings..."
	$(CARGO) clippy --workspace --all-targets --all-features --fix --allow-dirty

fmt:
	@echo "🎨 Formatting code..."
	$(CARGO) fmt --all

fmt-check:
	@echo "🎨 Checking formatting..."
	$(CARGO) fmt --all -- --check

check: fmt-check lint test

pre-commit: check-compile fmt-check lint
	@echo "✅ All pre-commit checks passed"

security-audit:
	@echo "🔒 Running security audit..."
	cargo deny check
	$(CARGO) audit

check-deps:
	@echo "📦 Checking dependencies..."
	$(CARGO) tree --duplicates

# ==============================================================================
# Run Targets
# ==============================================================================

run:
	@echo "🚀 Running CLI (debug)..."
	$(CARGO) run --package clawdius

run-release:
	@echo "🚀 Running CLI (release)..."
	$(CARGO) run --package clawdius --release

tui:
	@echo "🖥️ Running TUI mode..."
	$(CARGO) run --package clawdius -- --tui

watch:
	@echo "👁️ Running in watch mode..."
	$(CARGO) run --package clawdius -- watch .

dev-server:
	@echo "🌐 Starting development server..."
	$(CARGO) run --package clawdius -- --serve

mcp-server:
	@echo "🔌 Starting MCP server..."
	$(CARGO) run --package clawdius -- mcp serve

# ==============================================================================
# Benchmarking
# ==============================================================================

bench:
	@echo "⚡ Running benchmarks..."
	$(CARGO) bench --workspace

benchmark: bench

# ==============================================================================
# Documentation
# ==============================================================================

docs:
	@echo "📚 Generating documentation..."
	$(CARGO) doc --workspace --no-deps

docs-open:
	@echo "📚 Opening documentation..."
	$(CARGO) doc --workspace --no-deps --open

# ==============================================================================
# Installation
# ==============================================================================

install:
	@echo "📦 Installing to $(INSTALL_DIR)..."
	$(CARGO) install --path crates/clawdius

install-local: install

install-templates:
	@echo "📋 Installing templates..."
	@mkdir -p $(HOME)/.clawdius/templates
	@cp -r templates/* $(HOME)/.clawdius/templates/

# ==============================================================================
# Docker Targets
# ==============================================================================

docker-build:
	@echo "🐳 Building Docker image..."
	docker build -t clawdius:$(VERSION) .

docker-run:
	@echo "🐳 Running Docker container..."
	docker run -it --rm -v $(PWD):/workspace clawdius:$(VERSION)

# ==============================================================================
# VSCode Extension
# ==============================================================================

vscode-extension:
	@echo "🔌 Building VSCode extension..."
	cd editors/vscode && pnpm install && pnpm run compile

vscode-package:
	@echo "📦 Packaging VSCode extension..."
	cd editors/vscode && pnpm install && pnpm run compile && pnpm exec vsce package --allow-missing-repository

vscode-install: vscode-package
	@echo "📦 Installing VSCode extension..."
	@code --install-extension editors/vscode/*.vsix

# ==============================================================================
# JetBrains Plugin
# ==============================================================================

jetbrains:
	@echo "🔌 Building JetBrains plugin..."
	cd plugins/jetbrains/clawdius-plugin && ./gradlew buildPlugin

jetbrains-test:
	@echo "🧪 Testing JetBrains plugin..."
	cd plugins/jetbrains/clawdius-plugin && ./gradlew test

jetbrains-run:
	@echo "🚀 Running JetBrains plugin in IDE..."
	cd plugins/jetbrains/clawdius-plugin && ./gradlew runIde

# ==============================================================================
# WASM Target
# ==============================================================================

wasm:
	@echo "🌐 Building WASM target..."
	cd crates/clawdius-webview && $(CARGO) build --target wasm32-unknown-unknown

wasm-release:
	@echo "🌐 Building WASM (release)..."
	cd crates/clawdius-webview && $(CARGO) build --target wasm32-unknown-unknown --release

# ==============================================================================
# Cleanup
# ==============================================================================

clean:
	@echo "🧹 Cleaning build artifacts..."
	$(CARGO) clean
	rm -rf target/
	rm -rf .clawdius/graph/
	rm -rf .clawdius/sessions/
	rm -rf lcov.info
	rm -rf lcov.info-*.json

clean-deep: clean
	@echo "🧹 Deep clean..."
	rm -rf ~/.cache/cargo/
	rm -rf ~/.cargo/registry/cache/

# ==============================================================================
# Development Setup
# ==============================================================================

dev:
	@echo "🔧 Setting up development environment..."
	$(CARGO) build --workspace
	./scripts/setup-dev.sh || true

update:
	@echo "📦 Updating dependencies..."
	$(CARGO) update

completions:
	@echo "📝 Generating shell completions..."
	$(CARGO) run --package clawdius -- completions bash > /tmp/clawdius.bash
	$(CARGO) run --package clawdius -- completions zsh > /tmp/clawdius.zsh
	$(CARGO) run --package clawdius -- completions fish > /tmp/clawdius.fish
	@echo "✅ Completions generated in /tmp/"

# ==============================================================================
# Fuzzing (requires nightly)
# ==============================================================================

fuzz:
	@echo "🎲 Running fuzzer..."
	$(CARGO) +nightly fuzz run fuzz_parser

# ==============================================================================
# Help
# ==============================================================================

help:
	@echo "Clawdius Build System"
	@echo "====================="
	@echo ""
	@echo "Core Build Commands:"
	@echo "  make build          - Build workspace (debug)"
	@echo "  make release        - Build workspace (release)"
	@echo "  make check-quick    - Quick compilation check"
	@echo "  make clean          - Clean build artifacts"
	@echo ""
	@echo "Testing Commands:"
	@echo "  make test           - Run all tests"
	@echo "  make test-core      - Run core library tests"
	@echo "  make test-cli       - Run CLI tests"
	@echo "  make test-int       - Run integration tests"
	@echo "  make coverage       - Generate coverage report"
	@echo "  make bench          - Run benchmarks"
	@echo ""
	@echo "Code Quality:"
	@echo "  make fmt            - Format code"
	@echo "  make fmt-check      - Check formatting"
	@echo "  make lint           - Run clippy"
	@echo "  make check          - Full quality check"
	@echo "  make pre-commit     - Pre-commit checks"
	@echo ""
	@echo "Run Commands:"
	@echo "  make run            - Run CLI (debug)"
	@echo "  make run-release    - Run CLI (release)"
	@echo "  make tui            - Run TUI mode"
	@echo "  make watch          - Run watch mode"
	@echo "  make dev-server     - Start dev server"
	@echo "  make mcp-server     - Start MCP server"
	@echo ""
	@echo "Plugin/Extension Commands:"
	@echo "  make vscode-extension - Build VSCode extension"
	@echo "  make vscode-package   - Package VSCode extension"
	@echo "  make jetbrains        - Build JetBrains plugin"
	@echo "  make jetbrains-test   - Test JetBrains plugin"
	@echo ""
	@echo "Installation:"
	@echo "  make install        - Install to ~/.local/bin"
	@echo ""
	@echo "Other:"
	@echo "  make docs           - Generate documentation"
	@echo "  make docker-build   - Build Docker image"
	@echo "  make help           - Show this help"
