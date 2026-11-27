# Contributing to Poratake

Thanks for your interest in contributing to Poratake.

## Before You Start

- Discuss significant changes in an issue first.
- Keep pull requests focused and small.
- Follow the existing architecture, structure, and coding standards.
- Poratake targets macOS 15 or later (Intel + Apple Silicon), Windows 10/11 (x64 and arm64), and Linux (x86_64 and aarch64; see [docs/omarchy.md](docs/omarchy.md) for the Wayland limits).

## Development Setup

The desktop app lives in `src/main/app-gpui/`. Install Bun and the Rust toolchain before building it. The dev command uses Bacon 3.25.0.

The native binaries are not committed to the repository. Build them once before
your first run.

### macOS

```bash
xcode-select --install
brew install nasm pkg-config cmake
bun install
rustup target add aarch64-apple-darwin x86_64-apple-darwin
./scripts/build-daemon.sh
./scripts/build-ffmpeg.sh
./scripts/build-whisper.sh
```

- `build-daemon.sh` — the unified Swift daemon providing all native functionality
- `build-ffmpeg.sh` — LGPL-compliant FFmpeg, compiled from source, takes a while
- `build-whisper.sh` — whisper.cpp, used for transcription

### Windows

Install Bun, Git, the Rust MSVC toolchain with the Windows SDK, CMake with the
Visual Studio 2022 C++ toolchain, and MSYS2 UCRT64 with `gcc`, `make`, `nasm`,
`pkg-config`, `curl`, and `tar`.

```powershell
bun install
bun run build-native-win
```

`build-native-win` builds the Rust daemon, LGPL-compliant FFmpeg, and whisper.cpp.
Create the Windows installer with Velopack. Install the `vpk` CLI through the .NET SDK using `dotnet tool update -g vpk --version 1.2.161`, then run:

```powershell
cargo build --release --locked --manifest-path src/main/Cargo.toml --target x86_64-pc-windows-msvc --package poratake-daemon --package poratake-gpui
bun run package:gpui:win
```

### Linux

Install the GPUI dependencies listed in `.github/actions/linux-capture-deps/action.yml` (used by `.github/workflows/checks.yml`), then:

```bash
cargo build --locked --manifest-path src/main/Cargo.toml --package poratake-daemon-linux --package poratake-gpui
bun run dev:gpui
```

Omarchy 4.5 installation and the Wayland feature matrix are documented in
[docs/omarchy.md](docs/omarchy.md).

### HeroGPUI and GPUI dependencies

HeroGPUI is consumed as an exact crates.io pin and is not vendored. GPUI's
framework and platform backend crates are vendored under `vendor/gpui-pre*` as
a downstream patchset. Read [docs/gpui-vendoring.md](docs/gpui-vendoring.md)
before changing those crates, the HeroGPUI pin, or the GPUI version. The
document defines where fixes belong, how upstream Zed patches are prepared, and
how vendored patches must be forward-ported during GPUI upgrades.

### Running the app

```bash
bun run dev:gpui
```

Rebuild the daemon whenever you change anything under `src/main/daemon/` on
macOS, `src/main/daemon-win/` on Windows, or `src/main/daemon-linux/` on Linux.
Use `bun run build-daemon-win` for the Windows daemon.

### Testing auto-update in dev

Packaged builds check the GitHub release feed; dev mode does not. To simulate an
update, start GPUI with a fake newer version:

```bash
PORATAKE_DEV_UPDATE_VERSION=99.9.9 PORATAKE_DEV_UPDATE_NOTES="Test notes" bun run dev:gpui
```

Linux updates are disabled when the install directory is not writable, such as a
root-owned `/opt` package installed by `dpkg`; update those through the package
manager.

## Before You Open a Pull Request

```bash
bun run checks
```

Write tests for new features and bug fixes. Run Cargo tests and Clippy against
the affected crates in `src/main/Cargo.toml`. The Checks workflow also runs the
native Swift tests, Windows desktop tests, and Linux X11 integration tests.

## Commit and Pull Request Titles

Prefix titles with `feat:`, `fix:`, or `internal:` — release notes are generated from them.

Release steps are documented in [docs/release.md](docs/release.md).

## Contribution Terms

By opening a pull request, you agree to the following terms for that contribution.

1. **You have the right to contribute it.** The contribution is your original work, or you are otherwise authorized to submit it, and submitting it does not violate any agreement or third-party rights.

2. **It is licensed under this repository's license.** Your contribution is licensed under the GNU Affero General Public License v3.0, and remains available to the community under those same terms.

3. **You keep your copyright.** This is a license grant under the AGPL, not a transfer of ownership. No additional relicensing rights are granted unless you and the repository maintainer enter a separate written agreement.

4. **Patent rights follow the AGPL.** The patent license in section 11 of the GNU Affero General Public License v3.0 applies to your contribution.

5. **You preserve notices.** Contributions must retain applicable copyright, license, warranty, and attribution notices and identify modified work as required by the AGPL.

6. **It is provided as-is.** Unless required by applicable law, your contribution is provided without warranties or conditions of any kind.

If you do not agree to these terms, please do not submit a pull request.
