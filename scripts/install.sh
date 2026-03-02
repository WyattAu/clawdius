#!/usr/bin/env bash
set -euo pipefail

VERSION="${VERSION:-0.1.0}"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"
CLAWDIUS_HOME="${CLAWDIUS_HOME:-$HOME/.clawdius}"
REPO_URL="${REPO_URL:-https://github.com/clawdius/clawdius}"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1" >&2
}

detect_os() {
    case "$(uname -s)" in
        Linux*)  echo "linux" ;;
        Darwin*) echo "macos" ;;
        *)       echo "unknown" ;;
    esac
}

detect_arch() {
    case "$(uname -m)" in
        x86_64|amd64) echo "x86_64" ;;
        aarch64|arm64) echo "aarch64" ;;
        *)             echo "unknown" ;;
    esac
}

check_dependencies() {
    local deps=("curl" "tar")
    local missing=()
    
    for dep in "${deps[@]}"; do
        if ! command -v "${dep}" &>/dev/null; then
            missing+=("${dep}")
        fi
    done
    
    if [[ ${#missing[@]} -gt 0 ]]; then
        log_error "Missing dependencies: ${missing[*]}"
        exit 1
    fi
}

download_binary() {
    local os="$1"
    local arch="$2"
    local download_url="${REPO_URL}/releases/download/v${VERSION}/clawdius-${VERSION}-${arch}-${os}.tar.gz"
    local tmp_dir
    tmp_dir=$(mktemp -d)
    
    log_info "Downloading Clawdius v${VERSION} for ${os}/${arch}..."
    
    if ! curl -fsSL "${download_url}" -o "${tmp_dir}/clawdius.tar.gz"; then
        log_error "Failed to download from ${download_url}"
        log_info "Try building from source: cargo install --git ${REPO_URL}"
        rm -rf "${tmp_dir}"
        exit 1
    fi
    
    tar -xzf "${tmp_dir}/clawdius.tar.gz" -C "${tmp_dir}"
    
    echo "${tmp_dir}"
}

install_binary() {
    local tmp_dir="$1"
    
    log_info "Installing binary to ${INSTALL_DIR}..."
    
    if [[ ! -w "${INSTALL_DIR}" ]] && [[ $EUID -ne 0 ]]; then
        log_warn "Insufficient permissions for ${INSTALL_DIR}, trying with sudo..."
        sudo cp "${tmp_dir}/clawdius" "${INSTALL_DIR}/clawdius"
        sudo chmod +x "${INSTALL_DIR}/clawdius"
    else
        cp "${tmp_dir}/clawdius" "${INSTALL_DIR}/clawdius"
        chmod +x "${INSTALL_DIR}/clawdius"
    fi
    
    log_info "Binary installed successfully"
}

init_clawdius_home() {
    log_info "Initializing ${CLAWDIUS_HOME}..."
    
    mkdir -p "${CLAWDIUS_HOME}/specs"
    mkdir -p "${CLAWdiUS_HOME}/logs"
    mkdir -p "${CLAWDIUS_HOME}/cache"
    
    if [[ ! -f "${CLAWDIUS_HOME}/settings.toml" ]]; then
        cat > "${CLAWDIUS_HOME}/settings.toml" << 'EOF'
[general]
log_level = "info"

[host]
runtime = "monoio"

[sentinel]
default_tier = "native"

[brain]
provider = "openai"

[graph_rag]
db_path = "${CLAWDIUS_HOME}/cache/graph.db"
EOF
        log_info "Created default settings.toml"
    fi
}

verify_installation() {
    log_info "Verifying installation..."
    
    if ! command -v clawdius &>/dev/null; then
        log_error "clawdius not found in PATH"
        exit 1
    fi
    
    log_info "Clawdius v${VERSION} installed successfully!"
    log_info "Run 'clawdius --help' to get started"
}

cleanup() {
    if [[ -n "${tmp_dir:-}" ]] && [[ -d "${tmp_dir}" ]]; then
        rm -rf "${tmp_dir}"
    fi
}

trap cleanup EXIT

main() {
    log_info "Clawdius Installer v${VERSION}"
    
    check_dependencies
    
    local os arch
    os=$(detect_os)
    arch=$(detect_arch)
    
    if [[ "${os}" == "unknown" ]] || [[ "${arch}" == "unknown" ]]; then
        log_error "Unsupported platform: ${os}/${arch}"
        exit 1
    fi
    
    log_info "Detected platform: ${os}/${arch}"
    
    tmp_dir=$(download_binary "${os}" "${arch}")
    install_binary "${tmp_dir}"
    init_clawdius_home
    verify_installation
}

main "$@"
