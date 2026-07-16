#!/usr/bin/env bash
# Dispatch to the right per-OS FFmpeg sidecar downloader for the current
# GitHub-Actions runner. One step in the workflow instead of three
# per-OS branches. The per-OS scripts in this directory are unchanged —
# this is just the routing layer.
#
# Usage:
#   ensure-ffmpeg.sh <rust-target-triple> [dest-dir]
#
#   $RUST_TARGET : x86_64-pc-windows-msvc / x86_64-unknown-linux-gnu / …
#   dest-dir     : destination directory for the sidecars. Defaults to
#                  apps/desktop/src-tauri/binaries (matches the `tauri build`
#                  externalBin lookup).
#
# The existing per-OS scripts are intentionally not modified; this is the
# routing layer the YAML workflows call into so they don't need per-OS
# `if:` clauses.

set -euo pipefail

RUST_TARGET="${1:-}"
DEST="${2:-apps/desktop/src-tauri/binaries}"

# Resolve absolute paths so the pwsh call (Windows) sees the file the
# same way bash would. Re-anchors `dest` from the repo root (two levels up
# from this script) when a relative path is given.
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
case "$DEST" in
    /*) DEST_ABS="$DEST" ;;
    *)  DEST_ABS="$REPO_ROOT/$DEST" ;;
esac

case "${RUNNER_OS:-${RECAS_FFMPEG_OS:-}}" in
    Windows)
        # GitHub Actions' `windows-latest` runners do not have bash as the
        # default `shell:` for scripts; the workflow uses `shell: bash` for
        # cross-platform dispatch and lets bash hand off to `pwsh` here.
        # This mirrors how release-desktop.yml:215-240 + 280-290 chain.
        pwsh -File "$SCRIPT_DIR/download-ffmpeg-windows.ps1" \
            -RustTarget "$RUST_TARGET" \
            -Dest     "$DEST_ABS"
        ;;
    macOS)
        bash "$SCRIPT_DIR/download-ffmpeg-macos.sh" "$RUST_TARGET" "$DEST_ABS"
        ;;
    Linux)
        bash "$SCRIPT_DIR/download-ffmpeg-linux.sh" "$RUST_TARGET" "$DEST_ABS"
        ;;
    *)
        echo "ensure-ffmpeg.sh: unsupported RUNNER_OS: ${RUNNER_OS:-unset}" >&2
        echo "Set RECAS_FFMPEG_OS=Windows|macOS|Linux to force-dispatch."   >&2
        exit 2
        ;;
esac
