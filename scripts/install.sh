#!/usr/bin/env bash
set -euo pipefail

APP="cosmic-media-source-controller"
PREFIX="${PREFIX:-/usr/local}"
BINDIR="$PREFIX/bin"

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo is required. Install it first: sudo apt install cargo"
  exit 1
fi

if ! command -v playerctl >/dev/null 2>&1; then
  echo "playerctl is required. Install it first: sudo apt install playerctl"
  exit 1
fi

cargo build --release
sudo install -Dm755 "target/release/$APP" "$BINDIR/$APP"

echo "$APP installed to $BINDIR/$APP"
echo "Run: $APP list"
