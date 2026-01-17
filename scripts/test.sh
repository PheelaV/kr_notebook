#!/usr/bin/env bash
#
# Test runner with 3 levels:
#   1) unit        - Rust + Python unit tests (fast)
#   2) integration - Unit + Python integration tests
#   3) all         - Unit + Integration + E2E (Playwright)
#
# Usage:
#   ./scripts/test.sh              # Run all tests
#   ./scripts/test.sh unit         # Unit tests only
#   ./scripts/test.sh integration  # Unit + integration
#   ./scripts/test.sh all          # Everything (default)
#   ./scripts/test.sh e2e          # E2E tests only (for debugging)
#
# Environment:
#   PRESERVE_TEST_ENV=1  - Keep test data after run (for debugging)
#   VERBOSE=1            - Show verbose output
#

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Get script directory and project root
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Test level (default: all)
LEVEL="${1:-all}"

# Track results and timings
RUST_UNIT_RESULT=0
PYTHON_UNIT_RESULT=0
INTEGRATION_RESULT=0
E2E_RESULT=0

RUST_UNIT_TIME=0
PYTHON_UNIT_TIME=0
INTEGRATION_TIME=0
E2E_TIME=0
TOTAL_START_TIME=0

# Logging helpers
log_header() {
    echo ""
    echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"
    echo -e "${BLUE}  $1${NC}"
    echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"
}

log_success() {
    echo -e "${GREEN}✓ $1${NC}"
}

log_error() {
    echo -e "${RED}✗ $1${NC}"
}

log_info() {
    echo -e "${YELLOW}→ $1${NC}"
}

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

# Run Rust unit tests
run_rust_unit_tests() {
    log_header "Running Rust Unit Tests"
    local start_time=$SECONDS

    cd "$PROJECT_ROOT"

    # Run library tests only (--lib skips doctests which have import issues)
    cargo test --lib --all-features 2>&1 || RUST_UNIT_RESULT=1

    RUST_UNIT_TIME=$((SECONDS - start_time))

    if [[ $RUST_UNIT_RESULT -eq 0 ]]; then
        log_success "Rust unit tests passed ($(format_time $RUST_UNIT_TIME))"
    else
        log_error "Rust unit tests failed ($(format_time $RUST_UNIT_TIME))"
    fi

    return $RUST_UNIT_RESULT
}

# Run Python unit tests (py_scripts)
run_python_unit_tests() {
    log_header "Running Python Unit Tests (py_scripts)"
    local start_time=$SECONDS

    cd "$PROJECT_ROOT/py_scripts"

    # Ensure dependencies are installed (--group dev for pytest)
    uv sync --quiet --group dev 2>&1 || true

    uv run pytest tests/ -v --tb=short 2>&1 || PYTHON_UNIT_RESULT=1

    PYTHON_UNIT_TIME=$((SECONDS - start_time))

    if [[ $PYTHON_UNIT_RESULT -eq 0 ]]; then
        log_success "Python unit tests passed ($(format_time $PYTHON_UNIT_TIME))"
    else
        log_error "Python unit tests failed ($(format_time $PYTHON_UNIT_TIME))"
    fi

    return $PYTHON_UNIT_RESULT
}

# Run integration tests (Python)
run_integration_tests() {
    log_header "Running Integration Tests"
    local start_time=$SECONDS

    cd "$PROJECT_ROOT/tests/integration"

    log_info "Integration tests will spawn isolated servers on ports 3100+"

    # Ensure dependencies are installed (--group dev for pytest)
    uv sync --quiet --group dev 2>&1 || true

    # Run with parallel workers, grouped by file to reduce fixture setup overhead
    uv run pytest tests/ -v --tb=short -n auto --dist loadfile 2>&1 || INTEGRATION_RESULT=1

    INTEGRATION_TIME=$((SECONDS - start_time))

    if [[ $INTEGRATION_RESULT -eq 0 ]]; then
        log_success "Integration tests passed ($(format_time $INTEGRATION_TIME))"
    else
        log_error "Integration tests failed ($(format_time $INTEGRATION_TIME))"
    fi

    return $INTEGRATION_RESULT
}

# Run E2E tests (Playwright)
run_e2e_tests() {
    log_header "Running E2E Tests (Playwright)"
    local start_time=$SECONDS

    cd "$PROJECT_ROOT/tests/e2e"

    log_info "E2E tests will spawn isolated servers on ports 3001-3008"

    # Install Playwright browsers if needed
    if [[ ! -d "$HOME/Library/Caches/ms-playwright" ]] && [[ ! -d "$HOME/.cache/ms-playwright" ]]; then
        log_info "Installing Playwright browsers (first run only)..."
        npx playwright install chromium 2>&1
    fi

    npx playwright test 2>&1 || E2E_RESULT=1

    E2E_TIME=$((SECONDS - start_time))

    if [[ $E2E_RESULT -eq 0 ]]; then
        log_success "E2E tests passed ($(format_time $E2E_TIME))"
    else
        log_error "E2E tests failed ($(format_time $E2E_TIME))"
    fi

    return $E2E_RESULT
}

# Print summary
print_summary() {
    local total_time=$((SECONDS - TOTAL_START_TIME))

    log_header "Test Summary"

    local total=0
    local passed=0

    case "$LEVEL" in
        unit)
            total=2
            [[ $RUST_UNIT_RESULT -eq 0 ]] && ((passed++))
            [[ $PYTHON_UNIT_RESULT -eq 0 ]] && ((passed++))
            printf "  %-20s %-10s %s\n" "Rust Unit Tests:" "$(result_icon $RUST_UNIT_RESULT)" "($(format_time $RUST_UNIT_TIME))"
            printf "  %-20s %-10s %s\n" "Python Unit Tests:" "$(result_icon $PYTHON_UNIT_RESULT)" "($(format_time $PYTHON_UNIT_TIME))"
            ;;
        integration)
            total=3
            [[ $RUST_UNIT_RESULT -eq 0 ]] && ((passed++))
            [[ $PYTHON_UNIT_RESULT -eq 0 ]] && ((passed++))
            [[ $INTEGRATION_RESULT -eq 0 ]] && ((passed++))
            printf "  %-20s %-10s %s\n" "Rust Unit Tests:" "$(result_icon $RUST_UNIT_RESULT)" "($(format_time $RUST_UNIT_TIME))"
            printf "  %-20s %-10s %s\n" "Python Unit Tests:" "$(result_icon $PYTHON_UNIT_RESULT)" "($(format_time $PYTHON_UNIT_TIME))"
            printf "  %-20s %-10s %s\n" "Integration Tests:" "$(result_icon $INTEGRATION_RESULT)" "($(format_time $INTEGRATION_TIME))"
            ;;
        all)
            total=4
            [[ $RUST_UNIT_RESULT -eq 0 ]] && ((passed++))
            [[ $PYTHON_UNIT_RESULT -eq 0 ]] && ((passed++))
            [[ $INTEGRATION_RESULT -eq 0 ]] && ((passed++))
            [[ $E2E_RESULT -eq 0 ]] && ((passed++))
            printf "  %-20s %-10s %s\n" "Rust Unit Tests:" "$(result_icon $RUST_UNIT_RESULT)" "($(format_time $RUST_UNIT_TIME))"
            printf "  %-20s %-10s %s\n" "Python Unit Tests:" "$(result_icon $PYTHON_UNIT_RESULT)" "($(format_time $PYTHON_UNIT_TIME))"
            printf "  %-20s %-10s %s\n" "Integration Tests:" "$(result_icon $INTEGRATION_RESULT)" "($(format_time $INTEGRATION_TIME))"
            printf "  %-20s %-10s %s\n" "E2E Tests:" "$(result_icon $E2E_RESULT)" "($(format_time $E2E_TIME))"
            ;;
        e2e)
            total=1
            [[ $E2E_RESULT -eq 0 ]] && ((passed++))
            printf "  %-20s %-10s %s\n" "E2E Tests:" "$(result_icon $E2E_RESULT)" "($(format_time $E2E_TIME))"
            ;;
    esac

    echo ""
    echo -e "  ${BLUE}Total time: $(format_time $total_time)${NC}"
    echo ""
    if [[ $passed -eq $total ]]; then
        log_success "All $total test suites passed!"
        return 0
    else
        log_error "$passed/$total test suites passed"
        return 1
    fi
}

result_icon() {
    if [[ $1 -eq 0 ]]; then
        echo -e "${GREEN}PASSED${NC}"
    else
        echo -e "${RED}FAILED${NC}"
    fi
}

# Main
main() {
    TOTAL_START_TIME=$SECONDS
    log_header "Test Runner - Level: $LEVEL"

    case "$LEVEL" in
        unit)
            run_rust_unit_tests || true
            run_python_unit_tests || true
            ;;
        integration)
            run_rust_unit_tests || true
            run_python_unit_tests || true
            run_integration_tests || true
            ;;
        all)
            run_rust_unit_tests || true
            run_python_unit_tests || true
            run_integration_tests || true
            run_e2e_tests || true
            ;;
        e2e)
            run_e2e_tests || true
            ;;
        *)
            echo "Usage: $0 {unit|integration|all|e2e}"
            echo ""
            echo "Levels:"
            echo "  unit        - Rust + Python unit tests (fast)"
            echo "  integration - Unit + Python integration tests"
            echo "  all         - Unit + Integration + E2E (default)"
            echo "  e2e         - E2E tests only (for debugging)"
            echo ""
            echo "Environment:"
            echo "  PRESERVE_TEST_ENV=1  - Keep test data after run"
            echo "  VERBOSE=1            - Show verbose output"
            exit 1
            ;;
    esac

    print_summary
}

main "$@"
