# Build stage
FROM rust:1.93 AS builder

WORKDIR /app

# Copy workspace manifests
COPY Cargo.toml Cargo.lock ./
COPY crates/clawdius/Cargo.toml crates/clawdius/
COPY crates/clawdius-code/Cargo.toml crates/clawdius-code/
COPY crates/clawdius-core/Cargo.toml crates/clawdius-core/
COPY crates/clawdius-mcp/Cargo.toml crates/clawdius-mcp/

# Create dummy source files to cache dependency compilation
RUN mkdir -p crates/clawdius/src && echo "" > crates/clawdius/src/main.rs
RUN mkdir -p crates/clawdius-code/src && echo "" > crates/clawdius-code/src/lib.rs
RUN mkdir -p crates/clawdius-core/src && echo "" > crates/clawdius-core/src/lib.rs
RUN mkdir -p crates/clawdius-mcp/src && echo "" > crates/clawdius-mcp/src/lib.rs

# Build dependencies only (this layer is cached)
RUN cargo build --release -p clawdius 2>/dev/null || true

# Copy actual source code for all workspace members
COPY crates/clawdius/src/ crates/clawdius/src/
COPY crates/clawdius-code/src/ crates/clawdius-code/src/
COPY crates/clawdius-core/src/ crates/clawdius-core/src/
COPY crates/clawdius-mcp/src/ crates/clawdius-mcp/src/

# Touch source files to invalidate the cache
RUN touch crates/clawdius/src/main.rs crates/clawdius-code/src/lib.rs crates/clawdius-core/src/lib.rs crates/clawdius-mcp/src/lib.rs

# Build the actual binary
RUN cargo build --release -p clawdius

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/clawdius /usr/local/bin/clawdius

ENV RUST_LOG=info

EXPOSE 8080

ENTRYPOINT ["/usr/local/bin/clawdius"]
CMD ["generate", "--help"]
