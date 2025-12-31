#!/usr/bin/env bash
#
# Sync scraped audio assets to a remote instance
# Usage: ./scripts/sync-audio.sh [user@host] [remote_path]
#        ./scripts/sync-audio.sh -n  # dry-run with defaults/prompts
#
# If no arguments provided, uses .rpi-deploy.conf or prompts interactively.

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Script directory (to find repo root)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
CONFIG_FILE="$REPO_ROOT/.rpi-deploy.conf"

# Source directory (relative to repo root)
SOURCE_DIR="data/scraped"

usage() {
    echo "Usage: $0 [-n] [-v] [-d] [user@host] [remote_path]"
    echo ""
    echo "Sync scraped audio assets to a remote kr_notebook instance."
    echo "If arguments omitted, uses .rpi-deploy.conf or prompts interactively."
    echo ""
    echo "Options:"
    echo "  -n    Dry run (show what would be transferred)"
    echo "  -v    Verbose output"
    echo "  -d    Delete files on remote that don't exist locally"
    echo ""
    echo "Examples:"
    echo "  $0                              # use config/prompts"
    echo "  $0 pi@raspberrypi ~/kr_notebook"
    echo "  $0 -n                           # dry-run with defaults"
    exit 1
}

# Parse options
DRY_RUN=""
VERBOSE=""
DELETE=""

while getopts "nvdh" opt; do
    case $opt in
        n) DRY_RUN="--dry-run" ;;
        v) VERBOSE="-v" ;;
        d) DELETE="--delete" ;;
        h) usage ;;
        *) usage ;;
    esac
done
shift $((OPTIND-1))

# Load defaults from config if it exists
DEFAULT_HOST=""
DEFAULT_PATH=""
if [[ -f "$CONFIG_FILE" ]]; then
    source "$CONFIG_FILE"
    DEFAULT_HOST="${RPI_SSH:-}"
    DEFAULT_PATH="${RPI_INSTALL_DIR:-}"
fi

# Get remote host
if [[ $# -ge 1 ]]; then
    REMOTE_HOST="$1"
elif [[ -n "$DEFAULT_HOST" ]]; then
    REMOTE_HOST="$DEFAULT_HOST"
else
    read -p "Remote host (user@host or ssh alias) [raspberry]: " REMOTE_HOST
    REMOTE_HOST="${REMOTE_HOST:-raspberry}"
fi

# Get remote path
if [[ $# -ge 2 ]]; then
    REMOTE_PATH="$2"
elif [[ -n "$DEFAULT_PATH" ]]; then
    REMOTE_PATH="$DEFAULT_PATH"
else
    read -p "Remote path [~/kr_notebook]: " REMOTE_PATH
    REMOTE_PATH="${REMOTE_PATH:-~/kr_notebook}"
fi

# Validate source exists
if [[ ! -d "$REPO_ROOT/$SOURCE_DIR" ]]; then
    echo -e "${RED}Error: Source directory not found: $REPO_ROOT/$SOURCE_DIR${NC}"
    echo "Run the scraper first to download audio content."
    exit 1
fi

# Show what we're doing
echo -e "${GREEN}Syncing audio assets${NC}"
echo "  From: $REPO_ROOT/$SOURCE_DIR/"
echo "  To:   $REMOTE_HOST:$REMOTE_PATH/$SOURCE_DIR/"
echo ""

if [[ -n "$DRY_RUN" ]]; then
    echo -e "${YELLOW}DRY RUN - no files will be transferred${NC}"
    echo ""
fi

# Count local files
LOCAL_COUNT=$(find "$REPO_ROOT/$SOURCE_DIR" -type f | wc -l | tr -d ' ')
LOCAL_SIZE=$(du -sh "$REPO_ROOT/$SOURCE_DIR" 2>/dev/null | cut -f1)
echo "Local: $LOCAL_COUNT files ($LOCAL_SIZE)"
echo ""

# Ensure remote directory exists
if [[ -z "$DRY_RUN" ]]; then
    echo "Ensuring remote directory exists..."
    ssh "$REMOTE_HOST" "mkdir -p '$REMOTE_PATH/$SOURCE_DIR'"
fi

# Rsync with progress
echo "Starting transfer..."
echo ""

rsync -az --progress \
    $DRY_RUN \
    $VERBOSE \
    $DELETE \
    "$REPO_ROOT/$SOURCE_DIR/" \
    "$REMOTE_HOST:$REMOTE_PATH/$SOURCE_DIR/"

echo ""
echo -e "${GREEN}Done!${NC}"

if [[ -z "$DRY_RUN" ]]; then
    # Show remote count
    REMOTE_COUNT=$(ssh "$REMOTE_HOST" "find '$REMOTE_PATH/$SOURCE_DIR' -type f 2>/dev/null | wc -l" | tr -d ' ')
    echo "Remote now has $REMOTE_COUNT files"
fi
