# Build stage
FROM rust:1.96-bookworm AS builder
RUN apt-get update && apt-get install -y pkg-config libssl-dev protobuf-compiler cmake && rm -rf /var/lib/apt/lists/*
WORKDIR /app

# Copy manifests first for dependency caching
COPY Cargo.toml Cargo.lock ./
COPY .cargo-vendor/ .cargo-vendor/
# Every workspace member's manifest + the hakari workspace-hack (root-level)
# must be present or `cargo build` hard-fails resolving the workspace. This
# list previously covered 5 of 14 members and the error was swallowed by
# `|| true` — the prebuild had been silently skipping for months.
COPY crates/clawdius/Cargo.toml crates/clawdius/Cargo.toml
COPY crates/clawdius-core/Cargo.toml crates/clawdius-core/Cargo.toml
COPY crates/clawdius-gateway/Cargo.toml crates/clawdius-gateway/Cargo.toml
COPY crates/clawdius-mcp/Cargo.toml crates/clawdius-mcp/Cargo.toml
COPY crates/clawdius-code/Cargo.toml crates/clawdius-code/Cargo.toml
COPY crates/clawdius-plugin-sdk/Cargo.toml crates/clawdius-plugin-sdk/Cargo.toml
COPY crates/clawdius-lsp/Cargo.toml crates/clawdius-lsp/Cargo.toml
COPY crates/clawdius-ui/Cargo.toml crates/clawdius-ui/Cargo.toml
COPY crates/clawdius-tauri/Cargo.toml crates/clawdius-tauri/Cargo.toml
COPY crates/clawdius-web/Cargo.toml crates/clawdius-web/Cargo.toml
COPY crates/clawdius-auth/Cargo.toml crates/clawdius-auth/Cargo.toml
COPY crates/clawdius-unsafe/Cargo.toml crates/clawdius-unsafe/Cargo.toml
COPY crates/clawdius-metrics/Cargo.toml crates/clawdius-metrics/Cargo.toml
COPY workspace-hack/Cargo.toml workspace-hack/Cargo.toml

# Create dummy source files for dependency pre-building
RUN mkdir -p crates/clawdius/src && echo "fn main() {}" > crates/clawdius/src/main.rs
RUN for c in clawdius-core clawdius-gateway clawdius-mcp clawdius-code \
      clawdius-plugin-sdk clawdius-lsp clawdius-ui clawdius-tauri \
      clawdius-web clawdius-auth clawdius-unsafe clawdius-metrics; do \
      mkdir -p "crates/$c/src"; \
      echo "" > "crates/$c/src/lib.rs"; \
      echo "fn main() {}" > "crates/$c/src/main.rs"; \
    done && \
    mkdir -p workspace-hack/src && echo "" > workspace-hack/src/lib.rs && \
    # Every declared [[bench]] must exist for manifest parsing — generate
    # stubs generically so new benches never break the dummy stage.
    for m in crates/*/Cargo.toml; do \
      d=$(dirname "$m"); \
      grep -A1 '^\[\[bench\]\]' "$m" 2>/dev/null \
        | grep -oP 'name = "\K[^"]+' \
        | while read -r b; do \
            mkdir -p "$d/benches"; echo "" > "$d/benches/$b.rs"; \
          done; \
    done

# Pre-build dependencies (cached unless manifests change) — deliberately NOT
# soft-failed: a broken prebuild must fail here, not at the real build.
RUN cargo build --release --bin clawdius

# Copy actual source code
COPY crates/ crates/

# Touch source files to invalidate dummy build
RUN find crates -name "*.rs" -exec touch {} +

# Build the real binary
RUN cargo build --release --bin clawdius

# Runtime stage
FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/clawdius /usr/local/bin/clawdius
ENTRYPOINT ["clawdius"]
CMD ["chat"]
