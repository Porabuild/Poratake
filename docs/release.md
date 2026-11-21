# Releasing a GPUI build

The `Release` workflow (`.github/workflows/release.yml`) is the only shipping
path. Electron builds are frozen; `scripts/release.sh` exits unless
`PORATAKE_ELECTRON_RELEASE=1` is set for a historical build.

## Preconditions

- The `Checks` workflow is green on the release commit: TypeScript checks,
  Vitest, macOS GPUI, Linux x64/arm64, and Windows x64/arm64.
- The open gates in `GPUI-UI-PARITY-AUDIT.md` are resolved for the platforms
  being announced.
- An installed 0.9.6 Electron build has been updated to the GPUI build on real
  Windows and macOS machines.
- The target version is not tagged yet.

## Dry run

Run **Actions → Release → Run workflow** with:

- `version`: the new version, for example `0.9.7`
- `dry_run`: `true`

The workflow builds and validates every asset without creating a tag or
release. Expect these artifacts:

- macOS: `Poratake-<version>-universal.dmg`,
  `Poratake-<version>-universal-mac.zip`, `latest-mac.yml`
- Windows: `Poratake-<version>-win-x64.exe`,
  `Poratake-<version>-win-arm64.exe`
- Linux: `Poratake-<version>-linux-x64.tar.gz`,
  `Poratake-<version>-linux-x64.deb`, the arm64 equivalents, and
  `latest-linux.yml`
- Merged `latest.yml` with both Windows architectures

The macOS app and every nested binary must report `arm64` and `x86_64`; the
workflow checks this with `lipo`.

## Publish

Run the same workflow with `dry_run: false`. It:

1. builds and validates all assets;
2. commits the `package.json` version bump;
3. tags `v<version>` on that commit;
4. creates a draft release, validates asset names and sizes, then publishes it
   as the latest release.

Electron 0.9.6 remains downloadable; it is no longer rebuilt.

## Verify after publishing

- Confirm `latest-mac.yml`, `latest.yml`, and `latest-linux.yml` point at the
  new files and their SHA-512 values.
- Install on a clean macOS, Windows, and Omarchy 4.5 machine.
- Confirm the in-app update from 0.9.6 to the GPUI build on Windows and macOS.
- On Omarchy, run `packaging/omarchy/install.sh`, launch from the desktop entry,
  and verify the autostart block in `~/.config/hypr/autostart.conf`.
- Record any Wayland gaps in `docs/omarchy.md` and the audit.

## Failure handling

A failed publish triggers `cleanup-failed-release`, which removes the draft
release and staging tag. A published release can be removed with
`scripts/delete-release.sh <version> <previous-version>`; do not delete a tag
that a source archive is meant to preserve unless the release is being withdrawn.
