#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
LOCAL_DB="$PROJECT_DIR/data/hangul.db"
BACKUP_DIR="$PROJECT_DIR/data/backups"

# Configuration - override via environment or edit here
REMOTE_HOST="${KR_REMOTE_HOST:-}"
REMOTE_DB_PATH="${KR_REMOTE_DB_PATH:-~/kr_notebook/data/hangul.db}"

usage() {
    echo "Usage: $0 <push|pull> [OPTIONS]"
    echo ""
    echo "Sync SQLite database between local and remote machines."
    echo ""
    echo "Commands:"
    echo "  push    Send local database → remote server"
    echo "  pull    Get remote database → local machine"
    echo ""
    echo "Options:"
    echo "  -h, --help           Show this help message"
    echo "  -n, --dry-run        Show what would be transferred"
    echo "  --no-backup          Skip backup before sync (not recommended)"
    echo ""
    echo "Configuration (via environment variables):"
    echo "  KR_REMOTE_HOST       Remote hostname (e.g., myserver or 100.x.x.x)"
    echo "  KR_REMOTE_DB_PATH    Remote database path (default: ~/kr_notebook/data/hangul.db)"
    echo ""
    echo "Examples:"
    echo "  KR_REMOTE_HOST=myserver $0 pull"
    echo "  KR_REMOTE_HOST=100.64.0.1 $0 push --dry-run"
    echo ""
    echo "Tip: Add to ~/.bashrc or ~/.zshrc:"
    echo "  export KR_REMOTE_HOST=your-tailscale-hostname"
}

backup_local_db() {
    if [[ ! -f "$LOCAL_DB" ]]; then
        echo "No local database to backup."
        return
    fi

    mkdir -p "$BACKUP_DIR"
    local backup_file="$BACKUP_DIR/hangul-$(date +%Y%m%d-%H%M%S).db"
    cp "$LOCAL_DB" "$backup_file"
    echo "Backed up local DB to: $backup_file"

    # Keep only last 10 backups
    ls -t "$BACKUP_DIR"/hangul-*.db 2>/dev/null | tail -n +11 | xargs -r rm --
}

check_config() {
    if [[ -z "$REMOTE_HOST" ]]; then
        echo "Error: KR_REMOTE_HOST not set."
        echo ""
        echo "Set it via environment variable:"
        echo "  export KR_REMOTE_HOST=your-tailscale-hostname"
        echo ""
        echo "Or run directly:"
        echo "  KR_REMOTE_HOST=myserver $0 $1"
        exit 1
    fi
}

do_pull() {
    local dry_run="$1"
    local skip_backup="$2"

    check_config "pull"

    echo "Pulling database from $REMOTE_HOST..."

    if [[ "$skip_backup" != "true" ]]; then
        backup_local_db
    fi

    mkdir -p "$(dirname "$LOCAL_DB")"

    local rsync_opts=(-avz --progress)
    if [[ "$dry_run" == "true" ]]; then
        rsync_opts+=(--dry-run)
        echo "[DRY RUN] Would transfer:"
    fi

    rsync "${rsync_opts[@]}" "$REMOTE_HOST:$REMOTE_DB_PATH" "$LOCAL_DB"

    if [[ "$dry_run" != "true" ]]; then
        echo "Pull complete."
    fi
}

do_push() {
    local dry_run="$1"
    local skip_backup="$2"

    check_config "push"

    if [[ ! -f "$LOCAL_DB" ]]; then
        echo "Error: Local database not found at $LOCAL_DB"
        exit 1
    fi

    echo "Pushing database to $REMOTE_HOST..."

    if [[ "$skip_backup" != "true" ]]; then
        backup_local_db
    fi

    local rsync_opts=(-avz --progress)
    if [[ "$dry_run" == "true" ]]; then
        rsync_opts+=(--dry-run)
        echo "[DRY RUN] Would transfer:"
    fi

    # Create remote directory if needed
    ssh "$REMOTE_HOST" "mkdir -p \$(dirname $REMOTE_DB_PATH)"

    rsync "${rsync_opts[@]}" "$LOCAL_DB" "$REMOTE_HOST:$REMOTE_DB_PATH"

    if [[ "$dry_run" != "true" ]]; then
        echo "Push complete."
        echo ""
        echo "Reminder: Restart the remote container to pick up changes:"
        echo "  ssh $REMOTE_HOST 'cd ~/kr_notebook && docker compose restart'"
    fi
}

# Parse arguments
COMMAND=""
DRY_RUN="false"
SKIP_BACKUP="false"

while [[ $# -gt 0 ]]; do
    case $1 in
        push|pull)
            COMMAND="$1"
            shift
            ;;
        -n|--dry-run)
            DRY_RUN="true"
            shift
            ;;
        --no-backup)
            SKIP_BACKUP="true"
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

if [[ -z "$COMMAND" ]]; then
    echo "Error: No command specified."
    echo ""
    usage
    exit 1
fi

case $COMMAND in
    pull)
        do_pull "$DRY_RUN" "$SKIP_BACKUP"
        ;;
    push)
        do_push "$DRY_RUN" "$SKIP_BACKUP"
        ;;
esac
