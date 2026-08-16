#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET="${1:-${HOME}/.local/bin/cronbox}"

cd "$ROOT_DIR"
bun run build
cargo build --release --manifest-path src-tauri/Cargo.toml

mkdir -p "$(dirname "$TARGET")"
install -m 755 src-tauri/target/release/cronbox "$TARGET"

echo "installed: $TARGET"
if [[ "$(uname -s)" == "Darwin" || "$(uname -s)" == "Linux" ]]; then
  "$TARGET" service install
else
  echo "start CronBox: $TARGET"
fi
