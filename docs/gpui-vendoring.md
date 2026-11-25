# HeroGPUI and vendored GPUI policy

Poratake uses two different dependency models for its Rust UI stack. Treating
them the same causes fixes to land in the wrong repository or be lost during an
upgrade.

## Ownership model

| Dependency                           | Source of truth                       | How Poratake consumes it                                                     | Where fixes belong                                                            |
| ------------------------------------ | ------------------------------------- | ---------------------------------------------------------------------------- | ----------------------------------------------------------------------------- |
| HeroGPUI                             | `github.com/Porabuild/HeroGPUI`       | Exact crates.io pin, currently `herogpui = "=0.15.4"`                        | Fix in HeroGPUI, release it, then bump the pin here                           |
| GPUI framework and platform backends | `vendor/gpui-pre*` in this repository | `[patch.crates-io]` path overrides                                           | Fix in the vendored patchset now; upstream clearly generic fixes to Zed       |
| Zed GPUI                             | `github.com/zed-industries/zed`       | Not a direct dependency; it is the source snapshot republished as `gpui-pre` | Preferred upstream for generic GPUI framework and platform fixes              |
| `gpui-pre` crates                    | crates.io publication by `huacnlee`   | Base artifacts used to refresh the vendored copies                           | Not owned by Poratake or Zed; do not assume downstream patches can land there |

HeroGPUI is not vendored. Its component, theme, builder, test-support, and
facade behavior must be changed in the HeroGPUI repository. Never copy HeroGPUI
sources into Poratake to make a local correction.

GPUI is only partially vendored. Poratake has complete crate-package copies of:

- `vendor/gpui-pre`
- `vendor/gpui-pre-apple`
- `vendor/gpui-pre-macos`
- `vendor/gpui-pre-windows`

The other `gpui-pre-*` support crates still come from crates.io. This is not a
copy of the whole Zed repository.

## Why the GPUI patchset exists

`gpui-pre` is an unofficial crates.io snapshot of Zed's GPUI crates. HeroGPUI
pins the matching `gpui-pre` version exactly. Cargo package identity matters:
using another copy of GPUI, even one built from identical source, produces
incompatible types. Poratake therefore keeps the exact same package names and
versions but substitutes selected crates with `[patch.crates-io]` in
`src/main/Cargo.toml`.

That substitution applies to the entire dependency graph, including HeroGPUI.
It lets Poratake carry framework and platform corrections without waiting for
a new Zed snapshot or a HeroGPUI release, while keeping one GPUI type identity.

The vendored copies are a downstream patchset, not pristine crates. Every
intentional difference must be treated as a patch that has to survive the next
`gpui-pre` refresh.

## Required workflow for fixes

1. Classify the root cause before editing:
   - HeroGPUI component, theme, or facade behavior belongs in HeroGPUI.
   - GPUI windowing, rendering, input, accessibility, platform, or framework
     behavior belongs in the vendored GPUI patchset.
   - Poratake capture, window lifecycle, state, or product behavior belongs in
     `src/main/app-gpui`.
2. For HeroGPUI bugs, fix and test in `Porabuild/HeroGPUI`, publish the release,
   then update the exact pin in Poratake. Keep the `gpui-pre` version in
   `src/main/app-gpui/Cargo.toml` in lockstep with the GPUI version required by
   that HeroGPUI release.
3. For GPUI framework/platform bugs, fix the vendored crate directly. Do not
   add a Poratake-side wrapper that only hides the framework defect.
4. When the GPUI behavior is clearly wrong upstream, also prepare the equivalent
   patch against `zed-industries/zed`. The Zed patch is the long-term fix; the
   vendored patch protects Poratake until a future `gpui-pre` snapshot contains
   it.
5. Update the patch inventory in this document in the same commit as every new
   intentional vendored difference. Include the purpose, files, and upstream
   status. A future upgrader must be able to distinguish intentional patches
   from upstream drift.
6. Do not relax the exact `gpui-pre` pin or mix versions. HeroGPUI, the direct
   `gpui-pre` dependency, and the vendored crates must describe one coherent
   GPUI snapshot.

## Current base snapshot

The active patchset is based on `gpui-pre 0.3.11`, which identifies itself as a
snapshot of `zed@f816e1f9263e3a8ef63c4baaa02e1441ce1c6038`.

Poratake currently consumes HeroGPUI `0.15.4`, which requires the same GPUI
version. Newer `gpui-pre` releases are not automatically compatible because
HeroGPUI pins the GPUI version exactly.

## Current downstream patch inventory

This inventory is grouped by behavior. When editing one of these areas, compare
the exact file diff before assuming every hunk still belongs to the group.

| Patch                                             | Main files                                                                                                                 | Purpose                                                                                                                                                                    | Upstream status                                                                               |
| ------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------- |
| Accessibility test hooks                          | `gpui-pre/src/app/test_context.rs`, `gpui-pre/src/platform/test/window.rs`                                                 | Expose AccessKit activation, tree updates, and actions to visual tests                                                                                                     | Test-support enhancement; not yet upstream                                                    |
| Sprite and image edge correctness                 | `gpui-pre/src/scene.rs`, `gpui-pre/src/window.rs`, `gpui-pre-apple/src/shaders.metal`, `gpui-pre-windows/src/shaders.hlsl` | Preserve source rectangles, clamp atlas sampling, and filter/premultiply image edges correctly for captures and previews                                                   | Candidate for upstream after separating the platform-independent parts                        |
| Metal memory and parked-window lifecycle          | `gpui-pre-apple/src/metal_renderer.rs`, `gpui-pre-macos/src/window.rs`                                                     | Reduce drawable/path-texture memory, release unused path textures, stop hidden display links, and recreate the tiny Metal layer used by hidden parked windows              | Partly Poratake-specific parking behavior; memory pieces may be upstreamable                  |
| macOS popup mouse movement                        | `gpui-pre-macos/src/events.rs`, `gpui-pre-macos/src/window.rs`                                                             | Deliver mouse-entered movement and enable mouse-moved events in popup panels so hover works                                                                                | Generic framework candidate                                                                   |
| macOS frameless window behavior                   | `gpui-pre-macos/src/window.rs`                                                                                             | Keep frameless windows resizable when requested and hide traffic-light controls when no titlebar exists                                                                    | Generic framework candidate                                                                   |
| macOS application activation                      | `gpui-pre-macos/src/platform.rs`, `gpui-pre-macos/src/window.rs`                                                           | Activate the owning application before making an explicitly activated window key, allowing accessory-app popup panels to become/resign key                                 | Generic framework fix; Zed patch prepared at `docs/patches/zed-macos-window-activation.patch` |
| Windows clipboard image interoperability          | `gpui-pre-windows/src/clipboard.rs`                                                                                        | Read and write image clipboard data through PNG/DIB/DIBV5 conversions, including `CF_DIBV5` output                                                                         | Generic framework candidate                                                                   |
| Windows DirectX resilience and memory use         | `gpui-pre-windows/src/directx_devices.rs`, `directx_atlas.rs`, `directx_renderer.rs`, `platform.rs`                        | Fall back to WARP when appropriate, handle device loss, reduce idle renderer memory, release unused resources, and coordinate device invalidation                          | Mixed generic fixes and Poratake-specific memory policy                                       |
| Windows hidden/staging window and frame lifecycle | `gpui-pre-windows/src/events.rs`, `platform.rs`, `window.rs`                                                               | Create hidden staging windows safely, preserve initial placement, wake visible windows for frames, and keep hidden windows from consuming unnecessary presentation work    | Mixed generic fixes and Poratake-specific warm-window behavior                                |
| Windows popup input and activation                | `gpui-pre-windows/src/events.rs`, `window.rs`                                                                              | Restore popup hover/outside-click behavior, support resizable popups, avoid spurious Alt input when normal foreground activation succeeds, and clear hover on pointer exit | Mostly generic framework candidates                                                           |
| Windows window destruction safety                 | `gpui-pre-windows/src/window.rs`                                                                                           | Use weak drag-drop references and avoid destroying already-invalid window handles                                                                                          | Generic framework candidate                                                                   |

The exact current delta is authoritative, not this summary. Before upgrading,
generate the real diff as described below and reconcile every hunk.

## Upgrading `gpui-pre`

Do not upgrade GPUI merely because a newer `gpui-pre` exists. HeroGPUI must
first support and pin that version. Once a coherent target version exists:

1. Record the old version, target version, and matching Zed revisions from the
   crates' `[package.metadata.gpui-pre]` metadata.
2. Download pristine copies of the old crates:

   ```bash
   base=0.3.11
   rm -rf /tmp/gpui-pristine-$base
   mkdir -p /tmp/gpui-pristine-$base
   for crate in gpui-pre gpui-pre-apple gpui-pre-macos gpui-pre-windows; do
     mkdir -p "/tmp/gpui-pristine-$base/$crate"
     curl -L --fail "https://crates.io/api/v1/crates/$crate/$base/download" \
       -o "/tmp/gpui-pristine-$base/$crate.crate"
     tar -xzf "/tmp/gpui-pristine-$base/$crate.crate" \
       -C "/tmp/gpui-pristine-$base/$crate" --strip-components=1
   done
   ```

3. Diff each vendored crate against the pristine old crate:

   ```bash
   for crate in gpui-pre gpui-pre-apple gpui-pre-macos gpui-pre-windows; do
     git diff --no-index --stat \
       "/tmp/gpui-pristine-$base/$crate/src" "vendor/$crate/src"
   done
   ```

4. Reconcile every hunk with the inventory above. Drop hunks that were merged
   upstream, forward-port hunks still needed, and update the inventory for any
   changed upstream status.
5. Replace the vendored crate contents with the target snapshot and reapply the
   surviving intentional patches. Do not copy generated `.crate` metadata or
   unrelated files blindly.
6. Update the exact pins in `src/main/Cargo.toml`, `src/main/app-gpui/Cargo.toml`,
   and the lockfile through Cargo. Keep HeroGPUI's required GPUI version exact.
7. Run formatting, clippy, and the full GPUI test suite. Then run platform smoke
   tests for tray menus, capture overlay reuse, window activation, clipboard
   images, and memory idle behavior on macOS and Windows.

## Preparing a Zed upstream patch

The vendored crates are generated snapshots, so do not paste their normalized
`Cargo.toml` files into Zed. Use a sparse Zed checkout and patch the original
source files only:

```bash
git clone --filter=blob:none --sparse --depth 1 \
  https://github.com/zed-industries/zed.git /tmp/zed-upstream
cd /tmp/zed-upstream
git sparse-checkout set crates/gpui_macos crates/gpui_apple
```

Map the vendored crate names back to Zed crates using
`[package.metadata.gpui-pre]`; for example `gpui-pre-macos` maps to Zed's
`gpui_macos`. Keep the upstream patch minimal, remove Poratake-specific behavior,
and include a regression test when Zed's test infrastructure can express one.
Store prepared but unsubmitted patches under `docs/patches/` and link them from
the inventory so the handoff is not lost.
