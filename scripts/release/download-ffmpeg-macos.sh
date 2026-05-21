#!/usr/bin/env bash
# Download FFmpeg + FFprobe for macOS and place them as Tauri sidecars.
#
# evermeet.cx ships universal2 binaries that work on both arm64 and x64
# Macs; we copy the same binary into BOTH triple-specific sidecar names
# so the Tauri bundler never misses a sidecar when it expects the other
# triple (the runner arch vs the requested target can disagree at
# bundle time).
#
# Inputs:
#   $1 — Rust target triple (e.g. aarch64-apple-darwin)
#   $2 — destination directory for sidecars (e.g. apps/desktop/src-tauri/binaries)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$REPO_ROOT"

triple="${1:?Rust target triple required as arg 1}"
dest="${2:?Destination directory required as arg 2}"

mkdir -p "$dest"
curl -L "https://evermeet.cx/ffmpeg/getrelease/ffmpeg/zip" -o ffmpeg.zip
curl -L "https://evermeet.cx/ffmpeg/getrelease/ffprobe/zip" -o ffprobe.zip
unzip -o ffmpeg.zip -d ffmpeg-extract
unzip -o ffprobe.zip -d ffmpeg-extract

# Write the requested triple's sidecar first.
cp ffmpeg-extract/ffmpeg "$dest/ffmpeg-$triple"
cp ffmpeg-extract/ffprobe "$dest/ffprobe-$triple"
# Also write both common macOS triples so the bundler never misses a
# sidecar when it expects the other triple. The universal2 binary
# satisfies both arches.
cp ffmpeg-extract/ffmpeg "$dest/ffmpeg-aarch64-apple-darwin" || true
cp ffmpeg-extract/ffprobe "$dest/ffprobe-aarch64-apple-darwin" || true
cp ffmpeg-extract/ffmpeg "$dest/ffmpeg-x86_64-apple-darwin" || true
cp ffmpeg-extract/ffprobe "$dest/ffprobe-x86_64-apple-darwin" || true
chmod +x "$dest"/ffmpeg-* "$dest"/ffprobe-* || true
rm -rf ffmpeg.zip ffprobe.zip ffmpeg-extract
