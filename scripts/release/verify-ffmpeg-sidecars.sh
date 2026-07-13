#!/usr/bin/env bash
# Verify that the bundled FFmpeg sidecar exists, is executable, and ships every
# encoder AND filter the export pipeline needs.
#
# Catches the failure mode where the FFmpeg download step "succeeded" but the
# resulting binary is missing pieces the export depends on. Without this check,
# a release would bundle a half-functional FFmpeg and users would hit cryptic
# errors on their first export.
#
# Encoders and filters are gated by SEPARATE `--enable-` flags at FFmpeg build
# time, so a binary can pass the encoder check and still fail an export. Caption
# burn-in needs the libass `ass` filter, and a build without `--enable-libass`
# drops it while keeping every encoder below; the export then dies with
# `No such filter: 'ass'`. Both lists are therefore asserted.
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

fail=0

encoders=$("$bin" -hide_banner -encoders)
for codec in libx264 aac libvpx-vp9 libopus; do
  if ! echo "$encoders" | grep -q " $codec "; then
    echo "::error::Required encoder missing from bundled ffmpeg: $codec"
    fail=1
  fi
done

# Library-gated filters only. Built-in filters (overlay, scale, crop, ...) are
# always compiled in, so they'd never catch a bad build; these three are the ones
# an upstream can silently drop:
#   ass, subtitles: libass, caption burn-in
#   drawtext:       libfreetype, used by the OCR test harness
filters=$("$bin" -hide_banner -filters)
for filter in ass subtitles drawtext; do
  # Filter rows are `<flags> <name> <in>-><out> <description>`; anchoring on the
  # name plus the arrow avoids matching the word inside a description.
  if ! echo "$filters" | grep -qE "^\s*\S+\s+$filter\s+\S+->\S+"; then
    echo "::error::Required filter missing from bundled ffmpeg: $filter (was it built without libass/libfreetype?)"
    fail=1
  fi
done

if [[ "$fail" -ne 0 ]]; then
  echo "::error::Bundled FFmpeg is missing required capabilities; refusing to ship it."
  exit 1
fi

"$bin" -version | head -n1
