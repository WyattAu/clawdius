FROM rust:1.92-bookworm
RUN apt-get update && apt-get install -y valgrind pkg-config libssl-dev protobuf-compiler cmake && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY .cargo-vendor .cargo-vendor
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
RUN cargo build --bin clawdius 2>&1 | tail -20

CMD ["valgrind", "--tool=massif", "/app/target/debug/clawdius", "--help"]
