@echo off
REM build_windows.bat — Windows release build (delegates to build_release.sh)
REM
REM Requires: Rust, Node.js, pnpm, and Tauri system dependencies.
REM
REM Usage: scripts\build_windows.bat

cd /d "%~dp0.."
bash scripts/build_release.sh %*
