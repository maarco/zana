#!/bin/bash
set -euo pipefail

if [ $# -ne 1 ]; then
    echo "usage: scripts/cut-release.sh vX.Y.Z"
    exit 1
fi

TAG="$1"

if [[ ! "$TAG" =~ ^v[0-9]+\.[0-9]+\.[0-9]+([-.][0-9A-Za-z.-]+)?$ ]]; then
    echo "error: release tag must look like v1.2.3, v1.2.3-rc.1, or v1.2.3-beta.1"
    exit 1
fi

if ! git remote get-url origin >/dev/null 2>&1; then
    echo "error: git remote 'origin' is required before cutting a release"
    exit 1
fi

git diff --quiet
git diff --cached --quiet

if [ -n "$(git status --porcelain)" ]; then
    echo "error: worktree must be clean before cutting a release"
    git status --short
    exit 1
fi

git fetch origin --tags

if [ "$(git rev-parse --abbrev-ref HEAD)" != main ] || [ "$(git rev-parse HEAD)" != "$(git rev-parse origin/main)" ]; then
    echo "error: release tags are cut from main matching origin/main"
    exit 1
fi

want="${TAG#v}"
cargo_version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
tauri_version="$(python3 -c 'import json; print(json.load(open("src-tauri/tauri.conf.json"))["version"])')"
if [ "$cargo_version" != "$want" ] || [ "$tauri_version" != "$want" ]; then
    echo "error: $TAG needs Cargo.toml ($cargo_version) and tauri.conf.json ($tauri_version) at $want"
    exit 1
fi

cargo metadata --locked --format-version 1 >/dev/null || {
    echo "error: Cargo.lock is out of date; run cargo check and commit Cargo.lock"
    exit 1
}

if git ls-remote --exit-code --tags origin "refs/tags/$TAG" >/dev/null 2>&1; then
    echo "tag already on origin: $TAG (re-dispatching the release for it)"
else
    if git rev-parse "$TAG" >/dev/null 2>&1; then
        echo "error: local tag $TAG exists but is not on origin; push or delete it first"
        exit 1
    fi
    git tag -a "$TAG" -m "Release $TAG"
    git push origin "$TAG"
    echo "release tag pushed: $TAG"
fi

# The Release workflow is a dispatch-only controller that runs from main.
gh workflow run release.yml --ref main -f tag="$TAG" -f signing="${SIGNING:-developer-id}"
echo "release workflow dispatched; watch it with: gh run watch \$(gh run list --workflow release.yml --limit 1 --json databaseId -q '.[0].databaseId')"
