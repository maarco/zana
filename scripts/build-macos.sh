#!/bin/bash
set -e

# kVoice macOS Build Script
# Builds a universal binary (Intel + Apple Silicon)

echo "=== kVoice macOS Build ==="

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

# Get script directory
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_ROOT"

# Check for Rust targets
echo -e "${YELLOW}Checking Rust targets...${NC}"
if ! rustup target list --installed | grep -q "aarch64-apple-darwin"; then
    echo "Adding aarch64-apple-darwin target..."
    rustup target add aarch64-apple-darwin
fi

if ! rustup target list --installed | grep -q "x86_64-apple-darwin"; then
    echo "Adding x86_64-apple-darwin target..."
    rustup target add x86_64-apple-darwin
fi

# Parse arguments
BUILD_TYPE="release"
UNIVERSAL=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --debug)
            BUILD_TYPE="debug"
            shift
            ;;
        --universal)
            UNIVERSAL=true
            shift
            ;;
        *)
            shift
            ;;
    esac
done

if [ "$UNIVERSAL" = true ]; then
    echo -e "${YELLOW}Building universal binary...${NC}"

    # Build for Apple Silicon
    echo "Building for Apple Silicon (aarch64)..."
    cargo tauri build --target aarch64-apple-darwin

    # Build for Intel
    echo "Building for Intel (x86_64)..."
    cargo tauri build --target x86_64-apple-darwin

    # Create universal binary
    echo "Creating universal binary..."
    ARM_APP="target/aarch64-apple-darwin/release/bundle/macos/kVoice.app"
    INTEL_APP="target/x86_64-apple-darwin/release/bundle/macos/kVoice.app"
    UNIVERSAL_APP="target/universal-apple-darwin/release/bundle/macos/kVoice.app"

    mkdir -p "$(dirname "$UNIVERSAL_APP")"
    cp -R "$ARM_APP" "$UNIVERSAL_APP"

    # Merge binaries with lipo
    lipo -create \
        "$ARM_APP/Contents/MacOS/kVoice" \
        "$INTEL_APP/Contents/MacOS/kVoice" \
        -output "$UNIVERSAL_APP/Contents/MacOS/kVoice"

    echo -e "${GREEN}Universal binary created at: $UNIVERSAL_APP${NC}"
else
    echo -e "${YELLOW}Building for current architecture...${NC}"

    if [ "$BUILD_TYPE" = "debug" ]; then
        cargo tauri build --debug
    else
        cargo tauri build
    fi

    echo -e "${GREEN}Build complete!${NC}"
fi

# Show output location
echo ""
echo "=== Build Output ==="
if [ "$UNIVERSAL" = true ]; then
    ls -la target/universal-apple-darwin/release/bundle/macos/
    ls -la target/universal-apple-darwin/release/bundle/dmg/ 2>/dev/null || true
else
    ARCH=$(uname -m)
    if [ "$ARCH" = "arm64" ]; then
        TARGET_DIR="target/aarch64-apple-darwin/release"
    else
        TARGET_DIR="target/x86_64-apple-darwin/release"
    fi
    ls -la "$TARGET_DIR/bundle/macos/" 2>/dev/null || ls -la target/release/bundle/macos/
fi
