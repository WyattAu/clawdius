# Clawdius Server Docker Image
# Multi-stage build for optimized production image

# Build stage
FROM rust:1.88-slim AS builder

WORKDIR /build

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY crates/clawdius-core ./crates/clawdius-core
COPY crates/clawdius ./crates/clawdius
COPY crates/clawdius-code ./crates/clawdius-code
COPY crates/clawdius-server ./crates/clawdius-server
COPY crates/clawdius-webview ./crates/clawdius-webview
COPY src ./src

# Build release binary
RUN cargo build --release -p clawdius --bin clawdius

# Runtime stage
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -r -s /bin/false clawdius

# Copy binary from builder
COPY --from=builder /build/target/release/clawdius /usr/local/bin/clawdius

# Create directories for data and config
RUN mkdir -p /app/data /app/config /app/logs \
    && chown -R clawdius:clawdius /app

# Copy default configuration
COPY deploy/docker/config.toml /app/config/config.toml

# Switch to non-root user
USER clawdius

# Expose default port
EXPOSE 3000

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:3000/health || exit 1

# Default configuration file path
ENV CLAWDIUS_CONFIG=/app/config/config.toml
ENV CLAWDIUS_DATA_DIR=/app/data
ENV CLAWDIUS_LOG_DIR=/app/logs

# Run the server
ENTRYPOINT ["clawdius"]
CMD ["server", "--host", "0.0.0.0", "--port", "3000"]
