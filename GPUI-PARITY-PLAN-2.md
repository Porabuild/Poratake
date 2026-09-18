# GPUI Parity Plan 2

Second parity pass over the GPUI shell (`src/main/app-gpui/`) against the Electron
shell (`src/renderer/` + `src/main/`), which stays the reference. Scope was every UI
element, every animation or transition, and every interactive behavior on every
surface — read at code level on both sides, then re-checked with a live screenshot
pass of both shells running on macOS 15 (Apple Silicon, 2x display).

Method: six surface reports (settings, screenshot editor, video editor, capture,
small windows, cross-cutting), two adversarial verification passes over those
reports, and one runtime pass. The verification passes override the raw reports:
findings they marked FALSE are dropped, PARTIAL descriptions are the corrected ones,
RESEVERITY grades are applied, and corrected fixes replace the originals. Where a
report and its verification disagree, the verification wins. The runtime pass is
direct evidence and is folded in with its caveats (see "Runtime pass caveats").

Evidence convention: every item cites `file:line` on both sides. When the GPUI side
is genuinely absent the item says "absent" and lists the terms that were searched, so
the claim can be re-tested rather than re-guessed.

Severity: P0 = data loss, dead input, or a broken take that the user cannot work
around. P1 = a missing feature or wrong behavior. P2 = a design, motion, or polish
delta. Items 1–84 belong to `GPUI-PARITY-IMPLEMENTATION-PLAN.md` and are not
renumbered here; this document starts at 85.

| Severity | Count | Range   |
| -------- | ----- | ------- |
| P0       | 2     | 85–86   |
| P1       | 53    | 87–139  |
| P2       | 115   | 140–254 |
| Total    | 170   | 85–254  |

The preview-equals-export rule in `CLAUDE.md` is mandatory and is the reason several
items that read as polish are graded P1: they change the exported file. Fixes must
also respect the repo rules — no code comments, no third-party packages without
approval, Tailwind-equivalent built-in values rather than arbitrary ones, and any new
bundled asset updates `THIRD_PARTY_NOTICES.md` and `electron-builder.json5`
`extraResources` in the same change.

---

## Implementation status (2026-09-17)

Every item in this document has been implemented, or is recorded below and in
"Accepted platform limits" as deliberately not built. The work landed across the
GPUI shell only: no file under `src/renderer/`, `src/preload/`, `src/types/` or the
Electron side of `src/main/` was modified, because Electron is the reference.

Verification at the end of the run: `cargo test -p poratake-gpui` 759 passed and 0
failed, `cargo clippy --all-targets` zero warnings, `cargo fmt --check` clean,
`bun run typecheck` clean, `bun run lint` clean, `bun run format:check` clean, and
`bun run test` 2221 passed with 3 skipped.

Both P0 items are closed: the video editor drawing overlay now exists and produces
annotations that render through the same composition engine the export uses, and the
editor colour picker's hex field is a real input.

Partial or deferred, each with its reason recorded at the item or in the limits
section: 115 (Electron's 600 weight is approximated by the bold face), 140 (HeroGPUI
`PickerItem` has no per-row content hook, so only the trigger carries a swatch), 173
(clip geometry is deliberately unanimated), 180 (complete), 200, 234, 236 and the
separator half of 243 (all upstream or disproven), and 186 (single-window registry
kept as a deliberate simplification).

Three premises in this document were disproven by measurement during implementation
rather than fixed, which is why items 234 and 236 produced no code. Their entries in
the limits section carry the evidence.

## Corrections to the existing audit and plan

These are claims in `GPUI-UI-PARITY-AUDIT.md` or `GPUI-PARITY-IMPLEMENTATION-PLAN.md`
that verification proved false or partial. They are listed here because they are the
reason those two documents cannot be trusted as a closed list.

- Plan item 28 ("Capture-and-attach is a file picker, not live capture", status done) is false for the hold-Cmd half: `src/renderer/windows/screenshot-window.tsx:143,1191-1196,1245` drives `isMetaHeld` into `CaptureEdgeOverlay` at `:1391`, while `src/main/app-gpui/src/editor/window.rs:2446` gates the overlay on `self.capture_mode` only, and a repo-wide grep for `on_modifiers_changed` in `src/main/app-gpui/src/` returns zero hits. The tooltip at `editor/title_bar.rs:193-197` advertises a gesture that does nothing. Tracked as item 124.
- Plan item 40 ("Export shows percent only", status done, "in panel and title bar") is true for the panel and false for the title bar: the ETA math at `windows/video_editor/mod.rs:466-489` is a faithful port, but `windows/video_editor/title_bar.rs:50-81` renders only a bar, a percent and a cancel button, where `src/renderer/components/video-editor/export-progress-indicator.tsx:127-146` shows elapsed, remaining and `Calculating...` inside a popover. Tracked as item 108.
- Audit "Video editor — match: export `ProgressBar`" is false. Electron's title-bar export affordance is a `CircularProgress` SVG ring inside a 28px icon button that opens a 256px popover (`export-progress-indicator.tsx:86-104`, `src/renderer/components/ui/circular-progress.tsx:25-58`); GPUI renders a 64px linear `herogpui::ProgressBar` inline (`title_bar.rs:50-81`). Different component, different layout, no popover, no completion state.
- Audit "Video editor — match: Persist path matches Electron `use-editor-state-persistence`" is partial. The 500ms delay and the field set match, but Electron is a trailing debounce (`src/renderer/components/video-editor/hooks/use-editor-state-persistence.ts:153-160`) while GPUI is a leading-schedule throttle (`windows/video_editor/mod.rs:777-792`), and `saved_at` is never assigned (`windows/video_editor/model.rs:215,262`). Tracked as item 187.
- Audit "Recording control / all-in-one — match: bar widths 236 / 400 / +140" is partially false. The constants are pinned (`ui/chrome.rs:222-226`), but Electron measures the toolbar with a ResizeObserver and resizes the window from the measurement (`src/renderer/windows/recording-control-window.tsx:360-377`, `src/main/capture/video/recording-control-window.ts:86-101,177-187`); GPUI has no measurement path anywhere. Tracked as item 130.
- Audit line 36 ("Accent / danger hover | app mixes | HeroUI stock hover math | Intentional HeroGPUI default"): the cross-cutting report's claim-check declared this row false, and the verifier reversed that check. The row's description of GPUI is correct, but calling it "intentional" hides a real divergence — `theme/vars.rs:118-122,156` computes the Electron accent hover and `theme/bridge.rs:187` never passes it to HeroGPUI, so every HeroGPUI accent control hovers toward the accent foreground, the opposite direction from Electron. Net-new, tracked as item 254.
- The screenshot-editor report's verified-claim note "Cmd+A is a GPUI-only addition; the Electron screenshot editor has no select-all" is false: `src/renderer/hooks/useKeyboardShortcuts.ts:27-38` implements it, wired at `src/renderer/components/editor/editor-canvas.tsx:213`. Cmd+A is genuine parity.
- The small-windows report's verified-claim "brand logo wordmark pixel math holds" is partial: horizontal and size math at `windows/settings/about.rs:61-88` reproduce `src/renderer/components/brand-logo.tsx:24-30` exactly, but the vertical placement uses `items_end` in an `h(29.0)` row with `bottom(px(1.0))` against Electron's `align-baseline` SVG with `overflow-visible` — a different alignment model with an eyeballed compensation. Tracked as item 237.
- The settings report's Matches list says "All 36 shortcut items … the same 35 ids"; the real counts are 36 (Electron) and 37 (GPUI), diff exactly `shortcuts.scrollCapture`. Tracked as item 226.
- The capture report's claim "no popover exit animation anywhere on this surface" is false: `ui/menu/mod.rs:387-399` wraps a closing popup in `crate::ui::primitives::overlay_exit` (`ui/primitives.rs:125-146`), a real `with_animation`, and `ui/menu/mod.rs:371-385` holds the popup alive for the duration. Only the enter animation and the tab indicator are missing. Tracked as item 221.
- The screenshot-editor report's finding "Cloud upload copies the URL and shows a toast; Electron does neither" is FALSE and is dropped: `src/main/cloud/index.ts:270-280` does both in the Electron main process, with the same strings GPUI uses at `editor/window.rs:3500-3512`.
- The capture report's sub-claim "GPUI renders nothing at all until `color_frame` arrives" is false: `src/renderer/components/area-overlay/color-picker.tsx:261-263` returns null until `ready` for exactly the same reason. Only the hint row is a real delta (item 204).
- The cross-cutting report's "dead `ThemeVars` fields" list is wrong: `popover`, `card`, `primary` and `secondary` all have downstream readers. Only `card_foreground`, `secondary_foreground` and `destructive_foreground` have zero reads. Tracked as item 253.
- Plan item 5 ("Area selection hides desktop icons at entry, except Recording") holds as written, but the Electron reference does not hide at entry — `src/main/capture/screenshot/capture-area.ts:92-99` hides inside `captureArea`, after confirm. Tracked as item 219.
- Two doc comments in the GPUI tree assert a shared preview/export path that does not exist and must be deleted with their items: `editor/glyphs.rs:1-3` ("The live canvas and the export rasterizer share it so a badge previews exactly as it is written to the file") — nothing under `render/` references `glyphs` (item 121); and `editor/text_render.rs:1-4` ("the export loads the same family from the OS so a saved image matches what the editor showed") — true only for `sans` on Windows (item 122). A third, `editor/canvas.rs:1101`, describes the selection halo as "A dashed box", which it is not (item 192).

---

## Runtime pass caveats

The live screenshot pass produced direct evidence, but three things could not be
driven, and the implementer must re-check them by hand.

- Background synthetic input (AX and CG events, via `computer_use`) does not reach GPUI windows at all — every attempt reported `delivery.verified: "unchanged"`. So every GPUI interactive state is un-verified at runtime: settings search results, every popover and dropdown, editor tool option rows, crop mode, selected annotations, hover, press and focus transitions. Only idle, first-paint states were compared.
- Electron's transparent windows (capture preview, pin) come back blank from `Page.captureScreenshot`, and its overlay windows are excluded from `screencapture`, so those Electron surfaces have no image. The all-in-one toolbar, the recording control bar, the pin window, the toast and the mid-drag area overlay have GPUI screenshots with no Electron pair.
- Electron could not be run through `bun run dev`: `app.requestSingleInstanceLock()` at `src/main/main.ts:46` loses to the installed `/Applications/Poratake.app`. It was run from the freshly built `dist/` + `dist-electron/` with a separate profile, so its update channel and any packaged-only behavior differ from a real install. Electron shots are CDP page captures, so vibrancy composites against nothing and native traffic lights are absent; only in-content geometry and typography were compared.
- One runtime observation is explicitly inconclusive and is not filed as a finding: the GPUI pre-recording bar showed no target-name chip, but it was opened through `--preview-window recording-control`, which passes `None` for the target name. Re-check with a real window recording before acting.

---

## P0 — fix first

### 85. Video editor preview has no drawing overlay, so the drawing tools cannot create anything

- Type: behavior.
- Electron: `src/renderer/components/video-editor/video-drawing-overlay.tsx` owns pointer capture, live stroke preview and selection, mounted over the canvas at `native-video-player.tsx:1342-1379` and wired to `onAnnotationAdded` at `src/renderer/windows/video-editor-window.tsx:1079-1103`.
- GPUI: the preview is a bare `img` with no mouse handlers (`windows/video_editor/mod.rs:2517-2554`); searched `annotations`, `drawing_overlay`, `VideoDrawingOverlay`, `on_add_drawing` across `src/main/app-gpui/src` — the only hits are the model field (`windows/video_editor/model.rs:100`), the label helper (`model.rs:107`), timeline carry-along (`timeline/edit.rs:26`) and the renderers, so `DrawingSegment::default()` clips added from the timeline stay permanently empty.
- Fix: add `windows/video_editor/drawing_overlay.rs` rendering an absolutely positioned `gpui::canvas` over the preview; build `crate::editor::annotations::Annotation` values (confirm the serde shape against `editor/annotations.rs:3`, which states it serializes exactly what `state.json` carries) and set `DrawingSegment::canvas_width`/`canvas_height` (`model.rs:96-99`) to the composition size the stroke was authored against, or `video/composition/drawing.rs` will mis-scale every stroke on export. Land item 87 in the same change. Folding in here: `redactOnlyDrawings` is hard-coded `false` at `video/composition/mod.rs:625-638`; that parameter is only meaningful once this overlay exists, and Electron never sets it on the export path either, so it is not a separate delta.
- Acceptance: drawing a stroke on the GPUI preview creates an annotation on the selected drawing segment, and a preview raster of that frame matches the exported frame pixel-for-pixel at the same composition size.

### 86. The editor colour picker's hex field is a dead input

- Type: behavior.
- Electron: `src/renderer/components/editor/color-picker/index.tsx:128-138` renders a real editable `ColorField` with a swatch prefix and a hex `ColorField.Input`.
- GPUI: `ui/color_picker.rs:256-279` is a plain `div()` row holding a swatch `div` and a second `div().flex_1()` printing `selected_hex.to_uppercase()`; the file has zero `TextField` references, and the only other affordances are 12 fixed swatches, the 2D area and the hue slider — none of which can land on a specified value.
- Fix: replace the readout `div` with `herogpui::components::TextField` backed by an `Entity<InputState>` held on the `ColorPicker` view (the pattern is already in `windows/settings/mod.rs:26,478`, `editor/window.rs:28,1834` and `editor/wallpaper_sheet.rs:22,402` — `ui/color_picker.rs` must add its own import), parse on commit with the existing `Srgba::parse` / `hsv_from_hex` (`ui/color_picker.rs:105-106`) and call `emit` (`:142`); re-sync the field whenever `self.hsv` changes from the area, slider or shuffle.
- Acceptance: typing a hex value into the picker's field and committing sets the annotation colour to exactly that value.

---

## P1 — Video editor

### 87. Drawing panel style controls never apply to the selected annotation

- Type: behavior.
- Electron: `src/renderer/components/video-editor/drawing-settings-panel.tsx:153-157` resolves the annotation from the timeline clip selection, `:171-177` reads the displayed colour and stroke from that annotation, and every handler calls `applyToSelected` → `onUpdateDrawingAnnotation` (`:187-212`).
- GPUI: `windows/video_editor/panels.rs:653-658` resolves the same selected `DrawingSegment` but uses it only for a label and a delete button (`:672-697`); `drawing_style_rows` (`:732-1010`) reads every value from `tools` and every one of the 13 handler bodies is `this.update_drawing_tools(...)`, with no writer to `annotations` anywhere under `windows/video_editor/`.
- Fix: pass the resolved segment's first annotation into `panels.rs::drawing_style_rows`, read displayed values from it when present, and add `VideoEditorWindow::update_selected_annotation(cx, |annotation| …)` in `mod.rs` called alongside each `update_drawing_tools`; mirror Electron's `configType` fallback (`drawing-settings-panel.tsx:172-173`), which takes the annotation's type rather than the active tool, so which rows appear also switches on selection.
- Acceptance: selecting a drawing annotation shows its own colour, stroke and type-specific rows, and changing any of them mutates that annotation rather than the tool defaults.

### 88. No text editor for a selected text annotation

- Type: behavior.
- Electron: `drawing-settings-panel.tsx:152-168` resolves `selectedAnnotation` and focuses the textarea on a `textFocusNonce`; `:494-506` renders a `Textarea` bound to `selectedAnnotation.text` with `min-h-20 resize-none`.
- GPUI: `windows/video_editor/panels.rs:673-696` pushes only a `kit::label("Selected Drawing")` row and a `trash-2` button; searched `TextArea`, `text_annotation`, `annotation_text`, `textFocusNonce` under `windows/video_editor/` — only `data_editor.rs:256` and `panels.rs:1915` match, both unrelated.
- Fix: add an `Entity<InputState>` for the selected text annotation to `VideoEditorWindow` (same shape as `prompt_field`, `mod.rs:292`), render `herogpui::components::TextArea::new(state).rows(3)` (`herogpui-components-0.9.0/src/textarea.rs:60,179`) in `panels.rs::drawing_panel`, and register the field in `editing_text()` (`mod.rs:2282-2287`). Land with item 85, which is what makes it reachable.
- Acceptance: selecting a text annotation shows its text in an editable field, and edits update the annotation and the composited preview.

### 89. Music track rows reuse constant element ids across tracks

- Type: behavior.
- Electron: `src/renderer/components/video-editor/audio-settings-panel.tsx:75` keys each row by `track.groupId`, so the volume `Slider` (`:105-120`) and the speed control are independent React nodes per group.
- GPUI: `windows/video_editor/panels.rs:1401` and `:1415` pass the literals `"music-volume"` and `"music-speed"` inside `music_row` (`:1317`), which runs once per track, and the enclosing row `div()` (`:1327-1334`) has no `.id()`, so sibling rows share one `GlobalElementId` and therefore share hover and drag state. The switch (`:1368`) and remove button (`:1383`) do namespace correctly.
- Fix: build `format!("music-volume-{id}")` / `format!("music-speed-{id}")` and widen the `panel_kit::slider_row` / `select_row` signatures (`panel_kit.rs:124-125,183-185`) from `&'static str` to `impl Into<ElementId>`; `ui/rows.rs:108-109` and `herogpui-components-0.9.0/src/select.rs:491` already accept that. Not a crash: gpui's duplicate-id assertion (`gpui-pre-0.3.5/src/window.rs:4125`) only fires on reentrant `with_element_state`.
- Acceptance: dragging the volume slider on one music track leaves every other track's slider untouched.

### 90. The project info popover has no GPUI counterpart

- Type: behavior.
- Electron: `src/renderer/components/video-editor/project-path-indicator.tsx:111-205` — a 28px ghost `FolderOpen` trigger with the literal tooltip `Project Info`, opening a `w-80` popover holding a rename `Input` with a `Save` disabled until changed, an inline `text-destructive` rename error (`:161-163`), a read-only path `Input`, `Copy Path` with a 2s green `Check` (`:175-186`), and `Show Original` (`:187-196`). Mounted at `video-title-bar.tsx:67-72`; the rename handler returns an error string for inline display (`src/renderer/windows/video-editor-window.tsx:485-513`).
- GPUI: `windows/video_editor/title_bar.rs:41-47` is a single `folder-open` button whose tooltip is the raw path and whose click calls `reveal_project` (`mod.rs:1990-1995`); rename is a separate affordance on the filename button (`title_bar.rs:154-198`) and failures go out as `Toast::show(cx, "Rename failed", ...)` (`mod.rs:2023`), which forwards to an OS notification (`windows/toast.rs:17-18`). Searched `copy_path`, `Copy Path`, `clipboard`, `write_to_clipboard`, `show original`, `project info` under `windows/video_editor/` — no hits.
- Fix: build the popover on the overlay this window already renders, `crate::ui::menu::MenuHandle` (`mod.rs:41,2746`; `Menu::toggle_with` + `MenuPlacement::below(..).aligned_right()`, `ui/menu/mod.rs:72,103,305`), the way `editor/title_bar.rs:388-412` does for the colour picker, rather than introducing `herogpui::components::Popover` here; reuse `rename_field` (already an `Entity<InputState>` driving `TextField` at `title_bar.rs:171`), render the rename error with `crate::ui::rows::error` (`ui/rows.rs:53`), reuse `reveal_project` for Show Original, and copy through `crate::system::clipboard::ClipboardService::write_text(cx, path)` (`system/clipboard.rs:9-11`) rather than `cx.write_to_clipboard` directly.
- Acceptance: the folder button opens a popover with the project name, the full path, a working Copy Path that acknowledges for 2s, Show Original, and an inline rename error on failure.

### 91. Escape does not close the data-editor dialog, and there is no click-outside dismiss

- Type: behavior.
- Electron: `cursor-data-editor-dialog.tsx:112-114` and `subtitle-data-editor-dialog.tsx:99-101` use a Radix `Dialog`, which dismisses on Escape and on outside pointer-down and adds a close X.
- GPUI: `windows/video_editor/data_editor.rs:176-300` has no `on_key_down` or `on_mouse_down` anywhere; the backdrop (`:185-191`) and the card (`:192-203`) carry no handlers, and the only Escape branch (`mod.rs:2299-2308`) is unreachable because `on_key` returns at `mod.rs:2290-2292` whenever `editing_text()` is true — which includes `data_editor.field`. Cancel and Save are the only exits.
- Fix: handle `escape` in `mod.rs::on_key` before the `editing_text()` early return when `self.data_editor.is_some()` (gpui bubbles key events up the focus path, so an ancestor handler fires while the `TextArea` holds focus), and add `.on_mouse_down(MouseButton::Left, …)` on the backdrop with `cx.stop_propagation()` on the card.
- Acceptance: Escape and a click on the backdrop both close the data-editor dialog without saving.

### 92. Sliders ignore the Electron step ladder

- Type: behavior.
- Electron steps: `zoom-settings-panel.tsx:247` (`ZOOM_LEVEL_STEP`), `:268` `0.1`, `:291` and `:310` `0.02`; `cursor-settings-panel.tsx:282` `5`, `:299` `0.1`, `:338` `0.05`, `:393` `0.5`; `audio-settings-panel.tsx:121` and `:203` `1`; `drawing-settings-panel.tsx:309` `1`.
- GPUI: `panel_kit.rs:218` hard-codes `0.0` as the step into `rows::slider_control`, and `ui/rows.rs:116-127` only quantizes when `step > 0.0`; uncompensated handlers at `panels.rs:404-406,426-431,444-448,459-461,176,202-204,252-254,1401-1411`.
- Fix: add a `step: f64` parameter to `panel_kit::slider_row` and pass it through to the existing quantizer, then set each call site to the Electron value; remove the ad-hoc `.round()` compensations such as cursor size (`panels.rs:165`, which quantizes to 1 where Electron uses 5) in favour of the new argument rather than leaving both.
- Acceptance: each slider lands only on the Electron step values for its control.

### 93. Rail tab click toggles the sidebar closed where Electron always opens

- Type: behavior.
- Electron: `src/renderer/windows/video-editor-window.tsx:150-153` `activateSidebarTab` sets the tab and forces `setIsSidebarOpen(true)`, wired to the rail at `:1412-1414` and to the shortcuts.
- GPUI: `windows/video_editor/sidebar.rs:183-185` calls `select_tab`, and `mod.rs:886-892` closes the sidebar when the active tab is re-clicked, while the keyboard path uses `activate_tab` (`mod.rs:2389`, `:2314`) — so rail and keyboard disagree.
- Fix: change `sidebar.rs:184` to `this.activate_tab(tab, cx)` and make `activate_tab` visible to `sidebar.rs` (currently private, `mod.rs:924`); `select_tab` (`mod.rs:886-899`) then has no caller — the title-bar button goes through `toggle_sidebar` (`title_bar.rs:116-129` → `mod.rs:901-906`) — so delete it rather than keeping it.
- Acceptance: clicking the active rail tab leaves the sidebar open; only the title-bar toggle closes it.

### 94. Custom cursor thumbnail is missing

- Type: visual.
- Electron: `cursor-settings-panel.tsx:231-249` renders a `size-10` bordered, rounded box containing `<img src={cursorStyle.customCursorImage} className="max-h-full max-w-full object-contain" />` beside the `Remove` button.
- GPUI: `windows/video_editor/panels.rs:120-135` pushes only a `Remove` field and a `Using custom cursor image` hint; no image element.
- Fix: in `panels.rs::cursor_panel`, lead the row with `gpui::img(PathBuf::from(path)).size(px(40.)).object_fit(gpui::ObjectFit::Contain)` inside a `rounded(px(4.)).border_1()` box. `style.custom_cursor_image` holds a filesystem path, not a data URL (`mod.rs:1709-1712`, `video/composition/cursor.rs:334`), so `img(PathBuf)` works; copy the structure of `first_frame_panel` (`panels.rs:2140-2152`) but not its `ObjectFit::Cover`.
- Acceptance: with a custom cursor image set, the panel shows a 40px contained thumbnail of that file beside Remove.

### 95. Camera and music clip gradients are swapped

- Type: visual.
- Electron: `timeline/camera-track.tsx:87` uses `colors="pink"` → `['#f472b6','#be185d']` (`timeline/track-colors.ts:25-29`); `timeline/music-track.tsx:78` uses `TRACK_COLORS.purple` → `['#c084fc','#7e22ce']` (`track-colors.ts:19-23`).
- GPUI: `windows/video_editor/timeline/tracks.rs:81` maps Camera to the purple pair and `:83` maps Music to the pink pair, and the same literals are repeated at `mod.rs:2431` and `mod.rs:2459`.
- Fix: swap the two arms in `tracks.rs::TrackKind::gradient`, make it `pub(crate)`, delete the literal tuples in `mod.rs::tracks()` and pass `|_| TrackKind::Camera.gradient()` into `range_clips` (`mod.rs:2426-2432`) so the colour lives in one place.
- Acceptance: camera clips render pink and music clips render purple, matching the Electron timeline.

### 96. Timeline pane is not resizable and does not persist its height

- Type: behavior.
- Electron: `src/renderer/windows/video-editor-window.tsx:965-986` sets MIN 3 / MAX 12 / DEFAULT 5 tracks plus a 12px scrollbar and uses `useResizablePane({storageKey:'video-editor:timeline-height', axis:'vertical'})`; the handle at `:1123-1139` is a `role="separator"` `h-1.5 cursor-ns-resize` strip with `bg-primary/40` while dragging and an inner `h-0.5 w-8 rounded-full transition-colors` grip; persistence at `src/renderer/hooks/use-resizable-pane.ts:54-56`.
- GPUI: `mod.rs:2616-2623` fixes the height from `chrome::video_timeline_tracks_height(timeline::TRACK_HEIGHT)` with `VIDEO_TIMELINE_TRACKS = 5` (`ui/chrome.rs:171-172,423-425`); searched `timeline_height`, `resize_handle`, `ns_resize` — only the sidebar handle (`sidebar.rs:111-144`) matches.
- Fix: add `timeline_height: f32` and `timeline_resize: Option<f32>` to `VideoEditorWindow`, mirror `sidebar::resize_handle` (`sidebar.rs:111-144`, `begin_sidebar_resize`) with `cursor_ns_resize()`, clamp to `[3,12] * TRACK_HEIGHT + 12`, and persist it in `EditorUiState` the way `sidebar_width` already is.
- Acceptance: dragging the divider resizes the timeline within the 3–12 track bounds, and the height survives closing and reopening the editor.

### 97. Hover-to-scrub is missing; the timeline only scrubs while a button is held

- Type: behavior.
- Electron: `timeline/timeline-tracks.tsx:9` `SCRUB_STEP = 1/120`, `:48-86` `computeAndSeek` (bails on `isPlaying || isTrimming || !isHovering`, quantizes, calls `onPreviewSeek`), `:89-96` rAF scheduling, `:105-112` mouse-move, `:118-132` mouse-leave clearing the preview. The hint "Hover to scrub" is at `timeline-controls.tsx:242`.
- GPUI: `timeline/tracks.rs:526-531` and `timeline/ruler.rs:104-109` both return unless `event.dragging()`, while the identical hint string ships at `timeline/controls.rs:18`; searched `preview_seek`, `preview_playhead`, `hover_playhead` across `src/main/app-gpui/src` — zero hits.
- Fix: in `tracks.rs::render`, handle non-dragging `MouseMoveEvent`, quantize to 1/120 and call a new `VideoEditorWindow::preview_seek(Option<f64>)` feeding `request_frame`; clear it on `on_mouse_exit` (`gpui-pre-0.3.5/src/elements/div.rs:319`), not `on_mouse_out`, and bail while `is_playing` or `is_dragging_clip()` to mirror Electron's `isTrimming` guard. No extra bounds check is needed — gpui's move listener already gates on `hitbox.is_hovered(window)` (`div.rs:307-312`). This also makes the scrub-audio path of plan item 37 reachable.
- Acceptance: moving the pointer over the timeline without pressing a button updates the preview frame, and leaving the timeline restores the playhead frame.

### 98. No wheel or pinch zoom and no wheel scroll on the timeline

- Type: behavior.
- Electron: `timeline/timeline-tracks.tsx:11,141-213` — a ctrl/meta branch that anchors zoom at the pointer, clamps to `MIN/MAX_PIXELS_PER_SECOND` and re-anchors `container.scrollLeft` inside a rAF; a shift or `|deltaX|>|deltaY|` branch for horizontal scroll; and a plain vertical branch scrolling the `#timeline-container` ancestor.
- GPUI: searched `on_scroll_wheel`, `ScrollWheelEvent`, `scroll_wheel` across `src/main/app-gpui/src` — hits only in `editor/window.rs:2582` and `editor/zoom_fit.rs:147,169` (the image editor), zero under `windows/video_editor/`. Timeline zoom is only the slider, the buttons (`timeline/controls.rs:148-180`) and Cmd+/−/0 (`mod.rs:2317-2319`).
- Fix: add `.on_scroll_wheel(cx.listener(…))` on the tracks scroll div (`Div::on_scroll_wheel` at `gpui-pre-0.3.5/src/elements/div.rs:390`); on `modifiers.platform || modifiers.control` compute `pixels_per_second * (1 - delta.y * 0.01)`, clamp, then re-anchor with `ScrollHandle::set_offset` (`div.rs:4394-4398`) — note gpui scroll offsets are negative as you scroll right, so the re-anchor is `set_offset(point(-new_scroll_left, offset.y))`.
- Acceptance: ctrl/cmd + wheel zooms the timeline around the pointer, shift + wheel scrolls horizontally, and plain wheel scrolls the lane stack.

### 99. Ruler and tracks do not scroll together, and the ruler ignores the music-extended duration

- Type: behavior.
- Electron: `timeline/timeline-ruler.tsx:52-60` pushes the ruler's `scrollLeft` into the tracks and `timeline/timeline-tracks.tsx:98-105,134-139` pushes back; `timeline-ruler.tsx:43-44` uses `displayDuration = max(totalDuration, minDisplayDuration)`, fed from `video-editor-window.tsx:1169-1172,383-398` where the minimum is the music end over enabled music tracks, and the same value goes to `TimelineTracks` (`:1218-1220`).
- GPUI: two independent handles `ruler_scroll` / `tracks_scroll` (`mod.rs:117-118,264-265`) passed separately to `timeline::ruler::render` (`mod.rs:2588`) and `timeline::tracks::render` (`mod.rs:2582-2591`), both containers independently scrollable, with no mirroring (`set_offset` has zero hits). Width comes from `total_duration * pixels_per_second` (`ruler.rs:38`, `tracks.rs:416`) and `total_duration` (`mod.rs:701-708`, `model.rs:351-356`) never considers music `end_time`, so music past the video end is off-canvas.
- Fix: compute `display_duration = total.max(enabled music end)` in `mod.rs::render` and pass it to both `ruler::render` and `tracks::render`; for the sync half, keep both handles and mirror `x` inside the wheel/scroll listener added for item 98 (`other.set_offset(point(offset.x, other.offset().y))`) rather than sharing one handle — `ScrollHandleState` also stores `bounds`/`child_bounds`/`max_offset`, which the 28px ruler row and the taller tracks row would overwrite for each other, and `timeline/mod.rs:49-59` reads `scroll.bounds()`.
- Acceptance: scrolling either lane moves the other in lock-step, and a music track extending past the video end is reachable on both.

### 100. Trim handles have no visible grip, no hover state, and the wrong hit size

- Type: visual.
- Electron: `timeline/track.tsx:425-434` and `timeline/timeline-track.tsx:114-126` each render two `absolute h-full w-3 cursor-ew-resize bg-transparent transition-colors hover:bg-white/20` overlays containing an `h-4 w-1 rounded-full bg-white/40` pill; the hit test is time-based, `min(segmentDuration * 0.15, 0.3)` (`track.tsx:65-66,158-161`).
- GPUI: `timeline/tracks.rs:29` `RESIZE_HANDLE = 6.0`, chosen purely from the grab pixel inside the clip's own mouse-down (`:233-262`); `clip_element` adds no child overlays, no cursor and no hover. Searched `cursor_ew_resize` under `windows/video_editor/` — only the sidebar splitter (`sidebar.rs:125`).
- Fix: add two absolutely positioned 12px children to `tracks.rs::clip_element` with `cursor_ew_resize()`, a `bg(crate::ui::colors::white(0.4))` grip pill and `hover(|el| el.bg(white(0.2)))`, widen `RESIZE_HANDLE` to 12, keep the time-based `min(duration*0.15, 0.3)` clamp for short clips, and call `cx.stop_propagation()` in the handle mouse-down so the parent clip handler does not also fire.
- Acceptance: hovering a clip edge shows the ew-resize cursor and a lit grip, and resizing starts from anywhere in the 12px zone.

### 101. Drag-to-reorder video clips and its drop indicator are missing

- Type: behavior.
- Electron: `src/renderer/components/video-editor/hooks/use-reorder-drag.ts:11,50-70,126-152` — a 5px threshold, a scroll-compensated drop index, commit on mouse-up and capture-phase click suppression; the dragged clip drops to `opacity 0.4` (`track.tsx:403,421`), cut markers hide while reordering (`timeline-track.tsx:159`), and the drop indicator is a `w-0.5 bg-white shadow-[0_0_6px_rgba(255,255,255,0.6)]` rule (`timeline-track.tsx:182-187`).
- GPUI: searched `reorder`, `drop_index`, `drag_threshold`, `drop_indicator` across `src/main/app-gpui/src` — only `reorder_selected_segment` (`mod.rs:1147`) and its Alt+←/→ bindings (`mod.rs:2328-2329`). `update_clip_drag`'s `TrackKind::Video` arm handles only resize; `DragMode::Move => {}` (`mod.rs:1923-1931`), so a body drag is a no-op.
- Fix: add a `ReorderDrag { id, drop_index }` to `VideoEditorWindow`, compute the drop index from segment mid-points in the `DragMode::Move` arm, render the 2px white indicator in `tracks.rs::render`, and commit in `end_clip_drag` (`mod.rs:1937-1957`, which already special-cases `TrackKind::Video`) through the existing `reorder_selected_segment` logic.
- Acceptance: dragging a video clip past a neighbour shows the drop indicator and reorders the segments on release.

### 102. Drag-to-draw new clips is missing, and a click adds a 1.5s clip instead of 3s

- Type: behavior.
- Electron: `timeline/track.tsx:209-216` starts a draw drag on an empty area of a `canDraw` lane, previews it as a `border-2 border-dashed` box over a 40%-alpha gradient with a 2px minimum width (`:481-491`), and commits at `:323-340` — under a `CLICK_THRESHOLD = 0.1` it inserts `DEFAULT_SEGMENT_DURATION = 3` clamped to the total, otherwise the dragged range gated by `MIN_SEGMENT_DURATION = 0.3` (`:61-63`).
- GPUI: `timeline/tracks.rs:32` `ADDED_CLIP_DURATION = 1.5`, applied immediately in the lane's mouse-down (`:471-473`), with no drag state and no preview element.
- Fix: add a `DrawDrag { kind, start, end }` to the window, start it in the lane mouse-down when `can_add_to(kind)`, update from the lane `on_mouse_move`, render the dashed preview in `tracks.rs::render`, and commit in `end_clip_drag` using Electron's click/drag branch with the 3s default and the 0.3s minimum.
- Acceptance: a click on an empty zoom lane adds a 3s clip; a drag adds a clip spanning the dragged range, refused below 0.3s.

### 103. Minimum clip duration on resize is 0.1s instead of 0.3s

- Type: behavior.
- Electron: `timeline/track.tsx:61` `MIN_SEGMENT_DURATION = 0.3`, applied on both edges at `:280-296`, and music resizes through the same `<Track>` (`music-track.tsx:131-147`). The 0.1 value is only the split floor (`timeline-split.ts:8`).
- GPUI: `timeline/edit.rs:216-231` `resize_range` uses `T::minimum_duration()`, whose default is `MIN_SPLIT_DURATION = 0.1` (`edit.rs:9,33-35`) with the only override also 0.1 (`edit.rs:10-11,93-95`), so zoom, camera and music all resize down to 0.1s.
- Fix: introduce `MIN_RESIZE_DURATION: f64 = 0.3` in `edit.rs` and use it in `resize_range`, keeping `minimum_duration()` for `split_ranges`. Video is unaffected — it goes through `edit::trim_video` with `MIN_VIDEO_TRIM_DURATION = 0.5` (`edit.rs:13`).
- Acceptance: resizing a zoom, camera, drawing or music clip stops at 0.3s.

### 104. One context menu is used for every track kind

- Type: behavior.
- Electron: per-kind menus — zoom has a Zoom Level submenu, `Apply zoom to All` disabled at one segment, Delete and Delete Others (`timeline/zoom-track.tsx:158-209`); camera has Delete only (`camera-track.tsx:113-126`); drawing has Delete only (`drawing-track.tsx:171-178`); music has a Speed submenu over `PLAYBACK_SPEED_PRESETS` with the current one marked and `Remove` only when the group's source is `music` (`music-track.tsx:125,162-193`); the video lane has no context menu at all.
- GPUI: one `clip_menu` (`timeline/tracks.rs:288-391`) with a zoom-only prefix and then `Cut at Playhead`, `Cut All Tracks`, `Delete` and `Delete Others` for every kind, invoked from a generic right-mouse-down on every clip (`:266-283`) including video and music. No music speed submenu, no `Remove` gating, no empty-lane placeholder item.
- Fix: branch `clip_menu` on `TrackKind`; drop the right-click handler from the video lane. The named `set_music_group_speed` does not exist — GPUI has per-track `VideoEditorWindow::set_music_speed(id, speed)` (`mod.rs:1685`) and no group concept yet (item 105), so the Speed submenu must either apply `set_music_speed` to every track sharing the clip's `group_id` or land together with per-group lanes.
- Acceptance: right-clicking a clip shows exactly the items its Electron counterpart shows, and right-clicking a video clip shows no menu.

### 105. Music lanes are collapsed into one lane and disabled groups are still drawn

- Type: visual.
- Electron: `src/renderer/windows/video-editor-window.tsx:729-733` builds `enabledMusicTrackGroups` from tracks with `enabled`, renders one gutter `TrackRow` per group with `SOURCE_ICONS[group[0].source]` (`:1205-1215`) and one `<MusicTrack>` per group (`:1299-1319`). A disabled group is not drawn at all, which makes `DISABLED_COLORS` (`music-track.tsx:40-44`) dead code on the timeline.
- GPUI: one `TrackKind::Music` lane over all `state.music_tracks` regardless of `group_id` or `enabled` (`mod.rs:2450-2463`), one hardcoded gradient and one fixed `volume-2` gutter icon (`tracks.rs:66-75,437`); `MusicTrack::enabled` (`model.rs:131-132`) is never read by the timeline.
- Fix: make `tracks()` emit one `Track` per `group_id` over enabled tracks only, dropping disabled groups entirely as Electron does, and carry the group's `source` into `Track` so the gutter can choose `volume-2` / `mic` / `music`. Do not add a grey disabled gradient — that would be a GPUI-only behavior.
- Acceptance: each enabled music group has its own lane with its own source icon, and disabling a group removes its lane.

### 106. Changing the export format does not renormalize resolution, quality or frame rate

- Type: behavior.
- Electron: `export-settings-panel.tsx:425-437` rewrites resolution, quality preset and frame rate from `FORMAT_CONFIGS[value]` (`src/types/video.ts:109-126`: mp4 → `4k`/`studio`/`30`, gif → `720p`/`web`/`20`), and the `normalized` memo plus the effect at `:385-415,463-481` re-normalize any out-of-list value on every change, not only on mount.
- GPUI: the format select writes only `settings.format` (`panels.rs:2244-2254`) and `update_export` is a blind mutate (`mod.rs:2262-2268`); `ExportSettings::default` is `original`/`studio`/`60` (`styles.rs:284-303`). The option lists are filtered (`panels.rs:2211-2231`), and `rows::selected_value` (`ui/rows.rs:101-106`) returns `None` for an absent value, so `Select` falls back to its `"Select an item"` placeholder — the visible symptom is a blank Resolution or Frame Rate field, not a stale one. The substantive export delta is the frame rate: `export::frame_rate` parses the raw string (`video/export.rs:127-129,173`), so a GIF exports at 60fps where Electron's list caps at 50. `gif_dimensions(.., "original")` is not a problem — `gif_width` falls through to 1280 (`export.rs:86-92`), the same as `720p`.
- Fix: clamp inside `VideoEditorWindow::update_export` (`mod.rs:2262`) after mutating and once in `VideoEditorWindow::new` after `load_state`, mirroring Electron's full `setFormat` semantics (a format switch resets all three fields to that format's defaults, not only the out-of-range ones); add `MP4_DEFAULTS`/`GIF_DEFAULTS` next to `MP4_RESOLUTIONS`/`GIF_RESOLUTIONS` in `styles.rs`, and fix `ExportSettings::default` to Electron's mp4 defaults of `4k`/`studio`/`30`.
- Acceptance: switching mp4 → gif sets 720p / web / 20 and leaves no blank select; the exported GIF's frame rate is one the Electron list offers.

### 107. Cancelling an export reports it as a failure

- Type: behavior.
- Electron is silent on cancel: `src/renderer/components/video-editor/hooks/use-video-export.ts:331,360-373,400,404-408` gate every report on `!exporter.isCancelled()`, and `handleSaveCancelled` (`:326-329`) just resets progress.
- GPUI: `video/export.rs:255-256,311-313` both return `Err("the export was cancelled")`, and `windows/video_editor/mod.rs:384-391` maps every `Err` to `("Export failed", error)` and shows the toast unconditionally, so the user's own cancel produces an "Export failed / the export was cancelled" notification.
- Fix: add a dedicated `Cancelled` error variant to `export::run` and match it in the completion closure so the toast is skipped without the closure having to capture the `AtomicBool`. The cloud-upload half needs no change — the upload already runs off `result.ok()` (`mod.rs:392-399`), which is `None` on cancel.
- Acceptance: cancelling an export shows no failure notification and leaves no error state behind.

### 108. Title-bar export UI is a different component with no popover and no completion state

- Type: visual.
- Electron: `export-progress-indicator.tsx:85-104` is a 28px `size-7!` ghost/tertiary icon button holding a 16px `CircularProgress` with `strokeWidth 2` (track `stroke-muted-foreground/30`, indicator `stroke-foreground`, `src/renderer/components/ui/circular-progress.tsx:38-53`), or a filled `bg-primary` circle with a `Check` when complete (`:36-52`, held for `COMPLETION_DISPLAY_MS = 3000`, `:18`); it opens a `w-64` popover with `Exporting...`, the rounded percent, an `h-1.5` progress bar, elapsed and remaining, a full-width `Cancel`, and an `Export Complete` row (`:106-160`). Mounted at `video-title-bar.tsx:73-79`.
- GPUI: `windows/video_editor/title_bar.rs:48-81` renders an always-expanded `rounded_full` pill with a 64px `herogpui::ProgressBar` forced to 8px, an 11px percent label and an `x` button; no popover, no elapsed or remaining, and completion clears the pill unconditionally (`mod.rs:392-397`). Searched `Popover`, `circular`, `CircularProgress`, `Export Complete`, `export_completed` across `src/main/app-gpui/src` — only `ui/color_picker.rs::ColorPickerPopover` and a comment in `render/canvas.rs`.
- Fix: rebuild the affordance as a 28px icon button with a `gpui::canvas` ring (`gpui-pre-0.3.5/src/elements/canvas.rs:10`, `PathBuilder::stroke` + `arc_to` at `path_builder.rs:88,155`, both already used in `ui/icon.rs:6,136`), and put the detail in the window's existing `crate::ui::menu::MenuHandle` overlay (`mod.rs:41,2746`, `ui/menu/mod.rs:72,103,267,305`) as `editor/title_bar.rs:388-412` does, rather than introducing `herogpui::components::Popover` here; add a completion state held for 3000ms. This closes the false half of plan item 40.
- Acceptance: during an export the title bar shows a ring button opening a popover with percent, elapsed, remaining and Cancel, and on completion a check that clears after 3s.

### 109. Export settings changes are pushed onto the undo stack

- Type: behavior.
- Electron: export settings live in `use-video-export.ts:75-77` local state and are absent from the history-tracked slice set (`hooks/use-editor-history.ts:23-36`); they are only persisted, restored and handed to the panel (`video-editor-window.tsx:310,888-889,1394-1395`).
- GPUI: `mod.rs:2262-2268` `update_export` calls `self.commit`, and `commit` (`mod.rs:713-736`) snapshots the whole `VideoEditorState`, pushes it onto `self.history` and clears `self.future` — so Cmd+Z after changing the resolution undoes the dropdown and wipes the redo stack. Because entries are whole-state snapshots, undoing an unrelated earlier edit also silently reverts every export-settings change made after it.
- Fix: add a `commit_without_history` next to `commit` that mutates `self.state.export_settings`, early-returns on a no-op, then calls `self.persist(cx)` and `cx.notify()`; it must not call `sync_preview_state` or `request_audio_sync` and must leave `gesture_snapshot` alone.
- Acceptance: changing any export setting leaves the undo and redo stacks untouched, and an undo of an earlier edit preserves the current export settings.

### 110. Deleting every camera segment leaves the camera bubble on for the whole video

- Type: behavior.
- Electron: `composition/camera-canvas-renderer.ts:457-462` returns early when `cameraVisibleRanges` is present and `isCameraVisibleAt` is false, and `src/types/camera.ts:58-67` returns true only for a null or undefined array — so an empty array hides the bubble everywhere. The editor passes `editorData.cameraData ? cameraControl.cameraSegments : null` into both preview and export (`video-editor-window.tsx:328-330,435`).
- GPUI: `video/composition/mod.rs:526-527` collapses the empty case to `None` with `(!camera_segments.is_empty()).then_some(...)`, and `video/sidecars.rs:293-296` treats `None` as always-visible. The helper is a faithful port; the caller is not.
- Fix: `render_camera` is only reached when a camera frame decoded (`mod.rs:261-263`), i.e. exactly when the project has camera data, so `mod.rs:526-527` can unconditionally pass `Some(self.config.state.camera_segments.as_slice())`. No extra has-camera flag is needed.
- Acceptance: deleting every camera clip removes the bubble from both the preview and the exported file; a preview frame and the exported frame at the same timestamp match pixel-for-pixel.

### 111. GPUI ignores the audio-track timeline model and mixes system and mic from the legacy style

- Type: behavior.
- Electron: `hooks/use-music-tracks.ts:97-143` creates System Audio and Microphone as real `MusicTrack`s with `fileName: ''`, `source: 'system' | 'mic'` and their own placement, trim and speed; the exporter resolves them through that model (`export/webcodecs-exporter.ts:803-845`), and the legacy `audioStyle` branch (`:766-786`) is unreachable from the editor. The tracks are persisted (`use-editor-state-persistence.ts:123`) and restored (`video-editor-window.tsx:865-874`).
- GPUI: `video/export.rs:456-507` `build_audio` adds system and mic straight from `state.audio_style` (`:475-484`) and skips any music track with an empty `file_name` (`:488`) — which is exactly how every built-in track is marked; `video/preview_audio.rs:168-188` does the same. Searched `built_in`, `builtin`, `"System Audio"` across `src/main/app-gpui/src` — no hits. The tracks are deserialized (`model.rs:246`) and silently ignored.
- Fix: port `buildBuiltInMusicTracks` / `mergeBuiltInMusicTracks` into `windows/video_editor/model.rs` and make `export::build_audio` and `preview_audio::build_stems` resolve `track.source` to `project::system_audio_path` / `project::mic_audio_path` / the music folder, running every track through `place_music_track`; drop the `audio_style` system and mic path entirely in the same change, or enabled built-in tracks would be mixed twice.
- Acceptance: trimming or moving the System Audio track on the timeline changes both preview playback and the exported audio identically.

### 112. Preview audio ignores a music track's enabled flag while the export honours it

- Type: behavior.
- Electron: the preview and the exporter both run off the same `musicTracks` model with `enabled` respected on each (`export/webcodecs-exporter.ts:828-845`, `hooks/use-music-playback.ts`).
- GPUI: `video/export.rs:488` skips disabled tracks while `video/preview_audio.rs:176-188` never checks `track.enabled`, so a muted music track is silent in the exported file and audible in the preview.
- Fix: add the same `track.enabled` guard to `preview_audio::build_stems` that `export::build_audio` already applies, and land it with item 111 so both read the audio model through one path.
- Acceptance: muting a music track silences it in the preview and in the export, and the two audio renders of the same project match sample-for-sample.

### 113. Keyboard click sound is never rendered into the export or the synced preview

- Type: behavior.
- Electron: `hooks/use-video-export.ts:242-282` generates the click track and `export/webcodecs-exporter.ts:786-793` pushes it into `enabledAudioTracks` at `keyboardSoundVolume` with `skipSegmentExtraction: true`; `hooks/use-keyboard-sound.ts:73-120` plays it live in the preview.
- GPUI: grepping `keyboard` in `video/export.rs`, `video/preview_audio.rs` and `video/audio.rs` yields exactly one hit — `export.rs:175`, which loads the on-screen keyboard pill overlay, not audio. The sidebar still exposes enable, type and volume (`panels.rs:1229-1287`) and the only playback is a 5s settings demo loop (`mod.rs:2848-2875`).
- Fix: add a keyboard stem to `export::build_audio` and `preview_audio::build_stems` — map each `down` event's video time to timeline time through the inverse of `segments::map_timeline_to_video_time` and mix the bundled `public/sounds/keyboard/<type>/press-N.mp3` samples at `audio_style.keyboard_sound_volume`.
- Acceptance: with keyboard sound enabled, the preview and the exported file both carry clicks at the recorded key-down times, and the two audio renders match.

### 114. A recording with embedded audio and no `system.m4a` exports silent

- Type: behavior.
- Electron: `export/webcodecs-exporter.ts:805-809` falls back to the source video path when there is no separate system stem and the source has embedded audio; the built-in track is created for `systemAudioPath || hasEmbeddedAudio` and renamed to `Audio` in the embedded-only case (`use-music-tracks.ts:106,110`), and the preview gets `embeddedAudioPath` at `video-editor-window.tsx:276`.
- GPUI: `video/export.rs:475-479` and `video/preview_audio.rs:170-171` decode `project::system_audio_path` only, always `<project>/system.m4a` (`video/project.rs:77-79`); a case-insensitive repo-wide grep for `embedded` in `src/main/app-gpui/src` returns only unrelated hits in `update.rs` and `ui/app_icon.rs`.
- Fix: add an embedded fallback in `video/project.rs` (the recording video path when `system.m4a` is absent) and use it from `export::build_audio` and `preview_audio::build_stems`, conditional on the source actually carrying an audio stream — `VideoDecoder` is already opened in `export::run` (`export.rs:161-163`) and can supply that flag, so a silent recording does not pay for a wasted decode.
- Acceptance: a recording whose audio is muxed into the video plays and exports with sound.

### 115. Subtitles and keyboard pills render at the wrong font weight and family

- Type: visual.
- Electron: `composition/subtitle-canvas-renderer.ts:148` and `composition/keyboard-canvas-renderer.ts:160` both set `ctx.font = "600 {size}px -apple-system, BlinkMacSystemFont, 'SF Pro Display', sans-serif"`, and those metrics drive the caption box width, the wrap points and every pill width (`subtitle-canvas-renderer.ts:152,184-189,228`, `keyboard-canvas-renderer.ts:165-170`).
- GPUI: `video/composition/subtitle.rs:29` and `video/composition/keyboard.rs:15` both declare `FONT_FAMILY = "sans"`, used for every measure and fill; `render/text.rs` exposes only `measure(text, family, size)` and `fill_text(...)` with no weight axis, and `editor/text_render.rs:12-23` maps `"sans"` to regular faces only, with `SFNS.ttf` rejected at load (`:62-69`) because fontdue cannot rasterize its CFF2 outlines.
- Fix: fontdue has no synthetic emboldening, so weight can only be a different face — add a `"sans-semibold"` entry to the `CANDIDATES` table in `editor/text_render.rs` (`seguisb.ttf`, `arialbd.ttf`, `/System/Library/Fonts/Supplemental/Arial Bold.ttf`, `DejaVuSans-Bold.ttf`) and point both `FONT_FAMILY` constants at it; `font_for` (`text_render.rs:76-84`) already falls back to `"sans"` when the bold face is missing. Do not add `HelveticaNeue.ttc` bold unless the loader also learns `FontSettings { collection_index, .. }` — `Font::from_bytes` defaults to index 0 for a `.ttc`. Apply the matching weight wherever the same text is drawn on the GPUI preview side.
- Acceptance: subtitle and keyboard-pill glyph weights, box widths and wrap points match Electron, and the GPUI preview and export agree on all three.

### 116. Export composes at full composition size and then bitmap-rescales

- Type: visual.
- Electron: `export/webcodecs-exporter.ts:425-446` creates the output canvas at the export size and pre-scales the context, so every layer is rasterized at the target resolution; `export/export-dimensions.ts:11-56` takes the target height straight from `RESOLUTION_MAP`, so the scale can be greater than 1 as well as less.
- GPUI: `video/export.rs:385-399` renders at the full composition size and then `scale_to` (`:420-432`) resamples the finished bitmap with `Canvas::draw_pixmap` at `FilterQuality::Bilinear` (`render/canvas.rs:286-303`). Downscaling 4K to 720p undersamples, so caption glyphs, cursor sprites and shadow edges alias where Electron's are clean; upscaling stretches an already-rasterized bitmap into a visibly blurry export. The GPUI idle preview composes at full size, so it shows neither artefact.
- Fix: build the canvas at `dimensions.width/height` in `export::compose_pixmap`, call `canvas.set_shadow_scale(s)` and `canvas.scale(s, s)` with `s = dimensions.width / composition_width`, then `engine.render_into(...)`. Do not call `Engine::render_frame_scaled` — it clamps scale with `.min(1.0)` (`video/composition/mod.rs:175-178`) and derives the size by rounding, so it cannot hit an exact even export size and cannot upscale; either relax that clamp behind an explicit `render_into_size(width, height)` entry point or inline the scaled-canvas route in `export.rs`.
- Acceptance: exporting the same project at 720p and at 4K produces text and shadow edges rasterized at those resolutions, and a preview raster at the export size matches the exported frame pixel-for-pixel.

### 117. Camera bubble shadow paints an opaque black caster under the video

- Type: visual.
- Electron: `composition/camera-canvas-renderer.ts:399-430` builds an expanded rect plus the rounded bubble and calls `ctx.clip('evenodd')` before filling the caster, so the black fill is knocked out inside the bubble.
- GPUI: `video/composition/camera.rs:453-471` `render_shadow` fills the whole rounded rect with opaque black and relies on the camera frame drawn afterwards (`:421-450`) to hide it; there is no clip anywhere in the module. `canvas.set_global_alpha(alpha)` is set once at `camera.rs:386` and covers both the caster and the frame, so at `alpha = 0.5` the bubble composites over a 50%-black plate instead of over the wallpaper. `opacity()` (`camera.rs:277-330`) runs on every project with cursor data, and every cursor approach and retreat triggers a fade, so the artefact recurs throughout a normal export.
- Fix: build the even-odd path (outer expanded rect plus inner rounded bubble) and call `clip_path(&path, FillRule::EvenOdd)` before the caster fill — `Canvas::clip_path` already takes a `FillRule` (`render/canvas.rs`), so this is the smaller change and matches Electron exactly.
- Acceptance: at any camera opacity below 1 the area inside the bubble shows the wallpaper, not a darkened plate, in both preview and export.

### 118. GIF export uses a different quantizer than Electron's FFmpeg palette path

- Type: visual.
- Electron: `hooks/use-video-export.ts:342-350` invokes `video-editor:convert-to-gif`, handled at `src/main/capture/video/ipc/export-handlers.ts:93`, which runs FFmpeg with `fps=…,scale=…:flags=lanczos,split;palettegen=max_colors=256:stats_mode=diff;paletteuse=dither=bayer:bayer_scale=5:diff_mode=rectangle` (`src/main/utils/ffmpeg.ts:912-915`) — a temporally stable palette with dithering.
- GPUI: `video/export.rs:307` uses `GifEncoder::new_with_speed(BufWriter::new(file), 10)`, the fastest and lowest-quality NeuQuant setting, encoding each frame independently (`:326-333`); the result bands heavily on gradient wallpapers and shimmers between frames.
- Fix: at minimum drop the speed argument to 1–4 in `export::run_gif`; for real parity build one palette over sampled frames, or feed the composed MP4 through the same FFmpeg palettegen/paletteuse path the Electron shell already ships (no new crate needed — the LGPL FFmpeg binary is already bundled).
- Acceptance: a GIF exported from GPUI and one exported from Electron for the same project show comparable banding and no frame-to-frame palette shimmer.

## P1 — Screenshot editor

### 119. Pen strokes are a plain polyline in the editor and perfect-freehand on export

- Type: visual.
- Electron: both the live render and the export use `perfect-freehand` — `src/renderer/components/editor/annotations/pen-renderer.tsx:27-31,52-56,72` fills `getStroke(coords, { size: strokeWidth * 2, thinning .5, smoothing .6, streamline .5 })` as a path, and `exportPen` (`:85-101`) is the identical call scaled.
- GPUI: `editor/canvas.rs:539-560` builds `stroked(stroke_width * scale)` and emits a raw `move_to`/`line_to` polyline (`stroked()` at `:932`), while `render/annotations.rs:176-191` `draw_pen` calls `freehand::stroke(&coordinates, freehand::Options::for_pen(stroke_width))` and fills it with `FillRule::Winding` (`render/freehand.rs:33-52`, `for_pen` is `size: stroke_width * 2.0`). The preview is roughly half the nominal width and untapered while the export is a tapered ribbon at 2x.
- Fix: in `editor/canvas.rs::draw_annotation`, replace the `Annotation::Pen` arm's polyline with `render::freehand::stroke` filled through `PathBuilder::fill()` + `finish_fill` (`canvas.rs:977`); `render::freehand::stroke` is `pub` and `render::annotations::freehand_path` already converts the outline into a quad path, so lift the shared edge math into `editor/annotations.rs` rather than duplicating it.
- Acceptance: a pen stroke rasterized from the preview and the same stroke in the exported PNG match pixel-for-pixel at the same zoom.

### 120. Highlighter preview is a stroke without multiply blend; the export is a ribbon with multiply

- Type: visual.
- Electron: `src/renderer/components/editor/annotations/highlight-renderer.tsx:6-56` builds the offset upper and lower ribbon and `:86-103` fills it at the annotation opacity with `mixBlendMode: 'multiply'`.
- GPUI: `editor/canvas.rs:411-439` paints a `stroked(stroke_width * scale)` polyline through `window.paint_path` with plain alpha — no ribbon and no multiply — while `render/annotations.rs:193-253` `draw_highlight` builds the exact offset ribbon and calls `fill_path_blended(..., BlendMode::Multiply)` at `:246-251`.
- Fix: share the ribbon geometry by extracting the edge math into `highlight_ribbon(points, stroke_width) -> Vec<Vec2>` in `editor/annotations.rs`. Multiply cannot be shared: `gpui::Window::paint_path` (`gpui-pre-0.3.5/src/window.rs:4457`) takes only `impl Into<Background>` and has no blend-mode parameter. The honest options are a pre-multiplied colour approximation, or rasterizing the committed highlight through `render::annotations` and painting it as an image patch, exactly as committed redactions already do (`editor/window.rs:585-618`). Prefer the patch route, since it is the only one that satisfies preview-equals-export.
- Acceptance: a committed highlight rasterized from the preview matches the exported highlight pixel-for-pixel, including where strokes overlap each other and the image.

### 121. Number badge glyphs are a 5x7 bitmap font in the editor and real text on export

- Type: visual.
- Electron: `src/renderer/components/editor/annotations/number-renderer.tsx:50-66` draws an SVG `<text>` with `font-family="system-ui, -apple-system, sans-serif"` and `font-weight="bold"`.
- GPUI: `editor/canvas.rs:459-481` computes `cell = font_size / glyphs::GLYPH_ROWS` and fills one square path per lit bitmap cell from `crate::editor::glyphs::cells`, while `render/annotations.rs:503-524` `draw_number` fills the disc and then calls `text::fill_text(..., DEFAULT_TEXT_FONT, ...)` with the real platform font.
- Fix: draw the badge label in `editor/canvas.rs` through the same positioned-text overlay path already used for `Annotation::Text` (`editor/canvas.rs:514-520`, `editor/text_render.rs`), which exists precisely so the preview uses the export's font; then delete `editor/glyphs.rs` — grep shows its only readers are `editor/mod.rs:10` and `editor/canvas.rs:460,462` — together with its false doc comment at `:1-3`.
- Acceptance: a number badge rasterized from the preview matches the exported badge pixel-for-pixel, and `editor/glyphs.rs` no longer exists.

### 122. Text annotations resolve fonts through two different paths in GPUI

- Type: visual.
- Electron: `src/renderer/components/editor/text/text-utils.ts:12-19` defines serif `Georgia, serif`, mono `Menlo, monospace`, comic `"Comic Sans MS", cursive, sans-serif`, with `Arial, sans-serif` as the default (`text-renderer.tsx:44,126,230`); the same `fontFamily` string drives the live `<text>` (`:105`) and the exported SVG `<text>` (`:259`), so preview and export are identical by construction.
- GPUI preview: `editor/canvas.rs:1248-1253` maps serif to `"Georgia"`, mono to `"Consolas"`, comic to `"Comic Sans MS"`, and sets no `font_family` at all for the default case, inheriting gpui's UI font. GPUI export: `editor/text_render.rs:12-49` probes OS font files, resolving mono to `consola.ttf` / `cour.ttf` / `Menlo.ttc` and sans to `segoeui.ttf` / `arial.ttf` / Arial / Helvetica, with `SFNS.ttf` explicitly rejected (`:65-69`). On macOS a mono annotation previews in the UI font (Consolas is not installed) and exports as Menlo; a sans annotation previews in SF and exports as Arial. Worse, `editor/window.rs:620-639` `refresh_rotated_text` rasterizes rotated text through the export renderer and `editor/canvas.rs:135-144` excludes those from the gpui overlay, so the font visibly changes on canvas the moment the user rotates the annotation.
- Fix: drive both sides from one table — keep `editor/text_render.rs`'s candidate list as the single source of truth and have `editor/canvas.rs::text_overlay` set `.font_family(...)` to the same resolved face name per platform (Menlo on macOS and Consolas on Windows for mono, Arial/Helvetica vs Segoe UI for sans) instead of a hardcoded Windows-only name and an unset default. Delete the false doc comment at `text_render.rs:1-4`.
- Acceptance: a text annotation previews in the same face it exports in, for every family and both rotated and unrotated, verified by rasterizing the preview and comparing it to the export.

### 123. Wallpaper SVG presets are flattened to two-stop linear gradients

- Type: visual.
- Electron: `src/renderer/hooks/useWallpaperState.ts:15+` holds 14 full inline SVG artworks base64'd into data URLs by `svgToDataUrl` (`:9-12`) — `crimson-wave` (`:16-38`) is a three-stop linear gradient plus two overlaid wave paths at 0.2 and 0.15 opacity; `forest-glow` (`:39-58`) is a radial gradient (cx .3 cy .7 r .9, three stops) plus a dark foreground wave at 0.25.
- GPUI: `editor/wallpaper.rs:180-213` is `SVG_PRESETS: [(&str, &str, [&str; 2], f64); 14]` — id, name, two hex stops and one angle. `crimson-wave` keeps only the outer two stops and loses both waves; `forest-glow` becomes a 225-degree linear two-stop and loses the radial falloff, the mid stop and the wave. Ids and count match; the artwork does not, in the sheet tile and in the composited and exported background alike.
- Fix (decided 2026-09-17, supersedes the baked-raster route): no new assets and no new crates are needed. `usvg` 0.45.1 and `resvg` 0.45.1 are already dependencies of the GPUI crate (`src/main/app-gpui/Cargo.toml:38-39`), and `video/composition/cursor_sprites.rs:178-188` already embeds SVG markup as a string constant and rasterizes it with `resvg` for exactly this reason, so the pointer drawn in GPUI is identical to the one Electron draws. Port the same way: move the 14 SVG sources from `src/renderer/hooks/useWallpaperState.ts` verbatim into a new `editor/wallpaper_svg.rs` as string constants keyed by id, replace `SVG_PRESETS`'s `[&str; 2]` plus angle tuple with a handle to that markup, and rasterize at the requested size for the sheet tile, the composited preview and the export. This keeps one source of artwork, needs no `electron-builder.json5` or `THIRD_PARTY_NOTICES.md` change, and reproduces radial falloffs, three-stop linears and overlay waves exactly.
- Acceptance: each of the 14 presets renders the same artwork in the GPUI tile, the GPUI composited preview and the GPUI export as it does in Electron.

### 124. Holding Cmd/Ctrl does not reveal the capture-edge picker

- Type: behavior.
- Electron: `src/renderer/windows/screenshot-window.tsx:143` holds `isMetaHeld`, set on keydown (`:1191`), cleared on keyup (`:1194`), on blur (`:1196`) and on cleanup (`:1214`); `:1245` computes `showCaptureOverlay = isCaptureMode || isMetaHeld`, feeding `<CaptureEdgeOverlay>` at `:1391`.
- GPUI: `editor/window.rs:2446` gates the overlay on `self.capture_mode` alone; the only `modifiers` reads in the whole `editor/` tree are the pan guard (`:2499`), the shift-constrain reads (`:2527,:2560`) and the wheel-zoom guard (`:2585`), and a repo-wide grep for `on_modifiers_changed` returns zero hits. The tooltip at `editor/title_bar.rs:193-197` advertises the gesture in the same words Electron uses (`src/renderer/components/editor/toolbar.tsx:204-206`).
- Fix: add `.on_modifiers_changed(...)` on the editor root in `EditorWindow::render` (`gpui-pre-0.3.5/src/elements/div.rs:534`), store `meta_held: bool`, and gate the overlay on `self.capture_mode || self.meta_held`; also clear the flag on window blur, because gpui will not deliver a key-up for a modifier released while unfocused — Electron handles that case explicitly with `handleBlur`. This reopens plan item 28.
- Acceptance: holding Cmd in the GPUI editor shows the capture-edge picker, releasing it hides it, and switching away from the window clears it.

### 125. The cloud-upload shortcut is displayed but never bound

- Type: behavior.
- Electron: `src/renderer/windows/screenshot-window.tsx:1045-1053` binds `uploadToCloud` through `useAcceleratorShortcut` with `editorActionShortcuts?.uploadToCloud ?? DEFAULT_UPLOAD_TO_CLOUD_SHORTCUT`.
- GPUI: `editor/title_bar.rs:255-259` renders the tooltip from `self.cloud_upload_shortcut`, fed at `editor/window.rs:2402` from `shortcuts.editor_actions.upload_to_cloud`, but `editor/actions.rs:10-40` has no cloud action and `:76-114` binds none; `EditorAction::CloudUpload` (`editor/options.rs:123`) is dispatched only from the button (`editor/window.rs:3287`). The key is dead.
- Fix: add a `CloudUpload` action to `actions!` in `editor/actions.rs` and bind it from config. The conversion is the catch: `EditorShortcuts` fields are raw gpui keystrokes (`config/shortcuts.rs:130-136`) while `editor_actions.upload_to_cloud` is an Electron accelerator (`"CommandOrControl+Shift+U"`, `config/shortcuts.rs:352-356`), and `system/accelerator.rs` exposes only `parse` → `HotKey` and `display`/`display_spaced` — there is no accelerator-to-gpui-keystroke converter. Add one, and re-run the binding from `init_bindings` so a rebind takes effect without a restart.
- Acceptance: pressing the configured cloud-upload shortcut in the GPUI editor uploads, and rebinding it in settings takes effect without restarting.

### 126. Clicking Wallpaper while the sheet is open does not close it

- Type: behavior.
- Electron: `src/renderer/windows/screenshot-window.tsx:595-614` `handleToolChange` computes `wasWallpaperOpen` and `willWallpaperOpen` and, when both are true, sets `Tool::Select` and refits the zoom.
- GPUI: `editor/window.rs:3293-3296` `set_tool` assigns unconditionally, reached from the toolbar via `apply_option` (`:2800`) and from the keymap (`:2320`).
- Fix: put the toggle in `set_tool` itself rather than in `apply_option`'s `EditorOption::Tool` arm — Electron's `handleToolChange` is shared by the toolbar and `useEditorToolShortcuts` (`screenshot-window.tsx:616-619`), so the `W` key must toggle too. Zoom refit is already driven off `sheet_open` (`editor/window.rs:2219`), so no extra work there.
- Acceptance: clicking the Wallpaper toolbar button or pressing `W` while the sheet is open closes it and returns to the select tool.

### 127. Desktop-wallpaper and custom-image tiles show an icon instead of the image

- Type: visual.
- Electron: `src/renderer/components/editor/wallpaper/index.tsx:459-499` renders the desktop tile with `backgroundImage: url(desktopWallpaperPreview)`, a `Monitor` placeholder only when there is no preview, an `animate-spin` ring and `cursor-wait` while loading, and `disabled` + `opacity-50` on error; `:540-556` renders custom image backgrounds with their own thumbnails.
- GPUI: `editor/wallpaper_sheet.rs:876-900` `desktop_tile` always calls `icon_tile(..., "monitor", ...)` and `:933-943` `custom_tile` always calls `icon_tile(..., "image", ...)`; `image_tile` is defined at `:981-1010` and, inside the screenshot editor, never called — its only callers are in the video editor. No loading or error state exists in the sheet.
- Fix: route both tiles through `image_tile` with an `Option<Arc<RenderImage>> → image_tile / icon_tile` fallback; `windows/video_editor/panels.rs:1568,1630-1660` already demonstrates exactly that pattern, so reuse it rather than inventing a prefetch. Add a spinner child while the fetch is in flight and a disabled style on failure.
- Acceptance: the desktop tile shows the current wallpaper, custom image tiles show their own thumbnails, and a failed desktop fetch renders a disabled tile.

### 128. "Save preset" skips Electron's naming dialog

- Type: behavior.
- Electron: `src/renderer/components/editor/wallpaper/preset-manager.tsx:78-141` replaces the block with a save panel — a settings preview line, an autofocused name input, Enter to save, and Cancel/Save with `disabled={!presetName.trim()}`; `:41-54` builds the preset with the trimmed name.
- GPUI: `editor/wallpaper_sheet.rs:612-637` wires Save straight to `EditorOption::WallpaperSavePreset`, and `editor/window.rs:2943-2958` auto-names it `format!("Preset {}", count + 1)` and pushes it immediately. There is no `TextField`, `preset_name` or naming panel in `wallpaper_sheet.rs`.
- Fix: add `preset_draft: Option<String>` to `EditorWindow`, render the naming panel in `preset_manager` reusing the `TextField` already imported at `wallpaper_sheet.rs:22`, and only emit `WallpaperSavePreset` on commit.
- Acceptance: Save preset opens a naming panel with the settings summary, Enter commits, and an empty name keeps Save disabled.

### 129. No per-tool cursor on the canvas

- Type: visual.
- Electron: `src/renderer/hooks/useBrushCursor.ts:10-60` gives `crosshair` for pen, rectangle, circle, line, arrow, crop and number; `text` for the text tool; `default` for select and wallpaper; and 20px SVG data-URL cursors with a `10 10` hotspot for highlight (tinted by the active colour) and redact (per-style pixelate, blur or blackout glyph). Applied at `src/renderer/components/editor/editor-canvas.tsx:201,428`.
- GPUI: `editor/window.rs:2438-2440` sets `.cursor_default()` on the `canvas-area` div unconditionally; a grep for `cursor_` and `CursorStyle` across `src/main/app-gpui/src/editor/` returns only that call plus `cursor_pointer()` on wallpaper-sheet rows and `canvas.rs:1008`.
- Fix: derive the cursor from `self.tool` in `EditorWindow::render` and call `.cursor(CursorStyle::Crosshair | ::IBeam | ::Arrow)` on the canvas div — there is no `cursor_crosshair` helper, so use `.cursor(CursorStyle::Crosshair)` (`gpui-pre-0.3.5/src/platform.rs:2338-2349`). gpui has no custom-bitmap cursor, so the highlight and redact brush cursors can only be approximated with `Crosshair` or drawn as a follow-the-pointer overlay (see Accepted platform limits).
- Acceptance: each tool sets the cursor Electron sets for it, with the two brush cursors documented as approximations.

---

## P1 — Capture and recording

### 130. Recording bar width is a fixed constant, not the measured toolbar width

- Type: behavior.
- Electron: `src/renderer/windows/recording-control-window.tsx:360-377` observes `toolbar.offsetWidth` with a ResizeObserver and sends it over `recording-control:content-width`; `src/main/capture/video/recording-control-window.ts:177-187` stores it and calls `setBounds`, and `:86-101` `getControlBounds` prefers `contentWidth` over `getRecordingControlWindowWidth(...)`. Electron does not hug the bar — it floors the width at `DEVICE_MENU_WIDTH = 300` and adds `CONTROL_WINDOW_HORIZONTAL_GUTTER = 16` per side — so what the measurement buys is correct centring and safety when the real bar exceeds the estimate.
- GPUI: no measurement path anywhere. Searched `sync_window_bounds`, `element_bounds`, `window.resize`, `canvas(`, `with_bounds`, `set_bounds`, `request_layout` across `src/main/app-gpui/src`: the only self-sizing is `windows/recording_control.rs:1239-1262`, which computes the width from `recording_control_width()` (`:1324-1331` → `ui/chrome.rs:288-299`) and `bar_bounds` (`:1436-1457`); `canvas(` appears only in `ui/icon.rs:109` and `ui/window_controls.rs:230`, both for painting. `bar_bounds` uses the raw constant with no gutter and no 300px floor, so a bar wider than the estimate is clipped inside the window (`recording_shell`, `:1581-1602`) and the end buttons become unreachable. The concrete risk is the target chip: GPUI always adds the full `RECORDING_TARGET_LABEL_WIDTH = 140` allowance while the chip itself is `max_w(140)` and shrinks to the text (`:1487-1496`).
- Fix: `window.element_bounds(...)` does not exist in gpui-pre 0.3.5 — the only `element_bounds` accessors are on `InputHandler` and `scene::Shadow`. Use `gpui::canvas(prepaint, paint)` (`gpui-pre-0.3.5/src/elements/canvas.rs:10-18`, whose prepaint closure receives `Bounds<Pixels>`) as an `absolute().size_full()` child of the bar `div`, stash the observed width on the entity and call `sync_window_bounds` on the next frame; `window.resize(Size<Pixels>)` does exist (`gpui-pre-0.3.5/src/window.rs:2704`). Note `windows/scroll_capture.rs:286-293` is not a precedent — it resizes from a computed height, not from a laid-out element.
- Acceptance: with a long target-name chip and every dropdown present, no control is clipped, and the bar is centred in its window on both platforms.

---

## P1 — Settings

### 131. Device dropdown never refreshes the device list on open

- Type: behavior.
- Electron: `src/renderer/components/settings/devices/device-select.tsx:60-62` calls `onOpen()` on open, which is `refresh` from `useMediaDevices()` (`microphone-device-setting.tsx:22,79`, `camera-device-setting.tsx:22`), re-invoking `devices:list` with a sequence guard (`src/renderer/hooks/use-media-devices.ts:27-37`).
- GPUI: `windows/settings/mod.rs:409-434` `device_lists` returns the cached `self.devices` whenever it is `Some` and sets `Some(default())` immediately before spawning, so the daemon is queried exactly once per settings-window lifetime; `windows/settings/item_row.rs:375-397` attaches only `on_selection_change`. Searched `on_open`, `refresh`, `reload`, `rescan`, `devices::list` under `windows/settings/` and `system/devices.rs` — the only other callers are the mic and camera test paths.
- Fix: use `herogpui::components::Select::on_open_change(impl Fn(&bool, &mut Window, &mut App) + 'static)` (`herogpui-components-0.9.0/src/select.rs:694`, invoked with `true` from the trigger press at `:1731-1771`), i.e. the exact analogue of Radix `onOpenChange`, as `.on_open_change(cx.listener(|this, open: &bool, _w, cx| { if *open { this.refresh_device_lists(cx); } }))`. Factor the spawn out of `device_lists` so the refresh bypasses the `self.devices.is_some()` early return.
- Acceptance: plugging in a microphone while the settings window is open and then opening the dropdown lists the new device.

### 132. The About page auto-downloads an update on open instead of showing an idle check row

- Type: behavior.
- Electron: the About page shows a divider, a refresh glyph, the label "Check for updates" and a "Check" button on the right, idle until asked; the Electron log confirms the dev build skips the check entirely ("Skip checkForUpdates because application is not packed and dev update config is not forced").
- GPUI: the same slot shows a spinner, "Downloading update…", a progress bar and "0%" within seconds of the window opening, with no "Check for updates" label or Check button in that state — `crate::update::spawn_auto_check` is started unconditionally in `src/main/app-gpui/src/main.rs`. Observed live in `gpui-settings-about.png` against `electron-settings-about.png`.
- Fix: gate the GPUI auto-check the way the Electron shell does (skip when not packaged and no forced dev update config), and make the idle row with a Check button the default state of that slot in `windows/settings/about.rs`.
- Acceptance: opening Settings ▸ About on an unpackaged build shows the idle Check row and starts no download.

---

## P1 — Small windows, tray, preview and pin

### 133. Tray click-to-stop during recording is not ported

- Type: behavior.
- Electron: `src/main/menu/recording-tray.ts:60-72` creates a second `Tray` with the tooltip "Click to stop recording" and `setIgnoreDoubleClickEvents(true)`, binding both `click` and `right-click` to `handleStopRecording` (`:45-58`), which flushes pending continuations, awaits the stop handler, hides the recording tray and rebuilds the menu. No menu is popped.
- GPUI: all three platform tray-click sites are unconditional menu toggles — `system/native/macos.rs:69-80`, `system/native/windows.rs:44-52`, `system/native/linux.rs:66-74` — routed through `main.rs:42-43,284` to `windows::tray_menu::TrayMenuWindow::toggle`, which never inspects recording state (`windows/tray_menu.rs:203-230`). `Intent::StopRecording` is reachable only as a menu row (`system/tray/intent.rs:24`, `system/tray/menu.rs:143-146`), and `system/tray/icons.rs:33-46` swaps only the glyph — its own doc comment at `:28-32` admits the click half was not ported. Two interactions instead of one, and no tooltip.
- Fix: branch on `crate::video::recorder::is_recording()` (already the source of `TrayMenuState::is_recording`, `system/tray/menu.rs:52`) either in `main.rs`'s `NativeEvent::ToggleTrayMenu` arm or at the top of `TrayMenuWindow::toggle`, dispatching `Intent::StopRecording` through the existing intents dispatcher; also set the tray tooltip to "Click to stop recording" while recording via `tray_icon::TrayIcon::set_tooltip`.
- Acceptance: while recording, one click on the tray icon stops the recording and no menu appears, and the icon's tooltip reads "Click to stop recording".

### 134. Capture-preview actions are not gated by busy or finished state

- Type: behavior.
- Electron: `src/renderer/windows/capture-preview-window.tsx:118` derives `isBusy = isCopying || isUploading || isPolishing` and `isFinished = isDone || isUploaded`; `handleClose` (`:186-196`) and `handleDoubleClick` (`:260-263`) return early on either, and `handleDelete` (`:245-257`), `handleEdit` and `handleUpload` (`:224-238`) return early on busy.
- GPUI: `windows/capture_preview.rs:1340-1362` checks only `event.click_count < 2` before removing the preview and opening the editor; the close button (`:1436-1458`) calls `begin_remove_preview` unconditionally and is reachable while busy because `show_controls(hovered, busy) = hovered || busy` (`:1233-1234`); `begin_remove_preview` (`:1123-1148`) and `remove_preview_now` (`:1217-1230`) never consult `preview.busy`. Broader than first reported: the delete button (`:1475-1495`) is ungated too, and the Edit pill (`:1563-1595`) is gated only by `!(is_video && busy)`, so a screenshot mid-upload still shows a live Edit pill. No "finished" concept exists (item 238).
- Fix: do not gate inside `begin_remove_preview` — auto-dismiss and the upload-success auto-close both go through it and must keep working. Short-circuit in the three `on_click` / `on_mouse_down` closures using the already-captured `busy` local (`:1303`) plus the new completed flag, and gate the delete handler and the Edit pill's `.when(...)` on `!busy`. Not data loss: the export and upload tasks hold a `WeakEntity` and keep running after removal (`:686-730`, `:1706-1775`).
- Acceptance: while a preview is copying, uploading or polishing, close, delete, double-click-to-edit and the Edit pill are all inert, and they re-enable when the operation finishes.

### 135. Video "Copy" reveals the file instead of writing it to the OS clipboard

- Type: behavior.
- Electron: `src/renderer/hooks/use-video-clipboard-export.ts:168-186` invokes `capture-preview:copy-video-to-clipboard`, and `src/main/capture/capture-preview/video-export.ts:167-188` writes a real file reference — `writeWindowsFileToClipboard(outputPath)` on win32, `clipboard.writeBuffer('public.file-url', pathToFileURL(outputPath).href)` elsewhere — then sets done and auto-closes after 800ms.
- GPUI: `windows/capture_preview.rs:652-655,715-724` shows an "Export ready" toast and calls `crate::system::desktop::reveal_in_file_manager(&path)`; there is no clipboard write. `ClipboardEntry::ExternalPaths` exists in the pinned toolkit but is write-ignored — `gpui-pre-macos-0.3.5/src/pasteboard.rs:172` and `gpui-pre-windows-0.3.5/src/clipboard.rs:81` both match it to a no-op on write, so `crate::system::clipboard::ClipboardService` cannot express this today.
- Fix: this does not require an upstream gpui change. The shell already calls Objective-C directly (`system/native/macos.rs:90-95`) and links `windows` APIs, so `ClipboardService` can write `NSPasteboard` `NSFilenamesPboardType` / a file URL on macOS and `CF_HDROP` on Windows itself.
- Acceptance: copying a video from the capture preview lets the file be pasted into Finder or Explorer and into a chat client, matching Electron.

### 136. History tab icons are on the wrong side of their labels

- Type: visual.
- Electron: the tab row renders icon-then-label — a camera glyph at x=117 followed by "Screenshots" at x=154, a video glyph at x=342 followed by "Videos" at x=379 (`electron-history.png`, 2x).
- GPUI: it renders label-then-icon — "Screenshots" at x=206 with the camera at x=394, "Videos" at x=508 with the video glyph at x=625 (`gpui-history-grid.png`).
- Fix: swap the child order in the tab item builder under `src/main/app-gpui/src/windows/history/` so the icon is emitted before the label.
- Acceptance: both history tabs render their glyph to the left of their label at the same offsets Electron uses.

### 137. History active-tab label is accent-coloured and the pill is oversized

- Type: visual.
- Electron: the active "All" pill is a 61x40 (2x) rounded chip with a white label on `#262626`.
- GPUI: the pill is 103x68 (2x) and the "All" label renders in the accent purple rather than the plain foreground.
- Fix: use the plain foreground token for the selected tab label and size the pill to 30x20pt in the history tab strip under `src/main/app-gpui/src/windows/history/`.
- Acceptance: the active history tab renders a foreground-coloured label in a 30x20pt chip, matching the Electron screenshot.

---

## P1 — Cross-cutting

### 138. Geist is never registered or applied, so the whole GPUI shell renders in the OS UI font

- Type: visual.
- Electron: `src/renderer/styles/base.css:4-16` declares `@font-face` for Geist and Geist Mono and `:113-123` sets `body { font-family: Geist, Inter, ui-sans-serif, system-ui, ... }`.
- GPUI: an exhaustive sweep of `src/main/app-gpui/` finds no `add_fonts` call, no `build.rs`, no `assets/` directory, no `AssetSource`, no `rust-embed` and no font crate in `Cargo.toml`; the only `include_bytes!` are PNG icons. A case-insensitive grep for `geist` returns zero hits in `app-gpui` — the fonts exist only as `src/renderer/fonts/Geist-Variable.woff2` and `GeistMono-Variable.woff2` with `licenses/Geist-OFL-1.1.txt`. No window root calls `.font_family()` (only four leaf sites set `crate::ui::colors::MONO_FONT = "Consolas"`), and HeroGPUI's `ThemeProvider` sets no font either, so gpui falls back to `Font::default()` = `.SystemUIFont` (`gpui-pre-0.3.5/src/text_system.rs:1082-1085`) — San Francisco on macOS, Segoe UI on Windows.
- Fix: `cx.text_system().add_fonts(...)` (`text_system.rs:98`) is the right entry point but cannot load WOFF2, which is the only format Geist ships in here. Add TTF or OTF (variable-TTF) copies of Geist and Geist Mono to the repo — not a WOFF2-decoding crate, which would need approval — `include_bytes!` them, register once at startup, and set `.font_family("Geist")` at each window root or via a new `ThemeVars` field. Adding the binaries triggers the licensing rules: `licenses/Geist-OFL-1.1.txt` already exists, so update `THIRD_PARTY_NOTICES.md` and the `extraResources` filter in `electron-builder.json5` in the same change.
- Acceptance: text in every GPUI window renders in Geist, and a settings row screenshot has the same glyph shapes and advance widths as the Electron one.

---

## P1 — Licensing

### 139. The GPUI shell's version is pinned separately and breaks the AGPL exact-version source link

- Type: behavior.
- Electron: `package.json:4` is `"version": "0.9.6"`, and the About tab's source link is built from that version, satisfying the AGPL requirement in `CLAUDE.md` that the About tab link to this exact version's source (`SOURCE_URL/tree/v{version}`).
- GPUI: `src/main/app-gpui/Cargo.toml:3` pins `version = "0.9.5"`, `src/main/app-gpui/src/product.rs:8` `source_url_for_version()` builds the link from that crate version, and `src/main/app-gpui/src/windows/settings/about.rs:300` renders it. The runtime pass confirmed the GPUI About page shows "Version 0.9.5" while Electron shows "Version 0.9.6", so the GPUI shell links to the wrong tag — a compliance defect, not a cosmetic one.
- Fix: derive the GPUI version from `package.json` at build time in a `src/main/app-gpui/build.rs` that emits it as an env var consumed by `product.rs`, or, if a build script is unwanted, keep the literal and add a parity test that fails when `Cargo.toml`'s version differs from `package.json`'s. Prefer the build script: it makes drift impossible rather than merely detectable.
- Acceptance: the GPUI About tab shows the same version string as `package.json` and its source link resolves to the tag for that exact version; a release with mismatched versions cannot be built or cannot pass tests.

## P2 — Video editor: sidebar and settings panels

### 140. Cursor colour selects lose their swatches and two-column layout

- Type: visual.
- Electron: `cursor-settings-panel.tsx:63-108,344-359` renders each option with a 12px round swatch beside the name and places Colour and Border side by side in `grid grid-cols-2 gap-3`.
- GPUI: `windows/video_editor/panels.rs:209-231` stacks two full-width text-only `kit::select_row` calls; `panel_kit.rs:124-149` builds the `Select` through `rows::picker_items` with no swatch support.
- Fix: extend `rows::picker_items` (`ui/rows.rs:92-97`) to take an optional leading swatch colour and render `PickerItem::new(..).icon(..)`, then wrap the two rows in a `div().flex().flex_row().gap(px(12.))` in `panels.rs::cursor_panel`.
- Acceptance: cursor Colour and Border sit side by side and each option shows its colour swatch.

### 141. Cursor and subtitle data sections drop their summary lines

- Type: visual.
- Electron: `cursor-settings-panel.tsx:405-407` renders `"{n} events, {d}s duration"` (or a fallback sentence) and `subtitle-settings-panel.tsx:428-432` renders `"{n} segments · Generated with {model} model · Using custom prompt"`, both below the button row.
- GPUI: the cursor section shows no summary at all (`panels.rs:258-283`), and the subtitle section shows only `"{count} segments"` above the buttons (`panels.rs:2023-2049`), dropping the model and prompt clauses.
- Fix: add a `summary: Option<SharedString>` to a new `panel_kit::data_editor_section` helper with the label → buttons → summary order, and read the model and prompt from the loaded `SubtitleData` meta alongside `refresh_subtitle_count` (`mod.rs:1472-1479`).
- Acceptance: both data sections show Electron's summary text in Electron's position.

### 142. Audio track rows use the wrong icon and a different slider layout

- Type: visual.
- Electron: the leading icon is `SOURCE_ICONS[track.source]` — `Volume2`, `Mic` or `Music` (`src/types/music.ts:40-44`, used at `audio-settings-panel.tsx:71,78`); Volume and Speed are inline rows with a `w-12 text-xs` muted label, a `flex-1` control and a `w-8` right-aligned percent (`:107-145`); the keyboard header carries a `Keyboard` icon (`:156`) and its volume slider has no label (`:194-209`).
- GPUI: `panels.rs:1349` hard-codes `icon_element("music", px(16.0))` for every source; volume and speed use `kit::slider_row` / `kit::select_row`, which stack a 14px label above a full-width control (`panel_kit.rs:105-122,186-217`); the keyboard switch row has no icon (`panels.rs:1225-1234`) and the keyboard volume slider is labelled "Volume" (`:1276-1288`).
- Fix: map `track.source` to `"volume-2"` / `"mic"` / `"music"` in `panels.rs::music_row`; add an inline `slider_inline_row` variant to `panel_kit.rs` (label `w(px(48.))` muted, slider `flex_1`, value `w(px(32.))` right-aligned) for the three audio sliders; drop the keyboard volume label and prepend `icon_element("keyboard", px(16.))` to the keyboard switch row.
- Acceptance: each audio row shows its source icon and an inline label/slider/value layout matching Electron.

### 143. Subtitle panel has no labelled "or" divider and no checking state

- Type: visual.
- Electron: a rule with a centred `or` chip separates the generate block from the manual-import block (`subtitle-settings-panel.tsx:319-323`), and while readiness is probed the button reads `Checking...` and is disabled (`:85-87,216-217,304-305`).
- GPUI: `kit::separator` (`panel_kit.rs:277-286`) is a plain 1px `bg(theme.border)` div used at `panels.rs:1934`, and `TranscriptionStatus` (`mod.rs:1443-1451`) has only `Idle | Downloading | Generating | Failed`.
- Fix: add `panel_kit::labelled_separator(text, theme)` overlaying a `bg(theme.card)`-padded label on the hairline. The `checking` state is a deliberate omission recorded in plan item 38 and may stay, provided that stays documented.
- Acceptance: the subtitle panel shows an `or` chip on its divider.

### 144. Generate/regenerate button has no spinner

- Type: animation.
- Electron: while downloading or generating the button leads with `<Loader2 className="mr-2 size-4 animate-spin" />` (`subtitle-settings-panel.tsx:301`).
- GPUI: the button always shows a static `"subtitles"` icon (`panels.rs:1925-1933,2058-2066`); the only `with_animation` calls reachable from this window are the sidebar slide (`mod.rs:2679-2681`) and the indeterminate bar.
- Fix: add a shared spinner helper to `ui/primitives.rs` (a `loader-2` icon under `with_animation(Animation::new(Duration::from_secs(1)).repeat(), |el, d| el.with_transformation(Transformation::rotate(percentage(d))))`) and use it here; `herogpui::components::Spinner` is the alternative.
- Acceptance: the generate button shows a rotating spinner while downloading or generating.

### 145. Subtitle Regenerate and Delete gain icons and lose the destructive tint

- Type: visual.
- Electron: both are text-only tertiary `xs` buttons and Delete adds `text-destructive hover:text-destructive` (`subtitle-settings-panel.tsx:437-468`).
- GPUI: `panels.rs:2057-2073` gives Regenerate a `"subtitles"` icon and Delete a `"trash-2"` icon and neither carries `.recipe("danger-text")`, which the drawing panel's delete button does use (`panels.rs:683-684`).
- Fix: use label-only tertiary buttons for both and add `.recipe("danger-text")` to the delete button.
- Acceptance: both buttons are text-only and Delete renders in the destructive colour.

### 146. Data-editor dialog has no field-documentation tooltip

- Type: behavior.
- Electron: a `HelpCircle` beside the label opens a `max-w-xs` `side="right"` tooltip documenting every JSON field (`cursor-data-editor-dialog.tsx:126-161`, `subtitle-data-editor-dialog.tsx:113-143`).
- GPUI: `windows/video_editor/data_editor.rs:223-249` renders only the label span and the two Load buttons; grepping `Tooltip`, `help-circle`, `tooltip` in that file returns nothing.
- Fix: wrap an `icon_element("help-circle", px(14.))` in `herogpui::components::Tooltip` in `data_editor.rs::render`, carrying the copy on `DataKind` alongside `title()` and `description()`. Note HeroGPUI 0.9.0 tooltips take a `SharedString` only, so the field table must be a formatted string, not a rich element.
- Acceptance: a help glyph beside the data-editor label opens a tooltip listing the JSON fields.

### 147. Data-editor example and template content and placeholders diverge

- Type: visual.
- Electron: the subtitle example is three segments of real sentences with no `words` (`subtitle-data-editor-dialog.tsx:31-46`); the template is one segment ending at `min(3.0, duration)` with the text "Your subtitle text here" (`:48-67`); placeholders are per-kind, "Enter cursor data JSON..." and "Enter subtitle data JSON..." (`cursor-data-editor-dialog.tsx:189`, `subtitle-data-editor-dialog.tsx:171`).
- GPUI: `data_editor.rs:76` is one `"Hello there"` segment with word timings, `:110` uses `duration.min(2.0)` and empty text, and `:256` uses one shared `"Enter data as JSON…"` placeholder for every `DataKind`.
- Fix: copy the Electron literals into `DataKind::example` and `DataKind::template`, and add a `DataKind::placeholder()` returning the two per-kind strings. The existing test `the_subtitle_example_validates` (`data_editor.rs:326-331`) must be updated in the same change.
- Acceptance: the example, template and placeholder text match the Electron dialogs exactly.

### 148. Data-editor dialog geometry, button variants and saving state differ

- Type: visual.
- Electron: `sm:max-w-2xl` (672px) and `max-h-[90vh]`, textarea `h-full min-h-60` mono `text-xs`; Cancel is tertiary, Save is primary and shows `Saving...` while disabled (`cursor-data-editor-dialog.tsx:114,188,201-208`, `hooks/use-json-data-editor.ts:43,63,75`).
- GPUI: `data_editor.rs:198-199` fixes 640px and `max_h(620)`, `:255-257` uses `TextArea::new(...).rows(16)`, Cancel is `Variant::Secondary` (`:280-288`) and Save is a static primary with no disabled or in-flight state (`:290-298`); `save_data_editor` is synchronous (`mod.rs:1843-1875`).
- Fix: widen to `px(672.)` and `max_h(relative(0.9))`, switch Cancel to `Variant::Tertiary`, and gate Save's label and `is_disabled` on a `saving: bool` on `DataEditor`.
- Acceptance: the dialog is 672px wide, caps at 90% of the window height, and Save reads `Saving...` and is disabled while a save is in flight.

### 149. Drawing tool buttons and camera position cells have no tooltips

- Type: behavior.
- Electron: each drawing tool button carries `title={item.label}` (`drawing-settings-panel.tsx:261`) and each camera position cell `title={position.replace('-', ' ')}` (`camera-settings-panel.tsx:65`).
- GPUI: `panels.rs:700-722` builds each tool `Button` with only an icon child and `panels.rs:1133-1155` builds each position cell as a plain `div()` with `on_mouse_down` only.
- Fix: wrap both in `herogpui::components::Tooltip::new(label).child(..)`, the way `sidebar.rs:173` already does for the rail.
- Acceptance: hovering any drawing tool or camera position cell shows its name.

### 150. Drawing tool grid is a wrapping flex row, not a five-column grid

- Type: visual.
- Electron: `grid grid-cols-5 gap-1` with `size-8!` buttons — two fixed rows of five (`drawing-settings-panel.tsx:248,259`).
- GPUI: `panels.rs:725-731` is `div().flex().flex_row().flex_wrap().gap(...)` over the whole tool list, so the wrap point follows the panel width.
- Fix: chunk `VIDEO_DRAWING_TOOLS` into rows of five in `panels.rs::drawing_tool_grid` and emit one `flex_row` per chunk with `flex_1` children.
- Acceptance: the tool grid is two rows of five at every sidebar width.

### 151. Drawing option controls are dropdowns instead of the Electron pickers

- Type: visual.
- Electron: colour uses the swatch `ColorPicker` (`drawing-settings-panel.tsx:273-294`) and arrow, highlight, shape, number, text and redact use their dedicated `*Options` components laid out inline to the right of a muted `SettingRow` label (`:126-141,318-477`).
- GPUI: `panels.rs:735-1008` uses `kit::select_row` / `tab_row` / `slider_row` — a 14px label above a full-width control — with colour palettes hard-coded at `:762-779` rather than shared with the screenshot editor's picker.
- Fix: reuse `crate::ui::color_picker::{trigger, ColorPickerPopover}` (the module is `ui::color_picker`, not under `editor/`; it is driven from `editor/title_bar.rs:383-405`) in `drawing_style_rows`, and add an inline `setting_row(label, control)` helper to `panel_kit.rs` with a muted left label and a right-aligned control.
- Acceptance: the drawing panel uses the shared colour popover and inline setting rows.

### 152. Zoom empty state is left-aligned instead of centred and vertically filled

- Type: visual.
- Electron: `zoom-settings-panel.tsx:178-191` with `components/empty-state.tsx:9-16` renders `flex h-auto flex-1 items-center justify-center p-4` with centred muted text.
- GPUI: `panels.rs:361-364` uses `kit::note`, which is a plain `div().w_full().min_w_0()` with no centring or `flex_1` (`ui/rows.rs:33-40`).
- Fix: call the existing `kit::empty_state` (already `flex`/`size_full`/`items_center`/`justify_center`, used by `camera_panel`) for the no-selection branch.
- Acceptance: with no zoom selected the message is centred in the remaining panel height.

### 153. Sidebar resize grip is a full-height hairline, not a short rounded pill

- Type: visual.
- Electron: the 6px track holds an `my-auto h-8 w-0.5 rounded-full` bar coloured `bg-muted-foreground/30` → `/50` on group hover → `bg-primary` while resizing with `transition-colors`, and the track itself tints `hover:bg-primary/20` → `bg-primary/40` (`editor-sidebar.tsx:367-383`).
- GPUI: `sidebar.rs:111-135` uses `div().my_auto().h_full().w(px(1.0))` with no rounding and no transition, coloured `theme.border` → `accent.opacity(0.65)` → `accent`, and the track has no background tint at any state.
- Fix: set the inner bar to `h(px(32.)).w(px(2.)).rounded_full().my_auto()` with `theme.muted_foreground.opacity(0.3/0.5)` and tint the track with `theme.accent.opacity(0.2/0.4)`.
- Acceptance: the grip is a 32x2 rounded pill and the track tints on hover and while resizing.

### 154. Sidebar panel container has no left border

- Type: visual.
- Electron: the sidebar wrapper carries `border-l border-border bg-card` (`editor-sidebar.tsx:364`).
- GPUI: `mod.rs:2634-2641` sets `bg(theme.card)` only; only the tab rail has a left border (`sidebar.rs:160-161`).
- Fix: add `.border_l_1().border_color(theme.border)` to `sidebar_panel` in `mod.rs:2634`.
- Acceptance: the sidebar panel has a hairline left border like Electron's.

### 155. Camera position cells have no hover state

- Type: visual.
- Electron: unselected cells are `bg-default hover:bg-default-hover` with `transition-colors` (`camera-settings-panel.tsx:59-64`).
- GPUI: `panels.rs:1137-1148` sets a flat `theme.default` with no `.hover(...)` and no transition.
- Fix: add `.hover(|s| s.bg(theme.default_hover))` in `panels.rs::camera_position_grid`; the token already exists in `ThemeVars`.
- Acceptance: unselected camera position cells lighten on hover.

### 156. First-frame preview is a fixed 144px box, not a 16:9 box

- Type: visual.
- Electron: `aspect-video w-full object-cover` inside a `rounded-lg border` frame (`first-frame-settings-panel.tsx:78-84`).
- GPUI: `panels.rs:2148-2151` uses `.w_full().h(px(144.0))` with `ObjectFit::Cover`, so the box stops tracking the sidebar width as it is resized.
- Fix: replace the fixed height with `.aspect_ratio(16.0 / 9.0)` on the wrapper in `panels.rs::first_frame_panel`.
- Acceptance: the first-frame thumbnail stays 16:9 at every sidebar width.

### 157. Motion Blur and Hide When Idle descriptions sit below the switch row

- Type: visual.
- Electron: the description is stacked under the label inside the row, so the switch is vertically centred against a two-line block (`cursor-settings-panel.tsx:306-321,361-376`).
- GPUI: `kit::switch_row` takes a label only (`panel_kit.rs:105-122`) and the hint is pushed as a following sibling (`panels.rs:183-193,233-241`).
- Fix: add an optional `description` to `panel_kit::switch_row` and use `rows::title_desc_stack` (`ui/rows.rs:65`, already used by `panel_kit.rs:51`) for the left side.
- Acceptance: both rows render label and description stacked inside the row with the switch centred against them.

### 158. Camera empty-state line break is an embedded newline

- Type: visual.
- Electron: the two sentences are separated by an explicit `<br />` (`camera-settings-panel.tsx:86-88`).
- GPUI: `panels.rs:1020` passes one string containing `\n` to `kit::empty_state`, relying on gpui text layout to honour it.
- Fix: have `panel_kit::empty_state` accept a `Vec<SharedString>` and emit one centred `div` per line.
- Acceptance: the camera empty state renders two centred lines regardless of text layout behaviour.

---

## P2 — Video editor: timeline

### 159. Drawing lanes are collapsed into one lane

- Type: visual.
- Electron: one `<DrawingTrack>` row per drawing segment with a matching `PenLine` gutter row, and a bare `<TrackRow />` placeholder when there are none (`video-editor-window.tsx:1191-1204,1274-1297`).
- GPUI: `mod.rs:2436-2449` pushes a single `TrackKind::Drawing` `Track` folding every drawing segment through `range_clips`.
- Fix: emit one `Track` per drawing segment in `mod.rs::tracks()`; the lane and gutter loop in `tracks.rs::render` already handles N tracks, but `TrackKind` needs a per-track id suffix so lane element ids stay unique.
- Acceptance: each drawing segment has its own lane and gutter row.

### 160. Camera lane visibility is keyed off segment count instead of camera data

- Type: behavior.
- Electron: the camera lane renders whenever `editorData.cameraData` exists, even with zero segments, showing the "Drag to show camera" empty state (`video-editor-window.tsx:1186-1190,1257-1272`, `camera-track.tsx:91`).
- GPUI: `mod.rs:2420-2432` gates the lane on `!self.state.camera_segments.is_empty()`, so a recording with a camera but no segments has no lane — and no way to add one, since `add_clip` needs the lane. A `has_camera` boolean is already computed at `mod.rs:2484-2487` and used elsewhere (`:2647`) but never by `tracks()`.
- Fix: key the lane on `has_camera` rather than on segment count.
- Acceptance: a camera recording with zero camera segments still shows an empty camera lane that accepts a new clip.

### 161. Clip labels have no per-kind rules, no speed badge and a film glyph on every track

- Type: visual.
- Electron: per-track `renderLabel`s — video shows duration plus a `rounded bg-white/20 px-1.5` speed badge when speed is not 1 and a film glyph below 100px (`timeline-track.tsx:78-103`); zoom shows a `ZoomIn` icon and `Nx` hidden below 50px (`zoom-track.tsx:101-116`); camera shows a `Camera` icon with the name above 100px and icon-only below 64px (`camera-track.tsx:57-73`); drawing uses per-type icons with the same 64/100 thresholds (`drawing-track.tsx:113-133`); music shows source icon, name at 100px, duration at 140px and a speed badge, icon-only below 60px (`music-track.tsx:80-112`).
- GPUI: `timeline/tracks.rs:190-210` has one label path for every kind — a truncated text child, or `icon_element("film")` below a single `CLIP_LABEL_MIN_WIDTH = 100.0` (`:397-402`) — with no icon or badge fields on `Clip`; music labels are `track.name` only (`mod.rs:2458`); zoom levels format as `{:.1}x` (`mod.rs:2416`), rendering `2.0x` where Electron prints `2x` (`zoom-track.tsx:105-106`).
- Fix: add `icon: &'static str` and `badge: Option<SharedString>` to `Clip`, use per-`TrackKind` narrow thresholds, compose the music label from name, duration and speed, and format zoom with Electron's integer rule.
- Acceptance: each lane's clip labels, icons, badges and hide thresholds match its Electron counterpart.

### 162. Clip geometry differs: rounded corners, inset, selected border, no adjacency gap

- Type: visual.
- Electron: clips fill the row (`absolute h-full`, no radius, no border) with a 2px left gap between adjacent clips so the boundary reads, and selection is signalled purely by a gradient swap to `var(--primary) → var(--accent-hover)` (`track.tsx:392-422`, `track-colors.ts:12`).
- GPUI: `timeline/tracks.rs:178-190` sets `.top(px(2.0))`, `.h(px(TRACK_HEIGHT - 4.0))`, `.rounded(px(4.0))` and `.border_1().border_color(theme.foreground)` when selected, with no adjacency gap anywhere in the file.
- Fix: drop the radius, inset and border in `clip_element` and port the `GAP = 2` adjacency offset from `track.tsx:393-401`; signal selection with the gradient swap.
- Acceptance: clips fill their lane, adjacent clips show a 2px seam, and a selected clip changes gradient rather than gaining a ring.

### 163. Cut markers between adjacent video clips are missing

- Type: visual.
- Electron: between every pair of video clips, a `size-5 rounded-full bg-amber-500` bubble with a `Scissors` glyph over a `rgba(245,158,11,0.5)` vertical rule (`track.tsx:455-476`, enabled by `showCutMarkers` at `timeline-track.tsx:159`).
- GPUI: absent; searched `cut_marker`, `showCutMarkers`, scissors marker — `"scissors"` appears only as toolbar and menu icons (`timeline/controls.rs:81`, `timeline/tracks.rs:342,355`).
- Fix: in `tracks.rs::render`, for `TrackKind::Video` emit a marker element between consecutive clips using `icon_element("scissors", px(12.))` inside a `rounded_full().bg(Srgba::parse("#f59e0b"))`, hidden while a reorder drag is in flight.
- Acceptance: a cut between two video clips shows the amber scissors bubble and rule.

### 164. Empty-lane placeholder text is missing on every track

- Type: visual.
- Electron: an empty `Track` renders centred `text-xs text-muted-foreground` copy (`track.tsx:497-503`) — "Click or drag to add zoom" (`zoom-track.tsx:135`) and "Drag to show camera" (`camera-track.tsx:91`).
- GPUI: absent; grepping `empty_text`, "Click or drag", "Drag to show" and `placeholder` in `timeline/tracks.rs` returns nothing.
- Fix: add `empty_text: Option<&'static str>` to `Track` and render a centred muted child in `tracks.rs::render` when `clips.is_empty()`.
- Acceptance: empty zoom and camera lanes show their Electron placeholder text.

### 165. Speed control is a slider popover with the wrong preset set

- Type: visual.
- Electron: an inline `−  1x  +` stepper — two ghost `size-7` icon buttons around a `w-10 text-center text-xs` readout, each tooltipped, stepping `PLAYBACK_SPEED_PRESETS = [0.5, 0.75, 1, 1.25, 1.5, 2, 3, 4]` and disabling at the ends (`timeline/speed-selector.tsx:19-78`, `src/types/playback-speed.ts:3-9`).
- GPUI: `timeline/controls.rs:249-335` opens a 208px popover with a continuous slider over `SPEED_PRESETS: [f64; 9] = [0.25, …, 4.0]` (`:228`), adding a 0.25x step Electron does not offer and allowing off-preset values through `speed_at_position`.
- Fix: replace `speed_selector` with two `icon_button::compact_sm` buttons around a `w(px(40.))` centred label stepping the Electron array, disabled at the ends, and drop 0.25x.
- Acceptance: speed steps only through Electron's eight presets and the end buttons disable at 0.5x and 4x.

### 166. Playhead has no glow and its knob is misplaced

- Type: visual.
- Electron: a 2px bar with `boxShadow: 0 0 8px rgba(239,68,68,0.6)` and a 12px knob at `top: -12px` carrying the same glow (`timeline/playhead.tsx:8-24`).
- GPUI: `timeline/tracks.rs:126-145` draws a 2px bar with no shadow and places the knob at `top(px(-6.0))`, half-overlapping the lane where Electron floats it fully above.
- Fix: add a red `gpui::BoxShadow` to both the bar and the knob and move the knob to `top(px(-12.))`.
- Acceptance: the playhead knob floats above the lanes with the same red glow as Electron's.

### 167. Timeline has no clip or lane cursors

- Type: visual.
- Electron: an inline amber `SCISSORS_CURSOR` on the whole lane and on clips while the cut tool is active (`video-editor/utils.ts:328`, applied through `track.tsx:359-370,414-419`), plus `grabbing` while moving, `ew-resize` while resizing and `crosshair` while drawing (`track.tsx:361-368`); the panel divider also uses a resize cursor (`video-editor-window.tsx:1128`).
- GPUI: `timeline/tracks.rs:451` sets only `el.cursor_pointer()` on the lane, gated by `is_cut_tool_active || can_add_to(kind)`; clips set no cursor. Under `windows/video_editor/` the only `CursorStyle` hits are the unrelated serde model type `styles::CursorStyle` — a name collision, not a cursor.
- Fix: gpui has no custom-image cursor, so approximate: `CursorStyle::Crosshair` on the lane while the cut tool is active, `CursorStyle::ResizeLeftRight` on the handle elements from item 100, and `CursorStyle::ClosedHand` on a clip while its drag is a `Move`. All three exist (`gpui-pre-0.3.5/src/platform.rs:2434+`) and are already used by the capture overlay.
- Acceptance: the timeline shows a resize cursor on clip edges, a grab cursor while moving a clip, and a crosshair while the cut tool is active.

### 168. No auto-scroll to keep the playhead visible during playback

- Type: behavior.
- Electron: once the playhead passes `containerWidth - SCROLL_MARGIN (100)` the tracks — and through the sync the ruler — scroll to follow (`timeline/timeline-tracks.tsx:10,217-232`).
- GPUI: absent; searched `SCROLL_MARGIN`, `set_offset`, `scroll_to` under `windows/video_editor/` — no hits.
- Fix: after `set_playhead` (or in `mod.rs::drive_playback`) compare `playhead * pixels_per_second` against `tracks_scroll.bounds().size.width + offset` and call `tracks_scroll.set_offset` with the same 100px margin, remembering gpui's negative horizontal offsets.
- Acceptance: during playback the playhead stays inside the visible timeline with a 100px margin.

### 169. Escape is swallowed while a text field is focused

- Type: behavior.
- Electron: `Escape` is dispatched at the very top of `handleKeyDown`, before the ignore-while-typing guard (`hooks/use-editor-shortcuts.ts:84-88`), and `handleEscape` (`video-editor-window.tsx:537-544`) clears every selection kind — segment, zoom, camera, drawing and music.
- GPUI: `mod.rs:2289-2292` returns whenever `editing_text()` is true, so the `escape` branch at `:2298-2309` is unreachable from a focused field, and even when reached it clears only the menu, the speed selector and `selected_clip`; the rename field handles its own escape separately (`title_bar.rs:159-168`).
- Fix: handle `escape` in `mod.rs::on_key` before the `editing_text()` early return, blurring the focused field first, and extend the branch to clear every selection kind. This is the same ordering change item 91 needs, so land them together.
- Acceptance: Escape from inside any video-editor text field blurs it and clears every selection.

### 170. Frame step uses the source frame rate and End lands past the last frame

- Type: behavior.
- Electron: `FRAME_STEP = 1/30` regardless of the media (`hooks/use-editor-shortcuts.ts:33,237-243`), and `End` seeks to `max(0, duration - 0.01)` so the last frame stays composited (`:214-219`).
- GPUI: `mod.rs:2370,2374` step by `frame_step(self.source_frame_rate)` (`:2772`), and `mod.rs:2361-2364` seeks `End` to `self.total_duration()` exactly, past which `preview.rs:124-127` falls back to the last segment's `end_time`.
- Fix: use `1.0/30.0` in the `,` and `.` arms and clamp `end` to `total_duration - 0.01` (in `on_key` or inside `set_playhead`, `mod.rs:977`).
- Acceptance: frame stepping moves 1/30s on any recording and `End` lands on the last composited frame.

### 171. Toggling the cut tool does not clear the clip selection

- Type: behavior.
- Electron: `toggleCutTool` clears `selectedSegmentId` (`hooks/use-segment-operations.ts:70-73`) and `handleSegmentSelect` refuses selections while the cut tool is on (`:75-81`).
- GPUI: `mod.rs:1068-1071` only flips `is_cut_tool_active`, and `timeline/tracks.rs:220-221` calls `select_clip` unconditionally in the clip mouse-down, checking `is_cut_tool_active` only afterwards — so a clip stays selected and the speed selector stays in the bar while cutting.
- Fix: clear `selected_clip` and `speed_selector_open` in `toggle_cut_tool`, and skip `select_clip` in `clip_element` when the cut tool is active.
- Acceptance: enabling the cut tool clears any selection, and clicking a clip while cutting cuts without selecting.

### 172. Zoom slider step is continuous rather than 1 px/s

- Type: visual.
- Electron: `<Slider min={10} max={500} step={1} className="w-24">` (`timeline/timeline-controls.tsx:177-185`).
- GPUI: `timeline/controls.rs:151-166` passes `0.0` as the step to `rows::slider_control`, which only quantizes above zero.
- Fix: pass `1.0` as the step in `controls.rs::render`.
- Acceptance: the timeline zoom slider lands only on integer pixels-per-second values.

### 173. No clip transition animation and no resize-handle hover transition

- Type: animation.
- Electron: clips carry `transition-all` when no drag is in flight (`track.tsx:412-414`, disabled during trim and reorder at `timeline-track.tsx:165`), the resize-handle overlays animate their hover fill with `transition-colors` (`track.tsx:427,430`), and the timeline resize grip uses `transition-colors` (`video-editor-window.tsx:1133`).
- GPUI: grepping `with_animation` and `transition` in `timeline/tracks.rs` returns nothing; the sidebar tween (`mod.rs:2679-2688`) is the window's only animation, and the resize handle swaps `bg()` directly by state (`sidebar.rs:111-131`).
- Fix: for the handle hover, `.hover(|el| el.bg(white(0.2)))` is enough — gpui hover styles are instantaneous, which is an accepted approximation. For clip geometry, either leave it unanimated as a documented approximation or wrap left and width in a 150ms `ease_out` animation keyed on the clip's geometry generation.
- Acceptance: handle hover has a visible state change and any clip animation choice is recorded in this document.

---

## P2 — Video editor: window chrome, preview and export

### 174. Export errors are never shown in the panel

- Type: behavior.
- Electron: `export-settings-panel.tsx:348-352` renders `{exportError && <p className="text-xs text-destructive" role="alert">}` below the export button, fed from `hooks/use-video-export.ts:74,84-89` and passed in at `video-editor-window.tsx:1400`.
- GPUI: `panels.rs:2378-2443` holds only the progress block, Cancel and the export button; grepping `export_error`, `exportError` and `rows::error` under `windows/video_editor/` returns nothing. The error text does reach the user — `mod.rs:383-393` calls `Toast::show`, which forwards to a real OS notification (`windows/toast.rs:17-18`) — so what is missing is the persistent inline line, not the information.
- Fix: add `export_error: Option<SharedString>` to `VideoEditorWindow`, set it in the completion closure and clear it in `start_export` and `cancel_export`, then push `kit::error(msg, theme)` (`ui/rows.rs:53`, reached through the local `kit` alias) into `footer` after the export button.
- Acceptance: a failed export leaves a destructive-coloured error line under the export button until the next attempt.

### 175. Preview stage uses the wrong background, drops the 16px inset and upscales small videos

- Type: visual.
- Electron: the player container is `p-4` with no background of its own, inheriting `bg-background` (`#070709`, `src/renderer/styles/base.css:26`) from the window root, and `displayScale = Math.min(scaleX, scaleY, 1)` never enlarges a composition smaller than the pane (`native-video-player.tsx:232-241,1311-1316`).
- GPUI: `mod.rs:2517-2554` sets `bg(theme.surface)` (`#0e0e14`), adds no padding, and paints `img(...).size_full().object_fit(Contain)`, which scales up to fill the pane; since `--card: var(--surface)`, the stage is also the same colour as the timeline panel below it instead of contrasting with it.
- Fix: change the preview `div` to `.bg(theme.background)` with `.p(px(16.0))`, and cap the upscale by measuring the pane with `gpui::canvas` bounds and sizing the `img` to `min(pane/frame, 1) * frame` rather than `size_full()`.
- Acceptance: the preview stage is the window background colour with a 16px inset, and a 480p composition renders at 1:1 in a larger pane.

### 176. Loading and error states use different copy, different iconography and no spinner

- Type: visual.
- Electron: three distinct states — a centred `Loading recording...` on `bg-background` (`video-editor-window.tsx:1021-1034`), a hard-failure state with a `TriangleAlert`, `Could not read this recording`, a two-sentence explanation naming FFmpeg and the offending path in mono (`:1005-1019`), and inside the player an 8px `Loader2` with `animate-spin` plus `Loading...` (`native-video-player.tsx:1318-1322`).
- GPUI: one shape for all three — a 32px static `film` icon with a 14px title and a 12px subtitle (`mod.rs:2530-2543,2755-2768`), no spinner, no file path on the error, and the error confined to the preview pane while the rest of the chrome stays live.
- Fix: use the shared spinner helper from item 144 for the loading case, extend `empty_preview` to take an optional third line so the unavailable case can show the project path, and align the copy with `video-editor-window.tsx:1005-1019`.
- Acceptance: loading shows a spinner and the failure state names FFmpeg and shows the file path, matching the Electron copy.

### 177. Panel progress rows do not right-align their values

- Type: visual.
- Electron: `justify-between` rows with a left label and a right-aligned `tabular-nums` value — `Exporting...` / `42%` and `0:12 elapsed` / `0:30 remaining` (`export-settings-panel.tsx:311-327`).
- GPUI: both children of those rows are `kit::hint`, which is `w_full().min_w_0()` (`ui/rows.rs:43-51`), so the two hints split the row and the value sits left-aligned in its half (`panels.rs:2384-2421`); there is also no tabular treatment, so the readout jitters as digits change.
- Fix: replace the right-hand `kit::hint` calls with a `div().text_size(px(chrome::TEXT_XS)).text_color(theme.muted_foreground).flex_none()`, or add a `rows::hint_inline` helper. gpui has no tabular-numerals feature toggle, so the jitter is only fixable by using the mono family — record that as a deliberate deviation if taken.
- Acceptance: the percent, elapsed and remaining values sit at the right edge of their rows.

### 178. `Exporting...` loses its emphasis and the export button gains an icon Electron does not have

- Type: visual.
- Electron: `Exporting...` is `text-xs font-medium` in the foreground colour while the percent is muted (`export-settings-panel.tsx:312-315`), and the export button is a text-only tertiary `xs` full-width button (`:338-347`).
- GPUI: both use `kit::hint`, i.e. 12px muted (`panels.rs:2390-2395`), and `kit::tertiary_button` (`panel_kit.rs:257-274`) always renders a leading icon through `rows::icon_text_button`, with `"download"` passed at `panels.rs:2433-2443` and no icon-less path available.
- Fix: use a foreground-coloured medium-weight label for `Exporting...` and add an icon-less variant to `panel_kit::tertiary_button` for the export action.
- Acceptance: `Exporting...` reads as emphasised against a muted percent and the export button carries no icon.

### 179. Cloud-upload success block is unstyled and Copy gives no feedback

- Type: visual.
- Electron: a `rounded-md bg-muted/50 p-3` card with a primary `Check`, `Uploaded to cloud`, the URL truncated with a `title` tooltip, and side-by-side `Copy` / `Open` where Copy swaps to `Copied` with a check for 2000ms (`export-settings-panel.tsx:97-115,156-195`); the uploading state is a muted label plus an indeterminate `Progress` and a ghost `Cancel` (`:123-142`).
- GPUI: `panels.rs:2318-2367` pushes three bare `kit::hint` lines with no card, no check and no truncation, and `copy_uploaded_url` (`mod.rs:2072-2078`) only writes the clipboard with no state change, so the button never acknowledges; the cancel is a full-width tertiary button with an `x`.
- Fix: wrap the success case in a `rounded(px(chrome::RADIUS_MD)).bg(theme.muted_background).p(px(12.0))` card with an `icon_element("check", …)` header and a truncated URL, and add a `copied_at: Option<Instant>` set by `copy_uploaded_url` so the label flips to `Copied` for 2s.
- Acceptance: the upload success state is a card with a check, a truncated URL and a Copy button that reads `Copied` for two seconds.

### 180. Export destination defaults differ and the last-used directory is not remembered

- Type: behavior.
- Electron: the save dialog is seeded with `${fileName}-exported.${ext}` (`hooks/use-video-export.ts:183-184`) resolved against a remembered per-kind directory, stored again on success (`src/main/capture/video/ipc/export-handlers.ts:39,55`), and only GIF has its extension forced (`:46-49`).
- GPUI: `video/export.rs:131-144` suggests `<project name>.<ext>` in the project's parent folder with no memory of the last destination, `mod.rs:312-350` sets a `set_title("Export video")` Electron does not set, and `:339-345` forces the extension for both formats.
- Fix: append `-exported` to the stem in `export::default_output_path`, drop the dialog title, force the extension for GIF only, and read and write a `last_export_dir` on the app config the way `rememberSaveDirectory('video', …)` does.
- Acceptance: the export dialog opens in the last used directory with `<name>-exported.<ext>` pre-filled.

### 181. Export completion is an in-app toast, not an OS notification, and the copy differs

- Type: behavior.
- Electron: an OS `Notification` titled `Export Complete` whose body is `Video exported successfully in <duration>` (`src/main/capture/video/ipc/export-handlers.ts:66-80`), then `shell.showItemInFolder` when Reveal is on (`:82-88`).
- GPUI: `mod.rs:380-393` reveals first, then shows `Toast::show(cx, "Export finished", path)` — an in-app toast whose body is the file path, never the elapsed duration — even though a real OS-notification path exists at `system/notification.rs`.
- Fix: use the notification path with Electron's title and body, porting `formatExportDuration` (`export-handlers.ts:14-23`), and reveal after notifying.
- Acceptance: a finished export raises an OS notification titled `Export Complete` naming the elapsed duration.

### 182. GIF export progress runs 0→100 on the frame pass instead of 0→70→100

- Type: behavior.
- Electron: the frame pass reports `Math.round(progress * 0.7)`, then pins 70 before the FFmpeg conversion and 100 after (`hooks/use-video-export.ts:323-328,340,375`), so the bar keeps moving through the conversion phase.
- GPUI: `video/export.rs:300-334` reports `(index + 1) / total_frames` straight through, so the bar reaches 100% before `drop(encoder)` flushes and the ETA collapses to zero.
- Fix: scale the per-frame fraction by 0.7 in `export::run_gif` and report 1.0 only after the encoder is flushed.
- Acceptance: a GIF export's progress bar keeps moving through the encode flush and only reaches 100% when the file is written.

### 183. `40 FPS` is offered in GPUI but is unreachable in Electron

- Type: visual.
- Electron: `ALL_FRAMERATE_OPTIONS` omits `40` (`export-settings-panel.tsx:67-75`) and the rendered list is that array filtered by the format config (`:491-497`), so no `40 FPS` row ever exists even though `FORMAT_CONFIGS.mp4.frameRates` includes it (`src/types/video.ts:111`).
- GPUI: `styles.rs:388,398` includes `"40"` in both `EXPORT_FRAME_RATES` and `MP4_FRAME_RATES`, producing an extra row.
- Fix: drop `"40"` from both arrays in `styles.rs`, and from the length assertion at `styles.rs:539` if it references the count.
- Acceptance: the mp4 frame-rate menu offers exactly the rows Electron offers.

### 184. Filename is an interactive ghost button rather than a plain truncating label

- Type: visual.
- Electron: the title is a non-interactive `<span className="truncate text-sm text-muted-foreground">` in the drag region with no hover state and no tooltip (`video-title-bar.tsx:62-64`).
- GPUI: `title_bar.rs:181-198` wraps the label in `toolbar::tooltip_button(Button::new("video-rename-project")…Ghost…Sm, "Rename project", …)`, adding hover and press surfaces, a tooltip and `BUTTON_SM_PAD_X = 12.0` of horizontal padding that shifts the title 12px right of the 88px inset; the `w(px(0.0))` spacer at `:145-152` is a measurement anchor, not a truncation mechanism.
- Fix: render the title as a plain muted 14px `div` with `truncate()` and move the rename affordance into the project info popover from item 90, which is where Electron keeps it.
- Acceptance: the title is a plain truncating label at the 88px inset with no hover state.

### 185. Title-bar tooltip placement and the folder tooltip text differ

- Type: visual.
- Electron: reset, delete and sidebar tooltips are explicitly `side="bottom"` (`video-title-bar.tsx:98,112,133`), undo and redo use the default side, and the folder trigger's tooltip is the literal `Project Info` (`project-path-indicator.tsx:125`).
- GPUI: every title-bar button goes through `toolbar::tooltip_button` → `icon_button::with_tooltip` (`ui/toolbar.rs:80-92`, `ui/icon_button.rs:59-63`), which calls `Tooltip::new` with no placement override and therefore defaults to `TooltipPlacement::Top` (`herogpui-components-0.9.0/src/tooltip.rs:286`); the folder tooltip is the full path (`title_bar.rs:42-47`).
- Fix: add a placement argument to `ui/icon_button::with_tooltip` (or a `tooltip_button_below` wrapper in `ui/toolbar.rs`) using `Tooltip::placement(TooltipPlacement)` (`tooltip.rs:314-317`), pass bottom placement for reset, delete and sidebar, and change the folder tooltip to `Project Info` once item 90 lands.
- Acceptance: those three tooltips open below their buttons and the folder tooltip reads `Project Info`.

### 186. Electron cascades multiple editor windows; GPUI has exactly one

- Type: behavior.
- Electron: every `createVideoEditorWindow` call makes a new `BrowserWindow`, offset 30px per already-open editor and clamped to the work area minus 100px, tracked in a map (`src/main/capture/video/window-manager.ts:60-104,111-117`).
- GPUI: `mod.rs:173-178` calls `registry::open_or_activate(RegistryKind::VideoEditor, ...)`, a single-entry registry keyed by window kind (`registry.rs:44-55`), so opening a second recording activates the existing window; bounds are a fixed centred 1280x800 with no work-area clamp.
- Fix: either accept single-window as a deliberate GPUI simplification and record it here, or key the registry by project path and apply the cascade offset and work-area clamp in `mod.rs::open` using `cx.displays()` and `Bounds::centered` arithmetic.
- Acceptance: the chosen behaviour is implemented and stated, and the window never opens larger than the work area.

### 187. `savedAt` is never refreshed and the persist debounce has the wrong shape

- Type: behavior.
- Electron: every save writes `savedAt: new Date().toISOString()` (`hooks/use-editor-state-persistence.ts:109`), and the debounce is trailing — each change clears and re-arms the 500ms timer (`:153-160`).
- GPUI: `saved_at` defaults to `String::new()` (`model.rs:215,262`) and `persist()` (`mod.rs:777-792`) calls `model::save_state` without ever assigning it, so the field only round-trips what was loaded; the same function is a leading-schedule throttle — the first change arms the timer and later changes inside the window are folded in but never re-arm it. This corrects the audit's "persist path matches" claim.
- Fix: set `self.state.saved_at` to an RFC 3339 timestamp immediately before `model::save_state` — on a clone, so the assignment does not make `commit`'s `state == before` comparison always false — and re-arm the timer on every change to match Electron's trailing debounce.
- Acceptance: `state.json` carries a fresh `savedAt` after every save, and a burst of edits writes once, 500ms after the last one.

### 188. Determinate progress fill has no transition

- Type: animation.
- Electron: the title-bar ring animates its `stroke-dashoffset` over `duration-300 ease-out` (`src/renderer/components/ui/circular-progress.tsx:53-56`), and the export and project popovers enter with `animate-in fade-in-0 zoom-in-95 slide-in-from-top-2`.
- GPUI: `title_bar.rs:63-66` and `panels.rs:2397-2399` set `.value(...)` on `herogpui::ProgressBar` with no `with_animation` wrapper. The indeterminate bar is a faithful port (`ui/primitives.rs:253-280`, 1200ms `ease_in_out`, 33% width) and the sidebar tween is an approximation (`mod.rs:2670-2685`), so this is the remaining gap along with the missing popovers.
- Fix: store `displayed_progress` on `VideoEditorWindow` and wrap the `ProgressBar` value in a `with_animation` easing between the previous and current fraction; add the popover enter animation using the existing `chrome::DIALOG_FADE_MS` / `DIALOG_ZOOM` constants (`ui/chrome.rs:85-87`) when items 90 and 108 land.
- Acceptance: the export progress fill eases between values rather than jumping.

### 189. Cursor motion blur composites per sample instead of through a group buffer

- Type: visual.
- Electron: `composition/cursor-canvas-renderer.ts:255-293` draws the nine blur copies into an offscreen buffer at absolute alphas `1/(i+1)` and blits the finished buffer once under `ctx.globalAlpha = opacity`.
- GPUI: `video/composition/cursor.rs:372-389` sets `canvas.set_global_alpha(opacity)` and draws each sample via `draw_sprite(..., 1.0/(index+1))`, which multiplies into the current alpha (`:399-405`); there is no scratch pixmap in the module. Identical while opacity is 1, and different only during the hide-on-idle fade.
- Fix: compose the samples into a scratch `Canvas` sized like Electron's buffer (`size*click_scale + |dx|,|dy|` plus pad) in `cursor::render` and draw that pixmap once at `opacity`.
- Acceptance: during a cursor fade, a preview raster and the exported frame both match Electron's trail density.

### 190. Export audio is not delayed by the first-frame still

- Type: behavior.
- Electron: `export/webcodecs-exporter.ts:858-866` passes `audioDelaySeconds: firstFrameDuration` into `muxAudioWithVideo`, forwarded to `muxAudio` (`export/audio-muxer.ts:37-45,254-261`).
- GPUI: `video/export.rs:249-251` writes audio at offset 0, and `build_audio` (`:456-507`) mixes from sample 0 while `total_duration` already includes `first_frame_duration` (`video/composition/mod.rs:136-157`). The magnitude is exactly one frame — 16.7ms at 60fps — and only when the first-frame still is enabled.
- Fix: pass `first_frame_duration` as the audio offset in `export::run`.
- Acceptance: with a first-frame still enabled, the exported audio starts one frame later, matching Electron's mux offset.

### 191. Wallpaper and camera shadow `offsetY` rounds twice

- Type: visual.
- Electron: `composition/wallpaper-canvas-renderer.ts:14-24` computes `offsetY: Math.round(shadowValue * 0.25 * 0.3)`.
- GPUI: `video/composition/wallpaper.rs:72-85` rounds twice — `blur = (shadow * 0.25).round()` and then `offset_y = (blur * 0.3).round()`. Values 50, 100 and 200 agree; `shadow = 6` gives Electron 0 and GPUI 1. `camera::render_shadow` (`video/composition/camera.rs:454`) reuses the same config, so the bubble shadow inherits the same off-by-one.
- Fix: compute `offset_y: (shadow * 0.25 * 0.3).round() as f32` in `wallpaper::shadow_config`.
- Acceptance: the shadow offset matches Electron's at every shadow value, in both preview and export.

## P2 — Screenshot editor

### 192. Selection halo is a padded bounding box instead of a shape-following stroke

- Type: visual.
- Electron: `src/renderer/components/editor/shared/renderers/rectangle-config.tsx:35-49` re-strokes the same shape at `strokeWidth + 6` using `SELECTION_STROKE = color-mix(in srgb, var(--primary) 80%, transparent)` (`shared/colors.ts:31-33`), and `annotations/number-renderer.tsx:39-48` strokes `r = radius + 3` at width 6.
- GPUI: `editor/canvas.rs:1101-1132` draws one `border_1()` `theme.primary` rounded rect padded 4px around `annotation.bounds()` for every selected annotation; the doc comment at `:1101` calls it "A dashed box", which it is not.
- Fix: move the halo into `editor/canvas.rs::draw_annotation` as a first pass per kind, re-stroking the same path at `stroke_width + 6` with `primary.opacity(0.8)`, and keep the bounding box only for multi-select. Delete the incorrect comment.
- Acceptance: selecting a circle or line shows a halo following that shape, not a rectangle.

### 193. Crop overlay border is 1px and its hint is a left-aligned chip

- Type: visual.
- Electron: `src/renderer/components/editor/svg-crop-overlay.tsx:352-361` strokes the box at `strokeWidth={2} rx={1}`, and `:305-306,379-388` renders the hint as `textAnchor="middle"` at the box centre, 24px below the box, `fontSize 14`, `fill var(--primary)`, with no chip, reading "Press Enter to crop".
- GPUI: `editor/canvas.rs:1190-1193` uses `border_1()`, and `:1207-1220` renders the hint left-aligned at `top + h + 6` in an 11px popover chip with a border, reading "Enter to crop · Esc to cancel". The colour is not a delta — `theme/vars.rs:154,174` resolve `accent` and `primary` from the same value, so the `theme.accent` token here is a maintainability wart, not a visual difference.
- Fix: `border_2().border_color(theme.primary).rounded(px(1.0))` plus a centred 14px `theme.primary` hint 24px below the box with no chip. Switching the token to `theme.primary` is a cleanup, not required work.
- Acceptance: the crop box has a 2px border and a centred 14px hint 24px below it.

### 194. Background and frame tiles lose the ring-offset selection, hover scale and tooltips

- Type: visual.
- Electron: `src/renderer/components/editor/wallpaper/index.tsx:501-517` uses `aspect-square rounded-lg transition-all` with `ring-2 ring-ring ring-offset-2` when selected and `hover:scale-105` otherwise, plus `title={preset.name}`; the desktop tile is wrapped in a real `Tooltip` (`:458-499`) and the action buttons are tooltipped (`:437-455`).
- GPUI: `editor/wallpaper_sheet.rs:948-975` (`icon_tile`) and `:1013-1042` (`gradient_tile`) take `_tooltip: &str` / `_name: &str` and discard them; selection is an inline `border_2().border_color(theme.ring)` with no offset gap, and there is no hover state or transition anywhere on the tiles.
- Fix: wrap the tiles in `herogpui::components::Tooltip` (already used at `editor/window.rs:2737`), draw selection as an outer ring with a 2px transparent gap, and add a hover scale via `with_animation` or, at minimum, a hover border or background change.
- Acceptance: tiles show their name on hover, lift on hover, and the selected tile has a ring with an offset gap.

### 195. The wallpaper sheet fades on enter and has no exit animation

- Type: animation.
- Electron: `src/renderer/components/editor/wallpaper/index.tsx:361-364,380-383` uses `transition-transform duration-300 ease-in-out` toggling `translate-x-0` / `-translate-x-full` with opacity untouched, and `:149-161` keeps `shouldRender` true for 300ms after close so it slides out.
- GPUI: `editor/wallpaper_sheet.rs:143-151` applies both `.opacity(delta)` and `.left(...)` in the enter animation, and `editor/window.rs:2420-2426` mounts the sheet only `when(self.tool == Tool::Wallpaper)`, so closing is an instant unmount.
- Fix: drop `.opacity(delta)` from the animation closure and keep the sheet mounted for one 300ms reverse animation on close, gated by a `sheet_closing: Option<Instant>` on `EditorWindow`.
- Acceptance: the sheet slides in and out over 300ms without fading.

### 196. Crop is never persisted as the last tool

- Type: behavior.
- Electron: `src/renderer/hooks/useEditorState.ts:70-72` restores `savedTool` unless it is `wallpaper`, so crop is a valid persisted tool.
- GPUI: `editor/window.rs:3252-3254` skips writing `last_tool` for both `Tool::Crop` and `Tool::Wallpaper`, while the restore side already filters only Wallpaper (`:501,549`).
- Fix: narrow the write guard to `Tool::Wallpaper` only, matching the restore side.
- Acceptance: leaving the editor with the crop tool active reopens it with crop selected.

### 197. Windows frame preview is missing its close button

- Type: visual.
- Electron: `src/renderer/components/editor/wallpaper/window-frame-preview.tsx:50-71` `WindowsControls` renders minimize (`h-px w-1.5`), maximize (`size-1.5 border`) and close (two absolutely positioned `h-px w-2` bars at `rotate-45` and `-rotate-45`).
- GPUI: `editor/wallpaper_sheet.rs:1319-1344` `windows_controls` renders only the minimize bar and the maximize square; there is no third child.
- Fix: append a third `div().w(px(12.0))` holding two absolutely positioned 1px x 8px bars rotated ±45 degrees. Confirm gpui rotation support before relying on it; the equivalent is to paint the X with a `gpui::canvas` stroke, as item 198 already proposes for dashes.
- Acceptance: the Windows frame preview shows three controls including a close glyph.

### 198. "No frame" tile border is solid, not dashed

- Type: visual.
- Electron: `window-frame-preview.tsx:108-114` adds `border-dashed border-border` for the none case on a `rounded-md border` tile.
- GPUI: `editor/wallpaper_sheet.rs:1253-1260` uses `rounded(px(6.0))` with a solid `border_1()`; the framed arm at `:1232` also hardcodes `rounded(px(6.0))` where `chrome::RADIUS_MD` is 1.5 (`ui/chrome.rs:13-18`).
- Fix: gpui has no dashed border, so paint the dashes with a `gpui::canvas` stroke, and replace both `rounded(px(6.0))` literals with `rounded(px(chrome::RADIUS_MD))`.
- Acceptance: the "no frame" tile has a dashed border and both tiles use the shared radius token.

### 199. Colour randomiser is time-seeded and clamped, not uniform

- Type: behavior.
- Electron: `src/renderer/components/editor/color-picker/index.tsx:53-59` uses `crypto.getRandomValues(new Uint8Array(3))` joined as hex — the full 24-bit RGB range.
- GPUI: `ui/color_picker.rs:154-165` seeds from `SystemTime` nanoseconds and derives `hue = seed % 360`, saturation in [0.55, 1.00) and value in [0.60, 1.00), so the result is never dark, never desaturated, and the three channels are correlated from one 30-bit seed.
- Fix: `src/main/app-gpui/Cargo.toml` has no `rand` or `getrandom` dependency and the repo forbids adding packages without approval. Either request approval for `getrandom`, or keep `SystemTime` as the seed and run it through a splitmix64 mixer, mapping the output to three raw bytes so the whole RGB range is reachable.
- Acceptance: repeated shuffles reach dark and desaturated colours across the full 24-bit range.

### 200. Number, text and redact secondary controls are submenus, not inline rows

- Type: visual.
- Electron: `src/renderer/components/editor/number/number-options.tsx:118-140` renders a `h-px bg-separator` divider and then an inline `Starting:` label with an `h-6 min-h-6 w-14 rounded-3xl` `Select` trigger; `text/text-options.tsx:75-130` and `redact/redact-options.tsx:85-128` have the same shape.
- GPUI: `editor/tool_options.rs:282-297,318-334,338-370` uses `.row("Starting:").submenu(...)` and friends — nested flyouts rather than inline selects.
- Fix: an accepted approximation given the menu primitive; if parity is wanted, reproduce the inline layout with a custom `MenuEntry::Custom` row hosting a HeroGPUI `Select`.
- Acceptance: the chosen approach is implemented and this deviation is recorded.

### 201. `TEXT_FONT_WEIGHT` and the number badge's bold weight are dropped on both GPUI paths

- Type: visual.
- Electron: `src/renderer/components/editor/text/text-utils.ts:32` sets `TEXT_FONT_WEIGHT = 500`, applied to the live `<text>` (`text-renderer.tsx:106`) and to the exported SVG (`:259`); number badges are `font-weight="bold"` (`number-renderer.tsx:50-66`).
- GPUI: `editor/canvas.rs:1239-1253` sets no `font_weight`, `editor/text_render.rs` loads one regular face per family with no weight axis, and `render/annotations.rs:488-499,514-523` pass no weight to `text::fill_text`. Both paths render one weight lighter than Electron, consistently with each other.
- Fix: add `.font_weight(FontWeight::MEDIUM)` on the preview element and extend the `text_render` candidate table with medium and bold face paths, as item 115 does for the composition fonts. Preview and export at least agree today, so recording this as an accepted deviation is defensible if the face files are not worth the packaging cost.
- Acceptance: text annotations and number badges render at Electron's weight on both paths, or the deviation is recorded here with a reason.

### 202. Editor stroke-width and colour chips sit about 7pt left of Electron's

- Type: visual.
- Electron: on a 1066x691 editor window the stroke chip spans x 512–606 and the colour chip 640–740 (2x, `electron-editor-toolbar.png`).
- GPUI: the stroke chip spans 497–591 and the colour chip 625–741 (`gpui-editor-toolbar.png`); every control from the pencil rightwards is pixel-identical, so the delta is confined to the two leading chips.
- Fix: match the chip width and the inter-chip gap in the GPUI editor toolbar so the group's right edge lands on the same separator as Electron's.
- Acceptance: the stroke and colour chips occupy the same x-ranges as Electron at the same window size.

### 203. Editor zoom pill is about 27pt wider in GPUI

- Type: visual.
- Electron: the bottom-right zoom control spans x 1740–1968 (2x) for the label "51%", i.e. 228px.
- GPUI: it spans 1688–1970 for the shorter label "26%", i.e. 282px at the same window size.
- Fix: reduce the minus/label/plus gaps or the label's min-width in the GPUI zoom control so the pill is 114pt wide. The audit's geometry claim (inset 16, pad 4, gap 2, reset min-width 56 in `ui/chrome.rs:48-51`) holds for the constants; the rendered width does not.
- Acceptance: the zoom pill measures 114pt wide at the same window size and label length.

---

## P2 — Capture and recording

### 204. Colour-picker card drops the "Click to copy · Esc to cancel" hint

- Type: visual.
- Electron: `src/renderer/components/area-overlay/color-picker.tsx:287-289` renders a third row, `mt-1 px-0.5 text-xs text-muted-foreground`, reading "Click to copy · Esc to cancel".
- GPUI: `capture/color_picker.rs:105-141` renders only the loupe row and the swatch/hex row, with `CARD_WIDTH = 128.0` fixed (`:41`) and `CARD_HEIGHT = SIZE + 32.0` (`:42`). Both behaviours themselves work (`capture/overlay.rs:1945-1949,1721-1725`); only the hint is missing.
- Fix: append a `div().mt(px(4.0)).px(px(2.0)).text_size(px(12.0)).text_color(theme.muted_foreground)` row in `capture::color_picker::render`. The hint needs roughly 170px at 12px, so drop or raise `CARD_WIDTH` well past 128, and derive the flip-at-viewport-edge maths from a single measured `CARD_HEIGHT` rather than guessing `SIZE + 52.0`.
- Acceptance: the loupe card shows the hint on one line and still flips correctly near a screen edge.

### 205. The all-in-one toolbar stays on screen while picking a colour

- Type: visual.
- Electron: `src/renderer/windows/area-overlay-window.tsx:251` renders the toolbar only when `toolbar && !handedOff && !isPickingColor`.
- GPUI: `capture/overlay.rs:1756-1769` ends the `picking_color` branch by adding the toolbar as a child, and `capture/all_in_one_toolbar.rs:132-134` `mode_selected` returns false for every mode while picking, i.e. the toolbar is deliberately drawn in a "pipette selected" state.
- Fix: return the picking branch without the toolbar child (delete the block at `overlay.rs:1766-1768`); `mode_selected`'s `picking_color` argument then becomes dead and can be simplified away.
- Acceptance: starting a colour pick hides the all-in-one toolbar.

### 206. Prompt pill sits 12–20px lower than Electron when the toolbar is present

- Type: visual.
- Electron: `area-overlay-window.tsx:236-244` uses `top-28` on macOS and `top-24` elsewhere with a toolbar (112 / 96), and `top-12` / `top-8` without.
- GPUI: `ui/chrome.rs:257-262` computes `overlay_prompt_top(true) = overlay_toolbar_top() + overlay_bar_height() + OVERLAY_PROMPT_TOOLBAR_GAP` = 48 + 44 + 40 = 132 on macOS and 108 on Windows.
- Fix: return the reference constants 112 and 96 from `ui/chrome.rs::overlay_prompt_top` when a toolbar is present, and update the `overlay_chrome_matches_electron` assertion at `chrome.rs:591-595`, which currently pins the wrong formula.
- Acceptance: with the all-in-one toolbar present the prompt pill sits at 112px on macOS and 96px on Windows.

### 207. Timer capture shows a prompt pill the reference suppresses

- Type: visual.
- Electron: `src/main/capture/timer-capture.ts:168-172` passes `showPrompt: false`, and the pill is gated on `params.showPrompt` at `area-overlay-window.tsx:230`.
- GPUI: `capture/intent.rs:29-32` `prompt()` returns `DRAG_PROMPT` for every intent and `capture/overlay.rs:1822` renders it unconditionally in the no-selection branch.
- Fix: add `CaptureIntent::shows_prompt(self) -> bool` returning false for `Timer`, gate `overlay.rs:1822` on it, and extend the `every_prompt_is_one_the_reference_actually_uses` test (`intent.rs:99-147`) to skip suppressed intents.
- Acceptance: a timer capture's area overlay shows no prompt pill.

### 208. Timer countdown runs over a bare screen with the selection frame gone

- Type: behavior.
- Electron: `src/main/capture/timer-capture.ts:130-149` shows the timer control, awaits the countdown, and only then calls `cancelAreaSelection(true)`.
- GPUI: `capture/overlay.rs:1396` dismisses the overlay before the deferred `capture_area_reserved`, and `capture/coordinator.rs:427-495` `run_countdown` then shows only the daemon countdown panel; `retains_overlay_after_selection` (`overlay.rs:1555-1557`) is `Recording`-only.
- Fix: treat `CaptureIntent::Timer` like `Recording` in `retains_overlay_after_selection`, keeping the overlay in a handed-off state showing only the scrim around the selection, and close it from `run_countdown` once the countdown resolves or is cancelled.
- Acceptance: the timer countdown runs with the selection frame and scrim still on screen.

### 209. Moving the selection shows a closed-hand cursor instead of `move`

- Type: visual.
- Electron: `src/renderer/utils/area-selection.ts:196-207` returns `'move'` inside the selection and `'crosshair'` outside.
- GPUI: `capture/selection.rs:267-271` returns `CursorStyle::ClosedHand` and `capture/overlay.rs:1968-1970` sets `ClosedHand` on a move gesture.
- Fix: gpui's `CursorStyle` has no `Move` variant (`gpui-pre-0.3.5/src/platform.rs:2437-2513`), so the closest accurate pair is `OpenHand` at rest and `ClosedHand` while dragging — change `selection::cursor_for` to `OpenHand` and leave `on_down` on `ClosedHand`.
- Acceptance: hovering inside the selection shows an open hand and dragging shows a closed hand.

### 210. Window pick commits on mouse-up and shows a pointer cursor

- Type: behavior.
- Electron: `src/renderer/hooks/use-area-selection.ts:253-287` commits the pick in `startDrag`, i.e. on mouse-down, and the cursor is `'default'` (`:100,161`).
- GPUI: `capture/overlay.rs:1950-1952` returns early on mouse-down while picking windows and `:2083-2086` commits in `on_up`, with `root.cursor_pointer()` at `:1777`. The display picker already commits on mouse-down (`:1953-1956`), so only the window path differs.
- Fix: move the `confirm_window` call into `on_down`, guarding against a second commit, and drop `.cursor_pointer()` so the picking overlay keeps `CursorStyle::Arrow`.
- Acceptance: picking a window commits on press with a default cursor.

### 211. iOS dropdown never shows that a device is selected

- Type: visual.
- Electron: `src/renderer/windows/recording-control-window.tsx:288-297` tints the trigger `text-primary hover:text-primary` when a device is selected and `text-white/85 hover:text-white` otherwise.
- GPUI: `windows/recording_control.rs:638-648` builds the iOS trigger through the generic `device_dropdown`, whose trigger `div` (`:653-733`, styling at `:670-700`) sets no `text_color` at all — only an opacity for the countdown-disabled state and a hover background. The selection is still discoverable through the menu check (`:868-899`).
- Fix: give `device_dropdown` a tint parameter, pass `self.selected_ios_id.is_some()`, and apply `theme.accent` on the trigger; since the trigger has no baseline text colour today, add the `white(0.85)` / `white(1.0)` hover pair in the same change.
- Acceptance: with an iOS device selected the dropdown trigger renders in the accent colour.

### 212. Target chip is 12px wider and has no tooltip for the truncated name

- Type: visual.
- Electron: `recording-control-window.tsx:590-599` renders `max-w-32 truncate px-1 text-xs text-foreground` with `title={state.targetName}`.
- GPUI: `windows/recording_control.rs:1487-1496` uses `max_w(px(TARGET_LABEL_WIDTH))` where `RECORDING_TARGET_LABEL_WIDTH = 140` (`ui/chrome.rs:224`), and there is no tooltip.
- Fix: add a separate `RECORDING_TARGET_CHIP_MAX_W = 128.0` in `ui/chrome.rs` for the chip and keep 140 as the window allowance, and wrap the chip as `Tooltip::new(name.clone()).id("recording-target-tip").child(chip)` — `Tooltip::new` alone is not usable, the builder needs `.id(...)` and `.child(...)` as at `recording_control.rs:668-671`.
- Acceptance: the chip caps at 128px and hovering a truncated name shows the full name.

### 213. Elapsed timer is not monospaced

- Type: visual.
- Electron: `recording-control-window.tsx:619-621` uses `min-w-16 px-1 text-center font-mono text-xs text-foreground tabular-nums` with no weight override.
- GPUI: `windows/recording_control.rs:1548-1556` uses `min_w(64) px(4) text_size(12) font_weight(MEDIUM) text_center` in the default family.
- Fix: add `.font_family(crate::ui::colors::MONO_FONT)` (real, used at `capture/color_picker.rs:134`) and drop `font_weight(MEDIUM)`.
- Acceptance: the elapsed timer is monospaced and does not shift as digits change.

### 214. Countdown surface spacing and numeral rendering differ

- Type: visual.
- Electron: `recording-control-window.tsx:231-242` uses `text-3xl leading-none font-semibold tabular-nums` inside a `flex flex-col gap-0.5`.
- GPUI: `windows/recording_control.rs:1610-1634` uses `text_size(30)` semibold with no `line_height`, and the hint column has no gap.
- Fix: add `.gap(px(2.0))` to the column and `.line_height(px(30.0))` to the numeral. Do not add `MONO_FONT` here — Electron keeps the default family and only asks for `tabular-nums`; gpui has no tabular-numerals toggle, so mono would be an over-correction and, if taken, must be recorded as a deliberate deviation.
- Acceptance: the countdown numeral has a 30px line height and the hint column has a 2px gap.

### 215. Close button stays live during the countdown

- Type: behavior.
- Electron: `recording-control-window.tsx:513,538,563,579,638,652` all pass `disabled={state.isStarting}`, and `isStarting` spans the whole countdown (`src/main/capture/video/recording-control.ts:500-531`).
- GPUI: `windows/recording_control.rs:1502-1506` disables only `recording-start`; the dropdowns fade and guard (`:679,700-710`) and system audio guards (`:630-633`), but `recording-cancel` (`:1515-1521`) is a plain `overlay_icon` with no disabled state.
- Fix: give `overlay_icon` a `disabled: bool` and pass `self.countdown_active` for `recording-cancel`, or switch it to `toolbar::button(...).is_disabled(self.countdown_active)` like the start button.
- Acceptance: every control on the pre-recording bar is disabled during the countdown.

### 216. Mode tab dims when the selected tab is hovered

- Type: visual.
- Electron: `src/renderer/components/area-overlay/all-in-one-toolbar.tsx:80,90` keeps the selected tab at `data-[selected=true]:hover:text-foreground`.
- GPUI: `ui/toolbar.rs:116-122` checks hover first — `if hovered { muted_foreground } else if active { foreground } else { muted_foreground.opacity(0.6) }` — so hovering the active tab dims it.
- Fix: reorder to `if active { theme.foreground } else if hovered { theme.muted_foreground } else { theme.muted_foreground.opacity(0.6) }`.
- Acceptance: hovering the selected mode tab leaves it at full foreground.

### 217. Scroll capture's "move cursor here" affordance moved off the capture area

- Type: visual.
- Electron: `src/renderer/windows/scroll-capture-overlay-window.tsx:58-73` draws a panel at the exact area rect — `rounded-lg bg-black/75 px-3 py-2 text-sm font-medium text-white` with a `MousePointerClick` size-4 glyph and "Move cursor here to continue" — and recolours the dashed area frame orange while the cursor is out (`:51-53`).
- GPUI: `windows/scroll_capture.rs:82-85,317-332` puts the string in the preview panel's status row in `rgb(0xf97316)` with a `mouse-pointer-2` glyph at 14px; `ScrollCaptureSession::open` opens exactly two popups (`:160-200`), so nothing covers the capture rect. The affordance is present and legible, just relocated to the adjacent 240px panel.
- Fix: open a third nonactivating, click-through popup covering the capture rect in `ScrollCaptureSession::open` using the same `popup_window_options` pattern, rendered only while the cursor is outside, and drop the orange status row. The popup must not itself capture the pointer or it will keep the cursor "outside". The orange frame recolour belongs to the daemon's frame on GPUI, so it must either go through the daemon or be dropped.
- Acceptance: with the cursor away from the capture area, the prompt panel is drawn over that area, and it disappears when the cursor returns.

### 218. Scroll capture preview panel chrome differs

- Type: visual.
- Electron: `scroll-capture-overlay-window.tsx:75-91` is `absolute overflow-hidden rounded-lg bg-black/40 shadow-2xl` with no border and no status row, height `min(PREVIEW_WIDTH / aspect, 360)` and the image `object-cover object-bottom`.
- GPUI: `windows/scroll_capture.rs:294-303` uses `rounded(8) bg(theme.popover) border_1 border_color(theme.border) shadow_lg` plus a 28px status row (`:317-334`) printing `"{n} frames · ~{h} px"` or `"Capturing…"`, strings the reference never shows.
- Fix: use `bg(crate::ui::colors::black(0.4))`, drop the border, use `shadow_2xl()` and remove the status row, which collapses `STATUS_HEIGHT` out of the height maths (`:286-293`) and out of `preview_bounds` (`:145-149`).
- Acceptance: the scroll preview panel matches Electron's translucent, borderless, status-free chrome.

### 219. Desktop icons disappear as soon as the selection overlay opens

- Type: behavior.
- Electron: `src/main/capture/screenshot/capture-area.ts:92-99` hides the icons inside `captureArea`, i.e. after the selection is confirmed; only `src/main/capture/timer-capture.ts:200-206` and `src/main/capture/scroll-capture/index.ts:71` hide before selection.
- GPUI: `capture/mod.rs:368-373` `restart_capture_hide` hides for every intent except `Recording` and is called from `start_area_selection` (`:382`) before the overlay opens. `GPUI-PARITY-IMPLEMENTATION-PLAN.md:86-88` documents this as intentional, and plan item 5's wording should be corrected to say the Electron reference hides after confirm.
- Fix: if reference behaviour is wanted, move the `hide_for_capture` call out of `restart_capture_hide` into `AreaOverlay::confirm` / `confirm_screen`, which already hide for the all-in-one and screen-picker paths (`capture/overlay.rs:1377-1382,2009-2014`), keeping the pre-selection hide only for `Timer` and `ScrollCapture`.
- Acceptance: the chosen behaviour is implemented and plan item 5's description states what Electron actually does.

### 220. Scroll control-bar icons are stroked, not filled

- Type: visual.
- Electron: `src/renderer/windows/scroll-capture-control-window.tsx:50-54` renders `<Square className="size-3.5 fill-current" />` and `<Play className="size-3.5 fill-current" />`.
- GPUI: `windows/scroll_capture.rs:383-386` picks `"square"` / `"play"` through `ScrollControlBar::button` → `toolbar::icon` → `ui/icon.rs`, which paints with `PathBuilder::stroke` at 16px (`ui/icon.rs:1-2,106-113`).
- Fix: `filled_glyph` (`windows/recording_control.rs:2006-2015`) only draws a 14px `div` that is a disc or a square, so it covers the Square state but cannot produce a filled Play triangle, and `ui/icon.rs` has no fill path. Lift `filled_glyph` into `ui/toolbar.rs` for the square, and add a filled-path helper in `ui/icon.rs` using `PathBuilder::fill` over the lucide path data (or hand-draw the triangle) for the play glyph.
- Acceptance: both scroll control-bar glyphs render filled at 14px.

### 221. Menus have no enter animation and the mode tabs have no moving indicator

- Type: animation.
- Electron: `recording-control-window.tsx:87-117,180-184` waits on the HeroUI popover exit, and `all-in-one-toolbar.tsx:82,92` renders a `<TabsIndicator>`.
- GPUI: the exit is already animated — `ui/menu/mod.rs:387-399` wraps a closing popup in `crate::ui::primitives::overlay_exit` (a real `with_animation` with opacity, slide and `ease_out` over `OVERLAY_EXIT_MS`, `ui/primitives.rs:125-146`) and `:371-385` holds the popup alive for the duration. What is absent is any enter animation (`MenuHandle::open_with` and `ui/menu/view.rs` render at full opacity immediately) and any tab-indicator motion (`ui/toolbar.rs:132` paints a static background on the active tab).
- Fix: add only the enter animation, mirroring the existing exit helper's shape in `ui/primitives.rs:125-146` rather than inventing a new one. `ease_out_quint()` does not exist — `crate::ui::primitives` exposes `ease_out()` and `ease_in_out()` (`ui/primitives.rs:242-252`). For the indicator, render one absolutely positioned pill inside the tab group and animate its `left` between slots.
- Acceptance: menus fade and slide in as they already fade and slide out, and the mode indicator slides between tabs.

### 222. Target-menu popover has no minimum width

- Type: visual.
- Electron: `src/renderer/components/area-overlay/capture-target-menu.tsx:36` sets `className="min-w-40"` on the popover.
- GPUI: `capture/all_in_one_toolbar.rs:216-222` uses `MenuPlacement::below(TARGET_MENU_ID)` with no width builder, unlike `windows/recording_control.rs:1280-1288`, which sets both.
- Fix: add `.min_width(px(160.0))` to the placement; `MenuPlacement::min_width(Pixels)` exists at `ui/menu/mod.rs:112-115` and is plumbed through at `:231,242-245`.
- Acceptance: the target menu is at least 160px wide regardless of its longest label.

### 223. Device menu width is pinned instead of elastic

- Type: visual.
- Electron: `recording-control-window.tsx:183,302` uses `max-w-64 min-w-56` (224 to 256) on both the media and iOS popovers.
- GPUI: `windows/recording_control.rs:33` `DEVICE_DROPDOWN_WIDTH = 256.0` is used for both `.min_width` and `.max_width` (`:1281-1283`).
- Fix: add `DEVICE_DROPDOWN_MIN_WIDTH = 224.0` and pass it to `.min_width`, keeping 256 as the max; the centring offset at `:1285-1288` must keep keying off the max or a narrow menu will be mis-anchored.
- Acceptance: a device menu with short labels renders at 224px and long labels expand it to 256px.

### 224. Overlay-surface tooltips are styled where Electron uses the native `title` attribute

- Type: visual.
- Electron: `recording-control-window.tsx:58-68` `ControlButton` forwards `aria-label` and `title` only, `scroll-capture-control-window.tsx:45-69` does the same, and the editor zoom cluster uses `title="Zoom Out" / "Reset Zoom" / "Zoom In"` (`src/renderer/components/editor/zoom/index.tsx:36,42,52`). The styled tooltip is used only in `all-in-one-toolbar.tsx:105-138`.
- GPUI: `ui/toolbar.rs:80-92` `tooltip_button` routes through `icon_button::with_tooltip` to `herogpui::components::Tooltip`, used by `overlay_icon` (`windows/recording_control.rs:1459-1466`), `ScrollControlBar::button` (`windows/scroll_capture.rs:344-350`) and the editor zoom cluster (`editor/window.rs:2728-2748`).
- Fix: record this as a deliberate deviation. The GPUI behaviour is the more consistent one — every other chrome button in the shell uses the styled tooltip — and these surfaces have no OS chrome, so removing accessible tooltips would be a regression. If strict parity is ever required, drop `tooltip_button` on those three clusters and keep only the accessible name.
- Acceptance: the decision is recorded here and the three clusters are consistent with it.

### 225. Area overlay shows a persistent idle hint chip that Electron does not

- Type: visual.
- Electron: the idle overlay is a plain dim with no hint; once dragging, an accent selection box with eight square handles appears with a dark rounded size chip anchored below-left (`electron-area-overlay-idle.png`, `electron-area-overlay.png`).
- GPUI: the idle overlay shows a dark chip reading "Drag to select an area · Esc to cancel" near the top-left (`gpui-area-overlay.png`); the mid-drag state could not be driven, so only the idle state is compared.
- Fix: this is the same prompt pill as items 206 and 207, so resolve it there — decide whether the reference behaviour (no idle hint outside the intents that ask for one) or the GPUI behaviour is wanted, and make `capture/intent.rs::prompt()` and `src/renderer/components/area-overlay/` agree. Do not fix it in only one shell.
- Acceptance: both shells show the same idle-overlay affordance for the same capture intent.

## P2 — Settings

### 226. GPUI exposes a "Scroll Capture" shortcut row that Electron's settings do not

- Type: behavior.
- Electron: extracting every id from `src/renderer/components/settings/registry/shortcuts.ts` yields 36, none of them `shortcuts.scrollCapture`; the field exists in config (`src/types/settings.ts:264,465`) and is a live accelerator (`src/main/menu/index.ts:268-271`), so it is genuinely user-invisible in Electron.
- GPUI: `windows/settings/shortcut_items.rs:117-126` declares a `Spec` for `shortcuts.scrollCapture` in the OTHER section, making 37 ids. The set difference is exactly that one entry.
- Fix: remove the `Spec` unless product wants the row added to Electron first — items must be 1:1. Keep the `Feature::ScrollCapture` enum variant, which the scroll-capture surfaces use.
- Acceptance: both shells expose the same 36 shortcut ids.
- Decision (implemented): the `Spec` was removed, leaving 36 ids. `Feature::ScrollCapture` and the `shortcuts.scrollCapture` config field stay; the accelerator remains live through the menu. `windows/settings/shortcut_items.rs` now scrapes `registry/shortcuts.ts` in `the_shortcut_ids_match_the_renderer_registry_exactly` and fails on any id either shell adds or drops.

### 227. "System Default" never shows the resolved default device name

- Type: visual.
- Electron: `src/renderer/components/settings/devices/device-select.tsx:27-33` builds `System Default (${defaultDevice.label})` from `defaultDeviceId` supplied by `useMediaDevices()` (`microphone-device-setting.tsx:22,77`, `camera-device-setting.tsx:22`).
- GPUI: `system/devices.rs:50` hardcodes `"System Default"`, and `windows/settings/item_row.rs:359-362` calls `options_with_selection` without passing `lists.default_microphone_id` / `default_camera_id`, even though both are parsed (`devices.rs:76-79`); the `Select` placeholder is the bare string too (`item_row.rs:374`).
- Fix: thread the default ids into `options_with_selection` and format the first entry the way Electron does when a default is resolvable.
- Acceptance: the first device option reads `System Default (MacBook Pro Microphone)` or equivalent.

### 228. Unavailable device label uses the raw id, not the persisted name

- Type: visual.
- Electron: `device-select.tsx:35-42` renders `${selectedName ?? 'Unknown device'} (unavailable)`, where `selectedName` is the persisted `settings.recording.selectedMicName` or `camera.selectedDeviceName`.
- GPUI: `system/devices.rs:52-58` renders `Unavailable ({selected_id})` — the id, in a prefix form — and `item_row.rs:340-397` reads only the selected ids; the persisted names are written on selection (`:383-394`) but never read back.
- Fix: pass the stored name into `options_with_selection`, format `"{name} (unavailable)"` and fall back to `"Unknown device"` only when no name was stored. The existing test `device_options_keep_default_and_disconnected_selection` (`devices.rs:99-107`) asserts the current string and must be updated in the same change.
- Acceptance: unplugging the selected microphone shows its last-known name followed by `(unavailable)`.

### 229. Naming-pattern token list is a hover tooltip in Electron and a click disclosure in GPUI

- Type: behavior.
- Electron: `src/renderer/components/settings/setting-item-renderer.tsx:113-135` wraps a `HelpCircle` in a `Tooltip` with `TooltipContent side="right"` holding a token, description and example table, opened on hover or focus.
- GPUI: `windows/settings/item_row.rs:518-540` has a hover tooltip that only says "Available tokens", and pressing the button toggles an inline disclosure of clickable token chips appended to the page (`:505-509,558-560`).
- Fix: do not file this as "swap in `Tooltip`". `herogpui::components::Tooltip::new` takes `impl Into<SharedString>` only (`herogpui-components-0.9.0/src/tooltip.rs:264,281`) and its children are the trigger, not the content, so HeroGPUI 0.9.0 cannot host that table in a tooltip. Either accept the deviation and record it — the GPUI version is strictly more capable, since the chips insert tokens — or build a hover-opened popover.
- Acceptance: the decision is recorded here, and if accepted the GPUI tooltip copy names what the disclosure does.
- Decision (implemented): the deviation is accepted. HeroGPUI 0.9.0's `Tooltip` cannot host the token table, and the click disclosure is strictly more capable because its chips insert tokens. The tooltip copy in `windows/settings/item_row.rs` now reads "Show available tokens — click one to insert it".

### 230. Shortcut input has no non-compact variant

- Type: visual.
- Electron: `src/renderer/components/settings/shortcut-input.tsx:136-170` varies row padding (`min-h-10 py-1` vs `py-2`), gap (`gap-1` vs `gap-2`), button size (`sm` vs `default`), text size (`text-sm` vs `text-base`) and min-width (`min-w-16`/`min-w-36` vs `min-w-[80px]`/`min-w-[180px]`). `compact` is true only for the Shortcuts category (`settings-category-page.tsx:99`); search results (`settings-search-results.tsx:65`) and onboarding (`src/renderer/windows/onboarding-window.tsx:249-265`) get the non-compact widget.
- GPUI: `windows/settings/item_row.rs:142-159` does not forward its `compact` parameter, and `ui/shortcut_input.rs:82-103` always uses `Size::Sm` with `SHORTCUT_MIN_WIDTH = 144.0` / `SHORTCUT_MIN_WIDTH_SINGLE = 64.0` and `SHORTCUT_GAP = 4.0` (`ui/chrome.rs:77-80`) — Electron's compact numbers only. Both callers, including `windows/onboarding.rs:350`, hit the same function. Row padding is not part of this: `labelled` (`item_row.rs:237,653-673`) already applies `min_h(40)` and `py(4)` in compact mode and `mod.rs:981,1090,1144` pass the right flag, so adding padding in the control would double it.
- Fix: thread `compact` into `shortcut_input::render` and branch only the button `Size` (Sm vs Md), the min-widths (144/64 vs 180/80), the text size and the gap — add `SHORTCUT_MIN_WIDTH_DEFAULT = 180.0`, `SHORTCUT_MIN_WIDTH_SINGLE_DEFAULT = 80.0` and `SHORTCUT_GAP_DEFAULT = 8.0` — and pass `compact: false` from `onboarding.rs:350`.
- Acceptance: a shortcut row in settings search and in onboarding renders at Electron's default size, while the Shortcuts page stays compact.

### 231. Sidebar search box activates on a non-empty query instead of on focus

- Type: visual.
- Electron: `src/renderer/components/settings/settings-sidebar.tsx:49` applies `focus-within:bg-[var(--row-active)] focus-within:text-foreground` with `hover:bg-[var(--row-hover)]`, so the active background appears on focus regardless of the text.
- GPUI: `windows/settings/mod.rs:810-811` keys the active background off `searching = !window.search_query(cx).is_empty()` (`:687`) and never consults focus. The hover-wins precedence matches.
- Fix: no separate flag is needed — the window already holds the field's focus handle (`mod.rs:400`), so use `self.search.read(cx).focus_handle(cx).is_focused(ui_window)` (`gpui-pre-0.3.5/src/window.rs:498`) as the condition. The clear button's own `searching` gate (`mod.rs:826`) must stay value-driven, matching `settings-sidebar.tsx:57`.
- Acceptance: clicking into the empty search box lights the active background.

### 232. Shortcut recording commits on key-down rather than key-up

- Type: behavior.
- Electron: `shortcut-input.tsx:76-118` commits single-key fields on key-down (`:89-94`) but stages accelerator combinations and commits them on key-up (`:111-118`).
- GPUI: `windows/settings/mod.rs:387-390` commits through `write_recorded_shortcut` straight from `on_key`'s `KeyDownEvent` for both kinds (`:356-365`). So single-key items already agree; the delta is confined to modifier combinations.
- Fix: buffer the candidate combination in the non-single-key path only and commit on key-up. gpui has `KeyUpEvent` and `on_key_up`, but the settings window currently registers only `on_key_down`. Accepting this as a documented low-risk timing deviation is also defensible.
- Acceptance: recording a modifier combination commits when the last key is released, or the deviation is recorded here.

### 233. Settings sidebar rows are taller and start lower

- Type: visual.
- Electron: sidebar buttons are 32px tall on a 34px pitch with the first row at y=80, giving centres at 96, 130, 164, 198, 232, 266, 300 and 334 (measured from the settings window's AX tree).
- GPUI: the same rows sit at centres 109, 145.5, 182.5, 218.5, 255.5, 291.5, 328.5 and 364.5 — a 36.5px pitch starting 13px lower; About is at the same bottom anchor in both, so the gap above it is 30px smaller in GPUI.
- Fix: in the GPUI settings sidebar under `src/main/app-gpui/src/windows/settings/`, set the category row height to 32px and the row gap to 2px and reduce the sidebar's top padding by about 13px so the first row lands at y=80.
- Acceptance: the eight category rows land on the same centres as Electron's.

### 234. Settings content rows are about 4px taller, accumulating about 32px of drift down the page

- Type: visual.
- Electron: on the Screenshot page the row baselines sit at 216, 329, 441, 553, 665, 793, 905, 1033 and 1161 (2x) — a 112px (56pt) pitch, 128px where a section gap falls.
- GPUI: the same rows sit at 216, 337, 457, 577, 697, 833, 953, 1089 and 1225 — a 120px (60pt) pitch and 136px with the gap. The first row aligns exactly; by the last row GPUI is 32pt lower.
- Fix: reduce the settings row vertical padding by 2pt per side in the GPUI settings row widget so the pitch is 56pt, matching Electron's `SettingRow`.
- Acceptance: the last row on the Screenshot page lands within 2pt of Electron's.

### 235. Settings content column has less right padding, so text wraps later

- Type: visual.
- Electron: the About blurb wraps after "macOS and" at x≈1622 (2x), and the Screenshot "Multi-image layout" description wraps after "edge of the".
- GPUI: the same blurb fits on one line and runs to x≈1700, and the Multi-image description wraps one word earlier, because the wrap box is positioned differently.
- Fix: match the settings page container's right padding to Electron's — the GPUI page appears to have about 24pt less — in `src/main/app-gpui/src/windows/settings/`.
- Acceptance: both pages wrap their descriptions at the same words as Electron.

### 236. Select controls wrap their label differently

- Type: visual.
- Electron: the Screenshot page's "Multi-image layout" select renders "Right (side-by-" / "side)".
- GPUI: the same select renders "Right" / "(side-by-side)" in a box of the same outer width (x 1392–1710 vs 1392–1712, 2x), so the inner text box is narrower — more horizontal padding or a different trigger inset.
- Fix: align the HeroGPUI select recipe's trigger padding with Electron's `px-3`, or give the label the same available width.
- Acceptance: the select trigger breaks its label at the same point as Electron's.

### 237. The About wordmark's accent dot is mispositioned

- Type: visual.
- Electron: the wordmark reads "Pora.take" with the accent dot on the baseline, tight against both words.
- GPUI: it reads "Pora take" with the dot rendered small and low between the words, leaving visible space on both sides. This is the visible symptom of the alignment-model difference noted in the corrections section: `windows/settings/about.rs:61-88` reproduces the horizontal and size math of `src/renderer/components/brand-logo.tsx:24-30` exactly, but replaces the baseline-aligned `overflow-visible` SVG (where `cy=96, r=9` puts the dot 1.2px below the 24px box) with `items_end` inside an `h(29.0)` row plus `bottom(px(1.0))`.
- Fix: render the dot as part of the same text run, or give it a fixed baseline offset with zero side padding, in the GPUI About header.
- Acceptance: the wordmark renders as "Pora.take" with no gap on either side of the dot.

---

## P2 — Small windows, tray, preview and pin

### 238. No "operation finished" confirmation state, and no copy/export progress bar

- Type: animation.
- Electron: `src/renderer/windows/capture-preview-window.tsx:118` derives `isFinished = isDone || isUploaded`, rendered at `:334-342` as a `bg-black/50` scrim with a 40px `bg-foreground` circle containing a `Check strokeWidth={3}`, entering with `animate-in duration-300 zoom-in-50`, and `showControls` (`:282-283`) is suppressed while finished. Separately, `:344-355` renders a copy/export progress bar while `isCopying`, driven by `copyProgress` (`src/renderer/hooks/use-video-clipboard-export.ts:155-157`).
- GPUI: `windows/capture_preview.rs` has no finished, done or completed state — searching `finish`, `Completed`, `done`, `copying`, `check` and `zoom` yields only an unrelated `trailing_check` at `:1803` — and `ui/preview.rs` renders no check, finish or progress element at all. The copy and polish handlers (`:1548-1558`, `:1626-1636`) dismiss immediately on success and the upload path (`:1729-1775`) shows a toast and dismisses after `UPLOAD_DONE_DISPLAY_MS`. `start_video_export` discards progress entirely (`:697`, `&mut |_| {}`) and shows a static "Exporting..." pill (`:1526-1535`). GPUI is not silently dropping feedback — it substitutes a dismissal plus a toast — which is why this is a missing visual confirmation rather than lost information.
- Fix: add a `Completed` state alongside `busy` on `CapturePreview`, set it on upload and export success, render the scrim and check with a 300ms `zoom-in-50` scale (0.5 to 1, not a generic scale-in) using `gpui::AnimationExt::with_animation` as `ui/primitives.rs:110-145,256-275` already does, and hold it briefly before `begin_remove_preview` fires or the badge will never be seen. Thread the real export progress through to an in-tile bar. `preview::circle` has no animation helpers of its own; model it on `primitives.rs`.
- Acceptance: a finished copy, export or upload shows the check badge for its hold duration, and a video copy shows a moving progress bar in the tile.

### 239. Pin window close affordance differs in position and style

- Type: visual.
- Electron: `src/main/capture/screenshot/pin.ts:75` uses `titleBarStyle: 'customButtonsOnHover'` with `frame: false`, and `src/renderer/windows/pin-window.tsx:37-55` renders only the drag container and the `<img>` — no in-content close control on either platform.
- GPUI: `windows/pin.rs:125-147` renders a black 24px circular "x" at top 6, right 6 on hover, on every platform.
- Fix: do not gate this to Windows — that would leave the macOS pin window with no close affordance at all and would lose the editor-restore behaviour (`pin.rs:90-96` reopens the editor with persisted state, which the Electron traffic light does not do), and GPUI popup windows open with `titlebar: None` (`windows/mod.rs:84-99`, asserted at `pin.rs:172`), so native traffic lights are unavailable for this window kind. Keep the custom button on both platforms as a deliberate improvement and record it, or move it to the top-left to match the traffic-light position if visual parity is required.
- Decision (recorded): the custom hover close button stays on both platforms, at its current top-right position. GPUI popup windows open with `titlebar: None`, so native traffic lights are not available for this window kind, and the button also carries the editor-restore behaviour Electron's traffic light does not have.
- Acceptance: the decision is recorded here and the pin window is closable on both platforms.

### 240. History popover renders opaque where Electron is vibrant

- Type: visual.
- Electron: `src/main/history/popover.ts:45-63` sets `vibrancy: 'popover'`, `visualEffectState: 'active'`, `backgroundColor: '#00000000'` and `transparent: false`.
- GPUI: `windows/history/mod.rs:103` sets `background: gpui::WindowBackgroundAppearance::Opaque`. macOS blur is already proven in this shell — `intents.rs:179` sets `WindowBackgroundAppearance::Blurred` unconditionally on all platforms for the settings window, with the Windows acrylic call at `:183-185` as an extra step.
- Fix: set `Blurred` on macOS and keep `Opaque` on Windows, matching Electron, where `vibrancy` is macOS-only; the popover currently paints its own opaque surface behind the content, so that fill must become translucent for the blur to show. Whatever gating item 248 adds must cover this surface too.
- Acceptance: the history popover shows the desktop through a blur on macOS and stays opaque on Windows.

### 241. History popover is placed on the tray's display instead of the primary display

- Type: behavior.
- Electron: `src/main/history/popover.ts:21-37` computes placement against `screen.getPrimaryDisplay()`.
- GPUI: `windows/history/mod.rs::popover_placement` places the popover on the tray's display. This also corrects the audit's "line-for-line" claim about the history popover geometry.
- Fix: decide which is wanted. The GPUI behaviour is arguably better on a multi-display setup, so the cheapest resolution is to record it as a deliberate improvement; otherwise key the placement off the primary display.
- Decision (recorded): the GPUI behaviour stays — the history popover opens on the display that owns the tray icon, which is where the user just clicked, rather than always on the primary display.
- Acceptance: the decision is recorded here and both shells place the popover consistently with it.

### 242. History header typography differs

- Type: visual.
- Electron: "History" is semibold with its baseline at y=52 (2x); "Clear All" starts at x=594 with the trash glyph at x=553, and the gear sits at x=740.
- GPUI: "History" renders at regular weight with baseline y=49; "Clear All" is set larger and starts at x=543 with the trash at x=524, so the whole cluster sits about 50px (2x) further left and the gap to the gear is bigger.
- Fix: set the history title to Electron's weight and size and reduce the "Clear All" label to Electron's size, after which the cluster re-registers against the gear.
- Acceptance: the header title weight and the Clear All cluster positions match the Electron screenshot within 2px.

### 243. Tray menu renders modifiers in the wrong order with taller rows

- Type: visual.
- Electron: the native NSMenu shows "⇧⌘S", "⇧⌘3", "⇧⌘4" and "⇧⌘2" — the macOS convention of shift before command — with a roughly 28px item pitch and inset separators.
- GPUI: the GPUI-drawn menu shows "⌘⇧S", "⌘⇧3", "⌘⇧4" and "⌘⇧2" with a roughly 32px pitch and full-width separators. The item set, order and icons are otherwise identical.
- Fix: order the modifier glyphs ⌃⌥⇧⌘ the way AppKit does in the GPUI accelerator formatter under `src/main/app-gpui/src/system/tray/`, tighten the item height to 28px and inset the separators.
- Acceptance: the tray menu's accelerators read in AppKit order and the row pitch matches the native menu.

---

## P2 — Cross-cutting design system and motion

### 244. Menu close loses the zoom-out

- Type: animation.
- Electron: `src/renderer/components/ui/context-menu.tsx:84,101` carry `data-[state=closed]:animate-out fade-out-0 zoom-out-95`, and the HeroUI `Dropdown.Popover` path gets the same zoom from HeroUI v3's own `[data-exiting]` CSS at `duration-100` (`capture-target-menu.tsx:36` carries no animate-out classes itself, correcting the original citation).
- GPUI: `ui/menu/mod.rs:387-401` wraps a closing popup in `overlay_exit`, which animates only `opacity(1.0 - delta)` and a 4px `mt` translate (`ui/primitives.rs:125-146`) — no scale anywhere in the menu path. `ui/menu/view.rs:94-140` sets `.animate_entry(...)` but never `.exiting(...)`, and `MenuView` has no closing field.
- Fix: thread a `closing` flag from `MenuHandle` into `MenuView` and call `.exiting(true)` on the underlying HeroGPUI `Menu` — `herogpui_components::dropdown::Menu::exiting(bool)` exists (`herogpui-components-0.9.0/src/dropdown.rs:371`, consumed at `:1546-1552`) and routes the panel through `Motion::LIST_OUT` = `{ ms: 100, scale: 0.95, curve: Smooth }` (`anim.rs:236-240`), i.e. exactly `duration-100 ease-smooth zoom-out-95`. Prefer that over hand-extending `overlay_exit`, so the curve and ZoomBox come from HeroGPUI.
- Acceptance: closing any menu or dropdown scales to 0.95 while fading over 100ms.

### 245. `--surface-shadow` and `--overlay-shadow` overrides never reach HeroGPUI

- Type: visual.
- Electron: `src/renderer/styles/base.css:54-55` sets `--surface-shadow: 0 20px 60px rgba(0,0,0,0.08)` and `--overlay-shadow: 0 20px 60px rgba(0,0,0,0.2)`, overriding HeroUI's stock values.
- GPUI: `theme/bridge.rs:157-213` sets radius, cursor, tooltip delay, colours and component themes but never touches layout shadows, and the comment at `:158-160` says the unset roles keep their HeroUI defaults. HeroGPUI's `LayoutTheme::dark()` (`herogpui-theme-0.9.0/src/layout.rs:106-144`) sets `surface_shadow`, `overlay_shadow` and `field_shadow` to empty vectors and `overlay_hairline` to `hsla(0,0,1,0.3)`, so dark mode gets an inset hairline instead of a shadow.
- Fix: assign `theme.layout.surface_shadow` and `theme.layout.overlay_shadow` after `.build()` in `bridge.rs`, the way `theme.colors.scrollbar` is already assigned at `:203`; both are public `Vec<BoxShadow>` fields (`layout.rs:43,45`). Clear `overlay_hairline` in dark at the same time, or the hairline and the new shadow will both paint.
- Acceptance: panels and overlays cast the Electron shadow in both light and dark, with no double hairline.

### 246. Indeterminate progress bar is 6px where the determinate bar is 8px

- Type: visual.
- Electron: `src/renderer/components/ui/progress.tsx:33` uses a single `h-2` (8px) track for both determinate and indeterminate modes (`:21,28`).
- GPUI: `ui/primitives.rs:254-279` hard-codes `.h(px(6.0))` for the hand-rolled indeterminate bar (used at `windows/video_editor/panels.rs:2328`), while the determinate path uses `herogpui::ProgressBar` whose default `Size::Md` track is 8px (`herogpui-components-0.9.0/src/progress.rs:270`) — so two bars in the same window differ by 2px.
- Fix: there is no `ProgressBar` recipe in `bridge.rs` to take a token from, so either change `px(6.0)` to `px(8.0)`, or better, delete the hand-rolled helper and use `herogpui::ProgressBar` in its indeterminate mode so both bars are one code path. The hand-rolled track also paints `theme.muted_background` where the component paints `colors.default.color`; reconcile that in the same change.
- Acceptance: both progress bars in the export panel are 8px tall and the same colour.

### 247. Focus-ring colour uses the 52%-alpha ring token instead of the full-opacity accent

- Type: visual.
- Electron: HeroUI stock maps `--focus: var(--accent)` and `base.css` never overrides it; `--ring` (`base.css:69`, accent at 52%) is a separate Tailwind token used only by `outline-ring/50` (`:111`) and `--color-ring` (`:95`).
- GPUI: HeroGPUI stock also maps focus to the accent (`herogpui-theme-0.9.0/src/semantic.rs:422,491`), and `ThemeBuilder::role("accent", ...)` already sets `colors.focus` (`theme.rs:283-287`) — so the accent role assignment at `bridge.rs:187` had it right. Then `bridge.rs:204` undoes it with `theme.colors.focus = vars.ring;`, and `colors.focus` drives HeroGPUI field rings (`herogpui-components-0.9.0/src/anim.rs:1745`, `util.rs:398`, `combo_box.rs:1976`). The app's own hand-rolled ring uses full accent (`ui/primitives.rs:57`), so the two ring implementations already disagree with each other.
- Fix: delete `bridge.rs:204` and update the stale comment at `:200-202`.
- Acceptance: a focused field's ring is the full-opacity accent and matches the app-drawn ring.

### 248. Native-material vibrancy is unconditional and ignores reduced transparency

- Type: behavior.
- Electron: `src/main/utils/title-bar.ts:25-51` gates acrylic on Windows build 22621 and returns vibrancy on macOS; `src/renderer/App.tsx:141-160` reads `matchMedia('(prefers-reduced-transparency: reduce)')` and skips `settings:apply-window-material` entirely when reduced.
- GPUI: `intents.rs:179-189` sets `window_background = WindowBackgroundAppearance::Blurred` unconditionally and then calls `configure_acrylic_surface` on Windows. The OS-version half is largely a non-issue in practice — `system/window_composition.rs:177-203` calls `DwmSetWindowAttribute(DWMWA_SYSTEMBACKDROP_TYPE, DWMSBT_TRANSIENTWINDOW)` and returns false on failure, so pre-22621 degrades on its own — and `nativeMaterial` is OS-capability-derived rather than a user setting, so there is no missing toggle either. The real gap is reduced transparency: grepping `reduce_transparency`, `reduced_transparency` and `ReduceTransparency` across `app-gpui` returns zero hits.
- Fix: mirror only the reduced-transparency preference, and gate `window_background` as well as the DWM call. gpui exposes no query for it, so this needs a platform read — macOS `NSWorkspace.accessibilityDisplayShouldReduceTransparency`, Windows `SystemParametersInfo(SPI_GETCLIENTAREAANIMATION)` or the transparency-effects registry value.
- Acceptance: with reduced transparency enabled, GPUI windows render opaque.

### 249. Context-menu exit duration is 100ms where Radix uses 150ms

- Type: animation.
- Electron: `node_modules/tw-animate-css/dist/tw-animate.css` defaults `--animate-out` to 150ms with CSS `ease`, and `context-menu.tsx:84,101` sets no override, so those two surfaces exit in 150ms. The HeroUI v3 surfaces exit at `duration-100`, which HeroGPUI pins as `anim::EXITING_MS = 100` (`herogpui-components-0.9.0/src/anim.rs:1167-1168`).
- GPUI: `ui/primitives.rs:77-78` sets `OVERLAY_EXIT_MS = 100` with `ease_out()` (`:141`), which already matches the HeroUI dropdowns.
- Fix: do not blanket-align to 150ms — that would break parity with the HeroUI dropdowns. Either accept 100ms everywhere, which is HeroUI v3's own convention and what item 244's `Motion::LIST_OUT` fix locks in, or split the constant so the context-menu surface alone runs 150ms with CSS `ease` = `cubic_bezier(0.25, 0.1, 0.25, 1.0)` (`ui/primitives.rs:148`). The former is recommended and makes item 244 a one-line change.
- Acceptance: the decision is recorded here and every overlay exit uses the chosen duration.

### 250. `cursor-wait` and `cursor-not-allowed` states have no GPUI equivalent

- Type: visual.
- Electron: `cursor-wait` at `src/renderer/components/shared/background-selector.tsx:334`, `components/editor/wallpaper/background-editor.tsx:288` and `components/editor/wallpaper/index.tsx:470`; `cursor-not-allowed` at `background-selector.tsx:335`, `wallpaper/index.tsx:471`, `src/renderer/windows/capture-preview-window.tsx:428,437,463,473` and `components/ui/textarea.tsx:12`.
- GPUI: the full `CursorStyle::` inventory across the tree is Arrow, Crosshair, ClosedHand, PointingHand and the four resize variants — no busy or disabled cursor anywhere.
- Fix: `cursor-not-allowed` maps cleanly to `CursorStyle::OperationNotAllowed`; thread the disabled state through to the cursor at those call sites. `CursorStyle::Wait` does not exist in gpui-pre 0.3.5 (`gpui-pre-0.3.5/src/platform.rs:2434-2510`), so the busy half must be dropped from the fix and covered by the existing in-app spinner affordance instead.
- Acceptance: disabled controls show the not-allowed cursor, and the absence of a wait cursor is recorded under accepted platform limits.

### 251. `oklch()` parsing mishandles percentage lightness and percentage alpha

- Type: visual.
- GPUI: `theme/color.rs:155-169` `from_oklch_str` parses tokens with `.trim_end_matches('%').parse::<f32>()` and hands them to `from_oklch`, which treats lightness as a 0..1 fraction — so `oklch(70% 0.1 200)` yields lightness 70.0, not 0.7. The alpha branch at `:165-168` parses the `/ <alpha>` token with a bare `parse::<f32>()` and never strips `%`, so `oklch(L C H / 50%)` silently falls back to alpha 1.0.
- Electron: every literal in the repo uses the fraction form (`theme/vars.rs:126-133`), so both bugs are latent today — but `heroui.min.css` does use the percentage form, so the risk is real.
- Fix: handle both fraction and percentage forms for lightness and for alpha in the same change to `from_oklch_str`.
- Acceptance: unit tests cover `oklch(70% 0.1 200)` and `oklch(0.7 0.1 200 / 50%)` and both parse to the expected values.

### 252. Gamut clamping differs from the CSS spec

- Type: visual.
- GPUI: `theme/color.rs:95-100` `from_oklab` applies `clamp01(linear_to_srgb(..))` independently to r, g and b, where CSS Color 4 gamut-maps by binary-searching OKLCh chroma down at fixed lightness and hue; per-channel clipping shifts hue for out-of-gamut colours.
- Electron: the browser implements the spec algorithm. No current preset lands out of gamut, so this is latent.
- Fix: align the clamping with the spec's chroma-reduction approach. This is a correctness-only change with no visible effect today, so weigh it against the project's bias against speculative work; if taken, pair it with item 251 in one `color.rs` change.
- Acceptance: an out-of-gamut OKLCh colour resolves to the same sRGB triple as the browser's, within one 8-bit step.

### 253. Three `ThemeVars` fields have no downstream readers

- Type: visual.
- GPUI: only `card_foreground`, `secondary_foreground` and `destructive_foreground` have zero reads. The fields the original report named as unused all have readers — `popover` (`windows/scroll_capture.rs:301`, `windows/video_editor/data_editor.rs:201`, `editor/wallpaper_sheet.rs:138` and more), `card` (`windows/video_editor/mod.rs:2607,2640` and four more), `primary` (42 reads), `secondary` (`windows/history/item.rs:247,374`), plus `popover_foreground`, `primary_foreground` and `input` with one or two each.
- Electron: the corresponding CSS variables are all live in `base.css`, so the deltas are GPUI-side dead code, not a parity gap.
- Fix: restrict the removal to those three fields, following the project rule to verify before removing; drop their computation in `theme/vars.rs` and any preset-parity assertions that cover them.
- Acceptance: the three fields are gone and `cargo test` still passes on the theme parity tests.

### 254. The app's `--accent-hover` never reaches HeroGPUI

- Type: visual.
- Electron: `src/renderer/theme/app-theme.ts:38-40,52` computes `accentHoverTarget = accentFg === '#ffffff' ? '#000000' : '#ffffff'` and sets `--accent-hover` to `mix(variant.accent, 90, accentHoverTarget)`, written with `root.style.setProperty` (`:84,93`), which beats the stylesheet — so HeroUI components get the app's value, deliberately mixed toward the inverse of the accent foreground. HeroUI stock is the opposite mix.
- GPUI: `theme/vars.rs:118-122,156` reproduces that computation exactly, but `theme/bridge.rs:187` passes only `.role("accent", vars.accent, vars.accent_foreground)`, and `ThemeBuilder::role` (`herogpui-theme-0.9.0/src/theme.rs:265-290`) sets only colour and foreground, leaving `RoleColor::hover_mix = 0.10` of the foreground (`semantic.rs:49-56`). Every HeroGPUI accent control therefore hovers toward the accent foreground, the opposite direction from Electron; `vars.accent_hover` is consumed by exactly one app-drawn surface (`windows/video_editor/timeline/tracks.rs:172`). `default_hover` and `danger_hover` do reproduce HeroUI stock, so only the accent role diverges. This is the net-new finding behind the audit line 36 correction.
- Fix: expose the app's accent hover to HeroGPUI — either a `hover_mix`-equivalent override on the accent `RoleColor` if one exists, or a component recipe carrying `ComponentColor::Literal(vars.accent_hover)` for the hover state, the way `bridge.rs:70` already overrides menu row hover with `ComponentColor::RoleHover(Color::Default)`.
- Acceptance: hovering an accent-filled HeroGPUI button darkens or lightens it in the same direction and to the same value as Electron's.

---

## Items found after the first pass (255-257)

A library-reuse audit run after items 85-254 closed turned up three deltas the
original survey missed. 255 and 256 are app-side and were fixed immediately. 257 is
a functional hole, not a polish delta, and is blocked upstream.

### 255. The "or" divider draws its label on the rule instead of above it

- Type: visual.
- Electron: draws a `border-t` rule and floats the label above it with a `-top-2.5` span, so the rule stays visible behind a gap.
- GPUI: `windows/video_editor/panel_kit.rs:651-677` centres the label on the rule, painting over the line.
- Fix: match the reference construction, using `herogpui::Separator` and the `--separator` token rather than `--border`.
- Acceptance: the label sits clear of the rule and the rule is visible behind it.

### 256. Background type toggle segments are not focusable

- Type: behavior.
- Electron: the two segments are real `button` elements, so they are reachable and activatable from the keyboard.
- GPUI: `editor/wallpaper_sheet.rs:312-368` renders them as plain `div`s, so neither segment can be focused or triggered by keyboard.
- Fix: make both segments focusable and keyboard-activatable with the shell's focus-visible treatment. HeroGPUI 0.9.0 `Tabs` cannot host an icon-only segment (no element trigger on `TabItem`, see upstream ask 10), so a focusable hand-rolled segment is correct here.
- Acceptance: both segments can be reached and activated from the keyboard and show the focus ring.

### 257. Six scroll surfaces have no scrollbar at all

- Type: visual, and functional rather than cosmetic.
- Electron: specifies three distinct scrollbars plus a hidden variant — an 8px global track with a `rgba(100,100,100,0.5)` thumb going to `0.7` on hover (`src/renderer/styles/app.css:46-64`), a 12px `.scrollbar-overlay` (`base.css:291-311`), an 8px `.scrollbar-overlay-vertical` with a 2px inset (`base.css:320-341`), and `.scrollbar-hide` for surfaces that must show none (`app.css:66-72`).
- GPUI: no scrollbar is rendered on any of the six scroll surfaces, because `herogpui::Scrollbar` cannot be styled to match: thickness is a private `const TRACK: f32 = 8.0` (`scrollbar.rs:18`), the thumb is a single theme token with no override (`:102`), visibility is forced through `should_auto_hide_scrollbars()` with no opt-out (`:98-101`), radius is derived from the track (`:164`), and there is no `sx`.
- Fix: blocked on upstream ask 11, which requests `track`, `inset`, `radius`, `thumb_color`, `thumb_hover_color`, `auto_hide` and `sx`. Adopt `Scrollbar` on all six surfaces once 0.9.1 lands.
- Acceptance: each scroll surface shows the scrollbar its Electron counterpart specifies, and the hidden variant shows none.

### 258. Unselected background-type segment has no hover state

- Type: visual.
- Electron: the unselected segment carries `hover:bg-muted` with `transition-colors` (`src/renderer/components/editor/wallpaper/background-editor.tsx:158-181`).
- GPUI: `editor/wallpaper_sheet.rs` painted a static unselected background with no hover response.
- Fix: hover the muted background on unselected segments only.
- Acceptance: hovering an unselected segment fills it with the muted background; the selected one does not change.

## Upstream asks filed (2026-09-17)

Five items are blocked on HeroGPUI capabilities rather than on app code. They were
filed as one thread in the HeroGPUI project titled "Parity gaps blocking Poratake's
GPUI shell (5 asks)", each with source-level evidence and an acceptance note:

1. `MenuStyle` separator inset and thickness, for item 243. The geometry is hardcoded at `dropdown.rs:1033-1039` and no theme can reach it; a real AppKit separator is inset 15pt per side and 1pt thick.
2. `SelectStyle::padding_y` on the trigger, for the residual half of item 234.
3. A way for a menu row to host an interactive control, for item 200.
4. Per-row leading content in `Select` and `PickerItem`, for item 140.
5. An element as tooltip body rather than only a string, for item 229.

The thread also carries a papercut report about `button.rs:612` dropping a recipe's
variant and hover colour, and a for-awareness list of gpui-level limits that are not
HeroGPUI's to fix: the hyphen line-breaking difference behind item 236, the
variable-font weight-matching trap behind item 138, and the missing blend mode,
transform, dashed border, tabular numerals and cursor styles behind several P2 items.

When HeroGPUI lands these, bump the pin and keep `gpui-pre` in lockstep with the
version HeroGPUI's workspace depends on, as `CLAUDE.md` requires, then close items
140, 200, 229, the residual of 234 and the separator half of 243.

## Tabs adoption deferred, and one correction to our own ask (2026-09-18)

HeroGPUI 0.10.0 shipped every capability asked for, and eleven of the twelve are
adopted. The capture toolbar's mode pills stay hand-rolled, and the reason is a gap in
our ask rather than in the delivery.

Ask 10 requested `TabItem::trigger` plus `radius`, `list_bg`, `indicator_bg` and
`indicator_shadow`, all of which landed. Those are paint-level. The reference toolbar
also strips both paddings — `TabsTrigger` carries `size-8 p-0` and `TabsList` carries
`p-0` — and 0.10.0 has no knob for either: `TabsSize::metrics()` is a closed table
(`Sm = (28, 12, 12)`, `Md = (32, 16, 14)`) applied as `.h(tab_h).px(tab_padding_x)`
unconditionally, and the Primary list inset is a literal `p(px(4.))`. An icon-only
group would therefore render 104x32 against the reference's 64x32 and push the capture
bar from 44px to 52px, which `overlay_chrome_matches_electron` correctly refuses.

A padding-override ask is filed upstream at low priority. Nothing is degraded in the
meantime: the hand-rolled control matches the reference exactly. Adopting the library
version later buys code reuse, width and height interpolation, and `reduce_motion`
snapping, which is the part worth having.

Do not pass `list_bg` on this surface when it is adopted. The hover wash resolves as
`tray.alpha(1.0 - tabs_hover_opacity)`, so the reference tray value becomes a
30%-opacity fill, while the reference has no hover fill on these triggers at all, only
a text-colour change.

Separately, item 57 in `GPUI-PARITY-IMPLEMENTATION-PLAN.md` ("All-in-one tab indicator
static", accepted because GPUI had no sliding thumb) is now stale twice over: our own
indicator already slides, and HeroGPUI's animates over 250ms with `Curve::OutFluid`,
re-targeting from the live rect when interrupted. That item can close.

## Accepted platform limits (do not build)

Each of these was confirmed against the pinned toolkit (`gpui = gpui-pre =0.3.5`,
`herogpui =0.9.0`) and has no sound fix inside it. They are listed so nobody re-files
them.

- No blend mode on `gpui::Window::paint_path` — it takes only `impl Into<Background>` (`gpui-pre-0.3.5/src/window.rs:4457`), so the highlighter's multiply cannot be done in the live canvas; item 120 routes around it by rasterizing the committed stroke as an image patch.
- No custom-bitmap cursors — gpui exposes a fixed `CursorStyle` enum, so Electron's tinted highlight brush, per-style redact glyph and amber scissors cursor can only be approximated with stock variants (items 129 and 167).
- No `CursorStyle::Move` and no `CursorStyle::Wait` in the enum (`gpui-pre-0.3.5/src/platform.rs:2434-2510`); `move` becomes the OpenHand/ClosedHand pair (item 209) and the busy cursor is dropped in favour of the in-app spinner (item 250).
- No tabular-numerals font feature in gpui, so Electron's `tabular-nums` can only be approximated by switching to the mono family — which changes the typeface, so it is a deliberate deviation rather than parity (items 177 and 214).
- No rich-element tooltip content in HeroGPUI 0.9.0 — `Tooltip::new` takes `impl Into<SharedString>` and its children are the trigger (`herogpui-components-0.9.0/src/tooltip.rs:264,281`), so the naming-token table cannot live in a tooltip (item 229).
- No dashed border primitive in gpui; dashes must be painted with a `gpui::canvas` stroke (item 198).
- No live backdrop filter, which is why the editor's zoom and preview blur is a 12px baked stand-in for `backdrop-blur-md`. This is already recorded in `GPUI-UI-PARITY-AUDIT.md` and stays.
- No reduced-transparency query in gpui — item 248 needs a platform read on each OS rather than a toolkit API.
- `window.element_bounds(...)` does not exist; element measurement must go through `gpui::canvas`'s prepaint bounds (item 130).
- `Engine::render_frame_scaled` clamps scale to at most 1.0 and derives its size by rounding (`video/composition/mod.rs:175-178`), so it cannot serve item 116's exact-size or upscale cases; that item adds an explicit target-size entry point instead.
- fontdue has no synthetic emboldening, so a bolder weight is only reachable by loading a different face file (items 115 and 201).
- No lazy panel loading is needed in GPUI: Electron's `React.lazy` Suspense fallback and its rail preload hook exist only because of code splitting, and `windows/video_editor/panels.rs:18-49` dispatches synchronously to compiled-in panels. Nothing to build.
- Reduced-resolution preview while playing or scrubbing (`windows/video_editor/mod.rs:13-23,564-566,2790-2799`) is a deliberate trade-off for a software rasterizer. Idle frames are full size and the exported file is unaffected, so it does not breach preview-equals-export. Keep as a known difference.
- Onboarding permission polling starting only on the Permissions step (`windows/onboarding.rs:150-182`) has no observable consequence: `onboarding.rs:760-761` recomputes the gate from the OS at render time, and off that step nothing renders permission state. Not a delta.

Added during implementation (2026-09-17), each with measured or source-level evidence:

- Item 200, inline secondary select rows: not reachable in HeroGPUI 0.9.0. `MenuItem` has only `SectionLabel`, `Separator` and `Item` (`dropdown.rs:16-32`); the only injection point is `item_content`, which renders inside a row that unconditionally calls `dismiss` on click (`dropdown.rs:1391-1435`), and `Select`'s trigger never stops propagation (`select.rs:1190-1192`), so an inline select closes the whole menu. The two ways to skip that handler are `is_item_disabled`, which dims the hosted control permanently (`:1155-1158`), and `has_submenu`, which is the flyout the item wants to remove. A hand-rolled nested popup is possible but amounts to a new nested-popup subsystem, which is disproportionate to the delta. The submenu approximation stays.
- Item 234, settings control height: the premise was wrong. Measured on the reference screenshots, the single-line select is 36pt in both shells, because Electron sets `--field-border-width: 0px` (`base.css:24`) and HeroGPUI's `FIELD_HEIGHT` 36 already matches. Setting `SelectStyle::height(40)` would have broken existing parity. No change made. A real residual remains: a two-line select is 56pt in Electron (`py-2` on the trigger) versus 40pt in GPUI, which needs an upstream `SelectStyle::padding_y`. The call-site `sx` route first suggested for this does NOT work and must not be used: `Select::sx` is applied to the component root (`herogpui-components-0.9.0/src/select.rs:2369`), not to the trigger, so `py` there pads outside the field box and adds drift instead of letting the trigger grow. The trigger itself is already `min_h` based (`select.rs:1071-1072`), so an upstream `padding_y` on that div is the whole fix. Filed upstream in the HeroGPUI project.
- Item 236, select label wrapping: not padding, and not fixable in the app. Both trigger boxes measure identically (outer box and an 11.5pt text inset). The difference is line breaking: `gpui-pre-0.3.5/src/text_system/line_wrapper.rs:478` classifies `-` as a word character, while Chromium breaks after hyphens per UAX #14 LB21, so `"Right (side-by-side)"` wraps differently. Needs an upstream gpui change.
- Item 243, separator inset: the row pitch half is done (measured 24pt, not the 28px the item claimed). The inset half is not reachable — `MenuStyle` exposes no separator fields and HeroGPUI hardcodes `div().w_full().my(px(4.)).h(border_width)` (`herogpui-components-0.9.0/src/dropdown.rs:1033-1039`). Needs an upstream `MenuStyle::separator_inset`.
- Item 182, GIF palette pass: `palettegen` emits no output frames until it has read the whole input, so FFmpeg reports no `out_time` during that pass. Reported progress is real rather than fabricated, and the pass is short next to the frame pass.

---

## Execution order

### Phase 1 — P0, preview-equals-export, and licensing

One PR per bullet unless noted. Everything here either breaks a rule in `CLAUDE.md`
or leaves an input dead.

- [ ] 139 — version parity and the AGPL source link. Land first; it is small, it is a compliance defect, and it must not ride behind a large refactor.
- [ ] 85 + 87 + 88 — the video drawing overlay, the panel writing to the selected annotation, and the text-annotation editor, in one PR. 87 and 88 are unreachable without 85.
- [ ] 86 — colour picker hex field.
- [ ] 119 + 120 + 121 + 122 + 201 — the screenshot editor's five preview-versus-export splits (pen, highlighter, number badge, text font resolution, font weight), in one PR, since they all converge on `editor/canvas.rs` and `editor/text_render.rs`. Delete the two false doc comments in the same change.
- [ ] 110 + 111 + 112 + 113 + 114 — the video editor's audio and camera-visibility divergences, in one PR; 111 and 112 must land together or the built-in tracks are mixed twice.
- [ ] 115 + 116 + 117 + 118 + 189 + 190 + 191 — the composition and export divergences that change the written file.
- [ ] 123 — wallpaper preset artwork. Needs a decision on baked raster assets before any code; the packaging and notices work is part of the same PR.

### Phase 2 — remaining P1, one PR per surface

- [ ] Video editor timeline: 96, 97, 98, 99, 100, 101, 102, 103, 104, 105.
- [ ] Video editor panels and chrome: 89, 90, 91, 92, 93, 94, 95, 106, 107, 108, 109.
- [ ] Screenshot editor: 124, 125, 126, 127, 128, 129.
- [ ] Capture and recording: 130.
- [ ] Settings: 131, 132.
- [ ] Small windows, tray and preview: 133, 134, 135, 136, 137.
- [ ] Cross-cutting: 138 — Geist registration, including the TTF/OTF assets, `THIRD_PARTY_NOTICES.md` and `extraResources`. This changes every screenshot in the shell, so land it before Phase 4's runtime re-run.

### Phase 3 — P2 polish, grouped by the file they touch

- [ ] Theme and design system: 244, 245, 246, 247, 249, 251, 252, 253, 254.
- [ ] Platform gating and cursors: 248, 250, plus the cursor half of 167.
- [ ] Video editor panels: 140–158.
- [ ] Video editor timeline: 159–173.
- [ ] Video editor chrome and export: 174–188.
- [ ] Screenshot editor: 192–203.
- [ ] Capture and recording: 204–225.
- [ ] Settings: 226–237.
- [ ] Small windows, tray and preview: 238–243.

Items 200, 219, 224, 229, 232, 239 and 241 need a decision rather than code. Resolve
each one by recording the choice in this document; do not leave them silently open.

### Phase 4 — re-run the live screenshot pass with a way to drive GPUI

The runtime pass covered only idle first-paint states because no synthetic input
reaches GPUI windows. Before re-running it:

- [ ] Add a debug-only intent or IPC channel to the GPUI shell that opens a named surface in a named state — for example `--intent debug-state settings:search=sound`, `editor:tool=highlight`, `editor:crop`, `video-editor:sidebar=export`, `history:tab=videos&layout=list`, `capture-preview:busy=uploading`. Route it through the existing `--intent` dispatcher in `src/main/app-gpui/src/intents.rs` and compile it out of release builds so it cannot ship.
- [ ] Give the Electron shell the same state entry points, or drive them over CDP as this pass did, so both sides can be put into the same state without hand-clicking.
- [ ] Re-run the pass against a packaged-equivalent Electron profile, so the About update row and the updater channel behave as they do for a user (item 132's evidence came from an unpackaged build).
- [ ] Re-check the items the runtime pass could not settle: the mid-drag area overlay, the all-in-one toolbar, the pre-recording bar with a real window target and its target chip, the capture preview and pin windows, every hover, press and focus transition, and the settings search results page.

---

## Tests to add

These mirror the existing parity guards — `tests/unit/daemon-module-parity.test.ts`,
which scrapes both daemons' dispatch tables, and the literal parity tests in
`src/main/app-gpui/src/theme/bridge.rs` — so drift fails a build rather than a review.

- Version parity: a test that reads `src/main/app-gpui/Cargo.toml` and `package.json` and fails when the versions differ (item 139). If the `build.rs` route is taken instead, assert that `product::source_url_for_version()` ends with the `package.json` version.
- Font registration: assert that the GPUI shell registers the Geist and Geist Mono faces at startup and that every window root resolves to the `Geist` family, so a missing `add_fonts` call cannot regress silently (item 138).
- Wallpaper preset artwork: hash each of the 14 rendered preset tiles and assert the hashes against fixtures generated from the Electron SVGs, so a flattened or swapped preset fails (item 123).
- Preview-versus-export rasters: for pen, highlighter, number badge and text annotations, render the same annotation through `editor/canvas.rs`'s path and through `render/annotations.rs` at the same scale and assert the two rasters match within one 8-bit step — the same shape as the existing `tone_map::tests` comparison in the Windows daemon (items 119–122).
- Composition parity: assert that `export::compose_pixmap` at a target size produces the same raster as the preview engine at that size, covering both downscale and upscale (item 116), and that an empty `camera_segments` array hides the bubble on both paths (item 110).
- Audio model parity: assert that `export::build_audio` and `preview_audio::build_stems` produce identical stem sets for the same project, including built-in system and mic tracks, the enabled flag and the keyboard stem (items 111–113).
- Shortcut id parity: extend the existing id comparison to fail when the Electron registry and `windows/settings/shortcut_items.rs` differ by any id (item 226).
- Slider step parity: assert that each video-editor slider's step matches its Electron counterpart's, driven from one table so a new slider cannot be added without a step (item 92).
- Overlay chrome parity: update the `overlay_chrome_matches_electron` assertion in `ui/chrome.rs` to the reference constants rather than the current formula (item 206), and add the recording-bar width case once measurement lands (item 130).
