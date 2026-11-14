//! Shared harness for the behaviour tests.
//!
//! Each feature suite loads this once and its test modules use
//! `crate::harness`; Cargo never treats it as a standalone test target. The
//! helpers exist because a control can draw perfectly and not work: they open
//! one real gpui window on the headless test platform (`test-support`,
//! enabled in this crate's dev-dependencies) and drive it with simulated
//! clicks and keystrokes.
//!
//! Two harness facts worth knowing before adding more tests:
//!
//! - Component state lives in the *window's* keyed state
//!   (`window.use_keyed_state`), so one host window must survive a whole test.
//!   The builder closures re-run on every frame — that is how these components
//!   are always driven — and only the keyed state carries the value across
//!   frames.
//! - The test platform ships no asset source (`AssetSource for ()` answers
//!   `Ok(None)`), so every `svg()` glyph silently renders nothing. That path
//!   logs instead of panicking, which is why no stub source is installed here.

// Each feature suite compiles this module separately, so a helper that one
// suite does not call is dead code there even though another uses it. The
// allow keeps the shared surface from sprouting per-suite copies.
#![allow(dead_code)]

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::OnceLock,
};

use gpui::{
    canvas, point, prelude::*, px, AnyElement, Context, KeyUpEvent, Keystroke, Modifiers, Render,
    TestAppContext, VisualTestContext, Window,
};
use herogpui_components::{extend, Date, DateSegment, Tooltip, TooltipHover};
use herogpui_theme::{set_reduce_motion, ThemeProvider};

thread_local! {
    static REDUCE_MOTION: Cell<bool> = const { Cell::new(false) };
}

/// Makes the next host opened on this test thread suppress motion.
pub fn still() {
    REDUCE_MOTION.set(true);
}

/// A zero-behaviour 10px probe with a debug selector, so a part's inset is
/// measurable through `debug_bounds`.
pub fn probe(name: &'static str) -> AnyElement {
    gpui::div()
        .w(px(10.))
        .h(px(10.))
        .debug_selector(move || name.to_owned())
        .into_any_element()
}

/// What the component callbacks recorded, cloned into each closure.
pub type Events = Rc<RefCell<Vec<String>>>;

/// An empty recorder, ready to be cloned into the builder closures.
pub fn events() -> Events {
    Rc::new(RefCell::new(Vec::new()))
}

/// Reads one tooltip's keyed open state from the component's render id path.
pub fn tooltip_open_probe(id: &'static str, seen: Events, focus_open: bool) -> AnyElement {
    canvas(
        move |_, window, cx| {
            let open = window.with_id(std::any::type_name::<Tooltip>(), |window| {
                let state = window
                    .use_keyed_state(gpui::ElementId::Name(id.into()), cx, |_, _| {
                        TooltipHover::closed()
                    })
                    .read(cx);
                if focus_open {
                    state.is_focus_open()
                } else {
                    state.is_open()
                }
            });
            seen.borrow_mut().push(format!("open:{open}"));
        },
        |_, _, _, _| {},
    )
    .size_0()
    .into_any_element()
}

/// Renders one component under test at the top-left corner of the window, with
/// no padding, so simulated click coordinates land where the layout says.
pub struct Host {
    content: Box<dyn Fn() -> AnyElement>,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let root = gpui::div().size_full().child((self.content)());
        extend::app_focus_root(root, window, cx)
    }
}

/// Installs the theme global and opens one host window on the test platform.
pub fn open_host(
    cx: &mut TestAppContext,
    content: impl Fn() -> AnyElement + 'static,
) -> &mut VisualTestContext {
    // Every component reads its tokens through the `ThemeProvider` global;
    // drawing without one panics.
    let reduce_motion = REDUCE_MOTION.replace(false);
    cx.update(|cx| {
        ThemeProvider::init(cx);
        if reduce_motion {
            set_reduce_motion(true, cx);
        }
    });
    let (_view, cx) = cx.add_window_view(|_, _| Host {
        content: Box::new(content),
    });
    // GPUI recomputes hover during layout; start outside the window so a
    // control at the origin is idle until the test supplies pointer input.
    cx.simulate_mouse_move(point(px(-100.), px(-100.)), None, Modifiers::none());
    cx
}

/// One hit-tested left click at window coordinates (`x`, `y`).
pub fn click(cx: &mut VisualTestContext, x: f32, y: f32) {
    cx.simulate_click(point(px(x), px(y)), Modifiers::none());
}

/// Types `keys` (a space-separated keystroke string) and releases the last key.
///
/// **The most surprising fact in this harness:** gpui activates a focused
/// element on key *up*. An element that holds the focus fires its click
/// listeners when Enter or Space is *released* — the listener is registered on
/// `KeyUpEvent` — and `dispatch_keystroke`, which backs
/// `simulate_keystrokes`, sends only the down half. Without the explicit
/// `KeyUpEvent` below, `"enter"` reaches every `on_key_down` along the focus
/// chain but activates nothing that waits for the click.
pub fn press(cx: &mut VisualTestContext, keys: &str) {
    cx.simulate_keystrokes(keys);
    if let Some(last) = keys.split_whitespace().next_back() {
        let os = cx.cx.read(|app| app.platform_info().os);
        cx.simulate_event(KeyUpEvent {
            keystroke: Keystroke::parse_with_os(last, os).unwrap(),
        });
    }
}

fn date_order_for_locale(locale: &str) -> Option<[DateSegment; 3]> {
    use icu_datetime::{
        fieldsets,
        input::Date as IcuDate,
        options::YearStyle,
        provider::{
            fields::FieldSymbol,
            pattern::{reference, runtime, PatternItem},
        },
        DateTimeFormatter,
    };
    use icu_locale_core::Locale as IcuLocale;

    let formatter = DateTimeFormatter::try_new(
        locale.parse::<IcuLocale>().ok()?.into(),
        fieldsets::YMD::short().with_year_style(YearStyle::Full),
    )
    .ok()?;
    let formatted = formatter.format(&IcuDate::try_new_iso(2000, 1, 1).ok()?);
    let pattern: runtime::Pattern<'_> = formatted.pattern().into();
    let order: Vec<_> = reference::Pattern::from(&pattern)
        .into_items()
        .into_iter()
        .filter_map(|item| match item {
            PatternItem::Field(field) => match field.symbol {
                FieldSymbol::Month(_) => Some(DateSegment::Month),
                FieldSymbol::Day(_) => Some(DateSegment::Day),
                FieldSymbol::Year(_) => Some(DateSegment::Year),
                _ => None,
            },
            PatternItem::Literal(_) => None,
        })
        .collect();
    order.try_into().ok()
}

/// The order expected from the operating system's regional date preference.
pub fn system_date_order() -> [DateSegment; 3] {
    static ORDER: OnceLock<[DateSegment; 3]> = OnceLock::new();
    *ORDER.get_or_init(|| {
        herogpui_components::__private::time_locale_tags()
            .iter()
            .find_map(|tag| date_order_for_locale(tag))
            .unwrap_or(DateSegment::ALL)
    })
}

/// Moves an already focused date-only field to `target` regardless of locale.
pub fn focus_date_segment(cx: &mut VisualTestContext, target: DateSegment) {
    for _ in 1..DateSegment::ALL.len() {
        press(cx, "left");
    }
    let target = system_date_order()
        .iter()
        .position(|segment| *segment == target)
        .unwrap();
    for _ in 0..target {
        press(cx, "right");
    }
}

/// Types a complete date into a field focused on its first regional segment.
pub fn type_date(cx: &mut VisualTestContext, date: Date) {
    for segment in system_date_order() {
        let digits = match segment {
            DateSegment::Month => format!("{:02}", date.month),
            DateSegment::Day => format!("{:02}", date.day),
            DateSegment::Year => format!("{:04}", date.year),
        };
        for digit in digits.chars() {
            press(cx, &digit.to_string());
        }
    }
}

// ---------------------------------------------------------------------------
// Painted-scene and inherited-style readback
// ---------------------------------------------------------------------------
//
// What the headless platform *can* read back, and what these helpers wrap:
//
// - `Window::painted_quads` (gpui `test-support`): every quad of the last
//   frame, with its solid fill, border colour and widths, corner radii and the
//   content mask it is clipped to. Quads are in scaled pixels; `Painted`
//   converts to logical ones. Deferred overlays (popovers, menus, dialogs)
//   paint into the same frame, so an open surface is in the same list.
// - The inherited `TextStyle` at any point of a component's tree, through a
//   canvas probe placed in a caller-owned slot: font family, weight, size,
//   line height and colour are what the text under that slot will draw with.
//
// What it cannot: shadows, paths and sprites (SVG icons, the focus ring's
// rasterised band, glyphs) are not exposed by the pinned GPUI, and the
// `NoopTextSystem` resolves every font to one id, so the *family* only shows
// through the inherited style, never through glyph metrics.

/// Redraws the window and parks the executor a few times, so keyed state
/// written during one frame is painted by the next.
pub fn settle(cx: &mut VisualTestContext) {
    for _ in 0..3 {
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
    }
}

/// Advances the test executor's clock by `ms`, then settles: since
/// `gpui-pre` 0.3.11 gpui's `Animation` measures time on the background
/// executor's clock, so motion is deterministic in tests and no real-time
/// sleep is needed (or effective).
pub fn wait_ms(cx: &mut VisualTestContext, ms: u64) {
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(ms));
    settle(cx);
}

/// The quads of the last painted frame, with the frame's scale factor.
pub struct Painted {
    pub quads: Vec<gpui::Quad>,
    pub scale: f32,
}

/// Settles and reads the painted quads.
pub fn painted(cx: &mut VisualTestContext) -> Painted {
    settle(cx);
    cx.update(|window, _| Painted {
        quads: window.painted_quads(),
        scale: window.scale_factor(),
    })
}

impl Painted {
    /// A quad's bounds in logical pixels.
    pub fn bounds(&self, quad: &gpui::Quad) -> gpui::Bounds<gpui::Pixels> {
        let b = quad.bounds;
        gpui::Bounds {
            origin: point(px(b.origin.x.0 / self.scale), px(b.origin.y.0 / self.scale)),
            size: gpui::size(
                px(b.size.width.0 / self.scale),
                px(b.size.height.0 / self.scale),
            ),
        }
    }

    /// The content mask a quad is clipped to, in logical pixels.
    pub fn mask(&self, quad: &gpui::Quad) -> gpui::Bounds<gpui::Pixels> {
        let b = quad.content_mask.bounds;
        gpui::Bounds {
            origin: point(px(b.origin.x.0 / self.scale), px(b.origin.y.0 / self.scale)),
            size: gpui::size(
                px(b.size.width.0 / self.scale),
                px(b.size.height.0 / self.scale),
            ),
        }
    }

    /// Whether the quad's own box reaches outside the mask it is clipped to.
    pub fn is_clipped(&self, quad: &gpui::Quad) -> bool {
        let b = self.bounds(quad);
        let m = self.mask(quad);
        let (bx, by) = (f32::from(b.origin.x), f32::from(b.origin.y));
        let (mx, my) = (f32::from(m.origin.x), f32::from(m.origin.y));
        bx < mx - 0.5
            || by < my - 0.5
            || bx + f32::from(b.size.width) > mx + f32::from(m.size.width) + 0.5
            || by + f32::from(b.size.height) > my + f32::from(m.size.height) + 0.5
    }

    /// Top-left, top-right, bottom-right, bottom-left radii, logical pixels.
    pub fn corners(&self, quad: &gpui::Quad) -> [f32; 4] {
        let c = quad.corner_radii;
        [c.top_left, c.top_right, c.bottom_right, c.bottom_left].map(|r| r.0 / self.scale)
    }

    /// Top, right, bottom, left border widths, logical pixels.
    pub fn borders(&self, quad: &gpui::Quad) -> [f32; 4] {
        let e = quad.border_widths;
        [e.top, e.right, e.bottom, e.left].map(|w| w.0 / self.scale)
    }

    /// All four corners at `value`, after the clamp to half the shorter side
    /// the painter applies to an oversized radius (a pill).
    pub fn is_uniform(&self, quad: &gpui::Quad, value: f32) -> bool {
        let b = self.bounds(quad);
        let value = value.min(f32::from(b.size.width).min(f32::from(b.size.height)) / 2.);
        self.corners(quad)
            .iter()
            .all(|corner| (corner - value).abs() < 0.05)
    }

    /// The quads whose four corners are `value`.
    pub fn rounded(&self, value: f32) -> Vec<&gpui::Quad> {
        self.quads
            .iter()
            .filter(|q| self.is_uniform(q, value))
            .collect()
    }

    /// The quads whose four corners are `value` *without* the pill clamp:
    /// the box is at least twice `value` on its shorter side, so the value
    /// is really painted rather than capped to half a hairline.
    pub fn rounded_exact(&self, value: f32) -> Vec<&gpui::Quad> {
        self.quads
            .iter()
            .filter(|q| {
                let b = self.bounds(q);
                let short = f32::from(b.size.width).min(f32::from(b.size.height));
                value <= short / 2. + 0.05
                    && self
                        .corners(q)
                        .iter()
                        .all(|corner| (corner - value).abs() < 0.05)
            })
            .collect()
    }

    /// The offset focus ring's gap bands: the overlay ring
    /// (`util::focus_ring_overlay` with `offset`) paints its accent band as a
    /// rasterised SVG the scene does not expose, but the `ring-offset` band
    /// between it and the control is a quad — a `gap`-wide border in the page
    /// `background`, rounded to the control's radius plus `gap`. Finding one
    /// proves the overlay ring is up; its content mask shows whether a
    /// clipping ancestor would cut it.
    ///
    /// gpui paints one border as several quads over the same bounds, so the
    /// result holds one quad per distinct band.
    pub fn ring_gaps(&self, background: gpui::Hsla, gap: f32) -> Vec<&gpui::Quad> {
        let mut seen: Vec<gpui::Bounds<gpui::ScaledPixels>> = Vec::new();
        self.quads
            .iter()
            .filter(|q| {
                q.border_color == background
                    && self.borders(q).iter().all(|w| (w - gap).abs() < 0.05)
            })
            .filter(|q| {
                let fresh = !seen.contains(&q.bounds);
                if fresh {
                    seen.push(q.bounds);
                }
                fresh
            })
            .collect()
    }

    /// Whether a clipping ancestor cuts `band`, a quad gpui paints as several
    /// slices over the same bounds (a rounded border): each slice carries its
    /// own sub-mask, so the band is whole when the union of its slices'
    /// masks covers its bounds.
    pub fn band_is_clipped(&self, band: &gpui::Quad) -> bool {
        let slices = self
            .quads
            .iter()
            .filter(|q| q.bounds == band.bounds && q.border_color == band.border_color);
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for q in slices {
            let m = self.mask(q);
            x0 = x0.min(f32::from(m.origin.x));
            y0 = y0.min(f32::from(m.origin.y));
            x1 = x1.max(f32::from(m.origin.x + m.size.width));
            y1 = y1.max(f32::from(m.origin.y + m.size.height));
        }
        let union = gpui::Bounds {
            origin: point(px(x0), px(y0)),
            size: gpui::size(px(x1 - x0), px(y1 - y0)),
        };
        !contains(union, self.bounds(band))
    }

    /// The quads filled with exactly `color`.
    pub fn filled(&self, color: gpui::Hsla) -> Vec<&gpui::Quad> {
        self.quads
            .iter()
            .filter(|q| q.background.as_solid() == Some(color))
            .collect()
    }

    /// Every solid fill, in paint order.
    pub fn solids(&self) -> Vec<gpui::Hsla> {
        self.quads
            .iter()
            .filter_map(|q| q.background.as_solid())
            .collect()
    }

    /// One line per quad (bounds, corners, fill, borders), for failure
    /// messages.
    pub fn describe(&self) -> String {
        self.quads
            .iter()
            .map(|q| {
                let b = self.bounds(q);
                format!(
                    "  ({:.1},{:.1} {:.1}x{:.1}) r={:?} bg={:?} border={:?}\n",
                    f32::from(b.origin.x),
                    f32::from(b.origin.y),
                    f32::from(b.size.width),
                    f32::from(b.size.height),
                    self.corners(q),
                    q.background.as_solid(),
                    self.borders(q),
                )
            })
            .collect()
    }

    /// The quads whose logical bounds contain `bounds` (within half a pixel).
    pub fn around(&self, bounds: gpui::Bounds<gpui::Pixels>) -> Vec<&gpui::Quad> {
        self.quads
            .iter()
            .filter(|q| contains(self.bounds(q), bounds))
            .collect()
    }
}

/// Two colours equal within float noise (a fade that settles through
/// `mix_oklab` lands a hair off the literal endpoint).
pub fn same_color(a: gpui::Hsla, b: gpui::Hsla) -> bool {
    let hue = (a.h - b.h).abs();
    (hue < 2e-3 || hue > 1. - 2e-3)
        && (a.s - b.s).abs() < 2e-3
        && (a.l - b.l).abs() < 2e-3
        && (a.a - b.a).abs() < 2e-3
}

/// Whether `fills` holds `color`, within [`same_color`]'s tolerance.
pub fn has_color(fills: &[gpui::Hsla], color: gpui::Hsla) -> bool {
    fills.iter().any(|c| same_color(*c, color))
}

/// Whether `outer` contains `inner`, within half a logical pixel.
pub fn contains(outer: gpui::Bounds<gpui::Pixels>, inner: gpui::Bounds<gpui::Pixels>) -> bool {
    let (ox, oy) = (f32::from(outer.origin.x), f32::from(outer.origin.y));
    let (ix, iy) = (f32::from(inner.origin.x), f32::from(inner.origin.y));
    ix >= ox - 0.5
        && iy >= oy - 0.5
        && ix + f32::from(inner.size.width) <= ox + f32::from(outer.size.width) + 0.5
        && iy + f32::from(inner.size.height) <= oy + f32::from(outer.size.height) + 0.5
}

/// What a [`style_probe`] saw: the text style inherited at its position.
pub type StyleSink = Rc<RefCell<Option<gpui::TextStyle>>>;

/// An empty [`StyleSink`].
pub fn style_sink() -> StyleSink {
    Rc::new(RefCell::new(None))
}

/// A 1px canvas that records the text style its ancestors composed at its
/// position. Put it in a caller-owned slot (children, content, a render
/// closure) and it reads what text in that slot would be drawn with.
pub fn style_probe(sink: &StyleSink) -> AnyElement {
    let sink = sink.clone();
    canvas(
        move |_, window, _| {
            *sink.borrow_mut() = Some(window.text_style());
        },
        |_, _, _, _| {},
    )
    .w(px(1.))
    .h(px(1.))
    .into_any_element()
}

/// The recorded style, panicking with `what` when the probe never painted.
pub fn seen_style(sink: &StyleSink, what: &str) -> gpui::TextStyle {
    sink.borrow()
        .clone()
        .unwrap_or_else(|| panic!("{what}: the style probe was never laid out"))
}
