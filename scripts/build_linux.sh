#!/usr/bin/env bash
#
# build_linux.sh — Linux release build (EXPERIMENTAL)
#
# Delegates to build_release.sh. Linux artifacts are automatically
# marked with "(EXPERIMENTAL)" in filenames and metadata.

set -euo pipefail
cd "$(dirname "$0")/.."
exec ./scripts/build_release.sh "$@"
