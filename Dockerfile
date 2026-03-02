FROM rust:1.85-slim AS builder

WORKDIR /app

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -r -s /bin/false clawdius

WORKDIR /app

COPY --from=builder /app/target/release/clawdius /usr/local/bin/clawdius

RUN chmod +x /usr/local/bin/clawdius

USER clawdius

ENV CLAWDIUS_HOME=/app/.clawdius

VOLUME ["/app/.clawdius"]

ENTRYPOINT ["clawdius"]
CMD ["--help"]
