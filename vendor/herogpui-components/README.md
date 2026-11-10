# HeroGPUI

HeroGPUI is a UI library for Rust desktop applications. It brings the HeroUI
design system to Rust, built on GPUI, the GPU-accelerated framework behind the
Zed editor. It runs on Windows, macOS and Linux from one codebase.

It implements every component HeroUI v3.2.7 documents; the gallery and
website show one page per component, except that a few pages cover a
component together with its group or slot siblings.

```rust
use herogpui::*;

impl Render for MyApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        app_focus_root(div()
            .size_full()
            .bg(cx.colors().background)
            .child(
                Button::new("save")
                    .label("Save changes")
                    .variant(Variant::Primary)
                    .on_press(cx.listener(|this, _, _, cx| this.save(cx))),
            ), window, cx)
    }
}
```

## Installation

Prerequisites: Rust 1.99 and the platform tooling GPUI needs (Xcode on macOS;
Wayland/X11 dev packages on Linux; nothing extra on Windows).

`herogpui` is published on crates.io. Add it with `cargo add herogpui`, or:

```toml
[dependencies]
herogpui = "0.15"
```

That is the whole list. A git or path dependency on this repository works the same way. `herogpui` is a facade: it depends on the matching
`gpui-pre` and `gpui-pre-platform` crates and re-exports them, so
`use herogpui::*;` **is** GPUI and `herogpui::application()` opens the
platform. Do not add `gpui` or `gpui_platform` to your own `Cargo.toml` — a
second copy of GPUI is how versions drift apart. (Zed does not publish `gpui`
under that name, which is why GPUI arrives as `gpui-pre`, a prerelease publish of the Zed GPUI
sources by crates.io user huacnlee (Jason Lee, the gpui-kit maintainer).
HeroGPUI pins it exactly (`=0.3.11`). The unrelated crates.io `gpui` 0.2.2
crate is a different library.)

Each layer is also reachable by name, and each is a Cargo feature:

| Path | Crate | Feature |
| --- | --- | --- |
| `herogpui::*` | `gpui` | always |
| `herogpui::platform`, `herogpui::application` | `gpui_platform` | always |
| `herogpui::core` | `herogpui-core` | `core` (via `theme`) |
| `herogpui::theme` | `herogpui-theme` | `theme` (via `components`) |
| `herogpui::components`, `herogpui::*` | `herogpui-components` | `components` (default) |
| `herogpui::web` | `gpui_platform` | always, `cfg(wasm)` only |

Two more features only forward to GPUI: `test-support`
(`gpui/test-support` + `gpui_platform/test-support`, for your own
`#[gpui::test]`) and `profiler` (`gpui/profiler`); `multithreaded` forwards
`gpui_platform/multithreaded`, GPUI's multi-threaded web platform (wasm32
only — it needs a nightly toolchain and a cross-origin-isolated page, and is
off by default). `serde` forwards
`herogpui-theme/serde` and is off by default: it is the sparse
`ThemeDocument` JSON applied through `ThemeBuilder`, not a dump of a
resolved `Theme`.

Then, inside `application().run(..)`:

1. Register the embedded icons with
   `herogpui::application().with_assets(HeroGpuiAssets)`, or
   `HeroGpuiAssets::with_fallback(MyAppAssets)` when the app has assets of its
   own.
2. Initialize the enabled layers with `herogpui::init(cx)` before opening a
   window. That is `ThemeProvider::init(cx)`; call
   `ThemeProvider::init_with(theme, cx)` instead to start from a custom theme.
3. Wrap the root element with `app_focus_root(root, window, cx)` so Tab and
   focus-visible behavior work across components.
4. Set the background, foreground and font family on the root view from
   tokens.

The whole program, with `herogpui` as its only dependency:

```rust
use herogpui::*;

actions!(demo, [Quit]);

struct Hello;

impl Render for Hello {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        app_focus_root(
            div()
                .size_full()
                .bg(cx.colors().background)
                .text_color(cx.colors().foreground)
                .font_family("Helvetica")
                .child(Button::new("save").label("Save changes")),
            window,
            cx,
        )
    }
}

fn main() {
    application().with_assets(HeroGpuiAssets).run(|cx: &mut App| {
        herogpui::init(cx);
        let bounds = Bounds::centered(None, size(px(1280.), px(820.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| cx.new(|_| Hello),
        )
        .expect("failed to open window");
    });
}
```

That program is a doctest in `crates/herogpui/src/lib.rs`, so
`cargo test --doc -p herogpui` compiles it rather than this file asserting that
it would. `actions!` is HeroGPUI's own copy of GPUI's macro: the upstream one
expands its derive as the absolute path `gpui::Action`, which does not resolve
when GPUI is reached only through a facade. Eight names — `ColorSpace`,
`FontWeight`, `Menu`, `MenuItem`, `Orientation`, `Size`, `Surface` and
`TextAlign` — exist in both GPUI and HeroUI v3; at `herogpui`'s root the
HeroUI spelling wins, and GPUI's keep the `gpui::` path (`gpui::Size`).

## Examples

Small runnable crates under `examples/` use only the `herogpui` facade:

```bash
cargo run -p herogpui-example-hello-button  # one button, one counter
cargo run -p herogpui-example-form          # validated Form with TextFields
cargo run -p herogpui-example-theme-switch  # runtime light/dark switching
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for contribution rules.

## Gallery

A desktop gallery ships with the library and documents every component:

```bash
cargo run -p herogpui-gallery   # open the component gallery
cargo install --path gallery --locked  # install the gallery CLI from this checkout
cargo install herogpui-gallery  # or install the published gallery CLI
```

There is no setup step: a fresh clone builds with plain `cargo` against the
published `gpui-pre` crates, with no `[patch.crates-io]` overrides, vendored
sources or git hooks.

`HEROGPUI_PAGE` and `HEROGPUI_THEME` select the page and appearance;
`HEROGPUI_WINDOW_SIZE` sets the window size.

## Documentation

- Website: <https://porabuild.com/herogpui> — every component page embeds
  HeroGPUI compiled to WebAssembly, running live next to its Rust code.
- `llms.txt` at the repository root: the full component API reference for
  agents. It is served verbatim at
  <https://porabuild.com/herogpui/llms.txt>.
- [`docs/migration-0.15.md`](docs/migration-0.15.md): upgrading from 0.14
  to 0.15, with before/after code for each breaking change.
- [`docs/migration-0.13.md`](docs/migration-0.13.md): upgrading from 0.12
  to 0.13, with before/after code for each breaking change.
- [`docs/migration-0.11.md`](docs/migration-0.11.md): upgrading from 0.10
  to 0.11, with before/after code for each breaking change.
- `AGENTS.md` and `docs/agents/`: the contributor and agent guides.

## Verification

Audit tooling under `.shots/` checks the implementation against the upstream
design system. See `docs/agents/parity.md` for what each audit reads and what
it proves. The lint gate is `bash .shots/lint.sh`.

## License

Apache-2.0. This library derives component behavior, design tokens, styles,
and documentation structure from HeroUI, Copyright 2025 NextUI Inc., also
Apache-2.0. See `LICENSE` and `NOTICE`.
