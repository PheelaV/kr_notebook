#!/bin/bash
# Build the offline-srs WASM module for browser usage
#
# Prerequisites:
#   cargo install wasm-pack
#
# Output files go to static/wasm/:
#   - offline_srs_bg.wasm  (~200KB)
#   - offline_srs.js       (~10KB)

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
CRATE_DIR="$PROJECT_ROOT/crates/offline-srs"
OUTPUT_DIR="$PROJECT_ROOT/static/wasm"

echo "Building offline-srs WASM module..."
echo "  Crate: $CRATE_DIR"
echo "  Output: $OUTPUT_DIR"

# Check wasm-pack is installed
if ! command -v wasm-pack &> /dev/null; then
    echo "Error: wasm-pack not found. Install with: cargo install wasm-pack"
    exit 1
fi

# Build the WASM module
cd "$CRATE_DIR"
wasm-pack build --target web --release

# Create output directory
mkdir -p "$OUTPUT_DIR"

# Copy relevant files (wasm + js loader)
cp pkg/offline_srs_bg.wasm "$OUTPUT_DIR/"
cp pkg/offline_srs.js "$OUTPUT_DIR/"

# Report sizes
echo ""
echo "Build complete! Bundle sizes:"
ls -lh "$OUTPUT_DIR"/offline_srs*

# Estimate compressed size
if command -v gzip &> /dev/null; then
    WASM_SIZE=$(gzip -c "$OUTPUT_DIR/offline_srs_bg.wasm" | wc -c)
    JS_SIZE=$(gzip -c "$OUTPUT_DIR/offline_srs.js" | wc -c)
    TOTAL_KB=$(( (WASM_SIZE + JS_SIZE) / 1024 ))
    echo ""
    echo "Compressed (gzip): ~${TOTAL_KB}KB total"
fi

echo ""
echo "WASM module ready for browser use!"
