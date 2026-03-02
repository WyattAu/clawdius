#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
VERSION="${VERSION:-$(grep '^version' "${PROJECT_ROOT}/Cargo.toml" | head -1 | cut -d'"' -f2)}"
IMAGE_NAME="${IMAGE_NAME:-clawdius}"
REGISTRY="${REGISTRY:-ghcr.io/clawdius}"

log_info() {
    echo "[INFO] $1"
}

log_error() {
    echo "[ERROR] $1" >&2
}

log_success() {
    echo "[SUCCESS] $1"
}

build_binary() {
    log_info "Building release binary..."
    cd "${PROJECT_ROOT}"
    cargo build --release
    
    BINARY="${PROJECT_ROOT}/target/release/clawdius"
    if [[ ! -f "${BINARY}" ]]; then
        log_error "Binary not found at ${BINARY}"
        exit 1
    fi
    
    SIZE=$(du -h "${BINARY}" | cut -f1)
    log_success "Binary built successfully (${SIZE})"
}

run_tests() {
    log_info "Running tests..."
    cd "${PROJECT_ROOT}"
    cargo test --release --quiet
    
    log_success "All tests passed"
}

build_docker() {
    log_info "Building Docker image..."
    cd "${PROJECT_ROOT}"
    
    docker build \
        --build-arg BUILD_DATE="$(date -u +'%Y-%m-%dT%H:%M:%SZ')" \
        --build-arg VERSION="${VERSION}" \
        --tag "${IMAGE_NAME}:${VERSION}" \
        --tag "${IMAGE_NAME}:latest" \
        .
    
    log_success "Docker image built: ${IMAGE_NAME}:${VERSION}"
}

push_docker() {
    log_info "Pushing Docker image to registry..."
    
    docker tag "${IMAGE_NAME}:${VERSION}" "${REGISTRY}/${IMAGE_NAME}:${VERSION}"
    docker tag "${IMAGE_NAME}:latest" "${REGISTRY}/${IMAGE_NAME}:latest"
    
    docker push "${REGISTRY}/${IMAGE_NAME}:${VERSION}"
    docker push "${REGISTRY}/${IMAGE_NAME}:latest"
    
    log_success "Image pushed to ${REGISTRY}/${IMAGE_NAME}:${VERSION}"
}

create_archive() {
    log_info "Creating release archive..."
    
    DIST_DIR="${PROJECT_ROOT}/dist"
    mkdir -p "${DIST_DIR}"
    
    ARCHIVE_NAME="clawdius-${VERSION}-$(uname -m)-$(uname -s | tr '[:upper:]' '[:lower:]')"
    ARCHIVE_PATH="${DIST_DIR}/${ARCHIVE_NAME}.tar.gz"
    
    tar -czvf "${ARCHIVE_PATH}" \
        -C "${PROJECT_ROOT}/target/release" \
        clawdius \
        -C "${PROJECT_ROOT}" \
        README.md \
        LICENSE \
        2>/dev/null || true
    
    log_success "Archive created: ${ARCHIVE_PATH}"
}

generate_sbom() {
    log_info "Generating SBOM..."
    
    if command -v syft &>/dev/null; then
        syft "${IMAGE_NAME}:${VERSION}" -o spdx-json > "${PROJECT_ROOT}/dist/sbom-spdx.json"
        log_success "SBOM generated"
    else
        log_info "syft not installed, skipping SBOM generation"
    fi
}

main() {
    local step="${1:-all}"
    
    log_info "Deploying Clawdius v${VERSION}"
    log_info "Step: ${step}"
    
    case "${step}" in
        build)
            build_binary
            ;;
        test)
            run_tests
            ;;
        docker)
            build_docker
            ;;
        push)
            push_docker
            ;;
        archive)
            create_archive
            ;;
        sbom)
            generate_sbom
            ;;
        all)
            build_binary
            run_tests
            build_docker
            create_archive
            generate_sbom
            ;;
        *)
            log_error "Unknown step: ${step}"
            echo "Usage: $0 [build|test|docker|push|archive|sbom|all]"
            exit 1
            ;;
    esac
    
    log_success "Deployment step '${step}' completed"
}

main "$@"
