#!/usr/bin/env bash
set -euo pipefail

APP="cosmic-media-source-controller"
REPO_URL="${REPO_URL:-https://github.com/Tihulu/cosmic-media-source-controller.git}"
BRANCH="${BRANCH:-main}"
PREFIX="${PREFIX:-/usr/local}"
BINDIR="$PREFIX/bin"
DESKTOP_DIR="$PREFIX/share/applications"
DESKTOP_FILE="com.github.tihulu.CosmicMediaSourceController.desktop"

log() {
  printf '\033[1;34m==>\033[0m %s\n' "$*"
}

warn() {
  printf '\033[1;33mWarning:\033[0m %s\n' "$*" >&2
}

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required command: $1" >&2
    exit 1
  fi
}

install_dependencies() {
  if command -v apt-get >/dev/null 2>&1; then
    log "Installing build and runtime dependencies"
    sudo apt-get update
    sudo apt-get install -y git cargo playerctl libnotify-bin
  else
    warn "Automatic dependency installation is only supported on apt-based systems."
    warn "Install these packages manually: git cargo playerctl libnotify-bin"
  fi
}

is_project_root() {
  [ -f "Cargo.toml" ] && grep -q "name = \"$APP\"" Cargo.toml
}

main() {
  install_dependencies

  need_cmd git
  need_cmd cargo
  need_cmd playerctl

  local workdir=""
  local cleanup_dir=""

  if is_project_root; then
    workdir="$PWD"
    log "Using current source tree: $workdir"
  else
    cleanup_dir="$(mktemp -d)"
    workdir="$cleanup_dir/$APP"
    log "Cloning $APP from GitHub"
    git clone --depth 1 --branch "$BRANCH" "$REPO_URL" "$workdir"
  fi

  cd "$workdir"

  log "Building release binary"
  cargo build --release

  log "Installing binary to $BINDIR/$APP"
  sudo install -Dm755 "target/release/$APP" "$BINDIR/$APP"

  if [ -f "packaging/$DESKTOP_FILE" ]; then
    log "Installing desktop entry"
    sudo install -Dm644 "packaging/$DESKTOP_FILE" "$DESKTOP_DIR/$DESKTOP_FILE"
  fi

  if [ -n "$cleanup_dir" ]; then
    rm -rf "$cleanup_dir"
  fi

  log "Installation complete"
  echo "Run: $APP list"
  echo "Example: $APP set spotify && $APP play-pause"
}

main "$@"
