//! ScrollShadow — port of `@heroui/scroll-shadow` (v3).
//!
//! A scrollable container with soft fading edges. Mirrors the React API:
//! `orientation`, `variant`, `size`, `offset`, `hideScrollBar`, `isEnabled`
//! and `visibility`.

use gpui::{
    canvas, div, prelude::*, px, AnyElement, App, ElementId, IntoElement, ParentElement, Pixels,
    RenderOnce, ScrollHandle, Styled, Window,
};
use herogpui_core::{element_id, Orientation};
use herogpui_theme::ActiveTheme;

/// The shadow effect style. v3 ships one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScrollShadowVariant {
    /// A fading shadow.
    #[default]
    Fade,
}

/// Which edges show a shadow (`visibility`).
///
/// `Auto` is derived from the live scroll offset: a tracked `ScrollHandle`
/// reports where the content sits, so the leading fade appears once it has been
/// scrolled away from the start and the trailing one goes when the end is
/// reached. The offset is a frame behind -- gpui fills the handle during
/// prepaint -- which `onVisibilityChange` reports from a canvas in the same
/// frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScrollShadowVisibility {
    /// Derived from the scroll position.
    #[default]
    Auto,
    /// Both edges of the scroll axis.
    Both,
    /// Shadow at the top edge only.
    Top,
    /// Shadow at the bottom edge only.
    Bottom,
    /// Shadow at the left edge only.
    Left,
    /// Shadow at the right edge only.
    Right,
    /// No shadow.
    None,
}

impl ScrollShadowVisibility {
    /// Whether the leading edge (top / left) should be shaded.
    fn shows_start(self, orientation: Orientation) -> bool {
        match self {
            // `Auto` is resolved before this is asked; `Both` is unconditional.
            Self::Auto | Self::Both => true,
            Self::Top => !orientation.is_horizontal(),
            Self::Left => orientation.is_horizontal(),
            _ => false,
        }
    }

    /// Whether the trailing edge (bottom / right) should be shaded.
    fn shows_end(self, orientation: Orientation) -> bool {
        match self {
            Self::Auto | Self::Both => true,
            Self::Bottom => !orientation.is_horizontal(),
            Self::Right => orientation.is_horizontal(),
            _ => false,
        }
    }
}

/// HeroUI ScrollShadow.
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct ScrollShadow {
    id: ElementId,
    orientation: Orientation,
    /// Gradient depth in pixels (`size`, default 40).
    size: Pixels,
    /// Scroll distance before the shadow appears (`offset`).
    offset: Pixels,
    is_enabled: bool,
    visibility: ScrollShadowVisibility,
    /// `onVisibilityChange` — reports the edges that are shaded, whenever that
    /// changes.
    on_visibility_change:
        Option<std::sync::Arc<dyn Fn(&ScrollShadowVisibility, &mut Window, &mut App) + 'static>>,
    max_h: Option<Pixels>,
    max_w: Option<Pixels>,
    gap: Pixels,
    children: Vec<AnyElement>,
    hide_scroll_bar: bool,
    /// Makes the scroll axis fill its parent. Standalone shadows keep their
    /// intrinsic/max-constrained sizing; composed controls opt into a bounded
    /// viewport when they own an adjacent panel.
    fill_axis: bool,
    /// An owner can provide its existing scroll handle so edge shadows and
    /// external controls observe the same offset and max range.
    external_scroll: Option<ScrollHandle>,
    /// The `sx` slot, refined over the root style at the end of render.
    sx: Option<Box<gpui::StyleRefinement>>,
}

impl ScrollShadow {
    /// Creates a scroll shadow with the given element id.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            orientation: Orientation::Vertical,
            size: px(40.),
            offset: px(0.),
            is_enabled: true,
            visibility: ScrollShadowVisibility::Auto,
            on_visibility_change: None,
            max_h: Some(px(240.)),
            max_w: None,
            gap: px(8.),
            children: Vec::new(),
            hide_scroll_bar: false,
            fill_axis: false,
            external_scroll: None,
            sx: None,
        }
    }

    /// `hideScrollBar` — omit the painted overlay thumb. GPUI has no native
    /// scrollbar on a overflow div; this is the port of v3's CSS hide.
    pub fn hide_scroll_bar(mut self, v: bool) -> Self {
        self.hide_scroll_bar = v;
        self
    }

    /// Reuses a caller-owned scroll handle for composed controls.
    pub(crate) fn scroll_handle(mut self, handle: ScrollHandle) -> Self {
        self.external_scroll = Some(handle);
        self
    }

    /// Sets the scroll direction (v3 `orientation`).
    pub fn orientation(mut self, orientation: Orientation) -> Self {
        self.orientation = orientation;
        self
    }

    /// Gradient depth in pixels.
    pub fn size(mut self, size: impl Into<Pixels>) -> Self {
        self.size = size.into();
        self
    }

    /// Sets the scroll distance before the shadow appears (v3 `offset`).
    pub fn offset(mut self, offset: impl Into<Pixels>) -> Self {
        self.offset = offset.into();
        self
    }

    /// Turns shadow rendering off while keeping the scroll behaviour.
    pub fn is_enabled(mut self, v: bool) -> Self {
        self.is_enabled = v;
        self
    }

    /// `onVisibilityChange` — fires when the shaded edges change, with the
    /// resolved visibility (never `Auto`: the resolved value is what changed).
    pub fn on_visibility_change(
        mut self,
        handler: impl Fn(&ScrollShadowVisibility, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_visibility_change = Some(std::sync::Arc::new(handler));
        self
    }

    /// Sets which edges show a shadow (v3 `visibility`).
    pub fn visibility(mut self, visibility: ScrollShadowVisibility) -> Self {
        self.visibility = visibility;
        self
    }

    /// Removes the standalone 240px height cap when the caller supplies a
    /// full-height viewport.
    pub(crate) fn unbounded_h(mut self) -> Self {
        self.max_h = None;
        self
    }

    pub(crate) fn fill_axis(mut self, value: bool) -> Self {
        self.fill_axis = value;
        self
    }

    /// Sets the maximum height.
    pub fn max_h(mut self, v: impl Into<Pixels>) -> Self {
        self.max_h = Some(v.into());
        self
    }

    /// Sets the maximum width.
    pub fn max_w(mut self, v: impl Into<Pixels>) -> Self {
        self.max_w = Some(v.into());
        self
    }

    /// Gap between children inside the scroll area.
    pub fn gap(mut self, gap: impl Into<Pixels>) -> Self {
        self.gap = gap.into();
        self
    }

    /// The one slot for caller-owned low-level styling: GPUI's styling methods
    /// (`bg`, `text_color`, `w`, `h`, `p`, `rounded`, `border_color`, …)
    /// applied to the scroll shadow's root element after every value the
    /// orientation, the size and the active theme chose, so they win.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        crate::util::refine_sx(&mut self.sx, style);
        self
    }
}

impl ParentElement for ScrollShadow {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for ScrollShadow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // Where the content sits, as of the last frame: `use_keyed_state` takes
        // `cx` mutably, so both slots precede the theme borrow.
        let scroll = self.external_scroll.clone().unwrap_or_else(|| {
            window
                .use_keyed_state(element_id::scoped(&self.id, "scroll"), cx, |_, _| {
                    ScrollHandle::new()
                })
                .read(cx)
                .clone()
        });
        let reported = window.use_keyed_state(
            element_id::scoped(&self.id, "shadow-visibility"),
            cx,
            |_, _| ScrollShadowVisibility::None,
        );
        let bg = cx.colors().background;
        let horizontal = self.orientation.is_horizontal();

        let mut scroller = div()
            .id(self.id.clone())
            .track_scroll(&scroll)
            .restrict_scroll_to_axis()
            .overflow_hidden()
            .flex()
            .gap(self.gap);

        scroller = if horizontal {
            scroller.flex_row().overflow_x_scroll()
        } else {
            scroller.flex_col().overflow_y_scroll()
        };

        // A mouse wheel reports only `deltaY`, and the web platform forwards
        // the raw axes, so shift+wheel never reaches a horizontal scroller
        // there (native platforms translate it themselves). Until that lands
        // upstream, drive the handle's x offset from the vertical delta here;
        // see `util::shift_wheel_scroll_x`. `restrict_scroll_to_axis` already
        // drops the vertical component from `track_scroll`, so the two do not
        // double-scroll. Native builds keep flowing through `track_scroll`
        // untouched: the helper is a web-only no-op elsewhere.
        if horizontal {
            let wheel_scroll = scroll.clone();
            scroller = scroller.on_scroll(move |event, window, _cx| {
                crate::util::shift_wheel_scroll_x(&wheel_scroll, event, window);
            });
        }

        if self.fill_axis {
            scroller = if horizontal {
                scroller.w_full().min_w(px(0.))
            } else {
                scroller.h_full().min_h(px(0.))
            };
        }

        if let Some(h) = self.max_h {
            scroller = scroller.max_h(h);
        }
        if let Some(w) = self.max_w {
            scroller = scroller.max_w(w);
        }

        scroller = scroller.children(self.children);

        let bar = (!self.hide_scroll_bar).then(|| {
            crate::scrollbar::Scrollbar::new(
                element_id::scoped(&self.id, "scrollbar"),
                scroll.clone(),
            )
            .orientation(self.orientation)
        });

        if !self.is_enabled || self.visibility == ScrollShadowVisibility::None {
            let mut root = div().relative().child(scroller);
            if let Some(bar) = bar {
                root = root.child(bar);
            }
            return crate::util::apply_sx(root, &self.sx).into_any_element();
        }

        // `Auto`: the leading fade once the content has been scrolled away from
        // the start, the trailing one until the end is reached. `offset` counts
        // *backwards* from zero, and `max_offset` is how far it can go.
        //
        // The scroller's wheel listener adds the delta straight into the
        // tracked handle's offset cell during event dispatch; layout clamps it
        // into `[-max, 0]` only on the next pass. A wheel over a box whose
        // content fits (`max` = 0) would therefore read as a one-frame -40px
        // offset here and resolve `Auto` to a spurious one-frame edge shadow.
        // Clamp what this render reads to the same range layout clamps to:
        // `Auto` must never report an edge while the offset sits outside the
        // scrollable range, which is exactly the "nothing to scroll" case —
        // v3's contract is that content which fits shows no shadow, ever.
        let offset = scroll.offset();
        let max = scroll.max_offset();
        let (scrolled, scroll_max) = if horizontal {
            (
                f32::from(offset.x).clamp(-f32::from(max.x), 0.0),
                f32::from(max.x),
            )
        } else {
            (
                f32::from(offset.y).clamp(-f32::from(max.y), 0.0),
                f32::from(max.y),
            )
        };
        let (past_start, before_end) = (
            scrolled < -f32::from(self.offset),
            scrolled - f32::from(self.offset) > -scroll_max,
        );
        let resolved = if self.visibility == ScrollShadowVisibility::Auto {
            match (past_start, before_end) {
                (true, true) => ScrollShadowVisibility::Both,
                (true, false) if horizontal => ScrollShadowVisibility::Left,
                (true, false) => ScrollShadowVisibility::Top,
                (false, true) if horizontal => ScrollShadowVisibility::Right,
                (false, true) => ScrollShadowVisibility::Bottom,
                (false, false) => ScrollShadowVisibility::None,
            }
        } else {
            self.visibility
        };
        let reports_visibility = self.visibility == ScrollShadowVisibility::Auto;

        // The fades are absolutely positioned siblings so they do not scroll
        // with the content.
        // Fade to the *same* colour at zero alpha. Interpolating toward
        // `transparent_black` would drag the midpoint through grey.
        let clear = bg.alpha(0.0);
        let fade = |from_start: bool| {
            let stops = if from_start { (bg, clear) } else { (clear, bg) };
            let angle = if horizontal { 90.0 } else { 180.0 };
            let mut el = div().absolute().bg(gpui::linear_gradient(
                angle,
                gpui::linear_color_stop(stops.0, 0.0),
                gpui::linear_color_stop(stops.1, 1.0),
            ));
            el = if horizontal {
                el.top_0().bottom_0().w(self.size)
            } else {
                el.left_0().right_0().h(self.size)
            };
            match (horizontal, from_start) {
                (true, true) => el.left(self.offset),
                (true, false) => el.right(self.offset),
                (false, true) => el.top(self.offset),
                (false, false) => el.bottom(self.offset),
            }
        };

        let root = div()
            .relative()
            .child(scroller)
            .when(resolved.shows_start(self.orientation), |el| {
                el.child(fade(true))
            })
            .when(resolved.shows_end(self.orientation), |el| {
                el.child(fade(false))
            })
            .when_some(bar, |el, bar| el.child(bar))
            .when(reports_visibility, |el| {
                // The offset is written during prepaint, so what changed is
                // known here and reported from here.
                let handler = self.on_visibility_change.clone();
                el.child(
                    canvas(
                        move |_, window, cx| {
                            if *reported.read(cx) != resolved {
                                reported.update(cx, |value, cx| {
                                    *value = resolved;
                                    cx.notify();
                                });
                                if let Some(f) = &handler {
                                    f(&resolved, window, cx);
                                }
                            }
                        },
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .size(px(0.)),
                )
            });
        crate::util::apply_sx(root, &self.sx).into_any_element()
    }
}

crate::util::impl_component_styled!(ScrollShadow);
