#!/usr/bin/env bash
#
# Sync databases between local and remote kr_notebook instances
# Usage: ./scripts/sync-db.sh <push|pull> [OPTIONS]
#
# Supports both Docker and bare metal (systemd) deployments.
# Uses .rpi-deploy.conf for defaults, or prompts interactively.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
CONFIG_FILE="$PROJECT_DIR/.rpi-deploy.conf"

LOCAL_DATA="$PROJECT_DIR/data"

# Load defaults from config
REMOTE_HOST=""
REMOTE_PATH=""
REMOTE_SERVICE=""
if [[ -f "$CONFIG_FILE" ]]; then
    source "$CONFIG_FILE"
    REMOTE_HOST="${RPI_SSH:-}"
    REMOTE_PATH="${RPI_INSTALL_DIR:-}"
    REMOTE_SERVICE="${RPI_SERVICE:-kr_notebook}"
fi

usage() {
    echo "Usage: $0 <push|pull> [OPTIONS]"
    echo ""
    echo "Sync databases between local and remote kr_notebook instances."
    echo ""
    echo "Commands:"
    echo "  push    Send local databases → remote"
    echo "  pull    Get remote databases → local"
    echo ""
    echo "Options:"
    echo "  -u, --user USERNAME  Sync only this user's learning.db"
    echo "  -a, --auth-only      Sync only app.db (auth database)"
    echo "  --docker             Remote uses Docker (default: bare metal/systemd)"
    echo "  --no-restart         Don't restart remote service after sync"
    echo "  -n, --dry-run        Show what would be transferred"
    echo "  -h, --help           Show this help"
    echo ""
    echo "Examples:"
    echo "  $0 pull                      # pull all databases"
    echo "  $0 push --user filip         # push only filip's learning.db"
    echo "  $0 pull --docker             # pull from Docker deployment"
    echo "  $0 push -n                   # dry-run push"
}

ensure_config() {
    if [[ -z "$REMOTE_HOST" ]]; then
        read -p "Remote host (user@host or ssh alias) [raspberry]: " REMOTE_HOST
        REMOTE_HOST="${REMOTE_HOST:-raspberry}"
    fi

    if [[ -z "$REMOTE_PATH" ]]; then
        read -p "Remote install path [~/kr_notebook]: " REMOTE_PATH
        REMOTE_PATH="${REMOTE_PATH:-~/kr_notebook}"
    fi
}

stop_remote_service() {
    local use_docker="$1"

    echo "Stopping remote service..."
    if [[ "$use_docker" == "true" ]]; then
        ssh "$REMOTE_HOST" "cd $REMOTE_PATH && docker compose down" 2>/dev/null || true
    else
        ssh "$REMOTE_HOST" "sudo systemctl stop $REMOTE_SERVICE" 2>/dev/null || true
    fi
}

start_remote_service() {
    local use_docker="$1"

    echo "Starting remote service..."
    if [[ "$use_docker" == "true" ]]; then
        ssh "$REMOTE_HOST" "cd $REMOTE_PATH && docker compose up -d"
    else
        ssh "$REMOTE_HOST" "sudo systemctl start $REMOTE_SERVICE"
    fi
}

do_sync() {
    local direction="$1"
    local dry_run="$2"
    local user_only="$3"
    local auth_only="$4"
    local use_docker="$5"
    local no_restart="$6"

    ensure_config

    local remote_data="$REMOTE_PATH/data"
    local rsync_opts=(-avz --progress)

    if [[ "$dry_run" == "true" ]]; then
        rsync_opts+=(--dry-run)
        echo "[DRY RUN]"
        echo ""
    fi

    # Determine what to sync
    local sync_desc=""
    local local_paths=()
    local remote_paths=()

    if [[ -n "$user_only" ]]; then
        # Single user
        sync_desc="user '$user_only'"
        local_paths=("$LOCAL_DATA/users/$user_only/")
        remote_paths=("$remote_data/users/$user_only/")
    elif [[ "$auth_only" == "true" ]]; then
        # Auth database only
        sync_desc="auth database (app.db)"
        local_paths=("$LOCAL_DATA/app.db")
        remote_paths=("$remote_data/app.db")
    else
        # Everything
        sync_desc="all databases"
        local_paths=("$LOCAL_DATA/app.db" "$LOCAL_DATA/users/")
        remote_paths=("$remote_data/app.db" "$remote_data/users/")
    fi

    if [[ "$direction" == "push" ]]; then
        echo "Pushing $sync_desc to $REMOTE_HOST..."
    else
        echo "Pulling $sync_desc from $REMOTE_HOST..."
    fi
    echo ""

    # Stop remote service before sync (unless dry-run)
    if [[ "$dry_run" != "true" && "$no_restart" != "true" ]]; then
        stop_remote_service "$use_docker"
        echo ""
    fi

    # Perform sync
    for i in "${!local_paths[@]}"; do
        local local_path="${local_paths[$i]}"
        local remote_path="${remote_paths[$i]}"

        if [[ "$direction" == "push" ]]; then
            # Ensure remote directory exists
            if [[ "$dry_run" != "true" ]]; then
                if [[ "$local_path" == */ ]]; then
                    ssh "$REMOTE_HOST" "mkdir -p '$remote_path'"
                else
                    ssh "$REMOTE_HOST" "mkdir -p '$(dirname "$remote_path")'"
                fi
            fi

            if [[ -e "$local_path" ]]; then
                echo "  $local_path → $REMOTE_HOST:$remote_path"
                rsync "${rsync_opts[@]}" "$local_path" "$REMOTE_HOST:$remote_path"
            else
                echo "  [skip] $local_path (not found locally)"
            fi
        else
            # Pull
            if [[ "$local_path" == */ ]]; then
                mkdir -p "$local_path"
            else
                mkdir -p "$(dirname "$local_path")"
            fi

            echo "  $REMOTE_HOST:$remote_path → $local_path"
            rsync "${rsync_opts[@]}" "$REMOTE_HOST:$remote_path" "$local_path" 2>/dev/null || echo "  [skip] not found on remote"
        fi
    done

    echo ""

    # Restart remote service (unless dry-run or --no-restart)
    if [[ "$dry_run" != "true" && "$no_restart" != "true" ]]; then
        start_remote_service "$use_docker"
    fi

    echo ""
    echo "Done!"
}

# Parse arguments
COMMAND=""
DRY_RUN="false"
USER_ONLY=""
AUTH_ONLY="false"
USE_DOCKER="false"
NO_RESTART="false"

while [[ $# -gt 0 ]]; do
    case $1 in
        push|pull)
            COMMAND="$1"
            shift
            ;;
        -u|--user)
            USER_ONLY="$2"
            shift 2
            ;;
        -a|--auth-only)
            AUTH_ONLY="true"
            shift
            ;;
        --docker)
            USE_DOCKER="true"
            shift
            ;;
        --no-restart)
            NO_RESTART="true"
            shift
            ;;
        -n|--dry-run)
            DRY_RUN="true"
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            echo ""
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

do_sync "$COMMAND" "$DRY_RUN" "$USER_ONLY" "$AUTH_ONLY" "$USE_DOCKER" "$NO_RESTART"
