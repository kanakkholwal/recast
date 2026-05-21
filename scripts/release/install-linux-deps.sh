#!/usr/bin/env bash
# Install the Ubuntu system packages required to build the Tauri desktop
# app on Linux.
#
# The `pipewire` Rust crate 0.9.x (transitive dep of `xcap`) generates
# bindings against PipeWire >= 1.4. Ubuntu 24.04 ships PipeWire 1.0.5,
# which omits fields like `spa_video_info_raw.flags` and changes
# `modifier` from u64 to i64 — producing the libspa build errors we
# hit. Pulling pipewire-upstream's PPA gives us the newer headers
# without locking the runner image to a future Ubuntu release.
#
# Shared by both `release-desktop.yml` and `ci-desktop.yml` so the
# package list cannot drift between the compile-gate CI and the actual
# release build.
#
# No arguments; runs apt against the current environment.

set -euo pipefail

sudo add-apt-repository -y ppa:pipewire-debian/pipewire-upstream
sudo apt-get update
sudo apt-get install -y \
  libwebkit2gtk-4.1-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  patchelf \
  libssl-dev \
  libgtk-3-dev \
  libgbm-dev \
  libpipewire-0.3-dev \
  libspa-0.2-dev
