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
motion / polish delta. Items 1–84 are closed (done, decided, or accepted).

Working tree leftovers from the first audit (Cmd+S, crop undo, TARGET_CLOSED,
arrow style, fill, Cmd bindings) were implemented in checkpoints 1–2. The
updater looks up `-universal-mac.zip` / `latest-mac.yml` on macOS,
`*-win-${arch}.exe` / `latest.yml` on Windows, and `*-linux-${arch}.tar.gz` /
`latest-linux.yml` on Linux.

## P0 — correctness bugs (fix first, in order)

### 1. Cmd/Ctrl+S is bound but does nothing

- `editor/actions.rs:87,105` binds `SaveScreenshot`, but `editor/window.rs:1341-1420`
  has no `.on_action` for it. Toolbar Save goes through `EditorAction::Save` →
  `save_as` (`title_bar.rs:238-244`, `window.rs:1956,2209`). Electron saves via
  `screenshot-window.tsx:1139-1141`.
- Fix: route `SaveScreenshot` to the same `save_as` path as the toolbar button.
- Accept: Cmd+S (mac) / Ctrl+S (win) saves identically to the Save button.
- Status: done. `SaveScreenshot` routes to the same `save_as` path as the toolbar.

### 2. Undo after crop restores annotations but not the image

- `apply_crop` (`editor/window.rs:908-928`) replaces `base_image` but `history.push`
  (`:939`) stores annotations only; `undo` (`:1976-1979`, `annotations.rs:406-411`)
  moves the annotation index alone. Electron keeps `lastCropStateRef` with image +
  dimensions + annotations (`screenshot-window.tsx:641-653,832-837`).
- Fix: snapshot `base_image` (+ dimensions) alongside the annotation revision, or
  keep a pre-crop image on the undo stack.
- Accept: undo after crop restores pixels and annotations together; redo re-applies.
- Status: done. Crop snapshots `base_image` (+ dimensions) on the undo stack.

### 3. Closing the recorded window loses the take

- Electron turns daemon `TARGET_CLOSED` into a graceful stop preserving the take
  (`src/main/capture/video/recorder.ts:28,231-242,328-330`,
  `recording-actions.ts:195-197,377`); AGENTS.md documents it.
- GPUI has zero `TARGET_CLOSED` handling (`video/recorder.rs`,
  `windows/recording_control.rs`).
- Fix: subscribe to the daemon error event, map `TARGET_CLOSED` to a normal stop,
  finalize the partial take, open editor/history.
- Accept: closing a recorded window mid-take yields a playable file, no error toast.
- Status: done. Daemon `RECORDING_TARGET_CLOSED` maps to a normal stop.

### 4. Async recorder failures leave GPUI stuck

- Electron handles `screen-recorder:error` mid-take (`recorder.ts:38-41,212-253`,
  `recording-actions.ts:173-193`): teardown UI, clean up or preserve, show error.
- GPUI `recorder::start` handles only synchronous start errors
  (`recording_control.rs:381-387`); no async error listener exists under `app-gpui/`.
- Fix: same subscription as item 3, with a terminal-failure path for non-`TARGET_CLOSED`
  codes mirroring `handleTerminalRecordingFailure`.
- Accept: injected mid-take recorder failure tears down UI and surfaces an error
  instead of hanging on a live timer.
- Status: done. The same daemon-error subscription handles non-`TARGET_CLOSED`
  codes as a terminal failure.

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
- Status: done, with two corrections from the reference. Window screenshots
  never hide in Electron (`captureWindowToFile` is unwrapped in
  `screenshot.ts:140-160`), and screen screenshots hide only around the
  pixels, not during the display pick — GPUI matches both. Area selection
  hides at entry (except Recording), the screen picker and all-in-one hide at
  confirm, restores funnel through finalize/error/cancel/countdown/scroll-end
  exits, and every overlay entry clears an abandoned hide first
  (`capture/desktop_icons.rs`, `capture/mod.rs`, `capture/overlay.rs`,
  `capture/coordinator.rs`).

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
- Status: done — new macOS-only `windows/scroll_capture.rs` owns the session
  UI: a 240px preview panel fed by the daemon's `scroll-capture:frame` base64
  stitch (bottom-anchored cover crop, status line with frame count and
  estimated height, "Move cursor here to continue" hint while the cursor is
  outside) plus a 168x52 control bar with Start/Stop auto-scroll, Done (Enter)
  and Cancel (Esc). The coordinator subscribes to the macOS-only frame /
  auto-scroll / cursor progress events, the control bar toggles through the
  new `startAutoScroll` / `stopAutoScroll` client methods, and Enter/Escape
  are armed as temporary global shortcuts for the session, mirroring the
  Electron control window. Both popups are nonactivating so scrolling the
  target app never loses focus.

### 7. macOS scroll capture forces native daemon UI

- Contract says macOS `nativeControls: false` (`src/types/daemon-contract.json:377`);
  GPUI always passes `true` (`capture/coordinator.rs:177`), forcing Swift
  `showCaptureUI()` (`ScrollCaptureModule.swift:94-114`) instead of the
  Electron-equivalent UI.
- Fix: pass `nativeControls: false` on macOS once item 6 lands (keep `true` on Windows).
- Accept: macOS scroll uses the GPUI preview/control UI, not shortcut-only native UI.
- Status: done — the coordinator passes `nativeControls: false` on macOS with
  a new `boundaryOnly` start flag (documented in `daemon-contract.json`,
  ignored by the Rust/Linux daemons and never sent by Electron). Swift splits
  `showCaptureUI` so `boundaryOnly` draws just the click-through area frame —
  GPUI windows cannot be click-through, which is what the frame needs — and
  recolors it orange/blue as the cursor leaves/re-enters during auto-scroll,
  matching the Electron overlay frame. Windows and Linux keep
  `nativeControls: true` and the daemon-owned panel.

### 8. Scroll area selection must always be live

- Electron scroll passes `freeze: false` (`scroll-capture/index.ts:277`); GPUI
  `CaptureIntent::ScrollCapture` honors the user's freeze setting
  (`capture/mod.rs:306-307` via `with_frozen_screen`).
- Fix: force the live overlay path for scroll regardless of the freeze setting.
- Accept: scroll selection always runs over live pixels.
- Status: done — `CaptureIntent::allows_freeze` returns false for scroll and
  `with_frozen_screen` takes a force-live flag
  (`capture/intent.rs`, `capture/mod.rs`).

### 9. Record Screen is primary-display-only

- Electron opens display pick mode (`recording-actions.ts:707-710`);
  GPUI hardcodes the primary display (`capture/mod.rs:431-451`).
- Fix: route Record Screen through the overlay display-pick flow like Electron.
- Accept: Record Screen on multi-display lets the user pick the display.
- Status: done — `start_screen_recording` opens the screen picker with the
  Recording intent on multi-display setups (live, like `recordScreen`
  `freeze: false`); click opens the pre-recording bar and conceals the pickers,
  Escape cancels. Single display keeps the direct path (`capture/mod.rs`,
  `capture/overlay.rs`).

### 10. Full-screen screenshot uses a different picker UX

- Electron: native `display-selector select` (`screenshot.ts:96-111`,
  `display-selector/index.ts:37-40`); GPUI: per-display overlay picker
  (`capture/mod.rs:649-682`). Product decision, but behavior must be equivalent.
- Fix: decide native vs overlay, then match the chosen UX on both shells.
- Accept: multi-display screenshot pick behaves equivalently on both shells.
- Status: decided — keep the GPUI overlay picker. Verified equivalent to the
  native UI (`DisplaySelectorModule.swift`: dim + hover un-dim, click selects,
  Escape cancels with no fallback): the native UI shows no visible display
  numbers either (numbers exist only in the response payload), and GPUI's
  click-to-pick makes them redundant. The picker honors the freeze setting so
  the click captures the exact pixels shown; forcing it live would desync the
  `confirm_screen` reservation. No display-id routing gap either: macOS passes
  the CG display id through and the daemons resolve area/rect captures to the
  containing monitor (`screen_capture.rs` `monitor_contains`,
  `ScreenCaptureRecorder.swift` display filter).

### 11. Timer capture may re-freeze the final shot

- Electron releases freeze before countdown and captures live after
  (`timer-capture.ts:34-41,57`). GPUI dismisses → releases (`overlay.rs:1341-1391`,
  `capture/mod.rs:265-270`) but the post-countdown `capture_area_for` re-reserves
  freeze if the setting is on (`coordinator.rs:45-47,289-290`).
- Fix: capture the post-countdown shot from live pixels, not a fresh freeze.
- Accept: countdown runs over the live desktop and the final shot is live.
- Status: done — the post-countdown capture calls `capture_area_reserved` with
  no reservation (`capture/coordinator.rs`).

### 12. Escape during color pick kills the whole overlay

- Electron: Escape exits pick mode, overlay stays
  (`color-picker.tsx:154-155`, `area-overlay-window.tsx:173-174`,
  `area-overlay/color-picker.ts:37`). GPUI `Cancel` always dismisses
  (`capture/overlay.rs:1691-1696`) with no `picking_color` branch.
- NOTE: the loupe itself exists in both (`color-picker.tsx:34-55,170-235` vs
  `capture/color_picker.rs:65-116` 15x15 grid) — only the Escape semantics differ.
- Fix: branch `Cancel` on pick state: exit pick mode first, dismiss on second press.
- Accept: Escape during color pick returns to the toolbar; overlay stays open.
- Status: done. `Cancel` exits pick mode first; a second Escape dismisses.

### 13. Windows OCR skips the ffmpeg preprocess

- Electron preprocesses non-mac captures (`ocr/index.ts:95-98`
  `preprocessImageForOcr`); GPUI `capture/analysis.rs` has no equivalent.
- Fix: port the preprocess step to the Windows OCR path.
- Accept: Windows OCR accuracy on low-contrast captures matches Electron.
- Status: done — `recognize_text` preprocesses on non-mac via the `image`
  crate (upscale the longer side to 1300px Lanczos, grayscale, unsharp),
  mirroring the FFmpeg filter chain without bundling FFmpeg; any failure
  falls back to the raw capture and the processed file is deleted after
  (`capture/analysis.rs`).

### 14. No video/recording capture preview

- Electron shows a preview for recordings with play/export
  (`capture-preview/index.ts:284,366,566-597`, `capture-preview-window.tsx:31-32,203-206`).
  GPUI `CapturePreviewWindow::open` runs only from screenshot finalize
  (`capture/coordinator.rs:137`).
- Fix: open the preview flow on recording finish when `recording.showPreview` is on,
  with play/export actions.
- Accept: finished recording shows a video preview before/alongside the editor.
- Status: done — `CapturePreviewWindow::open_video` stacks a video preview
  with 8fps autoplay, Edit/double-click into the video editor, Delete
  recording, Show in Folder, and Copy running the standard export with a
  Cancel pill; recording finish shows the preview when `showPreview` is on
  and opens the editor directly when it is off, mirroring the screenshot
  flow. One deviation: Electron copies the exported file to the clipboard,
  which GPUI's clipboard cannot hold, so the export is revealed in the
  folder with a toast instead (`windows/capture_preview.rs`,
  `windows/recording_control.rs`).

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
- Status: done. Per-kind handles hit-test and resize with one undo step.

### 16. Tool options don't update selected annotations

- Electron `updateSelectedAnnotations` (`screenshot-window.tsx:353-415`) applies
  color/stroke/arrow-style/number-size/text-props/redact to the selection.
  GPUI `apply_option` updates tool defaults only (`editor/window.rs:1736-1752`).
- Fix: when a selection exists, apply option changes to it (and push undo).
- Accept: changing color/stroke/arrow style/etc. with a selection updates those
  annotations, not just the next draw.
- Status: done. `apply_option_to_selection` updates the selection and pushes undo.

### 17. Fill mode doesn't update selected shapes

- Electron `handleShapeFillModeChange` (`screenshot-window.tsx:503-527`); GPUI sets
  `shape_fill_mode` only (`editor/window.rs:1752`).
- Fix: apply the toggle to selected rects/circles (same path as item 16).
- Accept: outline/filled toggle re-renders selected shapes immediately.
- Status: done (same path as item 16).

### 18. Highlight color doesn't update selected highlights

- Electron `handleHighlightColorChange` (`screenshot-window.tsx:530-535`); GPUI sets
  the default only (`editor/window.rs:1743`).
- Fix: same as item 16 for highlight fill.
- Accept: picking a highlight color updates selected highlights.
- Status: done (same path as item 16).

### 19. No multi-select or marquee

- Electron: `selectedAnnotationIds[]` (`screenshot-window.tsx:136-138`),
  Shift-toggle (`svg-annotations-overlay.tsx:148-154`), marquee
  (`:129,424-453,750-755`). GPUI: `selected_annotation: Option<String>`
  (`editor/window.rs:98`).
- Fix: selection set + Shift-click toggle + drag-empty marquee; move applies to all.
- Accept: Shift-click toggles; marquee selects intersecting annotations.
- Status: done. Selection set + Shift-toggle + empty-drag marquee.

### 20. Clipboard is single-annotation only

- Electron multi copy/cut, paste selects all (`screenshot-window.tsx:932-965,947`);
  GPUI copies one id and selects the last paste (`editor/window.rs:592-648`).
- Fix: copy/cut/paste the whole selection with the same offset behavior.
- Accept: multi-select copy/paste round-trips with all pasted ids selected.
- Status: done. Clipboard copies/cuts/pastes the whole selection.

### 21. Arrow bend offset not editable

- Electron bend handle (`arrow-renderer.tsx:188`,
  `svg-annotations-overlay.tsx:303-306`); auto-curve when unset
  (`arrow-renderer.tsx:41-45`, ported in `render/annotations.rs:320-324`).
  GPUI hardcodes `bend_offset: None` (`editor/window.rs:1197`).
- Fix: start/end/bend handles on selected arrows; drag updates `bendOffset`.
- Accept: bent arrows render, export, and round-trip through save/reload.
- Status: done. Start/end/bend handles update `bendOffset`.

### 22. No double-click text re-edit

- Electron `svg-annotations-overlay.tsx:132-137` → `TextEditInput`
  (`editor-canvas.tsx:508-512`). GPUI has no double-click handler; text is
  new-placement only (`editor/window.rs:696-699`).
- Fix: double-click opens the inline editor with existing content.
- Accept: double-clicked text edits in place; Enter commits, Escape cancels.
- Status: done. Double-click opens the inline editor with existing content.

### 23. No text rotation

- Electron rotate handle (`annotations/text-renderer.tsx:148-199`,
  `svg-annotations-overlay.tsx:340-358`). GPUI writes `rotation: None`
  (`editor/window.rs:972`); the field exists (`annotations.rs:102`) with no UI.
- Fix: rotate handle on text selection; export matches preview.
- Accept: rotated text renders and exports at the chosen angle.
- Status: done. Rotate handle on text; preview-equals-export holds.

### 24. Missing selection keyboard shortcuts

- Electron: Backspace deletes selection, Escape deselects, Cmd/Ctrl+A selects all
  (`useKeyboardShortcuts.ts:27-54`). GPUI binds `delete` only
  (`editor/actions.rs:86`).
- Fix: add Backspace-for-annotation, Escape-clear-selection, Select-all bindings
  (with Cmd twins on macOS).
- Accept: Backspace/Escape/Cmd+A behave like Electron.
- Status: done. Backspace deletes, Escape deselects, Cmd/Ctrl+A selects all.

### 25. Crop rect not movable/resizable after drag

- Electron `svg-crop-overlay.tsx:79-375` (move + corner handles, MIN_SIZE 20).
  GPUI crop is set by the initial drag only (`editor/window.rs:701-708,772-780`);
  overlay is visual (`editor/canvas.rs:794-854`).
- Fix: drag-to-move + corner handles on the pending crop rect before Enter.
- Accept: crop box moves/resizes post-draw; Enter applies, Escape cancels.
- Status: done. Pending crop has move + corner handles.

### 26. Number badges not renumbered on delete

- Electron `renumberAnnotations` (`screenshot-window.tsx:797-818`, auto effect
  `:659-685`, `number/number-utils.ts:61`). GPUI filters by id only
  (`editor/window.rs:573-587`).
- Fix: resequence remaining numbers per style/start value on delete.
- Accept: deleting badge 2 of 1-2-3 leaves 1-2.
- Status: done. `renumber_annotations` resequences on delete and style/start.

### 27. No Shift-constrain on pen/highlight

- Electron locks to axis from the first point (`useDrawingTools.ts:167-195`).
  GPUI `extend_stroke` appends with no modifier check (`editor/window.rs:784-785`).
- Fix: Shift+drag constrains pen/highlight to horizontal/vertical.
- Accept: Shift strokes are axis-aligned.
- Status: done. Shift+drag constrains pen/highlight to an axis.

### 28. Capture-and-attach is a file picker, not live capture

- Electron hold-Meta edge picker + `screenshot:capture-for-editor`
  (`screenshot-window.tsx:1188-1245,1217-1218`). GPUI `attach_layer` opens a file
  picker (`editor/window.rs:516,507-510`).
- Fix: hold-Cmd edge picker capturing a live screenshot onto the edge.
- Accept: hold-Cmd shows edges; click attaches a live capture.
- Status: done. `attach_layer` hides the editor and runs a live area capture.

### 29. No drag-drop image layers

- Electron `drop-zone-overlay.tsx` + `useImageDrop`
  (`screenshot-window.tsx:880-912`). Nothing under `app-gpui/` accepts drops.
- Fix: accept image file drops onto the editor, attach to the configured edge.
- Accept: dragging an image onto the editor attaches a layer.
- Status: done. `on_drop` of image files attaches to the hovered edge.

### 30. No custom gradient background authoring

- Electron `wallpaper/background-editor.tsx:35-340` (multi-stop, angle, image type).
  GPUI can apply stored gradient customs (`editor/window.rs:1836-1843`) and add
  images (`:1800-1821`) but cannot author gradients in-editor.
- Fix: gradient editor in the wallpaper sheet (stops, angle, save as custom).
- Accept: user creates/edits/saves a gradient custom without leaving the editor.
- Status: done. Wallpaper sheet authors gradient stops, angle, and custom save.

### 31. Editor preferences not persisted

- Electron loads/saves via `useEditorState.ts:68-79,131-152` →
  `editor:updatePreferences`. GPUI hardcodes defaults (`editor/window.rs:168-182`);
  `EditorPreferences` (`config/schema.rs:18-49`) is unused at open.
- Fix: load on open, debounced save on change, same keys as Electron.
- Accept: tool/color/stroke/styles survive close/reopen.
- Status: done. Preferences load on open and save on change.

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
- Status: done. Camera is locked to the device the session started with.

### 33. No live camera preview bubble

- Electron `showCameraPreview`/`updateCameraPreviewPosition`
  (`recording-control.ts:294-319,757-782`, `camera-preview.ts:24+`). GPUI recording
  flow never calls `camera_preview()` (`windows/recording_control.rs`).
- Fix: show/hide/reposition the daemon camera bubble with camera state.
- Accept: enabling camera before/during recording shows the positioned bubble.
- Status: done. Daemon camera bubble show/hide/reposition follows camera state.

### 34. Keyboard capture hardcoded off

- Electron passes `keyboardEnabled: true` (`recording-control.ts:520`,
  `recorder.ts:280`); GPUI hardcodes `false`
  (`windows/recording_control.rs:353`, forwarded by `video/recorder.rs:165`).
- Fix: wire the setting through `RecordingConfig` to the daemon.
- Accept: recording produces `keyboard.json` shown in the video editor.
- Status: done. `keyboard_enabled: true` is passed through to the daemon.

## P1 — video editor

### 35. No video-segment trim on the timeline

- Electron trim handles + `handleTrimStart/Move/End`
  (`timeline-track.tsx:116-122`, `use-segment-operations.ts:103-194`). GPUI renders
  handles (`timeline/tracks.rs:244-250`) but `TrackKind::Video => {}`
  (`video_editor/mod.rs:1704-1711`) ignores the drag.
- Fix: implement video edge-drag → source in/out (`trimMinStart`/`trimMaxEnd`
  equivalent); preview and export must match.
- Accept: dragging a video clip edge trims it; preview-equals-export holds.
- Status: done — `timeline/edit.rs::trim_video` ports `handleTrimMove`
  (pointer timeline time back to source time through the segment offset and
  speed, Electron's clamp nesting, 0.5s minimum), wired into the
  `TrackKind::Video` resize arm with a seek-to-segment-start on gesture end.
  Preview and export both map through `to_video_segments`, so trims match.

### 36. Preview playback is silent

- Electron plays program audio + music (`native-video-player.tsx:163,1255+`,
  `use-music-playback.ts:33+`). GPUI `drive_playback` advances the playhead and
  requests frames only (`video_editor/mod.rs:828-838,571-595`).
- Fix: native audio playback path for preview (mixer for mic/system + music).
- Accept: pressing play produces synced audio with the preview frames.
- Status: done — new `video/preview_audio.rs` renders system/mic/music stems
  with the export's own decode/segment/placement pipeline (preview hears what
  export writes) and plays one rodio player per stem, so volumes and mutes
  apply live without re-rendering. The playhead timer stays master; the
  transport pauses through the silent first-frame section and re-anchors on
  jumps or drift past Electron's 0.3s threshold. No audio device degrades to
  silent preview. New `rodio` dependency (MIT/Apache, notices + Linux CI
  ALSA dep included).

### 37. Scrub audio toggle does nothing

- Electron `AudioContext` scrub loop while paused
  (`timeline-controls.tsx:48,225`, `native-video-player.tsx:174-176,790-872,1180-1184`).
  GPUI stores `scrub_audio_enabled` only (`video_editor/mod.rs:920-921`,
  toggle at `timeline/controls.rs:197-203`).
- Fix: requires item 36, then scrub-position audio while paused.
- Accept: with scrub audio on, scrubbing while paused plays mixed audio.
- Status: done — `set_playhead` while paused scrubs the program stems (music
  stays silent like Electron) and a 120ms generation-tagged timer stops them
  after the pointer goes quiet; play, pause and the toggle all cancel the
  scrub correctly.

### 38. No Whisper model download/readiness UI

- Electron checks binary/model + `whisper:download-progress`
  (`subtitle-settings-panel.tsx:57-70,107-136,123-126`, `utils/whisper.ts`,
  `subtitle-handlers.ts:85`). GPUI has picker + generate
  (`video_editor/panels.rs:1894-1928`) but `download_model` blocks with no progress
  (`video/transcription.rs:294+`).
- Fix: readiness state + percent progress events/UI before first transcribe.
- Accept: first transcribe shows check state and download % like Electron.
- Status: done. `download_model` streams the response body and reports the
  content-length percent (100 on completion); a cached model returns silently
  with no phantom percent. The panel shows the model description, download
  size, and memory usage under the tabs (same copy as Electron) and the
  button renders `Downloading model (N%)` from a `TranscriptionStatus` enum
  pumped through one ordered channel. The Electron `checking` flash has no
  equivalent — readiness is two synchronous file-exists probes, so the button
  jumps straight to downloading/generating. Model tabs are guarded against
  mid-flight changes in `set_transcription_model` (behaviorally the disabled
  tabs; no greyed visual since `tab_row` has no disabled flag across its 11
  callers) and the prompt stays editable (snapshotted at click; HeroGPUI
  `TextArea` has no disabled state).

### 39. No transcription generation progress

- Electron `subtitle:generation-progress` + panel UI
  (`subtitle-handlers.ts:113-114`, `subtitle-settings-panel.tsx:119-121,128-129`).
  GPUI shows a boolean "Generating..." (`video_editor/mod.rs:1319-1320,1918-1919`;
  `video/transcription.rs:433+` has no callback).
- Fix: 0-100% progress callback from the transcription path to the panel.
- Accept: transcribe shows live percent matching daemon/whisper stages.
- Status: done. `transcribe` takes a progress callback with the exact Electron
  stages (5 convert, 15 whisper start, 15-95 from stderr `progress = N%` via
  `Math.round(15 + N*0.8)`, 95 parse, 100 written); `run_whisper` pipes stderr
  (stdout nulled — nothing ever read it) and both the main and DTW-fallback
  attempts stream. Failures surface as an inline destructive error line plus
  the existing toast, cleared on model change. Parser covered by
  `whisper_progress_lines_parse_like_the_electron_regex` (first-hit-wins,
  case-insensitive, clamped to 100).

### 40. Export shows percent only (no elapsed/ETA)

- Electron percent + elapsed + remaining (`export-settings-panel.tsx:309-327`,
  `use-export-progress.ts:41-108`, `export-progress-indicator.tsx`). GPUI percent
  only (`video_editor/title_bar.rs:63-72`, `panels.rs:2368-2374`, `mod.rs:397-407`
  permille poll).
- Fix: track start time + rate; show elapsed and ETA in panel and title bar.
- Accept: export shows elapsed and remaining like Electron.
- Status: done (was already implemented; verified + regression-tested).
  `update_export_eta` ports `useExportProgress` exactly (5% gate, raw
  projection, 0.1 smoothing, rounded read), the footer renders `{m:ss} elapsed`
  / `{m:ss} remaining` / `Calculating...` with the same copy and `m:ss`
  format, and start/finish/cancel reset the state. Added
  `export_eta_matches_the_electron_estimator` covering the gate, the raw
  projection, and the smoothing step.

## P1 — settings / updater / tray / pin / theme

### 41. Reopening Settings to a tab doesn't switch when already open

- Electron `navigate-tab` IPC (`settings-window.tsx:44-52`,
  `main/settings/window.ts:24-26`); GPUI `open_or_activate` only activates
  (`intents.rs:134-166`, no `select_category` on the existing window).
- Fix: forward the requested category to the live window.
- Accept: tray "Update Ready" with Settings on General switches to About.
- Status: done (was already implemented; verified). `open_settings` forwards
  `select_category` to the live window before activating, and the tray update
  rows dispatch `Intent::OpenAbout` → `open_settings(Category::About)`.

### 42. Preview corner/dismiss changes don't reposition live previews

- Electron emits `capture-preview:reposition` (`settings-window.tsx:74-76`); GPUI
  has no equivalent (no `reposition` under `app-gpui/`).
- Fix: notify the preview stack on settings change; reposition without restart.
- Accept: changing preview corner moves existing thumbnails immediately.
- Status: done (was already implemented; verified).
  `CapturePreviewWindow::reposition` moves the live stack (in place on
  Windows, rebuilt elsewhere) and settings calls it on corner change.
  Electron fires on any preview update but its handler only moves windows, so
  the corner-only trigger is behaviorally equivalent.

### 43. About omits release notes on update-available

- Electron `releaseNotes` block (`about-tab.tsx:163-179`); GPUI version card only
  (`settings/about.rs:174-196`).
- Fix: fetch + render scrollable "What's New" from the release.
- Accept: update-available shows release notes in About.
- Status: done. `check` keeps the release `body` as `notes` on `Available`
  (carried through `Downloading` to `Ready`), converted by a `releaseNotesToText`
  port (block-tag breaks, tag stripping with Electron's prefix-match quirks,
  entity decoding, blank-line squash — covered by
  `release_notes_become_plain_text_like_the_reference`). About renders the
  "What's New:" heading with the notes capped at `max-h-32` scrollable; each
  line is its own element since GPUI has no pre-wrap.

### 44. No `unsupported` update state on Linux

- Electron `status: 'unsupported'` (`update/index.ts:139-142`,
  `about-tab.tsx:116-118`); GPUI has no `Unsupported` variant
  (`settings/about.rs:94-247`, `update.rs:19-40`).
- Fix: show "Automatic updates are not available on this platform" on Linux.
- Accept: Linux About never runs a broken check flow.
- Status: done. Electron Linux stays `unsupported` (no Electron Linux package).
  GPUI Linux checks `latest-linux.yml` and installs `*-linux-${arch}.tar.gz`.
  `Status::Unsupported` remains for OS without an installer suffix.

### 45. No automatic update check (startup + interval)

- Electron checks ~3s after launch and every 30m (`update/index.ts:19-20,178-190,
225-227`); GPUI checks only on user click (`settings/mod.rs:251-271`).
- Fix: schedule startup + periodic checks; reflect status in tray without About open.
- Accept: launch triggers a check; tray shows update state unprompted.
- Status: done. The status is a global `UpdateCell` (Electron's module-level
  `updateState`); `spawn_auto_check` runs one check 3s after launch then every
  30min, skipping downloading/ready (plus in-flight checks, so a manual click
  and the timer never fetch twice). Results repaint an open About and rebuild
  the tray only when the row changes, mirroring `setStatus`: available/ready
  transitions, download 10% buckets (first 10% keeps the available row),
  errors never touch the tray. A 10Hz tick while downloading repaints About's
  progress bar (previously frozen between start and finish) and feeds the tray
  buckets. `TrayMenuState::from_config` takes the live status so settings
  changes no longer wipe the update row. This also builds the progress feed
  item 70 assumed missing — see item 70.

### 46. macOS tray misses "Hide Menu Bar Icon"

- Electron shows it on both platforms (`menu/index.ts:417-448`); GPUI gates
  `HideTrayIcon` to Windows (`system/tray/menu.rs:262-265`, though `intents.rs:200-227`
  supports the macOS title).
- Fix: expose the entry + confirmation on macOS.
- Accept: macOS tray menu hides the menu-bar icon like Electron.
- Status: done. The `cfg!(windows)` gate is gone; the label is "Hide Menu Bar
  Icon" off Windows (Electron's `isWindows` split), the macOS confirmation
  detail matches Electron's copy including the Applications parenthetical, and
  `hide_icon_entry_uses_the_platform_label` pins the entry per platform.

### 47. Pin window not draggable by the image

- Electron `-webkit-app-region: drag` (`pin-window.tsx:38-41`); GPUI `pin.rs:66-79`
  sets no drag area.
- Fix: mark the client area draggable (`WindowControlArea::Drag` / `drag_area`).
- Accept: pinned window drags by grabbing the image.
- Status: done (was already implemented; verified). The image sits inside a
  `drag_area` (`WindowControlArea::Drag`) overlay.

### 48. Closing a pin doesn't restore the editor

- Electron `restoreToEditor` + `openScreenshotWindow` (`pin.ts:115-125`); GPUI opens
  the pin only (`editor/window.rs:1605-1609`), no close handler in `pin.rs`.
- Fix: on pin close, reopen the editor with the same state when pinned from editor.
- Accept: pin-from-editor → close pin → editor returns.
- Status: done (was already implemented; verified). The editor pins through
  `open_for_editor` with the file path and pin close reopens it via
  `open_editor_for`.

### 49. Pin window oversized on Retina

- Electron divides by the scale factor (`pin.ts:159-162`); GPUI uses raw pixels
  (`pin.rs:21-40`).
- Fix: size from logical (scale-divided) dimensions.
- Accept: pin size on Retina matches Electron.
- Status: done (was already implemented; verified). Decoded pixels are divided
  by the display scale factor like Electron's `pngSize / scaleFactor`, and
  `pin_window_size` floors like `Math.floor`.

### 50. macOS system appearance not followed live

- Electron follows via `nativeTheme` + `useAppTheme`; GPUI spawns the watcher only
  on Windows (`theme/watcher.rs:58-109`, `main.rs:130-138` `cfg(windows)`).
- Fix: macOS appearance watcher repainting on OS light/dark toggle when mode is `system`.
- Accept: OS toggle repaints GPUI without restart.
- Status: done. `watcher::spawn` has a macOS half: a thread re-reading
  `AppleInterfaceStyle` from `NSUserDefaults` every 2s (in-process — the probe
  itself moved off the `defaults` subprocess onto objc2) and reporting flips
  over the same channel/main-thread `apply_system_mode` path as Windows. The
  probe is validated against the `defaults` CLI by
  `the_macos_probe_matches_the_defaults_key`.

## P2 — design / motion / polish deltas (decide per surface, don't just build)

### 51. Window pick doesn't lock the box

- Electron locks post-pick (`use-area-selection.ts:277-280`); GPUI confirms/hands
  off immediately (`capture/overlay.rs:916-967`).
- Status: decided — no lock. Window screenshots auto-confirm; recording hands off
  to the control bar. The Electron locked readout exists because its overlay stays
  up after a pick; GPUI does not keep that surface.

### 52. All-in-one target menu visible in OCR mode

- Status: done. Target menu is omitted while OCR is selected
  (`all_in_one_toolbar.rs`).

### 53. Preview lacks Show-in-Finder/Explorer

- Status: done (was already implemented; verified). Preview hover chrome calls
  `reveal_in_file_manager`.

### 54. Preview lacks drag-out

- Status: done. Screenshot previews use `on_drag` + `external_drag_payload(Files)`.

### 55. Move-preview UX differs

- Status: done. Preview exposes a labeled display menu instead of cycling ids.

### 56. Overlay reveal timing

- Status: decided — `deferred_show` / `raise_all` on Windows is the equivalent of
  Electron disabling WM animations then revealing. Flash on HDR/multi-DPI is a
  runtime check for the screenshot round, not a missing API.

### 57. All-in-one tab indicator static

- Status: accepted static. Active `mode_tab` uses the muted fill; HeroUI
  `TabsIndicator` slide is motion-only and GPUI has no sliding tab thumb.

### 58. Recording outline re-assert

- Status: done. `recording_control` re-calls `show_window_recording_outline` when
  the recorder actually starts.

### 59. Recording tray indicator

- Status: done. One tray icon swaps to the recording glyph (non-Windows) and the
  menu gains Stop Recording. Not a second tray icon.

### 60. Control-bar width is constant

- Status: accepted. GPUI has no post-layout ResizeObserver; width stays the
  `recording_control_width` estimate and long device names truncate.

### 61. Control-bar top offset differs

- Status: done. `RECORDING_TOP_MARGIN` is 24px below the work-area origin on every
  platform.

### 62. Text inline editor sizing

- Status: done. `text_field_width` sums `TextSystem::advance` plus pad, min 160px.

### 63. Title bar vs split toolbar

- Status: decided — keep the unified GPUI title bar. Visual match claimed by the
  audit; no drift found.

### 64. Wallpaper sheet eager + no animation

- Status: done. Sheet already slides in over 300ms (`wallpaper_sheet.rs`). Eager
  load is accepted: no React Suspense in GPUI.

### 65. Post-recording goes straight to editor

- Status: done (was already implemented; verified). Capture preview opens first
  when enabled, matching item 14.

### 66. Pre-export time/size estimation

- Status: decided — do not build. Electron computes it but keeps the UI hidden.

### 67. Settings search lacks clear (x)

- Status: done. Search field shows a clear control when the query is non-empty.

### 68. No URL hash routing

- Status: done. `Category::from_id` plus `--intent open-settings <tab>` is the
  native equivalent of Electron's hash; an open Settings window already keeps the
  selected category.

### 69. Hide-icons permission gate differs

- Status: done. `accessibility_request` prompts via
  `AXIsProcessTrustedWithOptions`; the settings switch uses that path.

### 70. Update auto-download

- Status: done. `Available` starts `start_download` (Electron's
  `downloadUpdate()` from `update-available`). Tray progress still rebuilds on
  10% buckets. macOS fetches `-universal-mac.zip` + `latest-mac.yml` and opens
  the unpacked `Poratake.app`; Windows keeps `*-win-${arch}.exe` + `latest.yml`;
  Linux fetches `*-linux-${arch}.tar.gz` + `latest-linux.yml` and replaces the
  install directory after quit.

### 71. Tray menu icons

- Status: accepted. GPUI paints Lucide strokes; Electron ships PNG/templates.
  Theme follows by construction (pairs with 72).

### 72. Tray icon tint on theme change

- Status: done (was already implemented; verified with item 50): `tray_icon(dark,
recording)` re-tints the monochrome pixels (test-covered), every `RebuildMenu`
  re-sets the icon, and theme changes call `refresh_shell`.

### 73. History Clear All confirms

- Status: done. Matches Electron: Clear All deletes immediately, no modal.

### 74. Pin cascade offset

- Status: done (was already implemented; verified). `PIN_OFFSET` is 30px and
  origin counts existing pin windows.

### 75. Pin minimum size 100x100

- Status: done. `PIN_MIN_SIZE` is 100 and `window_min_size` uses it.

### 76. Onboarding dots static

- Status: done. Dot fills fade over 150ms (`transition-colors`) when the current
  step changes.

### 77. Onboarding macOS shortcuts step

- Status: done. Numbered rows with bold segments, matching the styled `<ol>`.

### 78. Toast durations/setting

- Status: done. Transient toasts use the 5s notification path (`show_transient`);
  history delete stays silent in both shells.

### 79. Settings nav/search hover

- Status: accepted static. `hover_flag` already drives the hover fill; GPUI has
  no CSS `transition-*` on background.

### 80. History cards/keys

- Status: accepted hover (instant `muted_background`); keyboard nav already
  `scroll_to_item`.

### 81. Spinners

- Status: done (was already implemented; verified). `spinner_element` is a 1s
  linear `loader-2` rotation.

### 82. Indeterminate progress

- Status: done. `indeterminate_progress` ports the 1.2s `progress-indeterminate`
  keyframe; cloud upload uses it.

### 83. Dialog fade/zoom

- Status: accepted. In-app confirms stay native `rfd`; `DIALOG_FADE_MS` is unused
  because there is no GPUI dialog surface to fade.

### 84. Select chevron rotation

- Status: done. Opening animates a half turn over 150ms; closing snaps so a first
  paint of a closed chevron does not spin.

Platform limits (accepted, no action unless revisited):

- **Backdrop blur is baked** (12px stand-in; settings sidebar mixes opacity instead
  of `blur(18px) saturate(125%)`). GPUI has no live backdrop-filter.
- **Geist fonts** — Electron `base.css:4-16,115-123`; GPUI uses system/HeroGPUI fonts.
- **Updater auto-downloads** a verified installer and installs from Ready
  (`start_download` / `quitAndInstall`). macOS uses the electron-updater zip
  (`-universal-mac.zip` + `latest-mac.yml`); Linux GPUI uses
  `*-linux-${arch}.tar.gz` + `latest-linux.yml`. Electron Linux stays
  `unsupported` because there is no Electron Linux package.
- **Wayland multi-display capture blocked** (`capture/mod.rs:411-421`) — documented
  limitation, not Electron drift.
- **Linux session matrix** (X11/Wayland/headless gating) is GPUI-only by design.

Non-gaps found during comparison (do not file):

- Overlay loupe exists in both — only Escape semantics differ (item 12).
- Aspect-ratio presets are unwired in BOTH shells (Electron `session.ts:877-888` has
  no non-test callers; GPUI `overlay.rs:1306-1310` hardcodes `None` with
  `selection.rs` geometry ready). Wire once, on both, when the product wants it.
- Crop-undo and TARGET_CLOSED were real gaps at audit time (items 2–3); both are
  done. The "HiDPI export geometry" and "wallpaper preset degradation" claims from
  the early agent pass did not reproduce: export uses natural resolution on both
  (`useCanvasExport.ts` vs `export.rs:27-38`, DPR is preview-only in
  `canvas-renderer.tsx:314`), and wallpaper presets/padding/frames/aspect/balance
  all port (`wallpaper_sheet.rs` vs `wallpaper/index.tsx`).
- Frame-step in GPUI uses the source frame rate (`mod.rs:2535-2537`) vs Electron's
  fixed 1/30 (`use-editor-shortcuts.ts:33`) — GPUI is more accurate; keep.
- `video/transcription.rs` audio conversion is portable Rust, not Media Foundation.

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
