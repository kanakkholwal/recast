#!/usr/bin/env bash
# Build the GitHub Release body markdown for a Recast release tag.
#
# Inputs (env vars):
#   TAG       — release tag (e.g. v1.2.3)
#   PREV_TAG  — previous release tag for the compare link (may be empty)
#   REPO      — owner/repo slug (e.g. kanakkholwal/recast)
#
# Outputs:
#   - Writes the assembled markdown to the file path in $1, or a mktemp file.
#   - When $GITHUB_OUTPUT is set, also writes:
#       found=true|false   — whether CHANGELOG.md had a section for this tag
#       body=<multiline>   — the full body as a heredoc output
#
# Release pages are skim targets, not docs. Five sections and out:
#   1. One-line product intro + platform status
#   2. What's new (curated CHANGELOG excerpt, falls back to GH auto-notes)
#   3. Downloads table
#   4. Permissions (one place, all platforms)
#   5. Per-platform install steps + system requirements
#
# The full diff compare link is included only when a curated CHANGELOG
# section was found (otherwise GitHub auto-generates it under
# "What's Changed").

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$REPO_ROOT"

: "${TAG:?TAG is required}"
: "${REPO:?REPO is required}"
PREV_TAG="${PREV_TAG:-}"

body_file="${1:-$(mktemp)}"
version="${TAG#v}"

# 1. Intro + platform status.
{
  echo "**Recast** is an offline-first screen recorder and editor. Record, edit"
  echo "(zoom, cursor smoothing, captions, annotations), and export entirely on"
  echo "your machine. Nothing is uploaded unless you choose to share it."
  echo
  echo "> **Platform status:** Windows is stable. macOS (Apple Silicon &"
  echo "> Intel) and Linux are in **beta**."
  echo
} > "$body_file"

# 2. What's new — curated CHANGELOG section, falls back to GH auto-notes.
found="false"
if node scripts/extract-changelog.mjs "$version" --out changelog-section.md > /dev/null 2>&1; then
  found="true"
  {
    echo "## What's new in ${TAG}"
    echo
    cat changelog-section.md
    echo
    if [[ -n "${PREV_TAG}" ]]; then
      echo "**Full changelog:** https://github.com/${REPO}/compare/${PREV_TAG}...${TAG}"
      echo
    fi
  } >> "$body_file"
else
  echo "::warning::No CHANGELOG.md section for ${version}; falling back to auto-generated notes."
fi

# 3. Downloads.
{
  echo "## Downloads"
  echo
  echo "| Platform | Asset |"
  echo "| --- | --- |"
  echo "| Windows (x64) | \`recast_${version}_x64-setup.exe\` |"
  echo "| macOS Apple Silicon — beta | \`recast_${version}_aarch64.dmg\` |"
  echo "| macOS Intel — beta | \`recast_${version}_x64.dmg\` |"
  echo "| Linux (AppImage) — beta | \`recast_${version}_amd64.AppImage\` |"
  echo "| Linux (Debian / Ubuntu) — beta | \`recast_${version}_amd64.deb\` |"
  echo
} >> "$body_file"

# 4. Permissions — one section, all platforms.
{
  echo "## Permissions"
  echo
  echo "Recast asks for each of these at first launch. Grant only the ones"
  echo "you record from:"
  echo
  echo "- **Screen Recording** — required to capture your screen."
  echo "- **Microphone** — required if you record voiceover."
  echo "- **Camera** — required if you record from a webcam."
  echo
  echo "macOS surfaces these in System Settings → Privacy & Security; Linux"
  echo "and Windows behave per distro / shell and appear in the runtime"
  echo "permission dialogs."
  echo
} >> "$body_file"

# 5. Install steps per platform + system requirements.
{
  echo "## Installation"
  echo
  echo "### Windows"
  echo
  echo "1. Download \`recast_${version}_x64-setup.exe\` above."
  echo "2. Run the installer."
  echo "3. WebView2 Runtime is bundled — no extra setup needed."
  echo "4. First launch prompts for Screen Recording permission."
  echo
  echo "### macOS — beta"
  echo
  echo "Homebrew picks the right build for your chip and skips Gatekeeper:"
  echo
  echo '```sh'
  echo "brew install --cask kanakkholwal/recast/recast"
  echo '```'
  echo
  echo "Manual install: download the DMG, drag Recast to Applications, then"
  echo "strip the quarantine attribute once (the build is unsigned):"
  echo
  echo '```sh'
  echo "xattr -dr com.apple.quarantine /Applications/Recast.app"
  echo '```'
  echo
  echo "First launch prompts for Screen Recording, Microphone, and Camera."
  echo
  echo "### Linux — beta"
  echo
  echo "**AppImage:**"
  echo
  echo '```sh'
  echo "chmod +x recast_${version}_amd64.AppImage && ./recast_${version}_amd64.AppImage"
  echo '```'
  echo
  echo "**Debian / Ubuntu:**"
  echo
  echo '```sh'
  echo "sudo dpkg -i recast_${version}_amd64.deb && sudo apt -f install"
  echo '```'
  echo
  echo "PipeWire 0.3+ is recommended for system audio capture."
  echo
  echo "## System requirements"
  echo
  echo "- **Windows:** Windows 10 1809+ or Windows 11, x64."
  echo "- **macOS:** macOS 13 Ventura or later."
  echo "- **Linux:** glibc 2.35+ (Ubuntu 22.04, Fedora 36, Debian 12 or newer)."
  echo
} >> "$body_file"

# Surface state to GitHub Actions when running in CI.
if [[ -n "${GITHUB_OUTPUT:-}" ]]; then
  echo "found=${found}" >> "$GITHUB_OUTPUT"
  {
    echo "body<<__BODY_EOF__"
    cat "$body_file"
    echo "__BODY_EOF__"
  } >> "$GITHUB_OUTPUT"
fi

echo "Release body written to: $body_file"
