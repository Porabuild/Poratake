# Integration test index

This indexes the integration test modules under this directory by feature
area. Cargo links them into nine `suite_*` binaries declared in
`crates/herogpui-components/Cargo.toml`; each suite file lists its modules.
The `suite_foundations` integrity test fails if a new `.rs` source is left
out. Unit tests for internals (`#[cfg(test)]` modules) live alongside the
implementation under `src/` instead, and are not listed here.

| Suite | Area |
|---|---|
| `suite_collections` | Collections, navigation and virtual lists |
| `suite_controls` | Buttons, checks and choice controls |
| `suite_feedback` | Feedback, media and chrome |
| `suite_fields` | Fields, forms and text selection |
| `suite_foundations` | Accessibility, focus, locale and theme behavior |
| `suite_overlays` | Menus, dialogs, placement and overlay stack |
| `suite_pickers` | Dates, colors and sliders |
| `suite_styling` | Styling, geometry, rendering and customisation |
| `suite_tables_layout` | Tables, layout, scrolling and desktop extensions |

## Buttons, toggles & choice controls

| Module | Covers | Tests |
|---|---|---|
| `button_geometry` | Button size ladder, box overrides, `grow`, and `font_weight` (style probe). | 8 |
| `buttons` | Button, ButtonGroup, ToggleButton, CloseButton, Link, Chip-close and Alert-close press behavior. | 31 |
| `close_button_deep` | CloseButton geometry, press and hover endpoints against pinned HeroUI v3.2.4 metrics. | 8 |
| `choice_controls_deep` | Deeper keyboard/read-only behaviour for Switch, RadioGroup and ToggleButtonGroup. | 48 |
| `checkbox_form_deep` | Checkbox keyboard, form, validation and focus contracts. | 22 |
| `checkbox_motion` | Checkbox selection motion (fill/scale/fade) contracts. | 8 |

## Text fields & forms

| Module | Covers | Tests |
|---|---|---|
| `text_fields` | TextField, TextArea, SearchField and related text component behaviour. | 62 |
| `fields` | FIELD/FORM group components: checkbox/radio/switch groups, number field, form submit/validation. | 21 |
| `field_keyboard_contracts` | Keyboard/pointer contracts inherited by HeroUI v3's field family (search, date, time, number fields). | 20 |
| `field_slots_deep` | Focused slot tests for `field.rs` composition parts (label, error, fieldset). | 11 |
| `forms_deep` | Form, InputOTP and Fieldset edge cases: validation, server errors, submit/reset, autofocus. | 79 |
| `form_state_lifecycle` | Form reset callbacks must not retain the fields that store them (no leak). | 1 |
| `slider_form_deep` | Slider form integration: live values, disabled controls, reset. | 11 |
| `slider_number_deep` | Deep Slider/NumberField contracts inherited from pinned React Aria/Stately. | 18 |
| `text_size_knobs` | Phase 5 `text_size` knobs: the label draws the size and its paired leading (style probe and line boxes). | 3 |
| `font_seams` | Phase 5 font seams: mono default and `font_family` overrides, read through an inherited-style probe. | 5 |

## Pickers (select/combo/autocomplete/dropdown/list box)

| Module | Covers | Tests |
|---|---|---|
| `pickers` | Behaviour for Autocomplete, ComboBox, Select search-and-select components. | 44 |
| `pickers_deep` | Pickers' remaining props and Drawer-adjacent picker behaviour. | 102 |
| `combo_box_open` | What opens a ComboBox's suggestion list (typing, focus, chevron, arrow, escape). | 14 |
| `combo_box_selection` | Single-selection callback parity with React Stately 3.50.0. | 4 |
| `autocomplete_hover_deep` | Pinned trigger-hover suppression contract around the clear button. | 8 |
| `select_clear_deep` | Select.ClearButton: real pointer and keyboard dispatch. | 5 |
| `dropdown_anatomy_deep` | Dropdown composition metrics, explicit identity, nested overlay behavior, row leading content. | 5 |
| `dropdown_close` | Dropdown's close-on-activate contract, including submenus. | 25 |
| `dropdown_seed_deep` | Dropdown.Menu's controlled/uncontrolled selection seeding. | 12 |
| `dropdown_viewport_deep` | Dropdown menu viewport correction (flip/shift/clamp near edges). | 19 |
| `collections` | ListBox, Tabs, Accordion, Pagination, Breadcrumbs and TagGroup collection behaviour. | 16 |
| `collections_deep` | Deeper collection behaviour: ListBox `onAction`, TagGroup edge cases, row padding. | 7 |
| `collection_contracts` | Collection contracts inherited from pinned React Aria: ListBox/TagGroup/Table selection and keyboard. | 70 |
| `allocation_axes` | Keyed selection axis allocation probe (positive control for allocation tests). | 2 |

## Date & time

| Module | Covers | Tests |
|---|---|---|
| `calendars_and_more` | Calendar/RangeCalendar family plus other un-driven behaviour (Select, ColorPicker, Toolbar, Disclosure). | 61 |
| `calendars_deep` | Edge dates and constraints on the calendar family: bounds, year picker, unavailable dates. | 65 |
| `date_field_form_deep` | Date/time field form submission, reset and disabled/read-only successful-control rules. | 11 |
| `date_field_picker_deep` | Deep inherited contracts for the v3 date field and picker family (segments, validation, value builders). | 45 |
| `date_picker_close` | Date pickers' close-on-select rule. | 10 |
| `date_picker_placement` | Placement contracts for the DatePicker family. | 9 |
| `overlay_stack_date_deep` | Explicit overlay-stack contracts for DatePicker and DateRangePicker. | 8 |

## Colour

| Module | Covers | Tests |
|---|---|---|
| `color_geometry_deep` | Deep pointer-geometry tests for ColorArea and ColorSlider (drag, keyboard, form, fields). | 44 |
| `overlay_stack_color_deep` | ColorPicker overlay-stack behavior against the pinned React Aria contract. | 10 |
| `theme_tokens` | Customisation tokens reach their consumers. | 2 |

## Overlays (modal/drawer/popover/toast/tooltip)

| Module | Covers | Tests |
|---|---|---|
| `overlays` | Modal, Drawer, AlertDialog and related overlay component behaviour. | 35 |
| `overlay_padding` | Overlay panel padding on the resting panel and through the entry/exit zoom (executor clock). | 3 |
| `panel_padding` | Phase 3 overlay panel padding: stock and overridden insets read off the painted panel. | 2 |
| `overlay_stack_dialogs_deep` | Explicit overlay-stack behavior for Modal, Drawer, AlertDialog. | 4 |
| `overlay_stack_menus_deep` | Nested Dropdown and Tooltip overlay-stack contracts. | 3 |
| `overlay_stack_pickers_deep` | Explicit overlay-stack coverage for the three picker surfaces (Select/DatePicker/ColorPicker family). | 10 |
| `popover_stack_deep` | Deep focus-scope and nested-dismissal contracts for Popover. | 15 |
| `sheet_deep` | Sheet state, dismissal, nesting, snap geometry, disabled parts and stable child state through motion, resize, exit safety and snap normalization. | 19 |
| `drawer_deep` | Drawer anatomy: drag-to-dismiss, handle/header/footer, focus trap. | 15 |
| `feedback` | Toast, Alert, Avatar, Badge, Progress, Meter, Spinner, Skeleton behaviour. | 35 |
| `feedback_compose` | Three feedback/composition parity gaps (avatar load, alert composed button, badge default color). | 4 |
| `toast_promise_deep` | `toast.promise`: loading toast updates in place. | 5 |
| `toast_stack_deep` | Toast stack: `isExpanded`, hover/focus expand, timer pause. | 9 |
| `toast_update_deep` | `toast.update`: in-place content replacement. | 5 |
| `placement` | Positional behaviour: surface placement, arrow, collision handling. | 19 |
| `placement_extra` | Tooltip delays and the slider's other axis. | 10 |

## Navigation (tabs/breadcrumbs/pagination/link)

| Module | Covers | Tests |
|---|---|---|
| `nav_deep` | Navigation family keyboard and edge cases (tabs, breadcrumbs, pagination, links). | 44 |
| `tabs_deep` | Deeper Tabs keyboard behaviour (vertical axis, ends) and the paint/toolbar overrides on the painted scene. | 39 |
| `parts_pagination` | Per-part disabling added to Pagination. | 8 |
| `link_deep` | Deep behaviour tests for `Link` against pinned HeroUI v3.2.4 Link contract. | 10 |
| `focus_ring_overlay` | Offset focus rings (Switch, ToggleButton, Checkbox) read through their painted gap band; field rings as scoped source checks. | 6 |
| `focus_root` | `app_focus_root` claims the focus only when nothing holds it: a handle focused before its first paint keeps it, and a released or unmounted focus returns to the root. | 4 |
| `focus_visible_deep` | Focus rings follow keyboard-control modality, not HeroUI's Escape restore. | 7 |

## Data display (table/avatar/badge/chip/skeleton/progress)

| Module | Covers | Tests |
|---|---|---|
| `table_and_drag` | Table and drag-driven controls (Slider, etc.) behaviour. | 31 |
| `table_deep` | Deeper Table behaviour not covered by sorting/selection/resize suites. | 42 |
| `table_tabs_accordion` | Three surfaces (Table, Tabs, Accordion) left half-driven by earlier suites. | 21 |
| `avatar_deep` | Avatar runtime behavior against pinned v3.2.4 contract (load/error/fallback lifecycle). | 21 |
| `badge_parts` | Badge part-composition: `BadgeAnchor`, `Badge`, `BadgeLabel`. | 6 |
| `chip_deep` | Headless coverage for the measurable half of `Chip`. | 5 |
| `alert_deep` | Alert anatomy and geometry against pinned v3.2.4 `alert.css`/`alert.tsx`. | 7 |
| `card_deep` | Card anatomy and geometry against pinned v3.2.4 `card.css`. | 6 |
| `kbd_deep` | Kbd anatomy and geometry against pinned v3.2.4 `kbd.css`. | 5 |
| `tag_geometry_deep` | Tag line boxes match v3.2.4 outside the gallery's default text root. | 4 |
| `parts_thumbs` | `Slider.Thumb` and `ColorSwatchPicker.Item` per-part state. | 16 |
| `slider_indicators` | Slider dots, mark layout, decimal bounds, RTL locale/input, decimal endpoint colors and painted corners. | 11 |
| `slider_geometry_deep` | Slider paint geometry: the 12px start/end border inset. | 13 |

## Layout & surfaces

| Module | Covers | Tests |
|---|---|---|
| `surface_deep` | Headless coverage for the measurable half of `Surface`. | 4 |
| `scroll_shadow_deep` | ScrollShadow paths not exercised by the vertical-scroll suite. | 3 |
| `toolbar_divider_deep` | Headless coverage for the divider a `Toolbar` draws between its groups. | 4 |
| `full_width` | Phase 6 `full_width` seams: containers expand their root across the row (measured). | 1 |
| `size_enums_deep` | Additive size enums step the pinned geometry they own. | 4 |
| `radius_painted` | The 17 box-painting components' `radius` builders on the painted scene. | 1 |
| `radius_builders` | Per-component `radius` builders on the painted scene: overlays, fields and composite inner parts. | 14 |
| `typography_deep` | Typography and Prose against pinned v3.2.4 `typography.css`. | 9 |

## Accessibility

| Module | Covers | Tests |
|---|---|---|
| `a11y_deep` | Baseline accessibility contract: AccessKit tree is not observable headlessly; keyed-instance separation. | 6 |
| `a11y_overlays_deep` | Wave 2: accordion/disclosure/popover/dropdown/toast accessibility behaviour. | 6 |
| `a11y_collections_deep` | Wave 3: list box/tabs/toolbar/breadcrumbs/pagination/tag group/table/autocomplete accessibility behaviour. | 10 |
| `a11y_pickers_deep` | Wave 5: calendar/date field/time field/date picker/color picker accessibility behaviour. | 7 |

## Theming & tokens

| Module | Covers | Tests |
|---|---|---|
| `component_themes` | Live `Theme.components` resolution: stock, defaults, recipes, instance overrides. | 11 |
| `theme_platform` | Platform integration of the theme provider: OS appearance and GPUI window theme. | 4 |
| `theme_repaint` | Theme provider repaint: global theme mutations must redraw every open window. | 1 |
| `cursor_token` | The interactive cursor is a theme token, not a hard-coded call (source-shape check). | 2 |
| `hover_overrides` | Phase 2 hover overrides: every owner resolves the named fill and its source shape. | 1 |
| `hover_repaint` | A pointer crossing must repaint the view for every control whose hover state affects it. | 3 |
| `sx_ownership` | The part-scoped `sx` ownership inventory (source-shape check). | 4 |
| `sx_radius_parts` | Per-corner `sx` reconciliation for painted parts, matched quad by quad. | 3 |
| `sx_slot` | The `sx` slot: the one caller-owned styling slot every component exposes. | 4 |
| `icon` | Lucide `IconName`/`Icon`: names, paths and files agree, every icon loads through `HeroGpuiAssets` and rasterizes, and `Icon` lays out at its size. | 5 |
| `hover_card` | HoverCard extension: open and close delays, the card keeping itself open under the pointer, Escape and outside-press dismissal, keyboard focus, controlled open. | 7 |
| `command_palette` | CommandPalette extension: search focus, filtering and re-highlight, wrapping arrows, Enter and press, empty state, Escape/outside dismissal with focus return. | 8 |
| `sidebar` | Sidebar extension: item press and keyboard activation, roving cursor over items and collapsible headings, group expansion, typeahead, collapse to icons. | 6 |
| `title_bar` | TitleBar/WindowBorder extension: drawn controls report actions, press-and-move is one window move, double-click, child presses, platform `Auto` controls, border pass-through. | 6 |
| `toolbar_extras` | Toolbar `size` / `sized_child` extension: builder-order-independent size propagation, plain children keep their own size. | 2 |
| `table_extras` | Table extension: column reordering by header drag and Alt+arrows (controlled, uncontrolled, virtual rows), cell selection by press and keys, reported by given column index. | 9 |

## Animation & motion

| Module | Covers | Tests |
|---|---|---|
| `interaction` | Interaction tests driving controls (not just observing them). | 3 |

## Source-shape / parity contracts

| Module | Covers | Tests |
|---|---|---|
| `migration_extensions` | Native application composition: seek gestures, compact pickers, standalone menus. | 7 |

## Render-prop / value-prop contracts

| Module | Covers | Tests |
|---|---|---|
| `render_props` | Collection render-prop closures — the inverted per-row rendering API. | 39 |
| `value_props` | Inverted value/output render props: the closures a caller supplies for value text. | 42 |
| `virtual_and_feedback` | Paths a screenshot cannot see: virtualized lists/tables and related feedback. | 29 |

## Test harness / support

| Module | Covers | Tests |
|---|---|---|
| `source_scan/mod.rs` | Shared helper module (not a standalone test binary): finds a named consumer's enclosing function and asserts a reader/call appears inside it, for source-shape wiring tests. | 0 |

## How to run

Run one suite:

```
cargo test -p herogpui-components --test suite_collections
```

Run one former test module within its suite:

```
cargo test -p herogpui-components --test suite_collections collections::
```

Run the full suite:

```
cargo test -p herogpui-components
```

Local workaround: on this machine, an Xcode license/toolchain issue sometimes
requires pointing `DEVELOPER_DIR` at the command-line tools before running
tests. This is a local workaround, not a repository requirement:

```
DEVELOPER_DIR=/Library/Developer/CommandLineTools cargo test -p herogpui-components
```

## Conventions

- **Painted-scene and style readback.** `harness` reads what the headless
  platform can observe beyond layout: `harness::painted` returns the last
  frame's quads (`Window::painted_quads`: solid fill, border colour and widths,
  corner radii, content mask) in logical pixels, with helpers for exact and
  pill-clamped radii, fills, and the offset focus ring's `ring-offset` band
  (`Painted::ring_gaps`, `band_is_clipped`). `harness::style_probe` is a 1px
  canvas that records the `TextStyle` inherited where it is placed, so a probe
  in a caller-owned slot (children, `prefix`, `start_content`, an indicator or
  trigger closure) reads the family, weight, size, line height and colour the
  component's text there draws with. Shadows, paths and sprites (svg icons,
  the focus ring's rasterised band, glyphs) and the cursor are *not*
  exposed, and the `NoopTextSystem` resolves every font to one id.
- **Motion on the executor clock.** Since gpui-pre 0.3.11 gpui's
  `Animation` measures background-executor time, so a motion path is
  observed with `harness::wait_ms` (advance the test clock, then settle) and
  the component's own timing is separated from the theme's by stretching the
  theme's `hover_fade_ms` to a minute (a component with a stylesheet-specific
  duration still settles within it; a Button is the positive control).
- **Source-shape / contract tests.** A few modules (`cursor_token`,
  `hover_overrides`, `sx_ownership`, `theme_tokens`, `i18n`) are inventories
  or bans by design and read implementation source to check that a call site
  is wired the way the contract requires. Every
  other remaining source-text check is listed, with its reason, under
  *Remaining source-text checks* below; no test includes a component source
  with `include_str!`.
- **`source_scan` helper.** `tests/source_scan/mod.rs` is a shared module
  (`#[allow(dead_code)]`, no tests of its own) that reads a file from
  `src/` and asserts a given call appears inside the enclosing top-level
  function of a named consumer, so a sibling function's call cannot
  satisfy the check. `tests/sx_ownership.rs` documents the fixtures it
  relies on.
- **`#[gpui::test]` render/behavior tests.** Most modules use
  `#[gpui::test]` with a `TestAppContext`/`VisualTestContext` to mount a
  component, drive it with simulated pointer/keyboard input (including
  focus and typeahead), then assert on reported state, painted
  `debug_bounds` geometry, or the painted scene above. The AccessKit tree is
  not observable headlessly (see `a11y_deep.rs`).
- **`test-support` feature.** `gpui`'s own `test-support` feature is
  enabled only under `[dev-dependencies]` in
  `crates/herogpui-components/Cargo.toml`; it is what supplies the
  headless platform, `VisualTestContext`, and simulated input.
  `herogpui-components` itself defines no `test-support` feature — its
  only feature is `gallery-source`, unrelated to testing.

## Remaining source-text checks

Each check below reads implementation source because the behaviour it guards
leaves no trace the headless platform can read back. They go through
`source_scan::component_src`; none uses `include_str!`.

| Test | What it pins | Why it cannot be observed headlessly |
|---|---|---|
| `alert_deep::pinned_source::the_unstyled_status_is_default` | `Alert::new` seeds `Color::Default` | Default and Accent differ only in the glyph/title colour: no quad, and no slot inside the title for a style probe. |
| `alert_deep::pinned_source::every_status_draws_its_pinned_glyph` | Each status's icon path | `svg()` draws nothing on the test platform (no asset source), and sprites are not exposed. |
| `alert_deep::pinned_source::the_paint_only_tokens_stay_pinned` | Title `font-medium`, muted description, `shadow-surface` | Text colour/weight have no slot to probe; shadows are not exposed. |
| `buttons::toggle_button_press_rides_the_pinned_ramp` (one assertion) | A grouped member's instant `.active` swap to its hover fill | The pressing pointer also hovers it, and the hover-fade layer paints over the skin, so the active endpoint is not separable on the scene. |
| `calendars_deep::calendar_unavailable_cells_ask_for_the_not_allowed_cursor` | `CursorStyle::OperationNotAllowed` on unavailable cells | The test platform keeps the requested cursor private. |
| `close_button_deep::default_svg_uses_the_pinned_margins` | The glyph's `-mx-0.5 my-0.5` | Symmetric margins on a centred 16px child cancel, and the svg is not drawn. |
| `color_geometry_deep::color_swatch_color_name_overrides_hex_accessible_name` | `colorName` as the swatch's accessible name | AccessKit tree only. |
| `focus_ring_overlay::{number_field_group_hands_its_ring_to_a_carrier, date_field_group_hands_its_ring_to_a_carrier, input_rings_both_spellings_as_an_overlay}` | Ringless field chrome plus a non-clipping ring carrier | Field rings have no offset, so they are the rasterised SVG band alone, which paints no quad. |
| `font_seams::color_picker_readout_keeps_mono_and_accepts_a_family` | The hex readout's mono default and `font_family` | The readout is a bare text child with no caller slot, and the text system resolves every family to one font. |
| `nav_deep::current_breadcrumb_publishes_accesskit_current_page_state` | `aria-current="page"` on the last crumb | AccessKit tree only. |
| `parts_pagination::active_page_publishes_accesskit_current_page_state` | `aria-current="page"` on the active page | AccessKit tree only. |
| `pickers_deep::picker_option_labels_never_add_a_virtual_row_ellipsis` | Fixed-height rows keep normal wrapping, no ellipsis | The label box has no selector or slot and glyph runs are not exposed (natural-height wrap is measured by `select_long_values_wrap_the_trigger_and_natural_option_row`). |
| `pickers_deep::picker_trigger_chevrons_use_one_rotating_down_svg` | One down-chevron svg rotated by the shared helper | Svg not drawn; transforms not exposed (the indicator slot is measured by `picker_indicators_sit_in_the_pinned_absolute_end_slot`). |
| `table_tabs_accordion::accordion_and_disclosure_indicators_use_shared_rotating_chevrons` | The shared 250ms chevron rotation | Svg not drawn; transforms not exposed (the shared panel motion is measured by `accordion_and_disclosure_panels_ease_open_and_closed`). |
| `table_tabs_accordion::tabs_use_a_constrained_normal_whitespace_label_slot` | Tab labels wrap in a released-min-width slot | A constrained tab keeps its box whether the label wraps or overflows (verified: the box tests pass with `nowrap`), and the label slot has no selector. |
| `tabs_deep::tabs_shadow_and_accessible_name_are_wired_at_their_parts` | Indicator `shadow-surface`; the pre-measurement fallback reading the overrides; the tab's accessible name; the indicator's default radius call | Shadows are not exposed; the fallback frame is never the presented one (the first frame already has geometry); AccessKit tree only. The `.rounded(crate::util::control_radius(cx))` line is the `design_audit.py` fixture anchor and stays by design (audit-reader integrity). |
| `text_fields::input_group_gates_chrome_and_forwards_the_seam` (one assertion) | InputGroup forwards `height` to the held field | The held field's box has no selector and its content centres in the group either way (the `padding_x` forward is measured). |

The inventories and bans by design: `sx_ownership` (the part-scoped `sx`
inventory, see `docs/agents/components.md`), `hover_overrides` and
`theme_tokens` (token consumers scoped to their owning function, each with a
negative fixture proving the scanner can fail), `cursor_token` (a crate-wide
ban on calling GPUI's `cursor_pointer` directly),
`color_field_blur::the_focus_out_listener_disarms_on_fire_so_every_frame_re_arms_fresh`
(the active-window leg cannot be driven headlessly, where gpui blanks window
focus events) and `i18n` (every `UiString` key resolved in its owning file).

`radius_builders::autocomplete_panel_override_does_not_reach_the_trigger_box`
guards a defect the conversion found and 0.13 fixed: `Autocomplete::radius`
documents a panel-only override, but the trigger painted it too.
