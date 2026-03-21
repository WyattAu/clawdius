# Clawdius Deployment Guide

This directory contains deployment configurations and scripts for self-hosted Clawdius deployments.

## Quick Start

### Docker Compose (Recommended)

```bash
cd deploy
./deploy.sh docker
```

This will:
1. Build the Clawdius Docker image
2. Pull Ollama for local LLM support
3. Start all services (Clawdius + Ollama)
4. Expose Clawdius at http://localhost:3000

### Podman Compose

```bash
cd deploy
./deploy.sh podman
```

### Systemd (Linux Native)

```bash
cd deploy
sudo ./deploy.sh systemd
```

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      User Interface                          │
│  (VSCode Extension / JetBrains Plugin / CLI / REST API)     │
└───────────────────────────┬─────────────────────────────────┘
                            │
┌───────────────────────────▼─────────────────────────────────┐
│                   Clawdius Server (Port 3000)                │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │ Session Mgr  │  │ Context Mgr  │  │ Plugin System│      │
│  └──────────────┘  └──────────────┘  └──────────────┘      │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │ LLM Providers│  │ Tool Executor│  │ Audit Logger │      │
│  └──────────────┘  └──────────────┘  └──────────────┘      │
└───────────────────────────┬─────────────────────────────────┘
                            │
┌───────────────────────────▼─────────────────────────────────┐
│                    Ollama (Port 11434)                       │
│              Local LLM Inference Engine                      │
│         (DeepSeek, CodeLlama, Phi-3, Qwen, etc.)            │
└─────────────────────────────────────────────────────────────┘
```

## Configuration

### Environment Variables

Copy `.env.example` to `.env` and customize:

| Variable | Default | Description |
|----------|---------|-------------|
| `CLAWDIUS_VERSION` | latest | Docker image tag |
| `CLAWDIUS_PORT` | 3000 | Server port |
| `OLLAMA_PORT` | 11434 | Ollama API port |
| `OLLAMA_VERSION` | latest | Ollama image tag |
| `RUST_LOG` | info | Log level |
| `OPENAI_API_KEY` | - | OpenAI API key (optional) |
| `ANTHROPIC_API_KEY` | - | Anthropic API key (optional) |

### Configuration File

Edit `config.toml` for detailed configuration:

```toml
[server]
host = "0.0.0.0"
port = 3000
workers = 4

[llm]
provider = "ollama"
model = "deepseek-coder:6.7b"
```

## Deployment Options

### 1. Docker Compose (Recommended)

**Pros:**
- Easiest setup
- Includes Ollama for local LLMs
- Optional Redis caching
- Optional Prometheus/Grafana monitoring

**Usage:**
```bash
# Start all services
./deploy.sh docker

# View logs
./deploy.sh logs

# Stop services
./deploy.sh stop

# Full cleanup
./deploy.sh clean
```

### 2. Podman Compose

**Pros:**
- Rootless containers
- Better security isolation
- Daemonless architecture

**Usage:**
```bash
./deploy.sh podman
```

### 3. Systemd Service

**Pros:**
- Native Linux service
- No container overhead
- Best performance

**Usage:**
```bash
sudo ./deploy.sh systemd
sudo systemctl status clawdius
sudo journalctl -u clawdius -f
```

## GPU Support

### NVIDIA GPUs (Docker)

The docker-compose.yml automatically detects and uses NVIDIA GPUs for Ollama.

Ensure nvidia-container-toolkit is installed:
```bash
# Ubuntu/Debian
distribution=$(. /etc/os-release;echo $ID$VERSION_ID)
curl -s -L https://nvidia.github.io/nvidia-docker/gpgkey | sudo apt-key add -
curl -s -L https://nvidia.github.io/nvidia-docker/$distribution/nvidia-docker.list | \
  sudo tee /etc/apt/sources.list.d/nvidia-docker.list
sudo apt update && sudo apt install -y nvidia-container-toolkit
sudo nvidia-ctk runtime configure --runtime=docker
sudo systemctl restart docker
```

### AMD GPUs

Set environment variable:
```bash
export OLLAMA_GPU_VENDOR=amd
```

## Monitoring

### Enable Prometheus + Grafana

```bash
docker compose --profile monitoring up -d
```

Access:
- Grafana: http://localhost:3001 (admin/admin)
- Prometheus: http://localhost:9090

### Metrics Endpoint

Clawdius exposes metrics at:
```
GET http://localhost:3000/metrics
```

## Resource Management

### Memory Limits

Adjust in `.env`:
```bash
CLAWDIUS_MEMORY=4G  # Clawdius container memory
```

### CPU Limits

```bash
CLAWDIUS_CPUS=4  # Clawdius container CPUs
```

## Security

### 1. Enable Authentication

Edit `config.toml`:
```toml
[security]
enable_auth = true
jwt_secret_env = "CLAWDIUS_JWT_SECRET"
```

Set environment variable:
```bash
export CLAWDIUS_JWT_SECRET=$(openssl rand -hex 32)
```

### 2. Enable SSO (Enterprise)

```toml
[enterprise]
enable_sso = true

[enterprise.sso]
provider = "oidc"
oidc_issuer = "https://your-idp.com"
oidc_client_id = "clawdius"
oidc_client_secret_env = "OIDC_CLIENT_SECRET"
```

### 3. CORS Configuration

```toml
[security]
cors_origins = ["https://your-domain.com"]
```

## High Availability

### Multiple Instances

```bash
docker compose up -d --scale clawdius-server=3
```

### Load Balancer (Traefik)

Add to docker-compose.yml:
```yaml
labels:
  - "traefik.enable=true"
  - "traefik.http.routers.clawdius.rule=Host(`clawdius.your-domain.com`)"
```

## Backup and Recovery

### Data Backup

```bash
# Backup volumes
docker run --rm -v clawdius-data:/data -v $(pwd):/backup \
  alpine tar czf /backup/clawdius-backup-$(date +%Y%m%d).tar.gz /data

# Backup config
docker run --rm -v clawdius-config:/config -v $(pwd):/backup \
  alpine tar czf /backup/clawdius-config-$(date +%Y%m%d).tar.gz /config
```

### Disaster Recovery

```bash
# Restore from backup
docker run --rm -v clawdius-data:/data -v $(pwd):/backup \
  alpine sh -c "cd / && tar xzf /backup/clawdius-backup-YYYYMMDD.tar.gz"
```

## Troubleshooting

### Check Logs

```bash
./deploy.sh logs
```

### Health Check

```bash
curl http://localhost:3000/health
```

### Common Issues

1. **Ollama not starting**: Check GPU drivers are installed
2. **Memory errors**: Increase `CLAWDIUS_MEMORY` in `.env`
3. **Connection refused**: Check firewall allows port 3000

## Upgrading

```bash
# Pull latest images
docker compose pull

# Restart with new images
docker compose up -d
```

## Directory Structure

```
deploy/
├── docker/
│   ├── Dockerfile              # Clawdius container image
│   ├── docker-compose.yml      # Full stack deployment
│   ├── config.toml             # Default configuration
│   ├── .env.example            # Environment template
│   └── prometheus.yml          # Monitoring config
├── podman/
│   └── (symlinks to docker/)   # Podman uses same files
├── systemd/
│   └── clawdius.service        # Systemd unit file
└── deploy.sh                   # Deployment script
```

## Support

- Documentation: https://docs.clawdius.dev
- Issues: https://github.com/WyattAu/clawdius/issues
- Discord: https://discord.gg/clawdius
