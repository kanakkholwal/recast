#!/usr/bin/env bash
# Verify that the bundled FFmpeg sidecar exists, is executable, and
# ships every encoder the export pipeline needs.
#
# Catches the failure mode where the FFmpeg download step "succeeded"
# but the resulting binary is missing the libx264/aac/libvpx-vp9/libopus
# encoders — without this check, a release would bundle a half-functional
# FFmpeg and users would hit cryptic encoder-not-found errors on their
# first export.
#
# Inputs:
#   $1 — Rust target triple
#   $2 — destination / sidecars directory
#   $3 — runner OS string (matrix.platform.os; e.g. "windows-latest")

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$REPO_ROOT"

triple="${1:?Rust target triple required as arg 1}"
dest="${2:?Sidecars directory required as arg 2}"
os="${3:?Runner OS required as arg 3}"

if [[ "$os" == "windows-latest" ]]; then
  bin="$dest/ffmpeg-$triple.exe"
else
  bin="$dest/ffmpeg-$triple"
fi

[[ -x "$bin" ]] || {
  echo "::error::ffmpeg sidecar missing or not executable: $bin"
  exit 1
}

encoders=$("$bin" -hide_banner -encoders)
for codec in libx264 aac libvpx-vp9 libopus; do
  if ! echo "$encoders" | grep -q " $codec "; then
    echo "::error::Required encoder missing from bundled ffmpeg: $codec"
    exit 1
  fi
done

"$bin" -version | head -n1
