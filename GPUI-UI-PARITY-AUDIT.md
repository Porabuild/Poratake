# GPUI versus Electron verification audit

Refreshed 2026-11-10 for Poratake 0.9.6, based on `ac384ae` plus this change.
Electron remains the product reference. This is a verification record and current
backlog; the earlier implementation plans are historical findings, not an open-bug
list. A source comparison or passing unit test does not establish full manual parity.

## Versions and dependency evidence

| Component          | Current source                                                                                                                                                              |
| ------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| HeroGPUI           | Registry version exactly 0.15.1 in `src/main/Cargo.toml`, with all four crate checksums in `Cargo.lock`; source tag `v0.15.1` at `09ec52be169bf25a6351a14088e34e22fe89fe9f` |
| GPUI               | `gpui-pre` and `gpui-pre-platform` exactly 0.3.11, matching HeroGPUI's workspace                                                                                            |
| Electron reference | `@heroui/react` and `@heroui/styles` 3.2.4 in the installed lockfile; manifest range `^3.2.4`                                                                               |
| HeroGPUI reference | HeroUI 3.2.7; its component audit is separate from Poratake's app parity                                                                                                    |
| Bundled guide      | `.agents/skills/herogpui/upstream.json` records revision, versions, source hashes and adapted local hashes                                                                  |

[HeroGPUI #43](https://github.com/Porabuild/HeroGPUI/pull/43) is merged. The
`v0.15.1` tag points to the release source commit. Release workflow
[42747398534](https://github.com/Porabuild/HeroGPUI/actions/runs/42747398534)
completed successfully, including SemVer, platform tests, gallery artifacts and
registry publication. The public release and 0.15.1 registry packages were checked.
Poratake's published-registry update in `ac384ae` is retained as the base of this change.

The former 0.3.3 GPUI reference, old HeroGPUI revision, hand-rolled control findings
and broad surface-level “Match” claims have been removed from this audit.

## Automated visual gate

Run `bun run test:visual` on Windows. The optional `parity` feature uses an isolated
profile, a deterministic 640×360 capture and the real Windows renderer, embedded
Geist fonts and DirectWrite. Hidden native render targets provide GPU readback;
input and frame scheduling use GPUI's headless harness. This is not installed-app
or physical-monitor DPI verification.

The matrix is light/dark × 950/1280 logical pixels × 100/125/150/200% scale, at
738 logical pixels high. Each case captures 117 states; the two 1280px, 100% cases
also capture 28 annotation/selection/timer/scroll/attachment/pin workflow states, for 1,928 baseline images per renderer.
Loaded-video windows respect their 1200×750 minimum: the 950px cases use 1200px
for that surface. Recording controls use their measured content width.

| Surface             | Exercised evidence                                                                                                                          | Limits                                                                                                    |
| ------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------- |
| Screenshot editor   | Default, representative hover/held press/keyboard focus, selected tools, thickness/color/tool panels and keyboard-operated nested menus     | Not every control's complete interaction matrix                                                           |
| Area overlays       | Screenshot, OCR, QR, timer, scroll, recording and editor-attach default/drag-selection surfaces; standalone window/display picker states    | No OCR/QR service result or stitched-scroll output from these fixtures                                    |
| All-in-one          | Screenshot/record area, window and display pickers, plus OCR area; frontmost and clipped window highlights, screen hover and pointer leave  | Window ownership is simulated; physical pointer transitions remain unverified                             |
| Recording controls  | Area/window defaults, hover/held press/keyboard focus, long device menus, countdown/cancel, active/paused/muted and locked camera choices   | Fixture RPCs assume granted media permission; hardware and real consent remain native checks              |
| Video editor        | Empty editor states plus a loaded encoded project, drawing, wallpaper and all ten side panels                                               | Playback and every panel's control states are not yet image baselines                                     |
| Annotation workflow | Annotated preview, history reopen and flattened PNG reopen                                                                                  | Built-in fixture at 1280px/100%; custom comparison images skip this workflow                              |
| Pin window          | Default, hover, enlarged and narrow windows; image bounds and close control; annotated editor restoration with history enabled and disabled | Simulated scale and headless input; native movement and OS close remain unverified                        |
| Capture preview     | Single cards and four-card stacks; four corner configurations, newest-four eviction, editor handoff and auto-dismiss                        | Headless content/layout proof; native docking, click-through regions and hover controls remain unverified |

Geometry assertions check 21 toolbar entries, 28/32px control sizes, spacing,
nonoverlapping options, visible caption buttons and the ability to scroll overflow
controls into view. Caption widths follow GPUI's device-pixel rounding. Selection
edges allow at most half a physical pixel of snapping. Menu actions assert selected
tools and open panels before their screenshots are named. Recording-bar children
must fit their viewport without overlapping. The recorder fixture asserts one
start/pause/resume/stop call, enabled inputs at start, disabled live toggles and
unchanged saved preferences. Discard must remove the sole project inside the
isolated recordings directory.
Picker fixtures place ordered overlapping windows on a negative-origin display
in correctly scaled physical coordinates. Pixel samples independently check the
undimmed candidate and 50% scrim, including targets crossing left, right and bottom edges
and restoration of dimming on leave. The headless platform always reports a
non-hovered window, so the fixture supplies window ownership to the shared
production surface; production rendering still reads the native hover state.
The microphone icon must provide enough pixels with at least 3:1 contrast
against the toolbar. Its actual canvas bounds are measured independently of the
dropdown chevron and checked against the containing button. The former
light-theme white content failed with zero contrasting pixels. Media permission
checks return granted only in a parity build running the daemon fixture, before
reading host consent or opening a permission dialog.
Held-button capture advances the mock clock before reading pixels and must
differ from hover inside the measured microphone button bounds.

Baseline comparison requires identical image dimensions, state filenames and every
pixel. A changed pixel fails; failed images get a highlighted diff. Updates require
`bun run test:visual --update-baselines`, which records replacements and removes
obsolete `gpui-*.png` states. Review images and geometry before committing updates.
CI runs the visual gate in the Windows native job and preserves PNG evidence
and GPU/driver metadata. Exact images are selected by GPUI's software/hardware
flag and device name, under each matrix case's renderer directory. Missing
profiles fail; no tolerance or fallback accepts another renderer's output.
Loaded-video captures use one pinned synthetic H.264 checker fixture rather than
re-encoding their input on each host. Separate workflow tests still encode/export.

The first [hosted Windows run](https://github.com/Porabuild/Poratake/actions/runs/42779094342/job/129704528224)
at `db35386` passed native checks and every visual case's geometry/workflow
assertions, then failed all 16 exact image comparisons against the local GPU's
images. Many differences were one or two channel levels; some UI edges and video
preview pixels differed more. Renderer-specific reviewed baselines and the fixed
video input address the differing capture environments without weakening the
pixel check. The hosted profile must still be recorded, reviewed and rerun.

The final local comparison passed all 16 cases. Contact sheets for all cases and
full-resolution narrow menus were reviewed. An isolated baseline with one changed
pixel failed with exit 101 and produced a diff image.

## Workflow and regression evidence

The commands in `bun run checks` passed against the published HeroGPUI 0.15.1
base: 180 Vitest files, 2,224 tests passed and three skipped; 956 native tests
passed and one skipped. Typecheck, lint, format, React Compiler, Rust Clippy and
license/advisory checks passed. After the composition-fit change, typecheck,
lint, format, the native gates and parity-feature Clippy passed again.
Render-unit-test state could lazily start the ambient native daemon, leaving
deferred capture work running after a test. Unit fixtures now inject an empty
daemon path. A controlled fake ambient daemon reproduced the failure before the
fix and was never started afterward. The native suite then passed 957 tests and
one skip without leak warnings; 40 repetitions of the previously flagged tests
also passed without leaks. Parity builds retain their explicit fake RPC client.
The latest completed 16-case comparison passed all 1,928 images, including the
permission isolation, icon-only contrast and capture-preview checks. The 64 added
preview images were reviewed; the preceding 1,864 images stayed byte-identical.
Moving these 1,928 images into the local NVIDIA renderer profile preserved every
hash. The fixed video input and renderer-profile lookup then passed a fresh
16-case exact comparison of all 1,928 images.
The attachment fit
assertion first failed with a 2,201.6px composition in a 1,280px viewport, then
passed after the fit cache included content dimensions. The pin's annotated
render has zero changed pixels against its saved PNG in the fixed fixture.

OCR/QR/scroll checks passed 19 production workflow scenarios and 53 Electron
reference tests in four files. Packaging/compliance checks passed 15 tests.
The full-window QR regression failed before the fix and passed afterward. The
rebuilt native daemon improved exact payload matches from 38/65 to 43/65 without
losing a previous success. The single-pixel baseline failure regression also
passed separately.

Hosted [Linux GPUI CI](https://github.com/Porabuild/Poratake/actions/runs/42779094342/job/129704528155)
passed at `db35386`: 797 GPUI tests, 28 Linux-daemon tests including the full-window
photographed QR regression, and both X11 capture runs (normal and `GDK_SCALE=2`).
This adds Linux service/test evidence; it does not establish interactive Linux
app parity or physical monitor scaling.

Hosted [macOS tests](https://github.com/Porabuild/Poratake/actions/runs/42779094342/job/129704528234)
passed at `db35386`, including 799 GPUI tests, the Swift daemon build, Rust checks
for Apple Silicon and Intel, and Clippy. Interactive macOS app parity remains
separate verification.

| Check                           | Verified result                                                                                                                                                                                                                                                                                                                    |
| ------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Annotate → save → reopen        | Real pointer events create pen, highlight, rectangle and redact annotations. Mouse release differs from the last move. Persisted history reopens identically; flattened PNG reopens without editable annotations and with identical decoded output.                                                                                |
| Screenshot preview versus PNG   | Native rendered image region versus saved output: zero pixels differ by more than two channel levels. Rectangle/circle/line/arrow previews use the export rasterizer; cache tests distinguish captures with identical annotations.                                                                                                 |
| Edited video versus MP4         | Windows Media Foundation fixture, trim/gap removal, 2×/0.5× speed, wallpaper and drawing; project save/reload; decoded output dimensions/duration, four sampled frames, whole-frame and drawing-region error checks.                                                                                                               |
| Native screenshot/editor        | Owned editor window captured through the daemon; native pointer annotation and Save Screenshot dialog produced a real PNG.                                                                                                                                                                                                         |
| Native static recording         | Pause/resume recording formerly finalized at 0.0333333s; fixed daemon reports 1.6345311s, decoded stream 1.634500s, excluding the pause.                                                                                                                                                                                           |
| Native changing recording       | Pointer/shape edits during recording finalized at 52.0008992s; decoded stream 52.000867s with nine change-driven frames, exercising the tracked-texture pool.                                                                                                                                                                      |
| Native record → edit → export   | The static recorded project opened in the GPUI editor, accepted a wallpaper change and exported through the native Save dialog: 1052×750, 50 frames, 1.666633s at 30 FPS.                                                                                                                                                          |
| Recording-control UI/RPC        | Production controls with deterministic device/recorder responses; Escape cancels countdown and leaves an active recording open; pause/resume and live input changes issue the expected requests; discard removes the fixture project.                                                                                              |
| Loaded-video state and preview  | Production editor opens an encoded project with a drawing; wallpaper/export changes autosave and reload; the UI preview bytes equal a fresh composition of persisted state. All ten side panels have image baselines.                                                                                                              |
| Native countdown cancellation   | Accessibility tools identify the named recording-control window; Start displays a ten-second countdown and a background Escape message closes the bar before recording starts.                                                                                                                                                     |
| Native light control menu       | A light-theme development window displays readable controls, enumerates the actual microphone list, opens its menu, dismisses it with Escape and closes through its accessible Close button.                                                                                                                                       |
| Selection capture routing       | Production window, area and all-in-one display startup, saved pixels, history registration and editor-export equality for live, frozen, failed and unavailable-freeze paths. Exact physical rectangles and capture-before-release ordering are asserted.                                                                           |
| Timer capture lifecycle         | Production selection/countdown completion, cancellation, timeout, show failure and retry. The frozen selection frame is released before countdown; completed captures use live pixels; unsuccessful attempts leave no image/history/active selector.                                                                               |
| OCR/QR capture lifecycle        | Twelve production scenarios cover success, whitespace-only results, recognition error, partial-file capture error, Escape cancellation and retry. Clipboard/notification outcomes, preprocessed OCR input, temp-file deletion and no history/output leaks are asserted.                                                            |
| Scroll capture lifecycle        | Seven production scenarios cover selection cancellation, terminal events before the start response, start/finish errors, empty output and retry. Exact live-screen parameters, cleanup and saved 640×900 output/history/editor equality are asserted. Actual stitching is outside the fake RPC fixture.                            |
| Editor attachment lifecycle     | Eight production scenarios cover all four edges, frozen/live/fallback pixels, partial capture failure, invalid image, cancellation and retry. Exact composed pixels and history reopen match; temporary files, new history entries and clipboard changes are rejected.                                                             |
| Pin composition and restoration | Rendered pin pixels match the annotated PNG within two channel levels. Closing the pin restores annotations and identical export bytes with history enabled or disabled, without adding history entries. Enlarged/narrow bounds are checked against natural logical size.                                                          |
| Capture preview lifecycle       | Four corner configurations preserve 200×140 cards with 12px gaps. Adding a fifth capture evicts the oldest card without deleting its file; color pixels verify order. Double-click opens the selected capture with identical PNG export. Auto-dismiss closes the stack while preserving files and history.                         |
| Native analysis services        | Windows capture of an owned OCR editor window contains the expected recognized text. The rebuilt QR service now reads the full 952×651 captured window, including its toolbar/margins. Before/after checks improve 38/65 to 43/65 exact payloads with zero regressions. These probes do not certify native app clipboard delivery. |
| Native live window pick         | Background input selected an owned editor window, closed the selector and opened its 952×651 capture; decoded pixels exactly matched a direct daemon capture of that target.                                                                                                                                                       |
| Native frozen window pick       | With freeze enabled, the former path captured a rectangle added after opening selection. The fixed saved output preserves the pre-change region: zero changed pixels versus before, while the live target differs by 3,761 pixels in that region.                                                                                  |
| Native frozen area capture      | The selector client origin formerly sat four pixels above the display after activation. It now aligns at (0,0); the saved 400×240 region exactly matches the intended frozen crop, excluding a later annotation that changes 2,393 live pixels.                                                                                    |
| Native timer capture            | A source annotation added during the real five-second countdown appears in the saved 400×240 capture: zero changed pixels versus the live target and 1,335 versus the pre-countdown region.                                                                                                                                        |
| Native timer cancellation       | The named native countdown panel is discoverable through accessibility inventory. A background click closes it and the selector; no screenshot or history file appears after the countdown would have ended.                                                                                                                       |

Local native artifacts are under `.scratch-parity/native-workflow-fixed/`,
`.scratch-parity/native-workflow-motion-tty/` and `.scratch-parity/live-data/`.
They are development evidence, not release binaries or portable CI fixtures.
Window-picker before/after evidence is in `.scratch-parity/picker-native-*/`;
the reviewed comparison sheet is `.scratch-parity/window-freeze-native-proof.jpg`.
Area/timer artifacts are in `.scratch-parity/selection-native-*/`; the reviewed
comparison sheet is `.scratch-parity/selection-native-proof.jpg`.
Native OCR/QR input, captured windows and decoder diagnostics are in
`.scratch-parity/analysis-native-*/`. The QR photo is ZXing's
[qrcode-1/1.png](https://github.com/zxing/zxing/blob/56b8a9a7b53392097891463023a26c406b422225/core/src/test/resources/blackbox/qrcode-1/1.png),
downloaded at the pinned revision with its Apache-2.0 license and expected result.
The grayscale full-window regression is retained in the native test fixtures;
its source and license are recorded in `THIRD_PARTY_NOTICES.md`. The remaining
probe assets stay outside the app and visual baselines. Native before/after
results for 20 QR photos, their grayscale originals and framed/resized variants
are in `.scratch-parity/qr-corpus-*/native-{before,after}.json`.

Bugs fixed in this pass:

- Area move/resize completion ignored the release coordinate in both shells.
- A window click used the last hovered candidate rather than hit-testing the
  click position. Empty-space clicks now leave selection open; a direct target
  click works without a preceding move.
- Dedicated window screenshots ignored "Freeze screen" and all window captures
  read live pixels. Screenshot pickers now freeze on entry and crop the retained
  frame with the picked window's alpha mask. Failed or unavailable freezes use live window
  capture. The selector hides immediately; capture completion releases its
  generation, including on capture errors.
- Capture overlays now have a native/accessibility window name, including pooled
  windows, so their lifecycle can be inspected.
- Area/display capture queued frozen-frame release before the reserved crop,
  which could block the crop on a single worker. Release now runs after capture,
  including errors. Failed or unavailable freeze responses explicitly use live pixels.
- A failed timer-control `show` left the handed-off selector active. It now closes
  the selector and clears the timer session; cancel/timeout/retry are also exercised.
- OCR/QR capture errors left partially written temporary PNGs behind and exposed
  an internal capture-method error. They now delete the partial file and use the
  same capture-failure notification text as Electron.
- QR thresholding depended on the selected image width and missed a photographed
  code surrounded by toolbar/margins. A local threshold pass now runs when the
  original decoder finds no payload. The full-window regression changed from
  empty to the exact expected payload; existing successful corpus cases remain
  unchanged. Windows and Linux loaders share the fix; Linux's regression passed
  in hosted CI. Interactive Linux app behavior remains unverified.
- Scroll capture subscribed after the start RPC, losing early completion or
  cancellation events and leaving the session active. Subscription now precedes
  start; both early events and a successful retry are exercised.
- Failed editor-attach captures also left a partially written temporary PNG.
  The attachment path now removes it before restoring the editor.
- The editor retained its original zoom after an attachment expanded the
  composition, clipping the result. The fit cache now includes composition
  dimensions, matching Electron's recalculation dependencies. Attachment checks
  require the entire composed rectangle to remain inside the editor viewport.
- Enlarging a pin upscaled its image beyond the original display size. Pin
  images now cap at their natural logical size and remain centered when resized.
  The shared drag area's inset also rescaled edge pixels; pin content now has
  zero inset and matches the annotated export.
- Closing a pin restored annotations only through persisted history, losing
  edits for untracked images or when history was disabled. The pin now retains
  the current annotations, wallpaper and layers in memory for restoration.
- The native timer panel had an empty accessibility window name. It is now named
  "Poratake timer capture", making the countdown identifiable by users and tools.
- GPUI's first activation restored a hidden Windows selector's initial placement
  after exact client bounds were applied, shifting its client origin four pixels
  above the display. The activation observer reapplies the display bounds, keeping
  area and timer output aligned with the selection.
- GPUI annotation completion also discarded the final release coordinate.
- Scheduling pointer updates requested an animation frame outside GPUI painting,
  causing a native debug panic during drawing.
- Shape previews differed from saved PNGs; cached patches could mix captures
  having otherwise identical annotation geometry.
- Narrow editor options could push Close outside the window.
- Countdown disabled Start while leaving focus on it, preventing Escape from
  reaching the control bar. Countdown now moves focus to the bar.
- The recording popup had no native or accessibility window title. It is now
  named "Poratake recording controls" and can be identified by desktop tools.
- Shared overlay buttons and recording device triggers used white foregrounds
  on light toolbars in both shells. Foreground and hover colors now follow the
  active theme. Electron light/dark computed styles were checked; browser
  screenshot and hover-state evidence remain unavailable in this environment.
- Windows change-driven capture wrote every frame with a fixed short duration,
  truncating static takes. Pending samples now receive their real duration before
  submission; finalization and pause accounting use the capture clock.

## Remaining bugs and verification backlog

Entries distinguish observed defects from missing evidence. An unverified path
is not a demonstrated product defect; reproduce before changing its behavior.

| Priority | Remaining work                                           | Acceptance evidence                                                                                                                                                                                                                                                                                                                                                                             |
| -------- | -------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| P1       | Native window/display highlighting and recording handoff | Simulated candidate highlights and a native live/frozen window screenshot pick are verified. Native overlay screenshots timed out. Verify physical pointer transitions on each display and no stale dim or duplicate outline when a recorded window moves/resizes/closes.                                                                                                                       |
| P1       | Physical Windows DPI and monitor transitions             | Run both shells at OS 100/125/150/200%, mixed-DPI and negative-origin displays; verify cursor, crop pixels, bounds, caption hit targets and saved output.                                                                                                                                                                                                                                       |
| P1       | Full capture-service matrix                              | Fixtures cover window/area/display, timer, OCR/QR and scroll lifecycles. Native area/timer output and OCR/QR service probes are verified. Extend complete native display/OCR/QR/scroll workflows, real stitching, clipboard, save, pin and attach; compare output with Electron.                                                                                                                |
| P2       | QR photo robustness                                      | The reproduced full-window failure is fixed. The native decoder matches 26/40 grayscale original/framed QR corpus cases, up from 23/40, preserving every previous success. Fourteen stress cases remain unmatched; diagnose their detector/decoder failures before changing behavior. Linux loader regression passes in hosted CI; interactive app clipboard delivery still needs verification. |
| P1       | Recording media and target continuity                    | Native mic/system/camera enable/disable and device changes, target close/move/resize, audio continuity and camera visible ranges. Control-bar fixtures and image baselines cover UI/RPC transitions; hardware output still needs proof.                                                                                                                                                         |
| P1       | Extended video interactions and export matrix            | Loaded panels are baselined. Add timeline/control interactions and verify zoom, camera, cursor, keyboard, subtitles, audio, cuts and trims across preview and exported frames. Current export workflow covers a documented subset.                                                                                                                                                              |
| P2       | Pre-recording preference parity                          | GPUI toggle actions change the pending take but leave stored enable flags unchanged; Electron regular pre-recording controls persist them and all-in-one uses session choices. Verify both entry paths and align their persistence rules.                                                                                                                                                       |
| P1       | Accessibility action delivery                            | Native Select expands by pointer and list selection works, but the exposed Expand action did not open it. Slider SetValue did not change padding/corners. Reproduce in a minimal HeroGPUI/GPUI case and fix the owning action bridge.                                                                                                                                                           |
| P2       | Complete state coverage beyond representative controls   | Add default/hover/pressed/focused/open cases for each shared control and overlay, with focused content remaining visible in narrow windows.                                                                                                                                                                                                                                                     |
| P2       | Review hosted Windows visual results                     | Geometry and workflows passed in the first hosted run; exact shared images differed. Record GPU metadata, review the hosted renderer's own exact baselines, and require a second run to compare them without updates.                                                                                                                                                                           |
| P2       | macOS and Linux runtime comparison                       | Re-run the same applicable workflows on actual target platforms; Windows evidence does not certify them.                                                                                                                                                                                                                                                                                        |
| P2       | Other app surfaces                                       | Settings, history, onboarding and tray need refreshed runtime comparison and their own image baselines. Pin and capture-preview representative layout/lifecycle states now have automated coverage; full native interaction, preview docking and hover controls remain outstanding.                                                                                                             |

Keep known platform differences explicit: preview/zoom use a baked blur because
GPUI has no live backdrop filter; the capture eyedropper remains an app overlay;
macOS traffic-light space and Windows custom captions are platform-specific.
These are not evidence that unrelated features match.

## Review gate

| Review category                       | Result                                                                                                                                                                                                                                                                                                                       |
| ------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Scope reviewed                        | Changed capture/editor/recording behavior, shared themes, visual/workflow fixtures and dependency evidence against `ac384ae`; independent behavior/interface and proof/simplification passes, with targeted review of attachment, pin, permission, icon measurement, capture previews, unit isolation and renderer profiles. |
| Fixed                                 | The defects listed above, including click-position window picking, frozen selection lifecycle, failed timer cleanup, native selector client alignment and full-window QR detection; workflow checks verify capture-before-release and exact output.                                                                          |
| Rejected after validation             | A proposed ZXing license truncation was rejected: raw bytes from the exact pinned upstream revision match the bundled file, including its upstream appendix (SHA-256 `3f62881f0566227a24b12e5a754cc79f39aaa94883038e95c94812e1f50af42f`).                                                                                    |
| Deferred major or dangerous refactors | None.                                                                                                                                                                                                                                                                                                                        |
| Verification                          | Local checks, exact visual comparison, native workflows and image review are recorded above.                                                                                                                                                                                                                                 |
| Blocked or unverified                 | Hosted renderer baseline review and repeat CI, physical mixed-DPI, remaining native media/capture workflows and other platforms remain in the backlog. The approved permission isolation and icon-only contrast refinements are implemented.                                                                                 |
