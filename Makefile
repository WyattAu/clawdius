.PHONY: all build test run clean docs lint fmt fmt-check check release install dev help \
	tui test-% wasm vscode bench fuzz audit update completions \
	test-int test-integration test-llm coverage coverage-html \
	docker-build docker-run run-release docs-open check-quick pre-commit

CARGO := cargo
BINARY_NAME := clawdius
INSTALL_DIR := $(HOME)/.local/bin

all: build

build:
	$(CARGO) build --workspace

release:
	$(CARGO) build --workspace --release

test:
	$(CARGO) test --workspace

test-%:
	$(CARGO) test --workspace $*

test-int:
	$(CARGO) test --workspace --ignored

test-integration: test-int

test-llm:
	$(CARGO) test --workspace --test llm_integration -- --ignored

run:
	$(CARGO) run --package clawdius

run-release:
	$(CARGO) run --package clawdius --release

tui:
	$(CARGO) run --package clawdius -- --tui

clean:
	$(CARGO) clean
	rm -rf target/
	rm -rf .clawdius/graph/
	rm -rf .clawdius/sessions/

docs:
	$(CARGO) doc --workspace --no-deps

docs-open:
	$(CARGO) doc --workspace --no-deps --open

lint:
	$(CARGO) clippy --workspace --all-targets --all-features -- -D warnings

fmt:
	$(CARGO) fmt --all

fmt-check:
	$(CARGO) fmt --all -- --check

check: fmt-check lint test

check-quick:
	$(CARGO) check --workspace --all-targets

check-compile:
	@echo "🔍 Checking compilation..."
	$(CARGO) check --all-targets --all-features

pre-commit: check-compile fmt-check lint
	@echo "✅ All pre-commit checks passed"

install:
	$(CARGO) install --path crates/clawdius

dev:
	$(CARGO) build --workspace
	./scripts/setup-dev.sh || true

wasm:
	cd crates/clawdius-webview && $(CARGO) build --target wasm32-unknown-unknown

vscode:
	cd editors/vscode && npm install && npm run compile

bench:
	$(CARGO) bench

fuzz:
	$(CARGO) +nightly fuzz run fuzz_parser

coverage:
	$(CARGO) llvm-cov --all-features --workspace --lcov --output-path lcov.info

coverage-html:
	$(CARGO) llvm-cov --all-features --workspace --html

audit:
	cargo deny check
	cargo audit

update:
	$(CARGO) update

completions:
	./scripts/generate-completions.sh || true

docker-build:
	docker build -t $(BINARY_NAME):latest .

docker-run:
	docker run --rm -it $(BINARY_NAME):latest

mutations:
	cargo mutants --in-place

help:
	@echo "Clawdius Makefile"
	@echo ""
	@echo "Usage: make [target]"
	@echo ""
	@echo "Build targets:"
	@echo "  build          Build all crates"
	@echo "  release        Build release binaries"
	@echo "  check-quick    Quick cargo check (faster than build)"
	@echo "  clean          Clean build artifacts"
	@echo ""
	@echo "Testing targets:"
	@echo "  test           Run all tests"
	@echo "  test-<name>    Run specific test (e.g., test-parser)"
	@echo "  test-int       Run integration tests"
	@echo "  test-llm       Run LLM integration tests"
	@echo "  bench          Run benchmarks"
	@echo "  coverage       Generate test coverage report"
	@echo "  coverage-html  Generate HTML coverage report"
	@echo ""
	@echo "Code quality:"
	@echo "  lint           Run clippy"
	@echo "  fmt            Format code"
	@echo "  fmt-check      Check formatting"
	@echo "  check-compile  Check compilation only"
	@echo "  check          Full CI check (fmt-check + lint + test)"
	@echo "  pre-commit     Run all pre-commit checks"
	@echo "  audit          Security audit"
	@echo "  mutations      Run mutation tests"
	@echo ""
	@echo "Run targets:"
	@echo "  run            Run the CLI"
	@echo "  run-release    Run release binary"
	@echo "  tui            Run TUI mode"
	@echo ""
	@echo "Install:"
	@echo "  install        Install from source"
	@echo "  dev            Development setup"
	@echo ""
	@echo "Additional:"
	@echo "  wasm           Build WASM webview"
	@echo "  vscode         Build VSCode extension"
	@echo "  fuzz           Run fuzzing (requires nightly)"
	@echo "  update         Update dependencies"
	@echo "  completions    Generate shell completions"
	@echo "  docker-build   Build Docker image"
	@echo "  docker-run     Run Docker container"
	@echo "  docs           Generate documentation"
	@echo "  docs-open      Generate and open documentation"
	@echo "  help           Show this help"
