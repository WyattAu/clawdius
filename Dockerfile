# Build stage
FROM rust:1.92-bookworm AS builder
RUN apt-get update && apt-get install -y pkg-config libssl-dev protobuf-compiler cmake && rm -rf /var/lib/apt/lists/*
WORKDIR /app

# Copy manifests first for dependency caching
COPY Cargo.toml Cargo.lock ./
COPY .cargo-vendor/ .cargo-vendor/
COPY crates/clawdius/Cargo.toml crates/clawdius/Cargo.toml
COPY crates/clawdius-core/Cargo.toml crates/clawdius-core/Cargo.toml
COPY crates/clawdius-gateway/Cargo.toml crates/clawdius-gateway/Cargo.toml
COPY crates/clawdius-mcp/Cargo.toml crates/clawdius-mcp/Cargo.toml
COPY crates/clawdius-code/Cargo.toml crates/clawdius-code/Cargo.toml

# Create dummy source files for dependency pre-building
RUN mkdir -p crates/clawdius/src && echo "fn main() {}" > crates/clawdius/src/main.rs
RUN mkdir -p crates/clawdius-core/src && echo "" > crates/clawdius-core/src/lib.rs
RUN mkdir -p crates/clawdius-gateway/src && echo "" > crates/clawdius-gateway/src/lib.rs
RUN mkdir -p crates/clawdius-mcp/src && echo "" > crates/clawdius-mcp/src/lib.rs
RUN mkdir -p crates/clawdius-code/src && echo "fn main() {}" > crates/clawdius-code/src/main.rs

# Pre-build dependencies (cached unless manifests change)
RUN cargo build --release --bin clawdius 2>/dev/null || true

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
