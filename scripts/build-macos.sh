#!/bin/bash
set -euo pipefail

# Zana macOS Build Script
# Builds for Apple silicon only.

echo "=== Zana macOS Build ==="

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

# Get script directory
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_ROOT"

# Check for the Apple silicon Rust target.
echo -e "${YELLOW}Checking Rust targets...${NC}"
if ! rustup target list --installed | grep -q "aarch64-apple-darwin"; then
    echo "Adding aarch64-apple-darwin target..."
    rustup target add aarch64-apple-darwin
fi

# Parse arguments.
BUILD_TYPE="release"
while [[ $# -gt 0 ]]; do
    case "$1" in
        --debug)
            BUILD_TYPE="debug"
            shift
            ;;
        --help|-h)
            echo "Usage: $0 [--debug]"
            exit 0
            ;;
        *)
            echo -e "${RED}Unsupported argument: $1${NC}" >&2
            echo "Usage: $0 [--debug]" >&2
            exit 2
            ;;
    esac
done

BUILD_ARGS=(--target aarch64-apple-darwin)
if [[ "$BUILD_TYPE" == "debug" ]]; then
    BUILD_ARGS+=(--debug)
fi

echo -e "${YELLOW}Building for Apple silicon (aarch64)...${NC}"
cargo tauri build "${BUILD_ARGS[@]}"
echo -e "${GREEN}Build complete!${NC}"

# Show output location.
echo ""
echo "=== Build Output ==="
TARGET_DIR="target/aarch64-apple-darwin/$BUILD_TYPE"
if [[ -d "$TARGET_DIR/bundle/macos" ]]; then
    ls -la "$TARGET_DIR/bundle/macos/"
elif [[ -d "$TARGET_DIR/bundle/dmg" ]]; then
    ls -la "$TARGET_DIR/bundle/dmg/"
else
    echo -e "${RED}No macOS bundle found under $TARGET_DIR/bundle${NC}" >&2
    exit 1
fi
