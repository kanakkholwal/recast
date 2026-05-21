#!/usr/bin/env bash
# Download FFmpeg + FFprobe for Linux x64 and place them as Tauri sidecars.
#
# johnvansickle.com ships a static glibc-compatible build that works on
# every distro we target (Ubuntu 22.04+, Fedora 36+, Debian 12+).
#
# Inputs:
#   $1 — Rust target triple (e.g. x86_64-unknown-linux-gnu)
#   $2 — destination directory for sidecars

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$REPO_ROOT"

triple="${1:?Rust target triple required as arg 1}"
dest="${2:?Destination directory required as arg 2}"

mkdir -p "$dest"
curl -L "https://johnvansickle.com/ffmpeg/releases/ffmpeg-release-amd64-static.tar.xz" -o ffmpeg.tar.xz
mkdir -p ffmpeg-extract
tar -xf ffmpeg.tar.xz -C ffmpeg-extract --strip-components=1
cp ffmpeg-extract/ffmpeg "$dest/ffmpeg-$triple"
cp ffmpeg-extract/ffprobe "$dest/ffprobe-$triple"
chmod +x "$dest/ffmpeg-$triple" "$dest/ffprobe-$triple"
rm -rf ffmpeg.tar.xz ffmpeg-extract
