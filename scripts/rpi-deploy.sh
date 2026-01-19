#!/bin/bash
# Deploy kr_notebook to Raspberry Pi
# Usage: ./scripts/rpi-deploy.sh [--no-tests] [--test-fail-fast] [--no-build] [--no-backup] [--debug] [--rollback]

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
DO_ROLLBACK=false
DO_TESTS=true
TEST_FAIL_FAST=false
PROFILE="--release"
PROFILE_NAME="release"

for arg in "$@"; do
    case $arg in
        --no-build)       DO_BUILD=false ;;
        --no-backup)      DO_BACKUP=false ;;
        --no-tests)       DO_TESTS=false ;;
        --test-fail-fast) TEST_FAIL_FAST=true ;;
        --debug)          PROFILE=""; PROFILE_NAME="debug" ;;
        --rollback)       DO_ROLLBACK=true ;;
    esac
done

BINARY="$PROJECT_DIR/target/$TARGET/$PROFILE_NAME/kr_notebook"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)

# Timing helpers
format_time() {
    local seconds=$1
    if [[ $seconds -ge 60 ]]; then
        local mins=$((seconds / 60))
        local secs=$((seconds % 60))
        echo "${mins}m ${secs}s"
    else
        echo "${seconds}s"
    fi
}

# Track timing for each step
STEP_TIMES=()
STEP_NAMES=()
TOTAL_START_TIME=$SECONDS

step_start() {
    STEP_START=$SECONDS
}

step_end() {
    local name="$1"
    local elapsed=$((SECONDS - STEP_START))
    STEP_NAMES+=("$name")
    STEP_TIMES+=("$elapsed")
}

print_timing_summary() {
    local total_time=$((SECONDS - TOTAL_START_TIME))
    echo ""
    echo "=== Timing Summary ==="
    for i in "${!STEP_NAMES[@]}"; do
        printf "  %-25s %s\n" "${STEP_NAMES[$i]}:" "$(format_time ${STEP_TIMES[$i]})"
    done
    echo "  -------------------------"
    printf "  %-25s %s\n" "Total:" "$(format_time $total_time)"
}

# =============================================================================
# ROLLBACK MODE
# =============================================================================
if [ "$DO_ROLLBACK" = true ]; then
    echo "=== Rolling back deployment on $RPI_SSH ==="
    echo ""

    # Test SSH connection
    echo "[1/5] Testing SSH connection..."
    if ! ssh -o ConnectTimeout=5 "$RPI_SSH" "echo 'OK'" &>/dev/null; then
        echo "Error: Cannot connect to $RPI_SSH"
        exit 1
    fi
    echo "  Connected to $(ssh "$RPI_SSH" 'hostname')"

    # Check rollback prerequisites
    echo ""
    echo "[2/5] Checking rollback prerequisites..."
    ssh "$RPI_SSH" bash -s "$RPI_INSTALL_DIR" << 'CHECK_SCRIPT'
        INSTALL_DIR="$1"
        ERRORS=0

        # Check for old binary
        if [ ! -f "$INSTALL_DIR/target/release/kr_notebook.old" ]; then
            echo "  ERROR: No old binary found at $INSTALL_DIR/target/release/kr_notebook.old"
            ERRORS=$((ERRORS + 1))
        else
            echo "  OK: Old binary exists"
        fi

        # Check for old static assets
        if [ ! -d "$INSTALL_DIR/static.old" ]; then
            echo "  ERROR: No old static assets found at $INSTALL_DIR/static.old"
            ERRORS=$((ERRORS + 1))
        else
            echo "  OK: Old static assets exist"
        fi

        # Check for backup marker
        if [ ! -f "$INSTALL_DIR/backups/latest" ]; then
            echo "  ERROR: No backup marker found at $INSTALL_DIR/backups/latest"
            ERRORS=$((ERRORS + 1))
        else
            BACKUP_TS=$(cat "$INSTALL_DIR/backups/latest")
            if [ ! -d "$INSTALL_DIR/backups/$BACKUP_TS" ]; then
                echo "  ERROR: Backup directory $INSTALL_DIR/backups/$BACKUP_TS not found"
                ERRORS=$((ERRORS + 1))
            else
                echo "  OK: Database backup exists ($BACKUP_TS)"
            fi
        fi

        exit $ERRORS
CHECK_SCRIPT

    # Stop service
    echo ""
    echo "[3/5] Stopping service..."
    ssh "$RPI_SSH" bash -s "$RPI_SERVICE" << 'STOP_SCRIPT'
        SERVICE="$1"
        if systemctl is-active --quiet "$SERVICE" 2>/dev/null; then
            sudo systemctl stop "$SERVICE"
            echo "  Stopped $SERVICE"
        else
            echo "  Service not running"
        fi
STOP_SCRIPT

    # Restore everything
    echo ""
    echo "[4/5] Restoring from backup..."
    ssh "$RPI_SSH" bash -s "$RPI_INSTALL_DIR" << 'RESTORE_SCRIPT'
        INSTALL_DIR="$1"

        # Restore binary
        echo "  Restoring binary..."
        cd "$INSTALL_DIR/target/release"
        if [ -f "kr_notebook.old" ]; then
            cp "kr_notebook.old" "kr_notebook"
            chmod +x "kr_notebook"
            echo "    Binary restored"
        fi

        # Restore static assets
        echo "  Restoring static assets..."
        if [ -d "$INSTALL_DIR/static.old" ]; then
            rm -rf "$INSTALL_DIR/static"
            cp -r "$INSTALL_DIR/static.old" "$INSTALL_DIR/static"
            echo "    Static assets restored"
        fi

        # Restore databases
        echo "  Restoring databases..."
        BACKUP_TS=$(cat "$INSTALL_DIR/backups/latest")
        BACKUP_DIR="$INSTALL_DIR/backups/$BACKUP_TS"

        if [ -f "$BACKUP_DIR/app.db" ]; then
            cp "$BACKUP_DIR/app.db" "$INSTALL_DIR/data/app.db"
            echo "    app.db restored"
        fi

        if [ -d "$BACKUP_DIR/users" ]; then
            for user_backup in "$BACKUP_DIR/users"/*/; do
                if [ -d "$user_backup" ]; then
                    username=$(basename "$user_backup")
                    if [ -f "$user_backup/learning.db" ]; then
                        mkdir -p "$INSTALL_DIR/data/users/$username"
                        cp "$user_backup/learning.db" "$INSTALL_DIR/data/users/$username/"
                        echo "    users/$username/learning.db restored"
                    fi
                fi
            done
        fi
RESTORE_SCRIPT

    # Start service
    echo ""
    echo "[5/5] Starting service..."
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
    echo "=== Rollback Complete ==="
    exit 0
fi

# =============================================================================
# NORMAL DEPLOYMENT MODE
# =============================================================================
echo "=== Deploying to $RPI_SSH ==="
echo ""

# Step 1: Run tests
if [ "$DO_TESTS" = true ]; then
    echo "[1/9] Running tests..."
    echo ""
    step_start
    TEST_ARGS="all"
    if [ "$TEST_FAIL_FAST" = true ]; then
        TEST_ARGS="all --fail-fast"
    fi
    if ! "$SCRIPT_DIR/test.sh" $TEST_ARGS; then
        echo ""
        echo "Error: Tests failed. Deployment aborted."
        echo "Fix the failing tests or use --no-tests to skip (not recommended)."
        exit 1
    fi
    step_end "Tests"
    echo ""
else
    echo "[1/9] Skipping tests (--no-tests)"
    echo ""
fi

# Step 2: Build
if [ "$DO_BUILD" = true ]; then
    echo "[2/9] Building for $TARGET ($PROFILE_NAME)..."
    step_start

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

    # Build WASM module for offline study (if crate exists)
    if [ -d "$PROJECT_DIR/crates/offline-srs" ]; then
        echo "  Building WASM module..."
        if command -v wasm-pack &>/dev/null; then
            cd "$PROJECT_DIR/crates/offline-srs"
            wasm-pack build --target web --release --out-dir "$PROJECT_DIR/static/wasm" --out-name offline_srs 2>/dev/null || {
                echo "  Warning: WASM build failed, using existing files if present"
            }
            cd "$PROJECT_DIR"
        else
            echo "  Warning: wasm-pack not installed, skipping WASM build"
            echo "  Install with: cargo install wasm-pack"
            if [ ! -f "$PROJECT_DIR/static/wasm/offline_srs_bg.wasm" ]; then
                echo "  Error: No existing WASM files found. Install wasm-pack and rebuild."
            fi
        fi
    fi
    step_end "Build"
    echo ""
else
    echo "[2/9] Skipping build (--no-build)"
fi

# Check binary exists
if [ ! -f "$BINARY" ]; then
    echo "Error: Binary not found at $BINARY"
    echo "Run without --no-build first"
    exit 1
fi

# Step 2: Test SSH connection
echo "[3/9] Testing SSH connection..."
step_start
if ! ssh -o ConnectTimeout=5 "$RPI_SSH" "echo 'OK'" &>/dev/null; then
    echo "Error: Cannot connect to $RPI_SSH"
    echo "Check RPI_SSH in .rpi-deploy.conf"
    exit 1
fi
echo "  Connected to $(ssh "$RPI_SSH" 'hostname')"

# Step 2b: Check database schema compatibility
echo ""
echo "[3b/9] Checking database schema..."
SCHEMA_CHECK=$(ssh "$RPI_SSH" bash -s "$RPI_INSTALL_DIR" << 'SCHEMA_SCRIPT'
    INSTALL_DIR="$1"
    APP_DB="$INSTALL_DIR/data/app.db"

    if [ ! -f "$APP_DB" ]; then
        echo "NEW"
        exit 0
    fi

    # Check for card_definitions table (required by modular_content branch)
    # Find sqlite3 binary
    SQLITE3=""
    for path in /usr/bin/sqlite3 /usr/local/bin/sqlite3 /bin/sqlite3; do
        if [ -x "$path" ]; then
            SQLITE3="$path"
            break
        fi
    done
    # Also try command lookup
    if [ -z "$SQLITE3" ]; then
        SQLITE3=$(command -v sqlite3 2>/dev/null || true)
    fi
    if [ -z "$SQLITE3" ] || [ ! -x "$SQLITE3" ]; then
        echo "SKIP"
        exit 0
    fi

    HAS_CARD_DEFS=$($SQLITE3 "$APP_DB" "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='card_definitions';" 2>/dev/null || echo "0")

    # Check for db_version table
    HAS_VERSION=$($SQLITE3 "$APP_DB" "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='db_version';" 2>/dev/null || echo "0")

    if [ "$HAS_CARD_DEFS" = "0" ]; then
        # Get schema version if available
        if [ "$HAS_VERSION" = "1" ]; then
            VERSION=$($SQLITE3 "$APP_DB" "SELECT MAX(version) FROM db_version;" 2>/dev/null || echo "0")
            echo "OLD:$VERSION"
        else
            echo "OLD:0"
        fi
    else
        VERSION=$($SQLITE3 "$APP_DB" "SELECT MAX(version) FROM db_version;" 2>/dev/null || echo "unknown")
        echo "OK:$VERSION"
    fi
SCHEMA_SCRIPT
)

case "$SCHEMA_CHECK" in
    NEW)
        echo "  Fresh install (no existing database)"
        ;;
    SKIP)
        echo "  Skipped (sqlite3 not available on remote)"
        ;;
    OLD:*)
        OLD_VERSION="${SCHEMA_CHECK#OLD:}"
        echo ""
        echo "  ⚠️  WARNING: Production database has OLD schema (version $OLD_VERSION)"
        echo "  The new binary uses card_definitions table which doesn't exist."
        echo ""
        echo "  This deployment will trigger a migration that:"
        echo "    1. Creates card_definitions table in app.db"
        echo "    2. Seeds baseline card definitions"
        echo "    3. Migrates user progress from legacy cards table"
        echo ""
        echo "  The migration matches cards by (main_answer, card_type, tier, is_reverse)."
        echo "  A backup will be created before deployment."
        echo ""
        read -p "  Continue with deployment? [y/N] " -n 1 -r
        echo ""
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            echo "  Deployment cancelled."
            exit 1
        fi
        ;;
    OK:*)
        CURRENT_VERSION="${SCHEMA_CHECK#OK:}"
        echo "  Schema version: $CURRENT_VERSION (compatible)"
        ;;
    *)
        echo "  Warning: Could not check schema (${SCHEMA_CHECK})"
        ;;
esac
step_end "Pre-flight checks"

# Step 3: Backup on RPi
if [ "$DO_BACKUP" = true ]; then
    echo ""
    echo "[4/9] Creating backup on RPi..."
    step_start
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
    step_end "Backup"
else
    echo ""
    echo "[4/9] Skipping backup (--no-backup)"
fi

# Step 4: Stop service
echo ""
echo "[5/9] Stopping service..."
step_start
ssh "$RPI_SSH" bash -s "$RPI_SERVICE" << 'STOP_SCRIPT'
    SERVICE="$1"
    if systemctl is-active --quiet "$SERVICE" 2>/dev/null; then
        sudo systemctl stop "$SERVICE"
        echo "  Stopped $SERVICE"
    else
        echo "  Service not running"
    fi
STOP_SCRIPT
step_end "Stop service"

# Step 5: Deploy binary
echo ""
echo "[6/9] Deploying binary..."
step_start
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
step_end "Deploy binary"

# Step 6: Deploy static assets (backup old first)
echo ""
echo "[7/9] Syncing static assets..."
step_start
ssh "$RPI_SSH" bash -s "$RPI_INSTALL_DIR" << 'STATIC_BACKUP_SCRIPT'
    INSTALL_DIR="$1"
    # Backup current static assets (overwrite previous .old)
    if [ -d "$INSTALL_DIR/static" ]; then
        rm -rf "$INSTALL_DIR/static.old"
        cp -r "$INSTALL_DIR/static" "$INSTALL_DIR/static.old"
        echo "  Backed up static/ -> static.old/"
    fi
STATIC_BACKUP_SCRIPT
rsync -av --delete --exclude='.DS_Store' "$PROJECT_DIR/static/" "$RPI_SSH:$RPI_INSTALL_DIR/static/"
step_end "Sync static assets"

# Step 7: Update backup marker
echo ""
echo "[8/9] Updating backup marker..."
step_start
ssh "$RPI_SSH" bash -s "$RPI_INSTALL_DIR" "$TIMESTAMP" << 'MARKER_SCRIPT'
    INSTALL_DIR="$1"
    TIMESTAMP="$2"
    mkdir -p "$INSTALL_DIR/backups"
    echo "$TIMESTAMP" > "$INSTALL_DIR/backups/latest"
    echo "  Backup marker set to: $TIMESTAMP"
MARKER_SCRIPT
step_end "Update backup marker"

# Step 8: Start service
echo ""
echo "[9/9] Starting service..."
step_start
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
step_end "Start service"

echo ""
echo "=== Deploy Complete ==="
print_timing_summary
echo ""
echo "To rollback (restores binary, static assets, and databases):"
echo "  ./scripts/rpi-deploy.sh --rollback"
