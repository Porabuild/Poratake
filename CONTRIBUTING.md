# Contributing to Poratake

Thanks for your interest in contributing to Poratake.

## Before You Start

- Discuss significant changes in an issue first.
- Keep pull requests focused and small.
- Follow the existing architecture, structure, and coding standards.
- Poratake targets macOS 15 or later (Intel + Apple Silicon) and Windows 10/11 x64.

## Development Setup

The native binaries are not committed to the repository. Build them once before your first run.

### macOS

```bash
xcode-select --install
brew install nasm pkg-config cmake
bun install
./scripts/build-daemon.sh
./scripts/build-ffmpeg.sh
./scripts/build-whisper.sh
```

- `build-daemon.sh` — the unified Swift daemon providing all native functionality
- `build-ffmpeg.sh` — LGPL-compliant FFmpeg, compiled from source, takes a while
- `build-whisper.sh` — whisper.cpp, used for transcription

### Windows

Install Bun, Git, the Rust MSVC toolchain with the Windows SDK, CMake with the Visual Studio 2022 C++ toolchain, and MSYS2 UCRT64 with `gcc`, `make`, `nasm`, `pkg-config`, `curl`, and `tar`.

```powershell
bun install
bun run build-native-win
```

`build-native-win` builds the Rust daemon, LGPL-compliant FFmpeg, and whisper.cpp. Create an NSIS installer with `bun run build-win`.

Then start the app:

```bash
bun run dev
```

Rebuild the daemon whenever you change anything under `src/main/daemon/` on macOS or `src/main/daemon-win/` on Windows. Use `bun run build-daemon-win` for the Windows daemon.

## Before You Open a Pull Request

```bash
bun lint
bun run test:run
bun run format
```

Write tests for new features and bug fixes. Main-process tests run with Vitest.

## Commit and Pull Request Titles

Prefix titles with `feat:`, `fix:`, or `internal:` — release notes are generated from them.

## Contribution Terms

By opening a pull request, you agree to the following terms for that contribution.

1. **You have the right to contribute it.** The contribution is your original work, or you are otherwise authorized to submit it, and submitting it does not violate any agreement or third-party rights.

2. **It is licensed under this repository's license.** Your contribution is licensed under the GNU Affero General Public License v3.0, and remains available to the community under those same terms.

3. **You keep your copyright.** This is a license grant under the AGPL, not a transfer of ownership. No additional relicensing rights are granted unless you and the repository maintainer enter a separate written agreement.

4. **Patent rights follow the AGPL.** The patent license in section 11 of the GNU Affero General Public License v3.0 applies to your contribution.

5. **You preserve notices.** Contributions must retain applicable copyright, license, warranty, and attribution notices and identify modified work as required by the AGPL.

6. **It is provided as-is.** Unless required by applicable law, your contribution is provided without warranties or conditions of any kind.

If you do not agree to these terms, please do not submit a pull request.
