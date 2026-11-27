#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
VERSION="${1:-$(node -e "console.log(JSON.parse(require('fs').readFileSync('$ROOT_DIR/package.json','utf8')).version)")}"
OUTPUT_DIR="${2:-$ROOT_DIR/release/$VERSION}"

TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT_DIR/src/main/target}"
GPUI="$TARGET_DIR/release/poratake-gpui"
DAEMON="$ROOT_DIR/src/main/daemon/poratake-daemon"
FFMPEG="$ROOT_DIR/src/main/binaries/ffmpeg/ffmpeg"
WHISPER="$ROOT_DIR/src/main/binaries/whisper/whisper"

for required in "$GPUI" "$DAEMON" "$FFMPEG" "$WHISPER"; do
  if [[ ! -x "$required" ]]; then
    echo "Missing executable binary: $required" >&2
    exit 1
  fi
done

mkdir -p "$OUTPUT_DIR"
APP="$OUTPUT_DIR/Poratake.app"
rm -rf "$APP"

CONTENTS="$APP/Contents"
MACOS="$CONTENTS/MacOS"
RESOURCES="$CONTENTS/Resources"
mkdir -p "$MACOS" "$RESOURCES/daemon" "$RESOURCES/binaries/ffmpeg" "$RESOURCES/binaries/whisper" "$RESOURCES/licenses" "$RESOURCES/sounds"

cp "$GPUI" "$MACOS/poratake-gpui"
chmod 755 "$MACOS/poratake-gpui"

cp "$DAEMON" "$RESOURCES/daemon/poratake-daemon"
cp "$FFMPEG" "$RESOURCES/binaries/ffmpeg/ffmpeg"
cp "$WHISPER" "$RESOURCES/binaries/whisper/whisper"
chmod 755 "$RESOURCES/daemon/poratake-daemon" "$RESOURCES/binaries/ffmpeg/ffmpeg" "$RESOURCES/binaries/whisper/whisper"

cp "$ROOT_DIR/build/icon.icns" "$RESOURCES/icon.icns"
cp -R "$ROOT_DIR/public/sounds/." "$RESOURCES/sounds/"

cp "$ROOT_DIR/LICENSE" "$RESOURCES/licenses/Poratake-AGPL-3.0.txt"
cp "$ROOT_DIR/THIRD_PARTY_NOTICES.md" "$RESOURCES/licenses/THIRD_PARTY_NOTICES.md"
cp "$ROOT_DIR/licenses/FFmpeg-LGPL-2.1.txt" "$RESOURCES/licenses/FFmpeg-LGPL-2.1.txt"
cp "$ROOT_DIR/licenses/HeroGPUI-Apache-2.0.txt" "$RESOURCES/licenses/HeroGPUI-Apache-2.0.txt"
cp "$ROOT_DIR/licenses/ZXing-LICENSE.txt" "$RESOURCES/licenses/ZXing-LICENSE.txt"
cp "$ROOT_DIR/licenses/Geist-OFL-1.1.txt" "$RESOURCES/licenses/Geist-OFL-1.1.txt"

cat > "$CONTENTS/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key><string>en</string>
  <key>CFBundleExecutable</key><string>poratake-gpui</string>
  <key>CFBundleIconFile</key><string>icon.icns</string>
  <key>CFBundleIdentifier</key><string>com.porabuild.poratake</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
  <key>CFBundleName</key><string>Poratake</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>${VERSION}</string>
  <key>CFBundleVersion</key><string>${VERSION}</string>
  <key>LSMinimumSystemVersion</key><string>15.0</string>
  <key>LSUIElement</key><true/>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSMicrophoneUsageDescription</key><string>Poratake needs microphone access to record voice audio along with screen recordings.</string>
  <key>NSCameraUsageDescription</key><string>Poratake needs camera access to record video from your camera.</string>
  <key>NSCameraUseContinuityCameraDeviceType</key><true/>
  <key>CFBundleDocumentTypes</key>
  <array>
    <dict>
      <key>CFBundleTypeName</key><string>Poratake Recording</string>
      <key>CFBundleTypeExtensions</key><array><string>poratake</string></array>
      <key>CFBundleTypeRole</key><string>Editor</string>
      <key>LSHandlerRank</key><string>Owner</string>
      <key>LSTypeIsPackage</key><true/>
    </dict>
    <dict>
      <key>CFBundleTypeName</key><string>Image</string>
      <key>CFBundleTypeExtensions</key>
      <array>
        <string>png</string><string>jpg</string><string>jpeg</string>
        <string>gif</string><string>webp</string><string>bmp</string><string>tiff</string>
      </array>
      <key>CFBundleTypeRole</key><string>Editor</string>
      <key>LSHandlerRank</key><string>Alternate</string>
    </dict>
  </array>
</dict>
</plist>
PLIST

if [[ -n "${PORATAKE_CODESIGN_IDENTITY:-}" ]]; then
  ENTITLEMENTS="$ROOT_DIR/build/entitlements.mac.dist"
  for bin in "$RESOURCES/daemon/poratake-daemon" "$RESOURCES/binaries/ffmpeg/ffmpeg" "$RESOURCES/binaries/whisper/whisper"; do
    codesign --force --options runtime --timestamp --sign "$PORATAKE_CODESIGN_IDENTITY" "$bin"
  done
  codesign --force --deep --options runtime --timestamp --entitlements "$ENTITLEMENTS" --sign "$PORATAKE_CODESIGN_IDENTITY" "$APP"
  codesign --verify --deep --strict --verbose=2 "$APP"
  if [[ -n "${APPLE_ID:-}" && -n "${APPLE_APP_SPECIFIC_PASSWORD:-}" && -n "${APPLE_TEAM_ID:-}" ]]; then
    NOTARY_ZIP="$OUTPUT_DIR/Poratake-${VERSION}-notary.zip"
    ditto -c -k --sequesterRsrc --keepParent "$APP" "$NOTARY_ZIP"
    xcrun notarytool submit "$NOTARY_ZIP" --apple-id "$APPLE_ID" --password "$APPLE_APP_SPECIFIC_PASSWORD" --team-id "$APPLE_TEAM_ID" --wait
    xcrun stapler staple "$APP"
    xcrun stapler validate "$APP"
    rm -f "$NOTARY_ZIP"
  fi
fi

ZIP_NAME="Poratake-${VERSION}-universal-mac.zip"
ZIP_PATH="$OUTPUT_DIR/$ZIP_NAME"
rm -f "$ZIP_PATH"
ditto -c -k --sequesterRsrc --keepParent "$APP" "$ZIP_PATH"

DMG_PATH="$OUTPUT_DIR/Poratake-${VERSION}-universal.dmg"
rm -f "$DMG_PATH"
DMG_STAGING="$(mktemp -d "${TMPDIR:-/tmp}/poratake-dmg-XXXXXX")"
cp -R "$APP" "$DMG_STAGING/Poratake.app"
ln -s /Applications "$DMG_STAGING/Applications"
hdiutil create -volname "Poratake" -srcfolder "$DMG_STAGING" -ov -format UDZO "$DMG_PATH" >/dev/null
rm -rf "$DMG_STAGING"

node -e "
const fs = require('node:fs');
const crypto = require('node:crypto');
const zipPath = process.argv[1];
const zipName = process.argv[2];
const version = process.argv[3];
const outPath = process.argv[4];
const buf = fs.readFileSync(zipPath);
const sha512 = crypto.createHash('sha512').update(buf).digest('base64');
const size = buf.length;
const releaseDate = new Date().toISOString();
fs.writeFileSync(outPath, [
  'version: ' + version,
  'files:',
  '  - url: ' + zipName,
  '    sha512: ' + sha512,
  '    size: ' + size,
  'path: ' + zipName,
  'sha512: ' + sha512,
  'releaseDate: ' + JSON.stringify(releaseDate),
  ''
].join('\n'));
" "$ZIP_PATH" "$ZIP_NAME" "$VERSION" "$OUTPUT_DIR/latest-mac.yml"

echo "$APP"
