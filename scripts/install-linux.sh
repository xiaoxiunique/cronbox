#!/usr/bin/env bash
set -euo pipefail

REPO="${CRONBOX_REPO:-xiaoxiunique/cronbox}"
VERSION="${CRONBOX_VERSION:-latest}"
TARGET="${CRONBOX_CLI_TARGET:-${HOME}/.local/bin/cronbox}"

fail() {
  echo "error: $*" >&2
  exit 1
}

[[ "$(uname -s)" == "Linux" ]] || fail "this installer only supports Linux"

case "$(uname -m)" in
  x86_64 | amd64) ARCH="x86_64" ;;
  *) fail "no published CronBox binary for CPU architecture: $(uname -m)" ;;
esac

command -v curl >/dev/null 2>&1 || fail "missing required command: curl"
command -v tar >/dev/null 2>&1 || fail "missing required command: tar"
command -v systemctl >/dev/null 2>&1 || fail "systemd user services are required"

TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/cronbox-install.XXXXXX")"
trap 'rm -rf "$TMP_DIR"' EXIT

if [[ "$VERSION" == "latest" ]]; then
  API_URL="https://api.github.com/repos/${REPO}/releases/latest"
else
  API_URL="https://api.github.com/repos/${REPO}/releases/tags/${VERSION}"
fi

ASSET="CronBox-linux-${ARCH}.tar.gz"
DOWNLOAD_URL="${CRONBOX_DOWNLOAD_URL:-$(
  curl -fsSL -H "Accept: application/vnd.github+json" -H "User-Agent: cronbox-installer" "$API_URL" |
    grep -o '"browser_download_url"[[:space:]]*:[[:space:]]*"[^"]*"' |
    sed -n 's/.*"\([^"]*\)"$/\1/p' |
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
