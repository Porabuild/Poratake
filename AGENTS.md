# Poratake - Screenshot Tool (Electron + React + Bun)

## Supported Platforms

- MacOS 15 or later (Intel + Apple Silicon) — full feature set
- Windows 10/11 (x64 and arm64) — full-screen, area, and window screenshots; editor, history, pin, and cloud upload; OCR; QR scanning; desktop icons and wallpaper; timer capture; display and window selectors; freeze screen; scroll capture; all-in-one; print; recording; video editor; and transcription. The screenshot capture sound remains macOS-only.

Windows native functionality is provided by the Rust daemon in `src/main/daemon-win/`, which speaks the same JSON-RPC protocol as the macOS Swift daemon. Windows screen pixels come from the daemon's `screenshot` module, which captures with Windows Graphics Capture and, on HDR displays, reads the scRGB float surface and normalizes it by the OS SDR white level before encoding sRGB. Electron `desktopCapturer` is not used for screenshots because it hands back frames already clipped to 8-bit, which is washed out on HDR. Recording applies the same rule: `CaptureTarget::layout` resolves `hdr_white_scale` for the display being captured and `capture_pixel_format` gives the frame pool `R16G16B16A16Float` on HDR displays and `B8G8R8A8UIntNormalized` everywhere else, so an SDR display keeps the original 8-bit fast path untouched. Window recording captures the window rather than the screen area it happens to cover, so it keeps recording when the window is moved, raised, or sent to another display. The picked window travels as `windowId` from the selector to `screen-recorder` `start`: the Electron overlay carries a `pickId` on each pick target back through `AreaSelection.windowId` (`overlay-backend.ts`) on every platform, and dragging or resizing the selection box drops the id so the capture falls back to a plain area. On Windows `CaptureTarget` opens the window with `CreateForWindow`; because the sink writer's frame size is fixed at `BeginWriting`, the video keeps the size the window started with and every frame goes through `FitStage` in `tone_map.rs`, a D3D11 pass that scales the captured region into the encoder texture and letterboxes the rest (it tone-maps in the same pass on HDR, so window capture never needs the crop-only `ToneMapStage`). A resized window also needs `Direct3D11CaptureFramePool::Recreate`, after which the encoder's cached source views must be dropped — they would otherwise keep rendering textures the pool no longer fills. macOS uses `SCContentFilter(desktopIndependentWindow:)` with `scalesToFit`, which gives the same fixed-size letterboxed result. Cursor tracking follows the window too: both trackers read the live window bounds per sample and place the position inside the same letterbox through `fit_rect`/`fitRect`, so the cursor stays on the pixel it is actually over. Closing a recorded window must not lose the take — the Windows worker reports `TARGET_CLOSED` and stays alive so the app's `stop` still finalizes everything captured, which `recorder.ts` turns into a normal stop instead of a failure. Picking a window is final: the box that appears is a readout of that choice, not an editable area, so the Electron overlay locks it once a pick target is committed (`use-area-selection.ts`); dragging or resizing is only possible before a pick, which is also what drops the window binding. A picked window gets exactly one highlight for its whole life, from the moment it is picked to the end of the recording: `recording-overlay` gained a `showWindow` method that frames the window from the outside with a hairline rounded ring, click-through and excluded from capture, driven by a `SetWinEventHook` on the target's thread on Windows and a sampling timer on macOS (AppKit has no cross-application window-move notification without the accessibility permission). The ring must match the selection frames drawn in CSS, so its colour is never hardcoded in the daemons: `getAccentColor()` resolves the active theme preset and light/dark variant in the main process and passes the accent as `#rrggbb` on every `showWindow`, because the accent changes with the theme the user picked. On Windows the ring is one layered window shaped by `SetWindowRgn` with a rounded outer region minus a rounded inner one — four edge bars cannot have rounded corners — and it sits a DPI-scaled pixel outside the window so it never touches it. On Windows it is also z-anchored below the Electron control bar: `showRecordedWindowOutline` passes the control window's HWND as an optional `belowWindowId` and the daemon re-inserts the ring directly under it on every reposition, so the ring can never climb over the toolbar (macOS ignores the extra param). The Electron selection box is not a second highlight — `recordWindow` hides the whole selector overlay as soon as a window is picked, because that box is frozen where the window happened to be and a dim drawn around it goes stale the moment the window moves. `showRecordedWindowOutline` is idempotent per window id so the pre-recording highlight survives into the recording without a teardown flicker. The window's name rides along as `AreaSelection.windowName` into `RecordingControlState.targetName`, which the Electron control bar shows as a chip and pays for with `TARGET_LABEL_WIDTH` extra width. Discarding a recording from the control bar is not confirmed — the bar is a deliberate, single-purpose surface and a modal in front of a capture costs more than the mistake it prevents. On the float path the crop `CopySubresourceRegion` is replaced by a D3D11 pass in `src/main/daemon-win/src/tone_map.rs`: a full-screen triangle whose pixel shader `Load`s the crop offset, divides by the SDR white level, applies the same luminance rolloff and sRGB encode as `ToneMapper` in `display_color.rs`, and writes straight into the pooled encoder texture. That keeps the recorder zero-copy — the frame never leaves the GPU, and `MFCreateDXGISurfaceBuffer` still wraps the same texture for the sink writer. Never tone-map on the CPU here; FP16 RGBA at 4K60 is roughly 4 GB/s of readback. Shader and CPU paths are kept honest by `tone_map::tests`, which asserts the shader's arithmetic against `ToneMapper::map` within one 8-bit step. HDR10 output is deliberately not supported: the video editor composites and exports through an 8-bit sRGB canvas and forces `codec: 'avc'` in `webcodecs-exporter.ts`, so a 10-bit PQ recording could not satisfy the preview-equals-export rule below. The area overlay stays an Electron window (`src/main/capture/area-overlay/`): the daemon writes each display's frozen frame to a temp `.bmp` and retains it in memory, the overlay renders that file, and the confirmed selection is cropped from the retained frame and released. The overlay's visibility and click-through hole are driven through the daemon's `area_selector` window-handle methods (`hideWindowWithoutTransitions`/`showWindowWithoutTransitions`/`setWindowRegion`) only on Windows, because HWNDs are valid across processes; on macOS an NSWindow pointer does not survive the process boundary, so `session.ts` shows/hides the pooled windows with Electron `hide`/`showInactive`/`setOpacity` and skips the hole region there — the renderer handles every overlay interaction on its own. Callers that keep working against the live screen — scroll capture, for example — pass `{ freeze: false }` to `selectAreaWithOverlay`, which skips the frozen frame entirely and renders a transparent overlay instead. Screenshot selections read the "Freeze screen" setting through `isFreezeScreenEnabled()` in `src/main/capture/freeze-screen/preference.ts` (the same gate the macOS daemon freeze uses), so with the setting off `captureAreaToFile` selects on a live overlay and captures uncached pixels from the live screen once the overlay windows are hidden on confirm. All area selection runs through that overlay on every platform: `src/main/capture/area-selector/` is a facade that exposes only `overlay-backend.ts`, which drives `startInteractiveOverlay`, so both platforms share one `startAreaSelection`/`confirmAreaSelection`/`updateAreaSelection`/`setAreaSelectorAspectRatio` contract. The interactive overlay keeps a live selection the user can move, resize and constrain to an aspect ratio; the geometry lives in `src/renderer/utils/area-selection.ts` and the pointer state machine in `src/renderer/hooks/use-area-selection.ts`. The Swift and Rust `area_selector` modules' native selector flows are therefore no longer reached from the app and have been removed; the modules now expose only the Windows window-handle methods above (stubbed on macOS). Window and display picking for recording runs inside the same overlay: the facade passes pick targets (windows via the daemon's `window-selector` `list` method, displays via Electron `screen`) and the overlay renderer hit-tests them on hover and commits on click, so the native selector UIs never appear in the recording flow on either platform. The native `window-selector` `select` method is no longer reached from the app and has been removed, leaving `list` as the module's only method — the dedicated window screenshot mode picks through the same overlay (`captureWindowToFile` in `src/main/capture/area-overlay/`), cropping the picked window from the retained frozen frame when freeze is on and falling back to `screenshot capture-window` on the live overlay otherwise. The all-in-one panel is an Electron surface on every platform: `startAllInOne` passes a `toolbar` option through `startAreaSelection`, the overlay renderer draws the top-centred control bar (`src/renderer/components/area-overlay/all-in-one-toolbar.tsx`), toolbar clicks come back over the `area-overlay:toolbar` IPC channel, and `open-all-in-one.ts` only tracks the current selection (the daemons' `all_in_one` modules are gone). The recording control bar is one Electron window (`src/main/capture/video/recording-control-window.ts`) on both platforms, anchored at the top centre of the display holding the selection in both pre-recording and recording modes by `calculateControlPosition` in `src/main/capture/video/recording-control.ts`. The window must hug the actual toolbar so its transparent margins can never swallow clicks over the recorded screen: the renderer measures the toolbar with a ResizeObserver and reports its width over `recording-control:content-width`, and the main process sizes the window from that measurement (the `CONTROL_WIDTHS` constants are only the pre-measurement estimate). The daemons' `recording-control` panel methods (`show`, `hide`, `update`, `setMode`, `updateTimer`, `updateState`, `updateSettings`) are no longer reached from the app and have been removed, leaving `listIOSDevices` as the module's only method. The pre-recording bar carries a macOS-exclusive iPhone/iPad dropdown: `listIOSDevices` on the `recording-control` module enumerates connected iOS devices (Swift enumerates CMIO capture devices after allowing screen-capture devices; Rust returns an empty list for parity), and picking one routes the recording through that device. The microphone and camera dropdowns each fetch only their own device kind (`recording-control:devices` → `media-devices list` with a `kinds` parameter), because the Swift daemon requests TCC authorization per kind — asking for both would make opening the mic dropdown trigger a camera permission prompt; the settings window's device list still requests both kinds because it shows both. While a recording runs on Windows, the dimmed overlay is the same Electron area-overlay surface, kept visible past the selection confirm through `confirmOverlaySelection(keepVisible)` and parked by `concealOverlayHandoff` when the recording ends, so the Rust `recording_overlay` module is no longer reached from the app and is kept only for protocol parity (macOS keeps the native dim). While a recording is running, the toolbar keeps its microphone dropdown, system-sound toggle and camera dropdown live on both platforms: `recording-control.ts` holds those changes in a session-scoped `RecordingSession` instead of writing them to `config.recording`, and applies them through the `screen-recorder` methods `setMicrophone`, `setSystemAudio` and `setCamera`. On Windows the Rust audio worker swaps its `WasapiCapture` in place, keeps the AAC encoder and one shared `AudioClockState` per recording so a track disabled mid-recording is padded with silence and a track enabled mid-recording is created lazily and back-filled, and the camera worker records the periods it was on into `visibleRanges` in `camera.json`, which `renderCamera` honours in both preview and export. On macOS the Swift recorder implements the same contract: system audio is always captured into `system.m4a` (the stream's audio is zeroed while it is off, so the timeline stays continuous, and the file is deleted when it was never on), the microphone's `AVCaptureSession` is created lazily on first enable with silence back-filled to the recording start (and swapped in place through `beginConfiguration` when the device changes mid-recording), and the camera is suspended/resumed on one continuous `camera.mov` while `ScreenCaptureRecorder` records the on-periods into `visibleRanges` in `camera.json` in video time. The camera device is locked to the one the recording started with on both platforms (Media Foundation fixes the frame size at `BeginWriting`; the mac recorder never recreates the writer mid-recording). Both daemons implement the `screenshot` module (`capture-area`, `capture-window`) because the unified overlay path crops the final pixels from daemon-retained frames on both platforms. The shared module and method contract both daemons must implement is declared once as `DAEMON_METHODS` in `src/types/daemon.ts`, which also types every `daemon.call`; `tests/unit/daemon-module-parity.test.ts` scrapes both daemons' dispatch tables and fails when either drifts from it, so a native method may only be added or removed together with that constant. The QR-code and OCR flows capture through the same overlay on every platform, so the `screencapture` CLI module is gone entirely; the capture shutter sound it used to provide is now played by `afplay` in `src/main/capture/screenshot/capture-sound.ts` from `finalizeCapture`, gated by the `capture-sound` capability and the "Play sound" setting — scroll captures pass `silent: true` so they stay quiet as before. Every daemon thread that initialises COM or WinRT must first call `retain_process_mta()` from `src/main/daemon-win/src/com.rs`, and must release its COM objects before the apartment guard drops. The `windows` crate caches WinRT activation factories in process-wide statics, so letting the last thread tear the multi-threaded apartment down leaves those cached pointers dangling and the next capture crashes with an access violation. Media Foundation interfaces are not all proxy-registered, so `AgileReference` cannot carry them between threads — an `IMFSourceReader` fails with `REGDB_E_IIDNOTREG` (`0x80040155`). Share those with `MtaInterface` from the same module, which skips marshaling entirely because every thread has joined the one retained process MTA, and only call `MtaInterface::with` from a thread that joined it. The on-screen daemon control panel (scroll capture) gets its look and layout from `src/main/daemon-win/src/panel.rs`: DPI-scaled pill buttons in a rounded, double-buffered bar with hover and press states. Add new panels on those helpers rather than hand-rolling GDI drawing. Gate platform-specific surfaces with `isFeatureSupported()` from `src/main/system/capabilities.ts` or `src/renderer/utils/capabilities.ts`; feature support is defined in `src/types/capabilities.ts`. Build the daemon with `bun run build-daemon-win` or the complete Windows package with `bun run build-win`.

## Tests

Write tests for new features and bug fixes according to the following configuration:

- **Main**: Vitest - Use `bun run test`
- **Renderer**: Vitest tests live in `tests/renderer/`
- **Coverage**: `bun run test:coverage` - reports in `coverage/` folder

Tests use Vitest with vi.mock() for mocking modules (electron, AWS SDK, config), class-based mocks for constructors, dynamic imports (await import()) after vi.resetModules() to get fresh module instances with updated mocks, and are organized in `tests/unit/`, `tests/integration/`, and `tests/renderer/` - run with bun run test

## Code Style

- **Formatting**: Oxfmt (Rust) - use `bun run format` to fix, `bun run format:check` to verify; config lives in `.oxfmtrc.json` (includes Tailwind class sorting via `sortTailwindcss`)
- **Linting**: Oxlint (Rust) - use `bun run lint` (zero warnings enforced via `--deny-warnings`); config lives in `.oxlintrc.json`
- **Typecheck**: TypeScript 7 native - use `bun run typecheck`
- **All checks**: `bun run checks` runs typecheck + lint + format check + tests + the Windows daemon tests (verify only, no fixing)
- **Pre-commit**: `.githooks/pre-commit` runs fast checks on staged files (oxfmt format check + oxlint with `--deny-warnings`) followed by a full `bun run typecheck`
- **Windows daemon**: `bun run test:daemon-win` runs `cargo test` for `src/main/daemon-win/`. It is a no-op off Windows or without cargo, so a Rust compile break can only be caught locally on Windows — CI's `windows-native` job is the backstop.
- **Imports**: Group by external → components → hooks → types → utils. Use `type` for type-only imports (`import type { ToolType }`)
- **Types**: Store shared types in `src/types/` (accessible to main + renderer). Use discriminated unions for polymorphic data
- **Naming**: kebab-case (components), camelCase (functions/vars), SCREAMING_SNAKE_CASE (constants like `MACOS_COLORS`)
- **Components**: Export as default, define interfaces inline. Prefer small, reusable components over monolithic files
- **React**: Use functional components, hooks (`useCallback` for event handlers), avoid unnecessary re-renders
- **Icons**: Use Lucide React (`lucide-react`)
- **Styling**: Tailwind classes only - prefer built-in values (e.g., `gap-1`) over arbitrary values (e.g., `px-[20px]`)
- **Error Handling**: Check for null/undefined (e.g., `screenshotWindow?.method()`) and handle errors gracefully

## Architecture

- **Electron**: Main process (`src/main/`), renderer (`src/renderer/`), preload (`src/preload/`)
- **IPC**: Use `ipcMain.on` (main) / `ipcRenderer.send` (renderer) for process communication
- **Main-process event loop**: in the main process, promise continuations only run when the libuv loop turns. While the app is idle (no pending timers or I/O) the loop parks, so a callback that arrives from Chromium rather than from libuv (`globalShortcut`, tray/menu clicks, `ipcMain`) stalls at its first `await` until some unrelated event wakes the loop — measured at 100–2250ms. A flow whose first act is real I/O is immune, because `daemon.call()` writes to the daemon's stdin synchronously before returning its promise; a flow that awaits first is not. Never rely on that incidentally: call `flushPendingContinuations()` from `@/main/utils/event-loop` once at such a boundary (the shortcut dispatcher in `src/main/system/shortcuts.ts` is the reference). It schedules one loop turn, and a single microtask checkpoint drains the queue to empty, so the whole resolved chain unwinds to the first real I/O. Do not replace it with a periodic timer — that masks the problem and costs battery on every idle machine.
- **State**: Local hooks (`useState`, custom hooks in `src/renderer/hooks/`) - no global state library

## Performance & Design

- Minimize bundle size, memory, and CPU usage. Use lazy loading where appropriate
- Follow Shadcn design guidelines
- Use Shadcn components unless a custom component is necessary
- For frameless Electron windows with solid backgrounds, use transparent: false with a matching backgroundColor instead of transparent: true to avoid dark border artifacts on macOS.

## Licensing (AGPL-3.0 — mandatory, never trade off against other rules)

Poratake is a rebranded fork of Capty (https://github.com/capty-app/capty) and is licensed AGPL-3.0-only as a whole. These rules keep every release compliant:

- Never edit, remove, or replace `LICENSE` (verbatim GNU AGPL v3), and keep `"license": "AGPL-3.0-only"` in `package.json`.
- Never remove or weaken existing copyright, license, warranty, or attribution notices — in `README.md`, the About tab, file headers, or vendored code. The upstream "Copyright (C) 2026 Capty" notice must stay.
- `README.md` and the About tab must keep the modification statement (Poratake is a modified version of Capty, with the year of modification) and the link to the upstream project.
- The About tab is the app's AGPL §5(d) "Appropriate Legal Notices". It must always show: the copyright notices, the AGPL v3 license with a working link, the no-warranty statement, a link to this exact version's source (`SOURCE_URL/tree/v{version}`), and the third-party notices link. Do not remove or hide these when redesigning settings.
- Every public release must make corresponding source available alongside the binaries: the release workflow publishes to a GitHub Release whose tag must point at the exact commit the binaries were built from, so GitHub's automatic source archives sit next to the downloads. Never distribute a build made from uncommitted or unpushed changes, and never publish binaries through a channel that lacks a source link.
- Packaged builds must ship license material via `extraResources` in `electron-builder.json5`: `licenses/Poratake-AGPL-3.0.txt`, `licenses/THIRD_PARTY_NOTICES.md`, `licenses/FFmpeg-LGPL-2.1.txt`, and the license texts of bundled npm packages. When adding a runtime dependency that ships in the app, add its `LICENSE*` glob to that filter.
- Any change that adds, removes, or upgrades a bundled third-party component (npm package, native binary, font, icon set, sound, model) must update `THIRD_PARTY_NOTICES.md` in the same change, including the pinned version for native binaries.
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
- Never build or run dev. user will do. Exception: `cargo check`/`cargo test`/`cargo fmt` against `src/main/daemon-win/Cargo.toml` may be run directly, because no other local gate compiles Rust. Never run `bun run dev`, `build-win`, or any packaged build.
- Write less code and maintainable code
- Always put modularity and reusability in priority
- Prefer tailwind's built-in classes over custom sizes like px[20px]
- Try to create re-usable components instead of writing big chunks of code
- Be mindful about app size and performance and memory and cpu usage.
- Use types and interfaces and store them in the src/types folder so they can be used in both main and renderer processes.
- Learn from project's structure and implement new features in the same way.
- Break code into smaller components and files.
- When implementing a feature that can also be configurable, ask user's opinion to make it configurable in the app settings or not.
- Consider SOLID principles and best practices while writing code.
- If there is a refactor needed, ask user's opinion first.
- Avoid creating big files and components. Instead, modularize and break them into smaller pieces.
- NEVER NEVER NEVER code comment!
- Native functionality is provided by a unified daemon (`poratake-daemon`): Swift on macOS (`src/main/daemon/`, build with `./scripts/build-daemon.sh` for universal arm64 + x86_64) and Rust on Windows (`src/main/daemon-win/`, build with `bun run build-daemon-win`). Both speak the same JSON-RPC protocol over stdin/stdout, and module contracts must stay identical across platforms.
- The daemon uses JSON-RPC over stdin/stdout.
- When adding new native modules, add them to `src/main/daemon/Modules/` and register in `main.swift` (macOS), and to `src/main/daemon-win/src/modules/` and register in `main.rs` (Windows).
- When adding assets to the project like images, icons, sounds, etc, make sure you also consider them for production build and packing and notarizing to work on packaged app too.
- Don't patch symptoms! Fix the root cause of the issues.
- Use early returns to reduce nesting
- Important to avoid nested if-else statements
- Prefer switch-case for multiple conditions
- Use agnostic implementations everywhere possible so we can re-use those codes
- Don't use just delays instead of promises! Handle them properly. (using delay is basically hacking and we should avoid)
- Code duplications should be avoided at any cost.
- Don't install third-party packages without approval.
- The result shown in the video-editor must be identical to the results after export. This applies to zoom, trims, cuts and other layers applied to the video.
- We might have unused codes that seemed to be in-use! if you face any of them, make sure if they are used or not then decide to keep or remove them.
- For menu icons, Use the png icon and user will download them.
- No guessing and No assumption! Work with certainity.

## Coding examples

### Nesting

Bad code example:

```typescript
if (
  selection.x !== undefined &&
  selection.y !== undefined &&
  selection.width !== undefined &&
  selection.height !== undefined
) {
  if (selection.status === 'selected') {
    showAllInOneControl({
      x: selection.x,
      y: selection.y,
      width: selection.width,
      height: selection.height,
    });
  } else if (selection.status === 'updated') {
    updateAllInOnePosition({
      x: selection.x,
      y: selection.y,
      width: selection.width,
      height: selection.height,
    });
  }
}
```

Good code example:

```typescript
if (
  selection.x === undefined ||
  selection.y === undefined ||
  selection.width === undefined ||
  selection.height === undefined
) {
  return;
}

const bounds = {
  x: selection.x,
  y: selection.y,
  width: selection.width,
  height: selection.height,
};

if (selection.status === 'selected') {
  showAllInOneControl(bounds);
  return;
}

if (selection.status === 'updated') {
  updateAllInOnePosition(bounds);
}
```

### IPC Naming

Bad example: `'screenshot-take'`, `'screenshot-save-as'`, `screenshot:saveAs`

Good example: `'screenshot:take'`, `'screenshot:save-as'`, `screenshot:pin:toggle`

### Tailwind classes

Bad example: `p-[20p]x]`, `text-[14px]`, `bg-red-500`

Good example: `p-5`, `text-sm`, `bg-destructive`

### Wrapper functions

Avoid creating wrapper functions that only repeats the job

Bad example:

```typescript
function getAreaSelectorBinaryPath(): string {
  return getNativeBinaryPath('area-selector');
}
```

Good example:

Just use `getNativeBinaryPath` without wrapper.

## Questions

- If you have questions, Use the question tool only for asking questions.

## Code Review

- Review the PRs/Code against the purpose of the PR/Issue/Asked. If you find unrelated issues to the PR during the review, Report them in a separate section.
- Apply review recommendations only after user's confirmation.

## PR and Commits

- After end of each task that has a chance to create a PR for it, suggest PR title and description
- Add feat: or fix: or internal: prefixes to the PRs or Commits because thats how they will be appeared on release logs.
