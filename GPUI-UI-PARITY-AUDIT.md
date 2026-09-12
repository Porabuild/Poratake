# GPUI vs Electron parity (post–HeroGPUI)

Compared after the GPUI shell moved onto HeroGPUI (`herogpui` pin
`63cd1cfd791813a4d72d7c71f88cd92568a7363b`, HeroUI v3.2.4). Electron:
`src/renderer/` + `src/renderer/styles/base.css` + `@heroui/react` ^3.2.4.
GPUI: `src/main/app-gpui/`, tokens in `ui/chrome.rs`, theme in
`theme/{vars,bridge}.rs`.

The previous audit's headline (4× radii, hand-rolled `ui/button.rs`) is no
longer true. Radii are derived from `--radius: 0.125rem` (`ROOT_RADIUS = 2`).
Shared controls go through HeroGPUI recipes on `ThemeBuilder::components`.

Verified locally: `cargo fmt --all`, `cargo test --locked` (574 passed),
`cargo clippy --locked --all-targets -- -D warnings`.

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

| Control                                         | Bridge                                       | Electron / HeroUI                   | Status                                                         |
| ----------------------------------------------- | -------------------------------------------- | ----------------------------------- | -------------------------------------------------------------- |
| Button `compact`                                | 28 / px 10 / text 12 / line 16               | `.button--xs`                       | Match                                                          |
| Button `compact-icon`                           | size 28, p 0                                 | `.button--xs.button--icon-only`     | Match                                                          |
| Button `muted` / `danger`                       | foreground tint                              | ghost + muted / danger fg           | Partial — not full HeroUI fill variants                        |
| Button `overlay`                                | radius 6                                     | overlay `rounded-3xl`               | Match at recording bar; all-in-one still paints radius in `sx` |
| Slider / Switch                                 | pill radius 9999                             | `base.css` 9999                     | Pill match; compact slider thumb/track still drift (see below) |
| Select default                                  | `FieldVariant::Secondary`                    | `variant="secondary"`               | Match                                                          |
| Select `compact`                                | h 28, padx 8, text 12, row 28/x8/y2, panel 4 | `.select__trigger--sm` / popover sm | Match                                                          |
| Menu default / `compact` / `accent` / `neutral` | panel_gap 0; compact 28                      | tray + editor menus                 | Match                                                          |
| TextField `search`                              | h 32, padx 10, text **13**                   | settings search `text-sm` (**14**)  | Height match; text 13 vs 14                                    |
| TextField `compact`                             | h 28, text 12                                | dense fields                        | Match                                                          |
| Tabs                                            | `TabsVariant::Secondary` at call sites       | `variant="secondary"`               | Match                                                          |
| Tooltip                                         | theme delay 150                              | 150                                 | Match                                                          |
| Separator                                       | stock + `chrome_tick` h 18 / `--border`      | title `h-[18px] w-px bg-border`     | Visual match; tick is still instance `sx`                      |
| ProgressBar                                     | stock, sites set h 8                         | `h-2` (8px)                         | Match                                                          |

Still painted at call sites (recipe candidates): history filter chips (h 24),
capture-preview chips, all-in-one overlay buttons, `chrome_tick`.

Electron `.slider--sm` (`base.css`): track 6px, transparent thumb wrapper,
12px round **foreground** knob. GPUI uses `SliderSize::Sm` + pill radius only.

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

Drifts: search text 13 vs 14; sidebar is a flat tint (no Electron
`backdrop-filter: blur(18px)` / native-material glass); `scrollCapture`
shortcut is GPUI-only under extras.

### Screenshot editor — match, with approximations

120px leading inset, compact-icon tools, 18px ticks, wallpaper sheet on
HeroGPUI Select/Switch/Slider/Separator. Zoom backdrop is baked, not live
`blur-md`. Color picker stays app-specific (eyedropper over the capture).

### Video editor — match

88px leading inset, export `ProgressBar`, sidebar icon rail 40 / tab 32,
`panel_kit` Tabs secondary + Slider sm + Select compact. Persist path matches
Electron `use-editor-state-persistence`.

Known (not applied): typing in HeroGPUI fields can still fire editor shortcuts;
Electron focuses the input and does not.

### History — match, chip styling debt

400×500 popover, radii, grid/list gaps pinned in `chrome.rs`. Filter / sort /
layout chips still use hand `sx` (h 24) instead of a `chip` recipe.

### Recording control / all-in-one — match

Bar widths 236 / 400 / +140 target label, height 52 / inner 44. Recording
buttons use `recipe("overlay")`. Mid-recording mic / system / camera stay on
the session and daemon setters — not written to `config.recording` on toggle
(same as Electron `RecordingSession`). Device picks write config.

System-audio toggle has no tooltip (mic/camera dropdowns do). All-in-one mode
pills are custom; its icon buttons paint overlay radius instead of the recipe.

### Onboarding, preview, pin, toast, tray

Onboarding 500×650. Preview 200×140 with baked blur and custom chips. Pin is
borderless image drag. Toast is the OS notification API, not the in-app
Electron toast. Tray menu is HeroGPUI `compact` + `accent`, width 304.

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

## Remaining gaps (current only)

1. Settings search text **13px** vs Electron **14px**.
2. Compact slider thumb/track vs `.slider--sm` (6px track, 12px foreground knob).
3. Settings sidebar glass / vibrancy.
4. History + capture-preview chips still instance-`sx`.
5. All-in-one overlay buttons paint radius instead of `recipe("overlay")`.
6. `muted` / `danger` button recipes are foreground tints, not full variants.
7. `chrome_tick` separator is instance-`sx`.
8. Baked blur where Electron uses live `backdrop-filter`.
9. In-app toast UI (GPUI uses OS notifications).
10. Color picker is still the capture-specific control, not HeroGPUI ColorPicker.
11. Video-editor shortcuts can steal keystrokes from focused text fields.
12. Recording system-audio toggle has no tooltip.
13. Crate-name shim: `gpui = { package = "gpui-pre", version = "=0.3.3" }` remains
    because GPUI's `#[test]` expansion still emits `::gpui::…`.

No evidence of the old 4× radius or deleted `ui/{button,select,slider,switch,tabs,text_field,tooltip}.rs` issues — those are gone and test-guarded in `bridge.rs`, `chrome.rs`, and `ui/lints.rs`.
