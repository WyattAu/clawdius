#!/bin/bash
# Clawdius Deployment Script
# Usage: ./deploy.sh [docker|podman|systemd]

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
DEPLOY_METHOD="${1:-docker}"

# Colors for output
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
    echo -e "${RED}[ERROR]${NC} $1"
}

show_usage() {
    cat << EOF
Clawdius Deployment Script

Usage: $0 [OPTIONS]

Options:
    docker    Deploy using Docker Compose (default)
    podman    Deploy using Podman Compose
    systemd   Deploy as systemd service
    stop      Stop running services
    logs      Show logs
    status    Show service status
    clean     Remove containers and volumes

Examples:
    $0 docker      # Deploy with Docker
    $0 podman      # Deploy with Podman
    $0 systemd     # Install as systemd service
    $0 stop         # Stop all services
EOF
}

check_dependencies() {
    log_info "Checking dependencies..."
    
    case "$DEPLOY_METHOD" in
        docker)
            if ! command -v docker &> /dev/null; then
                log_error "Docker is not installed"
                exit 1
            fi
            if ! command -v docker-compose &> /dev/null && ! docker compose version &> /dev/null; then
                log_error "Docker Compose is not installed"
                exit 1
            fi
            ;;
        podman)
            if ! command -v podman &> /dev/null; then
                log_error "Podman is not installed"
                exit 1
            fi
            ;;
        systemd)
            if [ "$(id -u)" != "0" ]; then
                log_error "systemd deployment requires root privileges"
                exit 1
            fi
            ;;
        stop|logs|status|clean)
            # These commands don't need dependency checks
            ;;
        *)
            log_error "Unknown deployment method: $DEPLOY_METHOD"
            show_usage
            exit 1
            ;;
    esac
    
    log_info "Dependencies OK"
}

deploy_docker() {
    log_info "Deploying with Docker Compose..."
    cd "$SCRIPT_DIR/docker"
    
    # Copy .env.example to .env if not exists
    if [ ! -f .env ]; then
        cp .env.example .env
        log_info "Created .env file from .env.example"
    fi
    
    # Pull images
    docker compose pull
    
    # Start services
    docker compose up -d
    
    log_info "Docker deployment complete!"
    log_info "Clawdius is running at http://localhost:3000"
}

deploy_podman() {
    log_info "Deploying with Podman Compose..."
    cd "$SCRIPT_DIR/docker"
    
    # Copy .env.example to .env if not exists
    if [ ! -f .env ]; then
        cp .env.example .env
        log_info "Created .env file from .env.example"
    fi
    
    # Pull images
    podman-compose pull
    
    # Start services
    podman-compose up -d
    
    log_info "Podman deployment complete!"
    log_info "Clawdius is running at http://localhost:3000"
}

deploy_systemd() {
    log_info "Installing as systemd service..."
    
    # Build release binary if not exists
    if [ ! -f /usr/local/bin/clawdius ]; then
        log_info "Building Clawdius binary..."
        cd "$PROJECT_ROOT"
        cargo build --release -p clawdius --bin clawdius
        sudo cp target/release/clawdius /usr/local/bin/
    fi
    
    # Create clawdius user if not exists
    if ! id -u clawdius &>/dev/null; then
        log_info "Creating clawdius user..."
        sudo useradd -r -s /bin/false clawdius
    fi
    
    # Create directories
    sudo mkdir -p /opt/clawdius/data / /opt/clawdius/logs
    
    # Copy configuration
    sudo cp "$SCRIPT_DIR/docker/config.toml" /opt/clawdius/config.toml
    
    # Install systemd service
    sudo cp "$SCRIPT_DIR/systemd/clawdius.service" /etc/systemd/system/
    sudo systemctl daemon-reload
    sudo systemctl enable clawdius
    sudo systemctl start clawdius
    
    log_info "systemd installation complete!"
    log_info "Check status with: sudo systemctl status clawdius"
}

stop_services() {
    log_info "Stopping services..."
    
    if command -v docker-compose &> /dev/null || docker compose version &> /dev/null; then
        cd "$SCRIPT_DIR/docker"
        docker compose down 2>/dev/null || true
    fi
    
    if command -v podman-compose &> /dev/null; then
        cd "$SCRIPT_DIR/docker"
        podman-compose down 2>/dev/null || true
    fi
    
    if systemctl is-active --quiet clawdius 2>/dev/null; then
        sudo systemctl stop clawdius
    fi
    
    log_info "Services stopped"
}

show_logs() {
    log_info "Showing logs..."
    
    if command -v docker-compose &> /dev/null || docker compose version &> /dev/null; then
        cd "$SCRIPT_DIR/docker"
        docker compose logs -f --tail=100
    elif command -v podman-compose &> /dev/null; then
        cd "$SCRIPT_DIR/docker"
        podman-compose logs -f --tail=100
    elif systemctl is-active --quiet clawdius 2>/dev/null; then
        sudo journalctl -u clawdius -f -n 100
    else
        log_error "No running services found"
        exit 1
    fi
}

show_status() {
    log_info "Service status..."
    
    echo ""
    echo "=== Docker Compose ==="
    if command -v docker-compose &> /dev/null || docker compose version &> /dev/null; then
        cd "$SCRIPT_DIR/docker"
        docker compose ps
    else
        echo "Not running"
    fi
    
    echo ""
    echo "=== Podman Compose ==="
    if command -v podman-compose &> /dev/null; then
        cd "$SCRIPT_DIR/docker"
        podman-compose ps
    else
        echo "Not running"
    fi
    
    echo ""
    echo "=== Systemd Service ==="
    systemctl status clawdius 2>/dev/null || echo "Not running"
}

clean_deployment() {
    log_warn "This will remove all containers and volumes!"
    read -p "Continue? (y/N) " -r response
    if [[ "$response" != "y" ]]; then
        log_info "Cancelled"
        exit 0
    fi
    
    stop_services
    
    log_info "Removing containers and volumes..."
    
    if command -v docker-compose &> /dev/null || docker compose version &> /dev/null; then
        cd "$SCRIPT_DIR/docker"
        docker compose down -v --rmi all
    fi
    
    if command -v podman-compose &> /dev/null; then
        cd "$SCRIPT_DIR/docker"
        podman-compose down -v --rmi all
    fi
    
    log_info "Cleanup complete"
}

# Main execution
check_dependencies

case "$DEPLOY_METHOD" in
    docker)
        deploy_docker
        ;;
    podman)
        deploy_podman
        ;;
    systemd)
        deploy_systemd
        ;;
    stop)
        stop_services
        ;;
    logs)
        show_logs
        ;;
    status)
        show_status
        ;;
    clean)
        clean_deployment
        ;;
    *)
        log_error "Unknown command: $DEPLOY_METHOD"
        show_usage
        exit 1
        ;;
esac
