# Changelog

All notable changes to Poratake are documented in this file. Entries are
generated from the commit history (`feat:` → Features, `fix:` → Bug Fixes,
everything else → Internal), matching the published release notes.

## [0.9.0] - 2026-08-17

First Poratake release.

### Features

- Publish Windows arm64 installers and shared product URLs
- Unify capture flows around Electron overlays
- Enable Windows updates and validate release assets
- Remove Capty licensing and complete Poratake rebrand (#2)
- Add window recording, live controls, and editor tracks
- Rebrand app as Poratake with new UI and theme system
- Add interactive area overlay and camera/mic device settings
- Render Windows area selection in an Electron frozen-frame overlay
- Add Windows support
- Add automated CI checks (#1)

### Bug Fixes

- Package Windows releases per architecture
- Select Clang for Windows arm64 FFmpeg
- Use native Bun on Windows arm64
- Satisfy workflow assertion lint
- Accept Windows workflow line endings
- Prevent release signing stalls
- Mark build-whisper.sh executable for CI
- Install nasm and cmake on the macOS release runner
- Retry pinned FFmpeg downloads
- Isolate dev lock tests from process queries
- Add Poratake authorship metadata
- Validate whisper help without PowerShell stderr errors
- Use native Windows threads for FFmpeg
- Launch FFmpeg build through BOM-free script
- Send BOM-free commands to MSYS2
- Support Windows PowerShell FFmpeg builds
- Preserve MSYS2 FFmpeg command quoting
- Avoid NSView flipped property collision
- Compile iOS recorder first-frame wait
- Make clipboard URL test platform-safe
- Remove Swift trailing whitespace
- Finish Poratake standalone repository readiness
- Complete Poratake repository move
- Hide recording bar before video finalization (#16)

### Internal

- Parallelize release builds
- Format release-assets test for prettier
- Cache macOS native binaries in Release CI
- Consolidate capture and annotation exports (#4)
- Unify capture controls and improve FFmpeg reliability (#3)
- Ignore direnv environment files
- Set version to 0.9.0 and point Porabuild URLs to /poratake
- Ignore .commandcode and .poracode directories
- Update packages
- Init
