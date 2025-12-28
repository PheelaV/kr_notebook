#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
LOG_DIR="${LOG_DIR:-$PROJECT_DIR/logs}"

usage() {
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  -b, --build    Force rebuild the Docker image"
    echo "  -h, --help     Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0              Start without rebuilding (uses cached image)"
    echo "  $0 --build      Rebuild image with latest code, then start"
}

BUILD=false

while [[ $# -gt 0 ]]; do
    case $1 in
        -b|--build)
            BUILD=true
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            usage
            exit 1
            ;;
    esac
done

# Create directories and ensure config exists
mkdir -p "$LOG_DIR"
mkdir -p "$PROJECT_DIR/data"
touch "$PROJECT_DIR/config.toml"

cd "$PROJECT_DIR"

echo "Starting kr_notebook..."
echo "  Logs: $LOG_DIR"
echo "  Data: $PROJECT_DIR/data"

if [ "$BUILD" = true ]; then
    echo "  Rebuilding image..."
    docker compose build
fi

docker compose up -d

echo ""
echo "Container started. Tailing logs (Ctrl+C to stop watching)..."
docker compose logs -f 2>&1 | tee -a "$LOG_DIR/kr_notebook-$(date +%Y%m%d).log"
