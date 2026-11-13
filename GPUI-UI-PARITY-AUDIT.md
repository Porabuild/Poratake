# GPUI versus Electron verification audit

Refreshed 2026-11-13 for Poratake 0.9.6.
Electron remains the product reference. This is a verification record and current
backlog; the earlier implementation plans are historical findings, not an open-bug
list. A source comparison or passing unit test does not establish full manual parity.

## Current critical scope

This pass covers major clipping, sizing, missing controls, stale selections,
capture corruption and preview/export mismatches in the core Windows and macOS
capture and recording workflows. Electron remains the reference where the same
workflow exists. The detailed backlog below remains useful, but is not a list of
required work for this pass. Minor antialiasing differences and Linux-only
enhancements are deferred unless they expose a critical shared defect or have a
small verified fix. Exhaustive states for every control remain deferred.

Each gate counts once only when its stated evidence exists. As of 2026-11-13,
seven of twelve gates are complete: 58% of the gates, not an estimate of elapsed or
remaining effort. Partial work does not count as a completed gate.

| Gate                                 | Status | Completion evidence                                                                                                                                                         |
| ------------------------------------ | ------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| HeroGPUI dependency                  | Done   | #43 released as 0.15.1; Poratake pins that release and its matching GPUI version.                                                                                           |
| Reviewed visual coverage             | Done   | Hosted CI at `258af434` passed 16 Windows profiles and 2,912 reviewed baselines on Microsoft Basic Render Driver.                                                           |
| Automated screenshot workflow        | Done   | Capture, annotate, save and reopen run in the visual workflow fixtures.                                                                                                     |
| Automated recording workflow         | Done   | Recording, editing and export fixtures compare tested preview/output cases.                                                                                                 |
| Native Windows tooltip               | Done   | Hosted checks verified recording and capture-preview tooltips, including screenshot exclusion; recording frames were decoded.                                               |
| Native Windows preview handoff       | Done   | A real screen capture showed a bottom-right preview and opened its saved image in the editor.                                                                               |
| Final-head CI                        | Open   | All required hosted jobs and the Windows visual matrix must pass on the final pushed commit.                                                                                |
| Windows capture and save             | Done   | Hosted Windows captured an area, annotated, saved and reopened a PNG with pixel and editor-size checks. A later run created a smaller selection inside the previous region. |
| Windows recording and export         | Open   | A real recording was trimmed through the UI and exported; compare its native preview with output and check system audio if available without prompting.                     |
| Windows major visual and DPI checks  | Open   | Check tray, size submenu, preview and toolbar hover/focus/open states at 100%, 125% and 150% OS scaling, including a narrow window and connected monitors.                  |
| macOS critical capture and recording | Open   | On an already-authorized Mac, verify area/scroll captures and full-display recordings omit controls and overlays, including after restart; fix confirmed failures.          |
| Final review and audit               | Open   | Review the final changes, resolve critical regressions and update the draft PR and this checklist with exact evidence.                                                      |

[Hosted Windows CI at `badb3d5b`](https://github.com/Porabuild/Poratake/actions/runs/43402764907)
passed native recording with programmatic cuts and annotation, and compared three
preview/export frames. It did not exercise editing in the UI or capture audio.

[Hosted Windows CI at `1d6a4d0a`](https://github.com/Porabuild/Poratake/actions/runs/43419762563)
captured a real area, reopened the selector, and drew a rectangle in the screenshot
editor. Saving then crashed while opening the native dialog, so that run did not
verify the saved image or reopen.
[The next Windows run](https://github.com/Porabuild/Poratake/actions/runs/43425397059)
passed real capture and reopened the selector but did not reach the editor: its Edit
click did not wait for the hover controls to render. The harness now checks their
readiness and the preview HWND hit test before clicking.
[The Windows native workflow at `19e8db81`](https://github.com/Porabuild/Poratake/actions/runs/43429066842)
passed capture, rectangle annotation, the owned Save dialog and reopen, including
sampled output pixels and matching editor dimensions. The selector reopened and
cancelled without producing another capture, but capture protection prevented
checking its selection pixels. All active CI jobs passed; Windows packaging was
skipped.
[The Windows workflow at `cd9c5d0e`](https://github.com/Porabuild/Poratake/actions/runs/43435257885)
created a smaller capture inside the previous selection, confirming the selector
was fresh. It later failed a screenshot color check because the second preview
overlapped the editor and the resulting annotation crossed a sample point. The
revised harness closes that preview and samples away from the annotation.
[The Windows workflow at `16f9df47`](https://github.com/Porabuild/Poratake/actions/runs/43438907936)
passed the corrected capture, selector reset, annotation, save and reopen checks.
It then exposed a video editor sizing defect: the 1200×750 minimum clipped the
Export control on the 1024×768 runner before the export click.
[The Windows workflow at `215d9070`](https://github.com/Porabuild/Poratake/actions/runs/43445368824)
verified the resized editor and Export control within the work area, opened the
owned Save dialog and produced a stable file. The output pixel and duration check
failed before decoding: a later capture-preview tooltip overwrote the recording
tooltip's paint-proof metadata.
[The native Windows workflow at `258af434`](https://github.com/Porabuild/Poratake/actions/runs/43449868862)
passed with isolated proof files. It exercised the owned video Save dialog and
verified the decoded export's duration, dimensions, marker pixels and tooltip
exclusion. All active jobs passed, including 16 visual profiles and 2,912 baseline
comparisons on Microsoft Basic Render Driver; Windows packaging was skipped.
A GPUI test added at `e8d1b9fe` clicks Rectangle, draws on a recording fixture,
reopens the saved project and compares its preview with a decoded export, including
the changed annotation pixels. It passed in the Windows Rust suite at `e3907912`;
it is not a native UI recording workflow. The same full-width tooltip pattern also
prevented screenshot-editor Reset Zoom clicks. A pointer test verifies both edges
of its 56px control and passed in that Windows suite.
[Windows CI at `baa80bc0`](https://github.com/Porabuild/Poratake/actions/runs/43458601974)
passed the native Rust suite but stopped before the picker and visual matrix: the
runner terminal covered the reopened screenshot editor during its pixel capture.
The harness now waits for the editor to be foregrounded, hit-tested and painted.
[Windows CI at `bae84b8f`](https://github.com/Porabuild/Poratake/actions/runs/43463210217)
passed screenshot reopen and verified that the recording selection hole is cleared
when the same native overlay window is parked and reused. A UI trim was saved, but
the after-image check missed the selected clip's changed color, so export and picker
checks did not run. GitHub cancelled the macOS job before it started because no
hosted runner was available.
[Windows CI at `e76c8e8e`](https://github.com/Porabuild/Poratake/actions/runs/43467897146)
passed screenshot reopen and the native recording overlay reuse check, trimmed a
recorded clip through the video editor and verified the decoded export's duration,
marker pixels and tooltip exclusion. The cursor picker opened, cancelled and
reopened; its acceptance check stopped because UI Automation exposed nine edit
controls without the expected name. Native file acceptance still needs verification.
The macOS build and test job passed in that run; real macOS screen-capture exclusion
remains unverified.
[Windows CI at `847ab547`](https://github.com/Porabuild/Poratake/actions/runs/43473164699)
passed screenshot reopen and overlay reuse, but did not reach the picker: the
Export tab was not active when its button was queried. The harness now waits for
and clicks the actual sidebar tab before checking Export Video. The macOS CI
build and test job passed on that run.
[Windows CI at `e3907912`](https://github.com/Porabuild/Poratake/actions/runs/43477996629)
passed the tab, trim and decoded export checks. Picker acceptance again stopped;
the captured UI Automation inventory showed file-list properties and a search box,
but no filename Edit. The dialog did expose the isolated recording directory's
file items, so acceptance must use a verified item rather than an arbitrary edit.
[Windows CI at `02d33a22`](https://github.com/Porabuild/Poratake/actions/runs/43483480175)
found the unique fixture and state-file items, but the helper waited for an enabled
Open button before selecting the fixture and stopped. The captured evidence did not
establish whether the button was absent or disabled. GitHub cancelled the macOS job
before starting because a hosted runner was unavailable.
Video export, tray file selection, screenshot wallpaper and video media imports
use asynchronous dialogs to avoid the same nested event-loop pattern. The other
native selection and cancellation flows still need verification.

Estimate with an isolated Windows desktop and an already-authorized macOS test
environment: roughly 2–5 working days, assuming no new major defects. Windows
workflow and DPI checks are expected to take 4–8 focused hours; macOS capture and
exclusion work may take 1–3 days; review and follow-up checks may take 1–2 hours.
The hosted Windows gate previously took about 50 minutes and can run in parallel.
These are planning ranges, not guarantees. Without macOS access or the necessary
existing Screen Recording authorization, the macOS gate and overall finish date
remain unknown. No test in this pass should request that authorization.

## Versions and dependency evidence

| Component          | Current source                                                                                                                                                                                          |
| ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| HeroGPUI           | Exactly 0.15.1 in `src/main/Cargo.toml`; components use the source patch below and the other three crates retain registry checksums; source tag `v0.15.1` at `09ec52be169bf25a6351a14088e34e22fe89fe9f` |
| GPUI               | `gpui-pre` and `gpui-pre-platform` exactly 0.3.11, matching HeroGPUI's workspace                                                                                                                        |
| Electron runtime   | 43.4.0 in `bun.lock`; manifest range `^43.4.0`                                                                                                                                                          |
| Electron reference | `@heroui/react` and `@heroui/styles` 3.2.4 in the installed lockfile; manifest range `^3.2.4`                                                                                                           |
| HeroGPUI reference | HeroUI 3.2.7; its component audit is separate from Poratake's app parity                                                                                                                                |
| Bundled guide      | `.agents/skills/herogpui/upstream.json` records revision, versions, source hashes and adapted local hashes                                                                                              |

[HeroGPUI #43](https://github.com/Porabuild/HeroGPUI/pull/43) is merged. The
`v0.15.1` tag points to the release source commit. Release workflow
[42747398534](https://github.com/Porabuild/HeroGPUI/actions/runs/42747398534)
completed successfully, including SemVer, platform tests, gallery artifacts and
registry publication. The public release and 0.15.1 registry packages were checked.
Poratake's published-registry update in `ac384ae` is retained as the base of this change.

The former 0.3.3 GPUI reference, old HeroGPUI revision, hand-rolled control findings
and broad surface-level “Match” claims have been removed from this audit.

The existing GPUI 0.3.11 Windows backend is patched from
`vendor/gpui-pre-windows`, with provenance and the retained Apache-2.0 license in
`THIRD_PARTY_NOTICES.md`. Its image shader, clipboard and pointer-exit handling
differ from the published archive; all dependency versions remain unchanged.

HeroGPUI components 0.15.1 resolve to `vendor/herogpui-components`. The patched
`src/select.rs` preserves focus-ring gutters and handles accessibility expansion;
`src/dropdown.rs` preserves scrollable menu focus gutters within the available
panel padding; `src/slider.rs` handles accessibility value actions; and
`src/util.rs` releases a keyboard press after focus moves into a menu. The archive
checksum, original licenses and NOTICE are retained.
Zero-padding dropdown recipes still have no scroll-end focus gutter; Poratake's
current recipes use 4px or 6px of panel padding.

## Automated visual gate

Run `bun run test:visual` on Windows. The optional `parity` feature uses an isolated
profile, a deterministic 640×360 capture and the real Windows renderer, embedded
Geist fonts and DirectWrite. Hidden native render targets provide GPU readback;
input and frame scheduling use GPUI's headless harness. This is not installed-app
or physical-monitor DPI verification.

At `bca57fb0`, a local NVIDIA dark 1280px/100% profile passed all 193
reviewed visual baselines and the capture, annotation, save/reopen and
preview/output fixture checks. [Hosted CI](https://github.com/Porabuild/Poratake/actions/runs/43338500286)
passed Linux, macOS, typecheck, lint and formatting. Its Windows probe found
the real recording tooltip with the expected text outside its owner and capture
affinity set, but the tooltip did not disappear after pointer exit; the Windows
visual matrix did not run on that commit. The [follow-up Windows probe](https://github.com/Porabuild/Poratake/actions/runs/43343136797)
passed after the pointer-exit fix, including same-point re-entry and an
overlapping window; its visual matrix also passed.

Each profile has a ten-minute watchdog; the first timeout stops the matrix and
preserves that profile's partial evidence. On Windows, process-tree termination
uses `taskkill /T` only while the tracked profile process is still active. If its
leader has exited, the runner avoids targeting a potentially reused PID and
reports that cleanup failed; this is not a Job Object guarantee, so a descendant
that outlives its leader before timeout may remain. If `taskkill` fails, the
fallback stops only the root and reports the failure. POSIX cleanup targets the
detached process group after leader exit. The persistent timeout test verifies
profile-worker cleanup and that an unrelated sibling survives; its POSIX
leader-exit case is skipped on Windows.

[Hosted CI at `515fa5cd`](https://github.com/Porabuild/Poratake/actions/runs/43286352987)
passed the complete 16-case Windows visual gate, native Windows checks and
accessibility actions. Its macOS daemon build, scroll-preview and GPUI shell
tests passed, as did Linux, typecheck, lint and formatting; the remaining Windows
tests passed too. The preceding
[hosted evidence](https://github.com/Porabuild/Poratake/actions/runs/43273775369)
and local NVIDIA runs showed 192 changes confined to the video preview. Those
reviewed images for both renderers are now committed and passed the hosted gate.
The desktop-icon restore no longer blocks the held-response fixture, and the
screenshot save, attachment, pin and editor-reopen workflows reach the baseline gate.

[Hosted CI at `98d84f77`](https://github.com/Porabuild/Poratake/actions/runs/43296056019)
passed the workflow and geometry assertions in all 16 Windows visual profiles,
then failed on the expected changed state list. Its Microsoft Basic renderer
images were compared across every profile and the changed tooltip, pressed and
video-preview states were reviewed. Final local NVIDIA captures also reached the
state-list gate for all 16 profiles. Their changed states were reviewed, and all
2,912 resulting baseline images matched the captures pixel-for-pixel. A strict
rerun against those NVIDIA references passed all 16 profiles. Windows native
and accessibility checks passed; Linux Clippy found platform-specific
tooltip warnings that a follow-up gates. In the next run Linux passed and the
macOS daemon built, but its native fixture hung during the existing scroll checks.
The follow-up keeps those checks on the synchronous main thread and bounds their
response wait. The fixture compiled and passed in
[hosted CI at `5359f58f`](https://github.com/Porabuild/Poratake/actions/runs/43307382869);
the full run passed its active jobs, including all 16 Windows visual profiles.
The [macOS fixture at `83a84d45`](https://github.com/Porabuild/Poratake/actions/runs/43315713985)
also passed the expanded scroll alignment, fixed-footer, accepted-prefix and
cancellation checks. Its Windows visual job was cancelled when the next commit
was pushed.

The GPUI recording error listener now starts before the daemon request and stays
active through a failed stop. A fake-daemon run observed a failed start, retry,
two stop attempts and the expected camera and overlay cleanup, then stopped on a
test-harness redraw race. That race is fixed in the harness; the next local run
stopped at the GPU safety limit before reaching this workflow. A CPU-only daemon
protocol smoke test passed the injected failures and retries. At
[`fe64d934`](https://github.com/Porabuild/Poratake/actions/runs/43321344519),
14 of 16 Windows visual profiles passed. The two remaining profiles reached both
stop attempts and cleanup, then failed a test assertion that searched for the
window outline only after it had already been shown during selection. The test
now checks the complete handoff; [hosted CI at `0da6a9c6`](https://github.com/Porabuild/Poratake/actions/runs/43387203877)
passed all 16 Windows visual profiles.

For translucent video wallpaper, the preview now uses the encoder's opaque
pixel conversion. A Windows MP4 test compares the decoded export with the actual
preview image at four edited times, including drawing and redaction. A GIF test
also reopens every exported frame at several frame rates and checks opaque alpha
and preview colors for transparent and translucent padding.

[Hosted CI at `f9df4d81`](https://github.com/Porabuild/Poratake/actions/runs/43252165030)
passed 4 of 16 visual cases and 706 exact baseline comparisons. Ten cases stopped
in the prewarmed tray geometry check. Both 1280px/100% cases stopped because the
live all-in-one window fixture expected frozen-frame pixels; the saved light and
dark captures contain the fixture's live color in every pixel. That expectation
and its subsequent call-order checks passed in the `904073a1` run.
The preserved evidence contains 2,926 PNGs, 16 renderer profiles and no diff
images. All other required jobs passed. A focused prewarmed-tray test reproduced
the scale reset and passed at 100%, 125%, 150% and 200% after applying the
simulated scale after each show; the revised tray checks passed in the `904073a1` run.

The Windows harness now drains the native foreground queue when each hidden
renderer closes, allowing GPUI's asynchronous window destruction to complete.
Each visual case checks that a closed test window releases its native window.
In a focused local probe, the old harness accumulated 34 windows and reached
8.9 GiB of GPU use; the updated harness stayed at three to four windows and
about 1.2 GiB while exercising the same attachment cases.

The matrix is light/dark × 950/1280 logical pixels × 100/125/150/200% scale, at
738 logical pixels high. Each case captures 163 states; the two 1280px, 100% cases
also capture 28 annotation/selection/timer/scroll/attachment/pin workflow states.
The eight 950px cases each add 27 narrow-history states, for 2,880 images per
complete renderer profile. The 72 new 140px NVIDIA history images were reviewed;
the previous 1,448 images in the 950px cases were pixel-identical. The complete
local NVIDIA gate passed all 16 cases and 2,880 exact images. The 72 hosted
variants were also reviewed; all 2,808 earlier hosted images were pixel-identical.
[Hosted CI at `0b79bd9`](https://github.com/Porabuild/Poratake/actions/runs/42992001485)
passed all required jobs and the complete 2,880-image exact matrix. The 48 selected-OCR
tooltip images and 32 target-menu images were reviewed for NVIDIA; all 2,728
earlier images were pixel-identical.
[Hosted CI at `e76df4cc`](https://github.com/Porabuild/Poratake/actions/runs/43079082274)
passed all required jobs, including 16 visual cases, 2,880 exact baselines and
the native Windows clipboard round trip. This run precedes the zoom changes
described below.
The zoom follow-up changed 148 NVIDIA images; their differences were reviewed
within the screenshot canvas or its one-pixel lower edge. The other 2,732 images
were unchanged, and all eight affected cases passed exact comparison after the
reviewed updates. The [hosted run at `e6454be4`](https://github.com/Porabuild/Poratake/actions/runs/43088770389)
reported 148 changed images across eight cases, but its evidence upload failed
after the artifact API returned HTML instead of JSON. The [run at `8b764bc0`](https://github.com/Porabuild/Poratake/actions/runs/43098740249)
preserved all 16 renderer profiles and the 148 changed Basic Render Driver
images; the other 2,732 images matched exactly. The changed pixels were checked
against the fixture image bounds and representative controls were inspected in
both themes. [Hosted CI at `2d50016b`](https://github.com/Porabuild/Poratake/actions/runs/43107376955)
passed all required jobs and all 16 visual cases with 2,880 exact baselines. Its
evidence includes 16 renderer profiles and 3,373 PNGs, with no diff images.
The [run at `64317044`](https://github.com/Porabuild/Poratake/actions/runs/43123318593)
also passed all required jobs and the 2,880-image visual matrix, before the
wallpaper-redaction change.
The [run at `fb13d389`](https://github.com/Porabuild/Poratake/actions/runs/43128687500)
passed 14 of 16 visual cases. A transparent wallpaper exposed the editor stage's
shadow in its preview, and a fractional-position test compared GPU-rendered pixels
directly with raw PNG pixels. The stage now has the editor's background behind
its wallpaper, and the fractional test compares a reopened PNG at the same bounds.
The follow-up also adds alpha-gradient and transparent-image cases.
The [run at `642d16a7`](https://github.com/Porabuild/Poratake/actions/runs/43132888460)
passed the Windows native, accessibility and clipboard checks and 14 of 16 visual
cases. Both 1280px/100% themes passed the alpha wallpaper checks, then failed the
same zoomed blackout-edge comparison.
Its preserved artifact contains 16 renderer profiles and 3,193 PNGs with no diff
images. After aligning blackout patches, local NVIDIA runs passed the light/dark
1280px/100% workflows and 191 baselines per theme, plus 190 baselines per theme
at 950px/125%. The [run at `9a0eb806`](https://github.com/Porabuild/Poratake/actions/runs/43136845938)
passed the zoom checks but failed two history-reopen baselines: each changed two
green-channel values by one at the same pen endpoint. The same pixels appeared
in the earlier hosted artifact; both baselines were reviewed and updated.
Fourteen of 16 visual cases passed, and the artifact retains 16 renderer profiles,
3,381 PNGs and both diff images. The Windows test step did not run after the
visual failure. Linux, macOS, lint, formatting and typecheck passed.
The [run at `8f6b7169`](https://github.com/Porabuild/Poratake/actions/runs/43141680459)
passed all required jobs, including 16 visual cases and 2,880 exact baselines,
Windows native and accessibility tests, clipboard roundtrip and the project test
suite. Its artifact contains 16 renderer profiles and 3,381 PNGs with no diffs;
the artifact and Windows log are retained under `.scratch-parity/hosted-8f6b7169`.
[Hosted CI at `2d26dab5`](https://github.com/Porabuild/Poratake/actions/runs/43148945257)
also passed all required jobs, including all 16 visual cases and 2,880 exact
baselines. Its artifact contains 3,393 PNGs with no diff files; the evidence is
retained under `.scratch-parity/hosted-2d26dab5`, alongside the Windows log.
[Hosted CI at `1e5d547c`](https://github.com/Porabuild/Poratake/actions/runs/43157671728)
passed all required jobs, including 16 visual cases and 2,880 exact baselines.
Its preserved artifact contains 3,395 PNGs and no diff files; the artifact and
Windows log are retained under `.scratch-parity/hosted-1e5d547c`.
[Hosted CI at `b2d00217`](https://github.com/Porabuild/Poratake/actions/runs/43165588606)
passed all required jobs, including 16 visual cases and 2,880 exact baselines.
Its preserved artifact contains 3,397 PNGs and 16 renderer profiles with no
diff files; the artifact and log are retained under `.scratch-parity/hosted-b2d00217`.
[Hosted CI at `6eb45d57`](https://github.com/Porabuild/Poratake/actions/runs/43182302140)
passed all required jobs, including 16 visual cases and 2,880 exact baselines.
Its preserved artifact contains 3,409 PNGs and 16 renderer profiles with no diff
files. It predates the selected-outline follow-ups below.
[Hosted CI at `27798e87`](https://github.com/Porabuild/Poratake/actions/runs/43189153989)
passed all required jobs and 2,880 exact comparisons across 16 visual cases. Its
preserved artifact contains 3,423 PNGs and 16 renderer profiles with no diff
files. It predates the subsequent redaction and circle-selection checks.
The desktop-icon transition follow-up passed 191 exact baselines in each local
light/dark 1280px/100% NVIDIA run. Its fake-daemon checks cover failed and delayed
icon transitions, stale direct capture errors and attachment recovery; the dark
run also holds a scroll finish for 31 seconds. The full GPUI suite passed 939
tests with one ignored, and the project suite passed 2,281 tests with three skipped.
The frozen-area icon follow-up also passed 191 exact baselines in each local
light/dark 1280px/100% NVIDIA run. Its full GPUI suite passed 938 tests with one
ignored, and the project suite passed 2,281 tests with three skipped.
The multi-display icon follow-up passed 191 exact baselines in each local
light/dark 1280px/100% NVIDIA run, including a simulated two-target workflow.
Its full GPUI suite passed 938 tests with one ignored; the project suite passed
2,281 tests with three skipped.
The scroll-start follow-up passed 191 exact baselines in each local light/dark
1280px/100% NVIDIA run with no diff images. Its GPUI suite passed 939 tests with
one ignored; the project suite passed 2,281 tests with three skipped using four
workers after the default run timed out while inventorying installed licenses.
The earlier frame and aspect-ratio follow-up passed 191 exact baselines in
each local light/dark 1280px/100% NVIDIA run, including the new workflow checks.
The earlier scroll follow-up passed 191 exact baselines in each of the isolated
1280px/100% light and dark NVIDIA cases, with no diff images. The runs peaked at
4,070 MiB and 4,065 MiB of total GPU memory respectively.
An exact rerun after the target-menu interaction fixes passed all 16 cases and
2,808 images. The corresponding 80 hosted images were reviewed, and all 2,728
earlier hosted images were pixel-identical. [Hosted CI at `a9d025f`](https://github.com/Porabuild/Poratake/actions/runs/42980916944)
passed all required jobs, including the 2,808-image Windows visual gate. The 96 earlier NVIDIA all-in-one tooltip images were
reviewed; all 2,632 existing images were unchanged. The corresponding 96 hosted
images were also reviewed, and all 2,632 previous hosted images were pixel-identical. The local
exact-baseline matrix at `80a5700` passed all 16 cases and 2,632 images. After
the color-capture failure fix, the two
1280px/100% workflow cases passed again in light and dark themes. The 96 new
NVIDIA color-picker images were reviewed; all 2,536 existing images were
unchanged. The 96 hosted color-picker images were also reviewed, and the 2,536
previous hosted images were unchanged. The 144 new NVIDIA history images were reviewed. The earlier matrix's 64 corner
states and 160 revised selection images were reviewed; the remaining 2,312 existing
images were unchanged. The DPI follow-up changed 24 NVIDIA history images; all
24 were reviewed and the other 2,512 images were unchanged. The final exact
matrix passed all 16 cases. The latest local Windows native gate passed 1,084 tests with two skipped.
All 144 hosted history images, 64 selection-corner images, 160 revised selection
images and 24 DPI-adjusted history images were also reviewed. The other 2,512
hosted images were unchanged.
At 950 logical pixels and 125%, the synthetic picker backdrop spans 1187.5
physical pixels, leaving a pale final screenshot column. The real color-picker
overlay is transparent; this fixture artifact does not verify physical displays.
Loaded-video windows respect their 1200×750 minimum: the 950px cases use 1200px
for that surface. Recording controls use their measured content width.

| Surface             | Exercised evidence                                                                                                                          | Limits                                                                                            |
| ------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- |
| Screenshot editor   | Default, representative hover/held press/keyboard focus, selected tools, thickness/color/tool panels and keyboard-operated nested menus     | Not every control's complete interaction matrix                                                   |
| Area overlays       | Screenshot, OCR, QR, timer, scroll, recording and editor-attach default/drag-selection surfaces; standalone window/display picker states    | No OCR/QR service result or stitched-scroll output from these fixtures                            |
| Color picker        | Center, four display corners and 400×300 narrow window; exact sampled loupe pixels and pointer clearance                                    | Physical pointer transitions and native clipboard delivery remain unverified                      |
| All-in-one          | Screenshot/record area, window and display pickers; OCR area, window highlights, pointer leave and normal/narrow tooltips                   | Window ownership is simulated; physical pointer transitions remain unverified                     |
| Recording controls  | Area/window defaults, hover/held press/keyboard focus, long device menus, countdown/cancel, active/paused/muted and locked camera choices   | Fixture RPCs assume granted media permission; hardware and real consent remain native checks      |
| Video editor        | Empty editor states plus a loaded encoded project, drawing, wallpaper and all ten side panels                                               | Playback and every panel's control states are not yet image baselines                             |
| Annotation workflow | Annotated preview, history reopen and flattened PNG reopen                                                                                  | Built-in fixture at 1280px/100%; custom comparison images skip this workflow                      |
| Pin window          | Default, hover, enlarged and narrow windows; image bounds and close control; annotated editor restoration with history enabled and disabled | Native movement and Alt+F4 restoration verified on Windows at 100%; mixed DPI remains unverified  |
| Capture preview     | Single cards and four-card stacks; four corners, eviction, explicit editor handoff, auto-dismiss, hover ownership and a four-display menu   | Native capture handoff verified; native hover delivery and multi-monitor menus need verification  |
| Tray menu           | Window height equals the painted menu at every simulated scale; fitted-menu image baselines                                                 | Native prewarmed client height verified at 100%; physical mixed-DPI transitions remain unverified |

Geometry assertions check 21 toolbar entries, 28/32px control sizes, spacing,
nonoverlapping options, visible caption buttons and the ability to scroll overflow
controls into view. Caption widths follow GPUI's device-pixel rounding. Selection
edges allow at most half a physical pixel of snapping. Menu actions assert selected
tools and open panels before their screenshots are named. Recording-bar children
must fit their viewport without overlapping. The recorder fixture asserts one
start/pause/resume/stop call, enabled inputs at start, disabled live toggles and
unchanged saved preferences. Discard must remove the sole project inside the
isolated recordings directory.
At each simulated scale, an unfrozen screenshot fixture drags from device-pixel
coordinates on a negative-origin display to its right and bottom edges. It checks
the painted selection, exact daemon request, saved image dimensions and four
preview pixels against the saved four-color synthetic image. The fixture does not
capture physical monitor pixels. Temporarily disabling edge snapping made its
125% frame check fail.
Four additional small selections at the display corners check the size label's
painted bounds and clipping mask. Bottom-edge labels must also clear the selection.
Unit coverage checks left, center and right positions, pointer routing through
the label, resizing from the outside outline and simulated 100%, 125%, 150%, 175% and 200%
scaling. Pixel checks scan the dimmed scrim at the corners and across
fractional-position selections at every matrix scale, rejecting both undimmed
gaps and double-dimmed overlaps while
checking that selected pixels remain undimmed. Samples on all four sides verify
the surrounding dim, the undimmed interior and the outside outline.
Picker fixtures place ordered overlapping windows on a negative-origin display
in correctly scaled physical coordinates. Pixel samples independently check the
undimmed candidate and 50% scrim, including targets crossing left, right and bottom edges
and restoration of dimming on leave. The headless platform always reports a
non-hovered window, so the fixture supplies window ownership to the shared
production surface; production rendering still reads the native hover state.
An additional picker scan checks every pixel across the lower viewport. The
recording handoff test checks every physical pixel of its painted scrim and clip
at 100%, 125%, 150%, 175% and 200%, with centered and edge selections. It requires one
dim layer outside the selection and none inside.
The microphone icon must provide enough pixels with at least 3:1 contrast
against the toolbar. Its actual canvas bounds are measured independently of the
dropdown chevron and checked against the containing button. The former
light-theme white content failed with zero contrasting pixels. Media permission
checks return granted only in a parity build running the daemon fixture, before
reading host consent or opening a permission dialog.
Held-button capture advances the mock clock before reading pixels and must
differ from hover inside the measured microphone button bounds.
The font-size submenu samples the keyboard focus ring on all four sides of the
current, first and last options, independently of baselines, and preserves each
as `submenu-text-focus-*.png`. The original virtual viewport failed with zero
focus pixels on the selected option's left edge. The shared Select fix passed
that probe while preserving its 24px rows and 64×276px light-theme panel.
All-in-one Capture text, Pick color and Close tooltips must fit below their
controls, within the viewport and their clipping masks, in both normal and
400×300 overlays and while Capture text is selected. The original top placement
started 5px above the display. An additional 175% tooltip and target-menu probe
passed in both themes; it has no image baseline. The narrow target menu must remain
anchored below its trigger and inside its clipping mask. Synthetic keyboard input
checks target selection, Escape dismissal and reopening from the focused trigger.
Capture-preview hover ownership and keyboard dismissal are checked in all four
corners. Windows and macOS use native hover tooltips, which are outside GPUI's
headless screenshots; Linux keeps the in-window HeroGPUI tooltip. Keyboard focus
labels and Linux preset names wrap inside the card, with long visual labels
ellipsized while the full name remains available to accessibility tools. The
fixture checks that Edit has no tooltip and remains clickable. Video Copy and
Show in Folder hover images use a missing-media fallback, not a decoded recording.

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
pixel check. Subsequent hosted capture and comparison status are recorded below.

The next [hosted Windows run](https://github.com/Porabuild/Poratake/actions/runs/42786386596/job/129728542684)
at `2cfb70e` passed 957 native tests with one skip and all 16 cases' geometry and
workflow assertions. Exact comparisons failed because the reported
`hardware-microsoft-basic-render-driver` profile had no reviewed baseline.
Review of all 1,928 captures found two image-edge changes against the preceding
hosted run; the other 1,686 shared non-video states were byte-identical.
Those edge changes exposed actual atlas bleed, so these images were not accepted
as baselines. A new GPU probe reproduced it locally: source `[240, 80, 64]`
became `[245, 142, 132]` near an image edge. Clamping samples to each tile's texel
centers passed the same probe and the capture workflows. Every Windows matrix
case runs this color assertion before baseline comparison and saves
`image-atlas-sampling.png`. After the atlas and Select fixes, 340 changed local
baselines were reviewed and regenerated. A fresh run of the frozen verified
binary passed all 16 cases and 1,928 exact images with updates disabled. The Select
fix changed only the 48 keyboard submenu images; the other 1,880 captures stayed
byte-identical to the atlas-only run.

The next [hosted run](https://github.com/Porabuild/Poratake/actions/runs/42799398288/job/129772749091)
at `dd6b68e` passed 960 native tests with one skip, plus every case's atlas,
focus-ring, geometry and workflow checks. Exact comparison again rejected the
missing hosted profile. All 1,928 captures were reviewed; the 340 changed images
and 47 changed states versus the rejected run match the local shader/Select
change set. The other 1,588 images stayed byte-identical. Those reviewed captures
now establish `hardware-microsoft-basic-render-driver` with driver
`10.0.26100.33648`. Artifact `13758275750` has SHA-256
`3caeda627db64c2dc92184f09f61042cf6ed24ff2ea78e71f0db188d1467eebf`.
The next [hosted run](https://github.com/Porabuild/Poratake/actions/runs/42806652330/job/129797393662)
at `abc940f` passed 961 native tests with one skip and all 16 cases’ geometry,
workflow and atlas checks. It rejected the incomplete state list as intended.
Its 176 new tray/tooltip/Edit-hover images were reviewed, and all 1,928 earlier
hosted images remained byte-identical. That 2,104-image profile was complete at
`460ed6f`. The artifact SHA-256 is
`53a763a3afcd985d4cba1c731526a88896a6f10ca5b480bc86e9793048e1e42a`.

The next [hosted run](https://github.com/Porabuild/Poratake/actions/runs/42813783806)
at `460ed6f` passed all jobs. Windows passed 963 native tests with one skip and
all 16 exact visual cases against the 2,104-image Microsoft Basic Render Driver
profile.

The [next hosted run](https://github.com/Porabuild/Poratake/actions/runs/42819345589)
at `d83494f` passed 965 native tests with one skip and reached the visual
state-list assertion in all 16 cases. The gate rejected the 32 new edge images
because they were not yet in the hosted profile. All 2,104 existing images are
byte-identical to their baselines; the 32 new default and hover images were
reviewed across themes, widths and scales. Artifact `13768140621` has SHA-256
`3365f2021acf721a6425bc179dfbb78e34dde355b84e9e92eb29001446233bdf`.
The next [hosted visual run](https://github.com/Porabuild/Poratake/actions/runs/42823276665)
at `b091e35` passed all 16 cases against the 2,136-image profile without baseline
updates. Artifact `13769796702` has SHA-256
`19cc87f9786816d08d3b7a4c10f1c6238a05b121a11ae4be196eb2e5cd18d98c`.
The [hosted run at `0c5e134`](https://github.com/Porabuild/Poratake/actions/runs/42827223536)
also passed the required jobs, including 971 Windows native tests with one skip and the
same 16 visual cases. Artifact `13770638988` has SHA-256
`dd121943c27275e18143be5a837a289f94ab52791226fc43aee3cc960eeb0d52`.
The [hosted run at `da95f73`](https://github.com/Porabuild/Poratake/actions/runs/42832652057)
passed the required jobs, including 972 Windows native tests with one skip and all
16 visual cases against 2,136 exact images. Its visual artifact `13774605959` has SHA-256
`4ee4a1c8778869a8527cc2784c354fe0b986cf9314a7c2806ae7d59a6b3237ed`. Linux also
passed the unevenly lit QR photograph regression.
The [hosted run at `9407fa6`](https://github.com/Porabuild/Poratake/actions/runs/42845902225)
passed the required Windows, Linux and macOS jobs, including the Windows visual
comparison.
The [hosted run at `9669cde`](https://github.com/Porabuild/Poratake/actions/runs/42851318895)
also passed the required jobs, including the scaled selection and preview fixture.
The [hosted run at `372db36`](https://github.com/Porabuild/Poratake/actions/runs/42862315214)
passed all required jobs, including the Windows visual comparison.
The [hosted run at `a0168f3`](https://github.com/Porabuild/Poratake/actions/runs/42871830573)
also passed all required jobs, including Windows, Linux and macOS tests.
The [run at `831c9de`](https://github.com/Porabuild/Poratake/actions/runs/42876204997)
passed Windows native tests and the visual geometry checks, then rejected the
16 new canceling-state images absent from the hosted profile. All 2,136 existing
hosted images were byte-identical; the 16 new images were reviewed and added.
GitHub reports artifact `13791799183` with SHA-256
`9a5748d75fd3255d023979cd25b9a30b6ec95ca97cd867d439dff8eaba5ff356`.
The [run at `a16a0a7`](https://github.com/Porabuild/Poratake/actions/runs/42879631430)
passed all required jobs, including all 16 Windows visual cases and the normal
2,152-image hosted comparison.
The [run at `72f0f59`](https://github.com/Porabuild/Poratake/actions/runs/42913126458)
also passed all required jobs and the 2,152-image hosted comparison. The next
hosted run captured 80 new tooltip images; they were reviewed and added, while
the previous 2,152 images remained byte-identical. Its Linux X11 test failed on
daemon exit once and passed on retry; the cause of the first exit is unverified.
The [run at `090954e`](https://github.com/Porabuild/Poratake/actions/runs/42968353563)
also failed its scaled GTK X11 test after the print-dialog workflow completed,
while waiting for daemon exit. Its exit status was not reported; the test now
prints it if the failure recurs. An isolated GTK print-dialog probe exited cleanly
300 times, but did not exercise the daemon and did not establish the cause.
The same run's Windows job passed its native tests and all visual geometry and workflow checks,
then rejected the 96 new color-picker images absent from its baseline. All 96
were reviewed; the other 2,536 images were unchanged. Artifact `13834135629` has
SHA-256 `5d8e382bbbc961eb0f2a6d463db48d7ff5e77f496e1c7d6f4afe188f9ebd1a21`.
The Linux job in the [next run](https://github.com/Porabuild/Poratake/actions/runs/42972460141)
passed both X11 tests, including scaled GTK; the earlier exit remains unexplained.
Its Windows native and geometry checks passed, then the visual gate rejected the
96 new all-in-one tooltip states. All were reviewed; the previous 2,632 hosted
images were pixel-identical. Artifact `13835681057` has SHA-256
`69194ad53c1987a87bc5af0710f22a33f3929226c1fe2888a068176adac98d74`.
The [run at `bf4ac143`](https://github.com/Porabuild/Poratake/actions/runs/42977256566)
passed Linux, macOS, Windows native and geometry checks, then rejected 80 new
selected-OCR and target-menu images. All 80 were reviewed; the previous 2,728
hosted images were pixel-identical. Artifact `13837611366` has SHA-256
`08b97e272ba8d5a2c60a905936f643e8847b3185a908bd027d6f87da5bc34b5a`.

The earlier local exact comparison passed all 16 cases. Contact sheets for all cases and
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
The pin close-callback fix subsequently passed the native gate with 959 tests
and one skip. Its regression first found no restored editor; after the fix it
verified the original file, annotations and exact exported PNG with history
disabled. A standalone pin closes without opening an editor. These tests invoke
the platform should-close callback and dispose the test window separately;
native OS event delivery was subsequently verified in an isolated development instance.
Pin windows now have an OS title so native window inspection can identify them;
the title change passed the four pin tests and parity-feature Clippy.
Native foreground input added a rectangle to an owned checker fixture, pinned
it, moved the pin and sent Alt+F4. The pin HWND disappeared and a new editor
retained the rectangle. A real screen capture opened only a bottom-right preview;
double-clicking that preview opened its saved image in the editor. The older
development editor remained open. A separate preview on the earlier inward-tooltip build showed the complete
Close tooltip inside the native card.

A native attachment captured darkened pixels because the editor was still
animating out when the frozen frame was acquired. A distinct red/yellow editor
fixture reproduced its translucent image over the underlying desktop. Windows
now disables that window transition and flushes the compositor after hiding.
With a separate blue/gray fixture underneath, the fixed 200×120 captured layer
matched all 24,000 reference pixels exactly. Its full composition was visible,
and the saved 1240×360 PNG retained the original 640×360 pixels exactly; the
enlarged attachment differed from bilinear reference by at most one channel
level. The isolated test windows were closed and the older editor remained open.
The real native handoff is not yet automated in the hosted harness; the headless
fixture does not test Windows compositor transitions. Evidence is in
`.scratch-parity/native-handoff-green-9dfaa280082844d9bba4b43f417955c0/`.

The tray sizing regression reproduced a 632px viewport around a 564px painted
menu. Measured child height now sizes the window; all four simulated scales fit.
A native test using a synthetic tray rectangle and the original shortcut labels
placed the 564px menu 8px above the taskbar work area. The older development
instance still uses its earlier binary; it was kept open at the user’s request.
Geometry tests also check that changing the measured height preserves the
taskbar anchor and that top/side placement stays inside the work area.
Windows monitor lookup now uses GPUI's selected monitor identity instead of
passing a logical display center to a physical-coordinate API. A regression first
showed an invalid identity selecting a nearby monitor; it now rejects that
identity and matches Windows' monitor and work rectangles for a valid one. The
physical mixed-DPI and monitor-transition checks remain outstanding.
A tooltip regression reproduced a 94x32px label above and left of the preview.
The initial fix used inward HeroGPUI placements; Edit has no tooltip. Hover labels
now use native tooltips on Windows and macOS, while keyboard focus labels and
Linux retain HeroGPUI tooltips.
The tooltip-presence guard also rejects a build with tooltips disabled. A preset
name later produced a 254px Polish tooltip inside a 200px card; it now wraps
within the card and limits very long visual labels to three lines. All 80 new
local renderer images were reviewed, and the previous 2,152 stayed byte-identical.
A normal run then passed all 16 cases and 2,232 images without updating baselines.
The four-display preview menu originally painted 220×158px outside its 200×140px
card. It now fits inside the card and scrolls, stays open after pointer leave,
and closes when its owner is removed or display topology changes. Synthetic input
checks keyboard and mouse selection, short-menu scrolling and auto-dismiss. GPU
samples check all four sides of the first and last focus rings at every simulated
scale and corner. The 96 local menu images were reviewed; the previous 2,232 local
images were byte-identical. A normal run passed all 16 cases and 2,328 images
without updating baselines. The Windows native gate passed 1,006 tests with one
skip, and 74 vendored dropdown tests passed. Physical multi-monitor behavior
remains unverified.
Review of the hosted menu images then exposed a clipped label when keyboard focus
moved to the last display. A shorter menu initially exposed parts of the preview
controls beneath it. The menu now fits three complete rows and hides those controls
while open. Pixel checks reproduced both defects at 100% before the fixes; the
outside-click test also checks clipboard preservation and restored controls. All
16 local matrix cases passed, and the 96 revised menu images were reviewed; the
other 2,232 images remained byte-identical. The [hosted run](https://github.com/Porabuild/Poratake/actions/runs/42935006992)
reached the state-list check in all 16 cases. Its 96 new menu images were reviewed,
and its 2,232 existing images were byte-identical. Artifact `13819691455` has
SHA-256 `9514eb08c98dbb5471a375a8cc2d86473d7c736e505252098e3d74780113503b`.
That run's Linux and macOS Clippy jobs rejected unused test-handle assignments;
those assignments were subsequently corrected. The [later hosted run](https://github.com/Porabuild/Poratake/actions/runs/42938607663)
passed Linux, macOS and Windows, including all 16 cases and 2,328 exact images.
The [framed-transparency run](https://github.com/Porabuild/Poratake/actions/runs/42941558246)
also passed all required jobs and all 16 visual cases, including the new pixel
checks on the hosted renderer. The [history and frame-only run](https://github.com/Porabuild/Poratake/actions/runs/42945831848)
passed all required jobs, 1,015 native tests and all 16 visual cases. The
[later hosted history run](https://github.com/Porabuild/Poratake/actions/runs/42948956984)
passed its history geometry and deletion checks but failed the eight 950px
state-list comparisons because their new images had not yet been added. Its
2,328 existing images remained pixel-identical, and the 80 new images were
reviewed. Artifact `13826065362` has SHA-256
`5a4f1960ff24a1920489f6568c8f81170a27b2b81806ec7fd8ff1219ee8c54fd`.
The [grid-tooltip run](https://github.com/Porabuild/Poratake/actions/runs/42952945962)
passed the same geometry checks and eight 1280px cases; the eight 950px state
lists lacked the 64 new images. All 2,408 existing hosted images were pixel-identical,
and the 64 new images were reviewed. Artifact `13827306848` has SHA-256
`d9cbd127c06a8e3ea96f4c020fdadbc77d6760b8f32f49ece77acbd48214c90c`.
The [selection run](https://github.com/Porabuild/Poratake/actions/runs/42960670267)
passed the geometry and workflow checks but failed the 16 state-list comparisons
because 64 new corner images were absent. Those images and the 160 changed
selection images were reviewed; the other 2,312 existing images were unchanged.
Artifact `13829827867` has SHA-256
`e8ed36fccbec9406e86d1411a6964032d79f9a86785d28c09aad9c2bd1e5598e`.
The [DPI follow-up run](https://github.com/Porabuild/Poratake/actions/runs/42963926798)
passed the geometry and workflow checks and 14 visual cases. The two narrow
125% cases differed in 24 history-grid images, all reviewed; the other 2,512
hosted images were unchanged. Artifact `13832141444` has SHA-256
`d61c324f6de4a9030ecf81bed2977ed43963a693bfa22ad4fb6fa72e349eea6d`.
The [140px history run](https://github.com/Porabuild/Poratake/actions/runs/42986753020)
passed the geometry and workflow checks and eight wider visual cases. The eight
narrow cases rejected their 72 missing states; those images were reviewed and
all 2,808 existing images were pixel-identical. Artifact `13841822779` has SHA-256
`0bdfea91ec8148b6390a7d7aead6dff99b853ce1f8374c88d66a7cd3b99049a6`.
Check the current PR for subsequent changes.
The display-change test checks preview repositioning in a headless window, and
synthetic geometry covers negative origins and mixed scaling. Physical monitor
changes and destination-DPI movement remain unverified.
The pooled-overlay regression reproduced retained selection and drag state, which
are now cleared on park. The editor-size regression found no stored initial
size; successful opening now remembers it before another image can choose a
different size. Reopening small/large images and a resized editor are exercised.
The expanded local matrix recorded all 16 cases and 2,104 images. All 176 new
images were reviewed, and the existing 1,928 local images stayed byte-identical.
A fresh normal matrix passed all 16 cases and 2,104 exact images with baseline
updates disabled.
The attachment-edge expansion adds 32 reviewed local images; the earlier 2,104
local images remain unchanged. A normal run passed all 16 cases and 2,136 exact
images. The four edge indicators are also checked independently of baselines.
The native gate passed 963 tests with one skip after the tray anchor tests and
compositor fix; typecheck, lint, formatting and parity-feature Clippy also passed.
With the attachment-edge interaction and focus tests, it passed 965 tests with
one skip; typecheck, lint, formatting and parity-feature Clippy also passed.
After the recording-preference and toolbar-hit fixes, the local matrix again
passed all 16 cases and the native gate passed 967 tests with one skip.
An additional Windows encode/decode test checks cursor, keyboard shortcuts and
subtitles before and after a cut at 1.5× and 0.5× speeds. It checks which keys
and captions are active, proves each layer changes the preview, and compares
the affected pixels in the export with the preview. The native gate passes 968
tests with one skip after this addition.
An edited GIF workflow now checks saved and reopened state, preview pixels and
every decoded frame boundary at all six offered frame rates. Before the timing
fix, a two-second export played for 1.8 seconds at 30 FPS and 1.92 at 24 FPS;
afterward each frame boundary stays within 5 ms of the edited timeline.
Windows export tests cancel MP4 and GIF at an early frame and at the last frame,
then retry the same path and decode the result. A valid editor project also checks
that retry is blocked until cancellation finishes and succeeds afterward, with no
cancel notification and one success notification. The test passed ten isolated
runs; the native gate passed 992 tests with one skip. The canceling label and its
footer are checked against their actual bounds at every matrix scale. Sixteen new
local canceling-state images were reviewed, and the normal matrix passed all 16
cases and 2,152 exact images without updating baselines. The hosted renderer's new
images were also captured and reviewed. Moving the label outside the footer made
the geometry assertion fail; the restored layout passed. The video editor fixture
uses a 1,200px minimum width even when the matrix case is 950px.
Isolated Linux encoder tests exercise blocked input, cancellation during mux and
staged-file cleanup. A simulated macOS backend on Linux exercises blocked-input
cleanup; neither fixture establishes actual macOS or Linux app behavior.
The audio check compares preview stems and a decoded AAC export across a cut and
speed changes. It verifies 440 Hz and 880 Hz source tones, two short sounds near
their edited positions, two keyboard clicks after raising their volume from zero,
and the full two-second output duration. Signal tests cover stereo tones from
50 Hz to 12 kHz at the non-unit speed presets from 0.5× to 4×, nine short sounds
within 40 ms of their expected positions, and sounds at the end of input and before
silence. A file-only synthesized speech sample was also run through the production
stretch algorithm and Electron's FFmpeg filter at the non-unit speed presets; a
coarse windowed comparison found no silent windows in either output. This was not
a listening test or a full editor/export test. These are synthetic fixtures, not
live speech or music. A scheduler test
covers undoing an edit while its audio rebuild is in flight. The local native gate
passes 984 tests with one skip after the audio, attachment, QR and monitor fixes.
The normal local visual matrix also passed all 16 cases and 2,136 exact images
after the audio, attachment and monitor fixes, without updating baselines.
Most GPUI surfaces use HeroGPUI tooltips and shared helpers. The tray icon,
macOS scroll controls, and Windows and macOS recording and capture-preview
hover controls use native tooltips.
The earlier camera/zoom export expansion passed the native gate with 960 tests and one
skip, plus typecheck, lint, formatting and parity-feature Clippy. The seven decoded
samples verify camera visibility and source-frame changes across a cut; an
independent checker comparison requires the manual zoom to affect the image.
The preceding capture-preview expansion passed all 1,928 images, including the
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
The full-window QR regression failed before the fix and passed afterward. Against
the original payload files, the local-threshold fix improved exact native matches
from 42/65 to 48/65; a global-threshold fallback raised this to 50/65. Additional
local threshold scales raised it to 59/65. Regional and additional threshold
retries raised it to 65/65 without losing a previous success. The
earlier 38/65 and 43/65 counts normalized line
endings on only the expected side and undercounted CRLF payloads. The single-pixel
baseline failure regression also passed separately. The Windows native gate passed
980 tests with one skip after the earlier QR threshold regressions.

A low-contrast photographed code failed before the guarded normalization retry.
Against 520 original and brightness/contrast variants of 40 existing photographs,
the shared decoder improved from 367 exact matches to 493, with no prior match lost.
The Windows regression also covers quarter-turns and a valid code beside a damaged
candidate. These generated variants are development evidence, not a CI fixture.
The Linux QR loader passed eight isolated tests against the same shared decoder.
For a synthetic low-contrast 4K UI without a code, an alternating-order run with
two warmups and eight measured samples had a median of 1.32 seconds before the
retry and 2.04 seconds with the guard. The guard avoids the final retry when it
finds no candidate, but its threshold scans still add latency. The normalization
buffer can reach 64 MiB at the supported image limit.
The shared decoder now computes the same clamped 5×5 black-point averages with
rolling sums. The direct-average regression covers grids from 5×5 to 17×17;
all 520 brightness/contrast variants produced identical payloads, with the same
493 expected matches. On this Windows machine, an alternating release-mode
benchmark with one warmup and eight samples reduced median processing of an
already-decoded 3840×2160 low-contrast screen from 991.35ms to 927.75ms, and a
4K image with an embedded code from 398.80ms to 352.95ms. These timings exclude
capture, image decoding and IPC; the rolling sums add two bytes per block column.

Hosted [Linux GPUI CI](https://github.com/Porabuild/Poratake/actions/runs/42786386596/job/129728542346)
passed at `2cfb70e`: 798 GPUI tests, 28 Linux-daemon tests including the full-window
photographed QR regression, and both X11 capture runs (normal and `GDK_SCALE=2`).
This adds Linux service/test evidence; it does not establish interactive Linux
app parity or physical monitor scaling.

Hosted [macOS tests](https://github.com/Porabuild/Poratake/actions/runs/42786386596/job/129728542646)
passed at `2cfb70e`, including 800 GPUI tests, the Swift daemon build, Rust checks
for Apple Silicon and Intel, and Clippy. Interactive macOS app parity remains
separate verification.

| Check                           | Verified result                                                                                                                                                                                                                                                                                                                                                                         |
| ------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Annotate → save → reopen        | Real pointer events create pen, highlight, rectangle and redact annotations. Mouse release differs from the last move. Persisted history reopens identically; flattened PNG reopens without editable annotations and with identical decoded output.                                                                                                                                     |
| Screenshot preview versus PNG   | Native rendered image region versus saved output: zero pixels differ by more than two channel levels. Rectangle/circle/line/arrow previews use the export rasterizer; cache tests distinguish captures with identical annotations.                                                                                                                                                      |
| Wallpaper scaling and output    | At simulated 125% and 200%, simple framed and rounded single-image regions match the PNG within two channel levels; translucent custom backgrounds can differ by three. Frame styles, shadows, corner annotations and selection/crop handles are checked in their supported combinations. Complex compositions remain outside this check.                                               |
| Edited video versus MP4         | Windows Media Foundation fixture, trim/gap removal, 2×/0.5× speed, wallpaper and drawing; project save/reload; decoded output dimensions/duration, four sampled frames, whole-frame and drawing-region error checks.                                                                                                                                                                    |
| Camera and zoom versus MP4      | Synthetic two-color camera, recorded on-periods mapped across a source cut, manual 2× checker zoom, saved-state reload and seven decoded frames. Independent visibility, source-color and zoom-change assertions accompany preview/export error checks.                                                                                                                                 |
| Audio preview versus MP4        | Synthetic audio and keyboard sounds are mixed in preview and compared with a decoded AAC export across a cut and speed changes. Both source pitches, two short sounds, the two-second duration and each keyboard click are checked independently.                                                                                                                                       |
| Native screenshot/editor        | Owned editor window captured through the daemon; native pointer annotation and Save Screenshot dialog produced a real PNG.                                                                                                                                                                                                                                                              |
| Native static recording         | Pause/resume recording formerly finalized at 0.0333333s; fixed daemon reports 1.6345311s, decoded stream 1.634500s, excluding the pause.                                                                                                                                                                                                                                                |
| Windows recorder status         | Seventeen CPU-only Windows recorder tests pass, including static-source live-duration and queued Pause/Stop checks. Hosted Windows runtime verification is pending.                                                                                                                                                                                                                     |
| Native changing recording       | Pointer/shape edits during recording finalized at 52.0008992s; decoded stream 52.000867s with nine change-driven frames, exercising the tracked-texture pool.                                                                                                                                                                                                                           |
| Native record → edit → export   | The static recorded project opened in the GPUI editor, accepted a wallpaper change and exported through the native Save dialog: 1052×750, 50 frames, 1.666633s at 30 FPS.                                                                                                                                                                                                               |
| Recording-control UI/RPC        | Production controls with deterministic device/recorder responses; Escape cancels countdown and leaves an active recording open; pause/resume and live input changes issue the expected requests; discard removes the fixture project.                                                                                                                                                   |
| Accessibility action dispatch   | An isolated GPUI test platform exercises the actual AccessKit dispatch path for Select expansion and Slider value actions, including disabled and range controls. It does not exercise an operating system accessibility adapter or screen reader.                                                                                                                                      |
| Loaded-video state and preview  | Production editor opens an encoded project with a drawing; wallpaper/export changes autosave and reload; the UI preview bytes equal a fresh composition of persisted state. All ten side panels have image baselines.                                                                                                                                                                   |
| Native countdown cancellation   | Accessibility tools identify the named recording-control window; Start displays a ten-second countdown and a background Escape message closes the bar before recording starts.                                                                                                                                                                                                          |
| Native light control menu       | A light-theme development window displays readable controls, enumerates the actual microphone list, opens its menu, dismisses it with Escape and closes through its accessible Close button.                                                                                                                                                                                            |
| Selection capture routing       | Production window, area and all-in-one display startup, saved pixels, history registration and editor-export equality for live, frozen, failed and unavailable-freeze paths. Physical rectangles and capture-before-release ordering are asserted. In-memory clipboard checks cover automatic copy, disabled copy and clipboard-only capture.                                           |
| Timer capture lifecycle         | Production selection/countdown completion, cancellation, timeout, show failure and retry. The frozen selection frame is released before countdown; completed captures use live pixels; unsuccessful attempts leave no image/history/active selector.                                                                                                                                    |
| OCR/QR capture lifecycle        | Twelve production scenarios cover success, whitespace-only results, recognition error, partial-file capture error, Escape cancellation and retry. Clipboard/notification outcomes, preprocessed OCR input, temp-file deletion and no history/output leaks are asserted.                                                                                                                 |
| Scroll capture lifecycle        | Fourteen scenarios and a held-start probe cover cancellation, early events, errors, held responses and retry. Live-screen parameters, cleanup and saved output/history/editor equality are asserted. Two-frame stitches reopen pixel-identically, including exact 97px and 217px overlaps earlier rejected or misidentified, and a 101px tie. Real-screen stitching remains unverified. |
| Editor attachment lifecycle     | Eight production scenarios cover all four edges, frozen/live/fallback pixels, partial capture failure, invalid image, cancellation and retry. Exact composed pixels and history reopen match; temporary files, new history entries and clipboard changes are rejected. Competing capture requests preserve the pending selector, and a selector that cannot open restores the editor.   |
| Pin composition and restoration | Rendered pin pixels match the annotated PNG within two channel levels. Closing the pin restores annotations and identical export bytes with history enabled or disabled, without adding history entries. Enlarged/narrow bounds are checked against natural logical size.                                                                                                               |
| Capture preview lifecycle       | Four corner configurations preserve 200×140 cards with 12px gaps. Adding a fifth capture evicts the oldest card without deleting its file; color pixels verify order. Double-click opens the selected capture with identical PNG export. Auto-dismiss closes the stack while preserving files and history.                                                                              |
| Native analysis services        | Windows capture of an owned OCR editor window contains the expected recognized text. The rebuilt QR service reads the full 952×651 captured window, including its toolbar/margins. Native corpus checks improve from 42/65 to 65/65 exact payloads without regressions. These probes do not certify native app clipboard delivery.                                                      |
| Native live window pick         | Background input selected an owned editor window, closed the selector and opened its 952×651 capture; decoded pixels exactly matched a direct daemon capture of that target.                                                                                                                                                                                                            |
| Native frozen window pick       | With freeze enabled, the former path captured a rectangle added after opening selection. The fixed saved output preserves the pre-change region: zero changed pixels versus before, while the live target differs by 3,761 pixels in that region.                                                                                                                                       |
| Native frozen area capture      | The selector client origin formerly sat four pixels above the display after activation. It now aligns at (0,0); the saved 400×240 region exactly matches the intended frozen crop, excluding a later annotation that changes 2,393 live pixels.                                                                                                                                         |
| Native timer capture            | A source annotation added during the real five-second countdown appears in the saved 400×240 capture: zero changed pixels versus the live target and 1,335 versus the pre-countdown region.                                                                                                                                                                                             |
| Native timer cancellation       | The named native countdown panel is discoverable through accessibility inventory. A background click closes it and the selector; no screenshot or history file appears after the countdown would have ended.                                                                                                                                                                            |

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
The grayscale full-window and threshold-scale regressions are retained in the
native test fixtures; their sources and license are recorded in `THIRD_PARTY_NOTICES.md`. The remaining
probe assets stay outside the app and visual baselines. Native before/after
results for 20 QR photos, their grayscale originals and framed/resized variants
are in `.scratch-parity/qr-corpus-*/native-{before,after,otsu,medium}.json`.
The current decoder passed all 65 cases and four placements of a photographed QR
inside a synthetic 3840×2160 image. An isolated WSL harness compiled the actual
Linux QR loader and passed its eight tests; it did not build the full Linux daemon.
In an interleaved release benchmark, a 4K gradient without a QR took 493–496 ms
versus 334–370 ms before the regional and extra threshold retries. A uniform 4K
image took about 2 ms after an early exit, versus 291–323 ms before. A later
same-machine run measured 481–505 ms before the transition preflight and
381–404 ms afterward; an embedded 4K QR remained around 367–385 ms in both.
The Windows daemon still matched all 65 payloads and the Linux loader passed its
eight isolated tests. The preflight preserves thresholded rows with at least four
color transitions, including edge patterns. Evidence is in
`.scratch-parity/qr-preflight-corpus.log` and `.scratch-parity/qr-preflight-bench-idle.log`.

Bugs fixed in this pass:

- Editor changes made shortly after opening could remain absent from history
  until the window closed. A trailing save now persists the latest committed
  state; light and dark workflows check the deadline, an unchanged selection,
  reopening matching annotations and PNG pixels while the editor stays open,
  and closing during a new unfinished annotation.
- Windows screenshot requests now reject overflowing bounds. Monitor midpoint
  and crop offset calculations use wider intermediates; synthetic tests cover
  both overflow axes, extreme coordinates and negative-origin crops. Physical
  multi-monitor capture remains unverified.
- Cached Windows captures now reject disjoint frames before allocation and
  stream output in bands of at most 4 MiB instead of allocating the entire
  composition. Dimension checks accept an eight 8K-display layout with padding
  under the 512 Mi-pixel limit; larger cached requests fail before mask capture or file
  creation. Tests preserve off-screen padding, display gaps, alpha masks and
  the empty-cache fallback. A window alpha mask and encoder internals can still
  use additional memory; large native captures and slower hardware remain
  unprofiled.
- Clicking an annotation or resize handle, or reapplying its current option,
  discarded Redo without changing the image. Moves, resizes and option changes
  now create a history entry only when the annotation differs; returning to the
  starting point restores the original. Tests cover all four rectangle handles,
  a clamped circle, unchanged and mixed-selection options, real moves and
  resizes, and queued pointer updates before Undo or an option change. A GPUI
  workflow also selects a circle and exercises Redo and Undo through the window.
  Electron's source commits a drag only after an update was staged, but does not
  compare the final state with the starting state; native Electron behavior for
  these cases remains unverified.
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
- A photograph with uneven lighting needed a global histogram threshold after
  both adaptive passes failed. Windows and Linux loader regressions use the same
  test-only photograph; the Windows service returned the exact expected payload
  and the Linux regression passed hosted CI.
- The fixed local threshold size could merge a photographed finder ring into its
  background, or leave an otherwise detected grid undecodable. Two bounded local
  threshold scales run only after the earlier passes fail. Two new photographs
  now decode through the Windows loader; Linux loader tests passed hosted CI.
- Some photographed codes produced a grid that could not be decoded against its
  surroundings, or needed a different threshold scale. Bounded regional retries
  and two final threshold scales recover the remaining corpus cases, including a
  QR at four positions inside a synthetic 4K capture.
- GIF frame delays were truncated independently to centiseconds, speeding up
  24 and 30 FPS exports. Rounded cumulative frame boundaries preserve the edited
  timeline within GIF's centisecond precision.
- Canceling an export could leave its worker writing while a retry reused the
  output, or report success at the final frame. Cancellation now waits for the
  worker and suppresses success actions; Unix encoder children are stopped and
  reaped before the output can be reused.
- Scroll capture subscribed after the start RPC, losing early completion or
  cancellation events and leaving the session active. Subscription now precedes
  start; both early events and a successful retry are exercised.
- Scroll startup waited for the daemon on the GPUI thread. It now reserves the
  session before starting the RPC in the background. A held-reply fixture checks
  that the coordinator returns, rejects a second user request, and still saves
  and reopens an early completion. An isolated test checks that a queued start
  skips the daemon after shutdown begins. The headless dispatcher exercises
  ordering, not native OS responsiveness; an already admitted start can still
  complete during shutdown and is then canceled.
- A new selection could release another active capture's desktop-icon hide.
  Capture flows now retain their own hide through selection, handoff and cleanup.
  An isolated Windows GPUI test covers replacement, cancellation, disabled
  hiding and failed timer/scroll handoffs without changing the real desktop.
  Windows scroll completion and cancellation attempt to restore icons before
  waiting for the daemon's final response. Pending icon transitions are serialized
  off the UI thread; replacement selectors and captures wait before freezing or
  reading pixels. Failed restores are retried; a second failure stops the next
  capture with an error. The fake-daemon workflow holds hide/show replies, checks
  saved pixels and failed-reply recovery, verifies rapid tray toggles, and checks
  that a superseded direct capture cannot report a stale error. An attachment
  restore failure clears the pending attachment and focuses its editor in the
  headless fixture. Isolated tests cover replacement, timer ownership and shutdown.
  Failed hide requests now stop the capture and attempt to restore icons even
  when the reply was lost. The fake daemon checks area, display, direct-screen,
  OCR and QR failures, restoration retry and release of a previous frozen frame.
  Native overlapping captures and real desktop-icon transitions still need
  verification.
- GPUI area screenshots froze the screen before hiding desktop icons, so the
  frozen image could retain them. Like Electron's area flow, GPUI now hides icons
  before selection and retains the hide through capture or cancellation. A fake
  frozen-frame fixture checks saved and reopened pixels and cancellation; real
  desktop compositor timing still needs verification.
- All-in-one now begins the configured icon hide before freezing. Screenshot
  window selections retain that hide through capture, and recording selections
  wait for restoration before opening the controls. New fake-daemon workflows
  cover saved pixels, recording targets, failures, replacement and a second overlay;
  they still need a hosted visual run and native desktop verification.
  Electron also hides icons before taking its frozen snapshot and restores its
  capture reference before revealing the overlay. Synthetic-pixel tests cover
  cancellation, replacement and restoration failures; native icon transitions
  remain unverified. Confirmed frozen frames stay owned until the screenshot
  consumer finishes; held-selection and held-capture tests check that a
  replacement waits. Windows live captures fall back to Electron's window hide
  if the native hide fails; unit tests also cover a delayed hide failure after a
  newer show.
- The frozen multi-display screenshot picker had the same ordering problem. It
  now hides icons before freezing and retains the hide until capture or
  cancellation. A fake-daemon workflow duplicates a display target and verifies
  request order and saved pixels; a separate simulated two-display test checks
  cancellation and the live-picker path. Physical monitors remain unverified.
- A timer could close a newer selector and capture its old area after the
  selector was replaced. Replacement now cancels its countdown; isolated GPUI
  tests cover replacement, queued completion or cancellation events and a held
  hide operation. A normal completion still attempts capture through the fake
  daemon. Native overlapping countdown and selection behavior still needs
  verification.
- On Windows, a single-display screen capture could leave a previous selector
  open and use its frozen pixels. It now closes the selector and releases its
  freeze before capturing live pixels; a newer selector invalidates a pending
  screen request. Mock-platform tests cover live and frozen replacement. A
  daemon-fixture workflow verified release before an uncached capture, saved
  pixels and clipboard pixels in the hosted run at `2d50016b`.
- On macOS and Linux a replaced selector could open after its freeze completed,
  or a live capture could start before that freeze was released. Pending freezes
  now retain ownership across replacements and cancellation; a failed release
  prevents the next selector from opening and remains available for retry.
  Isolated GPUI tests force the delayed-open path, simulate successful freezes
  and release failures, and cover rapid replacements and cancellation. Native
  macOS and Linux handoffs remain unverified.
- A failed or timed-out freeze could continue in the daemon after the app opened
  a live selector. The app now releases the failed freeze before falling back;
  if cleanup fails, it closes the selector and retains the generation for retry.
  An isolated GPUI test exercises failed cleanup and live fallback. A daemon
  fixture simulates a partially installed failed freeze, but its hosted run and
  a real delayed timeout remain unverified.
- On Linux, release could complete while an X11 freeze window was still being
  presented or a replaced window was still closing. Generation changes,
  presentation and release now share a lock, and partial presentation failures
  explicitly destroy and synchronize their created windows. Deterministic lock
  tests cover the ordering. Xvfb keeps the client connection open and confirms
  a mapped window is removed after a later frame fails; an isolated probe of the
  previous cleanup fails that check. A real desktop handoff remains unverified.
- Starting a screen recording on one display could leave a previous selector
  open or let it appear after the recording bar. The recording request now
  replaces the selector and waits for its freeze to release. Isolated GPUI
  tests cover an existing selector, a pending freeze, a newer selection,
  a superseded screen capture and a failed release followed by retry. Native
  recording handoffs remain unverified. A daemon-fixture workflow checks that
  the bar is initially closed, then checks the release RPC and the settled bar;
  it passed in the hosted run at `bbf5e4e9`.
- Releasing a selector while its freeze was starting could deadlock the freeze
  worker. Release now waits outside the operation lock and rechecks later
  reservations. Thread-coordinated tests cover the deadlock and a reservation
  arriving during release.
- A full GPUI scroll progress queue could discard Done or Cancel. Terminal and
  interruption signals now use a separate queue while preview updates stay
  bounded. A GPU-free test fills the progress queue, sends 20 warnings, and
  verifies both terminal actions. The burst-event workflow fixtures completed in
  the hosted run at `8b764bc0` before its image baseline comparisons failed.
- Scroll capture cleared its active state before finish or cancellation completed,
  allowing a second session to start while the daemon was still processing the
  first. Held-response fixtures now verify the state through both RPCs.
- Both shells previously timed out a scroll stitch after 30 seconds while the
  daemon could still be writing. They now wait for the response or daemon exit.
  A 31-second held-worker GPUI workflow saves the expected pixels; restoring
  the old timeout makes it fail. Electron tests advance the daemon client's
  clock and verify ordinary requests retain their timeout.
- Electron scroll capture also allowed a new session before cancellation
  completed. It now waits for cancellation or an in-flight stitch, stages the
  stitch in a unique sibling file, and removes only that file if canceled.
  Filesystem tests preserve an existing destination on cancellation and compare
  promoted bytes on success. A failed promotion retains the staged PNG and
  reports its location. Auto-scroll cannot restart while the stitch is in flight;
  real daemon and native UI delivery remain unverified.
- The macOS GPUI scroll controls treated global logical selection coordinates as
  scaled, display-local coordinates. Control, preview and prompt bounds now use
  the selected display's local coordinates; a platform-specific unit test covers
  a negative display origin at 1× and 2×. Native multi-display behavior remains
  unverified. Narrow selections at either display edge also placed the GPUI and
  Electron control bars offscreen; both now clamp the horizontal position, with
  Electron tests covering primary and negative-origin displays. The macOS GPUI
  preview now uses the same native click-through setting as its prompt, matching
  Electron's overlay; native pointer delivery remains unverified. Both shells
  keep the growing preview within the display, including a 200px-wide simulated
  display. Electron renderer tests cover offset and short displays, and a
  headless GPUI session verifies window and painted-image positions as frames
  arrive; a narrow-display test also checks that tall previews retain the bottom
  pixels. Physical narrow-display behavior remains unverified.
- The macOS GPUI scroll preview's rounded container did not clip the image
  corners. The image now has the same radius, and the Windows GPU harness samples
  all four corners and the center of the production preview at each matrix scale.
  Native macOS rendering remains unverified.
- The macOS scroll-control tooltips painted above their 52px window and were
  clipped. They now use the native view tooltip, matching Electron's native
  tooltip behavior without enlarging the window. A headless test exercises all
  three controls, checks their hover labels and verifies Start/Stop requests and
  Done/Cancel signals. Native tooltip appearance and timing remain unverified.
- The macOS scroll preview appended the first rows of each new frame, while the
  saved stitch appends its last rows. It now uses the trailing rows. A macOS CI
  fixture compares one, two and three preview frames with saved PNG pixels when
  measured overlap matches the expected scroll step; the fixture passed in
  [hosted CI](https://github.com/Porabuild/Poratake/actions/runs/42998144733).
- A macOS scroll capture could discard its first frame when its sampled hash was
  zero. The duplicate detector now starts without a previous hash. The macOS CI
  fixture exercised a black zero-hash frame and subsequent duplicates in the
  same hosted run.
- The coarse scroll overlap search rejected a true 97px overlap and selected
  716px for a true 217px overlap in patterned fixtures. When the sampled
  result is rejected or farther from the expected scroll, a unique exact
  full-row match now takes precedence. Two-frame PNGs reopen with identical
  pixels, including an equal-distance tie. A synthetic 4K no-overlap check
  took a median 3.20–3.34 ms locally versus 1.43 ms before; physical
  pages and slower machines remain unverified.
- The Windows GPUI backend wrote PNG without a standard bitmap format and
  misread DIB headers with embedded masks. It now supplies CF_DIB alongside PNG;
  five byte-level tests cover bitmap headers, alpha, 16-bit input and invalid
  offsets. An isolated [hosted Windows test](https://github.com/Porabuild/Poratake/actions/runs/43059430548)
  passed native text and PNG writes, PNG and DIB format availability, and
  PNG-only and DIB-only reads. Interactive delivery through the app and another
  application still needs verification. A synthetic 4K
  PNG-to-DIB conversion took 48–53 ms locally, excluding clipboard API calls,
  and produced a 31.6 MiB bitmap; slower hardware has not been measured.
- Polish-copy and editor wallpaper padding previously used unscaled image pixels.
  A 1600×1000 capture with 50px padding produced 1700×1100 at 200% instead of
  Electron's 1800×1200. Wallpaper geometry and frame scaling now use the same
  displayed-image ratio; padding without a background remains transparent in the
  saved PNG. Same-size effect edits retain the prior preview while its replacement
  renders. Physical monitor changes remain unverified.
- The padded, unframed single-image preview painted an opaque surface beneath
  partially transparent pixels. It now reveals the wallpaper backdrop in the
  tested case without corners, shadow, inset, balance or attached layers.
- A framed single-image preview also blended translucent pixels against the
  editor surface, and its extra GPUI shadow darkened them. The frame compositor
  now follows Electron: wallpaper shows through without a shadow, and the frame
  content color backs the image when a shadow is present. Shared raster edges
  also preserve the frame border and rounded clipping above the image and its
  annotations; selection and crop handles stay visible above the frame. At
  simulated 125% and 200%, every pixel of a 640×360 preview matches the PNG
  within two channel levels for all four styles, with and without shadow.
  Transparent and frame-only backgrounds, corner annotations and GPU atlas
  eviction are checked. Translucent custom backgrounds with blur are within
  three channel levels in the interior and two at the frame edges after theme
  changes. Generating 4K edge tiles took 4–5 ms locally and used
  about 40 MiB of temporary mask and border buffers, excluding the backdrop and
  GPU upload. Complex compositions and arbitrary zoom remain unverified.
- A rounded single-image preview left square corners and painted an opaque
  surface beneath transparent pixels. Without a window frame, inset, balance or
  attached layers, the shared raster mask now clips the live image and
  annotations to the exported shape, with or without a shadow. At simulated
  125% and 200%, the whole padded canvas matches the PNG within two channel
  levels over solid or transparent backgrounds; translucent custom backgrounds
  are within three levels. Corner
  annotation pixels, all four selection and crop handle quadrants, and crop
  dimming over the rounded cutout are checked. A local
  4K probe generated four corner tiles in 1 ms at 48px radius and 5 ms at 400px;
  the latter used 2.5 MiB of tiles plus an 8 MiB temporary mask, excluding the
  backdrop and GPU upload. A tiled shadow keeps the outline at its true size;
  a local 4K backdrop took 53–58 ms without a shadow and 88–93 ms with a 24px
  radius and moderate shadow. A 400px radius with the strongest tested shadow
  took 196–200 ms; GPU upload and slower hardware are not included. Plain,
  inset and framed 17,000px-wide and -tall captures retain shadows in saved and
  reopened PNGs. CPU preview shadow checks match the saved output; GPUI GPU
  rendering at those dimensions remains unverified.
- Unframed attachments without inset or balance were rounded and shadowed as one
  group, unlike Electron's per-image rendering. Their images now receive their
  own corners and shadows in both preview and PNG. Light/dark GPU fixtures at
  simulated 125% and 200% compare all four attachment edges, a five-layer stack,
  two aspect ratios, transparent content, annotation pixels and history reopen.
  They also verify replaced tiles leave the GPU atlas. Solid or transparent
  backgrounds match within two channel levels and a translucent background
  within three, except at most 16 overlapping-shadow pixels within four levels.
  A local 4K moderate-shadow probe now takes 248–266 ms with one attachment and
  501–516 ms with five, retaining 38.69 MiB and 76.41 MiB of backdrop textures
  respectively, before GPU upload. The previous tiled preview retained 45.59 MiB
  and 118.35 MiB in the same probe.
  Physical DPI, interactive performance and framed or balanced combinations
  remain unverified.
- Unframed attachments ignored the inset when there was no outer padding. Each
  image now has its own inset and edge color, and annotations use the inset
  origin after the attached images are drawn. A 144×72 Electron canvas fixture
  independently confirms the layout and colors for two 60×60 images with a
  6px inset. CPU checks cover all four attachment edges and transparent source
  pixels. Light and dark GPU fixtures check zero-padding right attachments at
  simulated 125% and 200%, including transparent edge pixels, an annotation
  crossing the attachment, and a PNG reopened at 73% and 100% zoom. Blackout,
  blur and pixelation now cross a textured right attachment; the checks verify
  their effects on both images, full blackout coverage, no changes outside the
  redaction bounds, and preview/export equality. At 125%,
  selection halos are checked both inside the primary and across the attachment;
  a selected highlight retains its independently checked blended color. Tied
  edge colors now use the first sampled color, matching Electron and avoiding
  random preview/export differences. The two local NVIDIA runs each passed 191
  exact baselines without updates. Frames, balance, other inset effects, physical
  DPI and native interaction remain unverified.
- GPUI drew annotations into the primary image before attached images and their
  shadows, so a bright mark could be obscured at an edge. In the tested
  unframed, non-inset, non-balanced path, preview and export now draw annotations
  after the attachments, as Electron's rendering order does. Black, pixelated
  and blurred redactions crossing an edge are checked in both themes at
  simulated 125% and 200%, along with overlapping and clipped redactions,
  history reopen, crop, undo and texture retirement. Direct cross-shell pixel
  comparison and other wallpaper combinations remain unverified.
- A frame-only preset with no background or padding was ignored. It now activates
  the framed editor and export paths: the original 1600×1000 Polish capture is
  1600×1056 at 200%, and Polish copy matches editor export. The title bar and
  translucent image patch are checked in the preview at 125% and 200%.
- Corner- and shadow-only settings without padding or a background were also
  ignored. Without balance or inset, they now activate for single captures and
  attached images. Light/dark previews match saved pixels at simulated 125% and
  200%; Polish copy matches the editor for zero-padding corners. Transparent
  previews now use the active theme background, and a same-window theme change
  replaces and retires the old backdrop. Balanced effects and additional framed attached
  inset combinations still need visual verification.
- Framed attached screenshots previously used one title bar for the group. The
  compositor now frames each image before laying out attachments, and restores
  the balanced primary crop and sampled inset color on reopen. A CPU fixture
  matches the 119×184 layout and interior landmarks of an isolated Electron
  canvas fixture; 510 of 21,896 pixels differ by more than two channel levels,
  up to 123 at transparent frame corners. This does not represent a complete
  Electron editor workflow. An anchored wide-stroke preview patch matches the
  full composition in CPU tests, including clipping at 100%, 125% and 200%
  scale. Hairline, dashed and shadowed stroke paths retain the prior behavior.
  [Hosted CI at `808730eb`](https://github.com/Porabuild/Poratake/actions/runs/43240023837)
  passed 14 of 16 visual cases and 2,498 exact baselines. In both themes, the
  balanced and unbalanced attachment previews and reopened PNGs matched within
  one channel level at simulated 100% and 125%. The 200% case stopped before
  its preview comparison because a title-bar landmark sampled the antialiased
  frame corner; the landmark now samples its interior. Physical DPI and broader
  frame/inset combinations remain unverified.
- An unframed single-image inset was omitted at zero padding when corners or a
  shadow were enabled, and otherwise occupied the outer padding. The inset now
  surrounds the image and the preview uses the exported composition. Light and
  dark GPU checks at simulated 125% and 200% cover rounded, shadowed and combined
  effects with zero or 20px padding and transparent or solid backgrounds. Both
  themes also check edge-crossing blur and a flattened PNG reopened at 73% and
  150% zoom. A CPU test verifies the corner, shadow and 17,000px tile edges.
  An isolated native Windows editor displayed the composition and responded to
  an inset slider click; native save and reopen were not exercised in that probe.
  An Electron canvas comparison of one 170×150 rounded-shadow export found 185
  of 23,475 pixels outside the annotation area more than two channel levels
  apart, with a maximum difference of eight near rounded edges. A local 4K CPU
  composite took 151–159 ms in six samples, excluding inset-color detection,
  GPU upload and interactive behavior; slower hardware is unmeasured.
- A window frame ignored the inset around a single unbalanced capture. The
  frame, inset and annotations now share the saved composition. Light/dark GPU
  checks cover all four frame styles, zero or 20px padding, transparent or solid
  backgrounds, shadow, translucent content and clipped blur at simulated 125%
  and 200%. A solid-background PNG was reopened at 73%, 100% and 150% zoom.
  At simulated 125%, a pointer test selects and drags a framed-inset annotation;
  its handle remains visible two pixels beyond the image's left and top edges,
  and the moved result matches a reopened PNG at those three zoom levels.
  An independent Electron 170×185 frame sample originally differed by more than
  two channel levels at 786 of 28,950 pixels outside the annotation area, with
  a maximum difference of 43. Higher-precision antialiasing for the Windows
  maximize icon reduces that sample to 707 pixels and a maximum of 27; other
  controls, shadows and frame edges still differ. A reviewed Electron image
  baseline covers the maximize icon in both themes at seven scales from 100%
  to 250% and four fractional positions, with every sampled channel within two
  levels. CPU checks also cover clipping, alpha and canvas edges; transformed
  canvases retain the previous stroke path. Local light and dark Windows GPU
  runs each passed 191 exact baselines and the framed preview/export checks at
  simulated 125% and 200%. Physical DPI remains unverified.
- Fixed aspect ratios that add height placed unbalanced content at the top.
  The shared layout now centers it; a 9:16 framed and unframed inset with a
  solid background matched its reopened PNG at 73%, 100% and 150% zoom. A
  composited attachment also rounded its preview height differently from its
  PNG; its 9:16 preview now matches the reopened output at those zooms in the
  local light/dark probes. Source inspection shows Electron applies the aspect
  ratio to its editor preview but does not pass it to PNG export.
- Single unbalanced frames and shadows used separate scaled image and effect
  layers in the preview. In a light-theme, simulated 125% probe, a 9:16 frame
  without inset differed from its reopened PNG at 73% zoom by 948 pixels with a
  shadow and 947 without one, beyond two channel levels. They now share the
  saved composition and match the reopened PNG at 73%, 100% and 150% in the
  local light/dark probes. CPU tests cover horizontal and vertical 17,000px tiled
  frames, including the unshadowed case. Full-sized backing pixels remain
  necessary for blur and pixelation sampling across tiles. For a 3840×2160
  shadow with 20px padding, an alternating optimized CPU test with one warmup and
  six measured samples found median backdrop preparation times of 89ms before
  and 149ms after for an unframed image, and 92ms before and 146ms after for a
  Windows frame. The new image buffers were 32.56 and 32.98 MiB respectively;
  a subsequent row-order and opaque-pixel conversion optimization reduced median
  preparation from 142–148ms to 109–113ms in a separate alternating 4K test
  using opaque and mixed-alpha sources. The compared output buffers were byte
  identical. Source pixels, temporary allocations, GPU upload, interactive
  behavior and slower hardware are not measured by these tests.
- Balanced single captures now restore their source crop on reopen and recompute
  it after crop and undo. The crop is detected before annotations, and preview
  geometry and translucent pixels follow the saved PNG. Light/dark GPU checks at
  simulated 100%, 125% and 200% cover zero or 20px padding, transparent and solid
  backgrounds, border annotations and history reopen. An unframed inset now
  increases the canvas by its width on each side, as in Electron, instead of
  consuming the padding. Light/dark GPU checks at all three scales cover zero
  and 20px padding, transparent and solid backgrounds, translucent pixels and
  annotations; balanced cases also check the source-edge color and backdrop
  replacement after an equal-size crop. Annotation patches are composited against
  the underlying backdrop once, avoiding double blending on transparent captures.
  Attachments on all four edges without a frame,
  inset, corners or shadow are checked at those scales with padding and
  background variants, including a pixelated redaction crossing each join and
  the reopened preview. Annotations outside the right-attachment canvas are
  clipped. Complex balanced combinations remain unverified.
- Pixelated video redactions could shift a block boundary by one pixel between
  the saved composition and the local preview patch when their canvas origins
  differed. Their nearest-sample expansion now uses a position-independent
  mapping, with its boundary choice checked against an Electron canvas. A
  Recorded striped patches verify that both pixelation and blur visibly change
  the video preview and match decoded MP4 pixels across a cut and speed edits.
- Screenshot pixelation now averages fixed-size blocks like Electron instead of
  using the video renderer's downsampling. Electron canvas samples cover alpha,
  clipping and scale. Light/dark GPU checks compare saved and restored plain
  previews at simulated 100%, 125% and 200%, including translucent and clipped
  regions, 50% and 150% zoom, wallpaper toggles and a same-window DPI change.
  At 100%, an earlier shape under the redaction is checked with and without a
  gradient wallpaper for opaque and translucent images. Plain captures with an
  earlier shape also match the flattened image, including translucent source
  pixels. The redaction and its edge band now match within two channel levels
  at 50%, 73%, 137% and 150% effective zoom in both themes at simulated 100%,
  125% and 200% DPI, including clipped regions and a later shape and blackout.
  At 150%, the edge band previously differed by up to 37 channel levels for an
  opaque image and 18 for a translucent image. New blur and pixelation drafts
  over an existing filtered redaction retain their dark drag preview in single
  captures and attached-image compositions. Six earlier NVIDIA baseline
  changes and two later history-reopen changes were reviewed. A local opaque 4K block operation
  took 29–31 ms in six samples, excluding GPU upload; a redundant retained
  plain-image cache that took 128–137 ms and 31.6 MiB per redaction was removed.
  Complex compositions, physical DPI and slower hardware remain unverified.
- Committed screenshot blur now replays preceding annotations at source
  resolution and composites the backdrop once. Its radius scales with native
  image scale; light/dark GPU checks compare unframed transparent captures with
  and without inset to saved pixels at simulated 100%, 125% and 200%, including
  a clipped blur and annotations before and after it. The blur region and its
  immediate boundary now match a flattened PNG within two channel levels at
  50%, 73%, 137% and 150% effective zoom in both themes at simulated 100%,
  125% and 200% DPI, including inset, transparent padding, a gradient and
  overlapping shape, blackout and pixelation annotations. Before the change,
  an inset fixture differed by up to eight levels at 50% zoom and 65 at 150%.
  The image and annotation overlay use the same measured layout bounds. An
  aligned patch is reused across integer-device-pixel panning when its size and
  relative stage position remain unchanged. At 4K, preparing a 1920×1080 blur
  at 150% took 179–189 ms in three local samples, excluding GPU upload; cached
  lookups took about 2 μs. Large textures that exceed the existing patch budget
  fall back to the previous rendering path.
  Before the patch-local composition change, a whole-frame comparison of the
  inset fixture at simulated 100% DPI stayed within one level at those zooms
  when the blur was not clipped. The clipped variant differed outside the
  checked region, reaching 56 levels at 137% and 55 at 150%. Repeat those
  comparisons for the patch-local path; physical DPI remains unverified.
  The shared blur kernel previously used the box width as its radius. Sampled
  Electron canvas edge profiles at sigma 0.5–32 now differ by at most three
  channel levels; 56 shadow samples differ by at most four alpha levels.
  Translucent screenshot blur now composites the filtered source over the
  original temp image as Electron does, with interior and clipped samples within
  two channel levels. Narrow buffers and transparent padding have CPU checks.
  Unframed single images without balance, corners or shadows now composite the
  gradient or inset before redaction in export and patch-local preview tests.
  Twelve independent Electron canvas samples, including a clipped blur, differ
  by at most three channel levels. Light and dark local GPU workflows at 1280px
  and 100% now check annotation and draft transitions and a fractional aspect
  ratio against reopened PNG pixels. A CPU sample at one fractional origin
  matches export with either the rounded or unrounded sampling offset. Screenshot
  export and CPU preview patches now apply redactions before vectors and
  highlights, with a rounded blackout pass in vector order. Eight independent
  Electron center samples for overlapping annotations match within two levels;
  one rounded corner differs by 22 levels. Light and dark local GPU checks cover selection
  and both blackout/shape orders at 73% and 150% zoom, within two channel levels.
  Framed, balanced,
  rounded, shadowed and attached compositions, native interaction and broader
  images remain unverified. On this machine, the revised transparent kernel took
  220–235 ms for 19,200×160 pixels at sigma 104 in three local kernel-only
  samples; the screenshot path's estimated peak live buffers, including the
  canvas, are about 49 MB for that case. At 3840×2160 and sigma 20, three
  kernel-only samples took 265–280 ms for color and 86–92 ms for alpha, versus
  258–263 ms and 67–74 ms before this change. GPU upload and slower machines
  remain unmeasured.
- Plain attached captures could show a one-pixel seam at their join. Unframed
  attachments without balance or inset now share a single preview composition,
  with annotations above the images. The workflow checks joins on both sides;
  preview and export comparisons cover transparent attachments and black,
  pixelated and blurred redactions at simulated 125% and 200%. Redactions sample
  the image's transparency before the preview applies the theme background. Ten
  NVIDIA attachment baselines were reviewed; the other 372 images in those two
  workflow runs were unchanged. The [hosted Windows run](https://github.com/Porabuild/Poratake/actions/runs/43037376649)
  passed its native tests but failed the visual gate on those same ten attachment
  states. Of 2,880 Basic Render Driver images, the other 2,870 were unchanged;
  the ten replacements were reviewed against the joins and copied from the run.
- An attachment composition wider or taller than the Windows 16,384px texture
  limit could leave the preview blank. The display now uses bounded, overlapping
  source tiles with opaque theme-matted pixels; the original transparent pixels
  remain available for annotations and redactions. NVIDIA GPU checks cover
  19,200×160 and 160×19,200 compositions in both themes, including translucent
  joins, the final tile, annotations, 25% zoom scrolling and texture eviction.
  A small 2D reconstruction test checks tile intersections without allocating a
  large surface. Long annotation patches also respect the texture
  limit; very large two-dimensional GPU compositions and memory use remain to
  be measured.
- A small selection at a display edge could place its size label outside the
  overlay; near the bottom, the label could also cover the selection. The label
  now snaps inside the viewport and moves above small bottom-edge selections.
  Electron's selection label is centered without horizontal clamping in source;
  native cross-shell edge behavior has not been compared.
- At 125% and 150% simulated scaling, independently rounded dim regions left a one-pixel
  vertical strip of the desktop undimmed. The right region now shares the
  selection edge, and pixel scans check fractional selection positions.
- The GPUI frame's drop shadow darkened the whole selected region by 35%.
  A separately painted outside border preserves the outline without changing
  selected pixels; its position and width snap to the frame at fractional DPI.
- The recording handoff and window-picker scrims could leave the last display
  column or row undimmed at fractional scale. Their shared edges now snap to
  device pixels and their outer strips anchor to the viewport.
- The color picker's hardcoded card height clipped it near a display edge, and
  its marker shadow darkened the sampled pixel by 40%. Placement now uses the
  measured card and a separately painted outline preserves the source color.
  Tests cover pointer clearance at five simulated scales and click-to-copy from
  a four-color frame. If a 200×180 window cannot fit both the full card and
  its cursor gap, the card takes the least-clipped placement with the gap.
- A failed color-frame capture left the all-in-one overlay without a redraw.
  It now exits through the normal picker shutdown path. The isolated daemon
  fixture writes a partial file and fails the request; the test verifies the
  stopped-state notification, restored overlay pixels, error message and cleanup.
- All-in-one toolbar tooltips opened above the top edge and clipped their labels.
  They now open below their controls, matching Electron. Normal and 400×300
  overlays, including selected Capture text mode, check all three tooltip bounds
  and clipping masks at each matrix scale.
- Escape closed the all-in-one capture overlay while its target menu was open.
  It now dismisses the menu first and returns focus to its trigger. A second
  Escape closes the overlay; keyboard input can reopen the menu and select a target.
- Selecting a target left keyboard focus on the disappearing menu, and clicking
  a menu item could start a full-screen capture behind it. Selection now returns
  focus to the trigger after the activation key is released; pointer selections
  return it immediately. The fixture exercises repeated Enter/Space keydowns,
  pointer and outside presses, and selection without an underlying capture. A
  window-deactivation test clears state if a key release is lost.
- A history grid at simulated 175% and 456 physical pixels wrapped its second
  card onto a new row. The cards now fit the snapped available width. The unit
  test sweeps every physical width corresponding to 140–400 logical pixels at
  100%, 125%, 150%, 175% and 200%, and verifies the scale after each resize.
- At 140px the two-column history cards clipped their actions, and the list
  clipped its labels. The grid now uses one column with room for the selection
  ring; the list stacks its thumbnail and details. Keyboard navigation follows
  the displayed grid, and a visible selection stays in view across resize,
  scale and layout changes without pulling back a selection the user scrolled
  away from. The visual fixture checks all four sides of the selection ring.
- History could open partly outside a display when its tray icon was near an
  edge. GPUI now fits the popover to the work area and converts macOS
  secondary-display bounds as the tray menu does. Geometry tests cover offset
  displays, 140–400px window widths, both themes, simulated 100/125/150/175/200% scaling,
  grid and list selection, and wrapped header and filter controls. Narrow list
  actions now sit over the thumbnail so labels remain readable, with feature
  icons wrapping as needed. Grid tooltips align inward according to their column.
  The local visual fixture checks complete reveal and delete tooltips in grid
  and list states, and deletes an isolated item without opening an editor. Its
  144 NVIDIA and 144 hosted history images were reviewed; physical monitors
  remain unverified. Electron's horizontal placement remains unclamped.
- Failed editor-attach captures also left a partially written temporary PNG.
  The attachment path now removes it before restoring the editor.
- A competing capture or second attachment could replace the active attachment
  selector and leave its editor hidden. Those requests now leave the attachment
  in place, and a selector that cannot open clears the pending operation and
  reactivates its editor. The fixture covers competing intents and unavailable
  displays with freeze on and off.
- Choosing an attachment edge could also begin an annotation on the editor
  canvas. Edge presses now stop at the overlay; both mouse buttons work, and
  the edge and camera icon highlight on hover as in Electron. The workflow
  fixture presses the actual strips and rejects an unintended draft. The four
  edges now have names, keyboard activation and in-bounds focus borders. Windows
  accessibility inspection exposed all four buttons; invoking the right edge
  opened the selector, and Escape returned to the isolated test editor.
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
- OS close requests bypassed the pin's hover Close handler and never restored
  the editor. Both close paths now consume the same in-memory state once.
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
- Pre-recording audio and camera choices now use Electron's global or
  all-in-one preference scope. The fixture exercises ordinary controls,
  remembered and temporary all-in-one choices, live recording changes, and
  area, window and screen handoffs. Closing the bar also hides its camera
  preview; physical media-device verification remains in the backlog.
- The all-in-one toolbar's full-width positioning container intercepted clicks
  outside its visible controls. The fixture now starts a recording selection
  beside the toolbar and verifies that a toolbar click does not start a box.
- Keyboard sounds enabled at zero volume did not become audible in preview when
  the volume increased. Audio rebuilds could also replace an undone timeline with
  a stale result. Both transitions have regression coverage.
- Speed edits weakened the original tone in preview and export. Constrained
  overlap alignment preserves the tested pitches while limiting source drift;
  regressions cover short sounds, including at silence and input boundaries.
- Cut All split only the first overlapping drawing and audio clip. It now splits
  each eligible clip, matching Electron and preserving the audio source trims.
- Shared overlay buttons and recording device triggers used white foregrounds
  on light toolbars in both shells. Foreground and hover colors now follow the
  active theme. Electron light/dark computed styles were checked; browser
  screenshot and hover-state evidence remain unavailable in this environment.
- The recording bar and capture-preview controls now use native hover tooltips
  on Windows and macOS to let their labels extend beyond the small window.
  An isolated Windows tooltip painted readable text outside a 52px owner window;
  focused Windows headless tests exercise GPUI pointer hit-testing, the 150 ms
  presenter request, leave cancellation and keyboard dismissal. A hosted
  real-app probe displayed and measured the Windows recording tooltip, then
  verified dismissal and re-entry after the Windows backend began dispatching
  pointer-exit input. macOS tooltip appearance still needs native verification.
- A keyboard-opened all-in-one target menu could leave its trigger pressed after
  focus moved. The shared keyboard release now runs in the capture phase; the
  vendored control test reproduces the focus transfer and stopped propagation.
- The missing-thumbnail video placeholder now fades when its controls appear,
  keeping the film icon out of the Copy and Edit controls. Idle and hover
  pixel checks reached the visual baseline gate across both Windows renderers.
- Windows change-driven capture wrote every frame with a fixed short duration,
  truncating static takes. Pending samples now receive their real duration before
  submission; finalization and pause accounting use the capture clock.

## Remaining bugs and verification backlog

Entries distinguish observed defects from missing evidence. An unverified path
is not a demonstrated product defect; reproduce before changing its behavior.

In an isolated native Windows editor, the inset slider responded to a pointer
click. Two control-tool set-value attempts left its value unchanged, but that
tool documents set-value for text fields, not numeric range controls. The
Windows adapter maps a range-value request to the numeric action handled by the
slider; delivery through the operating system remains unverified.

Unframed layered annotations now render after attachments in the tested path,
including the inset rectangle and cross-boundary redactions above. Verify their
ordering where annotations overlap, and with frames and balance; compare the
complete workflow directly with Electron.
The 4K attachment preparation cost above also needs interactive and
slower-hardware measurement.

Selected highlights and committed redactions now draw their outlines above all
annotations, so later marks cannot cover them. This selection order deliberately
differs from Electron; saved pixels and highlight blending remain unchanged.
Light and dark fixtures check all four sides of blackout, blur and pixelate
outlines, an overlapping highlight, and plain and inset-attached images at
simulated 125% DPI. Rectangles now use an outside-only outline above later marks
so their stroke and fill remain visible. Light and dark fixtures check overlapping
thin and thick rectangles, all four sides, sampled stroke and fill pixels, export,
and plain and inset-attached images at simulated 125% DPI. Circles use the same
outside-only order; the fixtures check all four sides, the interior, thick stroke,
and unchanged export for thin and thick circles. At fractional zoom, antialiasing
can blend the ring and a thin stroke into the same device pixel. Verify selection
outlines for other overlapping annotation types and on physical displays.

An isolated Electron canvas probe applied its current redaction operations to the
same unredacted inset-attachment PNG used by GPUI. Across 4,136 interior pixels
per effect at simulated 125% and 200%, blackout and pixelation matched exactly.
Blur differed by up to five and four channel levels respectively; 487 and 602
pixels differed by more than two levels. This checks the effect functions, not
the complete Electron editor workflow.

A separate 1× offscreen probe compared Electron's video blur operations with
the GPUI blur function on a manually composed translucent checker and wallpaper.
Across 1,088 interior pixels, the mean RGBA-channel difference was 0.480 and
the maximum was three; across 64 pixels at a flat clip edge, the mean was 0.473
and the maximum was two. Pixels just outside the annotation matched. This does
not exercise rounded-edge antialiasing, playback or the full export pipeline.

A CPU regression compares complete GPUI preview and export pixmaps for two
fractional annotation origins, both rectangle/blackout input orders and five
coordinate scales. A separate Chrome 157 proxy matched the saved Electron corner
sample at 1×, but 27 of 120 corner samples differed from GPUI by more than two channel
levels, up to 27. This proxy does not establish parity with Poratake's pinned
Electron version at the other positions and scales. An offscreen fixture using
the pinned Electron 43.4.0 reproduced the 1× corner sample and the tested export
order; it does not exercise the complete editor workflow.

A 1024px-wide Windows/Linux scroll fixture stops after the first frame when a
21×16px footer block changes. A wider structural footer candidate recovered that
case but stopped a valid periodic page early, so it was not adopted. Rounding the
2% tolerance up recovered the fixture but introduced 33 early stops in the
5,412-case periodic corpus. A narrower contiguous-band fallback avoided those
stops but misclassified a no-footer page and shifted an animated block by 16px,
changing 672 pixels relative to the baseline stitch. Neither candidate was applied.
A three-frame uniqueness prototype also changed 672 pixels in that no-footer case.
Separate periodic-content models with 16px and 24px scroll steps produced identical
three-frame pixels but different expected heights. Additional frames alone cannot
resolve every sequence; a safe lookahead would require lifecycle changes and must
retain the accepted prefix when evidence remains ambiguous.

| Priority | Remaining work                                           | Acceptance evidence                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| -------- | -------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| P1       | Native compact-window tooltips                           | A hosted real-app probe verified a Windows recording tooltip's text, outside-window placement, dismissal, re-entry and exclusion from a WGC screenshot and three decoded full-monitor recording frames, with painted-foreground, backdrop and known-visible controls. Verify other capture targets, macOS appearance and hover exit, and physical mixed-DPI placement. The macOS tooltip is registered on the whole native view and cleared on hover exit.                                                                                                          |
| P2       | Linux recording tooltips                                 | [Hosted Linux CI](https://github.com/Porabuild/Poratake/actions/runs/43362521397/job/131679275651) passed hover and keyboard-focus tooltip geometry tests, including unbroken and Unicode titles at simulated 100% and 200%. Very long titles remain shortened. Verify real Linux, macOS focus and native screen-reader behavior. A separate popup cannot yet be verified as excluded from Linux recordings.                                                                                                                                                        |
| P1       | Linux recording control capture exclusion                | An outside-window tooltip needs click-through behavior and capture exclusion so it neither intercepts toolbar input nor appears in recordings. X11 screen/area capture reads the root drawable; Wayland recording is currently disabled and its recorder requests no window exclusions. A private Xvfb probe captured a synthetic override-redirect popup while mapped and the underlying pixel after unmapping; it did not exercise Poratake's actual GPUI toolbar. Verify that toolbar on supported X11 compositors and Wayland.                                  |
| P1       | Linux window recording                                   | Hosted Xvfb raw-frame tests cover movement, resize, occlusion, remapping, cursor tracking and closure; fixtures also cover reparenting, missing Composite and XI2.1 grabs. A private Xvfb run decoded 93 H.264 frames. Blocked encoders report STOP_FAILED; permanently stuck capture workers keep retries reserved. Verify real window managers, outlines and GPUI handoffs. Wayland window recording is unsupported. XI2.0 can miss input during grabs, and rapid events can lag pointer motion. Electron's Linux capability helper disables recording.           |
| P1       | Wayland screen and area recording                        | GPUI recording is currently disabled on Wayland: static tracing shows the recorder passes zero dimensions from zed-scap to FFmpeg and ignores the selected area. The portal can return a window, but the capturer does not expose its source type, position or size, so first-frame sizing alone cannot establish a correct area crop. Implement and verify source-aware capture on a real Wayland session before re-enabling it.                                                                                                                                   |
| P1       | Native window/display highlighting and recording handoff | Simulated candidate highlights and a native live/frozen window screenshot pick are verified. Native overlay screenshots timed out. Verify physical pointer transitions on each display and no stale dim or duplicate outline when a recorded window moves/resizes/closes.                                                                                                                                                                                                                                                                                           |
| P1       | Physical Windows DPI and monitor transitions             | Run both shells at OS 100/125/150/200%, mixed-DPI and negative-origin displays; verify cursor, crop pixels, bounds, caption hit targets and saved output.                                                                                                                                                                                                                                                                                                                                                                                                           |
| P1       | Preview display preference identity                      | Windows IDs match Electron 43 on the current monitor; simulated removal repositions the preview and retains the selected ID or falls back to primary. Arithmetic checks cover 125%/150% and negative origins. Unmarked legacy GPUI indexes may require reselection. Verify native multi-monitor transitions and restarts, and cross-shell IDs on macOS/Linux.                                                                                                                                                                                                       |
| P1       | Complex framed screenshot preview versus export          | Single unbalanced frames with inset match the PNG in the tested light/dark cases at simulated 125% and 200%; one solid-background case also matches a reopened PNG at 73%, 100% and 150%. A 9:16 frame without inset, with or without shadow, matches a reopened PNG at 73%, 100% and 150% zoom in the local light/dark probes. At simulated 125%, pointer selection and dragging with a framed inset and an edge handle are checked. Verify more styles and zooms, transparent reopen, balance, attached layers and physical DPI transitions.                      |
| P1       | Complex rounded screenshot preview versus export         | Single unframed captures with and without inset or shadow match the PNG in the tested light/dark cases at simulated 125% and 200%, including transparency and annotations. Verify frames, balance, attachments, more zoom levels and physical DPI.                                                                                                                                                                                                                                                                                                                  |
| P1       | Zero-padding wallpaper effects                           | Frame-only, corner/shadow-only and single unbalanced framed or unframed inset effects activate at zero padding. Unframed attached insets are checked on all edges by CPU and on the right edge by GPU. Balanced unframed right attachments with inset match preview/export in CPU tests at 100% and 125%. Verify GPU behavior, balanced corners and shadows, framed attachments and more inset combinations.                                                                                                                                                        |
| P1       | Wallpaper shadows and complex image compositions         | Single unframed captures with or without inset match saved output in the tested shadow cases. After a pixel-conversion optimization, a local 4K shadow composite took a median 109–113 ms versus 142–148 ms with the preceding conversion; GPU upload, interactive behavior and slower hardware remain unmeasured. At 17,000px, rounded inset tiles, plain or framed shadows and unshadowed frames have CPU checks; real GPUI GPU previews and other combinations remain unverified.                                                                                |
| P1       | Complex screenshot pixelation                            | Fixed-size averaging matches Electron canvas samples. The tested plain redaction and boundary match saved pixels at 50%, 73%, 137% and 150% effective zoom in both themes at simulated 100%, 125% and 200% DPI, including transparency, clipping and later overlapping annotations. Verify frames, inset, balance, attached layers, physical DPI, large patches and interactive performance.                                                                                                                                                                        |
| P1       | Screenshot blur preview and Electron parity              | Earlier, blur and nearby annotations matched flattened PNG pixels within two levels at 50%, 73%, 137% and 150% zoom for unframed light/dark cases and simulated 100%, 125% and 200% DPI. Whole-frame differences remain outside that region, and oversized patches use the old path. Simple unframed gradient and inset samples now filter the composed backdrop; new annotation order passed local GPU checks. Verify broader fractional offsets in GPUI, frames, balance, corners, shadows, attachments, physical DPI, large patches and interactive performance. |
| P1       | Screenshot annotation order                              | GPUI CPU preview/export match within two levels at tested fractional origins and scales, but the rounded blackout corner differs from the pinned Electron sample: GPUI `[191, 0, 191]`, Electron `[169, 0, 169]`. The current corner assertion expects the Electron value with a ±24 per-channel allowance, so it accepts that mismatch. Verify live GPUI pixels, other corners, complex compositions and physical DPI. Electron screenshot preview paints blackout in export order; DOM tests cover both orders and selection.                                     |
| P1       | Full capture-service matrix                              | Fixtures cover window/area/display, timer, OCR/QR and scroll lifecycles. Native area/timer output and OCR/QR service probes are verified. Native pin movement/OS-close and one controlled attachment/save are verified. Extend native display/OCR/QR/scroll, real stitching, clipboard and remaining attachment cases; compare output with Electron.                                                                                                                                                                                                                |
| P1       | Frozen all-in-one desktop icons                          | Capture and recording unit checks pass; hosted fake-daemon workflows reach the visual baseline gate for hide-before-freeze and recording restoration. Verify native icon timing, mode switches and recording handoffs on real displays.                                                                                                                                                                                                                                                                                                                             |
| P1       | Native pending-freeze handoff                            | Mock-platform tests cover delayed selector replacement, cancellation, successful freeze ownership and failed-release retry. Verify the same transitions with real macOS and Linux daemons and multiple displays.                                                                                                                                                                                                                                                                                                                                                    |
| P1       | Native screen-recording selector handoff                 | Mock-platform tests cover replacing an existing or pending selector, release failure and retry, and a newer request that supersedes recording. Verify real single-display recording handoffs and the corresponding multi-display picker behavior.                                                                                                                                                                                                                                                                                                                   |
| P1       | Capture filename collisions                              | GPUI and Electron screenshot, scroll and clipboard-to-editor captures now use exclusive filenames. Real-filesystem tests cover concurrent saves, cancellation, forced unsupported-hard-link fallback copying, timeout recovery and history/editor handoff. Verify simultaneous captures across both shells and on filesystems without hard links.                                                                                                                                                                                                                   |
| P1       | Linux daemon shutdown after printing                     | A scaled GTK X11 test failed while waiting for daemon exit with the print dialog open; the next run passed. An earlier X11 exit failure also passed on retry. Capture the exit status if it recurs and identify the cause.                                                                                                                                                                                                                                                                                                                                          |
| P1       | Scroll stitching with fixed footers                      | Windows/Linux PNGs retain one footer in 20 light/dark three-frame fixtures and synthetic changes to footer size and appearance. A 2% per-row tolerance retains tested small animated footer blocks at three widths, while unrelated-frame controls are rejected. Some animations and footer changes still stop early or produce incorrect output in synthetic multi-frame cases. The separate macOS matcher passed hosted synthetic fixed-footer, changing-footer and ambiguity fixtures. Verify real surfaces, weak overlaps and fixed headers.                    |
| P1       | Scroll stitching with repeated content                   | Windows/Linux retain the accepted prefix for a tested periodic false seam at 30% and 90% expected overlap; a noisy unique seam with variable movement still stitches in the tested cases. Earlier probes found 1,310 incorrect periodic cases rejected and 2,795 early stops, and 164 early stops in 360 translated-UI cases; those counts predate the new ambiguity guard. Quantify early stops and correctness on broader patterns, animations and real pages; macOS uses a separate stitcher.                                                                    |
| P1       | Unresponsive scroll stitch                               | Cancellation now cooperatively stops the stitch writer, and the shells wait for the finish and cancellation responses before discarding their temporary output. Windows and Linux worker tests cover joining and retryable startup failures; hosted macOS fixtures and GPUI parity cases also ran. A noninterruptible file write can still delay cancellation, and daemon shutdown can interrupt an in-flight stitch. Verify native UI handoffs and graceful shutdown on all three platforms.                                                                       |
| P1       | macOS scroll preview with variable overlap               | The preview shares measured overlap with the saved stitch and coalesces background rendering. Hosted native fixtures compare variable-overlap pixels and cancellation; verify real scrolling surfaces.                                                                                                                                                                                                                                                                                                                                                              |
| P1       | Native Windows clipboard formats                         | GPUI supplies CF_DIB alongside PNG; byte-level tests cover the bitmap and its headers. An isolated hosted Windows test verifies native format availability and PNG-only/DIB-only reads. Verify interactive app copy and paste into other applications, compare Electron, and profile large captures on slower hardware.                                                                                                                                                                                                                                             |
| P1       | Native attachment compositor coverage                    | The frozen handoff fix passed a real pixel-exact native capture and save. Add automated native Windows coverage for hide/show transitions and verify freezing off, cancellation and repeated attachments.                                                                                                                                                                                                                                                                                                                                                           |
| P1       | Cached Windows screenshot resources                      | Cached composition streams with 4 MiB scratch bands and rejects requests over 512 Mi pixels. Tests retain offscreen padding, gaps, alpha and the empty-cache fallback. Window alpha-mask capture and WIC internals can allocate separately; profile large native captures, encoding latency and slower hardware.                                                                                                                                                                                                                                                    |
| P2       | QR photo robustness and latency                          | The native decoder matches 65/65 original corpus cases and 493/520 brightness/contrast variants without losing a prior match. The attributed photo fixture is also tested at 10°, 25° and 45° rotations, with perspective distortion and a combined 15° rotation plus perspective warp. Verify additional real photographs and interactive app clipboard delivery; further reduce low-contrast 4K no-code latency and test slower hardware across both desktop versions.                                                                                            |
| P1       | Recording media and target continuity                    | Native mic/system/camera enable/disable and device changes, target close/move/resize, audio continuity and camera visible ranges. Control-bar fixtures and image baselines cover UI/RPC transitions; hardware output still needs proof.                                                                                                                                                                                                                                                                                                                             |
| P1       | Real-media audio speed edits                             | Synthetic checks now verify source pitch and short sounds in preview and decoded export, plus stereo tones and boundary transients across speeds. Verify live microphone, speech and music against Electron; non-x86 alignment performance is unmeasured.                                                                                                                                                                                                                                                                                                           |
| P1       | Extended video interactions and export matrix            | Preview/export checks cover zoom, camera, cursor, shortcuts, subtitles, blur at the composition edge, and some audio across cuts and speeds. Verify mic/music and live playback; expand interactions, styles and combined edits. Compare video blur with Electron near translucent or clipped source edges. Representative coverage only.                                                                                                                                                                                                                           |
| P1       | Native accessibility action delivery                     | An isolated GPUI platform verifies Select Expand/Collapse and Slider SetValue/Increment/Decrement dispatch after fixing their HeroGPUI handlers. Verify the operating system accessibility adapters and screen readers on each target platform; the isolated platform does not exercise them.                                                                                                                                                                                                                                                                       |
| P2       | History visual and native verification                   | Verify native reveal, tooltip interaction and multi-monitor placement at high DPI. Mock geometry tests cover first-row grid and compact-list tooltips as well as widths down to 140px; smaller layouts and other item states need verification.                                                                                                                                                                                                                                                                                                                     |
| P2       | Attachment edge accessibility                            | Named controls, keyboard activation and in-bounds focus borders are tested. Native Windows accessibility invocation is verified for the right edge; extend physical keyboard and screen-reader checks to every edge and target platform.                                                                                                                                                                                                                                                                                                                            |
| P2       | Complete state coverage beyond representative controls   | Add default/hover/pressed/focused/open cases for each shared control and overlay, with focused content remaining visible in narrow windows.                                                                                                                                                                                                                                                                                                                                                                                                                         |
| P2       | macOS and Linux runtime comparison                       | Re-run the same applicable workflows on actual target platforms; Windows evidence does not certify them.                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| P2       | Other app surfaces                                       | Settings, history and onboarding need refreshed runtime comparison and their own image baselines. Tray, pin and capture-preview representative states now have automated coverage; complete native interaction, preview docking and hover coverage remain outstanding.                                                                                                                                                                                                                                                                                              |

A zero-padding framed attachment also matches CPU preview and export over a
rectangle and blackout crossing the primary-image seam. A positive control checks
the attachment pixel before the blackout is drawn.

[Hosted Windows CI](https://github.com/Porabuild/Poratake/actions/runs/43387203877)
verified the native capture-preview tooltip's text, window state and paint, then
compared every pixel in its screenshot region while the tooltip was hidden and
shown. That paint check does not independently prove its monitor-composited pixels.

macOS scroll-overlay capture exclusion remains unverified. The preview can overlap
the selected area when there is no room beside it, and the retained Electron
selection overlay also draws a border and tint inside that area. The daemon captures
on-screen windows. The daemon's native boundary and control windows use
`NSWindowSharingNone`, a legacy value that [Apple does not recommend relying on to omit captured content](https://developer.apple.com/documentation/appkit/nswindow/sharingtype-swift.enum/none).
Verify saved pixels against a real selected area, including a full-width area,
before claiming that the overlay is absent from capture output. This is separate
from the ScreenCaptureKit recording-window exclusions below.

On Linux the recording-control and capture-preview tooltips are drawn inside
their GPUI windows. X11 screen and area recording reads the root drawable with
no window exclusions, so a tooltip overlapping the recorded area can appear in
the output. Native GPUI hover and recorded pixels still need verification.

Windows tooltip capture protection falls back to `WDA_MONITOR` if full exclusion
fails; that mode can leave a black rectangle in a recording. If both affinity
methods fail, the tooltip is suppressed and the error is logged. Verify native
hover delivery, click-through, and recorded pixels on supported Windows displays.

The macOS GPUI recording path requests exclusion of its recording control and
registers daemon-owned camera and overlay windows with its ScreenCaptureKit filter. New
synthetic fixtures cover filter updates and stop coordination and passed in
[hosted CI at `5359f58f`](https://github.com/Porabuild/Poratake/actions/runs/43307382869).
Verify a real recording, including whether
staged transparent windows appear in `SCShareableContent` before they are shown.
Electron does not pass its control or retained selector-window IDs to that filter.
Its content-protection setting does not prevent capture by newer ScreenCaptureKit
applications, according to [Electron's documentation](https://www.electronjs.org/docs/latest/api/browser-window#winsetcontentprotectionenable).
Verify the hidden control's exclusion across restart before changing this handoff.
Electron's `getMediaSourceId()` exposes the macOS `CGWindowID`; the existing
overlay native-handle helper returns an `NSView` pointer instead. The Swift
recorder initially queries only on-screen windows, while Electron hides its
control before restart. Passing that hidden control's ID directly can therefore
make the exclusion resolver reject the restart.

[The hosted macOS fixture](https://github.com/Porabuild/Poratake/actions/runs/43399120426)
resolved a test-owned hidden window, but trapped when starting its filtered stream.
The debugger rerun did not have Screen Recording access and skipped the native
check, so it produced no backtrace. This does not establish an app failure or
verify captured pixels; an already-authorized interactive Mac is still needed.
The native check now skips by default. On such a Mac, run the fixture compiled
by the workflow with `--screen-capture-exclusion` to opt in without a permission
request.

A full-frame comparison alone was rejected as a replacement for the sampled
duplicate hash: a blinking pixel on a stationary page was accepted as scrolling.
The Windows/Linux footer path requires an exact overlap and either movement across
a majority of matching rows or a row appearing at its translated position and
disappearing from its old one. After detecting a footer, it can also accept one
changed pixel on the last overlapping row. In 1,236 generated cases, all accepted
frames produced the expected rows and 291 cases stopped early. Ambiguous or
animated content can still require different evidence. macOS uses a separate
duplicate check and stitcher.

When a sampled hash changes because one pixel blinks on a stationary page,
Windows and Linux now treat the frame as repeated if neither footer detection nor
an exact full-row overlap identifies scrolling. The accepted frames and saved PNG
remain unchanged through the three-frame duplicate limit. An edge pixel leaving
a genuinely scrolled viewport still stitches with both an exact and a biased
expected step. In 5,184 generated stationary-animation variants, erroneous frame
additions fell from 374 to 132; no previous stop became an addition. A 4K local
probe measured 9.86 ms before and 11.29 ms after for the one-pixel case, and
2.47 ms versus 2.61 ms for its translated control. Multi-pixel animations, real
pages, native capture time and slower hardware remain unverified.

The footer matcher now tries exact stationary rows first, then allows sparse
changes above an exact stationary suffix while retaining the full-row overlap and
movement checks. A three-frame test stitches a footer with a 16×16 animation;
the saved PNG reopens pixel-identically, and a sparse-content control retains the
exact-match path. In 240 translated UI cases with small animated blocks, eight
structurally incorrect outputs and 16 early stops became correct; the remaining
accepted-output differences were confined to the animated blocks.

The footer matcher now also finds an exact suffix around a localized animation in
the scrolling content, after trying exact matches first. A three-frame test and a
reopened PNG match every expected pixel. In 2,460 synthetic four-frame cases,
1,886 completed correctly, 471 produced incorrect pixels or dimensions, and 103
stopped early; before this fallback the counts were 1,479, 739 and 242. In 2,400
translated UI variants, 532 earlier stops became correct; 160 incorrect results
and 292 stops remain. A 5,412-case periodic-content matrix preserves all 2,084
previously correct outputs, with 1,310 incorrect results and 1,485 early stops
after the change. No previously correct case regressed in these matrices.
Animation pixels were excluded from those comparisons, and real pages remain
unverified. An exhaustive 1,172,808-case small-row check covers the matcher's
shorter-input boundary. In two local 4K repeated-content probes, median submit
times increased from 7.25 to 9.78 ms and from 9.45 to 11.01 ms; native capture
and slower hardware remain unmeasured.

After footer arbitration, Windows and Linux reject a sampled-perfect overlap
when no exact alternative exists and more than 5% of its full RGB pixels
disagree. A targeted animated-footer fixture retains the prior frame; recognized
footers, exact alternatives and small localized changes remain accepted. In
5,412 synthetic periodic cases, the results changed from 2,617 correct, 1,310
incorrect and 1,485 early stops to 2,617 correct, two incorrect and 2,793 early
stops. No correct case regressed. The 1,236-case footer corpus and translated-UI
corpus were unchanged. A local sampled-perfect, non-exact overlap benchmark
measured an additional 0.25 ms at 1080p and 0.79 ms at 4K, excluding native
capture. Large moving content may stop early, and sparse aliases and slower
hardware remain unverified.

The two remaining incorrect periodic cases were caused by a repeated body being
mistaken for a large fixed footer. A guard now stops those captures before
appending corrupt pixels: the same 5,412-case matrix has 2,617 correct outputs,
no incorrect outputs and 2,795 early stops. The footer and translated-UI probes
showed no new incorrect results. On one synthetic 4K input with one matching
offset and early mismatches at most others, the short-circuit guard took a median
0.035 ms versus 12.326 ms for the equivalent full-score scan. A repetitive 4K
input with mismatches at the last sampled pixel took 5.745 ms versus 11.032 ms.
These isolated timings exclude native capture and slower hardware and do not
bound every input.

In a synthetic two-frame scroll, period-8 main content shifted by 96px with an
independently moving edge produces the same pixels as a complete 104px shift.
Both have a unique 56px full-row match, although the first main-content overlap is
64px. When the expected sampled strip matches exactly and conflicts with a
different alignment supported by full-row evidence, Windows and Linux now stop
and retain the previous frames. The guard also checks three regions when the
expected alignment differs only in up to two sampled columns. Where the overlap
is long enough, those columns must recur in the first and last non-overlapping
regions; shorter conflicts stop because they cannot supply that evidence. Tests
cover sampled and unsampled edge pixels, with and without a fixed footer; the
retained PNG reopens pixel-identical after one or two accepted frames. A local
repeated strip or isolated changes in different columns of the first and middle
regions do not trigger the new guard.
The 1,236-case footer matrix still had no incorrect accepted output and 291 early
stops. A synthetic 4K exact-overlap probe had median times of 3.42 ms before and
4.63 ms after over 30 samples on this machine for the earlier guard. A separate
30-sample 4K probe measured 1.56 ms before and 1.74 ms after this extension for an
exact overlap, and 1.43 ms versus 1.64 ms with a single animated pixel.
A new guard stops when a perfect sampled match disagrees with repeated exact
full-row matches and the selected rows differ in multiple rows and columns. A
synthetic conflict now preserves the first frame, while the correctly aligned pair
still stitches with a one-pixel or 16-pixel single-row or single-column animation;
all saved PNGs reopen pixel-identical. A true conflict in one row or column can
still be mistaken for animation, and a multi-row, multi-column animation may stop
early. A 360-case probe translated four captured UI baselines
with two footer settings, five shifts and nine expected-offset values. The guard
changed 16 incorrect outputs into early stops: 196 correct, 12 incorrect and 152
early stops, versus 196, 28 and 136 before. Extending the conflict check to unique
full-row matches changed the remaining 12 incorrect results into early stops:
196 correct, zero incorrect and 164 stops. All 40 cases with the exact expected
offset remained correct. A valid translation can stop if its expected step is
wrong and the sampled strip is repetitive. A synthetic case with three
independently moving sampled columns still appends an ambiguous frame.
The same input frames also represent a valid 104px translation: the current
matcher selects its correct 40px overlap, while a tested three-column guard
stops. For a 96px main translation with independently moving columns, those
identical frames instead require a 48px overlap. The guard was not adopted;
the input alone cannot distinguish these cases. It left the 360-case translated
UI probe unchanged and had no incorrect output in the 1,236-case footer probe.
Injecting one changed pixel or a 16-pixel horizontal or vertical animation into
each of those 40 exact-offset cases produced 24 correct outputs and 16 early stops,
with no incorrect outputs. All 16 stops had a fixed footer and also occurred before
this guard. The newer localized fallback reduces the stops to four for a single
pixel and eight for a vertical 16-pixel animation; all 16 horizontal cases still
stop. An earlier approximate fallback resumed those cases but changed 13 pixels in
two vertical-animation outputs because its detected footer included static body
rows. That fallback was not adopted; the fixed-footer matcher still needs broader
animation tolerance.
An older near-edge animation failure no longer reproduces: eight light/dark,
image and scroll-distance cases now match all expected pixels with the current
localized-footer fallback. This is synthetic evidence, not a real-page test.
For a synthetic 4K repeated-row overlap with one animated pixel, ten local
post-warmup samples had medians of 3.13 ms before and 4.12 ms after this guard;
native capture and slower machines were not measured.
Isolated changes can also be indistinguishable from a different full-frame
translation: changes in one column at both outer regions can stop early, while
the prior overlap choice remains in effect for the tested changes in two regions.
Other repeated patterns and real scrolling surfaces remain unverified.

Windows and Linux daemons report overlap, capture and input errors when automatic
scrolling stops. GPUI and Electron now notify the user while retaining the previous
frames for saving; normal end-of-content and height-limit stops remain quiet.
Electron event tests cover both native and overlay-based paths and save the capture
after interruption. A GPU-free GPUI unit test checks the event mapping. A GPUI
workflow fixture checks the same sequence and passed in the hosted run at
`27798e87`; native notification presentation still needs verification.

Keep known platform differences explicit: preview/zoom use a baked blur because
GPUI has no live backdrop filter; the capture eyedropper remains an app overlay;
macOS traffic-light space and Windows custom captions are platform-specific.
These are not evidence that unrelated features match.

## Review gate

| Review category                       | Result                                                                                                                                                                                                                                                                                                |
| ------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Scope reviewed                        | Capture/editor/recording behavior, shared themes, visual/workflow fixtures and dependency evidence. The initial review used `ac384ae`; later reviews covered tooltip client geometry, Electron frozen-frame ownership and hide failure handling, along with the targeted workflows listed above.      |
| Fixed                                 | The defects listed above, including click-position window picking, frozen selection lifecycle, failed timer cleanup, native selector client alignment and full-window QR detection; workflow checks verify capture-before-release and exact output.                                                   |
| Rejected after validation             | A proposed ZXing license truncation was rejected: raw bytes from the exact pinned upstream revision match the bundled file, including its upstream appendix (SHA-256 `3f62881f0566227a24b12e5a754cc79f39aaa94883038e95c94812e1f50af42f`).                                                             |
| Deferred major or dangerous refactors | None.                                                                                                                                                                                                                                                                                                 |
| Verification                          | Local checks, exact visual comparison, native workflows and image review are recorded above.                                                                                                                                                                                                          |
| Blocked or unverified                 | All active CI jobs passed at [`258af434`](https://github.com/Porabuild/Poratake/actions/runs/43449868862); Windows packaging was skipped. Pull request checks show the current head's status. Physical mixed-DPI, remaining native media/capture workflows and other platforms remain in the backlog. |

An earlier local Windows GPUI suite passed serially with 948 tests and one ignored. One
earlier parallel run exited with access violation 0xc0000005 in ntdll.dll while
three tests were unfinished. Those tests passed individually, and the video-
export group passed with four test threads. An earlier revision also recorded
the cancellation-and-retry test failing alone with the same exception. The
parallel run's unsymbolized crash dump contains stack-memory addresses from
Media Foundation and NVIDIA's H.264 transform; this is a lead, not an unwound
stack or a confirmed cause. The intermittent failure remains unverified.
