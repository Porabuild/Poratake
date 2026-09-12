# GPUI vs Electron parity (post–HeroGPUI)

Compared after the GPUI shell moved onto HeroGPUI (`herogpui` pin
`63cd1cfd791813a4d72d7c71f88cd92568a7363b`, HeroUI v3.2.4). Electron:
`src/renderer/` + `src/renderer/styles/base.css` + `@heroui/react` ^3.2.4.
GPUI: `src/main/app-gpui/`, tokens in `ui/chrome.rs`, theme in
`theme/{vars,bridge}.rs`.

The previous audit's headline (4× radii, hand-rolled `ui/button.rs`) is no
longer true. Radii are derived from `--radius: 0.125rem` (`ROOT_RADIUS = 2`).
Shared controls go through HeroGPUI recipes on `ThemeBuilder::components`.

Verified locally: `cargo fmt --all`, `cargo test --locked` (577 passed),
`cargo clippy --locked --all-targets -- -D warnings`.

Windows (`src/renderer/windows/`) compose reusable pieces in
`src/renderer/components/` (`ToolbarButton`, `ToolbarSurface`, history toolbar,
`ui/button`). GPUI mirrors that: `windows/` and `capture/` views compose
`ui/toolbar.rs`, `ui/icon_button.rs`, and `ui/preview.rs` instead of painting
overlay / chip / preview metrics at each call site.

---

## Theme and tokens

`ThemeVars::resolve()` ports `app-theme.ts`. `to_herogpui` publishes the tokens
HeroGPUI components read, plus layout overrides the renderer already made:

| Concern                           | Electron                  | GPUI                                                    | Status                                              |
| --------------------------------- | ------------------------- | ------------------------------------------------------- | --------------------------------------------------- |
| `--radius` / `--field-radius`     | 2px / 6px                 | `.radius(2)` / `.field_radius(6)` and `chrome.rs` scale | Match                                               |
| Palette                           | `applyVariant()`          | `ThemeVars` 1:1, tests in `vars.rs`                     | Match                                               |
| `--muted`                         | fg 76% → bg               | `vars.muted` → HeroGPUI `.muted()`                      | Match                                               |
| `--muted-background` / `bg-muted` | remapped in `base.css`    | `vars.muted_background`; **not** a HeroGPUI theme key   | App chrome paints it; stock HeroGPUI widgets do not |
| Sidebar / row / hairline          | CSS vars                  | `ThemeVars` only                                        | Correct for chrome; not on ThemeProvider            |
| Accent / danger hover             | app mixes                 | HeroUI stock hover math                                 | Intentional HeroGPUI default                        |
| `--cursor-interactive`            | `default`                 | `.cursor_interactive(Arrow)`                            | Match (stock HeroGPUI is a hand)                    |
| Tooltip open                      | 150ms (`TooltipProvider`) | `.tooltip_delay_ms(150)`                                | Match (stock HeroGPUI is 1500)                      |
| Tooltip close                     | 500ms                     | 500ms                                                   | Match                                               |

---

## Shared HeroGPUI controls

Recipes live in `theme/bridge.rs::component_themes()`.

| Control                                         | Bridge                                       | Electron / HeroUI                   | Status                                    |
| ----------------------------------------------- | -------------------------------------------- | ----------------------------------- | ----------------------------------------- |
| Button `compact`                                | 28 / px 10 / text 12 / line 16               | `.button--xs`                       | Match                                     |
| Button `compact-icon`                           | size 28, p 0                                 | `.button--xs.button--icon-only`     | Match                                     |
| Button `muted` / `danger`                       | muted fg / `Outline`                         | ghost + muted / outline trash       | Match                                     |
| Button `overlay` / `chip` / `preview`           | radius 6; chips 24; preview 24               | overlay `rounded-3xl`; `h-6` chips  | Match                                     |
| Slider / Switch                                 | pill radius 9999; `SliderSize::Sm` is 6×12   | `base.css` 9999; `.slider--sm`      | Match                                     |
| Select default                                  | `FieldVariant::Secondary`                    | `variant="secondary"`               | Match                                     |
| Select `compact`                                | h 28, padx 8, text 12, row 28/x8/y2, panel 4 | `.select__trigger--sm` / popover sm | Match                                     |
| Menu default / `compact` / `accent` / `neutral` | panel_gap 0; compact 28                      | tray + editor menus                 | Match                                     |
| TextField `search`                              | h 32, padx 10, text **14**                   | settings search `text-sm` (**14**)  | Match                                     |
| TextField `compact`                             | h 28, text 12                                | dense fields                        | Match                                     |
| Tabs                                            | `TabsVariant::Secondary` at call sites       | `variant="secondary"`               | Match                                     |
| Tooltip                                         | theme delay 150                              | 150                                 | Match                                     |
| Separator                                       | stock + `chrome_tick` h 18 / `--border`      | title `h-[18px] w-px bg-border`     | Visual match; tick is still instance `sx` |
| ProgressBar                                     | stock, sites set h 8                         | `h-2` (8px)                         | Match                                     |

`chrome_tick` stays instance-`sx` (HeroGPUI has no `SeparatorStyle`). Overlay and
preview colours live on `ui/toolbar.rs` and `ui/preview.rs` (white-on-desktop,
`bg-background/80`), not on each window.

Electron `.slider--sm` (`base.css`): track 6px, 12px foreground knob.
`SliderSize::Sm` is that geometry. Settings sliders stay `Md`, matching Electron
`Slider` default `size="md"`.

---

## Window chrome (macOS traffic lights)

| Surface                | Electron                 | GPUI                                              | Status |
| ---------------------- | ------------------------ | ------------------------------------------------- | ------ |
| Settings sidebar title | `pl-20` (80)             | `MACOS_TITLE_LEADING_INSET` 80                    | Match  |
| Settings content drag  | `h-10`, no leading inset | `content_drag_strip(..., reserve_leading: false)` | Match  |
| Video title            | `pl-20` + `px-2` → 88    | 80 + `TITLE_BAR_PADDING_X` 8                      | Match  |
| Screenshot editor      | `w-[120px]` spacer       | `TRAFFIC_LIGHT_INSET` 120                         | Match  |
| Windows captions       | 46×3                     | `WINDOW_CONTROL_WIDTH` 46                         | Match  |

---

## Surfaces

### Settings — mostly match

Search uses `TextField::recipe("search")` inside the same rounded shell as
Electron. Item rows use Switch / Select compact / Slider / Tooltip / Button
recipes. About keeps AGPL notices, source, license, and third-party links.

Search text is 14px. The settings window is `WindowBackgroundAppearance::Blurred`
(Windows acrylic) with the same native-material tint mix as
`html[data-native-material='on']`. `scrollCapture` shortcut is GPUI-only under
extras.

### Screenshot editor — match, with approximations

120px leading inset, compact-icon tools, 18px ticks, wallpaper sheet on
HeroGPUI Select/Switch/Slider/Separator. Editor colour popover composes
HeroGPUI `ColorSwatchPicker` / `ColorArea` / `ColorSlider`. Capture eyedropper
stays the overlay loupe (same as Electron). Zoom/preview blur is a 12px baked
stand-in for `backdrop-blur-md` — GPUI has no live backdrop-filter.

### Video editor — match

88px leading inset, export `ProgressBar`, sidebar icon rail 40 / tab 32,
`panel_kit` Tabs secondary + Slider sm + Select compact. Persist path matches
Electron `use-editor-state-persistence`.

Focused text fields (`rename_field`, `prompt_field`, data editor) skip
`on_key`, so Space / sidebar letters do not steal typing.

### History — match

400×500 popover, radii, grid/list gaps pinned in `chrome.rs`. Filter / sort /
layout chips use `chip` / `chip-icon` recipes.

### Recording control / all-in-one — match

Bar widths 236 / 400 / +140 target label, height 52 / inner 44. Recording and
all-in-one icon buttons use `recipe("overlay")`. System-audio tooltip matches
Electron (`Turn system sounds on/off`). Mid-recording mic / system / camera
stay on the session and daemon setters — not written to `config.recording` on
toggle (same as Electron `RecordingSession`). Device picks write config.

All-in-one mode pills stay custom (not HeroGPUI Button) and live on
`ui/toolbar::mode_tab`. Overlay icon buttons, hairlines, and the bar surface
are the same helpers recording control uses — matching Electron
`ToolbarButton` / `ToolbarSurface`.

### Onboarding, preview, pin, toast, tray

Onboarding 500×650. Preview 200×140 with baked blur and `preview` /
`preview-pill` recipes. Pin is borderless image drag. Toast is the OS
notification API on both shells (`showNotification`). Tray menu is HeroGPUI
`compact` + `accent`, width 304.

---

## State

| Concern                                 | Status                                                       |
| --------------------------------------- | ------------------------------------------------------------ |
| Config schema / store                   | Shared `config/{schema,store}.rs` — match                    |
| Theme presets / light / dark / system   | Literal parity tests — match                                 |
| System appearance live updates          | Windows registry watcher; macOS keepalive appearance — match |
| Window geometry / video persist         | Match                                                        |
| Recording session vs `config.recording` | Match (toggles session-only; device pick persists)           |
| Daemon contract                         | Unchanged `DAEMON_METHODS`                                   |
| Linux session matrix                    | GPUI-only (X11 / Wayland / headless)                         |
| Capture sound                           | Capability-gated, macOS-only, both shells                    |

---

## Remaining platform limits (not Electron product drift)

1. GPUI has no live `backdrop-filter`. Preview/zoom bake a 12px blur to stand in
   for `backdrop-blur-md`. Settings uses real window blur + native-material tint.
2. Capture eyedropper stays the overlay loupe — HeroGPUI ColorPicker has no
   screen sampler, and Electron's loupe is also custom.
3. `chrome_tick` is instance-`sx` because HeroGPUI has no `SeparatorStyle`.
4. `gpui = { package = "gpui-pre", version = "=0.3.3" }` remains: GPUI derive
   macros emit `::gpui::…`. HeroGPUI documents this; a crate named `gpui` is
   required until those macros use `$crate`.

No evidence of the old 4× radius or deleted `ui/{button,select,slider,switch,tabs,text_field,tooltip}.rs` issues — those are gone and test-guarded in `bridge.rs`, `chrome.rs`, and `ui/lints.rs`.
