#!/bin/bash

# =============================================================================
# FFmpeg LGPL Build Script for Poratake
# =============================================================================
# This script builds an LGPL-compliant FFmpeg universal binary for macOS.
# The resulting binary uses VideoToolbox for H.264 encoding instead of libx264,
# making it safe for commercial, closed-source distribution.
#
# Prerequisites:
#   - Xcode Command Line Tools: xcode-select --install
#   - Homebrew packages: brew install nasm pkg-config
#
# Usage:
#   ./scripts/build-ffmpeg.sh
#
# Output:
#   src/main/binaries/ffmpeg/ffmpeg (universal binary)
# =============================================================================

set -e

# Configuration
FFMPEG_VERSION="9.0.1"
FFMPEG_URL="https://ffmpeg.org/releases/ffmpeg-${FFMPEG_VERSION}.tar.xz"
FFMPEG_SHA256="cf38e0e28c7e5605942c4a77755349b0145804a397af37eb1fb4c77cb237f635"
BUILD_DIR="/tmp/ffmpeg-build-$$"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
OUTPUT_DIR="$PROJECT_ROOT/src/main/binaries/ffmpeg"
STAMP_PATH="$OUTPUT_DIR/.ffmpeg-build"
BUILD_ID="$(shasum -a 256 "$SCRIPT_DIR/build-ffmpeg.sh" | awk '{print $1}'):universal"

if [ -f "$OUTPUT_DIR/ffmpeg" ] && [ "$(cat "$STAMP_PATH" 2>/dev/null || true)" = "$BUILD_ID" ]; then
    echo "[INFO] ffmpeg already built, skipping."
    exit 0
fi

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check prerequisites
check_prerequisites() {
    log_info "Checking prerequisites..."
    
    if ! command -v nasm &> /dev/null; then
        log_error "nasm is required. Install with: brew install nasm"
        exit 1
    fi
    
    if ! command -v pkg-config &> /dev/null; then
        log_error "pkg-config is required. Install with: brew install pkg-config"
        exit 1
    fi
    
    if ! xcode-select -p &> /dev/null; then
        log_error "Xcode Command Line Tools required. Install with: xcode-select --install"
        exit 1
    fi
    
    log_info "All prerequisites satisfied."
}

# Clean up on exit
cleanup() {
    if [ -d "$BUILD_DIR" ]; then
        log_info "Cleaning up build directory..."
        rm -rf "$BUILD_DIR"
    fi
}

trap cleanup EXIT

# Download and extract FFmpeg source
download_ffmpeg() {
    log_info "Downloading FFmpeg ${FFMPEG_VERSION}..."
    
    mkdir -p "$BUILD_DIR"
    cd "$BUILD_DIR"
    
    curl --fail --location --retry 5 --retry-delay 2 --retry-all-errors --retry-max-time 120 --output ffmpeg.tar.xz "$FFMPEG_URL"
    echo "$FFMPEG_SHA256  ffmpeg.tar.xz" | shasum -a 256 -c -
    tar xf ffmpeg.tar.xz
    cd "ffmpeg-${FFMPEG_VERSION}"
}

# Build for a specific architecture
build_for_arch() {
    local arch=$1
    local extra_cflags=""
    local extra_ldflags=""
    local frameworks="-framework CoreFoundation -framework CoreMedia -framework CoreVideo -framework VideoToolbox -framework AudioToolbox -framework CoreServices -framework Security"
    
    log_info "Building FFmpeg for $arch..."
    
    cd "$BUILD_DIR/ffmpeg-${FFMPEG_VERSION}"
    
    # Clean any previous build artifacts
    make clean 2>/dev/null || true
    
    if [ "$arch" = "arm64" ]; then
        extra_cflags="-arch arm64 -mmacosx-version-min=11.0"
        extra_ldflags="-arch arm64 -mmacosx-version-min=11.0 $frameworks"
    else
        extra_cflags="-arch x86_64 -mmacosx-version-min=11.0"
        extra_ldflags="-arch x86_64 -mmacosx-version-min=11.0 $frameworks"
    fi
    
    # Configure
    ./configure \
        --prefix="$BUILD_DIR/install-$arch" \
        --enable-cross-compile \
        --arch="$arch" \
        --target-os=darwin \
        --enable-static \
        --disable-shared \
        --disable-gpl \
        --disable-nonfree \
        --disable-autodetect \
        --disable-libx264 \
        --disable-libx265 \
        --disable-libvpx \
        --disable-libaom \
        --enable-videotoolbox \
        --enable-audiotoolbox \
        --disable-doc \
        --disable-ffplay \
        --disable-ffprobe \
        --disable-debug \
        --enable-pthreads \
        --pkg-config-flags=--static \
        --cc="clang" \
        --extra-cflags="$extra_cflags" \
        --extra-ldflags="$extra_ldflags" \
        --enable-encoder=h264_videotoolbox \
        --enable-encoder=hevc_videotoolbox \
        --enable-encoder=prores_videotoolbox \
        --enable-encoder=aac_at \
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
    
    # Build
    make -j$(sysctl -n hw.ncpu)
    make install
    
    log_info "Build for $arch complete."
}

# Create universal binary using lipo
create_universal_binary() {
    log_info "Creating universal binary..."
    
    local candidate="$BUILD_DIR/ffmpeg"
    local next_output="$OUTPUT_DIR/.ffmpeg-next-$$"
    local next_stamp="$OUTPUT_DIR/.ffmpeg-stamp-next-$$"

    lipo -create \
        "$BUILD_DIR/install-arm64/bin/ffmpeg" \
        "$BUILD_DIR/install-x86_64/bin/ffmpeg" \
        -output "$candidate"
    
    # Make executable
    chmod +x "$candidate"
    
    # Verify the binary
    log_info "Verifying universal binary..."
    file "$candidate"
    lipo -info "$candidate"
    
    # Check license compliance
    log_info "Checking license configuration..."
    VERSION_OUTPUT="$("$candidate" -version 2>&1)"
    printf '%s\n' "$VERSION_OUTPUT" | head -5
    
    # Verify no GPL in configuration
    if grep -q -- '--enable-gpl\|--enable-nonfree' <<<"$VERSION_OUTPUT"; then
        log_error "WARNING: GPL flag detected! This build may not be LGPL-compliant."
        exit 1
    fi

    mkdir -p "$OUTPUT_DIR"
    cp "$candidate" "$next_output"
    printf '%s\n' "$BUILD_ID" > "$next_stamp"
    mv -f "$next_output" "$OUTPUT_DIR/ffmpeg"
    mv -f "$next_stamp" "$STAMP_PATH"
    
    log_info "License check passed - build is LGPL-compliant."
}

# Main build process
main() {
    log_info "=== FFmpeg LGPL Build Script for Poratake ==="
    log_info "Building FFmpeg ${FFMPEG_VERSION} for macOS (universal binary)"
    log_info ""
    
    check_prerequisites
    download_ffmpeg
    
    # Build for both architectures
    build_for_arch "arm64"
    
    # Re-extract for clean x86_64 build
    cd "$BUILD_DIR"
    rm -rf "ffmpeg-${FFMPEG_VERSION}"
    tar xf ffmpeg.tar.xz
    cd "ffmpeg-${FFMPEG_VERSION}"
    
    build_for_arch "x86_64"
    
    create_universal_binary
    
    log_info ""
    log_info "=== Build Complete ==="
    log_info "Universal FFmpeg binary created at:"
    log_info "  $OUTPUT_DIR/ffmpeg"
    log_info ""
    log_info "Binary size: $(du -h "$OUTPUT_DIR/ffmpeg" | cut -f1)"
    log_info ""
    log_info "Next steps:"
    log_info "  1. Test the binary: $OUTPUT_DIR/ffmpeg -version"
    log_info "  2. Update electron-builder.json5 to include the new binary"
    log_info "  3. Remove @ffmpeg-installer/ffmpeg from package.json (optional)"
}

main "$@"
