#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT_DIR/src/main/target}"
MANIFEST="$ROOT_DIR/src/main/app-gpui/Cargo.toml"

rustup target add aarch64-apple-darwin x86_64-apple-darwin
cargo build --release --locked --manifest-path "$MANIFEST" --target-dir "$TARGET_DIR" --target aarch64-apple-darwin
cargo build --release --locked --manifest-path "$MANIFEST" --target-dir "$TARGET_DIR" --target x86_64-apple-darwin

mkdir -p "$TARGET_DIR/release"
lipo -create \
  "$TARGET_DIR/aarch64-apple-darwin/release/poratake-gpui" \
  "$TARGET_DIR/x86_64-apple-darwin/release/poratake-gpui" \
  -output "$TARGET_DIR/release/poratake-gpui"

ARCHS="$(lipo -archs "$TARGET_DIR/release/poratake-gpui")"
[[ " $ARCHS " == *" arm64 "* ]]
[[ " $ARCHS " == *" x86_64 "* ]]
