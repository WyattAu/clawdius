# Docker Build Report

**Date:** 2026-05-14
**Tag:** `clawdius:1.0.0-rc.1`
**Builder:** Docker legacy builder (BuildKit unavailable)

## Build Result: SUCCESS

| Metric | Value |
|--------|-------|
| Build status | Passed |
| Image size | **164 MB** |
| Version output | `clawdius 1.0.0-rc.1` |
| Build time | ~12 min (first build, no cache) |

## Files Created/Modified

- `Dockerfile` — Multi-stage build (rust:1.93-bookworm -> debian:bookworm-slim)
- `.dockerignore` — Excludes target/, .git/, .specs/, .lake/, .reports/, .cargo-vendor/, node_modules/, openclaw/, ironclaw/, picoclaw/, paperclip/, lean4/, testbed/, fuzz/

## Build Details

- **Build stage:** Installs `pkg-config`, `libssl-dev`, `protobuf-compiler`, `cmake`
- **Dependencies:** Vendored via `vendor/` directory (offline build, no network needed in build stage)
- **Patch:** `.cargo-vendor/half/` copied for `[patch.crates-io]` lint suppression
- **Runtime stage:** `debian:bookworm-slim` with `ca-certificates` and `libssl3` only

## Issues Encountered & Resolved

1. **Stale `.cargo-vendor/`** — Missing `.cargo-checksum.json` for `half` crate. Resolved by regenerating vendor dir with `cargo vendor` into `vendor/`.
2. **`[patch.crates-io]` path** — `Cargo.toml` patches `half` from `.cargo-vendor/half/`. Dockerfile copies this directory explicitly.
3. **`echo \n` not producing newlines** — Shell `echo` in Dockerfile doesn't interpret `\n`. Fixed with `printf`.
4. **Build timeout** — First compile takes ~12 min due to full workspace build. Subsequent builds will be faster with layer caching.

## Multi-Arch Recommendations (buildx)

The current setup builds for the host architecture only (`linux/amd64`). For cross-platform support:

```bash
# Install buildx
docker buildx install

# Create multi-arch builder
docker buildx create --name multiarch --use

# Build for multiple platforms
docker buildx build --platform linux/amd64,linux/arm64 \
  -t clawdius:1.0.0-rc.1 \
  --push \
  .

# Or build and load for local testing (single platform)
docker buildx build --platform linux/amd64 \
  -t clawdius:1.0.0-rc.1 \
  --load \
  .
```

**Key considerations for multi-arch:**
- Rust cross-compilation requires `rustup target add` for each target (e.g., `aarch64-unknown-linux-gnu`)
- Cross-compilation needs additional tools: `gcc-aarch64-linux-gnu`, `linker` config in `.cargo/config.toml`
- Consider using `cross` (https://github.com/cross-rs/cross) for simpler cross-compilation
- The vendored dependencies approach works across architectures
- CI/CD integration: GitHub Actions with `docker/build-push-action` supports multi-platform natively
