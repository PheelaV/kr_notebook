#!/bin/bash
# Run this on the Raspberry Pi to discover system info
# Usage: ssh user@rpi 'bash -s' < scripts/rpi-discover.sh
# Or:    ./scripts/rpi-discover.sh  (if run directly on RPi)

set -e

echo "=== kr_notebook RPi Discovery ==="
echo ""

# System info
echo "[system]"
echo "hostname=$(hostname)"
echo "arch=$(uname -m)"
echo "kernel=$(uname -r)"
echo "os=$(cat /etc/os-release 2>/dev/null | grep ^PRETTY_NAME= | cut -d'"' -f2)"

# CPU info
echo ""
echo "[cpu]"
if [ -f /proc/cpuinfo ]; then
    echo "model=$(grep 'Model' /proc/cpuinfo | head -1 | cut -d':' -f2 | xargs)"
    echo "cores=$(grep -c ^processor /proc/cpuinfo)"
fi

# Memory
echo ""
echo "[memory]"
echo "total=$(free -h | awk '/^Mem:/ {print $2}')"
echo "available=$(free -h | awk '/^Mem:/ {print $7}')"

# Disk
echo ""
echo "[disk]"
echo "root_free=$(df -h / | awk 'NR==2 {print $4}')"

# Rust target
echo ""
echo "[rust_target]"
ARCH=$(uname -m)
case "$ARCH" in
    aarch64)
        echo "target=aarch64-unknown-linux-gnu"
        echo "linker=aarch64-linux-gnu-gcc"
        ;;
    armv7l)
        echo "target=armv7-unknown-linux-gnueabihf"
        echo "linker=arm-linux-gnueabihf-gcc"
        ;;
    armv6l)
        echo "target=arm-unknown-linux-gnueabihf"
        echo "linker=arm-linux-gnueabihf-gcc"
        ;;
    *)
        echo "target=unknown"
        echo "linker=unknown"
        ;;
esac

# Check for existing installation
echo ""
echo "[installation]"
if command -v kr_notebook &>/dev/null; then
    echo "binary_path=$(which kr_notebook)"
elif [ -f "$HOME/kr_notebook/kr_notebook" ]; then
    echo "binary_path=$HOME/kr_notebook/kr_notebook"
elif [ -f "/opt/kr_notebook/kr_notebook" ]; then
    echo "binary_path=/opt/kr_notebook/kr_notebook"
else
    echo "binary_path=not_found"
fi

# Check for systemd service
if systemctl list-unit-files | grep -q kr_notebook; then
    echo "service=systemd"
    echo "service_status=$(systemctl is-active kr_notebook 2>/dev/null || echo 'unknown')"
else
    echo "service=none"
fi

# Data directory
for data_dir in "$HOME/kr_notebook/data" "/opt/kr_notebook/data" "./data"; do
    if [ -d "$data_dir" ]; then
        echo "data_dir=$data_dir"
        break
    fi
done

# Network
echo ""
echo "[network]"
echo "ip=$(hostname -I 2>/dev/null | awk '{print $1}')"

echo ""
echo "=== Discovery Complete ==="
