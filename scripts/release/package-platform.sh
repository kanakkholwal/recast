#!/usr/bin/env bash
# Turns a finished `tauri build` into the release-assets/ directory for this runner,
# dispatched on RUNNER_OS so the workflow carries one step instead of three per-OS
# branches. Mirrors ensure-ffmpeg.sh; the per-OS scripts are unchanged.
#
# Usage: package-platform.sh <platform-name> <rust-target-triple>
# Env:   TAG (release tag) is required by the macOS and Windows legs.

set -euo pipefail

PLATFORM="${1:?platform name required}"
RUST_TARGET="${2:?rust target required}"

script_dir="$(cd "$(dirname "$0")" && pwd)"

case "${RUNNER_OS:-${RECAST_PACKAGE_OS:-}}" in
macOS)
	echo "::group::Tag macOS updater bundle with arch"
	RUST_TARGET="$RUST_TARGET" bash "$script_dir/rename-macos-updater-bundle.sh"
	echo "::endgroup::"
	;;
Windows)
	echo "::group::Package MSIX"
	pwsh -File "$script_dir/package-msix.ps1" -RustTarget "$RUST_TARGET"
	echo "::endgroup::"
	;;
Linux) ;;
*)
	echo "::error::package-platform.sh: unsupported RUNNER_OS: ${RUNNER_OS:-unset}" >&2
	echo "Set RECAST_PACKAGE_OS=Windows|macOS|Linux to force-dispatch." >&2
	exit 2
	;;
esac

echo "::group::Prepare release assets"
pwsh -File "$script_dir/prepare-release-assets.ps1" \
	-PlatformName "$PLATFORM" \
	-RustTarget "$RUST_TARGET"
echo "::endgroup::"

# A silently-empty release-assets/ would upload nothing and still pass the leg.
if [ -z "$(ls -A release-assets 2>/dev/null)" ]; then
	echo "::error::release-assets/ is empty after packaging $PLATFORM ($RUST_TARGET)" >&2
	exit 1
fi
