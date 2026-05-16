#!/usr/bin/env bash
#
# One-shot local development setup for the Recast desktop app (macOS / Linux).
#
# Bootstraps a fresh machine so a contributor can build and run the Tauri
# desktop app. It:
#   1. Detects the OS + architecture and the Rust target triple.
#   2. Auto-installs missing toolchains: Node.js, Rust (rustup), and the
#      OS-level prerequisites Tauri needs (Xcode CLT / webkit2gtk + friends).
#   3. Enables pnpm through corepack (pinned to package.json#packageManager).
#   4. Installs the FFmpeg + ffprobe sidecar binaries into
#      apps/desktop/src-tauri/binaries/ with Tauri's target-triple names.
#   5. Runs `pnpm install` for the whole workspace.
#   6. Produces a debug build of the desktop app to verify the toolchain.
#
# Usage:
#   bash scripts/setup.sh [--skip-build] [--skip-toolchains]
#
set -euo pipefail

SKIP_BUILD=0
SKIP_TOOLCHAINS=0
for arg in "$@"; do
  case "$arg" in
    --skip-build)      SKIP_BUILD=1 ;;
    --skip-toolchains) SKIP_TOOLCHAINS=1 ;;
    *) echo "Unknown option: $arg" >&2; exit 1 ;;
  esac
done

# --- colours / logging ------------------------------------------------------

if [ -t 1 ]; then
  C_CYAN=$'\033[36m'; C_GREEN=$'\033[32m'; C_YELLOW=$'\033[33m'
  C_RED=$'\033[31m'; C_GRAY=$'\033[90m'; C_RESET=$'\033[0m'
else
  C_CYAN=; C_GREEN=; C_YELLOW=; C_RED=; C_GRAY=; C_RESET=
fi
step() { printf '\n%s==> %s%s\n' "$C_CYAN"  "$1" "$C_RESET"; }
ok()   { printf '%s    OK  %s%s\n' "$C_GREEN" "$1" "$C_RESET"; }
info() { printf '%s    %s%s\n'     "$C_GRAY"  "$1" "$C_RESET"; }
warn() { printf '%s    !   %s%s\n' "$C_YELLOW" "$1" "$C_RESET"; }
fail() { printf '\n%sFAILED: %s%s\n' "$C_RED" "$1" "$C_RESET" >&2; exit 1; }

has() { command -v "$1" >/dev/null 2>&1; }

# --- locate repo root -------------------------------------------------------

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
DESKTOP_DIR="$REPO_ROOT/apps/desktop"
BIN_DIR="$DESKTOP_DIR/src-tauri/binaries"

[ -f "$REPO_ROOT/pnpm-workspace.yaml" ] || fail "Could not locate the repo root."

# --- 1. detect platform / target triple ------------------------------------

step "Detecting platform"
OS="$(uname -s)"
ARCH="$(uname -m)"
case "$OS" in
  Darwin) PLATFORM=macos ;;
  Linux)  PLATFORM=linux ;;
  *) fail "Unsupported OS: $OS (this script handles macOS and Linux; use scripts/setup.ps1 on Windows)." ;;
esac
case "$ARCH" in
  x86_64|amd64) RUST_ARCH=x86_64 ;;
  arm64|aarch64) RUST_ARCH=aarch64 ;;
  *) fail "Unsupported architecture: $ARCH" ;;
esac
if [ "$PLATFORM" = macos ]; then
  TRIPLE="$RUST_ARCH-apple-darwin"
else
  TRIPLE="$RUST_ARCH-unknown-linux-gnu"
fi
ok "$PLATFORM / $ARCH  ->  target triple: $TRIPLE"

echo "Recast desktop — local setup ($PLATFORM)"
info "Repo: $REPO_ROOT"

# --- 2. toolchains ----------------------------------------------------------

step "Checking toolchains"
[ "$SKIP_TOOLCHAINS" -eq 1 ] && warn "--skip-toolchains set; only verifying."

install_macos_prereqs() {
  # Xcode Command Line Tools — provides clang/git needed to compile Rust.
  if ! xcode-select -p >/dev/null 2>&1; then
    info "Installing Xcode Command Line Tools (a GUI prompt may appear) ..."
    xcode-select --install || true
    fail "Finish the Xcode Command Line Tools install, then re-run this script."
  fi
  ok "Xcode Command Line Tools"
  # Homebrew — used to install node + ffmpeg.
  if ! has brew; then
    if [ "$SKIP_TOOLCHAINS" -eq 1 ]; then
      fail "Homebrew not found. Install it from https://brew.sh/"
    fi
    info "Installing Homebrew ..."
    /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
    # Make brew available in this shell for both Apple Silicon and Intel.
    for p in /opt/homebrew/bin/brew /usr/local/bin/brew; do
      [ -x "$p" ] && eval "$("$p" shellenv)"
    done
  fi
  has brew || fail "Homebrew installed but not on PATH; open a new terminal and re-run."
  ok "Homebrew"
}

install_linux_prereqs() {
  # Tauri v2 system dependencies for Linux (webkit2gtk 4.1 + friends).
  if has apt-get; then
    info "Installing Tauri system dependencies via apt ..."
    sudo apt-get update
    sudo apt-get install -y \
      libwebkit2gtk-4.1-dev build-essential curl wget file \
      libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev \
      libgtk-3-dev xz-utils
  elif has dnf; then
    info "Installing Tauri system dependencies via dnf ..."
    sudo dnf install -y webkit2gtk4.1-devel openssl-devel curl wget file \
      libappindicator-gtk3-devel librsvg2-devel gtk3-devel libxdo-devel \
      xz @development-tools
  elif has pacman; then
    info "Installing Tauri system dependencies via pacman ..."
    sudo pacman -Syu --needed --noconfirm webkit2gtk-4.1 base-devel curl \
      wget file openssl appmenu-gtk-module libappindicator-gtk3 librsvg \
      gtk3 xdotool xz
  else
    warn "Unknown Linux package manager — install Tauri's prerequisites manually:"
    warn "https://v2.tauri.app/start/prerequisites/#linux"
  fi
}

if [ "$SKIP_TOOLCHAINS" -eq 0 ]; then
  if [ "$PLATFORM" = macos ]; then
    install_macos_prereqs
  else
    install_linux_prereqs
  fi
fi

# Node.js
if has node; then
  ok "Node.js $(node --version)"
elif [ "$SKIP_TOOLCHAINS" -eq 1 ]; then
  fail "Node.js not found. Install Node.js LTS (v18+) from https://nodejs.org/"
else
  info "Installing Node.js ..."
  if [ "$PLATFORM" = macos ]; then
    brew install node
  elif has apt-get; then
    curl -fsSL https://deb.nodesource.com/setup_lts.x | sudo -E bash -
    sudo apt-get install -y nodejs
  elif has dnf; then
    sudo dnf install -y nodejs
  elif has pacman; then
    sudo pacman -S --needed --noconfirm nodejs npm
  fi
  has node || fail "Node.js installed but not on PATH; open a new terminal and re-run."
  ok "Node.js $(node --version)"
fi

# Rust
if has rustc; then
  ok "Rust $(rustc --version)"
elif [ "$SKIP_TOOLCHAINS" -eq 1 ]; then
  fail "Rust not found. Install it from https://rustup.rs/"
else
  info "Installing Rust (rustup) ..."
  curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs | sh -s -- -y
  # shellcheck disable=SC1090,SC1091
  [ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
  has rustc || fail "Rust installed but not on PATH; open a new terminal and re-run."
  ok "Rust $(rustc --version)"
fi

# pnpm via corepack (version resolved from packageManager in package.json).
step "Enabling pnpm (corepack)"
export COREPACK_ENABLE_DOWNLOAD_PROMPT=0
if ! has pnpm; then
  # corepack enable writes shims next to `node`; that dir may need sudo.
  corepack enable pnpm 2>/dev/null \
    || sudo corepack enable pnpm 2>/dev/null \
    || npm install -g pnpm 2>/dev/null \
    || sudo npm install -g pnpm \
    || true
  hash -r 2>/dev/null || true
fi
has pnpm || fail "pnpm not available after corepack. Run 'corepack enable pnpm' in a new terminal, then re-run."
ok "pnpm $(cd "$REPO_ROOT" && pnpm --version)"

# --- 3. FFmpeg + ffprobe sidecars ------------------------------------------

step "Setting up FFmpeg sidecar binaries"
FFMPEG_DST="$BIN_DIR/ffmpeg-$TRIPLE"
FFPROBE_DST="$BIN_DIR/ffprobe-$TRIPLE"

if [ -f "$FFMPEG_DST" ] && [ -f "$FFPROBE_DST" ]; then
  ok "Sidecars already present in src-tauri/binaries/"
else
  mkdir -p "$BIN_DIR"
  if [ "$PLATFORM" = macos ]; then
    # macOS: Homebrew's ffmpeg works for local dev (it links against dylibs
    # under the Homebrew prefix, which exist on the contributor's machine).
    # Release builds use self-contained sidecars produced by CI.
    has brew || fail "Homebrew required to install ffmpeg on macOS."
    if ! brew list ffmpeg >/dev/null 2>&1; then
      info "Installing ffmpeg via Homebrew ..."
      brew install ffmpeg
    fi
    BREW_PREFIX="$(brew --prefix)"
    cp "$BREW_PREFIX/bin/ffmpeg"  "$FFMPEG_DST"
    cp "$BREW_PREFIX/bin/ffprobe" "$FFPROBE_DST"
  else
    # Linux: BtbN publishes static, self-contained gpl builds.
    case "$RUST_ARCH" in
      x86_64)  FF_ARCH=linux64 ;;
      aarch64) FF_ARCH=linuxarm64 ;;
    esac
    URL="https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-$FF_ARCH-gpl.tar.xz"
    TMP="$(mktemp -d)"
    trap 'rm -rf "$TMP"' EXIT
    info "Downloading $URL"
    curl -fsSL "$URL" -o "$TMP/ffmpeg.tar.xz"
    info "Extracting ..."
    tar -xJf "$TMP/ffmpeg.tar.xz" -C "$TMP"
    FFMPEG_SRC="$(find "$TMP" -type f -name ffmpeg  | head -n1)"
    FFPROBE_SRC="$(find "$TMP" -type f -name ffprobe | head -n1)"
    [ -n "$FFMPEG_SRC" ] && [ -n "$FFPROBE_SRC" ] \
      || fail "Could not find ffmpeg / ffprobe inside the downloaded archive."
    cp "$FFMPEG_SRC"  "$FFMPEG_DST"
    cp "$FFPROBE_SRC" "$FFPROBE_DST"
  fi
  chmod +x "$FFMPEG_DST" "$FFPROBE_DST"
  ok "ffmpeg-$TRIPLE"
  ok "ffprobe-$TRIPLE"
fi

# --- 4. install workspace dependencies -------------------------------------

step "Installing workspace dependencies (pnpm install)"
( cd "$REPO_ROOT" && pnpm install ) || fail "pnpm install failed."
ok "Dependencies installed"

# --- 5. debug build ---------------------------------------------------------

if [ "$SKIP_BUILD" -eq 1 ]; then
  step "Skipping build (--skip-build)"
else
  step "Building the desktop app (debug — this takes a few minutes)"
  ( cd "$REPO_ROOT" && pnpm --filter recast-desktop run build:debug ) \
    || fail "Desktop debug build failed."
  ok "Debug build complete"
fi

# --- done -------------------------------------------------------------------

printf '\n%sSetup complete.%s\n' "$C_GREEN" "$C_RESET"
echo "Start the desktop app in dev mode with:"
printf '%s    pnpm --filter recast-desktop dev%s\n\n' "$C_CYAN" "$C_RESET"
