#!/bin/bash
set -e

# Zana macOS Code Signing and Notarization Script
# Requires Apple Developer account and certificates

echo "=== Zana Code Signing & Notarization ==="

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

# Configuration - Set these environment variables or modify here
DEVELOPER_ID="${APPLE_DEVELOPER_ID:-}"           # "Developer ID Application: Your Name (TEAMID)"
APPLE_ID="${APPLE_ID:-}"                          # Your Apple ID email
APP_PASSWORD="${APPLE_APP_PASSWORD:-}"            # App-specific password from appleid.apple.com
TEAM_ID="${APPLE_TEAM_ID:-}"                      # Your 10-character Team ID

# Get script directory
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Find the app bundle
APP_PATH=""
if [ -d "$PROJECT_ROOT/target/universal-apple-darwin/release/bundle/macos/Zana.app" ]; then
    APP_PATH="$PROJECT_ROOT/target/universal-apple-darwin/release/bundle/macos/Zana.app"
elif [ -d "$PROJECT_ROOT/target/release/bundle/macos/Zana.app" ]; then
    APP_PATH="$PROJECT_ROOT/target/release/bundle/macos/Zana.app"
elif [ -d "$PROJECT_ROOT/target/aarch64-apple-darwin/release/bundle/macos/Zana.app" ]; then
    APP_PATH="$PROJECT_ROOT/target/aarch64-apple-darwin/release/bundle/macos/Zana.app"
else
    echo -e "${RED}Error: Could not find Zana.app bundle${NC}"
    echo "Run ./scripts/build-macos.sh first"
    exit 1
fi

echo "App bundle: $APP_PATH"

# Check for required credentials
if [ -z "$DEVELOPER_ID" ]; then
    echo -e "${YELLOW}Warning: APPLE_DEVELOPER_ID not set${NC}"
    echo "Set environment variable: export APPLE_DEVELOPER_ID=\"Developer ID Application: Your Name (TEAMID)\""
    echo ""
    echo "Available signing identities:"
    security find-identity -v -p codesigning
    exit 1
fi

# Sign the app bundle
sign_app() {
    echo -e "${YELLOW}Signing app bundle...${NC}"

    # Sign frameworks and helpers first
    find "$APP_PATH/Contents/Frameworks" -type f -name "*.dylib" -exec \
        codesign --force --options runtime --sign "$DEVELOPER_ID" \
        --entitlements "$PROJECT_ROOT/src-tauri/entitlements.plist" {} \; 2>/dev/null || true

    # Sign the main executable
    codesign --force --options runtime --sign "$DEVELOPER_ID" \
        --entitlements "$PROJECT_ROOT/src-tauri/entitlements.plist" \
        "$APP_PATH/Contents/MacOS/Zana"

    # Sign the entire app bundle
    codesign --force --deep --options runtime --sign "$DEVELOPER_ID" \
        --entitlements "$PROJECT_ROOT/src-tauri/entitlements.plist" \
        "$APP_PATH"

    # Verify signature
    echo "Verifying signature..."
    codesign --verify --deep --strict --verbose=2 "$APP_PATH"

    echo -e "${GREEN}App signed successfully!${NC}"
}

# Create DMG
create_dmg() {
    echo -e "${YELLOW}Creating DMG...${NC}"

    DMG_PATH="${APP_PATH%%.app}.dmg"
    TEMP_DMG="/tmp/Zana-temp.dmg"

    # Remove existing
    rm -f "$DMG_PATH" "$TEMP_DMG"

    # Create DMG
    hdiutil create -srcfolder "$APP_PATH" -volname "Zana" -fs HFS+ \
        -fsargs "-c c=64,a=16,e=16" -format UDRW "$TEMP_DMG"

    # Convert to compressed DMG
    hdiutil convert "$TEMP_DMG" -format UDZO -imagekey zlib-level=9 -o "$DMG_PATH"
    rm -f "$TEMP_DMG"

    # Sign the DMG
    codesign --force --sign "$DEVELOPER_ID" "$DMG_PATH"

    echo -e "${GREEN}DMG created: $DMG_PATH${NC}"
}

# Notarize the app
notarize_app() {
    if [ -z "$APPLE_ID" ] || [ -z "$APP_PASSWORD" ] || [ -z "$TEAM_ID" ]; then
        echo -e "${YELLOW}Skipping notarization - credentials not set${NC}"
        echo "Set: APPLE_ID, APPLE_APP_PASSWORD, APPLE_TEAM_ID"
        return
    fi

    echo -e "${YELLOW}Submitting for notarization...${NC}"

    DMG_PATH="${APP_PATH%%.app}.dmg"

    # Submit for notarization
    xcrun notarytool submit "$DMG_PATH" \
        --apple-id "$APPLE_ID" \
        --password "$APP_PASSWORD" \
        --team-id "$TEAM_ID" \
        --wait

    # Staple the notarization ticket
    echo "Stapling notarization ticket..."
    xcrun stapler staple "$DMG_PATH"

    echo -e "${GREEN}Notarization complete!${NC}"
}

# Main
sign_app
create_dmg
notarize_app

echo ""
echo "=== Done ==="
echo "Distributable DMG: ${APP_PATH%%.app}.dmg"
