#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────
# Clawdius Air-Gapped Installation Script
# ─────────────────────────────────────────────────────────
#
# This script installs Clawdius on an air-gapped system using
# pre-downloaded artifacts. No internet access is required
# during installation.
#
# Prerequisites (download on connected machine):
#   1. clawdius binary (Linux x86_64 musl)
#   2. clawdius-gateway binary (Linux x86_64 musl)
#   3. This script
#
# Usage:
#   ./install-airgapped.sh --artifacts /path/to/artifacts --install /opt/clawdius

set -euo pipefail

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info()  { echo -e "${GREEN}[INFO]${NC} $*"; }
log_warn()  { echo -e "${YELLOW}[WARN]${NC} $*"; }
log_error() { echo -e "${RED}[ERROR]${NC} $*" >&2; }

# Defaults
INSTALL_DIR="/opt/clawdius"
ARTIFACTS_DIR="./artifacts"
CONFIG_DIR="/etc/clawdius"
DATA_DIR="/var/lib/clawdius"
USER="clawdius"
SYSTEMD=true
VERIFY_CHECKSUMS=true

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --artifacts)
            ARTIFACTS_DIR="$2"
            shift 2
            ;;
        --install)
            INSTALL_DIR="$2"
            shift 2
            ;;
        --config)
            CONFIG_DIR="$2"
            shift 2
            ;;
        --data)
            DATA_DIR="$2"
            shift 2
            ;;
        --user)
            USER="$2"
            shift 2
            ;;
        --no-systemd)
            SYSTEMD=false
            shift
            ;;
        --no-verify)
            VERIFY_CHECKSUMS=false
            shift
            ;;
        --help|-h)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --artifacts DIR   Path to artifact directory (default: ./artifacts)"
            echo "  --install DIR     Installation directory (default: /opt/clawdius)"
            echo "  --config DIR      Configuration directory (default: /etc/clawdius)"
            echo "  --data DIR        Data directory (default: /var/lib/clawdius)"
            echo "  --user USER       System user (default: clawdius)"
            echo "  --no-systemd      Skip systemd service installation"
            echo "  --no-verify       Skip checksum verification"
            echo "  -h, --help        Show this help"
            exit 0
            ;;
        *)
            log_error "Unknown option: $1"
            exit 1
            ;;
    esac
done

log_info "Clawdius Air-Gapped Installation"
log_info "================================"
log_info "Install dir:  $INSTALL_DIR"
log_info "Config dir:   $CONFIG_DIR"
log_info "Data dir:     $DATA_DIR"
log_info "Artifacts:    $ARTIFACTS_DIR"
log_info "System user:  $USER"
echo ""

# ── Pre-flight checks ──

if [[ ! -d "$ARTIFACTS_DIR" ]]; then
    log_error "Artifacts directory not found: $ARTIFACTS_DIR"
    exit 1
fi

if [[ $EUID -ne 0 ]]; then
    log_error "This script must be run as root"
    exit 1
fi

# Check for required binaries
for bin in clawdius clawdius-gateway; do
    if [[ ! -f "$ARTIFACTS_DIR/$bin" ]]; then
        log_error "Binary not found: $ARTIFACTS_DIR/$bin"
        exit 1
    fi
done

# ── Verify checksums ──

if [[ "$VERIFY_CHECKSUMS" == "true" ]]; then
    log_info "Verifying checksums..."
    CHECKSUM_FILE="$ARTIFACTS_DIR/sha256sums.txt"
    if [[ -f "$CHECKSUM_FILE" ]]; then
        (cd "$ARTIFACTS_DIR" && sha256sum -c sha256sums.txt) || {
            log_error "Checksum verification failed!"
            exit 1
        }
        log_info "Checksums verified"
    else
        log_warn "No sha256sums.txt found, skipping verification"
    fi
fi

# ── Create system user ──

if ! id "$USER" &>/dev/null; then
    log_info "Creating system user: $USER"
    useradd --system --home-dir "$INSTALL_DIR" --shell /usr/sbin/nologin "$USER"
else
    log_info "User $USER already exists"
fi

# ── Create directories ──

log_info "Creating directories..."
mkdir -p "$INSTALL_DIR/bin"
mkdir -p "$INSTALL_DIR/static"
mkdir -p "$CONFIG_DIR"
mkdir -p "$DATA_DIR/sessions"
mkdir -p "$DATA_DIR/workspaces"
mkdir -p "$DATA_DIR/keys"
mkdir -p "/var/log/clawdius"

# ── Install binaries ──

log_info "Installing binaries..."
for bin in clawdius clawdius-gateway; do
    cp "$ARTIFACTS_DIR/$bin" "$INSTALL_DIR/bin/$bin"
    chmod 755 "$INSTALL_DIR/bin/$bin"
    log_info "  Installed: $INSTALL_DIR/bin/$bin"
done

# Install static dashboard if present
if [[ -f "$ARTIFACTS_DIR/index.html" ]]; then
    cp "$ARTIFACTS_DIR/index.html" "$INSTALL_DIR/static/index.html"
    log_info "  Installed: $INSTALL_DIR/static/index.html"
fi

# ── Generate encryption key ──

KEY_FILE="$DATA_DIR/keys/master.key"
if [[ ! -f "$KEY_FILE" ]]; then
    log_info "Generating encryption key..."
    # Generate 32 random bytes and encode as hex
    if command -v head &>/dev/null && command -v od &>/dev/null; then
        head -c 32 /dev/urandom | od -A n -t x1 | tr -d ' \n' > "$KEY_FILE"
    elif command -v openssl &>/dev/null; then
        openssl rand -hex 32 > "$KEY_FILE"
    else
        # Fallback: use /dev/urandom directly
        dd if=/dev/urandom bs=32 count=1 2>/dev/null | od -A n -t x1 | tr -d ' \n' > "$KEY_FILE"
    fi
    chmod 600 "$KEY_FILE"
    chown "$USER:$USER" "$KEY_FILE"
    log_info "  Generated: $KEY_FILE"
else
    log_info "Encryption key already exists: $KEY_FILE"
fi

# ── Generate admin API key ──

ADMIN_KEY_FILE="$CONFIG_DIR/admin-api-key"
if [[ ! -f "$ADMIN_KEY_FILE" ]]; then
    log_info "Generating admin API key..."
    ADMIN_KEY=$(head -c 32 /dev/urandom | od -A n -t x1 | tr -d ' \n' 2>/dev/null || echo "change-me-$(date +%s)")
    echo "$ADMIN_KEY" > "$ADMIN_KEY_FILE"
    chmod 600 "$ADMIN_KEY_FILE"
    log_info "  Generated: $ADMIN_KEY_FILE"
    log_warn "  IMPORTANT: Save this API key! It will not be shown again."
    log_warn "  Admin API Key: $ADMIN_KEY"
else
    log_info "Admin API key already exists: $ADMIN_KEY_FILE"
fi

# ── Create default config ──

CONFIG_FILE="$CONFIG_DIR/config.toml"
if [[ ! -f "$CONFIG_FILE" ]]; then
    log_info "Creating default configuration..."
    cat > "$CONFIG_FILE" << 'TOML'
# Clawdius Air-Gapped Configuration

[general]
# Air-gapped mode: blocks all outbound connections except LLM providers
air_gapped = true

[llm]
# Configure your local/self-hosted LLM endpoint
default_provider = "local"
model = "local-model"

[llm.providers.local]
base_url = "http://localhost:11434/v1"  # Ollama default
api_key = "ollama"

[storage]
# Local SQLite storage (no cloud databases)
backend = "sqlite"
path = "/var/lib/clawdius/sessions/clawdius.db"

[security]
# Encryption key location
encryption_key_file = "/var/lib/clawdius/keys/master.key"

[telemetry]
# Disable all telemetry in air-gapped mode
enabled = false
crash_reports = false

[gateway]
host = "0.0.0.0"
port = 8080
admin_api_key_file = "/etc/clawdius/admin-api-key"

[billing]
# Self-hosted: no Stripe
stripe_enabled = false
TOML
    chmod 644 "$CONFIG_FILE"
    log_info "  Created: $CONFIG_FILE"
else
    log_info "Configuration already exists: $CONFIG_FILE"
fi

# ── Set ownership ──

log_info "Setting ownership..."
chown -R "$USER:$USER" "$INSTALL_DIR"
chown -R "$USER:$USER" "$CONFIG_DIR"
chown -R "$USER:$USER" "$DATA_DIR"
chown -R "$USER:$USER" "/var/log/clawdius"

# ── Install systemd services ──

if [[ "$SYSTEMD" == "true" ]] && command -v systemctl &>/dev/null; then
    log_info "Installing systemd services..."

    # Clawdius Gateway service
    cat > /etc/systemd/system/clawdius-gateway.service << SYSTEMD
[Unit]
Description=Clawdius Gateway
After=network.target
Wants=network.target

[Service]
Type=simple
User=${USER}
Group=${USER}
ExecStart=${INSTALL_DIR}/bin/clawdius-gateway serve --config ${CONFIG_DIR}/config.toml
Restart=on-failure
RestartSec=5
Environment=CLAWDIUS_ENCRYPTION_KEY_FILE=${KEY_FILE}
Environment=RUST_LOG=info
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=${DATA_DIR} ${CONFIG_DIR} /var/log/clawdius
PrivateTmp=true

[Install]
WantedBy=multi-user.target
SYSTEMD

    systemctl daemon-reload
    systemctl enable clawdius-gateway.service
    log_info "  Installed: clawdius-gateway.service"
else
    log_warn "Skipping systemd service installation"
fi

# ── Create PATH symlink ──

ln -sf "$INSTALL_DIR/bin/clawdius" /usr/local/bin/clawdius 2>/dev/null || true
ln -sf "$INSTALL_DIR/bin/clawdius-gateway" /usr/local/bin/clawdius-gateway 2>/dev/null || true

# ── Done ──

echo ""
log_info "Installation complete!"
echo ""
log_info "Next steps:"
log_info "  1. Edit configuration:  ${CONFIG_DIR}/config.toml"
log_info "  2. Start gateway:       systemctl start clawdius-gateway"
log_info "  3. Open dashboard:      http://localhost:8080/static/index.html"
log_info "  4. Admin API key:       ${ADMIN_KEY_FILE}"
log_info "  5. Encryption key:      ${KEY_FILE}"
echo ""
log_warn "SECURITY REMINDERS:"
log_warn "  - Restrict access to ${KEY_FILE} (mode 600)"
log_warn "  - Restrict access to ${ADMIN_KEY_FILE} (mode 600)"
log_warn "  - Configure firewall to only allow necessary ports"
log_warn "  - Review LLM provider endpoint in config.toml"
log_warn "  - Schedule regular backups of ${DATA_DIR}"
echo ""

exit 0
