#!/usr/bin/env bash
set -euo pipefail

REPO="${CRONBOX_REPO:-xiaoxiunique/cronbox}"
VERSION="${CRONBOX_VERSION:-latest}"
TARGET="${CRONBOX_CLI_TARGET:-}"

fail() {
  echo "error: $*" >&2
  exit 1
}

[[ "$(uname -s)" == "Darwin" ]] || fail "this installer only supports macOS"

case "$(uname -m)" in
  arm64 | aarch64) ARCH="aarch64" ;;
  x86_64) ARCH="x86_64" ;;
  *) fail "unsupported CPU architecture: $(uname -m)" ;;
esac

if [[ -z "$TARGET" ]]; then
  for candidate in /opt/homebrew/bin/cronbox /usr/local/bin/cronbox; do
    if [[ -d "$(dirname "$candidate")" && -w "$(dirname "$candidate")" ]]; then
      TARGET="$candidate"
      break
    fi
  done
  TARGET="${TARGET:-${HOME}/.local/bin/cronbox}"
fi

TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/cronbox-install.XXXXXX")"
trap 'rm -rf "$TMP_DIR"' EXIT

if [[ "$VERSION" == "latest" ]]; then
  API_URL="https://api.github.com/repos/${REPO}/releases/latest"
else
  API_URL="https://api.github.com/repos/${REPO}/releases/tags/${VERSION}"
fi

ASSET="CronBox-macos-${ARCH}.tar.gz"
DOWNLOAD_URL="${CRONBOX_DOWNLOAD_URL:-$(
  curl -fsSL -H "Accept: application/vnd.github+json" -H "User-Agent: cronbox-installer" "$API_URL" |
    sed -n 's/.*"browser_download_url"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' |
    grep "/${ASSET}$" |
    head -n 1
)}"
[[ -n "$DOWNLOAD_URL" ]] || fail "release asset not found: $ASSET"

echo "downloading: $DOWNLOAD_URL"
curl -fL --progress-bar -o "$TMP_DIR/$ASSET" "$DOWNLOAD_URL"
tar -xzf "$TMP_DIR/$ASSET" -C "$TMP_DIR"
[[ -x "$TMP_DIR/cronbox" ]] || fail "archive does not contain cronbox"

mkdir -p "$(dirname "$TARGET")"
install -m 755 "$TMP_DIR/cronbox" "$TARGET"

echo "installed: $TARGET"
"$TARGET" service install
echo "CronBox is running at http://127.0.0.1:4317"
