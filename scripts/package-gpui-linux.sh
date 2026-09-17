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
  x86_64 | amd64) ARCH="x64" ;;
  aarch64 | arm64) ARCH="arm64" ;;
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

mkdir -p "$STAGING/daemon" "$STAGING/licenses"
cp "$GPUI" "$STAGING/poratake-gpui"
cp "$DAEMON" "$STAGING/daemon/poratake-daemon-linux"
chmod 755 "$STAGING/poratake-gpui" "$STAGING/daemon/poratake-daemon-linux"

cp "$ROOT_DIR/LICENSE" "$STAGING/licenses/Poratake-AGPL-3.0.txt"
cp "$ROOT_DIR/THIRD_PARTY_NOTICES.md" "$STAGING/licenses/THIRD_PARTY_NOTICES.md"
cp "$ROOT_DIR/licenses/FFmpeg-LGPL-2.1.txt" "$STAGING/licenses/FFmpeg-LGPL-2.1.txt"
cp "$ROOT_DIR/licenses/HeroGPUI-Apache-2.0.txt" "$STAGING/licenses/HeroGPUI-Apache-2.0.txt"
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
ARCHIVE="$OUTPUT_DIR/Poratake-${VERSION}-linux-${ARCH}.tar.gz"
tar -C "$STAGING" -czf "$ARCHIVE" .
test -s "$ARCHIVE"
echo "$ARCHIVE"
