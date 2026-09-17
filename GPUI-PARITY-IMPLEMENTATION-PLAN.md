# GPUI Parity Implementation Plan

Electron (`src/renderer/`, `src/main/`) is the reference. GPUI (`src/main/app-gpui/`)
matches it on theme tokens, settings registry rows, tray structure, shortcuts
coverage, recording control state (pause/countdown/iOS/TARGET chip), history
filter/sort, cloud upload (REST/S3), editor export/print/pin, transcription
pipeline, video-editor cut/split/timeline-zoom/shortcuts/first-frame/camera
`visibleRanges`/subtitle dialog, capture-preview stack/polish/upload, freeze
pipeline, window/display picking, all-in-one modes, capture sound, and the
`DAEMON_METHODS` contract (see `GPUI-UI-PARITY-AUDIT.md` for the UI-token audit).

What follows are the verified deltas, ordered by priority. Every item has
file:line evidence and an acceptance check. Severity: P0 = data loss / dead
input / broken take; P1 = missing feature or wrong behavior; P2 = design /
motion / polish delta. Items marked [fixed in tree] are already implemented in
the working tree but uncommitted — verify, don't rebuild.

## Already fixed in working tree (verify, don't rebuild)

- **Arrow style on create** [fixed in tree] — `editor/window.rs:737-745,816-825`
  passes `self.arrow_style` into `build_segment`; `:1193-1197` stores it. Remainder:
  bend-handle editing is still missing (item 12) and style doesn't apply to the
  current selection (item 16).
- **Shape fill on create** [fixed in tree] — `editor/window.rs:724,770,813` derives
  fill from `shape_fill_mode`; `build_shape` writes it at `:1161-1175`. Remainder:
  fill toggle doesn't update selected shapes (item 17).
- **macOS Cmd bindings** [fixed in tree, partial] — `editor/actions.rs:96-112` adds
  Cmd twins for undo/redo/copy/cut/paste/save/zoom/print/delete-file on macOS.
  Remainders: Cmd+S has no handler (item 1); Backspace/Escape/Cmd+A selection keys
  missing (item 24).

## P0 — correctness bugs (fix first, in order)

### 1. Cmd/Ctrl+S is bound but does nothing

- `editor/actions.rs:87,105` binds `SaveScreenshot`, but `editor/window.rs:1341-1420`
  has no `.on_action` for it. Toolbar Save goes through `EditorAction::Save` →
  `save_as` (`title_bar.rs:238-244`, `window.rs:1956,2209`). Electron saves via
  `screenshot-window.tsx:1139-1141`.
- Fix: route `SaveScreenshot` to the same `save_as` path as the toolbar button.
- Accept: Cmd+S (mac) / Ctrl+S (win) saves identically to the Save button.

### 2. Undo after crop restores annotations but not the image

- `apply_crop` (`editor/window.rs:908-928`) replaces `base_image` but `history.push`
  (`:939`) stores annotations only; `undo` (`:1976-1979`, `annotations.rs:406-411`)
  moves the annotation index alone. Electron keeps `lastCropStateRef` with image +
  dimensions + annotations (`screenshot-window.tsx:641-653,832-837`).
- Fix: snapshot `base_image` (+ dimensions) alongside the annotation revision, or
  keep a pre-crop image on the undo stack.
- Accept: undo after crop restores pixels and annotations together; redo re-applies.

### 3. Closing the recorded window loses the take

- Electron turns daemon `TARGET_CLOSED` into a graceful stop preserving the take
  (`src/main/capture/video/recorder.ts:28,231-242,328-330`,
  `recording-actions.ts:195-197,377`); AGENTS.md documents it.
- GPUI has zero `TARGET_CLOSED` handling (`video/recorder.rs`,
  `windows/recording_control.rs`).
- Fix: subscribe to the daemon error event, map `TARGET_CLOSED` to a normal stop,
  finalize the partial take, open editor/history.
- Accept: closing a recorded window mid-take yields a playable file, no error toast.

### 4. Async recorder failures leave GPUI stuck

- Electron handles `screen-recorder:error` mid-take (`recorder.ts:38-41,212-253`,
  `recording-actions.ts:173-193`): teardown UI, clean up or preserve, show error.
- GPUI `recorder::start` handles only synchronous start errors
  (`recording_control.rs:381-387`); no async error listener exists under `app-gpui/`.
- Fix: same subscription as item 3, with a terminal-failure path for non-`TARGET_CLOSED`
  codes mirroring `handleTerminalRecordingFailure`.
- Accept: injected mid-take recorder failure tears down UI and surfaces an error
  instead of hanging on a live timer.

## P1 — capture flows

### 5. Desktop icons are never hidden during capture

- Module exists (`system/desktop_icons.rs:39-67`) but nothing calls
  `HideSource::Capture` in `capture/coordinator.rs`, `capture/overlay.rs`, or intents.
  Electron hides for screenshot/OCR/QR/scroll/timer when the setting is on
  (`screenshot.ts:59-94`, `ocr/index.ts:54-57`, `qrcode/index.ts:33-36`,
  `scroll-capture/index.ts:68-72`, `timer-capture.ts:200-205`).
- Fix: wrap every capture entry (area/window/screen/OCR/QR/scroll/timer) with
  hide → capture → restore, behind the existing setting + accessibility gate.
- Accept: with the setting on, icons hide during each flow and restore after.

### 6. macOS scroll capture has no preview / control UI

- Electron: overlay + control bar + live stitched preview
  (`scroll-capture/index.ts:336-348`, `scroll-capture-overlay-window.tsx:15-80`,
  `scroll-capture-control-window.tsx`, `scroll-capture-window.ts:23-26,168-195`).
- GPUI `capture/scroll.rs:18-28` is daemon start/finish only; no windows.
  (Windows uses the native daemon panel on both shells — parity there.)
- Fix: GPUI preview surface fed by daemon progress (frame count, cursor-outside
  hint) + Done/Cancel/auto-scroll control.
- Accept: macOS scroll session shows live stitch progress and finishes/cancels
  without hotkeys.

### 7. macOS scroll capture forces native daemon UI

- Contract says macOS `nativeControls: false` (`src/types/daemon-contract.json:377`);
  GPUI always passes `true` (`capture/coordinator.rs:177`), forcing Swift
  `showCaptureUI()` (`ScrollCaptureModule.swift:94-114`) instead of the
  Electron-equivalent UI.
- Fix: pass `nativeControls: false` on macOS once item 6 lands (keep `true` on Windows).
- Accept: macOS scroll uses the GPUI preview/control UI, not shortcut-only native UI.

### 8. Scroll area selection must always be live

- Electron scroll passes `freeze: false` (`scroll-capture/index.ts:277`); GPUI
  `CaptureIntent::ScrollCapture` honors the user's freeze setting
  (`capture/mod.rs:306-307` via `with_frozen_screen`).
- Fix: force the live overlay path for scroll regardless of the freeze setting.
- Accept: scroll selection always runs over live pixels.

### 9. Record Screen is primary-display-only

- Electron opens display pick mode (`recording-actions.ts:707-710`);
  GPUI hardcodes the primary display (`capture/mod.rs:431-451`).
- Fix: route Record Screen through the overlay display-pick flow like Electron.
- Accept: Record Screen on multi-display lets the user pick the display.

### 10. Full-screen screenshot uses a different picker UX

- Electron: native `display-selector select` (`screenshot.ts:96-111`,
  `display-selector/index.ts:37-40`); GPUI: per-display overlay picker
  (`capture/mod.rs:649-682`). Product decision, but behavior must be equivalent.
- Fix: decide native vs overlay, then match the chosen UX on both shells.
- Accept: multi-display screenshot pick behaves equivalently on both shells.

### 11. Timer capture may re-freeze the final shot

- Electron releases freeze before countdown and captures live after
  (`timer-capture.ts:34-41,57`). GPUI dismisses → releases (`overlay.rs:1341-1391`,
  `capture/mod.rs:265-270`) but the post-countdown `capture_area_for` re-reserves
  freeze if the setting is on (`coordinator.rs:45-47,289-290`).
- Fix: capture the post-countdown shot from live pixels, not a fresh freeze.
- Accept: countdown runs over the live desktop and the final shot is live.

### 12. Escape during color pick kills the whole overlay

- Electron: Escape exits pick mode, overlay stays
  (`color-picker.tsx:154-155`, `area-overlay-window.tsx:173-174`,
  `area-overlay/color-picker.ts:37`). GPUI `Cancel` always dismisses
  (`capture/overlay.rs:1691-1696`) with no `picking_color` branch.
- NOTE: the loupe itself exists in both (`color-picker.tsx:34-55,170-235` vs
  `capture/color_picker.rs:65-116` 15x15 grid) — only the Escape semantics differ.
- Fix: branch `Cancel` on pick state: exit pick mode first, dismiss on second press.
- Accept: Escape during color pick returns to the toolbar; overlay stays open.

### 13. Windows OCR skips the ffmpeg preprocess

- Electron preprocesses non-mac captures (`ocr/index.ts:95-98`
  `preprocessImageForOcr`); GPUI `capture/analysis.rs` has no equivalent.
- Fix: port the preprocess step to the Windows OCR path.
- Accept: Windows OCR accuracy on low-contrast captures matches Electron.

### 14. No video/recording capture preview

- Electron shows a preview for recordings with play/export
  (`capture-preview/index.ts:284,366,566-597`, `capture-preview-window.tsx:31-32,203-206`).
  GPUI `CapturePreviewWindow::open` runs only from screenshot finalize
  (`capture/coordinator.rs:137`).
- Fix: open the preview flow on recording finish when `recording.showPreview` is on,
  with play/export actions.
- Accept: finished recording shows a video preview before/alongside the editor.

## P1 — screenshot editor

### 15. No resize handles on selected annotations

- Electron renders per-kind handles (`svg-annotations-overlay.tsx:626-653` +
  `shared/renderers/`, arrow `arrow-renderer.tsx:167-218`, text rotate/resize
  `:338-383`, redact `redact/index.tsx:78-126`). GPUI select = hit-test + move
  (`editor/window.rs:491-505,552-571`); selection is a dashed box
  (`editor/canvas.rs:762-789`).
- Fix: handle hit-testing + drag-resize per annotation kind, reusing `bounds()`;
  one undo step per resize; text keeps font size.
- Accept: rect/circle/line/arrow/text/redact drag-resize with pushed undo steps.

### 16. Tool options don't update selected annotations

- Electron `updateSelectedAnnotations` (`screenshot-window.tsx:353-415`) applies
  color/stroke/arrow-style/number-size/text-props/redact to the selection.
  GPUI `apply_option` updates tool defaults only (`editor/window.rs:1736-1752`).
- Fix: when a selection exists, apply option changes to it (and push undo).
- Accept: changing color/stroke/arrow style/etc. with a selection updates those
  annotations, not just the next draw.

### 17. Fill mode doesn't update selected shapes

- Electron `handleShapeFillModeChange` (`screenshot-window.tsx:503-527`); GPUI sets
  `shape_fill_mode` only (`editor/window.rs:1752`).
- Fix: apply the toggle to selected rects/circles (same path as item 16).
- Accept: outline/filled toggle re-renders selected shapes immediately.

### 18. Highlight color doesn't update selected highlights

- Electron `handleHighlightColorChange` (`screenshot-window.tsx:530-535`); GPUI sets
  the default only (`editor/window.rs:1743`).
- Fix: same as item 16 for highlight fill.
- Accept: picking a highlight color updates selected highlights.

### 19. No multi-select or marquee

- Electron: `selectedAnnotationIds[]` (`screenshot-window.tsx:136-138`),
  Shift-toggle (`svg-annotations-overlay.tsx:148-154`), marquee
  (`:129,424-453,750-755`). GPUI: `selected_annotation: Option<String>`
  (`editor/window.rs:98`).
- Fix: selection set + Shift-click toggle + drag-empty marquee; move applies to all.
- Accept: Shift-click toggles; marquee selects intersecting annotations.

### 20. Clipboard is single-annotation only

- Electron multi copy/cut, paste selects all (`screenshot-window.tsx:932-965,947`);
  GPUI copies one id and selects the last paste (`editor/window.rs:592-648`).
- Fix: copy/cut/paste the whole selection with the same offset behavior.
- Accept: multi-select copy/paste round-trips with all pasted ids selected.

### 21. Arrow bend offset not editable

- Electron bend handle (`arrow-renderer.tsx:188`,
  `svg-annotations-overlay.tsx:303-306`); auto-curve when unset
  (`arrow-renderer.tsx:41-45`, ported in `render/annotations.rs:320-324`).
  GPUI hardcodes `bend_offset: None` (`editor/window.rs:1197`).
- Fix: start/end/bend handles on selected arrows; drag updates `bendOffset`.
- Accept: bent arrows render, export, and round-trip through save/reload.

### 22. No double-click text re-edit

- Electron `svg-annotations-overlay.tsx:132-137` → `TextEditInput`
  (`editor-canvas.tsx:508-512`). GPUI has no double-click handler; text is
  new-placement only (`editor/window.rs:696-699`).
- Fix: double-click opens the inline editor with existing content.
- Accept: double-clicked text edits in place; Enter commits, Escape cancels.

### 23. No text rotation

- Electron rotate handle (`annotations/text-renderer.tsx:148-199`,
  `svg-annotations-overlay.tsx:340-358`). GPUI writes `rotation: None`
  (`editor/window.rs:972`); the field exists (`annotations.rs:102`) with no UI.
- Fix: rotate handle on text selection; export matches preview.
- Accept: rotated text renders and exports at the chosen angle.

### 24. Missing selection keyboard shortcuts

- Electron: Backspace deletes selection, Escape deselects, Cmd/Ctrl+A selects all
  (`useKeyboardShortcuts.ts:27-54`). GPUI binds `delete` only
  (`editor/actions.rs:86`).
- Fix: add Backspace-for-annotation, Escape-clear-selection, Select-all bindings
  (with Cmd twins on macOS).
- Accept: Backspace/Escape/Cmd+A behave like Electron.

### 25. Crop rect not movable/resizable after drag

- Electron `svg-crop-overlay.tsx:79-375` (move + corner handles, MIN_SIZE 20).
  GPUI crop is set by the initial drag only (`editor/window.rs:701-708,772-780`);
  overlay is visual (`editor/canvas.rs:794-854`).
- Fix: drag-to-move + corner handles on the pending crop rect before Enter.
- Accept: crop box moves/resizes post-draw; Enter applies, Escape cancels.

### 26. Number badges not renumbered on delete

- Electron `renumberAnnotations` (`screenshot-window.tsx:797-818`, auto effect
  `:659-685`, `number/number-utils.ts:61`). GPUI filters by id only
  (`editor/window.rs:573-587`).
- Fix: resequence remaining numbers per style/start value on delete.
- Accept: deleting badge 2 of 1-2-3 leaves 1-2.

### 27. No Shift-constrain on pen/highlight

- Electron locks to axis from the first point (`useDrawingTools.ts:167-195`).
  GPUI `extend_stroke` appends with no modifier check (`editor/window.rs:784-785`).
- Fix: Shift+drag constrains pen/highlight to horizontal/vertical.
- Accept: Shift strokes are axis-aligned.

### 28. Capture-and-attach is a file picker, not live capture

- Electron hold-Meta edge picker + `screenshot:capture-for-editor`
  (`screenshot-window.tsx:1188-1245,1217-1218`). GPUI `attach_layer` opens a file
  picker (`editor/window.rs:516,507-510`).
- Fix: hold-Cmd edge picker capturing a live screenshot onto the edge.
- Accept: hold-Cmd shows edges; click attaches a live capture.

### 29. No drag-drop image layers

- Electron `drop-zone-overlay.tsx` + `useImageDrop`
  (`screenshot-window.tsx:880-912`). Nothing under `app-gpui/` accepts drops.
- Fix: accept image file drops onto the editor, attach to the configured edge.
- Accept: dragging an image onto the editor attaches a layer.

### 30. No custom gradient background authoring

- Electron `wallpaper/background-editor.tsx:35-340` (multi-stop, angle, image type).
  GPUI can apply stored gradient customs (`editor/window.rs:1836-1843`) and add
  images (`:1800-1821`) but cannot author gradients in-editor.
- Fix: gradient editor in the wallpaper sheet (stops, angle, save as custom).
- Accept: user creates/edits/saves a gradient custom without leaving the editor.

### 31. Editor preferences not persisted

- Electron loads/saves via `useEditorState.ts:68-79,131-152` →
  `editor:updatePreferences`. GPUI hardcodes defaults (`editor/window.rs:168-182`);
  `EditorPreferences` (`config/schema.rs:18-49`) is unused at open.
- Fix: load on open, debounced save on change, same keys as Electron.
- Accept: tool/color/stroke/styles survive close/reopen.

## P1 — recording

### 32. Mid-recording camera device can be swapped (must be locked)

- Electron locks to the start device (`recording-control.ts:180,461-467`,
  `recording-control-window.tsx:142-167,623` `isDeviceLocked`). GPUI
  `select_device(Camera, …)` swaps `selected_camera_id` and calls the daemon
  mid-recording with no lock UI (`windows/recording_control.rs:395,490-498,894-905`).
  (Mic reselect mid-recording is parity on both — keep it.)
- Fix: when the session started with a camera, disable other cameras in the menu
  and only allow re-enable of the locked device.
- Accept: mid-recording camera menu shows the locked device only.

### 33. No live camera preview bubble

- Electron `showCameraPreview`/`updateCameraPreviewPosition`
  (`recording-control.ts:294-319,757-782`, `camera-preview.ts:24+`). GPUI recording
  flow never calls `camera_preview()` (`windows/recording_control.rs`).
- Fix: show/hide/reposition the daemon camera bubble with camera state.
- Accept: enabling camera before/during recording shows the positioned bubble.

### 34. Keyboard capture hardcoded off

- Electron passes `keyboardEnabled: true` (`recording-control.ts:520`,
  `recorder.ts:280`); GPUI hardcodes `false`
  (`windows/recording_control.rs:353`, forwarded by `video/recorder.rs:165`).
- Fix: wire the setting through `RecordingConfig` to the daemon.
- Accept: recording produces `keyboard.json` shown in the video editor.

## P1 — video editor

### 35. No video-segment trim on the timeline

- Electron trim handles + `handleTrimStart/Move/End`
  (`timeline-track.tsx:116-122`, `use-segment-operations.ts:103-194`). GPUI renders
  handles (`timeline/tracks.rs:244-250`) but `TrackKind::Video => {}`
  (`video_editor/mod.rs:1704-1711`) ignores the drag.
- Fix: implement video edge-drag → source in/out (`trimMinStart`/`trimMaxEnd`
  equivalent); preview and export must match.
- Accept: dragging a video clip edge trims it; preview-equals-export holds.

### 36. Preview playback is silent

- Electron plays program audio + music (`native-video-player.tsx:163,1255+`,
  `use-music-playback.ts:33+`). GPUI `drive_playback` advances the playhead and
  requests frames only (`video_editor/mod.rs:828-838,571-595`).
- Fix: native audio playback path for preview (mixer for mic/system + music).
- Accept: pressing play produces synced audio with the preview frames.

### 37. Scrub audio toggle does nothing

- Electron `AudioContext` scrub loop while paused
  (`timeline-controls.tsx:48,225`, `native-video-player.tsx:174-176,790-872,1180-1184`).
  GPUI stores `scrub_audio_enabled` only (`video_editor/mod.rs:920-921`,
  toggle at `timeline/controls.rs:197-203`).
- Fix: requires item 36, then scrub-position audio while paused.
- Accept: with scrub audio on, scrubbing while paused plays mixed audio.

### 38. No Whisper model download/readiness UI

- Electron checks binary/model + `whisper:download-progress`
  (`subtitle-settings-panel.tsx:57-70,107-136,123-126`, `utils/whisper.ts`,
  `subtitle-handlers.ts:85`). GPUI has picker + generate
  (`video_editor/panels.rs:1894-1928`) but `download_model` blocks with no progress
  (`video/transcription.rs:294+`).
- Fix: readiness state + percent progress events/UI before first transcribe.
- Accept: first transcribe shows check state and download % like Electron.

### 39. No transcription generation progress

- Electron `subtitle:generation-progress` + panel UI
  (`subtitle-handlers.ts:113-114`, `subtitle-settings-panel.tsx:119-121,128-129`).
  GPUI shows a boolean "Generating..." (`video_editor/mod.rs:1319-1320,1918-1919`;
  `video/transcription.rs:433+` has no callback).
- Fix: 0-100% progress callback from the transcription path to the panel.
- Accept: transcribe shows live percent matching daemon/whisper stages.

### 40. Export shows percent only (no elapsed/ETA)

- Electron percent + elapsed + remaining (`export-settings-panel.tsx:309-327`,
  `use-export-progress.ts:41-108`, `export-progress-indicator.tsx`). GPUI percent
  only (`video_editor/title_bar.rs:63-72`, `panels.rs:2368-2374`, `mod.rs:397-407`
  permille poll).
- Fix: track start time + rate; show elapsed and ETA in panel and title bar.
- Accept: export shows elapsed and remaining like Electron.

## P1 — settings / updater / tray / pin / theme

### 41. Reopening Settings to a tab doesn't switch when already open

- Electron `navigate-tab` IPC (`settings-window.tsx:44-52`,
  `main/settings/window.ts:24-26`); GPUI `open_or_activate` only activates
  (`intents.rs:134-166`, no `select_category` on the existing window).
- Fix: forward the requested category to the live window.
- Accept: tray "Update Ready" with Settings on General switches to About.

### 42. Preview corner/dismiss changes don't reposition live previews

- Electron emits `capture-preview:reposition` (`settings-window.tsx:74-76`); GPUI
  has no equivalent (no `reposition` under `app-gpui/`).
- Fix: notify the preview stack on settings change; reposition without restart.
- Accept: changing preview corner moves existing thumbnails immediately.

### 43. About omits release notes on update-available

- Electron `releaseNotes` block (`about-tab.tsx:163-179`); GPUI version card only
  (`settings/about.rs:174-196`).
- Fix: fetch + render scrollable "What's New" from the release.
- Accept: update-available shows release notes in About.

### 44. No `unsupported` update state on Linux

- Electron `status: 'unsupported'` (`update/index.ts:139-142`,
  `about-tab.tsx:116-118`); GPUI has no `Unsupported` variant
  (`settings/about.rs:94-247`, `update.rs:19-40`).
- Fix: show "Automatic updates are not available on this platform" on Linux.
- Accept: Linux About never runs a broken check flow.

### 45. No automatic update check (startup + interval)

- Electron checks ~3s after launch and every 30m (`update/index.ts:19-20,178-190,
225-227`); GPUI checks only on user click (`settings/mod.rs:251-271`).
- Fix: schedule startup + periodic checks; reflect status in tray without About open.
- Accept: launch triggers a check; tray shows update state unprompted.

### 46. macOS tray misses "Hide Menu Bar Icon"

- Electron shows it on both platforms (`menu/index.ts:417-448`); GPUI gates
  `HideTrayIcon` to Windows (`system/tray/menu.rs:262-265`, though `intents.rs:200-227`
  supports the macOS title).
- Fix: expose the entry + confirmation on macOS.
- Accept: macOS tray menu hides the menu-bar icon like Electron.

### 47. Pin window not draggable by the image

- Electron `-webkit-app-region: drag` (`pin-window.tsx:38-41`); GPUI `pin.rs:66-79`
  sets no drag area.
- Fix: mark the client area draggable (`WindowControlArea::Drag` / `drag_area`).
- Accept: pinned window drags by grabbing the image.

### 48. Closing a pin doesn't restore the editor

- Electron `restoreToEditor` + `openScreenshotWindow` (`pin.ts:115-125`); GPUI opens
  the pin only (`editor/window.rs:1605-1609`), no close handler in `pin.rs`.
- Fix: on pin close, reopen the editor with the same state when pinned from editor.
- Accept: pin-from-editor → close pin → editor returns.

### 49. Pin window oversized on Retina

- Electron divides by the scale factor (`pin.ts:159-162`); GPUI uses raw pixels
  (`pin.rs:21-40`).
- Fix: size from logical (scale-divided) dimensions.
- Accept: pin size on Retina matches Electron.

### 50. macOS system appearance not followed live

- Electron follows via `nativeTheme` + `useAppTheme`; GPUI spawns the watcher only
  on Windows (`theme/watcher.rs:58-109`, `main.rs:130-138` `cfg(windows)`).
- Fix: macOS appearance watcher repainting on OS light/dark toggle when mode is `system`.
- Accept: OS toggle repaints GPUI without restart.

## P2 — design / motion / polish deltas (decide per surface, don't just build)

Capture / preview: 51. **Window pick doesn't lock the box** — Electron locks post-pick
(`use-area-selection.ts:277-280`); GPUI confirms/hands off immediately
(`capture/overlay.rs:916-967`). Decide whether area-screenshot window picks need
the locked readout state. 52. **All-in-one target menu visible in OCR mode** — Electron hides it
(`all-in-one-toolbar.tsx:100-102`); GPUI always renders (`all_in_one_toolbar.rs:65`). 53. **Preview lacks Show-in-Finder/Explorer** — Electron
(`capture-preview-window.tsx:240-243`, `index.ts:497`); absent in
`capture_preview.rs`. 54. **Preview lacks drag-out** — Electron (`capture-preview-window.tsx:273-279`,
`index.ts:625`); absent in GPUI. 55. **Move-preview UX differs** — Electron labeled menu
(`capture-preview-window.tsx:394-412`); GPUI cycles `display_id`
(`capture_preview.rs:1265-1287`). 56. **Overlay reveal timing** — Electron disables WM animations + reveal timing
(`window-pool.ts:95-105`); GPUI `deferred_show`/`raise_all`
(`capture/overlay.rs:40,248-260,511-603`). Verify no flash on Windows HDR/multi-DPI. 57. **All-in-one tab indicator static** — Electron `TabsIndicator`
(`all-in-one-toolbar.tsx:82`); GPUI static `mode_tab`
(`all_in_one_toolbar.rs:46-51`). 58. **Recording outline re-assert** — Electron calls `showRecordedWindowOutline` at
daemon start (`recorder.ts:290-291`); GPUI only at pick
(`capture/overlay.rs:144-176,948-949`), not in `recording_control.rs:329-387`.
Pick-time outline usually suffices; verify no flicker/loss at start. 59. **Recording tray indicator** — Electron (`recorder.ts:326`, `recording-tray.ts:60-74`);
absent in GPUI. Decide whether recording needs a tray icon + stop action.

Recording bar geometry: 60. **Control-bar width is constant** — Electron measures via ResizeObserver
(`recording-control-window.tsx:364-377`, `recording-control-window.ts:53,86-89,177,338`);
GPUI uses `chrome::recording_control_width` (`recording_control.rs:1136-1142`,
`ui/chrome.rs:284-294`). Locales/device menus may clip or pad. 61. **Control-bar top offset differs** — Electron 24px (`recording-control.ts:69-91`);
GPUI 48px on macOS (`ui/chrome.rs:115-117,128-133,297-305`).

Editor: 62. **Text inline editor sizing** — Electron measures text bounds
(`text-edit-input.tsx:56-63,111-137`); GPUI fixed heuristic
(`editor/window.rs:1019`). Box should track rendered text. 63. **Title bar vs split toolbar** — Electron embeds the toolbar in the app TitleBar
(`screenshot-window.tsx:1249-1295`); GPUI one unified bar (`title_bar.rs:3-5`).
Visual match claimed by the audit; keep unless drift is found. 64. **Wallpaper sheet eager + no animation** — Electron lazy Suspense + slide
(`screenshot-window.tsx:1298-1321`, `wallpaper/index.tsx:70-78`); GPUI eager when
tool = Wallpaper. Decide whether open/close motion matters.

Video editor: 65. **Post-recording goes straight to editor** — Electron shows the capture preview
first when enabled (`recording-actions.ts:249-259`); GPUI opens the editor
directly (`recording_control.rs:1029-1033`). Reconcile with item 14. 66. **Pre-export time/size estimation** — Electron computes it but keeps the UI
hidden (`export-estimation.ts:9-246`, `export-settings-panel.tsx:258-277`); GPUI
absent. Build only when Electron unhides it.

Settings / history / misc: 67. **Settings search lacks clear (x)** — Electron (`settings-sidebar.tsx:58-66`);
absent (`settings/mod.rs:803-810`). 68. **No URL hash routing** — Electron (`settings-window.tsx:10-16,37-41,55-57`);
GPUI `from_id` exists (`registry.rs:49-57`) with no hash read/write. 69. **Hide-icons permission gate differs** — Electron async accessibility request
(`screenshot.ts:68-72`, `permissions.ts:283-285`); GPUI opens prefs and returns
(`settings/item_row.rs:60-72`). Also: no runtime auto-disable of the setting when
denied (`desktop-icons/preference.ts:10-16`). 70. **Update auto-download** — Electron downloads on available (`update/index.ts:64,81`);
GPUI manual (`settings/about.rs:201-216`, `settings/mod.rs:1365-1409`). Decide
manual vs auto; tray progress wiring (`menu.rs:133-142` state exists, no
progress feed) follows the decision. 71. **Tray menu icons** — Electron bundled PNG/template images
(`menu/index.ts:104-185`); GPUI stroked Lucide (`system/tray/menu.rs:288-290`). 72. **Tray icon tint on theme change** — Electron rebuilds (`menu/index.ts:496-502`);
no GPUI equivalent. Pairs with item 50. 73. **History Clear All confirms** — Electron immediate
(`history-window.tsx:150-164`); GPUI `rfd::MessageDialog`
(`history/mod.rs:326-337`). Match Electron (no modal) or keep the guard deliberately. 74. **Pin cascade offset** — Electron 30px (`pin.ts:62-64,71-72`); GPUI count always 0
(`pin.rs:41`). 75. **Pin minimum size 100x100** — Electron (`pin.ts:69-70`); absent in GPUI. 76. **Onboarding dots static** — Electron `transition-colors`
(`onboarding-window.tsx:84-90`); GPUI static (`onboarding.rs:645-667`). 77. **Onboarding macOS shortcuts step** — Electron styled `<ol>`
(`onboarding-window.tsx:179-203`); GPUI plain string (`onboarding.rs:411-418`). 78. **Toast durations/setting** — Electron 5s transient
(`notification.ts:3-24`); GPUI `toast.rs:17-18` undocumented. History delete
ignores `showDeletionNotifications` (`history/mod.rs:307-311` vs editor/video
which read it).

Motion (Electron animation is modest — mostly hover `transition-*`, `animate-spin`,
dialog `animate-in/out`, one `progress-indeterminate` keyframe in `base.css:349-360`;
GPUI flips hover states instantly and has no CSS transitions): 79. **Settings nav/search hover** — add ~150ms ease or record as accepted static
(`settings-sidebar.tsx:49,84` vs `settings/mod.rs:710-711,795-796`). 80. **History cards/keys** — `transition-all` hover (`history-item.tsx:46` vs
`history/item.rs:244-247`); smooth `scrollIntoView` on keyboard nav
(`history-window.tsx:180-188` vs `history/mod.rs:431`). 81. **Spinners** — Tailwind `animate-spin` vs GPUI `loader-2` 1s rotation
(`about-tab.tsx:89`, `history-item.tsx:58` vs `ui/icon.rs:211-227`). 82. **Indeterminate progress** — `base.css:349-361` keyframe absent in GPUI. 83. **Dialog fade/zoom** — `ui/dialog.tsx:39,57` 200ms; GPUI uses native `rfd`
dialogs; `chrome.rs:85-87` `DIALOG_FADE_MS` unused for in-app modals. 84. **Select chevron rotation** — instant `rotate_180` (`ui/icon.rs:232-241`).

Platform limits (accepted, no action unless revisited):

- **Backdrop blur is baked** (12px stand-in; settings sidebar mixes opacity instead
  of `blur(18px) saturate(125%)`). GPUI has no live backdrop-filter.
- **Geist fonts** — Electron `base.css:4-16,115-123`; GPUI uses system/HeroGPUI fonts.
- **Updater is check+manual-download** — needs a signed-artifact + installer-handoff
  story before full parity (Electron downloads + installs:
  `src/main/update/index.ts:81,175`).
- **Wayland multi-display capture blocked** (`capture/mod.rs:411-421`) — documented
  limitation, not Electron drift.
- **Linux session matrix** (X11/Wayland/headless gating) is GPUI-only by design.

Non-gaps found during comparison (do not file):

- Overlay loupe exists in both — only Escape semantics differ (item 12).
- Aspect-ratio presets are unwired in BOTH shells (Electron `session.ts:877-888` has
  no non-test callers; GPUI `overlay.rs:1306-1310` hardcodes `None` with
  `selection.rs` geometry ready). Wire once, on both, when the product wants it.
- Crop-undo and TARGET_CLOSED are real gaps (items 2-3), but the "HiDPI export
  geometry" and "wallpaper preset degradation" claims from the early agent pass did
  not reproduce: export uses natural resolution on both (`useCanvasExport.ts` vs
  `export.rs:27-38`, DPR is preview-only in `canvas-renderer.tsx:314`), and
  wallpaper presets/padding/frames/aspect/balance all port (`wallpaper_sheet.rs`
  vs `wallpaper/index.tsx`).
- Frame-step in GPUI uses the source frame rate (`mod.rs:2535-2537`) vs Electron's
  fixed 1/30 (`use-editor-shortcuts.ts:33`) — GPUI is more accurate; keep.
- `video/transcription.rs:1-5` "Media Foundation" doc comment is stale wording; the
  code is portable pure Rust. Fix the comment when nearby code is touched.

## Suggested order

P0 first: 1 (dead key) + 2 (crop undo) are one editor-history pass → 3 + 4 are one
recorder-error-subscription pass. Then recording correctness: 32 (camera lock),
34 (keyboard), 33 (preview bubble). Then editor selection pass: 15 (handles) →
16/17/18 (options-to-selection) → 19/20 (multi-select/clipboard) → 21/23 (arrow
bend/text rotate) → 22/25/26/27 → 28/29/30/31. Then capture pass: 5 (desktop icons
— touches every entry) → 8/11 (scroll-live/timer-live) → 9/10 (display picks) →
12/13/14 → 6/7 (macOS scroll UI). Then video-editor media pass: 36 → 37 (audio
engine, then scrub) → 35 (trim) → 38/39 (transcription progress) → 40 (export
ETA). Then settings/updater/pin/theme: 41/42 → 43/44/45 (+70 decision) → 46 →
47/48/49 (+74/75) → 50 (+72). P2 last, per-surface decisions in items 51-84.

Verify with: `cargo fmt --all`, `cargo test --locked`, `cargo clippy --locked
--all-targets -- -D warnings` in `src/main/app-gpui/` (plus `bun run checks` for
the Electron side guards).
