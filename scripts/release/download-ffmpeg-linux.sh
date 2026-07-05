#!/usr/bin/env bash
# Download FFmpeg + FFprobe for Linux x64 and place them as Tauri sidecars.
#
# Primary source is BtbN/FFmpeg-Builds, hosted on GitHub's release CDN. Its
# linux64 builds link against an old glibc for broad compatibility (they run on
# every distro we target: Ubuntu 22.04+, Fedora 36+, Debian 12+) and, unlike
# johnvansickle.com, GitHub's CDN never throttles the Azure IP ranges the
# Actions runners live on. The `gpl` flavour bundles the encoders the export
# pipeline needs (libx264, aac, libvpx-vp9, libopus), which
# verify-ffmpeg-sidecars.sh asserts.
#
# johnvansickle.com is kept as a fallback. It serves an HTTP-200 HTML error
# page (not a 4xx) to cloud IPs when it throttles, so `curl --fail` can't catch
# it — every candidate is validated as a real tar.xz before we trust it.
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

# Ordered candidates. BtbN's `latest` tag only publishes the rolling `master`
# build for linux64-gpl (version-pinned builds live under dated tags it later
# prunes), so we track master — on par with the Windows/macOS scripts, which
# already pull the latest release. johnvansickle is the fallback: it ships a
# pinned release build and works off-CI where it isn't throttled.
urls=(
  "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-linux64-gpl.tar.xz"
  "https://johnvansickle.com/ffmpeg/releases/ffmpeg-release-amd64-static.tar.xz"
)

archive="ffmpeg.tar.xz"
got=""
for url in "${urls[@]}"; do
  echo "Fetching FFmpeg from $url"
  if ! curl -fSL --retry 3 --retry-delay 5 --connect-timeout 30 "$url" -o "$archive"; then
    echo "::warning::download failed for $url; trying next source"
    continue
  fi
  # Guard against throttled hosts that return a 200 HTML error page: only
  # accept the file if it actually parses as a tar.xz archive.
  if tar -tJf "$archive" >/dev/null 2>&1; then
    got="$url"
    break
  fi
  echo "::warning::$url returned $(wc -c <"$archive") bytes that are not a valid tar.xz; trying next source"
done

if [[ -z "$got" ]]; then
  echo "::error::could not fetch a valid FFmpeg archive from any source"
  exit 1
fi
echo "Using FFmpeg archive from $got"

rm -rf ffmpeg-extract
mkdir -p ffmpeg-extract
tar -xJf "$archive" -C ffmpeg-extract

# Locate the binaries by name so the script is agnostic to each mirror's
# layout (BtbN nests them under bin/, johnvansickle ships them at the root).
ff="$(find ffmpeg-extract -type f -name ffmpeg | head -n1)"
fp="$(find ffmpeg-extract -type f -name ffprobe | head -n1)"
if [[ -z "$ff" || -z "$fp" ]]; then
  echo "::error::ffmpeg/ffprobe not found in archive from $got"
  exit 1
fi

cp "$ff" "$dest/ffmpeg-$triple"
cp "$fp" "$dest/ffprobe-$triple"
chmod +x "$dest/ffmpeg-$triple" "$dest/ffprobe-$triple"
rm -rf "$archive" ffmpeg-extract
