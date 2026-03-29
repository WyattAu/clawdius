# Build stage
FROM rust:1.93-bookworm AS builder

WORKDIR /app

# Copy workspace manifests
COPY Cargo.toml Cargo.lock ./
COPY crates/clawdius/Cargo.toml crates/clawdius/
COPY crates/clawdius-code/Cargo.toml crates/clawdius-code/
COPY crates/clawdius-core/Cargo.toml crates/clawdius-core/
COPY crates/clawdius-server/Cargo.toml crates/clawdius-server/
COPY crates/clawdius-webview/Cargo.toml crates/clawdius-webview/

# Create dummy source files to cache dependency compilation
RUN mkdir -p crates/clawdius/src && echo "" > crates/clawdius/src/main.rs
RUN mkdir -p crates/clawdius-code/src && echo "" > crates/clawdius-code/src/lib.rs
RUN mkdir -p crates/clawdius-core/src && echo "" > crates/clawdius-core/src/lib.rs
RUN mkdir -p crates/clawdius-server/src && echo "" > crates/clawdius-server/src/main.rs
RUN mkdir -p crates/clawdius-webview/src && echo "" > crates/clawdius-webview/src/lib.rs

# Build dependencies only (this layer is cached)
RUN cargo build --release --bin clawdius-server 2>/dev/null || true

# Copy actual source code for all workspace members
COPY crates/clawdius/README.md crates/clawdius/
COPY crates/clawdius/src/ crates/clawdius/src/
COPY crates/clawdius/benches/ crates/clawdius/benches/
COPY crates/clawdius-code/src/ crates/clawdius-code/src/
COPY crates/clawdius-core/README.md crates/clawdius-core/
COPY crates/clawdius-core/src/ crates/clawdius-core/src/
COPY crates/clawdius-core/benches/ crates/clawdius-core/benches/
COPY crates/clawdius-server/src/ crates/clawdius-server/src/
COPY crates/clawdius-webview/README.md crates/clawdius-webview/
COPY crates/clawdius-webview/src/ crates/clawdius-webview/src/

# Touch source files to invalidate the cache
RUN touch crates/clawdius/src/main.rs crates/clawdius-code/src/lib.rs crates/clawdius-core/src/lib.rs crates/clawdius-server/src/main.rs crates/clawdius-webview/src/lib.rs

# Build the actual binary
RUN cargo build --release --bin clawdius-server

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/clawdius-server /app/clawdius-server

# Create data directory for SQLite
RUN mkdir -p /app/data

# Container-friendly defaults
ENV RUST_LOG=info
ENV CLAWDIUS_JSON_LOGS=true
ENV CLAWDIUS_DB_PATH=/app/data/sessions.db

EXPOSE 8080

ENTRYPOINT ["/app/clawdius-server"]
CMD ["--host", "0.0.0.0", "--port", "8080"]
