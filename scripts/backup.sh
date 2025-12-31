#!/bin/bash
# Create versioned backup of all kr_notebook databases
# Usage: ./scripts/backup.sh [backup_name]
#   backup_name: optional, defaults to timestamp (e.g., 20251230_213000)

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
DATA_DIR="$PROJECT_DIR/data"
BACKUP_BASE="$PROJECT_DIR/backups"

# Generate backup name
if [ -n "$1" ]; then
    BACKUP_NAME="$1"
else
    BACKUP_NAME="$(date +%Y%m%d_%H%M%S)"
fi

BACKUP_DIR="$BACKUP_BASE/$BACKUP_NAME"

# Check if data directory exists
if [ ! -d "$DATA_DIR" ]; then
    echo "Error: Data directory not found at $DATA_DIR"
    exit 1
fi

# Check if backup already exists
if [ -d "$BACKUP_DIR" ]; then
    echo "Error: Backup '$BACKUP_NAME' already exists at $BACKUP_DIR"
    exit 1
fi

# Create backup directory
mkdir -p "$BACKUP_DIR"

echo "Creating backup: $BACKUP_NAME"
echo "==============================="

# Backup auth database
if [ -f "$DATA_DIR/app.db" ]; then
    cp "$DATA_DIR/app.db" "$BACKUP_DIR/app.db"
    echo "  [OK] app.db (auth database)"
else
    echo "  [--] app.db (not found)"
fi

# Backup user databases
if [ -d "$DATA_DIR/users" ]; then
    mkdir -p "$BACKUP_DIR/users"
    for user_dir in "$DATA_DIR/users"/*/; do
        if [ -d "$user_dir" ]; then
            username=$(basename "$user_dir")
            if [ -f "$user_dir/learning.db" ]; then
                mkdir -p "$BACKUP_DIR/users/$username"
                cp "$user_dir/learning.db" "$BACKUP_DIR/users/$username/learning.db"
                echo "  [OK] users/$username/learning.db"
            fi
        fi
    done
fi

# Create manifest
cat > "$BACKUP_DIR/manifest.txt" << EOF
Backup: $BACKUP_NAME
Created: $(date -Iseconds)
Host: $(hostname)

Files:
$(cd "$BACKUP_DIR" && find . -name "*.db" -type f | sort)
EOF

echo "==============================="
echo "Backup complete: $BACKUP_DIR"
echo ""
echo "To restore: ./scripts/restore.sh $BACKUP_NAME"
