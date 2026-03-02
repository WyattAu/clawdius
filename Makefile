.PHONY: build release test test-integration test-llm lint fmt fmt-check check clean run run-release \
	docker-build docker-run install bench coverage audit mutations help docs docs-open

CARGO := cargo
BINARY_NAME := clawdius
INSTALL_DIR := $(HOME)/.local/bin

help:
	@echo "Clawdius - High-Assurance Rust-Native Engineering Engine"
	@echo ""
	@echo "Build targets:"
	@echo "  build           - Build debug binary"
	@echo "  release         - Build release binary with optimizations"
	@echo "  check           - Quick cargo check (faster than build)"
	@echo "  clean           - Clean build artifacts"
	@echo ""
	@echo "Testing targets:"
	@echo "  test            - Run all unit tests"
	@echo "  test-int        - Run integration tests (with --ignored)"
	@echo "  test-llm        - Run LLM integration tests"
	@echo "  bench           - Run benchmarks"
	@echo "  coverage        - Generate test coverage report"
	@echo "  mutations       - Run mutation tests"
	@echo ""
	@echo "Code quality:"
	@echo "  lint            - Run clippy with pedantic lints"
	@echo "  fmt             - Format code with rustfmt"
	@echo "  fmt-check       - Check formatting (CI)"
	@echo "  audit           - Run security audit (cargo deny, cargo audit)"
	@echo ""
	@echo "Run targets:"
	@echo "  run             - Run debug binary"
	@echo "  run-release     - Run release binary"
	@echo ""
	@echo "Docker targets:"
	@echo "  docker-build    - Build Docker image"
	@echo "  docker-run      - Run Docker container"
	@echo ""
	@echo "Install:"
	@echo "  install         - Install binary to $(INSTALL_DIR)"
	@echo ""
	@echo "Documentation:"
	@echo "  docs            - Build documentation"
	@echo "  docs-open       - Build and open documentation"

build:
	$(CARGO) build

release:
	$(CARGO) build --release

check:
	$(CARGO) check --all-targets

test:
	$(CARGO) test

test-int:
	$(CARGO) test --ignored

test-integration: test-int

test-llm:
	$(CARGO) test --test llm_integration -- --ignored

bench:
	$(CARGO) bench

coverage:
	$(CARGO) llvm-cov --all-features --workspace --lcov --output-path lcov.info

coverage-html:
	$(CARGO) llvm-cov --all-features --workspace --html

lint:
	$(CARGO) clippy --all-targets --all-features -- -D warnings

fmt:
	$(CARGO) fmt

fmt-check:
	$(CARGO) fmt -- --check

clean:
	$(CARGO) clean

run:
	$(CARGO) run

run-release:
	$(CARGO) run --release

docker-build:
	docker build -t $(BINARY_NAME):latest .

docker-run:
	docker run --rm -it $(BINARY_NAME):latest

install: release
	install -m 755 target/release/$(BINARY_NAME) $(INSTALL_DIR)/

audit:
	cargo deny check
	cargo audit

mutations:
	cargo mutants --in-place

docs:
	$(CARGO) doc --no-deps

docs-open:
	$(CARGO) doc --no-deps --open
