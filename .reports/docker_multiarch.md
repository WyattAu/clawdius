# Multi-Arch Docker Build Setup

## Buildx Availability

| Item | Status |
|------|--------|
| Docker version | 29.4.3 |
| docker buildx | **Not installed** (`docker: unknown command: docker buildx`) |

To enable multi-arch builds, install buildx:
```bash
# Option A: Install buildx plugin (recommended)
mkdir -p ~/.docker/cli-plugins
curl -sSL https://github.com/docker/buildx/releases/latest/download/buildx-linux-amd64 \
  -o ~/.docker/cli-plugins/docker-buildx
chmod +x ~/.docker/cli-plugins/docker-buildx

# Option B: Use a buildx-enabled builder (e.g., in CI)
docker buildx create --use --name multiarch-builder
```

## Bake File Configuration (`docker-bake.hcl`)

Created at project root. Defines:
- **Group `default`**: builds the `clawdius` target
- **Target `clawdius`**:
  - Dockerfile: `Dockerfile`
  - Tags: `ghcr.io/clawdius/clawdius:1.0.0-rc.1`
  - Platforms: `linux/amd64`, `linux/arm64`
  - GitHub Actions cache: `type=gha`, mode `max`

Usage:
```bash
# Multi-arch build (requires buildx)
docker buildx bake

# Single-platform build (works without buildx)
docker build -t clawdius:1.0.0-rc.1 .

# Cross-compile single platform
docker buildx build --platform linux/arm64 -t ghcr.io/clawdius/clawdius:1.0.0-rc.1-arm64 .
```

## Docker Compose Configuration

Existing `docker-compose.yml` already provides a full development stack:
- **postgres**: PostgreSQL 17 (port 5432)
- **redis**: Redis 8 (port 6379)
- **gateway**: Messaging gateway with `Dockerfile.gateway` (port 8080)
- **cli**: Interactive CLI shell with `Dockerfile.cli`

No changes needed to docker-compose.yml.

## Multi-Arch Build Instructions for CI

### GitHub Actions

```yaml
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: docker/setup-buildx-action@v3
      - uses: docker/login-action@v2
        with:
          registry: ghcr.io
          username: ${{ github.actor }}
          password: ${{ secrets.GITHUB_TOKEN }}
      - uses: docker/bake-action@v5
        with:
          files: docker-bake.hcl
          push: true
```

### Gitea / Generic CI

```bash
# Set up buildx builder
docker buildx create --use --driver docker-container --name ci-builder

# Build and push multi-arch
docker buildx bake --push
```

## Image Manifest Strategy

| Tag | Platforms | Purpose |
|-----|-----------|---------|
| `ghcr.io/clawdius/clawdius:1.0.0-rc.1` | amd64, arm64 | Multi-arch manifest (default) |
| `ghcr.io/clawdius/clawdius:latest` | amd64, arm64 | Rolling release (add to bake file) |
| `ghcr.io/clawdius/clawdius:1.0.0-rc.1-amd64` | amd64 only | Debugging / pinned |
| `ghcr.io/clawdius/clawdius:1.0.0-rc.1-arm64` | arm64 only | Debugging / pinned |

The multi-arch manifest is automatically created by buildx when multiple platforms are specified in the bake file. Users pulling the image will receive the correct architecture automatically.

## Build Verification

**Status: BLOCKED** — the `.dockerignore` excludes `.cargo-vendor/` which the Dockerfile requires at line 9 (`COPY .cargo-vendor/half/ .cargo-vendor/half/`). This must be fixed before any build can succeed:

```dockerignore
# Remove this line from .dockerignore:
.cargo-vendor/
```

## Files Created/Modified

| File | Action |
|------|--------|
| `docker-bake.hcl` | **Created** |
| `docker-compose.yml` | Unchanged (already sufficient) |
| `.dockerignore` | Needs fix (remove `.cargo-vendor/` exclusion) |
