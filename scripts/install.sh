#!/usr/bin/env bash
set -euo pipefail

APP="tihulu-media-source-controller"
REPO_URL="${REPO_URL:-https://github.com/Tihulu/tihulu-media-source-controller.git}"
BRANCH="${BRANCH:-main}"
PREFIX="${PREFIX:-/usr}"
BINDIR="$PREFIX/bin"
DESKTOP_DIR="$PREFIX/share/applications"
DESKTOP_FILE="com.github.tihulu.TihuluMediaSourceController.desktop"
OLD_LOCAL_DESKTOP="/usr/local/share/applications/$DESKTOP_FILE"

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
    sudo apt-get install -y \
      build-essential \
      pkg-config \
      git \
      cargo \
      playerctl \
      libnotify-bin \
      libx11-dev \
      libxi-dev \
      libxcursor-dev \
      libxrandr-dev \
      libxinerama-dev \
      libgl1-mesa-dev \
      libxkbcommon-dev \
      libwayland-dev
  else
    warn "Automatic dependency installation is only supported on apt-based systems."
    warn "Install these packages manually: git cargo playerctl libnotify-bin pkg-config libx11-dev libxi-dev libxcursor-dev libxrandr-dev libxinerama-dev libgl1-mesa-dev libxkbcommon-dev libwayland-dev"
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
    log "Cloning Tihulu Media Source Controller from GitHub"
    git clone --depth 1 --branch "$BRANCH" "$REPO_URL" "$workdir"
  fi

  cd "$workdir"

  log "Building release binary"
  cargo build --release

  log "Installing binary to $BINDIR/$APP"
  sudo install -Dm755 "target/release/$APP" "$BINDIR/$APP"

  if [ -f "packaging/$DESKTOP_FILE" ]; then
    log "Installing COSMIC applet desktop entry to $DESKTOP_DIR/$DESKTOP_FILE"
    sudo install -Dm644 "packaging/$DESKTOP_FILE" "$DESKTOP_DIR/$DESKTOP_FILE"
  fi

  if [ -f "$OLD_LOCAL_DESKTOP" ] && [ "$PREFIX" = "/usr" ]; then
    log "Removing old /usr/local desktop entry"
    sudo rm -f "$OLD_LOCAL_DESKTOP"
  fi

  if command -v update-desktop-database >/dev/null 2>&1; then
    sudo update-desktop-database "$DESKTOP_DIR" >/dev/null 2>&1 || true
  fi

  if [ -n "$cleanup_dir" ]; then
    rm -rf "$cleanup_dir"
  fi

  log "Installation complete"
  echo "Run desktop GUI: $APP"
  echo "CLI example: $APP set spotify && $APP play-pause"
  echo "COSMIC applet entry installed. If it does not appear immediately, restart COSMIC Panel or log out/in."
}

main "$@"
