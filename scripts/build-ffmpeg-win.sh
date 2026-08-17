#!/usr/bin/env bash

set -euo pipefail

FFMPEG_VERSION="7.1"
FFMPEG_URL="https://ffmpeg.org/releases/ffmpeg-${FFMPEG_VERSION}.tar.xz"
FFMPEG_SHA256="40973d44970dbc83ef302b0609f2e74982be2d85916dd2ee7472d30678a7abe6"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
OUTPUT_DIR="$PROJECT_ROOT/src/main/binaries/ffmpeg"
BUILD_DIR="$(mktemp -d)"
HOST_MACHINE="$(uname -m)"
if [ "$HOST_MACHINE" = "aarch64" ] || [ "$HOST_MACHINE" = "arm64" ]; then
    HOST_ARCH="arm64"
else
    HOST_ARCH="x64"
fi
ARCH="${PORATAKE_WIN_ARCH:-$HOST_ARCH}"

if [ "$ARCH" != "x64" ] && [ "$ARCH" != "arm64" ]; then
    echo "Unsupported PORATAKE_WIN_ARCH: $ARCH" >&2
    exit 1
fi

if [ -f "$OUTPUT_DIR/ffmpeg.exe" ]; then
    echo "ffmpeg.exe already built, skipping."
    exit 0
fi

cleanup() {
    rm -rf "$BUILD_DIR"
}

trap cleanup EXIT

if [ "$ARCH" = "arm64" ]; then
    REQUIRED_COMMANDS=(curl sha256sum tar make clang nasm pkg-config)
    FFMPEG_CPU_ARCH="aarch64"
    FFMPEG_TARGET_OS="mingw32"
    FFMPEG_TOOLCHAIN_ARGS=(--cc=clang --cxx=clang++ --as=clang)
else
    REQUIRED_COMMANDS=(curl sha256sum tar make gcc nasm pkg-config)
    FFMPEG_CPU_ARCH="x86_64"
    FFMPEG_TARGET_OS="mingw64"
    FFMPEG_TOOLCHAIN_ARGS=()
fi

for command in "${REQUIRED_COMMANDS[@]}"; do
    if ! command -v "$command" >/dev/null 2>&1; then
        echo "Missing required MSYS2 command: $command" >&2
        exit 1
    fi
done

curl --fail --location --retry 5 --retry-delay 2 --retry-all-errors --retry-max-time 120 --output "$BUILD_DIR/ffmpeg.tar.xz" "$FFMPEG_URL"
echo "$FFMPEG_SHA256  $BUILD_DIR/ffmpeg.tar.xz" | sha256sum --check --status
tar -xf "$BUILD_DIR/ffmpeg.tar.xz" -C "$BUILD_DIR"
cd "$BUILD_DIR/ffmpeg-${FFMPEG_VERSION}"

./configure \
    --prefix="$BUILD_DIR/install" \
    --arch="$FFMPEG_CPU_ARCH" \
    --target-os="$FFMPEG_TARGET_OS" \
    "${FFMPEG_TOOLCHAIN_ARGS[@]}" \
    --enable-static \
    --disable-shared \
    --disable-gpl \
    --disable-nonfree \
    --disable-libx264 \
    --disable-libx265 \
    --disable-libvpx \
    --disable-libaom \
    --enable-mediafoundation \
    --enable-d3d11va \
    --disable-doc \
    --disable-ffplay \
    --disable-ffprobe \
    --disable-debug \
    --disable-pthreads \
    --enable-w32threads \
    --extra-ldflags=-static \
    --enable-encoder=h264_mf \
    --enable-encoder=aac \
    --enable-encoder=gif \
    --enable-encoder=png \
    --enable-encoder=mjpeg \
    --enable-decoder=h264 \
    --enable-decoder=hevc \
    --enable-decoder=aac \
    --enable-decoder=mp3 \
    --enable-decoder=gif \
    --enable-decoder=png \
    --enable-decoder=mjpeg \
    --enable-decoder=prores \
    --enable-muxer=mp4 \
    --enable-muxer=mov \
    --enable-muxer=gif \
    --enable-muxer=image2 \
    --enable-muxer=mjpeg \
    --enable-demuxer=mov \
    --enable-demuxer=mp4 \
    --enable-demuxer=gif \
    --enable-demuxer=image2 \
    --enable-demuxer=mjpeg \
    --enable-protocol=file \
    --enable-protocol=pipe \
    --enable-filter=scale \
    --enable-filter=fps \
    --enable-filter=palettegen \
    --enable-filter=paletteuse \
    --enable-filter=split \
    --enable-filter=pad \
    --enable-filter=format \
    --enable-filter=null \
    --enable-filter=aformat \
    --enable-filter=anull \
    --enable-filter=concat \
    --enable-filter=trim \
    --enable-filter=atrim \
    --enable-filter=setpts \
    --enable-filter=asetpts \
    --enable-filter=select \
    --enable-filter=aselect

make -j"$(nproc)"
make install
mkdir -p "$OUTPUT_DIR"
cp "$BUILD_DIR/install/bin/ffmpeg.exe" "$OUTPUT_DIR/ffmpeg.exe"

if [ "$ARCH" = "$HOST_ARCH" ]; then
    VERSION_OUTPUT="$($OUTPUT_DIR/ffmpeg.exe -version 2>&1)"
    ENCODER_OUTPUT="$($OUTPUT_DIR/ffmpeg.exe -hide_banner -encoders 2>&1)"

    if grep -q -- '--enable-gpl\|--enable-nonfree' <<<"$VERSION_OUTPUT"; then
        echo 'FFmpeg build is not LGPL compliant' >&2
        exit 1
    fi

    if ! grep -q 'h264_mf' <<<"$ENCODER_OUTPUT"; then
        echo 'FFmpeg build is missing h264_mf' >&2
        exit 1
    fi

    if ! grep -q ' aac ' <<<"$ENCODER_OUTPUT"; then
        echo 'FFmpeg build is missing the AAC encoder' >&2
        exit 1
    fi
fi

echo "Built $OUTPUT_DIR/ffmpeg.exe"
