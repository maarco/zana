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

if git rev-parse "$TAG" >/dev/null 2>&1; then
    echo "error: tag already exists: $TAG"
    exit 1
fi

git tag -a "$TAG" -m "Release $TAG"
git push origin "$TAG"

echo "release tag pushed: $TAG"
echo "github actions will build, sign, notarize, and publish the release"
