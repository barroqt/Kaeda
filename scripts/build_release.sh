#!/usr/bin/env bash
#
# build_release.sh — Cross-platform release build for Kaeda
#
# Usage: ./scripts/build_release.sh [--skip-tauri]
#
# Produces versioned artifacts in dist/:
#   dist/kaeda-<version>-<target>/    — unpacked binaries
#   dist/kaeda-<version>-<target>.tar.gz  (or .zip on Windows)
#
# Linux artifacts get an "(EXPERIMENTAL)" suffix.

set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

##############################
# 1.  Metadata
##############################

VERSION="$(cargo metadata --format-version 1 --no-deps | \
  python3 -c 'import sys,json; print(json.load(sys.stdin)["packages"][0]["version"])')"

ARCH="$(uname -m)"
case "$(uname -s)" in
  Darwin)
    OS="macos"
    TARGET="${ARCH}-apple-darwin"
    ZIP="tar czf"
    EXT=".tar.gz"
    ;;
  Linux)
    OS="linux"
    TARGET="${ARCH}-unknown-linux-gnu"
    ZIP="tar czf"
    EXT=".tar.gz"
    ;;
  MINGW*|MSYS*|CYGWIN*)
    OS="windows"
    TARGET="${ARCH}-pc-windows-msvc"
    ZIP="zip -r"
    EXT=".zip"
    ;;
  *)
    echo "::error Unsupported OS: $(uname -s)"
    exit 1
    ;;
esac

OUTPUT_DIR="dist/kaeda-v${VERSION}-${TARGET}"
EXPERIMENTAL_SUFFIX=""
if [ "$OS" = "linux" ]; then
  EXPERIMENTAL_SUFFIX=" (EXPERIMENTAL)"
  echo "::notice Linux build is marked experimental"
fi

echo "==> Building Kaeda v${VERSION} for ${TARGET}${EXPERIMENTAL_SUFFIX}"

##############################
# 2.  Clean previous artifacts
##############################

echo "==> Cleaning previous artifacts"
rm -rf "${OUTPUT_DIR}" "${OUTPUT_DIR}${EXT}"
rm -f target/release/kaeda*
rm -rf app/src-tauri/target/release/bundle

##############################
# 3.  Build CLI binary
##############################

echo "==> Building CLI binary (cargo build --release)"
cargo build --release --bin kaeda

CLI_BIN="target/release/kaeda"
if [ "$OS" = "windows" ]; then
  CLI_BIN="${CLI_BIN}.exe"
fi

if [ ! -f "$CLI_BIN" ]; then
  echo "::error CLI binary not found at ${CLI_BIN}"
  exit 1
fi

##############################
# 4.  Build Tauri desktop app (unless skipped)
##############################

SKIP_TAURI="${1:+skip}"

if [ "$SKIP_TAURI" != "skip" ]; then
  echo "==> Installing JS dependencies"
  (cd app && pnpm install --frozen-lockfile 2>/dev/null || pnpm install)

  echo "==> Building Tauri desktop app"
  (cd app && cargo tauri build --bundles "$([ "$OS" = "macos" ] && echo 'dmg' || \
    [ "$OS" = "windows" ] && echo 'msi,nsi' || echo 'deb,appimage')")

  echo "==> Locating Tauri bundle"
  BUNDLE_DIR="app/src-tauri/target/release/bundle"
  case "$OS" in
    macos)
      TAURI_ARTIFACTS=$(ls "${BUNDLE_DIR}/dmg/"*.dmg 2>/dev/null || true)
      [ -z "$TAURI_ARTIFACTS" ] && TAURI_ARTIFACTS=$(ls "${BUNDLE_DIR}/macos/"*.app 2>/dev/null || true)
      ;;
    linux)
      TAURI_ARTIFACTS=$(ls "${BUNDLE_DIR}/deb/"*.deb 2>/dev/null || true)
      [ -z "$TAURI_ARTIFACTS" ] && TAURI_ARTIFACTS=$(ls "${BUNDLE_DIR}/appimage/"*.AppImage 2>/dev/null || true)
      ;;
    windows)
      TAURI_ARTIFACTS=$(ls "${BUNDLE_DIR}/msi/"*.msi 2>/dev/null || true)
      [ -z "$TAURI_ARTIFACTS" ] && TAURI_ARTIFACTS=$(ls "${BUNDLE_DIR}/nsis/"*.exe 2>/dev/null || true)
      ;;
  esac
fi

##############################
# 5.  Stage into dist/
##############################

echo "==> Staging artifacts → ${OUTPUT_DIR}/"
mkdir -p "${OUTPUT_DIR}"

# CLI binary
cp "${CLI_BIN}" "${OUTPUT_DIR}/"
echo "  ✓ CLI: $(basename "${CLI_BIN}")"

# Tauri bundle(s)
if [ "$SKIP_TAURI" != "skip" ] && [ -n "${TAURI_ARTIFACTS:-}" ]; then
  echo "$TAURI_ARTIFACTS" | while read -r artifact; do
    dest="${OUTPUT_DIR}/"
    if [ "$OS" = "linux" ]; then
      base="$(basename "$artifact")"
      base="${base%.*}"
      ext="${artifact##*.}"
      cp "$artifact" "${OUTPUT_DIR}/${base}${EXPERIMENTAL_SUFFIX}.${ext}"
    else
      cp "$artifact" "$dest"
    fi
    echo "  ✓ Tauri: $(basename "$artifact")"
  done
elif [ "$SKIP_TAURI" != "skip" ]; then
  echo "  ! No Tauri bundles found (build may have failed)"
fi

# Metadata
echo "kaeda v${VERSION} — ${TARGET}${EXPERIMENTAL_SUFFIX}" > "${OUTPUT_DIR}/VERSION"
echo "Built: $(date -u +%Y-%m-%dT%H:%M:%SZ)" >> "${OUTPUT_DIR}/VERSION"
echo "Commit: $(git rev-parse HEAD 2>/dev/null || echo unknown)" >> "${OUTPUT_DIR}/VERSION"
[ "$OS" = "linux" ] && echo "STATUS: EXPERIMENTAL — not recommended for production use" >> "${OUTPUT_DIR}/VERSION"

##############################
# 6.  Create archive
##############################

echo "==> Creating archive"
ARCHIVE_NAME="kaeda-v${VERSION}-${TARGET}${EXT}"
(cd dist && ${ZIP} "${ARCHIVE_NAME}" "kaeda-v${VERSION}-${TARGET}")

echo ""
echo "==> Done!"
echo "  Directory: dist/kaeda-v${VERSION}-${TARGET}/"
echo "  Archive:   dist/${ARCHIVE_NAME}"
echo "  Contents:"
ls -lh "dist/kaeda-v${VERSION}-${TARGET}/"
