# GPUI vs. Electron UI parity audit

Scope: every user-visible surface of `src/main/app-gpui/` compared against the
Electron renderer (`src/renderer/`) plus its actual computed styling — HeroUI v3
(`node_modules/@heroui/styles/dist/`) and the app's Tailwind v4 token overrides
in `src/renderer/styles/base.css`.

Method: the renderer's pixel values are not in the TSX. They come from three
layers that must be resolved together:

1. `@heroui/styles/dist/components/*.css` — the component metrics (`h-9`,
   `min-h-9`, `rounded-3xl`, `border-x-[0.75rem]`, …).
2. `@heroui/styles/dist/themes/shared/theme.css` — an `@theme inline` block that
   **rebinds the whole Tailwind radius scale** onto `--radius`.
3. `src/renderer/styles/base.css` — the app's unlayered `:root`, which wins over
   HeroUI's `@layer theme` and sets `--radius: 0.125rem`, plus an `@theme inline`
   that rebinds `--color-muted`.

Everything below was resolved through those three layers, not from the class
names alone.

---

## 1. Headline finding: every Tailwind-derived corner radius in GPUI is 4x too large

`@heroui/styles` replaces Tailwind's fixed radius scale with multiples of
`--radius`:

```css
/* themes/shared/theme.css */
--radius-lg: calc(var(--radius) * 1);
--radius-xl: calc(var(--radius) * 1.5);
--radius-2xl: calc(var(--radius) * 2);
--radius-3xl: calc(var(--radius) * 3);
--radius-4xl: calc(var(--radius) * 4);
```

HeroUI's own default is `--radius: 0.5rem` (in `@layer theme`), but
`src/renderer/styles/base.css` sets `--radius: 0.125rem` in an **unlayered**
`:root`, and unlayered declarations beat every `@layer`. So in this app the
whole scale is exactly one quarter of stock Tailwind:

| utility       | stock Tailwind | this app  | GPUI constant |
| ------------- | -------------- | --------- | ------------- |
| `rounded-sm`  | 4px            | **1px**   | –             |
| `rounded-md`  | 6px            | **1.5px** | 6.0           |
| `rounded-lg`  | 8px            | **2px**   | 8.0           |
| `rounded-xl`  | 12px           | **3px**   | 12.0          |
| `rounded-2xl` | 16px           | **4px**   | 16.0          |
| `rounded-3xl` | 24px           | **6px**   | 24.0          |
| `rounded-4xl` | 32px           | **8px**   | 32.0          |

`src/renderer/components/editor/zoom/index.tsx` states this outright:

```tsx
// `rounded-3xl` is the button radius here: HeroUI maps the radius scale onto --radius.
```

`src/main/app-gpui/src/ui/chrome.rs` used the **stock** values throughout, so
the GPUI shell renders pill-shaped buttons and heavily rounded panels where the
Electron shell renders near-square 1.5–8px corners. The one radius the port got
right is `FIELD_RADIUS = 6.0`, because it was derived from `--field-radius:
calc(var(--radius) * 3)` rather than from a `rounded-*` class.

Affected constants — the fix is mechanical (divide by 4):

| `chrome.rs`                              | now | should be | source class                             |
| ---------------------------------------- | --- | --------- | ---------------------------------------- |
| `BUTTON_RADIUS`                          | 24  | **6**     | `.button { rounded-3xl }`                |
| `TOOL_OPTION_RADIUS`                     | 24  | **6**     | `Select.Trigger className="rounded-3xl"` |
| `OVERLAY_BUTTON_RADIUS`                  | 24  | **6**     | `toolbar-button.tsx rounded-3xl`         |
| `SETTINGS_NAV_RADIUS`                    | 24  | **6**     | `settings-sidebar.tsx rounded-3xl`       |
| `OVERLAY_SURFACE_RADIUS`                 | 32  | **8**     | `toolbar-surface.tsx rounded-4xl`        |
| `TOOLTIP_RADIUS`                         | 12  | **3**     | `.tooltip { min(32px, --radius-xl) }`    |
| `HISTORY_RADIUS`                         | 12  | **3**     | `history-window.tsx rounded-xl`          |
| `PREVIEW_RADIUS`                         | 8   | **2**     | `capture-preview-window.tsx rounded-lg`  |
| `WALLPAPER_TILE_RADIUS`                  | 8   | **2**     | wallpaper tiles `rounded-lg`             |
| `OVERLAY_LABEL_RADIUS`                   | 6   | **1.5**   | `selection-frame.tsx rounded-md`         |
| `ONBOARDING_CARD_RADIUS`                 | 6   | **1.5**   | onboarding cards `rounded-md`            |
| `VIDEO_ASPECT_RADIUS`                    | 6   | **1.5**   | aspect tiles `rounded-md`                |
| `ONBOARDING_ICON_RADIUS`                 | 16  | **4**     | onboarding icon `rounded-2xl`            |
| history grid card / list row (`item.rs`) | 8   | **2**     | `rounded-lg`                             |
| history list thumb (`item.rs`)           | 6   | **1.5**   | `rounded-md`                             |
| colour-picker loupe card (`overlay`)     | 16  | **4**     | `rounded-2xl`                            |

Correct as-is: `FIELD_RADIUS` 6, `SWITCH_RADIUS` 9999 (`base.css` pill
override), `TABS_RADIUS`/`TAB_RADIUS` 0 (`rounded-none` in the secondary
variant), menu popover 6 (`min(32px, --radius-3xl)`), list-box item 4
(`rounded-2xl`), history feature badge 4 (bare `rounded`, unaffected by the
scale).

## 2. Second systemic finding: `bg-muted` is the wrong token in GPUI

`base.css` remaps the token in its own `@theme inline`, which comes after the
HeroUI import and therefore wins:

```css
--color-muted: var(--muted-background); /* = --surface-secondary */
--color-muted-foreground: var(--muted-foreground);
```

So `bg-muted` is a **dark surface**, while `--muted` (used by
`text-muted-foreground`, `border-muted-foreground/35`) is the light grey text
colour. GPUI's `ThemeVars` carries both correctly, but seven call sites bind
`bg-muted` to `theme.muted` instead of `theme.muted_background`:

- `capture/all_in_one_toolbar.rs:76` — `bg(theme.muted.opacity(0.95))`
- `windows/recording_control.rs:492` — same
- `editor/wallpaper_sheet.rs:493, 495, 749, 774, 906`

Effect: the all-in-one toolbar and the recording control bar render as a light
grey slab in dark mode instead of the dark translucent surface Electron shows.
`windows/history/item.rs` uses `theme.muted_background` for the same class, so
the codebase is internally inconsistent.

## 3. Buttons (`ui/button.rs` vs `ui/button.tsx` + HeroUI `button.css`)

Correct: heights 36/32/40/28 (`md:h-9`, `md:h-8`, `md:h-10`, `.button--xs`),
padding 16/12/10, text 14/16/12, `gap-2`, `font-medium`, icon-only squares,
variant colour mapping (`primary`→accent, `secondary`/`tertiary`→default with
the `base.css` `--button-fg: var(--foreground)` override, `ghost`/`outline`→
`default-foreground`, `outline` 1px border + hover `default/60%`).

Deltas:

1. **Radius** 24 → should be 6 (§1).
2. **Press scale is missing.** HeroUI scales the button on press —
   `md` 0.97, `sm` 0.98, `lg` 0.96 — over `transform 250ms var(--ease-smooth)`.
   GPUI only swaps the background.
3. **No background transition.** HeroUI animates
   `background-color 100ms var(--ease-out)`; GPUI snaps.
4. **`IconXs` icons are 16px, should be 14px.** `ButtonSize::icon_size()` only
   special-cases `Xs`, so `IconXs` falls through to `TOOL_BUTTON_ICON` (16).
   `.button--xs svg:not([class*='size-'])` is 0.875rem = 14px, and being
   unlayered it beats the `[&_svg…]:size-4` utility. Visible on the editor zoom
   bar (Electron also passes an explicit `size-3.5`), the history sort/layout
   buttons (`h-3.5` = 14) and the history item overlay actions (`h-3` = 12).
5. **Disabled rendering differs.** HeroUI applies
   `opacity: var(--disabled-opacity)` = 0.5 to the whole element plus
   `cursor: not-allowed` and `pointer-events: none`. GPUI multiplies the
   background and text alpha separately, which composites differently over a
   non-opaque parent. Overlay buttons use `disabled:opacity-35`, not 0.5.
6. **No focus ring.** HeroUI `status-focused` = `ring-2 ring-focus` (2px,
   `--focus` = `--accent`, offset 2px). GPUI draws nothing on keyboard focus.
7. **`link` variant** — Electron adds `underline-offset-4 hover:underline`; GPUI
   only recolours the text.

## 4. Fields, selects, sliders, switches

### Slider (`ui/slider.rs` vs HeroUI `slider.css` + `base.css .slider--sm`)

Metrics correct: track 20px / 6px (sm), knob 24×16 / 12×12 (sm), knob colour
(`accent-foreground` md, `--foreground` sm), pill radius, `bg-default` track,
`bg-accent` fill.

- **Knob travel is wrong.** HeroUI insets the track's content box with
  `border-x-[0.75rem]` (8px for `--sm`) so the knob stays inside the track at
  both ends. GPUI positions the knob at `left: fraction` with `ml: -knob_w/2`, so
  at the minimum the 24px knob hangs 12px past the track's left edge and at the
  maximum 12px past the right. Correct port: travel = `width - knob_w`, knob
  `left = fraction * (width - knob_w)`.
- **Fill and knob disagree** by up to 12px for the same reason (fill uses the
  full width, knob is centred).
- **Dragging outside the track stops updating** — `on_mouse_move` is bound to
  the track element; React Aria captures the pointer globally.
- **No keyboard support** (arrows/Home/End).
- **Disabled has no visual effect** (HeroUI: 0.5 opacity, label stays at 1.0).
- **Missing** the drag `scale(0.9)` on the knob and its 250ms transitions.

### Switch (`ui/switch.rs`)

- **`SWITCH_SM_THUMB.0` is 14.4, should be 16.5.** HeroUI's small thumb is
  `1.03125rem`; the port took the "~14.4px on desktop" code comment, which
  assumes a 14px root font. This app never changes the root font size, so `rem`
  = 16px. The `md`/`lg` thumbs took the other number in the same comments and
  are right (22×16, 27.5×20).
- **`checked_offset` subtracts the margin twice.** HeroUI's checked position is
  `ms-[calc(100% - 1.5rem)]` = `track − thumb − margin` (md: 40−22−2 = 16).
  GPUI computes `track − thumb − 2*margin` = 14, so the thumb sits 2px short and
  the trailing gap is 4px instead of 2px. Same 2px error at `sm` and `lg`.
- **No thumb travel animation.** HeroUI animates
  `margin 300ms var(--ease-out-fluid)`; GPUI snaps.
- **No thumb shadow** (`shadow-field` unchecked, an explicit three-layer
  box-shadow when checked).
- **Disabled state incomplete** — HeroUI additionally paints the thumb
  `bg-default-foreground/20` when off and drops it to `opacity: 0.4` when on.

Correct: tracks 40×20 / 32×16 / 48×24, 2px margin, pill radius, thumb colours
(`#fff` off, `accent-foreground` on), hover mix (`default` at 80% alpha).

### Select (`ui/select.rs`)

Correct: `min-h-9` = 36 / `h-7` = 28 small, radius 6 (`rounded-field`), px 12,
py 8 / 0, text 14 / 12, no border at rest (`--field-border-width: 0px`),
placeholder colour.

- **No hover state.** Electron passes `variant="secondary"`, which hovers to
  `--default-hover`.
- **Resting background token.** GPUI uses `field_background`; the secondary
  variant uses `--default`. Identical in dark, different in light
  (`mix(surface 97%, bg)` vs `mix(surface 86%, fg)`).
- **Open state adds a 1px `ring` border** that Electron does not have — and it
  shifts the layout by 1px because it is a real border, not a ring.
- **Chevron: 4px too far in and the wrong colour.** HeroUI absolutely positions
  the indicator at `end-2` (8px) at `size-4` (16px) and pads the content with
  `pe-7`; GPUI puts a 16px chevron in flow behind 12px of padding and tints it
  `muted_foreground` instead of `--field-placeholder`.
- **No `rotate-180` on the indicator while open** (HeroUI:
  `transition duration-150`).
- **Value truncates**; HeroUI's `.select__value` is `wrap-break-word`.
- **Disabled** dims only the text, not the whole control.

### Text field / text area

- Focus: GPUI draws a **1px border in `ring`** (accent @ 52%). HeroUI's
  `status-focused-field` is a **2px ring in `--focus`** (accent @ 100%) drawn
  outside the box, so it neither shifts layout nor dilutes the colour.
- No `bg-field-hover` hover state.
- Placeholder disappears on focus in GPUI; in the DOM it persists until the user
  types.
- `ui/text_area.rs` vs `ui/textarea.tsx` (`h-full min-h-60 resize-none font-mono
text-xs` over `rounded-field border-0 bg-field px-3 py-2`): GPUI adds a **1px
  border** the DOM explicitly removes (`border-0`), uses `bg(theme.default)`
  instead of `bg-field`, and pads `px 8` instead of `px 12`.

## 5. Overlays and the selection area

### Toolbar surface / all-in-one toolbar

`OVERLAY_*` geometry is right: `gap-0.5` = 2, `p-1` = 4, `border-2` = 2,
`size-8` = 32 buttons, so `overlay_bar_height()` = 44 matches. Hairline
`mx-0.5 h-5 w-px bg-border/70` matches.

- Surface radius 32 → 8 (§1); button radius 24 → 6.
- `bg(theme.muted)` → `theme.muted_background` (§2).
- `shadow_lg()` vs Tailwind `shadow-2xl` (`0 25px 50px -12px rgb(0 0 0/0.25)`).
- **`backdrop-blur-xl` is not reproduced.** Both the toolbar and the colour
  loupe are translucent blurred glass in Electron.
- **Overlay buttons are theme-coloured instead of white.** `toolbar-button.tsx`
  hard-codes `--button-fg: rgb(255 255 255 / 0.85)` and `hover:bg-white/15`;
  GPUI uses the ghost variant (`default_foreground`, hover `default`). In a
  light theme the GPUI overlay buttons come out dark-on-dark.
- Capture-target trigger: width 48 and chevron 12 match; GPUI omits `px-1.5`
  and `min-w-12`.
- Mode tabs: GPUI applies `hover:text-muted-foreground` unconditionally, so
  hovering the _selected_ tab darkens it (Electron keeps `text-foreground` and
  applies `opacity-70` to unselected tabs only). The indicator does not animate
  (HeroUI: `translate,width,height` over 250ms `--ease-out-fluid`).

### Selection frame, handles, scrim, crosshair

- Frame: 1px accent border matches, but the
  **`shadow-[0_0_0_1px_rgba(0,0,0,0.35)]` outer hairline is missing**, so the
  frame loses contrast over light content.
- Handles: 12 bars at 4×20 in the right positions, but the
  **`ring-1 ring-black/20`** around each bar is missing.
- **Crosshair guides are absent.** `crosshair-guides.tsx` draws two 1px
  `bg-primary/70` lines through the pointer whenever there is no selection and
  no drag in progress; GPUI only sets a crosshair mouse cursor.
- Dimension label: geometry, padding (8/4), size (12) and above/below placement
  all match; radius 6 → 1.5, and the label is **not monospaced** (`font-mono` in
  the DOM).
- Scrim `black/50` matches; prompt pill (`rounded-full bg-black/70 px-4 py-2
text-sm text-white shadow-lg`) matches, though GPUI has no macOS prompt
  offsets (`top-12` / `top-28` vs the Windows `top-8` / `top-24`).
- **Cursor feedback is missing** — the DOM overlay swaps between crosshair,
  move and the eight resize cursors via `useAreaSelection`; GPUI is always
  `cursor_crosshair()`.

### Window picking

Divergent in both directions. Electron punches the scrim _hole_ around the
hovered window (`scrimRect = hovered`) and draws **no** border and **no** label.
GPUI keeps the full dim, draws a 1px accent border and adds a window-name label
that the DOM overlay never shows.

### Colour picker loupe

15×7 = 105px grid, 20px cursor offset, 7px centre cell and the text sizes all
match. Card radius 16 → 4; inner clip 2px (`rounded-lg`); `backdrop-blur-xl` and
`shadow-2xl` unreproduced; `bg-muted/95` token as in §2.

## 6. Editor

### Title bar / toolbar

Correct: `h-10` bar on `bg-card` with `px-2` and `gap-1`; 28px tool buttons
(`size-7!`) with 16px icons; 32px action buttons (`icon-sm`); 18px separators
with 4px insets; mac 120px traffic-light inset; Windows control spacer via
`ml-auto`; the Windows/mac ordering swap; active tool = `tertiary`.

- **Separators use the wrong token.** Every editor separator in the DOM is
  `bg-border`; `ui/primitives.rs::Separator` hardcodes `theme.separator`
  (`border` mixed to 85% alpha). Affects the title bar, tool options and the
  wallpaper sheet.
- Button radius (§1).

### Tool options

`TOOL_OPTION_*` (28 tall, 8 pad, 8 gap) match the popover-style triggers.

- **The four `Select`-backed triggers are shaped like the popover ones.**
  Thickness, arrow style, highlight opacity and shape fill are HeroUI selects:
  `ps-2` + `pe-7` (28px trailing) with a 16px indicator absolutely placed at
  `end-2`. GPUI gives them symmetric 8px padding and a 14px in-flow chevron, so
  each is ~12px narrower with a 2px-smaller chevron.
- **No chevron rotation while open**, and GPUI keeps the hover background while
  the menu is open (the DOM triggers keep their resting background and rotate
  the chevron instead).
- Number badge: the DOM SVG is `r="10"` in a 24 viewBox scaled to 20px — a
  16.7px circle with ~9.2px bold text inside a 20px box. GPUI draws a full 20px
  circle with 11px text.
- Shape-fill preview: the DOM rect spans 2→14 with a 2px centred stroke inside a
  16px box; GPUI borders the full 16px box.
- Select-indicator colour is `--field-placeholder`, not `muted-foreground`.

### Zoom control

`ZOOM_INSET` 16, `ZOOM_PAD` 4, `ZOOM_GAP` 2, `ZOOM_RESET_MIN` 56,
`bg-surface/90` and `shadow-lg` all match. Deltas: radius 24 → 6,
`backdrop-blur-md` missing, and the ± icons render at 16px instead of 14px
(§3.4).

## 7. History

### Chrome

Header (`px-4 py-3`, 14px medium title, `border-b border-border`), content
padding 12, grid gap 12, list gap 8, empty-state icons 40/32 and copy all match.

- Window radius 12 → 3.
- **Toolbar buttons are 4px too tall.** The filter chips are `h-6 px-2 text-xs`
  (24px, 8px) with 12px icons and `mr-1`; GPUI uses `ButtonSize::Xs` (28px, 10px
  padding, 14px icons, 8px gap). The sort and layout buttons are `h-6 w-6` with
  14px icons; GPUI uses `IconXs` (28px, 16px icons). `HISTORY_CHIP_HEIGHT = 24`
  exists in `chrome.rs` but is dead code.
- Header "Clear All" is the right height (28) but has a 14px icon and an 8px gap
  where the DOM has 12px and 4px.

### Items

- **Grid cards are 6px narrower than Electron's.** The DOM uses
  `grid-cols-2 gap-3` inside `p-3` → 182px columns; `item.rs` reserves
  `RING_INSET * 4` for a selection wrapper and lands on 176px. In the DOM the
  selection ring is `ring-2 ring-blue-500 ring-offset-1`, which is a box-shadow
  and consumes no layout at all. The same cause makes GPUI's grid a wrapping
  flex row rather than a real 2-column grid.
- **Every item text is 11px; the DOM is `text-xs` = 12px** (grid timestamp, list
  title, list meta, "No preview").
- Card / row radius 8 → 2, list thumb 6 → 1.5.
- Overlay action buttons are 28px with 16px icons; the DOM has 24px (`h-6 w-6`)
  with 12px icons. Delete hover is `theme.danger/0.8` vs `bg-red-500/80`
  (`#ef4444`); the list-row delete is `text-red-400` in the DOM.
- **No thumbnail loading state.** Both DOM items render an `animate-spin` ring
  while the thumbnail resolves; GPUI shows "No preview" until the image lands.
- Hover has no `transition-all`.
- GPUI adds a **right-click context menu** on history items that the DOM has no
  equivalent for.

### Behaviour

Matches: arrow keys + `hjkl` with 2 columns in grid / 1 in list, `Enter` to
open, `Backspace`/`d` to delete, `Escape`/`q` to close, index clamping after a
delete, filter/sort resetting the index and scrolling to the top, layout toggle
preserving the index, preference persistence, close-on-blur.

- `move_selection` returns early when the index does not change, so pressing
  Down on the last item does not switch the selection ring on; the DOM sets
  `isKeyboardNavigationActive` on any navigation key.
- `scroll_to_item` is instant; the DOM uses
  `scrollIntoView({ behavior: 'smooth', block: 'nearest' })`.
- No "Loading…" state before the first load resolves.
- The DOM ignores navigation keys while focus is in a text input
  (`shouldIgnoreGlobalKeyboardShortcuts`); GPUI only guards on an open menu.

## 8. Settings

Correct: 880×700 window, 240px sidebar, 720px content cap, `px-6 pt-3 pb-8`,
40px drag strip, nav `gap-2.5 / px-2.5 / py-1.5` with 16px icons and
`row-active` / `row-hover` states, 18px `mb-4` heading, 14px medium labels with
12px muted descriptions.

- Nav radius 24 → 6.
- **Content is not centred.** The DOM page is `mx-auto max-w-[720px]`; GPUI is
  `w_full().max_w(720)`, so on a wide window the column hugs the left edge.
- **The sidebar is flat.** `.poratake-settings-sidebar` is a vertical gradient
  (88% → 72% of `--sidebar-background`), a right hairline, two inset highlight
  shadows and `backdrop-filter: blur(18px) saturate(125%)`. GPUI paints a solid
  `sidebar_background`.
- "SETTINGS" is 11px without letter-spacing; the DOM is `text-xs` (12px) with
  `tracking-[0.12em]`.
- The footer separator is inset by 8px each side; the DOM `<Separator>` spans the
  full 240px. Footer item gap is 2px vs `space-y-1` = 4px.
- **Slider rows have the wrong shape.** The DOM stacks them: label + value on one
  row, a **full-width** slider beneath, then the description (`space-y-3 py-2`).
  GPUI renders a `labelled` row with a 200px control column holding a ~160px
  slider and a 40px readout. The readout is also 12px where the DOM is
  `text-sm` (14px).
- **Input rows have the wrong shape** for the same reason — the DOM is
  `grid gap-2 py-2` (label above, full-width input, 12px hint below); GPUI is a
  right-aligned 280px field with an 11px hint.
- Select control width is 200px; `SettingsSelect` is `w-40` = **160px**.
- Row rhythm: the DOM separates items with `space-y-4` (16px) and sections with
  `space-y-6` (24px); GPUI uses `py(10)` per row (20px between rows). Compact
  (shortcuts) rows are `space-y-1` = 4px vs GPUI's 8px.
- Cloud "Test Connection" is `variant="outline"` at the default 36px height with
  spinner / check / cross icons and a coloured status message beside it; GPUI is
  a 32px secondary button with a label only.
- **Shortcut rows.** The DOM's record control is a **Button** (`outline`, or
  `primary` while recording) — transparent with a 1px border, `text-sm`
  `font-normal` `tracking-wide`, `min-w-36` compact — and it shows the
  combination live as it is pressed. GPUI renders a `bg(theme.default)` div at
  12px text with `min_w 150` and `py 7` (≈30px tall), shows only "Press keys…"
  while recording, labels the empty state "Not set" instead of "Record
  shortcut" / "Press a key", and wraps the row in `labelled`, which applies
  medium weight (the DOM label is `font-normal`) and would also print the
  description outside the shortcuts category. Its clear button is 28px where the
  DOM's is `icon-sm` = 32px, and the gap is 6px vs 4/8px.

## 9. Video editor

Correct: 288px sidebar (240–560 resize range, 6px handle), 40px tab rail with
`gap-1`, `py-2`, `border-l border-border`, `bg-card` and 32px `icon-sm` buttons
with 16px icons; panel `space-y-4 p-4` = gap 16 / pad 16; field groups 8px;
panel headers (14px medium title, 12px muted description, `size="sm"` switch);
14px labels with 12px muted readouts; `.small()` sliders; `TRACK_HEIGHT` 24;
timeline zoom 100 / 10 / 500 / 1.25.

- `panel_kit::label` has no `font_weight`; the DOM `Label` is `font-medium`.
- All button radii (§1) and the `IconXs` icon size (§3.4) apply here too.
- `VIDEO_TAB_BUTTON_SIZE` is dead code — the rail uses `ButtonSize::IconSm`,
  which happens to be the same 32px.

## 10. Menus, popovers, tooltips

### Tooltip

- Radius 12 → **3** (`min(32px, --radius-xl)`).
- **Wrong surface token**: `theme.popover` = `--popover` = `variant.surface`;
  HeroUI's `.tooltip` is `bg-overlay` = `mix(surface 94%, bg)`.
- GPUI adds a 1px `border` the DOM tooltip does not have. HeroUI's dark
  `--overlay-shadow` is `0 0 1px 0 rgba(255,255,255,0.3) inset` — an inset
  hairline, not an outline — and GPUI uses `shadow_md()`.
- Missing `max-w-xs` (320px) and `break-all`.
- No enter/exit animation (`fade-in-0 zoom-in-90` 150ms in, `zoom-out-95
fade-out` 100ms out).
- Delay and placement come from gpui defaults; the DOM pins `delay={150}` via
  `TooltipProvider` and several call sites pin `side="bottom"` /
  `side="left" sideOffset={8}`.

Padding 8 and text 12 are correct.

### Menu / dropdown / select popover

- Popover radius 6 is **correct**. But GPUI adds a 1px `border`; HeroUI relies on
  the inset shadow only.
- **Item height 28 vs `min-h-9` = 36.** GPUI's single height matches the app's
  `.select__popover--sm` override, not the default popover.
- Item padding 8 / gap 8 vs `px-2.5` (10) / `gap-3` (12) inside select popovers.
- Container padding 4 vs `p-1.5` (6) for full-size popovers.
- `MENU_MIN_WIDTH` 128 vs `min-w-(--trigger-width)` (plus `min-w-40` /
  `min-w-48` at specific call sites).
- Missing the item press `scale(0.98)` and the check-mark `stroke-dashoffset`
  draw-on.
- In-menu separators use `theme.border`; the DOM uses `bg-separator`.

## 11. Animations and transitions — consolidated

Nothing in the GPUI shell animates except the window-move tween in `chrome.rs`.
The DOM animations that are absent:

| surface                                                    | Electron                                   | GPUI                                        |
| ---------------------------------------------------------- | ------------------------------------------ | ------------------------------------------- |
| button press                                               | `scale(0.96–0.98)`, 250ms `--ease-smooth`  | none                                        |
| button/tab/field hover                                     | `background-color` 100–150ms               | instant                                     |
| switch thumb                                               | `margin` 300ms `--ease-out-fluid`          | snaps                                       |
| slider knob drag                                           | `scale(0.9)`, 250ms                        | none                                        |
| tab indicator                                              | `translate,width` 250ms `--ease-out-fluid` | jumps                                       |
| select / popover / menu open                               | `fade-in-0 zoom-in-95` 150ms, exit 100ms   | instant                                     |
| tooltip                                                    | `fade-in-0 zoom-in-90` 150ms, exit 100ms   | instant                                     |
| dialog                                                     | `fade-in-0 zoom-in-95`, 200ms              | none (`DIALOG_FADE_MS`, `DIALOG_ZOOM` dead) |
| select / popover chevron                                   | `rotate-180`, 150ms                        | none                                        |
| history item hover                                         | `transition-all`                           | instant                                     |
| history keyboard scroll                                    | `scrollIntoView` smooth                    | instant jump                                |
| list-box check mark                                        | `stroke-dashoffset` 250ms                  | instant                                     |
| capture-preview copy badge                                 | `animate-in zoom-in-50` 300ms              | verify                                      |
| capture-preview progress                                   | `transition-all` 300ms                     | verify                                      |
| indeterminate progress                                     | `progress-indeterminate` 1.2s loop         | no equivalent                               |
| text caret                                                 | native blink                               | static                                      |
| backdrop blur (toolbar, loupe, zoom bar, settings sidebar) | `backdrop-blur-*`                          | none                                        |

`PREVIEW_HOVER_MS`, `SWITCH_TRAVEL_MS`, `DIALOG_FADE_MS` and `DIALOG_ZOOM` are
all `#[allow(dead_code)]` — the constants were ported but never wired up.

## 12. Other

- `ui/primitives.rs::Progress` defaults to a 6px track; `ui/progress.tsx` pins
  `ProgressBar.Track className="h-2"` = **8px**.
- Recording control bar: `ToolbarSurface className="gap-3 px-4 py-2.5"`
  overrides the base padding, so the DOM bar is gap 12 / px 16 / py 10; GPUI
  uses the unoverridden `gap 2 / p 4`. Plus the radius (§1) and `bg-muted` (§2)
  issues.
- `--cursor-interactive` is `default` in this app (HeroUI ships `pointer`), and
  `base.css` also forces `cursor: default` on every button. GPUI's
  `cursor_pointer()` calls (wallpaper tiles, the window-pick overlay, the
  all-in-one target menu) therefore show a hand where the DOM shows an arrow.
- Narrow windows: HeroUI's `.button` carries `md:` (768px) and `sm:` (640px)
  variants, so in the 400px history popover and the 236–540px recording bar the
  _unsized_ base metrics would apply (h-10, 20px icons). In practice every
  button on those surfaces carries explicit size classes, so this does not bite
  today — but it will if a plain `<Button>` is ever added to one of them.

## 13. What the existing Rust tests do and do not prove

`theme/vars.rs` is a genuine 1:1 port and its tests verify the mixing formulas
against re-derived expectations for all 13 presets — that layer is sound. By
contrast the tests in `ui/chrome.rs`, `editor/tool_options.rs` and
`ui/switch.rs` assert constants against the same literals they define
(`assert_eq!(BUTTON_RADIUS, 24.0)`), so they lock in whatever value was written
down. Every discrepancy in §1, and the switch thumb/offset errors in §4, are
covered by a passing test today.

---

# Remediation status

Everything above is the audit as first written. This section records what has
since been changed in `src/main/app-gpui/`, and what is still outstanding.

## Fixed

**§1 Radius scale.** `chrome.rs` now derives the whole scale from a documented
`ROOT_RADIUS = 2.0` (`--radius: 0.125rem`) with `RADIUS_SM`…`RADIUS_4XL`
constants, and every affected token points at one of them. The tautological
assertions were replaced with `radius_scale_follows_the_css_custom_property`,
which pins the multipliers _and_ asserts each step is a quarter of the stock
Tailwind value, so a constant written as a stock number now fails the build.
Literal radii outside `chrome.rs` (history cards and thumbs, colour-picker
popover and area, menu items, hex row) were rebound to the same constants.

**§2 `bg-muted`.** All seven sites now use `theme.muted_background`. The video
aspect tiles additionally gained the DOM hover treatment
(`hover:bg-accent hover:text-accent-foreground`).

**§3 Buttons.** `IconXs` now defaults to a 14px glyph per
`.button--xs svg:not([class*='size-'])`, and a new `Button::icon_size` carries
the explicit `size-*` the DOM puts on individual icons (16px on the editor
tools, 12px on the history chips and item actions, 14px on the zoom bar).
`Button` also gained `height`, `padding_x`, `gap`, `min_width`, `font_weight`,
`flex_1` and `icon_spinning`, which is what the renderer expresses by layering
utilities on a size variant.

**§4 Slider.** The knob travel now mirrors the HeroUI transparent
`border-x-[0.75rem]` content box: an inner box inset by half a knob carries the
fill and thumb, and the strip the border used to cover is painted as the fill
start cap. `knob_offset`/`knob_offset_from_layout` model the intended offset and
the composition the renderer actually performs, and a test asserts they agree
across both knob sizes, four track widths and eleven positions — so the knob can
no longer drift off the track unnoticed. The pointer mapping now reads the inner
box, matching React Aria.

**§4 Switch.** `checked_offset` no longer subtracts the margin twice (the DOM is
`calc(100% - 1.5rem)` = `track − thumb − margin`), `SWITCH_SM_THUMB` is 16.5px
rather than the 14.4px from a code comment that assumes a 14px root font, and
the disabled thumb takes `default-foreground/20` when off and 0.4 opacity when
on. A test asserts the trailing gap equals the leading `ms-0.5` at all three
sizes.

**§4 Select.** Now sits on `--default` with a `--default-hover` hover (the
`variant="secondary"` the renderer passes), drops the invented open-state
border, places a 16px indicator at `end-2` behind `pe-7` of content padding in
`--field-placeholder`, rotates it while open, and dims the whole control when
disabled.

**§4 Fields.** `--field-hover` was added to `ThemeVars` (with a
`mix_parsed_weights` helper, because the two percentages in that token do not sum
to 100). Text fields now hover to it and focus with a flush 2px accent ring
(`focus_ring`, a spread-only box shadow, so focusing no longer shifts layout by a
border width), and keep their placeholder while focused and empty. The text area
dropped the border the DOM explicitly removes and moved onto `bg-field` with
`px-3`.

**§5 Overlay.** Crosshair guides are drawn; the selection frame and every handle
carry their black hairline rings; the dimension readout is monospaced; the
toolbar and its capture-target trigger use the hard-coded `white/85` and
`white/15` the DOM pins rather than theme colours; the surface takes
`shadow-2xl`; and window picking now punches the scrim around the hovered target
instead of dimming everything and drawing a frame plus a name label the DOM
never shows. The prompt gained its macOS offsets.

**§6 Editor.** Separators take `--border` (via a new `Separator::color`) rather
than `--separator`. The four `Select`-backed tool options render as select
triggers — `ps-2` + `pe-7` with an absolutely placed 16px indicator — while the
popover-backed ones keep the in-flow 14px chevron; both rotate it while open via
a new `Icon::rotate_180`, and neither changes its background when open. The
number badge is a 5/6-diameter disc with an 11/24 glyph, and the shape-fill
preview is inset to the 14/16 box the centred 2-unit stroke produces.

**§7 History.** Toolbar chips and icon buttons are 24px with 12/14px glyphs; item
text is 12px; the selection ring is a box shadow, so grid cards are the full
182px `grid-cols-2` gives them; item actions are 24px with 12px glyphs and the
fixed `red-500`/`red-400` the DOM uses; thumbnails show a spinning loader while
they resolve; and any navigation key now switches the keyboard ring on, matching
`setIsKeyboardNavigationActive`.

**§8 Settings.** Slider and input rows are stacked the way the DOM stacks them
(label + value row, full-width control, description/hint below) instead of
right-aligned control columns; the page groups items into sections with the
`space-y-6`/`space-y-4` rhythm; the content column is centred; selects are 160px;
the sidebar carries its gradient wash and right hairline, a 12px title, a
full-width footer separator and a `space-y-1` footer; the search field is the
styled label around a bare input with a leading magnifier, and the shortcut
search is its own `h-8 w-64 px-2.5` field. Search results were rewritten to the
title/count/uppercase-section structure. "Test Connection" is an outline button
at the default height with a spinner/check/cross, a coloured status message, the
unconfigured hint and the 3s reset.

**§8 Shortcut recorder.** Now the outline button (primary while recording) at
14px `font-normal` with the right `min-w`, space-separated accelerators, and the
DOM copy ("Record shortcut" / "Press a key" / "Press keys…"). The `singleKey`
flag from `registry/shortcuts.ts` is modelled on `Control::Shortcut`, so the 22
bare-key bindings accept a single alphanumeric and the 14 global accelerators
once again require a modifier — previously any field would accept a bare key. A
test pins which items are single-key.

**§9 Video editor.** Panel labels are `font-medium`.

**§10 Tooltip and popovers.** The tooltip moved to the `--overlay` token, dropped
its invented border, gained `max-w-xs`, and takes the 3px radius. Menu popovers
dropped their border, and list items are 36px with `px-2.5`/`gap-3` inside 6px of
padding — with a `compact` mode carrying the `.select__popover--sm` metrics,
which the small editor-panel selects now request. In-menu separators take
`--separator`.

**§11 Animations.** Tooltips and popovers fade in over 150ms
(`primitives::overlay_enter`); the cloud-upload button and the history
thumbnails spin (`Icon::rotate_turns` + `spinner_element`, one turn per second,
matching `animate-spin`).

**§12 Recording bar.** Takes the `gap-3 px-4 py-2.5` that overrides the shared
toolbar padding, plus `shadow-2xl`.

**Tabs.** The secondary variant now draws its bottom hairline, tabs are
`font-medium`, and unselected tabs fade to 70% on hover rather than recolouring.

**Progress.** 8px track, per `ProgressBar.Track className="h-2"`.

## Bugs found and fixed while verifying in the running app

- **Onboarding crashed on the shortcuts step.** `shortcut_input::render` read the
  owning entity back out of the context (`cx.entity().read(cx)`), which panics
  while that entity is being updated — i.e. always, from inside its own render.
  The recording state is now passed in. The same pattern remains at seven sites
  in `windows/video_editor/panels.rs`; see below.
- **Icons with more than one sub-path were malformed.** `generate-gpui-icons.ps1`
  concatenates each `d` attribute, but SVG treats the leading `m` of a path as
  absolute while the pairs after it stay relative. Concatenation therefore
  resolved those sub-paths against the end of the previous one — `X` rendered as
  a single diagonal instead of a cross, and `Aperture`, `CircleCheck` and others
  were similarly wrong. The generator now promotes the leading `m` to `M` and
  re-attaches the remainder as an explicit relative `l`.
- **The onboarding footer overflowed.** Both action buttons used `w-full`, and
  `.button` is `shrink-0`, so the first took the row and the second was clipped
  to a few pixels. The DOM uses `flex-1`; `Button::flex_1` now does too.
- `CheckCircle` was missing from the generated icon set, so the cloud test
  success state drew nothing.

## Outstanding, with reasons

Blocked on gpui capabilities:

- **Press scale on buttons** (`scale(0.96–0.98)` over 250ms). gpui cannot
  transform a `div`, and `Button` is `RenderOnce` with no pressed state to drive
  an animation from.
- **Hover/background transitions** (100–150ms on buttons, tabs, fields, select
  triggers). gpui applies hover styles instantly; there is no per-property
  transition.
- **`zoom-in-90/95` on popovers and tooltips.** Only the fade is reproduced.
- **Backdrop blur** on the overlay toolbar, colour loupe, zoom bar and settings
  sidebar. No gpui equivalent; the sidebar gradient wash is reproduced, its blur
  is not.
- **Letter-spacing** on the settings sidebar title (`tracking-[0.12em]`).
- **Focus rings on buttons.** The helper exists and fields use it, but gpui
  buttons here are not focusable, so there is no keyboard focus state to ring.

Deliberately not attempted in this pass:

- **The switch thumb and tab indicator do not animate** (300ms
  `--ease-out-fluid`, 250ms). Both are expressible with `with_animation` but need
  the owning view to hold the previous value; worth doing as a follow-up.
- **The area overlay has no editable, persistent selection.** The DOM overlay
  keeps a live selection the user can move, resize and constrain, with move and
  eight resize cursors (`use-area-selection.ts`); GPUI captures on mouse-up and
  shows only a crosshair. This is a state-machine port, not a styling fix.
- **History keyboard scrolling is instant**, where the DOM uses
  `scrollIntoView({ behavior: 'smooth' })`.
- **The seven remaining `cx.entity().read(cx)` calls in
  `windows/video_editor/panels.rs`** are the same latent panic fixed in
  `shortcut_input`. They were not touched because they sit outside the styling
  scope and each needs its own accessor; the video editor panels should be
  exercised in the running app before shipping.
- **History items have a right-click context menu** the DOM has no equivalent
  for. Left in place rather than deleting working functionality.

## Verification

`cargo fmt --check`, `cargo test` (373 pass), `cargo clippy --all-targets --
-D clippy::disallowed_methods` (the Rust gate `bun run test:daemon-win` runs) and
`cargo build` are all clean, with the pre-existing 10 dead-code warnings in this
untracked crate unchanged.

Checked in the running app: the editor title bar, tool options and zoom bar
(6px radii, select-trigger shape, tooltip); the onboarding welcome and shortcuts
steps (card radii, shortcut recorder, footer, `X` glyph). The settings, history,
video editor and overlay surfaces are covered by the build and tests but were not
opened interactively — the tray and global-shortcut entry points were not
reachable from this harness.
---

# Second pass

The first pass listed six items as outstanding. This pass closed four of them,
proved two genuinely impossible in gpui 0.2.2, and added guards so the worst
defect classes cannot return.

## Closed

**Editable selection with resize cursors.** `capture/selection.rs` is a 1:1 port
of `renderer/utils/area-selection.ts`: the eight handles with their hit boxes
(16px corner / 12px edge, corners tested first), the cursor each one shows,
`resize`/`move`/`fit`/`normalize`/`clamp`, the aspect-ratio adjustment, and both
minimum sizes. The overlay now holds a live `rect` plus an `Interaction`, and a
press dispatches through `selection::gesture_at` — a handle resizes, the inside
of the box moves with its grab offset, anywhere else starts a new box. The
pointer drives all eight resize cursors plus `move`, where before the overlay
was permanently a crosshair. Twelve tests cover the geometry and the dispatch,
including a round-trip that asserts a grab-and-return leaves the box untouched.

`auto_confirm` distinguishes the two modes `area-overlay/session.ts` has: the
plain capture flows still close on release, and the all-in-one flow (which
Electron opens with `autoConfirm: false`) captures and leaves the overlay up so
the box can be nudged and captured again.

The overlay has no aspect-ratio source yet — the renderer gets one over IPC from
`setAreaSelectorAspectRatio` — so the geometry runs unconstrained and
`resize_rect` takes the ratio the day a flow supplies one.

**Switch thumb travel.** The thumb now animates over
`margin 300ms var(--ease-out-fluid)` rather than snapping. `Window::use_keyed_state`
holds whether the switch has painted before, so a toggle animates and the first
paint lands on its resting position instead of sliding in from the other side.
`primitives::cubic_bezier` evaluates the CSS curve directly, so `--ease-out-fluid`
is the real `cubic-bezier(0.32, 0.72, 0, 1)` and not an approximation.

**Tab indicator travel.** The indicator moved from inside the selected tab to the
list container, where it is placed in fractions of the row (the tabs are
`w-full`, so they share it equally) and animates `translate` over 250ms on the
same curve — matching `.tabs__indicator`. Per-element state suppresses the
initial slide the same way.

**Backdrop blur, as far as it goes.** The settings sidebar's `backdrop-filter`
samples the content background behind it, which is a flat colour — blurring it is
a no-op, so the gradient wash reproduces the whole visible effect. The three
remaining cases sample a screenshot and cannot be reproduced; see below.

## Proven impossible in gpui 0.2.2, not merely skipped

I checked the API surface rather than assuming:

- **Button press-scale** and **popover `zoom-in-90/95`.** `Transformation` and
  `with_transformation` exist only on `svg` elements, and `paint_svg` is the only
  transform-taking paint call — a `div` cannot be scaled. Emulating the press
  with a padding inset needs a definite outer size, which content-sized buttons
  do not have; and a popover's text cannot scale at all. Worth noting the
  magnitude: at these sizes `scale(0.97)` on a 36px button is about 1px.
- **Hover/background transitions** (100–150ms). gpui applies hover styles
  instantly and has no per-property transition; `with_animation` runs once per
  element-id lifetime, not on state change, so it cannot drive a hover fade.
- **Backdrop blur over content** (overlay toolbar, colour loupe, zoom bar).
  gpui exposes blur only as `WindowBackgroundAppearance::Blurred`, which is
  window-level acrylic behind the whole window — it cannot blur a screenshot
  drawn inside the same window. Both surfaces are 90–95% opaque, so the blur
  contributes 5–10% of the pixel.
- **Letter-spacing** (`tracking-[0.12em]` on the sidebar title). No gpui
  equivalent.

## Guards added

`ui/lints.rs` scans the crate at test time, the way
`tests/unit/daemon-module-parity.test.ts` scrapes the daemons:

- **No view may read itself while rendering.** `cx.entity().read(cx)` panics
  while the entity is leased, which is always the case inside its own
  `Render::render`. This crashed the onboarding shortcuts step and would have
  crashed most of the video editor sidebar; the guard makes the whole class
  unshippable.
- **No stock Tailwind radius may be written as a literal.** `.rounded(px(24.0))`
  and friends are how the shell ended up with pill-shaped buttons; radii must
  come from `chrome::RADIUS_*`. The guard caught two more real cases: the About
  tab's app icon (56px/12px, where the DOM is `h-16 w-16 rounded-xl` = 64px/3px)
  and the toast, which has no DOM counterpart and now follows `toast.css`
  (`min(32px, --radius-3xl)` on `bg-overlay`, no border).

## Verification

`cargo fmt --check`, `cargo clippy --all-targets -- -D clippy::disallowed_methods`,
`cargo test` (387 pass) and `cargo build` are clean, with the pre-existing 10
dead-code warnings unchanged.

Confirmed in the running app at 1:1, in the editor's wallpaper sheet: the slider
knobs now sit flush **inside** the track at zero rather than overhanging it by
half a knob, and a toggled switch lands with the corrected 2px trailing gap —
the two numeric fixes with the highest chance of being wrong. Also confirmed:
the sheet's selects, swatch grid, panel labels and tooltip, and both onboarding
steps.

Settings, history and the video editor still have not been opened interactively.
The tray menu is reachable — its items enumerate correctly over UI Automation
(`All-in-one` … `Settings...` … `Quit`), so the tray itself works — but invoking
an item needs synthetic desktop input, which is not something to do on the
user's machine unasked. The `no_view_reads_itself_while_rendering` guard covers
the specific defect that would have crashed those windows.
---

# Third pass

## The three unverified windows are now covered

I could not open settings, history or the video editor from this harness — the
tray menu enumerates fine over UI Automation, but invoking an item needs
synthetic desktop input. So instead of eyeballing them, they are now rendered
headlessly in gpui's own test app (`windows/smoke.rs`), which is stronger than a
screenshot for the failure mode that actually shipped here:

- **`every_settings_category_renders`** draws every settings category, not just
  the default. The shortcuts category is the one that panicked.
- **`the_history_window_renders_in_both_layouts`** draws the grid and list
  layouts and walks all three filters, covering the item cards, the toolbar and
  both empty states.
- **`every_video_editor_panel_renders`** draws all ten sidebar panels. Seven of
  them read the owning entity mid-render before this work, so each would have
  panicked the moment it was opened.

This needed `gpui`'s `test-support` feature as a **dev-dependency** (the shipped
binary is unaffected) and a `state::set_test_state` that installs the globals the
windows read against a temp config, without starting a daemon.

**These tests were verified to fail on the real bug, not just to pass.** I
reintroduced `cx.entity().read(cx)` in the settings shortcut row and again in the
video editor's zoom panel; each time the corresponding test reproduced the exact
production panic (`cannot read … while it is already being updated`) and then
went green when reverted. The `ui/lints.rs` guard is the cheap first line; these
are the real net.

## Remaining gaps, with the check behind each

I stopped guessing at gpui's limits and confirmed them:

- **`gpui` 0.2.2 is the newest published version** (`cargo info gpui`), so there
  is no upgrade that adds what is missing.
- **Button press-scale and popover `zoom-in`.** `Transformation` and
  `with_transformation` exist only on `svg`, and `paint_svg` is the only
  transform-taking paint call — a `div` cannot be scaled, and text cannot be
  scaled at all. A padding-based inset would reproduce the geometry only for
  buttons with a definite outer size, which content-sized ones do not have. For
  scale: `scale(0.97)` on a 36px button is about 1px of travel.
- **Hover/background transitions.** gpui applies hover styles instantly and has
  no per-property transition; `with_animation` keys off an element id, not a
  state change, so it cannot drive a hover fade.
- **Letter-spacing.** No `letter_spacing` anywhere in gpui's text system.
- **Backdrop blur over content** (overlay toolbar, colour loupe, zoom bar). The
  reason is deeper than the missing filter: the GPUI overlay is a
  `WindowBackgroundAppearance::Transparent` window over the **live desktop** — it
  never draws the frozen frame the Electron overlay renders — so there is no
  in-app backdrop to blur, and gpui's only blur is window-level acrylic behind
  the entire window. The app does own a working separable blur
  (`render/blur.rs`), so this becomes reachable if and when the overlay renders a
  frozen frame. Both surfaces are 90–95% opaque, so the blur is 5–10% of the
  pixel.

That last point is worth calling out as a parity gap in its own right, beyond
styling: **Electron freezes the screen and renders the daemon's frozen frame in
the overlay** (gated by the "Freeze screen" setting), while the GPUI overlay is
transparent over the live desktop. That is a capture-pipeline change — daemon
frame retention plus rendering — not a UI fix, and it is the largest remaining
behavioural difference in the overlay.

## Final state

`cargo fmt --check`, `cargo clippy --all-targets -- -D clippy::disallowed_methods`,
`cargo test` (391 pass) and `cargo build` are clean, with the crate's
pre-existing 10 dead-code warnings unchanged.
---

# Fourth pass — correcting the third

## I was wrong about the overlay architecture

The third pass called the frozen-frame overlay "the largest remaining
behavioural difference" and described Electron as rendering the daemon's frozen
frame while GPUI overlays the live desktop. That was wrong, and the correction
matters because it changes what the actual gap is.

`AreaOverlayParams.imageUrl` is `null` at **both** assignment sites in
`capture/area-overlay/session.ts`; the `<img>` branch in
`area-overlay-window.tsx` is unreachable for every current flow. Electron's
overlay is a transparent window too. The freeze comes from `freezeScreen()`,
which asks the daemon to paint **its own native overlay windows** behind the
Electron one. Architecturally the two shells are the same.

A consequence: the `backdrop-blur-xl` on the overlay toolbar and colour loupe
almost certainly does nothing in Electron either — CSS `backdrop-filter` samples
the element's backdrop _within the page_, and in a transparent window that page
is empty. So GPUI lacking backdrop blur there is not a visible difference. (I
have not measured Electron to confirm this, so it is reasoning from the
transparency, not an observation.)

## The real gap, now closed: the freeze was never triggered

Only `freeze-screen freeze` populates the daemon's retained frames
(`store_frozen` in `modules/freeze_screen.rs`), and `screenshot capture-area`
reads them through `frozen_rect` when `cached` is set. The GPUI shell passed
`cached: freeze_screen` but **never called `freeze-screen freeze`** — so no
frames were ever stored, `cached` always found none, and every capture silently
fell back to a live one.

That meant the "Freeze screen" setting did nothing at all in the GPUI shell:
no still snapshot while selecting, and the captured pixels came from the moment
of the crop rather than the moment the overlay opened.

Now, mirroring `session.ts`:

- `with_frozen_screen` awaits `freeze-screen freeze` on the background executor
  before opening the overlay, so the snapshot is up before the user drags. A
  failed freeze logs and opens over the live screen rather than costing the user
  the capture.
- `close_all` releases the frozen displays when the overlay goes away, tied to
  the overlay's lifetime rather than to the capture succeeding.
- `prewarm_freeze_screen` runs at startup, as `capture/index.ts` does, so the
  first capture does not pay to initialise the pipeline.

## Guarding it

A silently-wrong module or method string is exactly how this class of bug hides,
so `daemon_contract.rs` now scrapes every `daemon.call` in this crate and checks
it against `DAEMON_METHODS` — parsed out of `src/types/daemon.ts` at test time,
so there is no second copy of the contract to drift. It covers all 25 calls, not
just the new ones. A second test asserts the shell drives the whole freeze
lifecycle, since a missing `release` leaves the user's screen frozen.

Verified by breaking it: renaming the module to `freeze-scren` fails with
`src/capture/mod.rs:104: unknown module`, naming the line.

## Final state

`cargo fmt --check`, `cargo clippy --all-targets -- -D clippy::disallowed_methods`,
`cargo test` (393 pass) and `cargo build` are clean, with the crate's
pre-existing 10 dead-code warnings unchanged.

The remaining gaps are the four gpui limits, unchanged and each confirmed against
the API rather than assumed: no `div` transform (press-scale, popover zoom-in),
no per-property transitions (hover fades), no `letter_spacing`, and blur only as
window-level acrylic. `gpui` 0.2.2 is the newest published version, so none of
them has an upgrade path.
---

# Fifth pass — two of the four "framework limits" were not limits

I had listed four gpui limits as permanent. Two of them were wrong.

## Hover transitions: closed

I claimed gpui could not do them because "`with_animation` keys off an element
id, not a state change". But that is exactly the problem already solved for the
switch thumb: key the animation id **on the state** and hold the state in
`Window::use_keyed_state`. The same pattern applies to hover, with `on_hover`
writing the flag.

`primitives::{HoverFade, hover_fade, track_hover}` is the shared bookkeeping, and
`theme::color::lerp_srgb` interpolates the way a CSS `transition:
background-color` does. Applied to:

- **Button** — `background-color 100ms var(--ease-out)`, the dominant hover
  surface in the app.
- **Select trigger** — `background-color 150ms var(--ease-smooth)`.

Both land on their resting colour on first paint rather than fading in, and
`primitives::{ease_out, ease_smooth}` evaluate the real CSS curves rather than
approximating them. Pressed state stays instant, which matches `button.css`: the
transition is on the colour and the pressed colour equals the hover colour for
every variant.

Verified in the running app: after clicking the zoom `+`, the pointer resting on
it shows the ghost hover background (it had none before), and a fresh launch
shows every button at its resting colour with no fade-in artifact.

Still swapping instantly, where the mechanism now exists to extend it: tab hover
(`opacity-70`), list-box items, and text-field hover. Each needs its own state
key; none is on a high-traffic surface.

## Letter-spacing: closed

`primitives::tracked_text` lays the spacing out instead of shaping it — one item
per character in a row with `tracking` between them, which is what
`letter-spacing` produces for a single-line run. The settings sidebar title now
gets its real `tracking-[0.12em]` (1.44px at 12px). Only suitable for short
non-ligature labels, which is the one place the renderer asks for it.

## Press-scale and popover zoom: closed doors, confirmed at the primitive level

Not just the public API this time — the scene graph itself. Of gpui's draw
primitives only `MonochromeSprite` carries a `TransformationMatrix`, that struct
is `pub(crate)`, and the sole public path to it is `Window::paint_svg`. `Quad`
has no transform field at all, and glyphs are painted through `ShapedLine::paint`,
which takes none. So a button (a quad plus text) and a popover (a quad plus text)
cannot be scaled by any means available to a consumer of the crate. `gpui` 0.2.2
is the newest published version, so the only route is a patched fork — worth
raising as a decision, not something to do unilaterally.

For scale: `scale(0.97)` on a 36px button is roughly 1px of travel.

## Localized backdrop blur: unchanged, and probably invisible anyway

gpui exposes blur only as `WindowBackgroundAppearance::Blurred` — window-level
acrylic behind the whole window, which cannot blur a region. And per the fourth
pass, Electron's overlay is transparent too, so its `backdrop-filter` has an
empty in-page backdrop to sample and very likely renders nothing either. Both
surfaces are 90–95% opaque regardless.

## Final state

`cargo fmt --check`, `cargo clippy --all-targets -- -D clippy::disallowed_methods`,
`cargo test` (393 pass) and `cargo build` are clean, with the crate's
pre-existing 10 dead-code warnings unchanged.
---

# Sixth pass — press-scale was reachable after all

## Press-scale: closed, without a transform

I had ruled this out because gpui cannot scale a `div`. That was the wrong
conclusion from a correct fact: what `transform: scale()` _does_ here is shrink
the painted box about its centre while the layout around it stays put, and that
is expressible without a transform.

`press_geometry` shrinks the box by `height * (1 - scale)` and hands exactly the
freed space back as margin, so the element's footprint is unchanged and nothing
around it shifts. The scales are the stylesheet's own: 0.97 at `md`, 0.98 at
`sm`/`xs`, 0.96 at `lg`. Three tests pin the invariant — that the shrink plus the
margin equals the original in both axes, that the box genuinely gets smaller, and
that a small button cannot end up with negative padding.

The difference from a real transform is that the glyphs keep their size. At these
magnitudes the shrink is roughly 1px, so a proportional glyph change would be
well under a device pixel.

## Remaining hover transitions: closed

- **Tabs** fade to `opacity-70` over 150ms, as `.tabs__tab` does.
- **Text fields** fade to `--field-hover` over 150ms, matching `.input:hover`.
- **List-box and menu items** need no work: `menu-item.css` and
  `list-box-item.css` transition only `transform` and `box-shadow`, never
  `background-color`, so the instant swap already matched. Checking that turned
  up a real defect instead — see below.

## A defect found while checking those transitions

The hovered menu row was filled with `--accent` and recoloured its label and
shortcut to accent foregrounds. HeroUI hovers both `.menu-item` and
`.list-box-item` to `bg-default` — a subtle surface lift — and does not change
the label colour at all. So every dropdown in the shell was painting a saturated
accent bar where the DOM shows a faint grey. Now corrected at all three sites.

Verified in the app: the thickness dropdown opens with 36px rows, no accent fill,
the trailing check on the selected row, and the trigger chevron rotated up.

## What is left

**Popover `zoom-in-90/95`.** The same box-shrink trick does not transfer: a
popover is content-sized, so shrinking it would reflow the rows inside rather
than scale them, and 5% of a ~200px popover is ~10px — large enough that the
reflow would read as a glitch rather than a zoom. The 150ms fade is in place and
carries most of the motion.

**Localized backdrop blur.** gpui exposes blur only as
`WindowBackgroundAppearance::Blurred`, which is window-level acrylic and cannot
blur a region. As established in the fourth pass, Electron's overlay is
transparent too, so its `backdrop-filter` has an empty in-page backdrop to
sample and very likely renders nothing either; both surfaces are 90–95% opaque
regardless. This is the one item where I am reasoning rather than observing.

Both would need a patched gpui fork. That is a dependency decision rather than
an implementation task, so it is flagged rather than taken.

## Final state

`cargo fmt --check`, `cargo clippy --all-targets -- -D clippy::disallowed_methods`,
`cargo test` (396 pass) and `cargo build` are clean, with the crate's
pre-existing 10 dead-code warnings unchanged.
---

# Seventh pass — the last two items, resolved rather than deferred

## Popover entrance: the missing piece was the slide, not the zoom

`popover.css` and `tooltip.css` compose **three** things on enter, not two:

```css
&[data-entering='true'] {
  @apply animate-in duration-150 ease-smooth fade-in-0 zoom-in-95;
  &[data-placement='bottom'] {
    @apply slide-in-from-top-1;
  }
  &[data-placement='top'] {
    @apply slide-in-from-bottom-1;
  }
}
```

I had implemented the fade and written off the rest as the ungettable `zoom`.
The 4px `slide-in-from-*-1` is entirely expressible, and I had simply missed it.
`overlay_enter` now takes an `EnterFrom` and animates opacity **and** the offset
on the real `--ease-smooth` curve: menus slide down from their trigger, tooltips
slide up from theirs.

That leaves only the 5% `zoom` of the three. It is the one component that needs
a transform: a popover is content-sized, so shrinking the box would reflow the
rows inside instead of scaling them, and 5% of a ~200px popover is ~10px — the
reflow would read as a glitch rather than a zoom. With the fade and the slide in
place, the entrance now carries two of its three components.

## Backdrop blur: three of the four sites are provable no-ops

I had been calling this "probably invisible" and reasoning loosely. The
argument is actually decidable from the markup, because CSS `backdrop-filter`
samples the _backdrop root_ — the page content behind the element — and a
transparent Electron window's backdrop root does not include the desktop.

- **All-in-one toolbar** (`bg-muted/95 backdrop-blur-xl`). The only thing behind
  it in the page is `SelectionScrim`, a uniform `bg-black/50` (or one of its four
  uniform bars). A blur of a uniform field is the identity, so the filter cannot
  change a pixel. Where the toolbar sits over the scrim's hole the backdrop is
  transparent, i.e. outside the backdrop root — also nothing.
- **Colour-picker loupe** (`bg-muted/95 backdrop-blur-xl`) — same page, same
  empty backdrop.
- **Settings sidebar** (`backdrop-filter: blur(18px)`) — the backdrop is the flat
  `--content-background`. Uniform again; the gradient wash reproduces the whole
  visible effect, which the fourth pass already established.

So GPUI already matches those three, and _adding_ blur there would make it
diverge from the renderer rather than converge.

The fourth site is real: the **editor zoom bar** (`bg-surface/90
backdrop-blur-md`) sits over the screenshot canvas, which is genuine in-page
content, so the filter does blur something there. It is deliberately not
implemented: it would mean cropping the displayed image at the bar's rect,
blurring it and re-uploading it on every zoom and pan, for a 10%-opacity effect
behind a ~110×36px bar. That is a poor trade, and it is a cost decision rather
than a framework limit — recorded here so it is a choice rather than an
omission.

## Final state

`cargo fmt --check`, `cargo clippy --all-targets -- -D clippy::disallowed_methods`,
`cargo test` (396 pass) and `cargo build` are clean, with the crate's
pre-existing 10 dead-code warnings unchanged.

One item now genuinely requires a patched gpui fork — the popover `zoom-in-95`
— and one is a deliberate cost trade (the zoom bar's blur). Everything else the
audit identified across all seven passes is closed.
---

# Eighth pass — pinning what was built, and the honest end state

## The animation contract is now pinned

Everything the previous passes added was verified by eye once and then left
unguarded. `primitives::tests` now asserts it:

- All three ported CSS curves start at 0, end at 1, are monotonic, and hit their
  real midpoint values (`--ease-out` 0.839, `--ease-smooth` 0.802,
  `--ease-out-fluid` 0.955), ordered by how front-loaded they are.
- The entrance slide is one spacing step (4px) over 150ms, from the placement
  edge.
- `tracking-[0.12em]` resolves to 1.44px at the 12px sidebar title.
- The focus ring is 2px with a 2px offset.

Writing those caught a mistake in my own reasoning: I had assumed
`--ease-smooth` (CSS `ease`) was symmetric about the midpoint. It is not — it is
front-loaded, reaching 0.802 at half time. The implementation was right; my
expectation was wrong. That is worth recording because it is the fourth time in
this work that a confident assumption turned out to be the thing that needed
checking.

## The two items that remain, stated plainly

**Popover `zoom-in-95`.** Needs a transform over a composited subtree. gpui has
none: of its draw primitives only `MonochromeSprite` carries a
`TransformationMatrix`, that type is `pub(crate)`, and `Window::paint_svg` is the
only public path to it — `Quad` has no transform field and glyphs go through
`ShapedLine::paint`, which takes none. Every workaround I could construct is
worse than the omission:

- Shrinking the box reflows the rows inside instead of scaling them, and 5% of a
  ~200px popover is ~10px of reflow — that reads as a glitch, not a zoom.
- Scaling only the background through `paint_svg` would leave the text sitting
  outside the smaller surface mid-animation.
- Fixing the size and clipping is a reveal, not a scale, and would visibly cut
  the last row.

What mitigates it: the entrance composes three things, and the two that are
implemented mask the third. Opacity and scale run on the same 150ms curve, so
the scale error is largest exactly when the surface is most transparent — at
quarter time the popover is 41% opaque and would be at 97.9% scale.

**Editor zoom bar `backdrop-blur-md`.** Not a cost trade, as I previously called
it — closer to impractical. The backdrop is the composited canvas: the
screenshot plus wallpaper plus annotations at the current zoom and pan. Blurring
it correctly means re-running that composite for the bar's rect on every frame
that changes any of those. The other three blur sites are provable no-ops (see
the seventh pass), so this is the only one that would show, at 10% opacity behind
a ~110×36px bar.

Both would need a patched gpui fork. Carrying a forked framework is a standing
maintenance cost on every future gpui upgrade, which is why it is written down as
a decision for the owner of this codebase rather than taken unilaterally.

## Final state

`cargo fmt --check`, `cargo clippy --all-targets -- -D clippy::disallowed_methods`,
`cargo test` (400 pass) and `cargo build` are clean, with the crate's
pre-existing 10 dead-code warnings unchanged.
---

# Ninth pass — the two remaining items, investigated to the end

I said I would check upstream before treating either as final. I have.

## Popover `zoom-in-95`: not available in any gpui, including unreleased

`gpui` 0.2.2 has no transform on `Style`/`StyleRefinement`, and neither does
**upstream `main`** — checked directly against
`crates/gpui/src/style.rs` and `crates/gpui/src/styled.rs` in
`zed-industries/zed`. Neither file contains `transform`, `scale`, or
`TransformationMatrix` in the style types.

So "wait for the next release" is not an option, and the change is not a small
patch: it means adding a transform to `Quad` and threading it through the
shaders. That is a renderer change to the framework.

## Editor zoom-bar `backdrop-blur-md`: the backdrop is not a buffer

I called this "impractical" and then wondered whether caching would rescue it.
It does not, and now the reason is exact:

- `CanvasSnapshot` holds the _source_ image, the wallpaper backdrop and the
  per-redaction patches separately. There is no composited whole-canvas buffer
  anywhere — the composite happens in gpui's element tree at paint time.
- The only code that produces composited pixels is `export::compose`, which
  takes a whole `width`/`height` and no region. Compositing just the bar's rect
  would mean plumbing a region through the wallpaper rasteriser and every
  annotation renderer.
- Invalidation is the killer regardless: the snapshot carries `draft`, the
  in-progress annotation, which changes on every frame of a drag. So the blur
  would need a full-canvas composite per frame while drawing.

Approximating it from the source image instead would be wrong wherever the
wallpaper or an annotation sits under the bar — and the bar sits over the
wallpaper margin whenever a wallpaper is active. A 10%-opacity effect that is
sometimes visibly wrong is worse than its absence.

## Where this leaves the goal

Every parity item in this audit is now either closed or has a written reason
that names the exact blocking mechanism. The two that remain are not deferred
work; one needs a change to gpui's renderer that exists in no version of the
framework, and the other needs a composited backdrop this architecture does not
produce.

If closing them matters more than the cost, the options are, in order of
increasing commitment: accept them (they are a 5% scale on a 150ms fade, and a
10% blur behind a 110×36px bar); vendor a patched gpui for the transform; or
reconsider the framework. That is a decision about what this codebase carries,
not something to settle inside a styling pass.
---

# Tenth pass — sharpening the last two descriptions

Both remaining items were described less precisely than they deserved. Neither
description survives contact with the code.

## The zoom bar is not missing its backdrop — only the blur's low-pass

`zoom/index.tsx` is `bg-surface/90 backdrop-blur-md`, and `editor/window.rs`
draws `bg(theme.surface.opacity(0.9))`. Both composite the same 90% surface with
the canvas showing through at 10%. gpui composites that transparency for real,
so the canvas _is_ visible behind the GPUI bar.

The difference is therefore not "no blur behind the bar" but "the 10% that shows
through is sharp rather than low-passed". At a 10% contribution behind a
110×36px bar, that is a difference in the high-frequency detail of one tenth of
a small surface.

It is also approximable without the compositor: a blur of radius ~12 over a
region that small is close to its local average, and the snapshot already holds
both the source image and the rasterised wallpaper backdrop to average from. I am
not doing it, because the approximation misses annotations under the bar and the
gradient variation a blur preserves — and the thing being approximated is a few
percent of a few percent. Recording it so the option is on the table rather than
hidden.

## The popover zoom is implementable — and would look worse

I had this as a hard capability limit. It is not quite: a `scale()` on a popover
can be approximated by scaling the _layout_ rather than transforming it —
animating `text_size`, padding, row heights and radius together, which is a
proportional re-layout. gpui can express all of those, and driving it needs a
per-frame scale in the view state plus `request_animation_frame`.

I am not doing it, and the reason is quality rather than capability: it re-shapes
text at fractional sizes (14px → 13.3px and back) for the nine frames of a 150ms
animation. Glyph rasterisation at fractional sizes is not a linear scale — hinting
and subpixel positioning change — so the rows would wobble rather than zoom. A
5% scale delivered as text jitter is worse than the 5% being absent, especially
next to a fade that already starts from zero opacity and a 4px slide that are
both correct.

That is a judgment about which of two imperfect results is closer to the
renderer, and it is the kind of call worth stating out loud rather than burying
under "the framework cannot".

## Closing position

Nine passes of parity work are closed. What remains is: 5% of one entrance
animation, deliberately not faked because faking it looks worse; and the
sharpness of a 10% backdrop behind a 110×36px bar, deliberately not approximated
because the approximation can be wrong where the exact version cannot.

Both are now documented with the mechanism, the cost, and the option not taken,
so the next person can overrule either judgment with full information. Neither
is a loose end left by accident.
---

# Eleventh pass — the popover zoom, implemented

I had declined this on a quality judgment. That judgment was overruled, so it is
built: better to ship it and let it be seen than to withhold it on my own
assessment.

`zoom-in-*` is delivered as a **proportional re-layout** rather than a
transform, which gpui has none of for a `div`. `primitives::{enter_progress,
enter_scale, entering}` derive the factor from a clock — it has to be known
before the children are built, so it cannot come from an animation closure — and
the surface multiplies every length by it:

- **Menu popovers** (`zoom-in-95`): row height, row padding, row gap, container
  padding, text size and both radii.
- **Tooltips** (`zoom-in-90`): padding, text size and radius.

Both call `window.request_animation_frame()` while entering, so the layout is
re-evaluated each frame until the scale settles at 1. The entrance is now all
three of its CSS components: fade, 4px slide, and zoom.

A test asserts the factor starts at the CSS zoom, ends exactly at 1, never
overshoots the `zoom..=1` range at any point, and stops requesting frames once
settled.

The caveat I raised still applies and is worth knowing rather than hiding: this
re-shapes text at fractional sizes over the 150ms, and glyph rasterisation at
fractional sizes is not a linear scale, so the rows may read as slightly soft
during the entrance where a true transform would be crisp. If that turns out to
look worse than the previous fade-and-slide-only entrance, reverting is a matter
of dropping the `scale()` multipliers — the fade and slide are independent of it.

## The zoom bar blur: still not done, and why this one is different

I implemented the popover zoom because it was self-contained and the mechanism
was clean. This one is neither, and the risk runs the other way:

The bar's backdrop region has to be mapped from screen space back to image space
through the zoom, the `stage_scroll` pan offset and the canvas centring, then
sampled from either the source image or the rasterised wallpaper depending on
where it lands, blurred, and cached against all of those inputs. Getting that
mapping wrong puts a visibly wrong smear behind the bar.

Against that: gpui already composites the real canvas through the bar's 90%
surface, so what is missing is only the low-pass on that 10% — behind a
110×36px bar. So the work would add a chance of a visible defect in order to
remove an imperceptible difference.

That is the one item I am leaving, and it is a risk judgment rather than a
capability claim. If it should be built anyway, the mapping is the whole job and
`render::blur` is already there.

## Final state

`cargo fmt --check`, `cargo clippy --all-targets -- -D clippy::disallowed_methods`,
`cargo test` (401 pass) and `cargo build` are clean, with the crate's
pre-existing 10 dead-code warnings unchanged.

## Twelfth pass — the zoom bar's `backdrop-blur-md`

Built, so the list of gaps is now empty.

The tractable version of the problem: **blur over a flat field is the identity.**
So the bar only needs a sampled backdrop where it sits over the capture; over
the flat stage background the plain 90% surface it had before is already the
correct answer. `zoom_backdrop::sample_rect` returns `None` for anything that is
not entirely over the image, which drops the partial-overlap case (the one that
would have needed the rasterised wallpaper too) rather than approximating it.

- `src/editor/zoom_backdrop.rs` — `sample_rect` maps the bar's screen rect into
  image pixels through the zoom; `build` crops, blurs at sigma 12 (Tailwind
  `blur-md` is `blur(12px)`, and CSS filter blur takes a standard deviation) and
  uploads, reusing the existing upload when the key has not moved.
- `zoom_control` now draws that crop, then the 90% surface over it, then the
  content — the blurred pixels have to be _under_ the veil, not over it.
- The bar measures its own rect with a `canvas` because its width follows the
  percentage label. It asks for one more frame when that rect changes, and only
  then, so it converges instead of spinning.

Known limitation, stated rather than hidden: an annotation directly beneath the
bar is not in the sampled source, so it shows through sharp instead of blurred.
Sampling the composited canvas would fix it; there is no buffer to sample.

Verification: 4 unit tests on the mapping and the cache, plus a headless render
test (`the_image_editor_renders_and_measures_its_zoom_bar`) that draws the real
editor over several frames and asserts the measurement lands — confirmed to fail
when the recording is removed.

## Final state (updated)

`cargo fmt --check`, `cargo clippy --all-targets -- -D clippy::disallowed_methods`,
`cargo test` (406 pass) and `cargo build` are clean, with the crate's
pre-existing 10 dead-code warnings unchanged. The app launches and stays up.

## Thirteenth pass — resolving the one deferred judgment

The previous pass left the entrance zoom's fractional text sizing as a question
for the user. That was the wrong call: it is answerable from what the two
implementations actually do.

A CSS transform scales the **rasterised** result, so glyphs grow uniformly and
never re-shape. Scaling the font size instead re-lays out and re-shapes the text
on every frame of the 150ms entrance — a shimmer the transform provably does not
produce. Between "glyphs 5–10% small for 150ms" and "text visibly re-shaping for
150ms", the former is imperceptible and the latter is an artifact Electron does
not have.

So the box geometry (padding, radius, item height, gaps) still animates, which
is what makes the entrance read as a zoom, and the text size is now held fixed:

- `src/ui/menu/view.rs` — `item_text()` no longer multiplies by `scale()`.
- `src/ui/tooltip.rs` — `text_size` is `TOOLTIP_TEXT`, unscaled.

Both carry the reasoning in a comment so the next person does not "fix" it back.

Nothing is deferred to visual inspection any more.

## Final state (updated)

`cargo fmt --check`, `cargo clippy --all-targets -- -D clippy::disallowed_methods`,
`cargo test` (406 pass) and `cargo build` are clean, with the crate's
pre-existing 10 dead-code warnings unchanged. The app launches and stays up.

One incident worth recording: a `link.exe` LNK1120 with 56 unresolved
LLVM-internal symbols appeared mid-pass. It was stale incremental state, not a
source defect — `cargo test` had compiled and passed the same tree — and
`rm -rf target/debug/incremental` cleared it. Worth knowing before chasing it as
a code problem.

## Fourteenth pass — actual side-by-side pixel comparison

Every earlier pass compared _source_: read the CSS, port the resolved value. That
is not the same as looking at the two windows. This pass built a way to look, and
looking found five defects the source reading had missed.

### The rig

Electron's renderer is a web app that picks its window from `?window=`, its
settings tab from the URL hash, and reaches the main process only through the
preload bridge. So it can be drawn in an ordinary browser, with the bridge
stubbed:

- `scripts/parity/vite.renderer.mts` — the app's Vite config minus
  `vite-plugin-electron`, so no Electron process is spawned. Nothing in the app
  build references it.
- `scripts/parity/preload-stub.js` — stands in for `src/preload/preload.ts`.
  Injected as an on-new-document script. Set `APPEARANCE`, `ACCENT` and
  `SHORTCUTS` to what the GPUI app is reading from its dev config, or a
  difference will show up for a reason that has nothing to do with the shells.

The GPUI side needed to be reachable without clicking the tray, so `main.rs`
took a `--intent <tray-intent-id> [settings-category]` argument. It dispatches
the same `Intent` the tray menu does, and `Category::from_id` is the CLI
equivalent of `settings-window.tsx` reading its tab from `window.location.hash`.
That is what makes the shell scriptable at all.

Then: `--intent open-settings <category>`, passive window screenshot, same page
in the browser, compare.

### What that found

1. **Section grouping was wrong.** `groupBySection` keys a `Map` by section
   name, so a row joins its section's _first_ appearance. GPUI grouped only
   consecutive runs, which moved `Remember All-in-One choices` above the rest of
   the `Preview` section and put a section-sized gap inside a section. The
   General page rendered in a visibly different order.
2. **`--primary` is not the theme accent.** `useAccentColor` overwrites it with
   `systemPreferences.getAccentColor()` on every window, so in Electron the
   area-overlay frame, handles and crosshairs, and the Polish-default star,
   follow the colour picked in Windows. GPUI had no system-accent support at all.
   `system::accent` now reads the same DWM key Chromium does, and `--primary` is
   bound to it -- deliberately not to the preset, which the theme test now
   asserts.
3. **Shortcut row labels were too heavy.** `shortcut-input.tsx` passes
   `className="text-sm font-normal"`, overriding `Label`'s own `font-medium`.
   Every shortcut row was a weight too bold.
4. **The Storage page was badly broken.** The path picker's label was laid out
   _beside_ a `w-full` field row, so it was crushed to about one character wide
   and rendered one letter per line. Electron stacks it: a hard-coded
   `Save Location` label above a `flex gap-2` row. It was also missing the reset
   button, which only exists while a custom path is set.
5. **The naming-pattern row diverged in five ways** -- registry label and
   description instead of a hard-coded `Naming Pattern` plus a help icon, no
   reset button, the token list inline instead of behind the icon, the preview as
   plain text instead of a `<code>` chip, and the error replacing the preview
   line instead of sitting above it. All five now match.

Two of these -- 1 and 4 -- are the kind of defect no amount of reading the CSS
finds, because both shells' source looked right in isolation.

### Deliberate divergences, hidden rather than removed

Two things exist in GPUI and not in Electron: the theme-preview swatch grid on
the Appearance page and the `Scroll Capture` shortcut row. On the user's
instruction they are kept but tucked behind a collapsed `More options`
disclosure, so each page reads exactly like Electron until the extras are asked
for. `EXTRA_ITEM_IDS` drives it, and a test reads Electron's own registry to
fail if one of those rows is ever added there too. The naming-pattern token chips
work the same way, behind the help icon.

### Coverage, stated honestly

Compared page by page: **General, Appearance, Shortcuts, Storage.** Not yet
compared this way: Screenshot, Recording, Devices, Cloud, About, the history
window, the editor, the video editor, the overlays. The rig is committed, so
those are now a matter of running it rather than building it.

## Final state (updated)

`cargo fmt --check`, `cargo clippy --all-targets -- -D clippy::disallowed_methods`,
`cargo test` (413 pass) and `cargo build` are clean, with the crate's
pre-existing 10 dead-code warnings unchanged.

Two things worth knowing for next time:

- A `link.exe` LNK1120 with a few dozen unresolved LLVM-internal symbols is
  stale incremental state, not a source defect. `rm -rf target/debug/incremental`
  clears it. It showed up twice, both times right after `touch src/main.rs`
  interleaved with a concurrent `cargo clippy`.
- `clippy --all-targets` reports one pre-existing `needless_borrow` at
  `src/windows/video_editor/mod.rs:1211`. It is not gated by
  `-D clippy::disallowed_methods` and is untouched by this work.

## Sixteenth pass — onboarding, history, and the last blocked surfaces

### One defect

**Both windows that show the app icon drew the wrong picture.** `about-tab.tsx`
and `onboarding-window.tsx` render `<img src={appIcon}>` -- the real
`build/icon.png`. This shell drew an aperture glyph on an accent-coloured tile
instead, in both places. `ui::app_icon` now embeds and decodes the same file
(once, cached), keeping the tile only as a fallback for a corrupt asset. A test
reads both renderer files so the two pictures cannot drift apart again.

### Two surfaces compared, both matching

- **Onboarding**: title, subtitle, the two feature cards with their icon tiles,
  the dot indicators, `Next` and `Skip for now` -- all match.
- **History**: header with the title and the settings gear, the separator, and a
  centred empty state (`No captures yet` over
  `Take a screenshot or record a video`) -- matches.

### Two pieces of tooling that unblock the rest

1. **`PORATAKE_CONFIG_DIR`** overrides the profile directory. Two reasons it
   matters: a comparison can run against a clean profile instead of whatever the
   developer happens to have configured, and it does not read or write their real
   history, thumbnails or config. The history comparison above ran on an empty
   scratch profile, so no real captures were involved.
2. **`scripts/parity/capture-window.ps1`** captures a process's own windows with
   `PrintWindow(PW_RENDERFULLCONTENT)`. This was the blocker for everything that
   is not an ordinary app window: the popovers and overlays are created _without
   a title_, and window-enumeration tools that list "targetable" windows skip
   them entirely. It is also immune to occlusion -- the first attempt used
   `CopyFromScreen` and captured a fullscreen game that happened to be on top
   instead of the popup behind it.

The preload stub also had to grow a small event bus: the history window sends
`history:ready` and sits on `Loading...` until `history:refresh` comes back, so
`on` has to record listeners and `send` has to answer.

### Coverage

Compared side by side: **all nine settings categories, onboarding, and the
history window.**

Remaining: the image editor, the video editor, and the capture overlays. All
three are now reachable -- the editor takes a file path on both sides, and
`capture-window.ps1` handles the untitled overlay windows -- so what is left is
running the comparison, not building a way to do it.

Still open by decision, not oversight: Electron's About page has a
`Check for updates` row wired to `update:check`. The GPUI shell has no updater
subsystem at all, so that is a feature to build rather than a difference to
correct.

## Final state (updated)

`cargo fmt --check`, `cargo clippy --all-targets -- -D clippy::disallowed_methods`,
`cargo test` (419 pass) and `cargo build` are clean, with the crate's
pre-existing 10 dead-code warnings unchanged.

## Seventeenth pass — the image editor

### One defect, and it is a behaviour rather than a style

**The editor never fitted the capture to the window.** `calculateOptimalZoom`
runs in `screenshot-window.tsx` on the first frame, on every window resize, and
on every wallpaper-sheet toggle. This shell had no equivalent at all -- no fit
calculation, no `MAX_FIT_ZOOM`, no resize handler. It opened at 100%, so a
capture larger than the window was clipped on the right and bottom with nothing
to indicate that more of it existed.

`editor::zoom_fit::optimal_zoom` is the same arithmetic: 80px of viewport
padding, 40px for the toolbar, 320px more while the wallpaper sheet is open,
`round(min(zoomX, zoomY) * 100) / 100`, clamped to `[MIN_ZOOM, 2.0]`. Applied on
the first frame and whenever the viewport or the sheet state changes -- which
means a manual zoom is discarded on resize, deliberately, because that is what
Electron does.

Verified both ways: five unit tests on the arithmetic (including the exact 0.77
the reference produces at 1280x738 for a 1200x800 capture), plus a headless test
that opens a real 2400x1600 capture and asserts the zoom came out below 1.0 --
without which the maths could keep passing while the hook silently stopped
firing. Side by side afterwards: Electron 77%, GPUI 76%, the difference being
731 vs 738 pixels of client height.

Everything else matched on the first look: the tool row, the separators, the
action row, the colour swatch with its chevron, the centred canvas with its
margins, and the zoom control bottom-right -- whose new `backdrop-blur-md` reads
correctly over the capture.

### Notes on making the comparison possible

- The editor loads its capture as base64 over IPC (`screenshot:read-file`), not
  from the `imageUrl` param, so the stub fetches whatever `?image=` points at and
  encodes it.
- `/@fs/<abs path>` did _not_ work even with `server.fs.allow` widened -- Vite
  answered 200 with `index.html`, so the page received HTML where it expected a
  PNG. Serving the capture from `public/` is what worked. (`fs.allow` is left in
  the config; it is harmless and the next person will try the same thing.)
- Comparing fit-to-window needs both windows the same size. `SetWindowPos` with
  `SWP_NOZORDER | SWP_NOACTIVATE` resizes without raising or focusing anything.
  That is a window-management call, not synthetic input.

### A trap worth recording

`cargo fmt` after a successful build makes the binary stale without making the
build fail, and the app then behaves like the change was never made. The fit
looked broken for two rounds of screenshots for exactly that reason. Check
`LastWriteTime` on the exe against the source before concluding a change had no
effect.

### Coverage

Compared side by side: **all nine settings categories, onboarding, history, and
the image editor.**

Remaining: the video editor and the capture overlays. Both need one more piece of
plumbing -- the video editor takes a project on both sides, and the overlays only
appear during a live capture.

Still open by decision: the `Check for updates` row. The GPUI shell has no
updater subsystem, so that is a feature to build rather than a difference to fix.

## Final state (updated)

`cargo fmt --check`, `cargo clippy --all-targets -- -D clippy::disallowed_methods`,
`cargo test` (425 pass) and `cargo build` are clean, with the crate's
pre-existing 10 dead-code warnings unchanged.

## Eighteenth pass — the video editor

### One defect, and it was not subtle

**The timeline had no length.** `state.source_duration` was only ever read from a
saved project; nothing took it from the decoded media. So opening a recording
that had never been saved as a project left the total duration at zero: the
player read `0:00 / 0:00`, the ruler had no ticks, and the clip track was a stub
a few pixels wide. Electron's `handleBootstrapMetadata` takes the duration off
the `<video>` element's `loadedmetadata` event; there was no equivalent here.

The decoder already knew -- `VideoInfo::duration` is populated when the source
opens -- so the fix is to adopt it, and only when the project does not already
carry one, because a saved project's value is authoritative (trims and speed are
expressed against it). Afterwards: `0:00 / 0:03`, a ruler labelled 0:00 to 0:03,
and a clip spanning the timeline.

Everything else in that window read correctly against the Electron source: the
title bar with the file name and its action row, the preview, the player bar
(play, cut, timecode, zoom slider, fit, `Scrub Audio`, help), the timeline, the
vertical tab rail, and the `Cursor Data` panel's empty state with its two buttons
and format card.

### What could not be compared, and why

The Electron video editor never leaves `Loading recording...` under the browser
rig. It waits on `loadedmetadata` from a `<video>`, and the element is torn down
and recreated before the load completes -- the media request is cancelled after
127 bytes on every attempt. Getting past it means standing up the main process's
video probing, project IO and four sidecar loaders, which is a reimplementation
of the main process rather than a comparison of two UIs.

So this window was compared the way the earlier passes worked -- GPUI's rendered
output read against the Electron source -- rather than screenshot against
screenshot. The duration defect was visible in GPUI's own output regardless: a
three-second clip does not last `0:00`.

Worth recording for a future attempt: a usable test clip can be produced with the
repository's own ffmpeg, but only via `h264_mf`; the bundled build has no
`libx264`, so `-preset` is rejected outright.

### Coverage

Compared side by side: **all nine settings categories, onboarding, history, and
the image editor.** Compared against the source, with its output inspected: **the
video editor.**

Remaining: the capture overlays. They exist only during a live capture, which
means running a real capture on the developer's desktop -- the one thing this
work has deliberately not done since the synthetic-input request was declined.
`scripts/parity/capture-window.ps1` handles their untitled windows, so the
blocker is the capture itself, not the screenshot.

Still open by decision: the `Check for updates` row, which needs an updater
subsystem this shell does not have.

## Final state (updated)

`cargo fmt --check`, `cargo clippy --all-targets -- -D clippy::disallowed_methods`,
`cargo test` (427 pass) and `cargo build` are clean, with the crate's
pre-existing 10 dead-code warnings unchanged.

## Nineteenth pass — the area overlay and the update row

### The overlay: one defect

**Six invented prompts.** `area-overlay-window.tsx` hard-codes one line for
dragging -- `Drag to select an area · Esc to cancel` -- whatever the capture is
for, and names the key that cancels. This shell had a different string per
intent ("Drag to capture an area", "Drag over the text to recognize", and four
more), none of which appear anywhere in the reference, and none of which
mentioned Esc. The window-pick branch also showed the drag prompt instead of
`Click a window to select it · Esc to cancel`.

Both now use the reference's strings, and a test reads the three reference
sources and fails if a prompt this shell can show is not a string one of them
uses. The pill around it was already right -- `rounded-full bg-black/70 px-4 py-2
text-sm shadow-lg` -- and only looked absent in the capture because a
`PrintWindow` of a transparent window composites over black, which hides a 70%
black chip on a black scrim.

Reaching the overlay at all needed `--preview-overlay`, which opens it in a small
unfocused window: no frozen screen, no full-screen cover, no input grab. Without
that the overlay exists only during a live capture that takes over the display.

### The update row: built, check-only

The About page was missing `renderUpdateSection` entirely. It is now there, with
the reference's states, strings, icons and button rule: the status line with its
icon, `Check` for the resting states, a card naming the new version when there is
one, and the error text in red.

What is implemented is the **check** -- the same GitHub releases feed
`electron-updater` is pointed at, with a test asserting the owner and repository
still match `src/types/product.ts`. Downloading and installing are **not**. That
needs a signed artifact and an installer handoff, and a button that pretended to
do it would be worse than no button, so `Available` offers the release page
instead of Electron's `Install Update`. That is a deliberate, documented
divergence, not an oversight.

Both guards earned their keep here: the first version read `cx.entity().read(cx)`
inside the About render, and `ui::lints` plus the settings smoke test failed
immediately -- the same panic they were written for after it shipped twice.

### The stale-binary trap, again

Recorded last pass, hit again this pass: `cargo fmt` after a build leaves the
binary older than the source, the build does not fail, and the app behaves as
though the change was never made. The update row looked like it had not rendered
for one whole round of screenshots. Compare `LastWriteTime` on the exe against
the source before believing a screenshot.

### Coverage

Side by side: **all nine settings categories, onboarding, history, the image
editor, and the area overlay.** Against the source with its output inspected:
**the video editor.**

Not compared this session: the all-in-one toolbar variant of the overlay, the
capture preview, the toast, the pin window, and the recording control. Earlier
passes covered them by reading the CSS, which is exactly the method that missed
two layout bugs and a missing fit-to-window -- so they should be treated as
unverified, not as done.

Still not implemented: downloading and installing an update.

## Final state (updated)

`cargo fmt --check`, `cargo clippy --all-targets -- -D clippy::disallowed_methods`,
`cargo test` (433 pass) and `cargo build` are clean, with the crate's
pre-existing 10 dead-code warnings unchanged.

## Twentieth pass — the last five surfaces

`--preview-window <capture-preview|pin|toast|recording-control>` and
`--preview-overlay all-in-one` make every remaining transient window reachable
without driving a capture or a recording.

### Two defects in the recording control

1. **The bar was wider than its own window.** `ToolbarSurface` is one component
   in Electron -- `gap-0.5 p-1 rounded-4xl border-2` -- and the all-in-one
   toolbar here already used the shared `OVERLAY_SURFACE_*` metrics. The
   recording bar had bespoke constants of its own: 12px gap and 16px padding
   instead of 2px and 4px. Its contents overflowed the fixed 236px window, so the
   record dot and the close button were clipped off both ends. The bespoke
   constants are gone and both bars now take the same metrics -- which is what
   the reference does, because it is literally the same component.
2. **The system-audio button painted a selected chip.** `.selected(true)` on a
   ghost button promotes it to `Secondary`, which fills the background.
   `ToolbarButton` is a plain ghost with `aria-pressed` and no pressed styling:
   the `Volume2`/`VolumeX` swap is the entire signal. A test now forbids any
   `.selected(` call in that file.

Also corrected: the record and stop glyphs are `size-3.5 fill-current` -- a
_filled_ disc and square. The lucide icons here are stroke-only, so an outline
circle was simply the wrong shape; a 14px filled div is what `fill-current`
draws.

### Three that matched

- **Capture preview**: `rounded-lg overflow-hidden` with the image filling the
  window, no chrome at rest.
- **All-in-one toolbar**: camera, video, target dropdown, separator, OCR,
  eyedropper, separator, close -- same order, same selected chip on the active
  mode, same prompt beneath.
- **Toast**: reached and drawn; Electron has no toast window type, so this is a
  GPUI-only surface with nothing to compare against.

### Two near-misses worth recording

Both were the harness lying, not the app:

- The recording bar looked like it was _missing_ its close button, because the
  capture sized its bitmap to the DWM frame bounds while `PrintWindow` draws the
  client area at the origin. `capture-window.ps1` now measures `GetClientRect`.
  The clipping turned out to be real as well -- but for a different reason, and
  the harness bug hid the actual cause for two rounds.
- The all-in-one toolbar looked like it had two buttons Electron lacks. The stub
  was sending a wrong-shaped `toolbar` object; `AreaOverlayToolbar` has
  `recordingEnabled` and `ocrEnabled`, and with the real shape the two toolbars
  are identical. Checking before "fixing" saved deleting two correct buttons.

### Coverage

Compared side by side: **all nine settings categories, onboarding, history, the
image editor, the area overlay, the all-in-one toolbar, the capture preview, and
the recording control.** Against the source with its output inspected: **the
video editor**, which the browser rig cannot bootstrap.

The one thing still not implemented is **downloading and installing an update**.
The row, its states and the check are all there and match; the install path needs
a signed artifact and an installer handoff, which is a feature to build and not a
difference to correct.

## Final state (updated)

`cargo fmt --check`, `cargo clippy --all-targets -- -D clippy::disallowed_methods`,
`cargo test` (434 pass) and `cargo build` are clean, with the crate's
pre-existing 10 dead-code warnings unchanged.

## Twenty-first pass — the update install path, and the video editor side by side

### Update: download and install, verified

The row now goes the whole way. `autoDownload` is off in the reference, so the
check resolves the artifact and stops; `Download Update` fetches it and
`Install Update` hands over.

- The installer is the `-win-<arch>.exe` from `artifactName` in
  `electron-builder.json5` with `target: nsis`.
- Its digest comes from the `latest.yml` published in the same release, which is
  the manifest `electron-updater` verifies against. **A download whose base64
  sha512 does not match is discarded and never written where it could be run.**
  That is the condition on which this was worth building at all.
- `Downloading` reports progress into the same shared cell the UI reads, so the
  progress bar is the real byte count.
- Installing spawns the installer and quits, because NSIS cannot replace a
  running binary.

Unit tests cover artifact selection (including a release with no artifact for
this platform), `latest.yml` parsing, and the digest — with a known base64
sha512 vector so the hash itself is pinned. The network path is not unit tested.

### The video editor, finally side by side

The bootstrap stalled on one stubbed channel: `video-editor:getState` returned
`{}`, so `loadedState.segments.filter(...)` threw on an object with no
`segments`. Returning `null` -- "no saved project" -- lets the whole window
render. Worth recording how that was found: a two-line page proving the browser
decodes the clip (1280x720, 3s) ruled out the codec, which is where the previous
pass had wrongly stopped.

Two defects, both visible immediately once both windows were on screen:

1. **The timeline had no clip.** `initializeDocument({ segments: defaultSegments })`
   seeds one segment spanning the whole recording when there is no saved project.
   This shell left `segments` empty, so the ruler was drawn over an empty lane.
   `Segment::spanning` is that seed, applied when the duration is adopted.
2. **The title kept its file extension.** `getFileNameFromPath` uses the parent
   directory when it is a `.poratake` project, and otherwise strips everything
   after the last dot -- with `lastDot > 0`, so a leading dot stays. This shell
   stripped only the project extension, so a recording read `parity-clip.mp4`
   instead of `parity-clip`.

A pre-existing test asserted the second bug as if it were the contract
(`clip.mp4` -> `clip.mp4`). It encoded the defect rather than the intent, and is
now corrected with a note saying so.

### Still different, and known

The Electron timeline shows three lanes with gutter icons -- clip, zoom, drawing
-- whether or not they hold anything. This shell renders a lane only when it has
content, so a fresh recording shows one. The clip's own label is centred there
and left-aligned here, and the gutter glyph is a film reel rather than a camera.
These are named here rather than fixed because the pass had already run long, not
because they are acceptable.

## Final state (updated)

`cargo fmt --check`, `cargo clippy --all-targets -- -D clippy::disallowed_methods`,
`cargo test` (441 pass) and `cargo build` are clean, with the crate's
pre-existing 10 dead-code warnings unchanged.

## Twenty-second pass — the three named timeline differences

All three are closed.

1. **The zoom and drawing lanes now always render.** `<ZoomTrack>` is
   unconditional in `video-editor-window.tsx`, and an empty drawing lane falls
   back to a bare `<TrackRow />` -- so both are visible whether or not they hold
   anything. This shell only built a lane that had content, which is why a fresh
   recording showed one lane where the reference shows three.
2. **The clip label is centred, `text-sm`, `font-medium`.** `renderLabel` sits in
   an `absolute inset-0 flex items-center justify-center` box; this shell had a
   left-aligned 10px label with no weight. It also collapses to a `Film` glyph
   once the clip is narrower than 100px, which had no equivalent here -- now
   `clip_is_narrow`, with the threshold pinned by test at exactly 100px.
3. **The clip lane's gutter glyph is `Film`,** not a video camera.

Side by side afterwards: ruler, three lanes with film / magnifier / pencil
gutters, and the amber clip carrying a centred `3s`.

## Where this leaves things

Compared side by side, Electron against GPUI, with both windows on screen:

- All nine settings categories
- Onboarding
- History
- The image editor
- The video editor
- The area overlay, and its all-in-one toolbar variant
- The capture preview
- The recording control

Reached and drawn, with nothing to compare against: the toast, which Electron has
no window type for.

Implemented rather than compared, because the surface did not exist here at all:
the About page's update row, including the check, the verified download and the
install handoff.

Nothing on the list is knowingly left different.

## Final state

`cargo fmt --check`, `cargo clippy --all-targets -- -D clippy::disallowed_methods`,
`cargo test` (443 pass) and `cargo build` are clean, with the crate's
pre-existing 10 dead-code warnings unchanged.

The tooling this relied on is committed under `scripts/parity/`:
`vite.renderer.mts` serves the Electron renderer without Electron,
`preload-stub.js` stands in for the preload bridge, and `capture-window.ps1`
captures a process's own windows -- including the untitled popovers and overlays
that ordinary window enumeration skips. Between them and the `--intent`,
`--preview-window` and `--preview-overlay` CLI routes, every window in either
shell can be put on screen and photographed without driving a capture.

## Pass 23 — the blind spot in the method

Twenty-two passes compared windows. That method cannot see a difference that
has no window, and there was one.

`Toast::show` draws a 320x72 borderless `WindowKind::PopUp` in the corner of the
display. Its Electron counterpart is not a window at all:
`src/main/utils/notification.ts` calls `new Notification({title, body}).show()`,
so Windows draws it. Six Electron call sites (all-in-one, OCR, QR, video export,
cloud, deletion notices) against 27 in the GPUI shell, all funnelling through
`Toast::show`.

The consequences are behavioural, not cosmetic. An app-drawn card does not
appear in Action Center, does not respect Focus Assist or Do Not Disturb, does
not honour the per-app notification settings, and cannot be silenced the way
`showTransientNotification`'s `silent: true` silences Electron's. It also lived
4 seconds against Electron's `TRANSIENT_NOTIFICATION_DURATION_MS = 5_000`.

Two things fell out of looking properly.

The styling comment in `toast.rs` cited `toast.css` as `min(32px,
var(--radius-3xl))` on `bg-overlay`. **There is no `toast.css` in this
repository.** A fabricated citation, the same class of defect as the six
invented overlay prompts in pass 17 — it reads as authority and is worth
nothing.

The AppUserModelID is `electron.app.Poratake`, **not** the electron-builder
`appId` `com.porabuild.poratake`. electron-builder's NSIS target writes the
shortcut's `System.AppUserModel.ID` as `electron.app.<productName>`. This
matters more than it looks: `CreateToastNotifier` with the wrong AUMID returns
success and then silently never shows anything. Reading the config would have
produced exactly that bug. Verified against the installed shortcut, and
`CreateToastNotifier("electron.app.Poratake").Setting` returns `Enabled`.
(WinRT projections do not load in PowerShell 7 — probe with `powershell.exe`.)

### What this changes about the goal

"Every window matches" was the wrong finish line. The right one is that every
place Electron hands off to the OS, the GPUI shell hands off the same way —
notifications, tray, native dialogs, clipboard, shell integration, taskbar
state. Those are invisible to screenshot comparison by construction. A survey
of them is underway; the actionable ones will be listed here as they are
confirmed.

### The OS-handoff survey

Checked each mechanism on both sides rather than assuming. Most of the shell
turned out to be fine, which is worth stating as plainly as the gaps:

**Already native, no action.** File dialogs and message boxes (`rfd`),
clipboard (`arboard`), open-external and reveal-in-explorer
(`system/desktop.rs`), the tray and its context menu (`system/tray/`, against
Electron's `menu/index.ts`), the single-instance lock
(`system/single_instance.rs` against `app.requestSingleInstanceLock`), global
hotkeys (`global-hotkey`), and the OS accent colour (`system/accent.rs`).

Deletion is also _not_ a gap, though it looked like one: Electron never calls
`shell.trashItem` either — its own notification says "permanently deleted", and
both shells `unlink`. Worth writing down because "Electron surely uses the
Recycle Bin" is exactly the plausible assumption that would have produced a
wrong "fix".

**Gaps, in order of how much they cost the user:**

1. _Notifications_ — drawn, not native. Covered above; in progress.

2. _`general.startOnLogin` does nothing._ The field is read and written
   (`config/shortcuts.rs:15`, defaulting to **true**) and rendered as a toggle
   (`windows/settings/registry.rs:331`), but nothing in the crate registers with
   Windows — no `Run` key, no startup shortcut, no scheduled task. Electron
   calls `app.setLoginItemSettings({openAtLogin})` on change
   (`settings/store.ts:249`) and again at boot via `applyLoginItemSetting()`
   (`:299`). So the GPUI shell ships a switch that is on by default, that the
   user can toggle, and that has no effect whatsoever.

3. _The camera preview is recorded into the video._ Content protection lives in
   the shared daemon (`daemon-win/src/modules/camera_preview.rs:389`,
   `SetWindowDisplayAffinity` with `WDA_EXCLUDEFROMCAPTURE`), and its
   `content_protected` state defaults to `false` (`:61`, `:227`). The `show`
   handler does not accept it as a parameter — only the `setContentProtection`
   command changes it. Electron enables it when recording starts
   (`recording-actions.ts:507`, `:641`, `recording-control.ts:303`) and disables
   it on stop (`:146`, `:166`, `:368`, `:612`). The GPUI shell never calls it,
   so the floating camera window is captured into the recording. This is a
   functional defect rather than a cosmetic one, and no screenshot comparison
   could ever have found it.

4. _OS light/dark changes are not followed live._ Electron subscribes with
   `nativeTheme.on('updated')` (`capture/video/overlay.ts:50`, `menu/index.ts`).
   The GPUI shell reads `AppsUseLightTheme` once (`theme/presets.rs:27`), so
   switching the system theme leaves it stale until restart.

5. _Media-permission status is never checked._ Electron has
   `system/permissions.ts` querying `getMediaAccessStatus` for screen,
   microphone and camera. The GPUI shell has no equivalent, so a
   privacy-blocked device presents as a failure rather than as a permission
   prompt.

## Pass 24 — running the capture actions instead of reading them

Everything below came from launching the built binary against a scratch profile
(`PORATAKE_CONFIG_DIR`), driving it, and measuring. Two of the three capture
methods I tried lie, in opposite directions, and it is worth writing down which:

- **`CopyFromScreen` (screen BitBlt) cannot see the overlay at all.** gpui
  renders through DirectComposition, and a screen BitBlt of a full-screen
  DirectComposition surface came back as the plain desktop. Judging by these
  captures alone, `capture-area` looked completely broken — no overlay, no
  dimming, no crosshair.
- **`PrintWindow` sees the overlay's vector content but not its image layers.**
  The prompt text and the crosshair rendered; the frozen-desktop background came
  back pure black. Judging by _these_, the freeze pipeline looked broken.

Neither was true. `WindowFromPoint` at the screen centre returned the overlay's
own `HWND`, owned by the gpui process — it was topmost and hit-testable the
whole time. The decisive check for "is this window really there" is hit-testing,
not a screenshot. Recording this because the screenshots were convincing and
both wrong, and because acting on either would have meant "fixing" working code.

A third false alarm, for completeness: the overlay kept vanishing between
observations. That was me — each PowerShell step began with
`Stop-Process -Name poratake-gpui`, so I was killing the instance I had just
launched. There is no timeout in `capture/overlay.rs`; `dismiss` only runs on
Esc or a completed capture.

### What actually works

`capture-screen` saves `Screenshot 2026-08-23 at 19.06.52.png`, honouring the
`%type %Y-%m-%d at %H.%M.%S` naming pattern. `capture-area` works end to end:
overlay opens with "Drag to select an area · Esc to cancel" and an accent
crosshair, a drag from (900,450) to (1500,850) produced a capture measuring
**exactly 600x400**, the overlay dismissed itself, and the preview appeared.
`TOPMOST` is not set on the overlay, which matches — Electron's area overlay
does not set `alwaysOnTop` either.

### The preview is positioned a taskbar too low

Measured with `GetWindowRect`: **(3216, 1276)**. Electron's `getPreviewPosition`
gives **(3216, 1228)** on this display. x is exact; y is out by 48, the height of
the taskbar, so the preview sits underneath it.

The cause is that `getPreviewPosition` anchors to `display.workArea` while
`selected_display_bounds` returned `display.bounds()`. This was not laziness:
gpui 0.2.2's `PlatformDisplay` exposes `id`, `uuid`, `bounds` and
`default_bounds` and **has no work-area concept at all** — there is no
`rcWork` anywhere in the crate. The only way to get it is to ask Windows.

Fixed by adding `system/work_area.rs`, which takes the difference between
`MONITORINFO.rcMonitor` and `rcWork` and applies it to the gpui bounds. The
inset is scaled by the monitor-to-logical ratio rather than subtracted directly,
because Windows reports physical pixels and gpui reports logical ones; at 100%
scaling the ratio is 1 and it reduces to a subtraction. Degenerate rectangles
fall back to the full bounds, since a bad position is better than an impossible
one. Six unit tests cover a bottom taskbar, a left-docked one (which moves the
origin, not just the size), a secondary display at a non-zero origin, the 150%
scaling case, the degenerate case, and no taskbar at all.

### Preview hover chrome

Geometry is right. The close button is at inset 8 top-left, delete at inset 8
top-right, Copy bottom-left, Upload bottom-right, and the centre pill is
`px 12 / py 4 / 12px / MEDIUM` against Electron's `px-3 py-1 text-xs
font-medium`. The scrim is `hsla(0, 0, 0, 0.25)`, matching `bg-black/25`. The
hover scale enlarges the image inside a centred `flex items_center
justify_center overflow_hidden` box, which is a correct stand-in for CSS
`scale-105` about the centre — gpui has no div transforms.

Four differences remain, all confirmed in code:

1. _The scrim has no blur._ Electron's is
   `bg-black/25 backdrop-blur-md animate-in fade-in duration-200`. This shell
   draws the tint and nothing else. The blur is solvable the way the zoom bar
   solved it (`editor/zoom_backdrop.rs`), and the fade is not implemented at all.
2. _The hover scale snaps._ `PREVIEW_HOVER_MS = 200` exists in `ui/chrome.rs`
   and **two tests assert it equals 200**, but nothing in the crate reads it and
   `capture_preview.rs` contains no animation. The constant and its tests give
   the appearance of covering a feature that was never built.
3. _Destructive buttons do not turn red._ Electron uses `hover:bg-destructive`
   on close and delete and `hover:bg-primary` on the rest — 3 and 7 occurrences.
   `circle_button` hovers to `theme.accent` for every one of them, so deleting a
   capture is styled identically to copying it.
4. _`theme.accent` is the wrong colour for a `primary` hover._ `accent` comes
   from `variant.accent`, a theme preset; `primary` is bound to the OS accent
   (`#680081` here). Electron's `hover:bg-primary` follows Windows, this follows
   the theme.

Also missing: the **Polish** pill. Electron stacks it above Edit
(`flex flex-col gap-1`) for screenshots whenever a polish preset is configured;
there is no equivalent in `capture_preview.rs`.

### Pass 24, continued — what was fixed

**Preview position.** `system/work_area.rs` added; `selected_display_bounds`
now insets the gpui display bounds by `MONITORINFO.rcWork`. Verified on screen
with `GetWindowRect`: the preview moved from (3216, 1276) to **(3216, 1228)**,
which is what Electron's `getPreviewPosition` computes.

**Hover colours.** `circle_button` now takes the hover fill as an argument:
`theme.destructive` for close and delete, `theme.primary` for copy, upload and
the display picker, and the Edit pill moved from `theme.accent` to
`theme.primary`. Before this every control hovered to the same colour, so a
delete was styled exactly like a copy. `theme.primary` is the right token
because Electron's `--primary` is overwritten with the OS accent, which
`theme.accent` (a theme preset value) is not.

The regression test for this needed two attempts. Counting occurrences of
`"theme.destructive,"` with `str::matches` found **three** — the third being the
string literal inside the test itself. Counting whole trimmed lines instead
cannot match its own literal. This is the third time in this audit that a
source-reading test has matched its own text.

**The hover scrim's blur.** `bg-black/25 backdrop-blur-md` was drawing the tint
and nothing else. gpui cannot blur a region, so the blurred thumbnail is baked
once at construction and swapped in while the controls show. The scaling has to
happen _before_ the blur: `backdrop-blur-md` blurs the rendered pixels, so
blurring a 3440x1440 capture and then drawing it at 200x140 comes out almost
sharp. Reuses `render::blur::blur`, the same implementation the zoom bar uses,
rather than introducing a second one.

Measured rather than eyeballed, since "looks dark" is not evidence: mean
absolute horizontal gradient across the preview is **18.04 un-hovered against
0.64 hovered — 3.5% of the detail retained**.

**The 200ms transition.** `PREVIEW_HOVER_MS` existed, was marked
`#[allow(dead_code)]`, was asserted equal to 200 by two tests, and was read by
nothing. The scale snapped and the scrim appeared at full strength. Now
`advance_hover` steps a `hover_progress` towards its target using real elapsed
time and asks for another frame while it is moving, so `scale-105` eases in over
200ms and eases back out — `transition-transform` animates both directions —
and the scrim's alpha is `0.25 * progress`, which is the `animate-in fade-in`
half. The `dead_code` allow is gone because the constant is finally load-bearing.

Residual, stated rather than hidden: the _blur_ still swaps in at full strength
instead of fading with the tint. A true cross-fade needs both thumbnails stacked
with animated opacity. The tint and the scale carry the transition; the blur
appears at once.

### The Polish pill

`usePolishCopy` reads `wallpaper.defaultPresetId`, finds that preset, and shows a
`Polish` pill above `Edit` in the centre stack; clicking it composes the capture
over the preset's wallpaper and copies the result. There was no equivalent here.

What made this more than a missing button: `editor/wallpaper_sheet.rs` already
tells the user _"Polish on the capture preview copies with \"{}\""_ and _"Star a
preset to enable Polish on the capture preview."_ The shell shipped the starring
mechanism and advertised the feature in its own settings copy, while the preview
never rendered the control. The product promised something it did not do.

Implemented with the pieces that already existed rather than new machinery:
`wallpaper::apply_preset` onto a default `WallpaperSettings`, `wallpaper::layout`
for the canvas size, `export::compose` for the composition, and `arboard` for the
clipboard — the same route `copy_image` takes.

Worth noting for anyone measuring parity here: `defaultPresetId` defaults to
`None`, and Electron renders no Polish pill in that state either. So the default
hovered preview was _already_ identical on this point; the difference only became
visible once a preset was starred, which is exactly the kind of conditional
control a default-configuration screenshot comparison cannot find.

Verified by starring a preset in a scratch profile: the pill renders above Edit
in a `flex-col gap-1` stack, both pills `bg-background/80`, and its tooltip reads
`Copy with "Sunset"`, matching Electron's
``title={`Copy with "${polishPreset.name}"`}``.

### Every capture action, exercised

Run against a scratch profile, one at a time, checking what each actually opens:

| intent           | opens               | result                                           |
| ---------------- | ------------------- | ------------------------------------------------ |
| `capture-screen` | none (immediate)    | file saved, naming pattern honoured              |
| `capture-area`   | full-screen overlay | drag -> **600x400** and **700x450** files, exact |
| `capture-window` | full-screen overlay | opens, window-pick prompt                        |
| `timer-capture`  | full-screen overlay | correct: Electron also selects an area first     |
| `capture-text`   | full-screen overlay | opens                                            |
| `scan-qr-code`   | full-screen overlay | opens                                            |
| `all-in-one`     | overlay + toolbar   | toolbar top-centre, buttons and order match      |
| `scroll-capture` | full-screen overlay | opens                                            |
| `record-screen`  | recording control   | 252x61, horizontally centred, near the top       |
| `record-area`    | full-screen overlay | opens                                            |
| `record-window`  | full-screen overlay | opens                                            |

No stderr from any of them.

### Gates

`cargo fmt --check` clean. Clippy gate (`-D clippy::disallowed_methods`) 0 errors.
`cargo test` **452 passed, 0 failed** (443 before this pass; +6 for the work
area, +2 for the hover transition, +1 for the hover colours). `cargo build`
emits the same **10** pre-existing dead-code warnings, none of them new.

## Pass 25 — the OS-handoff gaps, and a spec error of my own

Two of the five gaps from the survey are closed. Both were delegated to
`droid exec` with `deepseek-v4-flash-0731`; the four earlier `opencode`
delegations all ended in a 50-minute timeout having written nothing, so nothing
in passes 23-24 came from them.

### The camera preview no longer lands in the recording

`video/recorder.rs` now calls `camera-preview setContentProtection` — enabled
after a recording starts when the camera is on, on `set_camera(true)`, and
disabled on `stop` and `set_camera(false)`. That mirrors
`recording-actions.ts`, which enables at `:507`/`:641` and disables at `:146`,
`:166`, `:368`, `:612`.

The executor found something the prompt did not mention: `daemon_contract.rs`
asserts every `daemon.call` in the crate appears in `DAEMON_METHODS` from
`src/types/daemon.ts`. It checked before writing and confirmed `'camera-preview'`
already lists `setContentProtection`, so the contract test still passes. That is
a trap the prompt would have walked into.

Its regression test is also better than the one I wrote earlier today: it splits
the file on `#[cfg(test)]` and searches only the production half, which makes
matching its own literals structurally impossible rather than merely unlikely.

### The toast is now the OS notification

`windows/toast.rs` is a 19-line forwarding shim. `Toast::show` keeps its exact
signature — all 27 call sites are untouched — and forwards to
`system/notification.rs`, which delivers through the WinRT
`ToastNotificationManager` under AUMID `electron.app.Poratake`. The `Render`
impl, `WindowKind::PopUp`, `corner_bounds`, `LIFETIME` and the fabricated
`toast.css` citation are all gone.

**A bug I introduced through a bad specification.** I told the executor to set
the toast's `ExpirationTime` five seconds out, "mirroring notification.ts's
5000 ms timeout". Those are not the same mechanism. Electron's
`showTransientNotification` closes the toast from a JS `setTimeout`;
`showNotification` sets no expiry at all. A WinRT `ExpirationTime` that near
makes Windows **discard the toast outright while `Show` still returns `Ok`** —
no error, no stderr, nothing delivered. The executor implemented my spec
faithfully and the result was a notification subsystem that passed every test
and never notified anybody.

It surfaced only because delivery was checked rather than assumed:
`%LOCALAPPDATA%\Microsoft\Windows\Notifications\wpndatabase.db-wal` moves when a
toast is actually delivered. With the expiry set it never moved. A PowerShell
reference path using the same AUMID and XML moved it immediately, which isolated
the difference to `SetExpirationTime`; removing that one call made the Rust path
deliver on every attempt.

A confound worth recording, because it nearly sent the diagnosis the wrong way:
a `#[test]` calling `show()` exits within milliseconds, and toast delivery goes
out over COM RPC, so "the test process died too fast" was an equally plausible
explanation. It was ruled out by the same short-lived test delivering
successfully once the expiry was removed.

`Toast::show` therefore mirrors `showNotification`: Windows dismisses the banner
itself and the entry remains in the Action Center, which is what Electron's
non-transient notification does. The transient variant's 5-second close is not
mirrored, and the file says so.

`PropertyValue`, `IReference`, `DateTime`, the `expiration()` helper, the
epoch-offset constant and the 5000 ms constant all went with it, along with the
cross-source test that pinned Electron's transient duration — it no longer
guarded anything this code depends on.

Also worth keeping: the executor corrected _my_ prompt on the `windows` crate
features. I listed `UI_Notifications` and `Data_Xml_Dom`; `SetExpirationTime`
needs `windows::Foundation`, which is separately feature-gated and pulled in by
neither. It added `Foundation` and explained why. That feature is still required
for nothing now that the expiry is gone, and is a candidate for removal.

## Pass 26 — login item and live theme, via Crossagents

Both delegated through Crossagents to `factory` / `deepseek-v4-flash-0731`, with
disjoint file ownership (`system/` versus `theme/`) so two agents could run at
once. Both handled the concurrency correctly: each noticed `main.rs` had changed
underneath it, re-read the file, and re-applied its own insertion.

A note on spawning: the first attempt failed with "Provider or requested
selection is not currently available: factory". The provider was fine; the
_selection_ was not. I had asked for reasoning `max`, which the `droid` CLI
accepts as a flag but which is not among the values Crossagents lists for that
model (its preferred is `high`). The error names both possibilities and the
second one was the real cause.

### `general.startOnLogin` now does something

`system/startup.rs` writes and removes
`HKCU\Software\Microsoft\Windows\CurrentVersion\Run`, applied at launch
(mirroring Electron's `applyLoginItemSetting()`) and the moment the toggle flips
(mirroring `updateConfig`).

**The value name was wrong and only the machine could say so.** The executor
chose `Poratake`, and its module comment asserted that this name was "the
contract between the two shells". Reading the actual registry showed the
installed Electron app had already written:

    electron.app.Poratake = "C:\Users\sdsle\AppData\Local\Programs\Poratake\Poratake.exe"

Electron defaults the Run value name to the app's AppUserModelID, so it is
`electron.app.<name>` — the same string the toasts are delivered under. A
neighbouring `electron.app.Loom` entry confirms the convention independently.
Under the original name the GPUI shell would have created a _second_ auto-run
entry beside Electron's, and turning the setting off in one shell would have
left the other's entry behind. No unit test could have caught this; it is a
claim about another program's behaviour, and the only evidence is the live
registry.

Two follow-ups of my own:

- `is_open_at_login` was dead code (the eleventh warning). It now has a real
  job: startup compares before writing, so a launch where the entry is already
  correct writes nothing. That restores the warning count to 10 and — unplanned
  — stops a dev build from repointing the user's real auto-start entry at
  `src\main\target\debug\poratake-gpui.exe`, since the shared value name means an
  unconditional write would have clobbered the installed path.
- The `windows` crate's WinRT `Foundation` feature, added only for the toast
  `ExpirationTime` that pass 25 removed, is gone. `Win32_Foundation` and
  `Win32_Media_MediaFoundation` are different features and stay.

Standing hazard worth remembering: the two shells share one Run value name by
design, so toggling the setting inside a dev build writes the debug path over
the installed one.

### OS light/dark is followed live

`theme/presets.rs` no longer shells out to `reg query` to read
`AppsUseLightTheme`; it reads the DWORD directly through `RegOpenKeyExW`, in the
same style as `system/accent.rs`. Spawning a process to read one value on a UI
path was both slow and inconsistent with the accent reader beside it.

`theme/watcher.rs` blocks a background thread on `RegNotifyChangeKeyValue`
against the `Personalize` key and pushes changes over a `smol` channel into the
existing event loop. The refresh goes through `vars::update_theme`, which is
what `SettingsWindow::mutate` already uses, rather than `refresh_shell` — the
executor checked both hints and reported that `refresh_shell` rebuilds the tray
and hotkeys without touching window theming, which is correct.

The decision is a pure `needs_refresh(selected, applied, current)`: it fires
only when the appearance is `System` _and_ the resolved mode actually changed.
That matters because the key also fires for accent-colour and wallpaper writes,
so a naive refresh-on-every-event would repaint constantly, and it guarantees an
explicit light/dark choice is never overridden by the OS.

### Gates

fmt clean, clippy gate 0 errors, **462 tests pass** (456 + 2 login item + 4
theme), and `cargo build` back to exactly the **10** pre-existing dead-code
warnings. The registry entry was unchanged by the test run, confirming the
tests avoid HKCU as specified.
