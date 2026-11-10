//! Painted scrollbar — the GPUI stand-in for HeroUI's CSS `scrollbar` token.
//!
//! GPUI's `overflow_*_scroll` moves content and `scrollbar_width` only
//! reserves gutter space; it paints nothing. This overlay reads a
//! [`ScrollHandle`] and draws a thumb in `--scrollbar`. Overlay versus
//! always-visible follows [`gpui::App::should_auto_hide_scrollbars`],
//! unless [`Scrollbar::auto_hide`] pins the thumb visible the way a
//! styled webkit scrollbar is.
//!
//! HeroUI styles scrollbars through CSS, so there is no v3 prop to port:
//! the instance-level builders here ([`Scrollbar::track`],
//! [`Scrollbar::inset`], [`Scrollbar::radius`], [`Scrollbar::thumb_color`],
//! [`Scrollbar::thumb_hover_color`], [`Scrollbar::auto_hide`]) and the
//! [`Scrollbar::sx`] slot are repository extensions, following the same
//! `sx`-ownership contract as every other component.

use std::time::Duration;

use web_time::Instant;

use gpui::{
    canvas, div, prelude::*, px, App, Bounds, Div, ElementId, Hsla, IntoElement, MouseButton,
    MouseDownEvent, MouseMoveEvent, Pixels, Point, RenderOnce, ScrollHandle, Window,
};
use herogpui_core::{element_id, Orientation};
use herogpui_theme::ActiveTheme;

const THUMB_MIN: f32 = 24.0;
const TRACK: f32 = 8.0;
const IDLE: Duration = Duration::from_millis(800);

#[derive(Clone, Copy)]
struct Drag {
    pointer: f32,
    offset: f32,
}

#[derive(Clone)]
struct BarState {
    drag: Option<Drag>,
    hover: bool,
    last_offset: f32,
    last_moved: Option<Instant>,
}

impl Default for BarState {
    fn default() -> Self {
        Self {
            drag: None,
            hover: false,
            last_offset: 0.0,
            last_moved: None,
        }
    }
}

/// A painted overlay thumb bound to one [`ScrollHandle`].
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct Scrollbar {
    id: ElementId,
    handle: ScrollHandle,
    orientation: Orientation,
    /// Track thickness, in place of the private 8px default.
    track: Option<Pixels>,
    /// Shrink the painted thumb from every edge. `None` paints edge to edge.
    inset: Option<Pixels>,
    /// Thumb corner radius, in place of half the effective track.
    radius: Option<Pixels>,
    /// Thumb fill, in place of the `--scrollbar` token.
    thumb_color: Option<Hsla>,
    /// The fill the thumb takes while the pointer is anywhere over the bar's
    /// track — the thumb itself paints without a hitbox, so there is no
    /// thumb-only hover. `None` keeps the thumb static on hover.
    thumb_hover_color: Option<Hsla>,
    /// Whether the platform's auto-hide preference may hide the thumb.
    auto_hide: bool,
    /// The `sx` slot, refined over the root style at the end of render.
    sx: Option<Box<gpui::StyleRefinement>>,
}

impl Scrollbar {
    /// Creates a scrollbar for the given scroll handle.
    pub fn new(id: impl Into<ElementId>, handle: ScrollHandle) -> Self {
        Self {
            id: id.into(),
            handle,
            orientation: Orientation::Vertical,
            track: None,
            inset: None,
            radius: None,
            thumb_color: None,
            thumb_hover_color: None,
            auto_hide: true,
            sx: None,
        }
    }

    /// Sets the scrollbar orientation.
    pub fn orientation(mut self, orientation: Orientation) -> Self {
        self.orientation = orientation;
        self
    }

    /// The track thickness, in place of the 8px box this overlay has always
    /// drawn. The thumb's default corner radius derives from the effective
    /// thickness, so overriding the track alone re-derives the stock pill
    /// shape. A definite cross-axis `sx` size (`h` on a horizontal bar, `w`
    /// on a vertical one) wins over this builder, matching the shared
    /// theme-default → instance → `sx` order.
    pub fn track(mut self, thickness: impl Into<Pixels>) -> Self {
        self.track = Some(thickness.into());
        self
    }

    /// Shrink the painted thumb from every edge of its box — the webkit
    /// `background-clip: content-box` look. The default inset of 0 paints
    /// the thumb edge to edge, as today. The inset is visual only: hit
    /// testing and thumb length still use the unpainted box.
    pub fn inset(mut self, inset: impl Into<Pixels>) -> Self {
        self.inset = Some(inset.into());
        self
    }

    /// The thumb's corner radius, in place of half the effective track.
    /// Per-corner `sx` radii win over this builder corner by corner.
    /// Not a v3 prop; this overlay is a repository extension and this is a
    /// per-component repository extension in the same sense as
    /// `Button::radius`.
    pub fn radius(mut self, radius: impl Into<Pixels>) -> Self {
        self.radius = Some(radius.into());
        self
    }

    /// The thumb fill, in place of the `--scrollbar` token. An `sx`
    /// background outranks it: the thumb paints the bar's only fill, so per
    /// the `sx`-ownership contract the override is its resting colour.
    pub fn thumb_color(mut self, color: impl Into<Hsla>) -> Self {
        self.thumb_color = Some(color.into());
        self
    }

    /// The fill the thumb takes while the pointer is anywhere over the bar's
    /// track, in place of today's static colour — the overlay never restyled
    /// on hover before this builder, and leaving it unset keeps exactly
    /// that. Unlike a webkit `::-webkit-scrollbar-thumb:hover` rule, the
    /// swap is not thumb-only: the thumb paints at paint time with no
    /// hitbox of its own, so the gate is the track root's hover. The swap
    /// is also immediate, not a transition — the thumb is painted from the
    /// scroll handle outside gpui's style hover system.
    pub fn thumb_hover_color(mut self, color: impl Into<Hsla>) -> Self {
        self.thumb_hover_color = Some(color.into());
        self
    }

    /// Whether the OS preference may hide the thumb when the content is
    /// idle. `auto_hide(false)` pins the thumb visible the way a styled
    /// webkit scrollbar is, bypassing
    /// [`gpui::App::should_auto_hide_scrollbars`]; `true` or unset keeps the
    /// stock behavior.
    pub fn auto_hide(mut self, v: bool) -> Self {
        self.auto_hide = v;
        self
    }

    /// The one slot for caller-owned low-level styling: GPUI's styling
    /// methods (`bg`, `w`, `h`, `rounded`, …) applied to the track root
    /// after every value the theme and the instance builders chose, so they
    /// win. Per the shared `sx`-ownership contract the thumb — the bar's
    /// only painted part — reads the override back: an `sx` background is
    /// the thumb's resting fill (it also paints the transparent gutter, so
    /// [`Scrollbar::thumb_color`] is the seam that moves only the thumb), a
    /// definite cross-axis `w`/`h` is the track thickness, and per-corner
    /// radii shape the thumb.
    pub fn sx(mut self, style: impl FnOnce(Div) -> Div) -> Self {
        crate::util::refine_sx(&mut self.sx, style);
        self
    }
}

impl RenderOnce for Scrollbar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let handle = self.handle;
        let horizontal = self.orientation.is_horizontal();
        let max = if horizontal {
            f32::from(handle.max_offset().x)
        } else {
            f32::from(handle.max_offset().y)
        };
        if max < 1.0 {
            return div().into_any_element();
        }

        let offset = if horizontal {
            f32::from(handle.offset().x)
        } else {
            f32::from(handle.offset().y)
        };
        let state = window.use_keyed_state(element_id::scoped(&self.id, "bar"), cx, |_, _| {
            BarState::default()
        });
        let snapshot = state.read(cx).clone();
        if (snapshot.last_offset - offset).abs() > 0.5 {
            state.update(cx, |s, _| {
                s.last_offset = offset;
                s.last_moved = Some(Instant::now());
            });
        }

        let platform_auto_hide = cx.should_auto_hide_scrollbars();
        let live = state.read(cx);
        let recently_moved = live.last_moved.is_some_and(|at| at.elapsed() < IDLE);
        let show_thumb = thumb_visible(
            self.auto_hide,
            platform_auto_hide,
            live.hover,
            live.drag.is_some(),
            recently_moved,
        );
        let hovered = live.hover;
        let token = cx.colors().scrollbar;

        // `sx` ownership (components.md): the thumb paints the bar's only
        // fill, so the `sx` background is its resting colour — ahead of the
        // instance override — and the hover endpoint is resolved against
        // that resting value, so a bare `sx` background cannot be eased
        // back over.
        let sx_background = crate::util::sx_background(&self.sx);
        let resting = sx_background.or(self.thumb_color).unwrap_or(token);
        let (idle_fill, hover_fill) = crate::util::fade_endpoints(
            Some((resting, resting)),
            sx_background,
            self.thumb_hover_color,
        )
        .unwrap_or((resting, resting));
        let thumb_fill = if hovered { hover_fill } else { idle_fill };

        // The track box: a definite cross-axis `sx` size is the thickness,
        // then the instance `track`, then the private const. The default
        // radius derives from the effective thickness, which is what keeps
        // the stock pill shape when only the track is overridden.
        let sx_size = crate::util::sx_pixel_size(&self.sx);
        let thickness = if horizontal {
            sx_size.height.or(self.track).unwrap_or_else(|| px(TRACK))
        } else {
            sx_size.width.or(self.track).unwrap_or_else(|| px(TRACK))
        };
        let default_radius = px(f32::from(thickness) / 2.0);
        let corners = crate::util::fill_unspecified_corners(
            crate::util::sx_radius(&self.sx),
            self.radius.or(Some(default_radius)),
        );
        let corner_radii = gpui::Corners {
            top_left: corners.top_left.unwrap_or(default_radius),
            top_right: corners.top_right.unwrap_or(default_radius),
            bottom_right: corners.bottom_right.unwrap_or(default_radius),
            bottom_left: corners.bottom_left.unwrap_or(default_radius),
        };
        let inset = self.inset.unwrap_or_else(|| px(0.));
        let axis = self.orientation;
        let handle_move = handle.clone();
        let drag_state = state.clone();
        let hover_state = state.clone();
        let leave_state = state.clone();

        let mut track = div()
            .id(self.id)
            .absolute()
            .overflow_hidden()
            .when(horizontal, |el| {
                el.left_0().right_0().bottom_0().h(thickness)
            })
            .when(!horizontal, |el| {
                el.top_0().bottom_0().right_0().w(thickness)
            })
            .on_mouse_down(MouseButton::Left, {
                let handle = handle.clone();
                let state = state.clone();
                move |ev: &MouseDownEvent, window, cx| {
                    jump_or_grab(&handle, axis, ev.position, &state, window, cx);
                }
            })
            .on_mouse_move({
                let handle = handle_move;
                let state = drag_state;
                move |ev: &MouseMoveEvent, window, cx| {
                    drag_to(&handle, axis, ev.position, &state, window, cx);
                }
            })
            .on_mouse_up(MouseButton::Left, {
                let state = state.clone();
                move |_, _, cx| {
                    state.update(cx, |s, cx| {
                        s.drag = None;
                        cx.notify();
                    });
                }
            })
            .on_hover(move |inside, _, cx| {
                if *inside {
                    hover_state.update(cx, |s, cx| {
                        s.hover = true;
                        cx.notify();
                    });
                } else {
                    leave_state.update(cx, |s, cx| {
                        s.hover = false;
                        cx.notify();
                    });
                }
            });

        if show_thumb {
            let paint_handle = handle;
            track = track.child(
                canvas(
                    move |bounds, _, _| bounds,
                    move |bounds, _, window, _| {
                        if let Some(thumb_bounds) = thumb_bounds(&paint_handle, axis, bounds) {
                            let painted = inset_bounds(thumb_bounds, inset);
                            window.paint_quad(
                                gpui::fill(painted, thumb_fill).corner_radii(corner_radii),
                            );
                        }
                    },
                )
                .size_full(),
            );
        }

        crate::util::apply_sx(track, &self.sx).into_any_element()
    }
}

/// Whether the thumb paints this frame.
///
/// `auto_hide` is the caller's [`Scrollbar::auto_hide`]; when it is `false`
/// the platform preference is bypassed outright. Otherwise a platform that
/// auto-hides still shows the thumb while the pointer is over the bar, a
/// drag is live, or the content moved recently.
fn thumb_visible(
    auto_hide: bool,
    platform_auto_hide: bool,
    hover: bool,
    dragging: bool,
    recently_moved: bool,
) -> bool {
    !auto_hide || !platform_auto_hide || hover || dragging || recently_moved
}

/// Shrinks a thumb box by `inset` on every side, collapsing to an empty box
/// rather than a negative-sized one when the inset exhausts an axis.
fn inset_bounds(bounds: Bounds<Pixels>, inset: Pixels) -> Bounds<Pixels> {
    let inset = f32::from(inset);
    Bounds {
        origin: Point {
            x: bounds.origin.x + px(inset),
            y: bounds.origin.y + px(inset),
        },
        size: gpui::Size {
            width: px((f32::from(bounds.size.width) - 2.0 * inset).max(0.0)),
            height: px((f32::from(bounds.size.height) - 2.0 * inset).max(0.0)),
        },
    }
}

fn axis_size(bounds: Bounds<Pixels>, horizontal: bool) -> f32 {
    if horizontal {
        f32::from(bounds.size.width)
    } else {
        f32::from(bounds.size.height)
    }
}

fn axis_origin(bounds: Bounds<Pixels>, horizontal: bool) -> f32 {
    if horizontal {
        f32::from(bounds.origin.x)
    } else {
        f32::from(bounds.origin.y)
    }
}

fn thumb_metrics(
    handle: &ScrollHandle,
    axis: Orientation,
    track: Bounds<Pixels>,
) -> Option<(f32, f32, f32)> {
    let horizontal = axis.is_horizontal();
    let max = if horizontal {
        f32::from(handle.max_offset().x)
    } else {
        f32::from(handle.max_offset().y)
    };
    if max < 1.0 {
        return None;
    }
    let offset = if horizontal {
        f32::from(handle.offset().x)
    } else {
        f32::from(handle.offset().y)
    };
    let track_len = axis_size(track, horizontal);
    if track_len < 1.0 {
        return None;
    }
    let content = track_len + max;
    let thumb_len = (track_len * (track_len / content))
        .max(THUMB_MIN)
        .min(track_len);
    let travel = (track_len - thumb_len).max(0.0);
    let t = (-offset / max).clamp(0.0, 1.0);
    Some((thumb_len, travel * t, max))
}

fn thumb_bounds(
    handle: &ScrollHandle,
    axis: Orientation,
    track: Bounds<Pixels>,
) -> Option<Bounds<Pixels>> {
    let (thumb_len, thumb_start, _) = thumb_metrics(handle, axis, track)?;
    let horizontal = axis.is_horizontal();
    Some(if horizontal {
        Bounds {
            origin: Point {
                x: track.origin.x + px(thumb_start),
                y: track.origin.y,
            },
            size: gpui::Size {
                width: px(thumb_len),
                height: track.size.height,
            },
        }
    } else {
        Bounds {
            origin: Point {
                x: track.origin.x,
                y: track.origin.y + px(thumb_start),
            },
            size: gpui::Size {
                width: track.size.width,
                height: px(thumb_len),
            },
        }
    })
}

fn jump_or_grab(
    handle: &ScrollHandle,
    axis: Orientation,
    pointer: Point<Pixels>,
    state: &gpui::Entity<BarState>,
    window: &mut Window,
    cx: &mut App,
) {
    let track = handle.bounds();
    let Some((thumb_len, thumb_start, max)) = thumb_metrics(handle, axis, track) else {
        return;
    };
    let horizontal = axis.is_horizontal();
    let pointer_v = if horizontal {
        f32::from(pointer.x)
    } else {
        f32::from(pointer.y)
    };
    let origin = axis_origin(track, horizontal);
    let rel = pointer_v - origin;
    let offset = if horizontal {
        f32::from(handle.offset().x)
    } else {
        f32::from(handle.offset().y)
    };
    if rel >= thumb_start && rel <= thumb_start + thumb_len {
        state.update(cx, |s, cx| {
            s.drag = Some(Drag {
                pointer: pointer_v,
                offset,
            });
            cx.notify();
        });
        return;
    }
    let travel = (axis_size(track, horizontal) - thumb_len).max(1.0);
    let t = ((rel - thumb_len / 2.0) / travel).clamp(0.0, 1.0);
    set_axis_offset(handle, axis, -t * max);
    state.update(cx, |s, cx| {
        s.last_offset = -t * max;
        s.last_moved = Some(Instant::now());
        cx.notify();
    });
    let _ = window;
}

fn drag_to(
    handle: &ScrollHandle,
    axis: Orientation,
    pointer: Point<Pixels>,
    state: &gpui::Entity<BarState>,
    _window: &mut Window,
    cx: &mut App,
) {
    let Some(drag) = state.read(cx).drag else {
        return;
    };
    let track = handle.bounds();
    let Some((thumb_len, _, max)) = thumb_metrics(handle, axis, track) else {
        return;
    };
    let horizontal = axis.is_horizontal();
    let pointer_v = if horizontal {
        f32::from(pointer.x)
    } else {
        f32::from(pointer.y)
    };
    let travel = (axis_size(track, horizontal) - thumb_len).max(1.0);
    let delta = (pointer_v - drag.pointer) / travel * max;
    let next = (drag.offset - delta).clamp(-max, 0.0);
    set_axis_offset(handle, axis, next);
    state.update(cx, |s, cx| {
        s.last_offset = next;
        s.last_moved = Some(Instant::now());
        cx.notify();
    });
}

fn set_axis_offset(handle: &ScrollHandle, axis: Orientation, value: f32) {
    let mut point = handle.offset();
    if axis.is_horizontal() {
        point.x = px(value);
    } else {
        point.y = px(value);
    }
    handle.set_offset(point);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The visibility decision, table-tested because the headless test
    /// platform always answers `should_auto_hide_scrollbars()` with `false`
    /// and cannot be flipped — the platform-hiding path is only reachable
    /// here.
    #[test]
    fn the_visibility_decision_follows_the_platform_and_the_caller() {
        // Platform asks for auto-hide, caller left the default: the thumb
        // only paints while the pointer is over the bar, a drag is live, or
        // the content just moved.
        assert!(!thumb_visible(true, true, false, false, false));
        assert!(thumb_visible(true, true, true, false, false));
        assert!(thumb_visible(true, true, false, true, false));
        assert!(thumb_visible(true, true, false, false, true));
        // `auto_hide(false)` bypasses the platform entirely.
        assert!(thumb_visible(false, true, false, false, false));
        // A platform that never auto-hides keeps the always-on behavior.
        assert!(thumb_visible(true, false, false, false, false));
        assert!(thumb_visible(false, false, false, false, false));
    }

    #[test]
    fn inset_shrinks_every_edge_without_going_negative() {
        let bounds = Bounds {
            origin: Point {
                x: px(10.0),
                y: px(20.0),
            },
            size: gpui::Size {
                width: px(8.0),
                height: px(40.0),
            },
        };
        let untouched = inset_bounds(bounds, px(0.0));
        assert_eq!(untouched.origin.x, px(10.0));
        assert_eq!(untouched.size.width, px(8.0));
        assert_eq!(untouched.size.height, px(40.0));

        let shrunk = inset_bounds(bounds, px(2.0));
        assert_eq!(shrunk.origin.x, px(12.0));
        assert_eq!(shrunk.origin.y, px(22.0));
        assert_eq!(shrunk.size.width, px(4.0));
        assert_eq!(shrunk.size.height, px(36.0));

        let collapsed = inset_bounds(bounds, px(30.0));
        assert_eq!(collapsed.size.width, px(0.0));
        assert_eq!(collapsed.size.height, px(0.0));
    }
}

crate::util::impl_component_styled!(Scrollbar);
