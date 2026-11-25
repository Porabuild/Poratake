---
name: herogpui
description: 'HeroGPUI — native Rust/GPUI port of HeroUI v3.2.7. Use when building GPUI UIs with HeroGPUI: ThemeProvider, ThemeBuilder::components, named recipes, Buttons, forms, overlays, or reading crate docs. Keywords: HeroGPUI, herogpui, GPUI, HeroUI, ThemeBuilder, recipe.'
metadata:
  author: herogpui
  source: https://github.com/Porabuild/HeroGPUI/blob/30fad022e1c3d3ae1a92bffba938e596f3cbecfe/skills/herogpui/SKILL.md
  version: '0.15.5'
  source_commit: 30fad022e1c3d3ae1a92bffba938e596f3cbecfe
---

# HeroGPUI Development Guide

HeroGPUI is a native Rust/GPUI port of **HeroUI v3.2.7**. Depend on the `herogpui` facade. It re-exports GPUI at its root and the theme/component layers behind features.

This skill is for HeroGPUI only. Do not apply HeroUI v2 APIs, do not invent a CSS/`className` layer, and do not treat latest HeroUI docs as the contract. The pinned source of truth is the HeroGPUI source tree: implementations, tests, and `llms.txt`.

---

## Poratake's pinned references

Use the exact published registry version in `src/main/Cargo.toml`: HeroGPUI 0.15.5 anchors slider, color-slider, and color-area thumb drags to the pointer delta (#52) and continues interrupted or reopened menu entry/exit motion from the painted frame (#53), on top of the compact slider hit-target/sizing fix (#46), in-window tooltip viewport clamping and compact-window inset fitting (#47, #51), and the prior consumer fixes from #44. The bundled references match that release; [upstream.json](upstream.json) records the tag, source revision and file checksums. Keep `gpui-pre` aligned with the release workspace.

- Start with [the local LLM component reference](references/llms.txt). Search for the relevant component or API instead of reading the whole reference.
- When modifying HeroGPUI itself, use its [workflow](references/agents/workflow.md), [component implementation](references/agents/components.md), [parity audits](references/agents/parity.md), and [gallery verification](references/agents/gallery.md) guides. Their paths and commands refer to the HeroGPUI checkout; Poratake's own repository instructions govern work here.
- Resolve implementation paths from the pinned dependency's `manifest_path` in `cargo metadata --locked --manifest-path src/main/Cargo.toml --format-version 1` when source inspection is needed.
- Poratake patches the existing GPUI 0.3.11 Windows backend from `vendor/gpui-pre-windows` to clamp image sampling inside atlas tiles. Keep this source patch when updating dependency pins until its regression passes against an upstream replacement; the other GPUI crates remain registry dependencies.
- All four HeroGPUI crates resolve to the published 0.15.5 registry release with locked checksums. Focus-ring gutters, Select/Slider accessibility actions and keyboard press ownership are provided upstream; do not restore the former vendored component patch.
- Use `Select::fit_panel_width(true)` for intrinsic panel sizing. Compact owner windows use Poratake’s `Tooltip::presenter` integration so HeroGPUI owns the lifecycle. Windows uses a capture-excluded native surface. macOS preserves AppKit hover help and in-window keyboard tooltips; its native capture behavior still needs verification. The built-in separate-window presenter does not enforce capture exclusion.

## CRITICAL: Theme-first appearance

**Put shared component appearance on the theme.** Do not copy height, padding, radius, text size, or semantic colours onto every call site.

| Shared look                                           | One-off layout                              |
| ----------------------------------------------------- | ------------------------------------------- |
| `ThemeBuilder::components` defaults and named recipes | Instance `sx` for placement, width, flex    |
| `Button::recipe("compact").recipe("muted")`           | `Button::new(id).sx(\|el\| el.w(px(120.)))` |

Precedence, resolved **live** from the active `ThemeProvider` each frame (not captured when the builder is constructed):

1. Stock HeroUI defaults
2. `theme.components.*.defaults`
3. Named recipes in `.recipe("name")` order
4. Explicit component builders (`height`, `variant`, `panel_gap`, …)
5. Instance `sx`

Empty styles preserve stock HeroUI behaviour. A missing recipe name is a no-op, so switching back to `Theme::light()` / `Theme::dark()` restores stock presentation.

Do not invent Poratake-specific or app-specific field names in the library. App recipes are defined by the application on `ThemeBuilder::components`.

---

## Installation

```toml
herogpui = "=0.15.5"
```

```rust
use herogpui::*;

fn main() {
    application().with_assets(HeroGpuiAssets).run(|cx: &mut App| {
        herogpui::init(cx); // ThemeProvider::init — registers light + dark
        // or ThemeProvider::init_with(custom, cx);
        cx.open_window(WindowOptions::default(), |_, cx| cx.new(|_| MyRoot));
    });
}
```

`herogpui::init` installs `ThemeProvider`. Rendering a themed component before that panics.

---

## ThemeProvider and the theme builder

```rust
use herogpui::prelude::*;
use herogpui::core::oklch;
use herogpui::theme::snow;

herogpui::theme::set_theme(
    Theme::builder("brand", Theme::dark())
        .accent(oklch(0.55, 0.23, 295.0))
        .role("success", oklch(0.73, 0.19, 150.0), snow())
        .radius(px(6.)) // field_radius follows at 1.5×
        .components(/* see below */)
        .build(),
    cx,
);

herogpui::theme::use_theme("dark", cx);
herogpui::theme::toggle_light_dark(cx);
```

Read tokens with `ActiveTheme`: `cx.colors()`, `cx.role(Color::Accent)`, `cx.layout()`, `cx.components()`, `cx.theme()`.

Sparse JSON overrides exist behind the `serde` feature (`ThemeDocument`). `components` is **not** a JSON key — it is a typed Rust subtree.

---

## ComponentThemes and `.recipe()`

```rust
use herogpui::prelude::*;

let components = ComponentThemes::default()
    .slider(ComponentTheme::new(SliderStyle::default().radius(px(9999.))))
    .switch(ComponentTheme::new(SwitchStyle::default().radius(px(9999.))))
    .select(
        ComponentTheme::new(SelectStyle::default().variant(FieldVariant::Secondary))
            .recipe(
                "compact",
                SelectStyle::default()
                    .height(px(28.))
                    .padding_x(px(8.))
                    .trigger_text_size(px(12.))
                    .row_height(px(28.))
                    .row_padding_x(px(8.))
                    .row_padding_y(px(2.))
                    .row_text_size(px(12.))
                    .panel_padding(px(4.)),
            ),
    )
    .menu(
        ComponentTheme::new(MenuStyle::default().panel_gap(px(0.)))
            .recipe(
                "compact",
                MenuStyle::default()
                    .row_height(px(28.))
                    .row_padding_x(px(8.))
                    .row_padding_y(px(2.))
                    .row_text_size(px(12.))
                    .row_gap(px(8.))
                    .panel_padding(px(4.)),
            )
            .recipe(
                "accent",
                MenuStyle::default()
                    .row_hover_bg(ComponentColor::Role(Color::Accent))
                    .row_hover_foreground(ComponentColor::RoleForeground(Color::Accent)),
            ),
    )
    .button(
        ComponentTheme::new(ButtonStyle::default())
            .recipe(
                "compact",
                ButtonStyle::default().style(|el| {
                    el.h(px(28.)).px(px(10.)).text_size(px(12.)).line_height(px(16.))
                }),
            )
            .recipe(
                "compact-icon",
                ButtonStyle::default().style(|el| el.size(px(28.)).p(px(0.))),
            )
            .recipe("muted", ButtonStyle::default().variant(Variant::Secondary))
            .recipe("danger", ButtonStyle::default().variant(Variant::Danger)),
    )
    .text_field(
        ComponentTheme::new(TextFieldStyle::default())
            .recipe("search", TextFieldStyle::default().height(px(32.)).text_size(px(13.)))
            .recipe("compact", TextFieldStyle::default().height(px(28.)).text_size(px(12.))),
    );

Theme::builder("app", Theme::light())
    .components(components)
    .build();
```

Call-site stacking:

```rust
Button::new("ok").label("OK").recipe("compact").recipe("muted");
Select::new("lang", items).recipe("compact");
Menu::new("tray", items).recipe("compact").recipe("accent");
Slider::new("vol", 0.5).recipe("pill");
Input::new(&state).recipe("search");
```

### APIs added in 0.15.1

These APIs are included in HeroGPUI 0.15.1. Existing 0.15.0 code remains compatible.

- `Select::item_content(|key, selected, window, cx| ..)` replaces visible option content while preserving collection labels and typeahead. `panel_gap(px)` and `panel_min_width(px)` also resolve through SelectStyle recipes.
- `ColorSwatchPicker::sx` customizes the group layout. Keyboard columns follow the rendered allocation.
- `Popover::panel_width(px)` sizes the panel and its motion together; `aria_label` names a dialog without adding a heading. `Modal::panel_width(px)` and `padding(px)` override its preset width and inset.
- `Switch::control_scale(f32)` scales the painted control and thumb while keeping layout, labels and the activation target stable.
- Button, standalone ToggleButton and CloseButton apply painted styles to the skin and placement/opacity to the resting slot. Explicit builders beat theme metrics; instance `sx` wins last.
- Pressable roots expose the `Button` keyboard context; open Popovers expose `Popover`. Scope application shortcuts so native controls own Enter and their list navigation.
- For short option lists, use natural ListBox rows. `row_height` enables virtualization with its own viewport; set an explicit viewport bound when virtualization is needed.

### APIs added in 0.15.2

HeroGPUI 0.15.2 adds `Select::fit_panel_width` / `intrinsic_panel_width`, external tooltip presenters (`TooltipPresenter`, `TooltipPresentationRequest`, `TooltipPresenterMode`), and typed `TooltipStyle`, `PopoverStyle`, `TextAreaStyle`, and `TabsStyle` recipes. Read the bundled LLM reference for their contracts. These additions are included in the published 0.15.2 release.

### Fixes in 0.15.5

HeroGPUI 0.15.5 keeps `Slider`, `ColorSlider`, and `ColorArea` thumb drags anchored to the grabbed point instead of jumping the thumb center to the pointer, and reverses `Dropdown`/`Menu` panel motion from the painted opacity, padding, and radius when a close interrupts entry or a reopen interrupts exit, rather than snapping to a full-size opaque panel.

### Fixes in 0.15.4

HeroGPUI 0.15.4 shrinks in-window `Tooltip` viewport edge insets proportionally when a tooltip fits inside a compact window (such as a 52px bar) without room for 8px margins on both sides, preventing bottom/right clipping.

### Fixes in 0.15.3

HeroGPUI 0.15.3 increases `SliderSize::Sm` to an 8px rail with a 14px knob centered inside an 18px interactive hit box (both horizontal and vertical) so clicks and drags on the overhanging handle register reliably, and keeps in-window `Tooltip` popups inside the window viewport with primary-axis flipping, 8px edge clamping, narrow-viewport wrapping, and popover/menu layering.

### Public types and methods

- `ThemeBuilder::components(ComponentThemes { … })` and fluent `ComponentThemes::{slider,switch,select,menu,button,text_field}(ComponentTheme<T>)`
- `ComponentTheme<T>::new(defaults).defaults(T).recipe("name", style).resolve(&[SharedString])`
- `ComponentTheme<SliderStyle | SwitchStyle | SelectStyle | MenuStyle | ButtonStyle | TextFieldStyle>`
- `ComponentColor`: `Literal(Hsla)`, `Background`, `Foreground`, `Muted`, `Surface`, `SurfaceForeground`, `SurfaceSecondary`, `SurfaceTertiary`, `Border`, `FieldBackground`, `FieldForeground`, `FieldPlaceholder`, `Role(Color)`, `RoleForeground(Color)`, `RoleHover(Color)`, `RoleSoft(Color)`, plus `From<Hsla>`
- `ButtonStyle::style(|el| …)` captures sparse GPUI metrics (height / padding / text / gap). Instance `sx` refines over it.
- Style fields are all `Option`. Unset keeps stock.
  - `SliderStyle` / `SwitchStyle`: `radius`
  - `SelectStyle`: `variant`, `height`, `padding_x`, `trigger_text_size`, `row_height`, `row_padding_x`, `row_padding_y`, `row_text_size`, `panel_padding`, `radius`, `row_hover_bg`, `is_bare`
  - `MenuStyle`: `panel_min_width`, `panel_max_width`, `panel_max_height`, `panel_padding`, `panel_gap`, `row_height`, `row_padding_x`, `row_padding_y`, `row_text_size`, `row_gap`, `row_hover_bg`, `row_hover_foreground`, `radius`, `animate_entry`
  - `TextFieldStyle`: `variant`, `height`, `padding_x`, `text_size`, `radius`, `is_bare`, `background`, `foreground`, `placeholder`
  - `ButtonStyle`: `variant`, `size`, `radius`, `background`, `foreground`, `hover_bg`, `hover_foreground`, `pressed_bg`, `pressed_foreground`, `disabled_foreground`, `style`
- `.recipe("name")` is on `Slider`, `Switch`, `Select`, `Menu`, `Dropdown`, `Button`, `Input`, `TextField`, `SearchField`. Recipes stack.
- `SliderStyle.radius` reaches the track, thumb layers, and caps using the seam-safe per-corner join: an `sx` corner still wins where it is named.
- `ActiveTheme::components()` / `cx.components()` reads the live table.

---

## Fetching crate docs

There is no separate HeroGPUI documentation site to scrape. Read:

1. **`llms.txt`** at the repository root — the public component API reference.
2. **Rustdoc** for the workspace crates:

```bash
cargo doc --manifest-path src/main/Cargo.toml -p herogpui --no-deps --open
cargo doc --manifest-path src/main/Cargo.toml -p herogpui-theme --no-deps --open
cargo doc --manifest-path src/main/Cargo.toml -p herogpui-components --no-deps --open
```

3. Implementation and focused tests under `crates/herogpui-components/` and `crates/herogpui-theme/`.
4. Gallery examples under `gallery/src/pages/components/` and getting-started pages under `gallery/src/pages/docs.rs`.

Do not invent URLs, fetch scripts, or a CSS class catalog. HeroUI's `heroui.com` MDX is the React product; use tagged HeroUI **v3.2.7** source only when checking port parity, never as the Rust API.

---

## Component patterns

Builders implement `RenderOnce`. Interactive elements need unique ids. Callbacks cloned into GPUI closures are `Arc<dyn Fn ...>`. Variants are semantic (`Primary`, `Secondary`, `Tertiary`, `Danger`, …), not raw colours.

```rust
Button::new("save")
    .label("Save")
    .variant(Variant::Primary)
    .recipe("compact");
```

---

## License

Apache-2.0. See `LICENSE.txt` in this skill directory.
