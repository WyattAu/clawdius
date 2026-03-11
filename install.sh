#!/bin/bash
set -euo pipefail

REPO="clawdius/clawdius"
BINARY="clawdius"
BINARY_CODE="clawdius-code"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

info() { echo -e "${BLUE}[INFO]${NC} $1"; }
success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }

detect_os() {
    case "$(uname -s)" in
        Darwin*) echo "apple-darwin" ;;
        Linux*)  echo "unknown-linux-gnu" ;;
        MINGW*|MSYS*|CYGWIN*) echo "pc-windows-msvc" ;;
        *)       error "Unsupported OS: $(uname -s)" ;;
    esac
}

detect_arch() {
    case "$(uname -m)" in
        x86_64|amd64) echo "x86_64" ;;
        arm64|aarch64) echo "aarch64" ;;
        *)             error "Unsupported architecture: $(uname -m)" ;;
    esac
}

detect_libc() {
    if [[ "$(uname -s)" == "Linux" ]]; then
        if ldd --version 2>&1 | grep -qi musl; then
            echo "musl"
        else
            echo "gnu"
        fi
    fi
}

get_latest_version() {
    local version
    version=$(curl -sf https://api.github.com/repos/${REPO}/releases/latest | grep '"tag_name":' | sed -E 's/.*"v([^"]+)".*/\1/')
    if [[ -z "$version" ]]; then
        error "Failed to get latest version. Please check your internet connection."
    fi
    echo "$version"
}

download_binary() {
    local version="$1"
    local target="$2"
    local tmp_dir="$3"
    local archive_ext="tar.gz"
    local archive_name="${BINARY}-${version}-${target}.${archive_ext}"
    local download_url="https://github.com/${REPO}/releases/download/v${version}/${archive_name}"
    
    info "Downloading ${archive_name}..."
    
    if ! curl -sfL -o "${tmp_dir}/${archive_name}" "${download_url}"; then
        error "Failed to download from ${download_url}"
    fi
    
    info "Verifying checksum..."
    local checksum_url="${download_url}.sha256"
    if curl -sfL -o "${tmp_dir}/${archive_name}.sha256" "${checksum_url}"; then
        cd "$tmp_dir"
        if command -v sha256sum &> /dev/null; then
            sha256sum -c "${archive_name}.sha256" || error "Checksum verification failed"
        elif command -v shasum &> /dev/null; then
            shasum -a 256 -c "${archive_name}.sha256" || error "Checksum verification failed"
        else
            warn "sha256sum not found, skipping checksum verification"
        fi
        cd - > /dev/null
    else
        warn "Checksum file not found, skipping verification"
    fi
    
    info "Extracting archive..."
    tar -xzf "${tmp_dir}/${archive_name}" -C "${tmp_dir}"
    
    echo "${tmp_dir}"
}

install_binary() {
    local tmp_dir="$1"
    local install_dir="${2:-/usr/local/bin}"
    
    if [[ ! -d "$install_dir" ]]; then
        sudo mkdir -p "$install_dir"
    fi
    
    for binary in "$BINARY" "$BINARY_CODE"; do
        if [[ -f "${tmp_dir}/${binary}" ]]; then
            info "Installing ${binary} to ${install_dir}..."
            if [[ -w "$install_dir" ]]; then
                cp "${tmp_dir}/${binary}" "${install_dir}/${binary}"
                chmod +x "${install_dir}/${binary}"
            else
                sudo cp "${tmp_dir}/${binary}" "${install_dir}/${binary}"
                sudo chmod +x "${install_dir}/${binary}"
            fi
            success "${binary} installed successfully"
        fi
    done
}

main() {
    local install_dir="/usr/local/bin"
    local version=""
    local use_musl=false
    
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --version|-v)
                version="$2"
                shift 2
                ;;
            --install-dir|-i)
                install_dir="$2"
                shift 2
                ;;
            --musl|-m)
                use_musl=true
                shift
                ;;
            --help|-h)
                echo "Usage: $0 [OPTIONS]"
                echo ""
                echo "Options:"
                echo "  -v, --version VERSION    Install specific version (default: latest)"
                echo "  -i, --install-dir DIR    Installation directory (default: /usr/local/bin)"
                echo "  -m, --musl               Use musl libc on Linux"
                echo "  -h, --help               Show this help message"
                exit 0
                ;;
            *)
                error "Unknown option: $1"
                ;;
        esac
    done
    
    info "Installing Clawdius..."
    
    local os
    os=$(detect_os)
    
    local arch
    arch=$(detect_arch)
    
    local libc_suffix=""
    if [[ "$(uname -s)" == "Linux" ]]; then
        if $use_musl || [[ "$(detect_libc)" == "musl" ]]; then
            libc_suffix="-musl"
        else
            libc_suffix="-gnu"
        fi
    fi
    
    local target="${arch}-${os}${libc_suffix}"
    info "Detected target: ${target}"
    
    if [[ -z "$version" ]]; then
        info "Fetching latest version..."
        version=$(get_latest_version)
    fi
    info "Installing version: ${version}"
    
    local tmp_dir
    tmp_dir=$(mktemp -d)
    trap "rm -rf ${tmp_dir}" EXIT
    
    download_binary "$version" "$target" "$tmp_dir"
    install_binary "$tmp_dir" "$install_dir"
    
    success "Clawdius ${version} has been installed!"
    info "Run 'clawdius --help' to get started"
}

main "$@"
