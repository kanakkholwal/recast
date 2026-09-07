#!/usr/bin/env bash
# Emits the pinned Rust channel from rust-toolchain.toml as a GITHUB_OUTPUT `channel`.
# dtolnay/rust-toolchain does not read that file, and installing `@stable` while cargo
# resolves the pinned channel adds targets and components to the wrong toolchain.

set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
channel=$(grep -m1 -oE 'channel[[:space:]]*=[[:space:]]*"[^"]+"' "$root/rust-toolchain.toml" |
	grep -oE '"[^"]+"' | tr -d '"')

if [ -z "$channel" ]; then
	echo "::error::no channel found in rust-toolchain.toml" >&2
	exit 1
fi

echo "channel=$channel" >>"${GITHUB_OUTPUT:-/dev/stdout}"
echo "pinned toolchain: $channel" >&2
