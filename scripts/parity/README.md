# UI comparison

The macOS-only `--native-smoke-recording` entry point is available in a
`--features parity` development build. It requires an existing isolated absolute
`PORATAKE_CONFIG_DIR`, an existing absolute `PORATAKE_NATIVE_SMOKE_OUTPUT`, a
`PORATAKE_NATIVE_SMOKE_RECT` in x,y,width,height points, and
`PORATAKE_DAEMON_PATH`. Its stdin commands drive the recording handlers or
`capture area`, `capture window`, and `capture timer`; `pointer down x y`,
`pointer move x y`, and `pointer up x y` dispatch events inside the selector.
`snapshot` captures the configured rectangle while the first selector is still
visible, with that selector's native ID excluded, to `active-selector.png` in
the configured output directory.
Coordinates are local to its first overlay window. The target defaults to area;
`PORATAKE_NATIVE_SMOKE_TARGET` can select `screen` or `window`, and the latter
also requires `PORATAKE_NATIVE_SMOKE_WINDOW`.

The macOS smoke in the audit used a temporary daemon that preflighted its own
screen access and disabled ScreenCaptureKit audio and input tracking. An
ordinary daemon may request other permissions even when the recording settings
disable audio, so use this harness only with a suitable already-authorized test
daemon and isolated synthetic windows. It does not establish physical-pointer,
audio, camera or other-display coverage.

On Windows, run the visual regression matrix:

```sh
bun run test:visual
```

It covers light/dark, 950/1280 logical pixels and 100/125/150/200% simulated
scale with the native Windows renderer and embedded fonts. Geometry and exact
pixel comparisons fail independently. The Windows CI job uploads captured and
diff PNGs plus `renderer.json`, which records the GPU and driver. Physical monitor
scaling remains a separate native check.

The picker fixture scans every pixel across the lower viewport for undimmed or
double-dimmed gaps. The recording handoff's unit test checks every painted pixel
around centered and edge selections at 100/125/150/175/200% simulated scale. History
grid tests sweep 140–400 logical pixels at 100/125/150/175/200% and check that
cards stay in bounds after resize. A visible keyboard selection stays in view
when the layout or DPI changes; an item scrolled out of view stays there.

The all-in-one color picker captures its center, four display corners and a
400×300 narrow window. It checks the sampled loupe pixel and the card's bounds;
the narrow case also checks that the card does not cover the sampled point.
Unit tests exercise five simulated scales, click-to-copy through the actual
overlay and a 200×180 case where preserving the cursor gap requires clipping.
The capture-failure workflow also checks the stopped-state redraw, restored
overlay, error notification and removal of the partial frame.
The all-in-one Capture text, Pick color and Close tooltips are captured in normal
and 400×300 overlays, and while Capture text is selected. Their measured bounds
must stay below their controls and inside the viewport and clipping masks. The
narrow overlay also checks the target menu's placement, keyboard selection and
focus restoration: Escape closes the menu before the overlay, and Enter reopens
the menu from its trigger. Pointer selection and outside dismissal must not start
an underlying capture. Repeated Enter/Space keydowns must not reopen the menu on
release. A separate 175% tooltip and target-menu probe passed in both themes;
the image-baseline matrix uses the four scales listed above.

Each Windows case first renders adjacent red/white image atlas tiles at fractional
bounds. Interior and edge samples must retain the red source color independently
of image baselines; `image-atlas-sampling.png` preserves the GPU evidence.
It also verifies that closing the headless test window destroys its hidden native
renderer window. The harness drains the Windows foreground queue on renderer
teardown so GPUI can complete its asynchronous window cleanup.
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
`PORATAKE_PARITY_SCALE` select a matrix case. Set
`PORATAKE_PARITY_EDITOR_HEIGHT` to exercise a shorter screenshot editor; this
also saves a full-window image for inspection. `PORATAKE_PARITY_VIDEO_HEIGHT`
sets the height of the loaded video-editor workflow on macOS and Windows.
On macOS, the hidden native popup probe checks its frame, content geometry,
click-through state and AppKit tooltip text replacement and clearing. It then
checks the three hidden scroll windows against a full-display selection and
verifies an off-screen selector's hide, lease and combined native exclusion IDs.
It does not start a daemon or capture the screen:

```sh
cargo run --locked --manifest-path src/main/Cargo.toml -p poratake-gpui --features parity -- --native-popup-frame-probe
```

On macOS, `--native-ui-probe` opens an isolated recording control and a preview
of a generated checkerboard without accessing capture devices. Tooltip hover can
start the configured daemon. It checks the AppKit frames and the preview's bottom-right work-area
margins. Set `PORATAKE_NATIVE_UI_THEME` to `light` or `dark`. Send `edit` to its
stdin to dispatch a simulated double-click on the preview, and `draw` to dispatch
a simulated drag in the editor after selecting an annotation tool. Send `quit`
or an empty line to close it. Set `PORATAKE_NATIVE_UI_NORMAL_LEVEL=1` to lower only
these probe windows to the normal AppKit window level for inspection by tools
that cannot target floating windows. This does not check production window
stacking, physical input delivery, saved capture pixels or native tooltip presentation.
`hover`, `hover-close`, `hover-delete`, `hover-status`, `tooltip-status` and
`hover-exit` exercise and report simulated preview pointer entry, requested tooltip
text and exit without moving the physical pointer. The reported text does not
establish that the daemon displayed the tooltip.
`rectangle` selects the rectangle tool and `annotation-status` compares the
editor's current PNG with its generated source. `reopen-saved` checks the one
saved PNG in the probe directory and opens it in a second isolated editor.
`preview-under-pointer` and `preview-away` reposition only the generated preview
relative to the stationary pointer, allowing AppKit entry and exit delivery to be
checked with `hover-status` without moving the pointer. `preview-close-under-pointer`
places the Close control under it for `tooltip-status`.
`record-hover`, `record-exit` and `record-tooltip-status` similarly check the
recording toolbar's tooltip registration without starting a recording.
With a fixture daemon configured, `record-open-camera-menu`,
`record-open-mic-menu` and `record-open-ios-menu` dispatch synthetic menu clicks;
`record-menu-status` reports the menu and native frame, and `record-dismiss-menu`
closes the menu. Wait until the reported frame has returned to the fitted size
before using `record-hit-test`. `record-scene-snapshot` writes the most recently
rendered recording-control scene to the probe's temporary directory without
capturing the screen. One way to run the probe with the existing fake
daemon is:

```sh
cargo build --offline --locked --manifest-path src/main/Cargo.toml -p poratake-gpui --features parity
probe_dir=$(mktemp -d)
cat > "$probe_dir/daemon" <<'SH'
#!/bin/sh
PORATAKE_PARITY_DAEMON=1 exec "$PORATAKE_NATIVE_FIXTURE_BINARY"
SH
chmod +x "$probe_dir/daemon"
PORATAKE_NATIVE_FIXTURE_BINARY="$PWD/src/main/target/debug/poratake-gpui" \
PORATAKE_PARITY_OUTPUT="$probe_dir" PORATAKE_DAEMON_PATH="$probe_dir/daemon" \
PORATAKE_NATIVE_UI_FAKE_DAEMON=1 PORATAKE_NATIVE_UI_NORMAL_LEVEL=1 \
  "$PWD/src/main/target/debug/poratake-gpui" --native-ui-probe
```

The fake daemon writes its requests to `daemon-fixture.jsonl` in that temporary
directory. It must be used for these menu commands: opening real device menus can
request macOS media permissions.
`record-click-sound` and `record-audio-status` exercise the isolated pre-recording
system-sound preference without starting capture.
`timer-edge-position` reports countdown positions for narrow selections at the
primary display's left and right edges. The hidden AppKit panel placement can be
checked against those eight reported display and position values with:

```sh
swiftc src/main/daemon/UI/Theme.swift src/main/daemon/UI/FloatingPanel.swift \
  src/main/daemon/UI/TimerButton.swift src/main/daemon/Core/DisplayCoordinates.swift \
  tests/native/timer-control-placement.swift -o /tmp/poratake-timer-control-placement
/tmp/poratake-timer-control-placement <display-x> <display-y> <width> <height> <left-x> <left-y> <right-x> <right-y>
```

The actual timer module's show, synthetic cancellation, completion and hide
paths can be checked using the two reported positions:

```sh
swiftc src/main/daemon/UI/Theme.swift src/main/daemon/UI/FloatingPanel.swift \
  src/main/daemon/UI/TimerButton.swift src/main/daemon/Core/DisplayCoordinates.swift \
  src/main/daemon/Modules/TimerControlModule.swift tests/native/timer-control-lifecycle.swift \
  -o /tmp/poratake-timer-control-lifecycle
/tmp/poratake-timer-control-lifecycle <left-x> <left-y> <right-x> <right-y>
```

This fixture briefly shows panels and brings its process forward, then restores
the previously frontmost app if the fixture is still frontmost. It mocks daemon
transport and does not capture the screen or access media devices.

`scroll-open`, `scroll-hover`, `scroll-exit` and `scroll-tooltip-status` open an
isolated scroll control and check its tooltip without starting auto-scroll or capture.
`display-open`, `display-hover`, `display-exit` and `display-status` open a small
isolated display picker and report simulated pointer state without starting capture.
Set `PORATAKE_NATIVE_UI_AUTO_DISMISS=1` to use a four-second preview dismissal
timer in this isolated probe.

Run `python3 tests/native/macos-save-panel-probe.py` on macOS to build the parity
executable offline, annotate a generated image in an isolated editor, check the
native save sheet's directory and suggested filename, and cancel the sheet. The
probe then writes a PNG directly through the screenshot save backend, reopens it
in a second editor and compares both editors' output with the saved pixels. Set
`PORATAKE_NATIVE_SAVE_EVIDENCE` to an existing absolute directory to retain the
generated source and saved PNG. It does not confirm the save sheet, start capture
or access media devices.

For manual foreground interaction with the isolated probe, verify that the
frontmost application PID matches the probe PID before sending input. A window
being visible, or an unverified activation result, does not establish focus.

On macOS with development FFmpeg available, set `PORATAKE_TEST_NARROW_VIDEO=1`
and run the ignored `drawing_tool_ui_edit_survives_reopen_and_matches_export`
Rust test to draw, reopen and export at 720×520. Set
`PORATAKE_TEST_VIDEO_EVIDENCE` to an existing absolute directory to retain the
synthetic preview frames, decoded frames, MP4 and comparison measurements.
On macOS, the 1280-point workflow uses generated media for area, screen and
window recording handoffs. `PORATAKE_PARITY_AREA_RECORDING`,
`PORATAKE_PARITY_SCREEN_RECORDING` and `PORATAKE_PARITY_WINDOW_RECORDING` can
instead point to existing local videos longer than 1.5 seconds. The fake daemon
copies those files; it does not start a capture or access audio or camera
devices. The editor workflow draws a zoom on the timeline and compares decoded
export frames with its preview. Set `PORATAKE_PARITY_RECORDING_ONLY=1` to run
just those handoffs at 1280 points and 1× simulated scale.
Set `PORATAKE_PARITY_SCROLL_ONLY=1` to run the isolated simulated scroll workflows
at the same width and scale, including selector exclusion IDs and cleanup.
Set `PORATAKE_PARITY_BLACKOUT_ONLY=1` to run the generated screenshot blackout
preview/export checks at the same width and scale on macOS.
Set `PORATAKE_PARITY_SCALED_BLACKOUT_ONLY=1` to compare a fractional blackout
over a shape with its reopened PNG at four zooms and three simulated scales.
Set `PORATAKE_PARITY_EVIDENCE` to an existing directory to retain the 2×,
137% comparison images.
Set `PORATAKE_PARITY_REDACTION_ORDER_ONLY=1` to compare blur and pixelation
before and after a shape with their reopened PNGs at simulated 1× and 2× scale
and 73%, 100% and 137% zoom.
The fixture checks that the shape is filtered only when it precedes the effect.
`PORATAKE_PARITY_EVIDENCE` retains the 2× filtered previews and reopened PNGs.
Set `PORATAKE_PARITY_OPAQUE_SOURCE=1` to repeat the screenshot checks with an
opaque source instead of the partially translucent generated region.
Set `PORATAKE_PARITY_ROUNDED_REOPEN_ONLY=1` to compare a transparent rounded,
shadowed screenshot with its reopened PNG at 73%, 100% and 150% effective zoom,
check macOS frames with and without shadows, and check redactions over a plain
transparent image.
Set `PORATAKE_PARITY_PIXELATION_ONLY=1` to check restored pixelation on opaque,
transparent and clipped generated images across the simulated scale matrix;
the displayed image is compared in full with the reopened PNG.
With the Vite server above running, run
`env -u ELECTRON_RUN_AS_NODE node_modules/.bin/electron scripts/parity/transparent-editor-probe.cjs`
to check a generated transparent PNG in an isolated offscreen Electron editor.
Set `PORATAKE_THEME=light` to repeat the check in light appearance.
With the same server, run
`env -u ELECTRON_RUN_AS_NODE node_modules/.bin/electron scripts/parity/blackout-editor-probe.cjs`
to compare a generated 2× image's blackout preview with its saved and reopened
PNG using the actual screenshot editor. The fixture hides the dashed editor
guide for the comparison and intercepts the save through its fake preload.
Set `PORATAKE_BLACKOUT_ORDER=1` to add a shape under the blackout and check that
it stays hidden; `PORATAKE_SHAPE_AFTER=1` places the shape afterward. Set
`PORATAKE_REDACT_STYLE` to `pixelate` or `blur` to check those effects instead.
`PORATAKE_ZOOM_STEPS`, `PORATAKE_ASPECT_RATIO`, `PORATAKE_FRAME`,
`PORATAKE_ATTACHMENT`, `PORATAKE_TRANSPARENT` and `PORATAKE_NOISE` vary the
generated composition. `PORATAKE_INTERACTION=1` selects, drags and resizes the
blackout before saving and comparing the reopened image. Set
`PORATAKE_PARITY_EVIDENCE` to an existing directory to retain the preview, saved
and reopened PNGs. The fixture checks generated pixels and does not capture a
display. Its offscreen Electron load can time out under heavy system load.
Run `env -u ELECTRON_RUN_AS_NODE node_modules/.bin/electron scripts/parity/shared-composition-probe.cjs`
to compare the shared export canvas with the encoded PNG for generated framed,
transparent and attached images in both appearances. The screenshot editor now
uses this same composition for its preview. `PORATAKE_ANNOTATION_HITS=1` checks
selection of nine annotation types, and `PORATAKE_COMPOSITION_CACHE=1` checks
cached background invalidation. `PORATAKE_COMPOSITION_PERF=1` measures a
synthetic 4K render; add `PORATAKE_COMPOSITION_PERF_NOISE=1` to include noise.
Set `PORATAKE_PARITY_TIMER_ONLY=1` to run the timer selection, completion,
cancellation, timeout, retry and display-edge fixture workflows.
The balanced macOS frame reference runs the Electron export hook in a browser
with synthetic images. With the Vite server above running and an isolated
browser profile, open `/scripts/parity/balanced-frame-reference.html?scale=1`,
then repeat for `1.25` and `2`, saving each rendered HTML document. Run
`python3 scripts/parity/compare-balanced-frame.py <gpui-profile> <html-1> <html-1.25> <html-2> --references <output-directory>`
against a GPUI parity profile produced at 1×. FFmpeg and FFprobe decode the
images; the script checks the three image interiors and reports differences over
a white matte. This is a browser execution of Electron's export hook, not a
packaged Electron run. To include the simulated 133⅓% case, save a browser result
at `scale=1.3333333` and add `--fractional-result <html>` to the comparison.
Use `scale=1&layers=0&inset=8` on the reference page to render the single-image
inset case, and pass its saved HTML as `--single-inset-result <html>` to include
it alongside the default layered cases in the comparison.
Use `scale=1&layers=0&alpha=1` and `--single-alpha-result <html>` for the
single-image transparency case; use `scale=1&layers=0&inset=8&alpha=1` and
`--single-alpha-inset-result <html>` for transparency with an inset.
For a two-stop 50%-alpha wallpaper behind a transparent image, save the result
at `scale=1&layers=0&alpha=1&wallpaperAlpha=1` and run
`python3 scripts/parity/compare-balanced-frame.py <gpui-profile> --references <output-directory> --single-translucent-wallpaper-result <html>`.
The comparison checks alpha in the wallpaper, translucent image and clear image
regions, then reports the whole-image difference over white.
Set `PORATAKE_PARITY_FRACTIONAL_FRAME_ONLY=1` with the normal 1× parity setting
to run framed-attachment and single-image inset comparisons at simulated 133⅓%
without the rest of the matrix. The same cases also run in the complete 1×
workflow. `PORATAKE_PARITY_BALANCED_FRAME_ONLY=1` runs the balanced single-frame
matrix at four simulated scales, with light and dark frames, shadows,
opaque and 50%-alpha backgrounds, insets, transparent source pixels, crop-edge
annotations and reopen. This matrix also runs in the
complete 1× workflow. It allows at most one rounded-edge pixel to differ by
three channel levels; all other pixels must stay within two.
Captures include the editor header, held buttons, selected tools, open dropdowns,
the color picker, recording controls and loaded video sidebars. The runner asserts selected
tools and menu state so a failed interaction cannot silently produce mislabeled
evidence. The held microphone button advances through its press animation and
must differ visibly from hover within that button's measured bounds.
At 950 points on macOS, the rectangle-tool case also checks vertical and
horizontal toolbar scrolling, access to the rightmost action, Copy activation,
and return to the contextual controls. A forced 720-point run can exercise the
same layout below the normal screenshot-editor minimum.

The built-in fixture additionally checks annotation save/history reopen/flattened
PNG reopen at 1280px and 100% scale, comparing the preview with saved pixels.
At simulated 125% and 200%, it compares every pixel of a 640×360 single-image
framed region with the PNG for all four frame styles, with and without shadow.
Translucent custom backgrounds with blur are checked after light/dark theme
changes, within two channel levels at the edges and three in the interior. It
also checks frame-only backgrounds, annotations at rounded frame corners, and
pointer interaction and visible handles for selection and crop.
On Windows, repeated frame changes verify that replaced edge images leave the
GPU atlas. Single-image previews without a window frame, inset, balance or
attachments are also compared at both scales, with and without corners or
shadows. The entire padded canvas is checked, including transparent backgrounds,
a corner annotation, and crop and selection handles. Translucent custom
backgrounds can differ by up to three color levels. For a transparent rounded
image with a shadow, at most one pixel on the rounded edge may differ by three
levels; the remaining pixels must stay within two. Unframed attachments without inset or balance are
checked on all four edges and in a five-layer composition,
including corners, shadows, transparent images, aspect ratios, save/reopen and
atlas eviction. Overlapping translucent shadows differ by four color levels in
at most 16 pixels; all other pixels use the limits above. Polish copy is checked
against editor export separately. Balanced single captures are checked at 100%,
125% and 200%, with zero or 20px padding, solid or transparent backgrounds,
translucent pixels, annotations, crop, undo and history reopen. Unframed single
images with inset and no corners or shadow are compared against saved pixels at
all three scales, with zero or 20px padding, transparent and solid backgrounds,
translucent pixels and annotations. Balanced inset cases check the source-edge
color and backdrop replacement after an equal-size crop. Plain translucent images
also check that annotation patches do not blend the source twice. Attachments on
all four edges without frames, inset, corners or shadows are
checked at all three scales with padding and background variants, including an
edge-crossing pixelated redaction and the reopened preview. The right attachment
also checks clipping outside the canvas. Balanced frames, corners, shadows,
arbitrary zoom and framed attachments remain outside these comparisons. In the
separate framed workflow, balanced macOS light and dark frames with top and right
attachments and a moved annotation are compared with export before and after
reopening at all three simulated scales.
In the
unframed layered path, annotations and black, pixelated and blurred redactions
crossing into attachments are compared with saved pixels at 125% and 200%;
overlapping and clipped redactions, crop, undo and atlas retirement are also
checked. A textured right attachment with inset checks all three redactions on
both sides of the boundary, blackout coverage, unchanged pixels outside the
redactions, preview/export equality and PNG reopen at 73% and 100% zoom. An
isolated Electron canvas comparison matched blackout and pixelation exactly;
blur differed by up to five channel levels. Framed attachments and a complete
Electron editor workflow remain outside these checks.
Screenshot pixelation uses fixed-size block averages checked against an Electron
canvas for alpha, clipping and scale. Restored plain previews are compared with
saved pixels at simulated 100%, 125% and 200%, including translucent and
clipped regions, 50% and 150% zoom, wallpaper toggles and a same-window DPI
change. A shape beneath pixelation is checked with and without gradient
wallpaper at 100% for opaque and translucent images. The redaction and its
edge band match a flattened PNG within two channel levels at 50%, 73%, 137%
and 150% effective zoom with simulated 100%, 125% and 200% DPI in both themes,
including clipped regions and overlapping later annotations. Blur and
pixelation drafts over a committed filtered redaction retain their dark drag
preview in single captures and attached-image compositions. Complex
compositions and physical DPI remain outside these checks.
The recorded video export fixture compares both blurred and pixelated striped
regions with decoded MP4 pixels across a cut and speed changes.
Committed screenshot blur is checked against saved pixels in unframed captures
at the same three simulated scales, with transparent pixels, preceding and later
annotations, and a clipped region. A CPU test checks low-zoom blur sampling and
replay beneath a later annotation at aligned offsets and all four surface edges.
Earlier runs matched the blur region and neighboring annotations to a flattened
PNG within two channel levels at 50%, 73%, 137% and 150% effective zoom with
simulated 100%, 125% and 200% DPI in both themes. Repeat them for the patch-local
change. A whole-frame comparison found differences outside the checked region
for clipped blur, and patches exceeding the image budget use the previous
rendering path. Sampled Electron canvas blur profiles now differ by at most three
channel levels, and translucent clipped screenshot samples by at most two. For
unframed single images without balance, corners or shadows, the gradient or inset
is now composed before redaction in the export and CPU patch tests. Twelve
Electron samples, including a clipped blur, are within three channel levels; the
hosted visual run at `fb13d389` failed a transparent wallpaper preview and a
fractional aspect-ratio comparison. The stage background and comparison bounds
have been corrected; alpha-gradient and transparent-image cases were added. The
hosted follow-up passed those wallpaper checks but failed a zoomed blackout edge
in both themes. Local light/dark NVIDIA workflows at 1280px and 100% passed the
updated checks and 191 exact baselines per theme; hosted verification is pending.
A CPU case confirms the
sampled backdrop matches export at a rounded fractional origin. Screenshot export
and CPU preview patches now apply redactions, vectors and highlights in annotation
order, with a rounded blackout pass. Earlier Electron corner samples used the
previous composition order. Both blackout/shape orders match reopened PNG pixels
at 73% and 150% zoom in those local workflows. The live editor case remains
unverified.
Corner and shadow effects with zero padding are also compared without a
background for single images and unframed attachments. A same-window theme
change checks the transparent matte and replacement of the old backdrop. Plain
attached captures are checked for visible seams on both sides; annotations and
redactions crossing a translucent attachment are compared against the PNG with
the theme matte applied. The Windows GPU fixture also checks 19,200×160 and
160×19,200 attached compositions, translucent tile joins, the last tile,
annotations, scrolling at 25% zoom and tile eviction. A unit test reconstructs
a two-dimensional tile grid; large two-dimensional GPU surfaces remain untested.
Separate CPU checks save and reopen 17,000px-wide and -tall captures with
plain, inset and framed shadows and compare their preview shadows with the PNG.
GPUI GPU rendering of those plain, inset and framed shadows remains unverified.
Custom `PORATAKE_PARITY_IMAGE` inputs skip that fixed-size workflow. Overlays use
a synthetic desktop with ordered overlapping windows, negative display origins
and correctly scaled physical bounds. Window/display picker states cover hover,
clipping at display edges and pointer leave. Pixel assertions check highlighted
and dimmed regions independently of baselines. GPUI's headless windows always
report non-hovered, so the fixture supplies ownership to the shared production
surface; physical pointer ownership still needs native verification.
Four small selections at the display corners also check the painted size-label
bounds and clipping mask. At the bottom edge, the label must clear the selection.
Pixel scans check the surrounding dim layer and fractional-position selections
for undimmed seams or double-dimmed overlaps; selected pixels must remain undimmed.
Samples on all four sides also check the dim, selected interior and outside outline.

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
The dark 1280px/100% case additionally holds a stitch for 31 seconds and checks
that the session remains active, then saves and reopens the expected pixels.
Scroll selection must stay live even when freezing is enabled. Saved portrait
output must match history/editor exports. At 1280px, the production macOS GPUI
preview panel is also rendered through the Windows GPU backend at every matrix
scale; the center must retain its source color and all four corners must be
clipped. Eight editor-attachment scenarios
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
27 history images each: default and selected grid, selected list, and both
action tooltips at 140px, 200px and 240px. The fixture checks thumbnail pixels,
keyboard selection, all four sides of the narrow selection ring, tooltip bounds
and clipping masks, and deletes a scratch item without opening an editor. A
complete renderer profile has 2,880 images. The new hosted history states were
reviewed, and the complete hosted profile passed exact comparison in
[CI at `0b79bd9`](https://github.com/Porabuild/Poratake/actions/runs/42992001485).
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
The camera fixture also delivers a dragged-position event and checks that a
disable/enable cycle restores that position while rejecting invalid or stale
position events.
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
