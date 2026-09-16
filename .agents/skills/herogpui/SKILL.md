---
name: herogpui
description: 'HeroGPUI — native Rust/GPUI port of HeroUI v3.2.5. Use when building GPUI UIs with HeroGPUI: ThemeProvider, ThemeBuilder::components, named recipes, Buttons, forms, overlays, or reading crate docs. Keywords: HeroGPUI, herogpui, GPUI, HeroUI, ThemeBuilder, recipe.'
metadata:
  author: herogpui
  version: '0.9.0'
---

# HeroGPUI Development Guide

HeroGPUI is a native Rust/GPUI port of **HeroUI v3.2.5**. Depend on the `herogpui` facade. It re-exports GPUI at its root and the theme/component layers behind features.

This skill is for HeroGPUI only. Do not apply HeroUI v2 APIs, do not invent a CSS/`className` layer, and do not treat latest HeroUI docs as the contract. The pinned source of truth is the HeroGPUI repository (https://github.com/Porabuild/HeroGPUI): implementations, tests, and `llms.txt`.

---

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
herogpui = "0.9.0"
```

`cargo add herogpui` writes the same line. The facade pulls the matching `herogpui-core`, `herogpui-theme`, and `herogpui-components` releases from crates.io.

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

1. **`llms.txt`** at the HeroGPUI repository root — the public component API reference.
2. **Rustdoc** for the published crates, from this repository's GPUI workspace:

```bash
cargo doc --manifest-path src/main/Cargo.toml --package herogpui --no-deps --open
cargo doc --manifest-path src/main/Cargo.toml --package herogpui-theme --no-deps --open
cargo doc --manifest-path src/main/Cargo.toml --package herogpui-components --no-deps --open
```

3. Implementation and focused tests under `crates/herogpui-components/` and `crates/herogpui-theme/` in that repository.
4. Gallery examples under `gallery/src/pages/components/` and getting-started pages under `gallery/src/pages/docs.rs` in that repository.

Do not invent URLs, fetch scripts, or a CSS class catalog. HeroUI's `heroui.com` MDX is the React product; use tagged HeroUI **v3.2.5** source only when checking port parity, never as the Rust API.

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
