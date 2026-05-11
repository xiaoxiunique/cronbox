#!/usr/bin/env bash
set -euo pipefail

APP_PATH="${1:-/Applications/CronBox.app}"
TARGET="${2:-/usr/local/bin/cronbox}"
BIN="$APP_PATH/Contents/MacOS/cronbox"

if [[ ! -x "$BIN" ]]; then
  echo "CronBox binary not found at: $BIN" >&2
  echo "Usage: scripts/install-cli.sh [/path/to/CronBox.app] [/usr/local/bin/cronbox]" >&2
  exit 1
fi

mkdir -p "$(dirname "$TARGET")"
ln -sf "$BIN" "$TARGET"
echo "installed: $TARGET -> $BIN"
