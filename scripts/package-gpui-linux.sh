#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
VERSION="${1:-}"
OUTPUT_DIR="${2:-}"

if [[ -z "$VERSION" || -z "$OUTPUT_DIR" ]]; then
  echo "Usage: package-gpui-linux.sh <version> <output-dir>" >&2
  exit 1
fi

MACHINE="$(uname -m)"
case "$MACHINE" in
  x86_64 | amd64)
    ARCH="x64"
    DEB_ARCH="amd64"
    ;;
  aarch64 | arm64)
    ARCH="arm64"
    DEB_ARCH="arm64"
    ;;
  *)
    echo "Unsupported Linux architecture: $MACHINE" >&2
    exit 1
    ;;
esac

TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT_DIR/src/main/target}"
GPUI="$TARGET_DIR/release/poratake-gpui"
DAEMON="$TARGET_DIR/release/poratake-daemon-linux"

if [[ ! -x "$GPUI" ]]; then
  echo "Missing GPUI binary: $GPUI" >&2
  exit 1
fi
if [[ ! -x "$DAEMON" ]]; then
  echo "Missing Linux daemon: $DAEMON" >&2
  exit 1
fi

STAGING="$(mktemp -d "${TMPDIR:-/tmp}/poratake-linux-XXXXXX")"
cleanup() {
  rm -rf "$STAGING"
}
trap cleanup EXIT

mkdir -p "$STAGING/daemon" "$STAGING/licenses" "$STAGING/share/applications" "$STAGING/share/icons/hicolor/256x256/apps"
cp "$GPUI" "$STAGING/poratake-gpui"
cp "$DAEMON" "$STAGING/daemon/poratake-daemon-linux"
chmod 755 "$STAGING/poratake-gpui" "$STAGING/daemon/poratake-daemon-linux"

if [[ -f "$ROOT_DIR/build/icon.png" ]]; then
  cp "$ROOT_DIR/build/icon.png" "$STAGING/share/icons/hicolor/256x256/apps/poratake.png"
fi

cat > "$STAGING/share/applications/com.porabuild.poratake.desktop" <<'DESKTOP'
[Desktop Entry]
Type=Application
Name=Poratake
Comment=Capture, annotate, record, and share screenshots and videos
Exec=poratake-gpui %U
Icon=poratake
Terminal=false
Categories=Utility;Graphics;AudioVideo;
MimeType=image/png;image/jpeg;image/webp;image/gif;video/mp4;video/quicktime;video/webm;
StartupNotify=true
DESKTOP

cp "$ROOT_DIR/LICENSE" "$STAGING/licenses/Poratake-AGPL-3.0.txt"
cp "$ROOT_DIR/THIRD_PARTY_NOTICES.md" "$STAGING/licenses/THIRD_PARTY_NOTICES.md"
cp "$ROOT_DIR/licenses/FFmpeg-LGPL-2.1.txt" "$STAGING/licenses/FFmpeg-LGPL-2.1.txt"
cp "$ROOT_DIR/licenses/HeroGPUI-Apache-2.0.txt" "$STAGING/licenses/HeroGPUI-Apache-2.0.txt"
cp "$ROOT_DIR/licenses/ZXing-LICENSE.txt" "$STAGING/licenses/ZXing-LICENSE.txt"
cp "$ROOT_DIR/licenses/Geist-OFL-1.1.txt" "$STAGING/licenses/Geist-OFL-1.1.txt"

if [[ -f "$ROOT_DIR/src/main/binaries/ffmpeg/ffmpeg" ]]; then
  mkdir -p "$STAGING/binaries/ffmpeg"
  cp "$ROOT_DIR/src/main/binaries/ffmpeg/ffmpeg" "$STAGING/binaries/ffmpeg/ffmpeg"
  chmod 755 "$STAGING/binaries/ffmpeg/ffmpeg"
fi
if [[ -f "$ROOT_DIR/src/main/binaries/whisper/whisper" ]]; then
  mkdir -p "$STAGING/binaries/whisper"
  cp "$ROOT_DIR/src/main/binaries/whisper/whisper" "$STAGING/binaries/whisper/whisper"
  chmod 755 "$STAGING/binaries/whisper/whisper"
fi

mkdir -p "$OUTPUT_DIR"
ARCHIVE_NAME="Poratake-${VERSION}-linux-${ARCH}.tar.gz"
ARCHIVE="$OUTPUT_DIR/$ARCHIVE_NAME"
tar -C "$STAGING" -czf "$ARCHIVE" .
test -s "$ARCHIVE"

bun "$ROOT_DIR/scripts/write-updater-yml.mjs" "$VERSION" "$OUTPUT_DIR/latest-linux.yml" "$ARCHIVE" >/dev/null

if command -v dpkg-deb >/dev/null 2>&1; then
  DEB_ROOT="$STAGING/deb-root"
  mkdir -p "$DEB_ROOT/DEBIAN" "$DEB_ROOT/opt/Poratake" "$DEB_ROOT/usr/bin" "$DEB_ROOT/usr/share/applications" "$DEB_ROOT/usr/share/icons/hicolor/256x256/apps"
  cp "$STAGING/poratake-gpui" "$DEB_ROOT/opt/Poratake/poratake-gpui"
  cp -R "$STAGING/daemon" "$DEB_ROOT/opt/Poratake/daemon"
  cp -R "$STAGING/licenses" "$DEB_ROOT/opt/Poratake/licenses"
  if [[ -d "$STAGING/binaries" ]]; then
    cp -R "$STAGING/binaries" "$DEB_ROOT/opt/Poratake/binaries"
  fi
  ln -sf /opt/Poratake/poratake-gpui "$DEB_ROOT/usr/bin/poratake-gpui"
  cp "$STAGING/share/applications/com.porabuild.poratake.desktop" "$DEB_ROOT/usr/share/applications/com.porabuild.poratake.desktop"
  if [[ -f "$STAGING/share/icons/hicolor/256x256/apps/poratake.png" ]]; then
    cp "$STAGING/share/icons/hicolor/256x256/apps/poratake.png" "$DEB_ROOT/usr/share/icons/hicolor/256x256/apps/poratake.png"
  fi
  cat > "$DEB_ROOT/DEBIAN/control" <<CONTROL
Package: poratake
Version: ${VERSION}
Section: utils
Priority: optional
Architecture: ${DEB_ARCH}
Maintainer: Porabuild <https://github.com/Porabuild/Poratake>
Homepage: https://github.com/Porabuild/Poratake
Depends: libgtk-3-0 | libgtk-3-0t64, libayatana-appindicator3-1, libdbus-1-3, libpipewire-0.3-0 | libpipewire-0.3-0t64, libx11-6, libx11-xcb1, libxcb1, libxcb-shm0, libxcb-randr0, libxcb-xfixes0, libxkbcommon0, libxkbcommon-x11-0, libwayland-client0, libfontconfig1, libasound2 | libasound2t64, ffmpeg
Recommends: xdg-desktop-portal, wl-clipboard
Description: Poratake screenshot, annotation, and screen recording tool
 Licensed under GNU AGPL-3.0-only.
CONTROL
  dpkg-deb --build --root-owner-group "$DEB_ROOT" "$OUTPUT_DIR/Poratake-${VERSION}-linux-${ARCH}.deb" >/dev/null
fi

echo "$ARCHIVE"
