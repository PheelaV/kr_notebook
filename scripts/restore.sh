#!/bin/bash
# Restore kr_notebook databases from a named backup
# Usage: ./scripts/restore.sh <backup_name> [--force]
#   backup_name: name of backup to restore (e.g., 20251230_213000)
#   --force: skip confirmation prompt

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
DATA_DIR="$PROJECT_DIR/data"
BACKUP_BASE="$PROJECT_DIR/backups"

# Check arguments
if [ -z "$1" ]; then
    echo "Usage: $0 <backup_name> [--force]"
    echo ""
    echo "Available backups:"
    if [ -d "$BACKUP_BASE" ]; then
        ls -1 "$BACKUP_BASE" 2>/dev/null | while read backup; do
            if [ -f "$BACKUP_BASE/$backup/manifest.txt" ]; then
                created=$(grep "^Created:" "$BACKUP_BASE/$backup/manifest.txt" | cut -d' ' -f2-)
                echo "  $backup  ($created)"
            else
                echo "  $backup"
            fi
        done
    else
        echo "  (no backups found)"
    fi
    exit 1
fi

BACKUP_NAME="$1"
BACKUP_DIR="$BACKUP_BASE/$BACKUP_NAME"
FORCE=""

if [ "$2" = "--force" ]; then
    FORCE="yes"
fi

# Check if backup exists
if [ ! -d "$BACKUP_DIR" ]; then
    echo "Error: Backup '$BACKUP_NAME' not found at $BACKUP_DIR"
    exit 1
fi

# Show what will be restored
echo "Restore from backup: $BACKUP_NAME"
echo "==============================="
if [ -f "$BACKUP_DIR/manifest.txt" ]; then
    cat "$BACKUP_DIR/manifest.txt"
fi
echo "==============================="
echo ""

# Confirm unless --force
if [ -z "$FORCE" ]; then
    echo "WARNING: This will OVERWRITE your current databases!"
    echo "Make sure the server is STOPPED before restoring."
    echo ""
    read -p "Are you sure you want to restore? (yes/no): " confirm
    if [ "$confirm" != "yes" ]; then
        echo "Restore cancelled."
        exit 0
    fi
fi

echo ""
echo "Restoring..."

# Restore auth database
if [ -f "$BACKUP_DIR/app.db" ]; then
    cp "$BACKUP_DIR/app.db" "$DATA_DIR/app.db"
    echo "  [OK] app.db (auth database)"
fi

# Restore user databases
if [ -d "$BACKUP_DIR/users" ]; then
    for user_backup in "$BACKUP_DIR/users"/*/; do
        if [ -d "$user_backup" ]; then
            username=$(basename "$user_backup")
            if [ -f "$user_backup/learning.db" ]; then
                mkdir -p "$DATA_DIR/users/$username"
                cp "$user_backup/learning.db" "$DATA_DIR/users/$username/learning.db"
                echo "  [OK] users/$username/learning.db"
            fi
        fi
    done
fi

echo ""
echo "Restore complete!"
echo "You can now start the server."
