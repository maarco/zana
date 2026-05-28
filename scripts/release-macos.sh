#!/bin/bash
set -euo pipefail

echo "=== Zana macOS Release Build ==="

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_ROOT"

if [ -z "${APPLE_SIGNING_IDENTITY:-${APPLE_DEVELOPER_ID:-}}" ]; then
    echo "error: APPLE_SIGNING_IDENTITY is required for release DMGs"
    echo "example: export APPLE_SIGNING_IDENTITY=\"Developer ID Application: Name (TEAMID)\""
    exit 1
fi

if [ -z "${APPLE_ID:-}" ] || [ -z "${APPLE_APP_PASSWORD:-}" ] || [ -z "${APPLE_TEAM_ID:-}" ]; then
    echo "error: notarization requires APPLE_ID, APPLE_APP_PASSWORD, and APPLE_TEAM_ID"
    exit 1
fi

cargo fmt --all -- --check
cargo check -p Zana-app --locked
cargo test -p Zana-app --locked
cargo clippy -p Zana-app --all-targets --all-features --locked -- -D warnings

"$PROJECT_ROOT/scripts/build-macos.sh" --universal
"$PROJECT_ROOT/scripts/sign-and-notarize.sh"

echo "=== Zana macOS Release Build Complete ==="
