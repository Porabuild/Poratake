# Releasing Poratake

The [Release workflow](../.github/workflows/release.yml) builds and publishes the native app.

## Preconditions

- Confirm the Checks workflow is green for macOS, Windows, and Linux on the release commit.
- Confirm the target version has no Git tag.

## Dry run

Run **Actions → Release → Run workflow** with the new version, such as `1.0.7`, and set `dry_run` to `true`. The workflow builds and validates the release assets without creating a Git tag or publishing a release.

- macOS universal DMG and ZIP, plus `latest-mac.yml`
- Windows x64 and arm64 installers and Velopack packages, plus the per-architecture release feeds and merged `latest.yml`
- Linux x64 and arm64 tarballs and DEB packages, plus `latest-linux.yml`

The workflow checks both architectures of the macOS app and nested binaries with `lipo`. It also verifies signing and notarization.

## Publish and verify

Run the workflow with `dry_run` set to `false`. It builds and validates the assets, commits the version bump to the metadata files, then creates a draft release. It checks the uploaded assets and publishes the release against the exact tagged source.

- Check the updater manifests against the published filenames, sizes, and SHA-512 digests.
- On macOS and Windows, verify clean installs and in-app updates from the previous release.
- On Omarchy, run `packaging/omarchy/install.sh`, launch from the desktop entry, and verify the Hyprland autostart entry. Record any Wayland gaps in [Linux support](omarchy.md).

If publishing fails, the `cleanup-failed-release` job removes the failed draft and staging tag.
