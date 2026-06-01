#!/usr/bin/env bash
set -euo pipefail

REPO="${CRONBOX_REPO:-xiaoxiunique/cronbox}"
VERSION="${CRONBOX_VERSION:-latest}"
APP_NAME="${CRONBOX_APP_NAME:-CronBox}"
INSTALL_DIR="${CRONBOX_INSTALL_DIR:-/Applications}"
CLI_TARGET="${CRONBOX_CLI_TARGET:-}"
FORCE="${CRONBOX_FORCE:-0}"

TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/cronbox-install.XXXXXX")"
DMG_MOUNT=""

cleanup() {
  if [[ -n "$DMG_MOUNT" && -d "$DMG_MOUNT" ]]; then
    hdiutil detach "$DMG_MOUNT" -quiet >/dev/null 2>&1 || true
  fi
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT

fail() {
  echo "error: $*" >&2
  exit 1
}

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || fail "missing required command: $1"
}

if [[ "$(uname -s)" != "Darwin" ]]; then
  fail "this installer only supports macOS"
fi

need_cmd curl
need_cmd ditto
need_cmd find
need_cmd grep
need_cmd hdiutil
need_cmd sed
need_cmd tar

auth_args=()
if [[ -n "${GITHUB_TOKEN:-}" ]]; then
  auth_args=(-H "Authorization: Bearer ${GITHUB_TOKEN}")
fi

release_api_url() {
  if [[ "$VERSION" == "latest" ]]; then
    printf 'https://api.github.com/repos/%s/releases/latest\n' "$REPO"
  else
    printf 'https://api.github.com/repos/%s/releases/tags/%s\n' "$REPO" "$VERSION"
  fi
}

download_url_from_release() {
  local api_url urls preferred
  api_url="$(release_api_url)"
  urls="$(
    curl -fsSL \
      -H "Accept: application/vnd.github+json" \
      -H "User-Agent: cronbox-installer" \
      "${auth_args[@]}" \
      "$api_url" |
      sed -n 's/.*"browser_download_url"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p'
  )"

  preferred="$(printf '%s\n' "$urls" | grep -Ei '\.dmg($|\?)' | grep -Ei 'universal|mac|darwin|apple|CronBox' | head -n 1 || true)"
  if [[ -z "$preferred" ]]; then
    preferred="$(printf '%s\n' "$urls" | grep -Ei '\.app\.tar\.gz($|\?)' | grep -Ei 'universal|mac|darwin|apple|CronBox' | head -n 1 || true)"
  fi
  if [[ -z "$preferred" ]]; then
    preferred="$(printf '%s\n' "$urls" | grep -Ei '\.dmg($|\?)|\.app\.tar\.gz($|\?)' | head -n 1 || true)"
  fi

  [[ -n "$preferred" ]] || fail "could not find a macOS .dmg or .app.tar.gz asset in ${api_url}"
  printf '%s\n' "$preferred"
}

run_install_cmd() {
  local probe="$INSTALL_DIR"
  while [[ ! -e "$probe" && "$probe" != "/" ]]; do
    probe="$(dirname "$probe")"
  done

  if [[ -w "$probe" ]]; then
    "$@"
  else
    sudo "$@"
  fi
}

install_app() {
  local source_app="$1"
  local dest_app="${INSTALL_DIR}/${APP_NAME}.app"

  [[ -d "$source_app" ]] || fail "app bundle not found: $source_app"

  run_install_cmd mkdir -p "$INSTALL_DIR"
  if [[ -e "$dest_app" ]]; then
    run_install_cmd rm -rf "$dest_app"
  fi
  run_install_cmd ditto "$source_app" "$dest_app"
  echo "installed app: $dest_app"
}

install_from_dmg() {
  local asset="$1"
  local source_app

  DMG_MOUNT="${TMP_DIR}/mount"
  mkdir -p "$DMG_MOUNT"
  hdiutil attach "$asset" -nobrowse -readonly -mountpoint "$DMG_MOUNT" >/dev/null
  source_app="$(find "$DMG_MOUNT" -maxdepth 2 -type d -name "${APP_NAME}.app" -print -quit)"
  install_app "$source_app"
}

install_from_tarball() {
  local asset="$1"
  local extract_dir source_app

  extract_dir="${TMP_DIR}/extract"
  mkdir -p "$extract_dir"
  tar -xzf "$asset" -C "$extract_dir"
  source_app="$(find "$extract_dir" -maxdepth 4 -type d -name "${APP_NAME}.app" -print -quit)"
  install_app "$source_app"
}

choose_cli_target() {
  local dir
  for dir in /opt/homebrew/bin /usr/local/bin "${HOME}/.local/bin"; do
    if [[ -d "$dir" && -w "$dir" ]]; then
      printf '%s/cronbox\n' "$dir"
      return
    fi
  done
  printf '%s/.local/bin/cronbox\n' "$HOME"
}

install_cli_link() {
  local app_path="${INSTALL_DIR}/${APP_NAME}.app"
  local bin="${app_path}/Contents/MacOS/cronbox"
  local target="${CLI_TARGET:-$(choose_cli_target)}"
  local target_dir

  [[ -x "$bin" ]] || fail "CronBox binary not found at: $bin"
  target_dir="$(dirname "$target")"

  mkdir -p "$target_dir"
  if [[ -e "$target" || -L "$target" ]]; then
    if [[ -L "$target" || "$FORCE" == "1" ]]; then
      rm -f "$target"
    else
      echo "skipped cli link: $target already exists; set CRONBOX_FORCE=1 to replace it"
      return
    fi
  fi

  ln -s "$bin" "$target"
  echo "installed cli: $target -> $bin"
}

download_url="${CRONBOX_DOWNLOAD_URL:-$(download_url_from_release)}"
asset_name="$(basename "${download_url%%\?*}")"
asset_path="${TMP_DIR}/${asset_name}"

echo "downloading: $download_url"
curl -fL --progress-bar -o "$asset_path" "$download_url"

case "$asset_name" in
  *.dmg)
    install_from_dmg "$asset_path"
    ;;
  *.app.tar.gz)
    install_from_tarball "$asset_path"
    ;;
  *)
    fail "unsupported macOS asset: $asset_name"
    ;;
esac

install_cli_link

echo "CronBox installed."
echo "Open it with: open \"${INSTALL_DIR}/${APP_NAME}.app\""
