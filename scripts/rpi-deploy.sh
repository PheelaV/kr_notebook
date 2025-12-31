#!/bin/bash
# Deploy kr_notebook to Raspberry Pi
# Usage: ./scripts/rpi-deploy.sh [--no-build] [--no-backup] [--debug]

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
CONFIG_FILE="$PROJECT_DIR/.rpi-deploy.conf"

# Load config
if [ -f "$CONFIG_FILE" ]; then
    source "$CONFIG_FILE"
else
    echo "Error: Config not found. Run ./scripts/rpi-setup.sh first"
    exit 1
fi

# Parse args
DO_BUILD=true
DO_BACKUP=true
PROFILE="--release"
PROFILE_NAME="release"

for arg in "$@"; do
    case $arg in
        --no-build)  DO_BUILD=false ;;
        --no-backup) DO_BACKUP=false ;;
        --debug)     PROFILE=""; PROFILE_NAME="debug" ;;
    esac
done

BINARY="$PROJECT_DIR/target/$TARGET/$PROFILE_NAME/kr_notebook"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)

echo "=== Deploying to $RPI_SSH ==="
echo ""

# Step 1: Build
if [ "$DO_BUILD" = true ]; then
    echo "[1/6] Building for $TARGET ($PROFILE_NAME)..."

    # Build features (optional)
    FEATURES_ARG=""
    if [ -n "$BUILD_FEATURES" ]; then
        FEATURES_ARG="--features $BUILD_FEATURES"
        echo "  Features: $BUILD_FEATURES"
    fi

    cd "$PROJECT_DIR"

    if [ "$USE_CROSS" = true ]; then
        echo "  Using 'cross' (Docker)..."
        cross build $PROFILE --target "$TARGET" $FEATURES_ARG
    elif command -v cargo-zigbuild &>/dev/null; then
        echo "  Using cargo-zigbuild..."
        cargo zigbuild $PROFILE --target "$TARGET" $FEATURES_ARG
    else
        echo "  Using native cross-compiler..."
        cargo build $PROFILE --target "$TARGET" $FEATURES_ARG
    fi
    echo ""
else
    echo "[1/6] Skipping build (--no-build)"
fi

# Check binary exists
if [ ! -f "$BINARY" ]; then
    echo "Error: Binary not found at $BINARY"
    echo "Run without --no-build first"
    exit 1
fi

# Step 2: Test SSH connection
echo "[2/6] Testing SSH connection..."
if ! ssh -o ConnectTimeout=5 "$RPI_SSH" "echo 'OK'" &>/dev/null; then
    echo "Error: Cannot connect to $RPI_SSH"
    echo "Check RPI_SSH in .rpi-deploy.conf"
    exit 1
fi
echo "  Connected to $(ssh "$RPI_SSH" 'hostname')"

# Step 3: Backup on RPi
if [ "$DO_BACKUP" = true ]; then
    echo ""
    echo "[3/6] Creating backup on RPi..."
    ssh "$RPI_SSH" bash -s "$RPI_INSTALL_DIR" "$TIMESTAMP" << 'BACKUP_SCRIPT'
        INSTALL_DIR="$1"
        TIMESTAMP="$2"
        BACKUP_DIR="$INSTALL_DIR/backups/$TIMESTAMP"

        if [ -d "$INSTALL_DIR/data" ]; then
            mkdir -p "$BACKUP_DIR"

            # Backup databases
            if [ -f "$INSTALL_DIR/data/app.db" ]; then
                cp "$INSTALL_DIR/data/app.db" "$BACKUP_DIR/"
            fi

            if [ -d "$INSTALL_DIR/data/users" ]; then
                mkdir -p "$BACKUP_DIR/users"
                for user_dir in "$INSTALL_DIR/data/users"/*/; do
                    if [ -d "$user_dir" ]; then
                        username=$(basename "$user_dir")
                        if [ -f "$user_dir/learning.db" ]; then
                            mkdir -p "$BACKUP_DIR/users/$username"
                            cp "$user_dir/learning.db" "$BACKUP_DIR/users/$username/"
                        fi
                    fi
                done
            fi

            echo "  Backup created: $BACKUP_DIR"
        else
            echo "  No data directory found, skipping backup"
        fi
BACKUP_SCRIPT
else
    echo ""
    echo "[3/6] Skipping backup (--no-backup)"
fi

# Step 4: Stop service
echo ""
echo "[4/6] Stopping service..."
ssh "$RPI_SSH" bash -s "$RPI_SERVICE" << 'STOP_SCRIPT'
    SERVICE="$1"
    if systemctl is-active --quiet "$SERVICE" 2>/dev/null; then
        sudo systemctl stop "$SERVICE"
        echo "  Stopped $SERVICE"
    else
        echo "  Service not running"
    fi
STOP_SCRIPT

# Step 5: Deploy binary
echo ""
echo "[5/6] Deploying binary..."
ssh "$RPI_SSH" "mkdir -p $RPI_INSTALL_DIR/target/release"
scp "$BINARY" "$RPI_SSH:$RPI_INSTALL_DIR/target/release/kr_notebook.new"
ssh "$RPI_SSH" bash -s "$RPI_INSTALL_DIR" << 'DEPLOY_SCRIPT'
    INSTALL_DIR="$1"
    cd "$INSTALL_DIR/target/release"

    # Backup old binary
    if [ -f "kr_notebook" ]; then
        mv "kr_notebook" "kr_notebook.old"
    fi

    # Install new binary
    mv "kr_notebook.new" "kr_notebook"
    chmod +x "kr_notebook"
    echo "  Binary installed"
DEPLOY_SCRIPT

# Step 6: Start service
echo ""
echo "[6/6] Starting service..."
ssh "$RPI_SSH" bash -s "$RPI_SERVICE" << 'START_SCRIPT'
    SERVICE="$1"
    if systemctl list-unit-files | grep -q "^$SERVICE"; then
        sudo systemctl start "$SERVICE"
        sleep 2
        if systemctl is-active --quiet "$SERVICE"; then
            echo "  Service started successfully"
        else
            echo "  Warning: Service may have failed to start"
            echo "  Check with: sudo journalctl -u $SERVICE -n 50"
        fi
    else
        echo "  No systemd service configured"
        echo "  Start manually with: ./target/release/kr_notebook"
    fi
START_SCRIPT

echo ""
echo "=== Deploy Complete ==="
echo ""
echo "To rollback:"
echo "  ssh $RPI_SSH 'cd $RPI_INSTALL_DIR/target/release && mv kr_notebook.old kr_notebook && sudo systemctl restart $RPI_SERVICE'"
