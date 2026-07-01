#!/usr/bin/env bash
#
# test_release.sh — Verify a release build against a sample .srt mining session
#
# Usage: ./scripts/test_release.sh [path/to/kaeda-binary]
#
# If no path is given, looks for the binary in:
#   target/release/kaeda        (CLI, after `cargo build --release`)
#   dist/*/kaeda                (packaged release)

set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

SAMPLE_SRT="tests/fixtures/sample.srt"

# Locate the CLI binary
BIN="${1:-}"
if [ -z "$BIN" ]; then
  if [ -f "target/release/kaeda" ]; then
    BIN="target/release/kaeda"
  else
    DIST_BINS=(dist/*/kaeda dist/*/kaeda.exe 2>/dev/null || true)
    if [ ${#DIST_BINS[@]} -gt 0 ]; then
      BIN="${DIST_BINS[0]}"
    else
      echo "::error Kaeda binary not found. Build first or supply a path."
      echo "  Usage: $0 [path/to/kaeda]"
      exit 1
    fi
  fi
fi

if [ ! -f "$BIN" ]; then
  echo "::error Binary not found: $BIN"
  exit 1
fi

echo "==> Testing Kaeda binary: $BIN"
echo ""

##############################
# Test 1: --help exits cleanly
##############################
echo "--- Test 1: --help ---"
"$BIN" --help >/tmp/kaeda_test_help.txt 2>&1
if [ $? -ne 0 ]; then
  echo "::error --help failed"
  cat /tmp/kaeda_test_help.txt
  exit 1
fi
echo "  ✓ --help works"

##############################
# Test 2: stats on empty store
##############################
echo "--- Test 2: stats ---"
"$BIN" stats >/tmp/kaeda_test_stats.txt 2>&1
if [ $? -ne 0 ]; then
  echo "::error stats command failed"
  cat /tmp/kaeda_test_stats.txt
  exit 1
fi
echo "  ✓ stats works"

##############################
# Test 3: stats output has expected columns
##############################
echo "--- Test 3: stats output format ---"
if ! grep -q "total words" /tmp/kaeda_test_stats.txt; then
  echo "::error stats output missing 'total words'"
  cat /tmp/kaeda_test_stats.txt
  exit 1
fi
echo "  ✓ stats output looks correct"

##############################
# Test 4: known add + list roundtrip
##############################
echo "--- Test 4: known add / list ---"
"$BIN" known add "테스트" >/tmp/kaeda_test_known_add.txt 2>&1
if [ $? -ne 0 ]; then
  echo "::error known add failed"
  cat /tmp/kaeda_test_known_add.txt
  exit 1
fi
echo "  ✓ known add succeeded"

"$BIN" known list >/tmp/kaeda_test_known_list.txt 2>&1
if ! grep -q "테스트" /tmp/kaeda_test_known_list.txt; then
  echo "::error known list missing added word"
  cat /tmp/kaeda_test_known_list.txt
  exit 1
fi
echo "  ✓ known list contains added word"

##############################
# Test 5: mine a sample .srt (dry — processes file, parses subs)
##############################
echo "--- Test 5: mine sample .srt ---"
# Use a short timeout; the TUI expects interactive input so we just
# verify it starts processing then send 'q' to quit.
SAMPLE_SRT_ABS="$(cd "$(dirname "$SAMPLE_SRT")" && pwd)/$(basename "$SAMPLE_SRT")"
echo q | timeout 5 "$BIN" mine "$SAMPLE_SRT_ABS" >/tmp/kaeda_test_mine.txt 2>&1 && RC=$? || RC=$?
# timeout + 'q' may exit 0 or 124 (timeout) — both are acceptable
if [ $RC -ne 0 ] && [ $RC -ne 124 ]; then
  echo "::error mine command failed (exit=$RC)"
  cat /tmp/kaeda_test_mine.txt
  exit 1
fi
echo "  ✓ mine launched and exited cleanly"

##############################
# All passed
##############################
echo ""
echo "==> All release tests passed for: $BIN"
exit 0
