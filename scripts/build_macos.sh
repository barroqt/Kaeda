#!/usr/bin/env bash
#
# build_macos.sh — macOS release build (delegates to build_release.sh)

set -euo pipefail
cd "$(dirname "$0")/.."
exec ./scripts/build_release.sh "$@"
