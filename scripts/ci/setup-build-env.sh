#!/usr/bin/env bash
# Native build prerequisites for the current runner, dispatched on RUNNER_OS so the
# workflows carry one step instead of a per-OS `if:` ladder. Mirrors ensure-ffmpeg.sh.
#
# Linux needs the GTK/WebKit stack plus CMake; macOS and Windows runners ship both.

set -euo pipefail

script_dir="$(cd "$(dirname "$0")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"

case "${RUNNER_OS:-${RECAST_BUILD_OS:-}}" in
Linux)
	echo "::group::Linux system dependencies"
	bash "$repo_root/scripts/release/install-linux-deps.sh"
	echo "::endgroup::"
	# transcribe.cpp builds ggml from source, so it needs CMake and a C/C++ compiler.
	if ! command -v cmake >/dev/null 2>&1; then
		echo "::group::CMake"
		sudo apt-get install -y cmake
		echo "::endgroup::"
	fi
	;;
macOS | Windows)
	echo "$RUNNER_OS runner ships the GTK-free toolchain and CMake; nothing to install."
	;;
*)
	echo "::error::setup-build-env.sh: unsupported RUNNER_OS: ${RUNNER_OS:-unset}" >&2
	echo "Set RECAST_BUILD_OS=Windows|macOS|Linux to force-dispatch." >&2
	exit 2
	;;
esac
