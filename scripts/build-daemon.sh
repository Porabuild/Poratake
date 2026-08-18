#!/bin/bash

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
DAEMON_DIR="$PROJECT_ROOT/src/main/daemon"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

PLIST_FILE="$DAEMON_DIR/Info.plist"
OUTPUT_FILE="$DAEMON_DIR/poratake-daemon"

echo -e "${YELLOW}Building poratake-daemon...${NC}"

SWIFT_FILES=$(find "$DAEMON_DIR" -name "*.swift" -type f)
if [ -z "$SWIFT_FILES" ]; then
    echo -e "${RED}Error: No Swift files found in $DAEMON_DIR${NC}"
    exit 1
fi

PLIST_ARGS=""
if [ -f "$PLIST_FILE" ]; then
    echo "  Embedding Info.plist..."
    PLIST_ARGS="-Xlinker -sectcreate -Xlinker __TEXT -Xlinker __info_plist -Xlinker $PLIST_FILE"
fi

echo "  Compiling for arm64..."
swiftc -O -target arm64-apple-macosx13.0 \
    -suppress-warnings \
    $PLIST_ARGS \
    -o "$OUTPUT_FILE-arm64" \
    $SWIFT_FILES

echo "  Compiling for x86_64..."
swiftc -O -target x86_64-apple-macosx13.0 \
    -suppress-warnings \
    $PLIST_ARGS \
    -o "$OUTPUT_FILE-x86_64" \
    $SWIFT_FILES

echo "  Creating universal binary..."
lipo -create \
    "$OUTPUT_FILE-arm64" \
    "$OUTPUT_FILE-x86_64" \
    -output "$OUTPUT_FILE"

rm -f "$OUTPUT_FILE-arm64" "$OUTPUT_FILE-x86_64"

ARCHS=$(lipo -archs "$OUTPUT_FILE")
if [[ "$ARCHS" == *"arm64"* ]] && [[ "$ARCHS" == *"x86_64"* ]]; then
    echo -e "${GREEN}Successfully built universal binary: $OUTPUT_FILE${NC}"
    echo "  Architectures: $ARCHS"
else
    echo -e "${RED}Error: Binary is not universal. Architectures: $ARCHS${NC}"
    exit 1
fi

chmod +x "$OUTPUT_FILE"
