# UI comparison

On Windows, run the visual regression matrix:

```sh
bun run test:visual
```

It covers light/dark, 950/1280 logical pixels and 100/125/150/200% simulated
scale with the native Windows renderer and embedded fonts. Geometry and exact
pixel comparisons fail independently. The Windows CI job uploads captured and
diff PNGs plus `renderer.json`, which records the GPU and driver. Physical monitor
scaling remains a separate native check.

Each Windows case first renders adjacent red/white image atlas tiles at fractional
bounds. Interior and edge samples must retain the red source color independently
of image baselines; `image-atlas-sampling.png` preserves the GPU evidence.
The vendored GPUI 0.3.11 Windows backend clamps sampling to tile texel centers.
Without that shader fix, the local probe sampled neighboring white pixels.

The font-size submenu checks all four sides of the keyboard focus ring on the
current, first and last options. Samples must resemble the active focus color
more than the panel background. `submenu-text-focus-*.png` preserves those
checks separately from the baselines. The vendored HeroGPUI 0.15.1 Select keeps
its existing row/panel dimensions while moving padding inside the virtual
viewport so the ring remains visible.

Exact baselines live in `baselines/win32/<theme>-<width>-<scale>/<renderer>/`.
The renderer name comes from GPUI's software/hardware flag and device name;
the console and `renderer.json` show the selected profile. An unreviewed renderer
fails rather than falling back to another device's images. Record its images,
review the geometry and visual output, then commit that profile. Driver changes
must still pass the exact pixel comparison.

To replace baselines deliberately:

```sh
bun run test:visual --update-baselines
```

Review the captured states before committing `baselines/win32/`. This command
also removes obsolete baseline states; normal runs reject changed state lists.
The console prints the temporary evidence directory. Do not accept a baseline
update to silence a regression without inspecting the changed image.

Run the Electron renderer with its fixture preload bridge:

```sh
bunx vite --config scripts/parity/vite.renderer.mts
```

Use `?window=screenshot&platform=darwin&image=/@fs/<absolute PNG path>` or
`?window=video-editor&platform=darwin&file=<HTTP video URL>`. Fixture files must
be under the repository or the system temporary directory allowed by Vite.

Capture GPUI with its real platform renderer and fonts:

```sh
PORATAKE_PARITY_OUTPUT=/tmp/poratake-parity \
PORATAKE_PARITY_IMAGE=/absolute/path/capture.png \
cargo run --locked --manifest-path src/main/Cargo.toml \
  -p poratake-gpui --features parity -- --parity
```

The optional `parity` feature is excluded from normal builds. The command uses
a fresh isolated profile and exits before normal app startup. It renders at
1280×738 logical pixels by default and advances GPUI's clock for hover and press
animations. `PORATAKE_PARITY_THEME`, `PORATAKE_PARITY_WIDTH` and
`PORATAKE_PARITY_SCALE` select a matrix case.
Captures include the editor header, held buttons, selected tools, open dropdowns,
the color picker, recording controls and loaded video sidebars. The runner asserts selected
tools and menu state so a failed interaction cannot silently produce mislabeled
evidence. The held microphone button advances through its press animation and
must differ visibly from hover within that button's measured bounds.

The built-in fixture additionally checks annotation save/history reopen/flattened
PNG reopen at 1280px and 100% scale, comparing the preview with saved pixels.
It also compares a translucent patch in each window-frame style against the saved
PNG at simulated 125% and 200%, with and without a frame shadow. A no-background
case checks saved alpha against the preview over the editor's display background.
With zero padding, the frame title bar and translucent content are also compared
at both scales; Polish copy is checked against editor export separately.
Rounded corners, insets, balance and attached layers are not part of those checks.
Custom `PORATAKE_PARITY_IMAGE` inputs skip that fixed-size workflow. Overlays use
a synthetic desktop with ordered overlapping windows, negative display origins
and correctly scaled physical bounds. Window/display picker states cover hover,
clipping at display edges and pointer leave. Pixel assertions check highlighted
and dimmed regions independently of baselines. GPUI's headless windows always
report non-hovered, so the fixture supplies ownership to the shared production
surface; physical pointer ownership still needs native verification.

The 1280px, 100% built-in cases also run production window, area and all-in-one
display capture startup and save/history/editor output for live, frozen, failed
and unavailable-freeze paths. The fixture checks exact physical rectangles and
requires the frozen frame to remain alive until cropping finishes. An error or
a successful `frozen: false` response must capture live pixels. Area input checks
the release coordinate independently of the last pointer move.

Timer workflows verify completion, cancellation, timeout, countdown-start
failure and a successful retry. The selection frame must be released before the
countdown, and the eventual capture must use live pixels. Events can arrive
before the countdown's `show` response. Cancelled, timed-out and failed timers
must leave no saved image, history entry or active selector.

Twelve OCR/QR scenarios check successful trimming, empty recognition, service
and capture failures, Escape cancellation and retry. They verify clipboard
preservation, notification copy, temporary-file cleanup and absence of library
entries. The headless app owns its clipboard; parity notifications are recorded
at the platform boundary instead of being sent to the OS.

Nine scroll scenarios check selection cancellation, completion/cancellation
before the start response, start/finish failures, empty output, held finish and
cancel responses, and retry. The session stays active until each held response.
Scroll selection must stay live even when freezing is enabled. Saved portrait
output must match history/editor exports. Eight editor-attachment scenarios
check all four edges, live/frozen/fallback pixels, capture and invalid-image failures,
cancellation, cleanup, clipboard preservation and identical history reopen.
They activate the actual edge strips with left and right mouse buttons and reject
an annotation draft from the same click. Each matrix case checks the strips' 32px
hit targets and all four hover indicators, with default and hover image baselines.
Native controls expose named buttons; unit tests exercise keyboard activation and
verify the focus borders remain inside the strips.
Pin workflows compare rendered annotations with exported pixels and restore
the editor with history enabled or disabled without adding history entries.
Unit regressions invoke the OS should-close callback and verify annotated
restoration with history disabled plus standalone close without an editor.
A native Windows Alt+F4 test closed the pin window and restored its annotated
editor; native close delivery on other platforms remains unverified.
The built-in 1280px, 100% cases add 28 workflow images each. The 950px cases add
ten history images each: default and selected grid, selected list, and both
list-action tooltips at 200px and 240px. The fixture checks thumbnail pixels,
keyboard selection, tooltip bounds and clipping masks, and deletes a scratch
item without opening an editor. A complete renderer profile has 2,408 images.
Each renderer's set must be reviewed before acceptance.

Capture, timer and scroll completion first open a bottom-right preview without
opening an editor. The workflow explicitly double-clicks that preview before
checking editor/export pixels. Initial editor dimensions are saved immediately;
opening small and large images preserves them, including after a user resize.
The pooled selector regression requires old rectangles and drag state to clear
when the overlay is parked.

Tray window height must equal its painted menu at every scale; the estimate is
used only until layout is measured. Each case also records the fitted tray menu.

Pin default, hover, enlarged and narrow windows run in every matrix case.
Source pixels must remain centered, fit the viewport and never upscale beyond
their natural logical size; close controls must stay inside the viewport.
The daemon fixture makes pin sizing use the matrix's simulated DPI rather than
the host monitor's geometry.

Capture-preview single cards and four-card stacks have top/bottom image
baselines in every matrix case. Four corner configurations check exact card
sizes, spacing, newest-four eviction and source-color order. Double-clicking
the newest card must open that capture in the editor with identical exported
pixels. Auto-dismiss must close the preview without deleting source files or
changing history. Hover fixtures use the production preview surface and HeroGPUI
tooltips. Close, delete, copy and upload tips must fit inside cards in all four
corners. Polish preset names wrap or ellipsize within the card, and Edit remains
clickable after each tooltip. Video Copy and Show in Folder tips use a missing-media
fallback fixture. Edit has no tooltip. A build with tooltips disabled fails the
presence assertion. A synthetic four-display menu must stay within each card and
remain open after pointer leave. GPU samples check all four sides of the first
and last keyboard focus rings and reject clipped text or exposed controls at the
scrolled menu edges; separate tests cover mouse selection, display
removal, preview repositioning, eviction and short-menu scrolling. The display
change test uses a headless window; native docking, click-through regions,
physical monitor changes and pointer ownership remain separate verification.

Recording controls use the production window and recorder commands with a
same-executable JSON-RPC fixture. Device names are deterministic, including long
labels. Assertions cover scoped device enumeration, countdown cancellation,
pause/resume, live toggles, saved-preference preservation and project discard.
These checks verify UI state and requests, not hardware media capture. A
parity-only guard assumes granted permission while the daemon fixture is
active, before reading host consent or opening permission dialogs. Normal app
permission checks retain their platform behavior.
The microphone icon's actual canvas bounds have a contrast assertion, so a
white icon on a light toolbar cannot become an accepted baseline. Its dropdown
chevron is excluded from the measured region.

Loaded-video baselines cover all ten panels using a real encoded project with a
drawing. `src/main/app-gpui/src/ui/parity/fixtures/capture.mov` fixes the input
across encoder backends: an owned synthetic blue/gray checker, H.264, 320×180,
24 frames at 12 fps, without audio. Its SHA-256 is
`69b5a0e2111a59e2d25c7389833d5583c5c012e7932677f77104586f6f5e57c0`.
Encoding and export are exercised separately by the video workflow tests.
The Windows export checks include camera on-periods mapped across a cut and a
manual 2× zoom, with project save/reload and seven decoded preview/output samples.
Independent camera visibility, source-color and checker-change assertions prevent
both pipelines from passing by ignoring a layer. Hardware capture and editor
control delivery remain separate verification.
Wallpaper changes autosave through the production editor; the preview
must equal a fresh composition of that saved project. Video windows respect the
editor's 1200×750 minimum, so the 950px matrix case uses 1200px for loaded video.
See [the audit](../../GPUI-UI-PARITY-AUDIT.md) for the current
workflow evidence and remaining backlog.

Match viewport, scale, theme, fixture and interaction state before comparing
images. Compare the changed surface separately from video playback, OS window
chrome and automation cursor markers.
