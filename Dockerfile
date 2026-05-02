# Stage 1: Build
FROM rust:1.83-bookworm AS builder
WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config libssl-dev protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

# Copy manifests
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY .cargo-vendor/ .cargo-vendor/

# Create .cargo/config.toml for vendored deps
RUN mkdir -p .cargo && echo '[source.crates-io]\nreplace-with = "vendored-sources"\n\n[source.vendored-sources]\ndirectory = ".cargo-vendor"' > .cargo/config.toml

# Build release binaries
RUN cargo build --release -p clawdius -p clawdius-gateway 2>&1 | tail -5

# Stage 2: Runtime
FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y \
    ca-certificates libssl3 \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -r -s /bin/false clawdius

COPY --from=builder /app/target/release/clawdius /usr/local/bin/
COPY --from=builder /app/target/release/clawdius-gateway /usr/local/bin/

USER clawdius
WORKDIR /home/clawdius

EXPOSE 8080 8081

ENTRYPOINT ["clawdius"]
CMD ["serve", "--host", "0.0.0.0", "--port", "8080"]
