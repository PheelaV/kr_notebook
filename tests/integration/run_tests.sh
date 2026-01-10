#!/bin/bash
# Integration test runner for kr_notebook
#
# Usage:
#   ./run_tests.sh              # Run all tests
#   ./run_tests.sh -v           # Run with verbose output
#   ./run_tests.sh -k "auth"    # Run only auth tests
#
# Prerequisites:
#   - Server must be running at http://localhost:3000
#   - Start with: cargo run (in project root)

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

echo "=== kr_notebook Integration Tests ==="
echo "Project root: $PROJECT_ROOT"
echo "Test directory: $SCRIPT_DIR"
echo ""

# Check if server is running
echo "Checking server availability..."
if ! curl -s -o /dev/null -w "%{http_code}" http://localhost:3000/login | grep -q "200\|302"; then
    echo "ERROR: Server is not running at http://localhost:3000"
    echo "Please start the server first with: cargo run"
    exit 1
fi
echo "Server is running."
echo ""

# Change to test directory
cd "$SCRIPT_DIR"

# Ensure dependencies are installed
echo "Ensuring dependencies..."
uv sync --quiet

# Run tests
echo ""
echo "Running tests..."
echo "=================================="
uv run pytest "$@"
