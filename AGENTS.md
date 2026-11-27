# Poratake

## Supported Platforms

Poratake ships a Rust GPUI desktop app on macOS 15 or later (Intel and Apple Silicon), Windows 10/11 (x64 and arm64), and Linux (x86_64 and aarch64). See [Linux support](docs/omarchy.md) for the X11 and Wayland feature matrix. Gate features through `src/main/app-gpui/src/system/capabilities.rs` and update `src/types/platform-capabilities.json` when platform support changes.

## Architecture

- `src/main/app-gpui/` owns the UI, capture coordination, editors, settings, history, cloud uploads and updates. Use HeroGPUI components and the app's shared controls and theme tokens. Keep saved video consistent with the preview, including zoom, trims, cuts and composited layers.
- The GPUI app communicates over JSON-RPC on stdin and stdout with the Swift daemon on macOS (`src/main/daemon/`) and Rust daemons on Windows (`src/main/daemon-win/`) and Linux (`src/main/daemon-linux/`). The Rust daemons and GPUI share protocol types in `src/main/daemon-common/`.
- Update `src/types/daemon-contract.json` when changing the protocol. Run `bun run generate:daemon-contract` to regenerate Rust and Swift bindings. Register new modules on every platform listed in the contract.
- Window recording uses ScreenCaptureKit on macOS and Windows Graphics Capture on Windows. It keeps the initial output dimensions and letterboxes resized windows. Keep cursor coordinates, encoder crop and the capture outline aligned to the live window bounds.
- On HDR Windows displays, keep the native scRGB capture path and the calibrated SDR transfer. Recording tone mapping runs on the GPU. Keep COM/WinRT work in the process MTA using `retain_process_mta()`, and release interfaces before the guard drops. Share Media Foundation interfaces with `MtaInterface` only on threads that joined that apartment.
- Windows daemon controls use the DPI-scaled, double-buffered panel helpers in `src/main/daemon-win/src/panel.rs`. Apply the active theme to daemon overlays. Preserve the independent camera, microphone and system-audio tracks and their behavior when toggled mid-recording.

## Development and Tests

- Launch the native app with `bun run dev` or `bun run dev:gpui`. Use the platform daemon build and setup steps in [CONTRIBUTING.md](CONTRIBUTING.md). Leave packaged builds to the Release workflow.
- Write tests for new features and bug fixes. Run `bun run checks` for generated contract validation, Oxfmt, Oxlint, tooling tests and the Windows native checks when available.
- Run `cargo fmt --manifest-path src/main/Cargo.toml --all -- --check` before submitting Rust changes. Run Cargo tests and Clippy for the affected workspace crates. On macOS, use `cargo xwin clippy --locked --manifest-path src/main/Cargo.toml --workspace --all-targets --target x86_64-pc-windows-msvc -- -D warnings` to cross-check Windows if cargo-xwin is available.
- Native Swift regressions live in `tests/native/` and run in the macOS Checks job. The Windows native visual and desktop checks are documented in [scripts/parity/README.md](scripts/parity/README.md).

## Licensing (AGPL-3.0 — mandatory, never trade off against other rules)

Poratake is a rebranded fork of Capty (https://github.com/capty-app/capty) and is licensed AGPL-3.0-only as a whole. These rules keep every release compliant:

- Never edit, remove, or replace `LICENSE` (verbatim GNU AGPL v3), and keep `"license": "AGPL-3.0-only"` in `package.json`.
- Never remove or weaken existing copyright, license, warranty, or attribution notices — in `README.md`, the About tab, file headers, or vendored code. The upstream "Copyright (C) 2026 Capty" notice must stay.
- `README.md` and the About tab must keep the modification statement (Poratake is a modified version of Capty, with the year of modification) and the link to the upstream project.
- The About tab is the app's AGPL §5(d) "Appropriate Legal Notices". It must always show: the copyright notices, the AGPL v3 license with a working link, the no-warranty statement, a link to this exact version's source (`SOURCE_URL/tree/v{version}`), and the third-party notices link. Do not remove or hide these when redesigning settings.
- Every public release must make corresponding source available alongside the binaries: the release workflow publishes to a GitHub Release whose tag must point at the exact commit the binaries were built from, so GitHub's automatic source archives sit next to the downloads. Never distribute a build made from uncommitted or unpushed changes, and never publish binaries through a channel that lacks a source link.
- The macOS, Windows and Linux packaging scripts must ship `licenses/Poratake-AGPL-3.0.txt`, `licenses/THIRD_PARTY_NOTICES.md`, `licenses/FFmpeg-LGPL-2.1.txt` and the license texts of bundled components.
- Any change that adds, removes, or upgrades a bundled third-party component (crate, native binary, font, icon set, sound, model) must update `THIRD_PARTY_NOTICES.md` in the same change, including the pinned version for native binaries.
- Only bundle components under AGPL-compatible licenses (MIT, BSD, Apache-2.0, LGPL, MPL-2.0, AGPL/GPLv3-compatible). Never bundle proprietary or no-license code, and never copy source from projects under incompatible licenses.
- FFmpeg must stay LGPL-configured: keep `--disable-gpl --disable-nonfree` in `scripts/build-ffmpeg.sh` and do not enable x264/x265/GPL filters. Keep native builds (`build-ffmpeg.sh`, `build-whisper.sh`) pinned to exact upstream release versions that match `THIRD_PARTY_NOTICES.md`.
- Trademark: do not brand Poratake with the Capty name or logo. Referring to Capty factually is fine (upstream attribution) — implying Poratake is Capty or is endorsed by Capty is not. Poratake is free software and must not call any Capty server infrastructure: no license API, no Capty Cloud, no capty.app links of any kind. Cloud uploads go only to the user's own S3/REST providers.
- The Windows daemon's statically linked crates are enumerated in `THIRD_PARTY_NOTICES.md`; any change to `src/main/daemon-win/Cargo.toml` or `Cargo.lock` that adds or removes crates must update that list in the same change. The macOS Swift daemon must stay free of third-party dependencies, or its dependencies must be added to the notices file the same way.
- If Poratake ever gains a network-accessible service mode, AGPL §13 applies: users interacting with it over a network must be offered the corresponding source.

## Top Level Rules

- Security first
- Maintainability
- Scalability
- Clean Code
- Clean Architecture
- Best Practices
- No Hacky Solutions

## General Guidelines

- Don't implement hacky solutions to just make it work. We need proper solutions.
- Write less code and maintainable code
- Be mindful about app size and performance and memory and cpu usage.
- Learn from project's structure and implement new features in the same way.
- Break code into smaller components and files.
- When implementing a feature that can also be configurable, ask user's opinion to make it configurable in the app settings or not.
- Consider SOLID principles and best practices while writing code.
- If there is a refactor needed, ask user's opinion first.
- Avoid creating big files and components. Instead, modularize and break them into smaller pieces.
- NEVER NEVER NEVER code comment!
- When adding assets to the project like images, icons, sounds, etc, make sure you also consider them for production build and packing and notarizing to work on packaged app too.
- Don't patch symptoms! Fix the root cause of the issues.
- Before adding an app-side workaround or wrapper for a bug or missing behavior in an upstream dependency we maintain (especially `HeroGPUI` or patched `gpui-pre` crates), inspect the upstream source first and evaluate whether the fix belongs upstream. If the root cause lives in upstream component or framework behavior (such as overlay/tooltip positioning, focus handling, accessibility, or component sizing), stop and ask the user whether to fix it upstream rather than working around it in Poratake.
- Use early returns to reduce nesting
- Important to avoid nested if-else statements
- Prefer switch-case for multiple conditions
- Use agnostic implementations everywhere possible so we can re-use those codes
- Don't use just delays instead of promises! Handle them properly. (using delay is basically hacking and we should avoid)
- Code duplications should be avoided at any cost.
- Don't install third-party packages without approval.
- We might have unused codes that seemed to be in-use! if you face any of them, make sure if they are used or not then decide to keep or remove them.
- No guessing and No assumption! Work with certainity.

## HeroGPUI and vendored GPUI

Canonical workflow: `docs/gpui-vendoring.md`. Read it before changing HeroGPUI, `vendor/gpui-pre*`, or the GPUI pins.

- HeroGPUI is not vendored here. HeroGPUI bugs are fixed in `Porabuild/HeroGPUI`, released, and then consumed here by bumping the exact pin.
- GPUI framework/platform bugs are fixed directly in Poratake's vendored `vendor/gpui-pre*` patchset, not hidden behind app-side wrappers. When the behavior is clearly wrong upstream, also prepare the equivalent minimal patch against `zed-industries/zed`.
- HeroGPUI, the direct `gpui-pre` dependency, and the vendored `gpui-pre*` crates must stay on one exact coherent GPUI version.
- Every intentional vendored GPUI difference must be recorded in the patch inventory in `docs/gpui-vendoring.md` in the same commit.
- When upgrading `gpui-pre`, diff the vendored crates against pristine copies of the previous crates.io version, reconcile every downstream hunk, forward-port only the patches still needed, and update the inventory.

## Questions

- If you have questions, Use the question tool only for asking questions.

## Code Review

- Review the PRs/Code against the purpose of the PR/Issue/Asked. If you find unrelated issues to the PR during the review, Report them in a separate section.
- Apply review recommendations only after user's confirmation.

## PR and Commits

- After end of each task that has a chance to create a PR for it, suggest PR title and description
- Add feat: or fix: or internal: prefixes to the PRs or Commits because thats how they will be appeared on release logs.
