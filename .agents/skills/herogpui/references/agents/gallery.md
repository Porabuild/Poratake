# Gallery and visual verification

Read this guide before changing `gallery`, capturing screenshots, driving a
component, or running route smoke. Gallery pages render lazily, so compilation
does not prove that a route draws without a GPUI panic.

## Build and smoke

After a component or gallery change, use:

```powershell
.shots/rebuild.ps1
.shots/smoke.ps1
```

Run the scripts in the current PowerShell session, not through
`powershell -File`. The rebuild script moves aside a recently used executable
before Cargo writes a new one; plain `cargo build` can fail with `Access is
denied` after a capture and leave the next screenshot using the old binary.

The gallery scripts expect
`E:\work\HeroGPUI\target\debug\herogpui-gallery.exe`. `rebuild.ps1` resolves the
image Cargo writes through `CARGO_TARGET_DIR` and re-points that launcher at
the fresh build, failing loudly when what it serves is not the image just
built; a bare `cargo build` leaves the launcher stale, so rebuild before
diagnosing a stale render.

`smoke.ps1` uses one off-screen process and the `HEROGPUI_CONTROL` protocol to
walk every current route, then retries a suspected crash alone. Trust the
script's current route list and output rather than a copied page count. Both
shared and isolated runs require an acknowledgement of a rendered frame;
remaining alive for a fixed delay is not a passing smoke result.

When maintaining a driver, keep the GPUI window off-screen rather than
minimized; a minimized window may never render. The gallery is a
console-subsystem executable, so launch it with `CreateNoWindow = $true` by
constructing a `Process` and calling `Start()`. `Process::Start(psi)` can return
null in PowerShell here, while `Start-Process -WindowStyle Hidden` also hides the
GPUI window and leaves nothing to capture.

## Choose the least intrusive driver

Use `.shots/drive.ps1` for most interaction checks. It posts input to an
off-screen, unfocused gallery window and captures with `PrintWindow`, so it does
not take over the user's desktop.

```powershell
python .shots/sections.py Table
.shots/drive.ps1 -Page Table -Section Sorting `
  -Do "click:353,387 key:enter"
```

Supported steps include `click:X,Y`, `dblclick:X,Y`, `drag:X,Y>X,Y`,
`key:tab`, `key:down*15`, `type:hello_world`, `wheel:N` (at `-WheelX,-WheelY`),
and `wait:400`. Drive waits `-SettleMs` (default 500) after the steps before
capturing; batch steps accept `wheelat='x,y'` and `settle=<ms>` for the same
wheel point and pre-capture delay.

Use `.shots/batch.ps1` when several checks can share one process:

```powershell
.shots/batch.ps1 -Steps @(
    @{ page='Table'; section='Sorting'; do='click:353,387 key:enter' },
    @{ page='Switch'; section='Usage'; out="$env:TEMP\switch.png" }
)
```

Use `.shots/capture2.ps1` only when real foreground input is necessary, such as
modifier chords or hover/focus-sensitive states. It moves the real cursor,
raises the gallery, and can interrupt the user.

```powershell
.shots/capture2.ps1 -PageList "Button,Calendar"
.shots/capture2.ps1 -PageList Tooltip -HoverX 455 -HoverY 544
```

On macOS, where the `PrintWindow` and posted-input drivers above do not run,
[`native_capture.py`](../../.shots/native_capture.py) is the host-specific
driver with the same control-file semantics: it captures the window with
`screencapture -l<window-id>` and injects no input. Its prerequisites,
permission notes, and the mapping to the PowerShell drivers are in
[`native_capture.md`](../../.shots/native_capture.md).

## Deep links and environment controls

- `HEROGPUI_PAGE` selects a route.
- `HEROGPUI_SECTION` filters the page to matching section titles. Prefer this
  over fragile wheel counts for long pages.
- `HEROGPUI_THEME=dark` selects dark appearance.
- `HEROGPUI_OPEN_OVERLAYS=1` starts overlay demos open.
- `HEROGPUI_UNFOCUSED=1` prevents the app from taking focus.
- `HEROGPUI_REDUCE_MOTION=1` substitutes for an OS preference GPUI does not
  expose. The gallery maps it onto `set_reduce_motion`; the library itself
  reads no environment variable.
- `HEROGPUI_THEME_DIR=<dir>` loads every theme JSON file in `<dir>`,
  activates the first, and reloads edits live (`herogpui-theme`'s `watch`
  feature; native only).
- The web bootstrap accepts `v=<artifact-hash-prefix>` on a deep link. The
  value is forwarded to both `herogpui_web.js` and `herogpui_web_bg.wasm`, so
  the documentation iframe can invalidate the glue and binary together after a
  gallery rebuild.
- The web bootstrap also accepts `overlays=1` (or `overlays=true`) on a deep
  link: the browser spelling of `HEROGPUI_OPEN_OVERLAYS`. In component-preview
  mode the constructed example's overlay starts open; in the full gallery the
  same demos the native variable seeds start open. Any other value is ignored,
  like an unknown `?theme=`.
- `HEROGPUI_CONTROL=<file>` selects page, section, specimen, theme, overlay and
  reset state in a running process. A `specimen` is a stable key claimed by one
  live example in component-preview mode; an unknown key is rejected after the
  rendered-frame check. The sibling result files replace the input extension:
  `control.txt` produces `control.ack` or `control.error`.

Publish complete UTF-8 control requests by atomic replacement with a new,
nonempty `seq` for every request. The PowerShell 7 drivers share
[control.ps1](../../.shots/control.ps1), whose writer uses a sibling temporary
file and [.NET's replacing move](https://learn.microsoft.com/en-us/dotnet/api/system.io.file.move).
Do not write directly to the live control file: a reader could observe a new
sequence number before its page and options have been written.

```text
seq=1
page=Select
section=Usage
theme=light
overlays=0
reset=1
preview=component
motion=reduce
specimen=btn-v-DangerSoft
```

`page` uses the exact navigation title, such as `Combo Box`. `section` retains
the case-insensitive substring/CSV filter; every requested filter must match
rendered content. In component-preview mode only the first matching example is
constructed, so request one example at a time. `specimen` narrows that preview
to one explicitly keyed example and requires `preview=component`; omit it to
clear the previous key. Unknown pages, unmatched filters,
duplicate keys and invalid option values report an error instead of acknowledging
the previous page or a blank preview.

Omitting `section`, `theme` and `overlays` means all sections, light theme and the
existing closed-overlay switch. Omitting `page`, `preview` or `motion` preserves
that setting. `reset=1` creates a fresh gallery root and retires its previous
demo state, focus, toasts and shared validation record. Late callbacks belonging
to the old root cannot update the new root. The gallery retains the tasks from
its promise demos and cancels them before clearing toasts, so an old upload
cannot repopulate the reset queue. `reset=0` or omission preserves demo
values. `preview` accepts `gallery` or `component`; `motion` accepts `full` or
`reduce`.

Wait for the case-sensitive matching sequence in `.ack`, or surface the matching `seq`/`error`
record in `.error`. The acknowledgement is scheduled from the requested render
and delivered on a subsequent frame; a fixed timer does not establish that
ordering. It proves one rendered frame, not asset completion, settled motion,
correct pixels or the success of subsequent input. Keep the screenshot and
behavior checks below. Run `.shots/test-control.ps1` for the protocol helpers;
CI runs it on Windows, including a failed-publication case with a locked file.
Batch and smoke own their process cleanup in `finally`: stop and wait for the
child before deleting result files. Their environment overrides belong to the
child's `ProcessStartInfo`, leaving the caller's environment intact.

Window sizing differs between the foreground capture path and the off-screen
driver, which creates its window at the requested size. Prefer a section deep
link for a stable subject, and trust the driver's reported/captured dimensions
rather than assuming a requested height was honored.

## Input limitations

- Posted input does not carry Windows modifier state. `shift+tab`, Ctrl/Cmd
  chords, capitals, and shifted symbols need real input through `capture2.ps1`.
- GPUI derives characters from the key event it handles; a posted `WM_CHAR`
  alone does not type.
- A click may first establish hover and be swallowed. When testing a click
  handler, send two clicks if necessary and assert on the effect rather than
  assuming the first press reached it.
- Pointer clicks focus a control without enabling `:focus-visible`. To capture a
  focus-only surface, click it and then send a key so the root records keyboard
  modality.
- Coordinates are bitmap/client coordinates used by the driver. Verify the
  target visually before concluding that a handler is broken.

PowerShell variable names are case-insensitive. In driver scripts, a local `$vk`
would overwrite a `$VK` key-code table; choose names that cannot collide.

## Screenshot integrity

Capture the window, never the monitor. `Graphics.CopyFromScreen` can save the
user's foreground app when Windows refuses a focus steal. The scripts use
`PrintWindow(hwnd, hdc, 2)` (`PW_RENDERFULLCONTENT`) because GPUI presents via
DirectComposition.

For every new or refreshed image:

1. Check the captured frame is not uniform or blank.
2. Open it and verify the requested route and section are visible.
3. Verify the intended theme, overlay, hover, focus, or post-action state.
4. Treat a correctly rendered wrong page as a failed capture.

Use `.shots/refresh.ps1` only when a broad reference refresh is requested.
Focused appearance changes should update only the relevant screenshots.

[`docs/parity-sweep.md`](../parity-sweep.md) records the last two-image
comparison for every component route, with the goldens that are stale against
intentional changes and the states that could not be observed. Check it before
re-judging a component or refreshing a golden.

## Behavioral proof

A screenshot proves what was drawn, not whether a control responds. For state,
focus, keyboard, overlay, or drag changes:

1. Run the focused headless component test.
2. Rebuild with `.shots/rebuild.ps1`.
3. Drive the exact gallery path, preferably off-screen.
4. Assert on the resulting visible state or callback effect.

Use dark and reduced-motion/open-overlay smokes when tokens, overlays, or motion
change. Do not claim those variants from a default route smoke.

## Interaction sweep

`gallery/src/interaction_sweep/` mounts every component example headlessly in
component-preview mode, finds the controls that react to the pointer, then
hovers and presses each one. A finding fails the sweep when:

1. the frame does not return to rest after the pointer leaves (stuck
   highlights, translucent fills painted twice);
2. leaving a control mid-fade moves its colours away from rest (a fade that
   snaps to the hover colour before fading out);
3. a fill added on hover or press paints over text (a fill appended after its
   label);
4. redraws never settle (hover feedback that resizes its own hitbox);
5. the settled frame still changes a second later.

It reads `Window::painted_quads` and `painted_underlines`, and the preview
root underlines all text during the sweep so every text run leaves a
primitive. Glyphs, SVG icons and shadows are not observable, so the text
check sees only those underlines. Examples that animate on their own are
swept under reduced motion.

```bash
cargo test -p herogpui-gallery --lib interaction_sweep
HEROGPUI_SWEEP_PAGE=Tabs HEROGPUI_SWEEP_SECTION="Wrapping Labels" \
    cargo test -p herogpui-gallery --lib interaction_sweep -- --nocapture
```

`HEROGPUI_SWEEP_PAGE` takes an exact navigation title and
`HEROGPUI_SWEEP_SECTION` an exact example heading; `HEROGPUI_SWEEP_DEBUG=1`
prints each probe. A finding triaged as correct behaviour goes into
`EXPECTED` with the evidence for that verdict. An entry that no longer
matches fails the sweep, so an exemption cannot outlive the behaviour it
excuses.

## Web gallery smoke

The browser build has its own input path: a hidden IME textarea, the
browser's wheel events, its bundled fonts only. `web/scripts/gallery-smoke.mjs`
boots the wasm gallery in headless Chrome over the DevTools protocol and:

- renders every example in light, and each page's first example in dark.
  Each must differ from the blank stage (and its dark render from its light
  one), with no page error or wasm panic;
- types into Input, TextField, TextArea, SearchField and NumberField. That
  covers `insertText`, Backspace, an IME composition commit and the
  platform select-all chord. The typed text must reach the hidden textarea
  and the field must paint it;
- wheels the gallery page vertically, and a Table and a ScrollShadow
  horizontally: once with a horizontal delta, once with shift+wheel.

```bash
cd web
pnpm run gallery:fetch                       # or: bash ../.shots/build-wasm.sh --no-opt
node scripts/gallery-smoke.mjs --record ../target/wasm-smoke.json --shots ../target/wasm-smoke
node scripts/gallery-smoke.mjs --only table,input --skip-dark
```

CI runs it on every pull request (`wasm-smoke`) against the artifact the
`wasm` job just built, and uploads the run record and one PNG per example.
It needs Chrome or Chromium (`--chrome <path>` or `$CHROME` otherwise); it
launches with extensions disabled, because a developer profile's external
extensions inject overlays that take the focus. `EXPECTED_BLANK` lists
examples that legitimately render nothing at rest, each with its reason; an
entry that renders something fails as stale.
