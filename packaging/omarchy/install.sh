#!/usr/bin/env bash
set -euo pipefail

REPOSITORY="Porabuild/Poratake"
INSTALL_DIR="${PORATAKE_INSTALL_DIR:-$HOME/.local/opt/poratake}"
VERSION=""

usage() {
  cat <<'USAGE'
Install Poratake on Omarchy (Arch Linux, Wayland or X11).

Usage: install.sh [options]

Options:
  --version <x.y.z>   Install a specific release instead of the latest
  --install-dir <dir> Install to another directory
  -h, --help          Show this help

The installer downloads the release tarball, verifies its SHA-512 from the
release updater manifest, installs into your home directory, and registers a
desktop entry. No root access is required.
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --version)
      VERSION="${2:-}"
      shift 2
      ;;
    --install-dir)
      INSTALL_DIR="${2:-}"
      shift 2
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if [[ -z "$INSTALL_DIR" ]]; then
  echo "The install directory cannot be empty." >&2
  exit 1
fi

case "$(uname -m)" in
  x86_64 | amd64)
    ARCH="x64"
    ;;
  aarch64 | arm64)
    ARCH="arm64"
    ;;
  *)
    echo "Unsupported architecture: $(uname -m)" >&2
    exit 1
    ;;
esac

for tool in curl tar openssl; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "Missing required tool: $tool" >&2
    exit 1
  fi
done

for tool in ffmpeg wl-copy; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "Warning: $tool is missing; some capture, export, or clipboard features need it." >&2
  fi
done

if command -v pgrep >/dev/null 2>&1 && pgrep -x poratake-gpui >/dev/null 2>&1; then
  echo "Poratake is running. Quit it before installing." >&2
  exit 1
fi

if [[ -z "$VERSION" ]]; then
  RELEASE_JSON="$(curl -fsSL "https://api.github.com/repos/$REPOSITORY/releases/latest")"
  VERSION="$(printf '%s' "$RELEASE_JSON" | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"v\{0,1\}\([^"]*\)".*/\1/p' | head -n 1)"
  if [[ -z "$VERSION" ]]; then
    echo "Could not resolve the latest release version." >&2
    exit 1
  fi
fi

ASSET="Poratake-$VERSION-linux-$ARCH.tar.gz"
BASE_URL="https://github.com/$REPOSITORY/releases/download/v$VERSION"
MANIFEST_URL="$BASE_URL/latest-linux.yml"

TEMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/poratake-install-XXXXXX")"
STAGING=""
BACKUP=""
cleanup() {
  rm -rf "$TEMP_DIR"
  if [[ -n "$STAGING" ]]; then
    rm -rf "$STAGING"
  fi
  if [[ -n "$BACKUP" ]]; then
    rm -rf "$BACKUP"
  fi
}
trap cleanup EXIT

echo "Downloading Poratake $VERSION ($ARCH)..."
curl -fL --progress-bar "$BASE_URL/$ASSET" -o "$TEMP_DIR/$ASSET"
curl -fsSL "$MANIFEST_URL" -o "$TEMP_DIR/latest-linux.yml"

EXPECTED="$(awk -v asset="$ASSET" '
  $1 == "-" && $2 == "url:" {
    url = $3
    gsub(/[\047"]/, "", url)
    match_asset = (url == asset)
    next
  }
  match_asset && $1 == "sha512:" {
    value = $2
    gsub(/[\047"]/, "", value)
    print value
    exit
  }
' "$TEMP_DIR/latest-linux.yml")"

if [[ -z "$EXPECTED" ]]; then
  echo "The release manifest does not list $ASSET." >&2
  exit 1
fi

ACTUAL="$(openssl dgst -sha512 -binary "$TEMP_DIR/$ASSET" | openssl base64 -A)"
if [[ "$ACTUAL" != "$EXPECTED" ]]; then
  echo "Checksum verification failed for $ASSET." >&2
  exit 1
fi

if tar -tzf "$TEMP_DIR/$ASSET" | grep -E '(^/|(^|/)\.\./)' >/dev/null; then
  echo "The release archive contains an unsafe path." >&2
  exit 1
fi

PARENT_DIR="$(dirname "$INSTALL_DIR")"
mkdir -p "$PARENT_DIR"
STAGING="$PARENT_DIR/.poratake-install-$$"
rm -rf "$STAGING"
mkdir -p "$STAGING"
tar -xzf "$TEMP_DIR/$ASSET" -C "$STAGING" --no-same-owner
if [[ ! -x "$STAGING/poratake-gpui" ]]; then
  echo "The release archive is missing poratake-gpui." >&2
  exit 1
fi

BACKUP="$PARENT_DIR/.poratake-backup-$$"
rm -rf "$BACKUP"
if [[ -d "$INSTALL_DIR" ]]; then
  mv "$INSTALL_DIR" "$BACKUP"
fi
if ! mv "$STAGING" "$INSTALL_DIR"; then
  rm -rf "$INSTALL_DIR"
  if [[ -d "$BACKUP" ]]; then
    mv "$BACKUP" "$INSTALL_DIR"
  fi
  echo "Could not install Poratake to $INSTALL_DIR." >&2
  exit 1
fi
rm -rf "$BACKUP"

BIN_DIR="${XDG_BIN_HOME:-$HOME/.local/bin}"
mkdir -p "$BIN_DIR"
ln -sf "$INSTALL_DIR/poratake-gpui" "$BIN_DIR/poratake-gpui"

DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
APPS_DIR="$DATA_HOME/applications"
ICON_DIR="$DATA_HOME/icons/hicolor/256x256/apps"
mkdir -p "$APPS_DIR" "$ICON_DIR"

if [[ -f "$INSTALL_DIR/share/icons/hicolor/256x256/apps/poratake.png" ]]; then
  cp "$INSTALL_DIR/share/icons/hicolor/256x256/apps/poratake.png" "$ICON_DIR/poratake.png"
fi

DESKTOP_SOURCE="$INSTALL_DIR/share/applications/com.porabuild.poratake.desktop"
if [[ ! -f "$DESKTOP_SOURCE" ]]; then
  echo "The release archive is missing its desktop entry." >&2
  exit 1
fi
awk -v executable="$INSTALL_DIR/poratake-gpui" '
  /^Exec=/ { print "Exec=\"" executable "\" %U"; next }
  { print }
' "$DESKTOP_SOURCE" > "$APPS_DIR/com.porabuild.poratake.desktop"

if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "$APPS_DIR" >/dev/null 2>&1 || true
fi

echo
echo "Poratake $VERSION installed to $INSTALL_DIR"
echo "Launch it from the Omarchy application launcher, or run:"
echo "  $INSTALL_DIR/poratake-gpui"
echo
echo "Optional Hyprland bindings for ~/.config/hypr/bindings.conf:"
echo "  bindd = SUPER SHIFT, S, Poratake area capture, exec, $INSTALL_DIR/poratake-gpui --intent capture-area"
echo "  bindd = SUPER SHIFT, A, Poratake all-in-one, exec, $INSTALL_DIR/poratake-gpui --intent all-in-one"
echo "  bindd = SUPER SHIFT, H, Poratake history, exec, $INSTALL_DIR/poratake-gpui --intent history"
echo "  bindd = SUPER SHIFT, P, Poratake settings, exec, $INSTALL_DIR/poratake-gpui --intent open-settings"
echo
echo "Start on login is enabled on first launch; manage it in Settings > General."
