//! Slider — HeroUI `slider.css` horizontal track plus the app's compact
//! `slider--sm` variant used by the editor panels (6px track, 12px knob).

use std::rc::Rc;

use gpui::{
    canvas, div, prelude::*, px, App, DispatchPhase, ElementId, KeyDownEvent, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Stateful, Styled, Window,
};

use crate::theme::vars::{active_theme, ThemeVars};
use crate::ui::chrome;

type ChangeHandler = Rc<dyn Fn(&f32, &mut Window, &mut App)>;
type DragHandler = Rc<dyn Fn(&mut Window, &mut App)>;

fn snap_to_step(value: f32, steps: &[f32]) -> f32 {
    steps
        .iter()
        .copied()
        .min_by(|left, right| (left - value).abs().total_cmp(&(right - value).abs()))
        .unwrap_or(value)
}

fn adjacent_step(current: f32, steps: &[f32], offset: isize) -> f32 {
    let Some(last) = steps.len().checked_sub(1) else {
        return current;
    };
    let index = steps
        .iter()
        .enumerate()
        .min_by(|(_, left), (_, right)| {
            (**left - current)
                .abs()
                .total_cmp(&(**right - current).abs())
        })
        .map(|(index, _)| index)
        .unwrap_or(0);
    let next = index.saturating_add_signed(offset).min(last);
    steps[next]
}

/// How far the fill and knob are inset from each end of the track.
///
/// HeroUI reserves half a knob at each end with a transparent
/// `border-x-[0.75rem]` on the track (`0.5rem` for `.slider--sm`), which is
/// what keeps the knob inside the track rather than overhanging it.
fn content_inset(knob_width: f32) -> f32 {
    knob_width / 2.0
}

#[cfg_attr(not(test), allow(dead_code))]
/// Where the knob's left edge sits relative to the track's left edge: flush at
/// the minimum, flush at the other end at the maximum.
///
/// This is the model; [`knob_offset_from_layout`] is the same quantity as the
/// renderer actually composes it, and the two are asserted to agree.
fn knob_offset(track_width: f32, knob_width: f32, fraction: f32) -> f32 {
    (track_width - knob_width).max(0.0) * fraction.clamp(0.0, 1.0)
}

#[cfg_attr(not(test), allow(dead_code))]
/// The knob offset the rendered element tree produces: the inner box starts at
/// `content_inset`, the knob is placed at `fraction` of that box's width and
/// then pulled back by `content_inset`.
fn knob_offset_from_layout(track_width: f32, knob_width: f32, fraction: f32) -> f32 {
    let inset = content_inset(knob_width);
    let inner_width = (track_width - inset * 2.0).max(0.0);
    inset + inner_width * fraction.clamp(0.0, 1.0) - inset
}

fn fraction_at_position(track_width: f32, knob_width: f32, position: f32) -> f32 {
    let inset = content_inset(knob_width);
    let inner_width = (track_width - inset * 2.0).max(0.0);
    if inner_width == 0.0 {
        return 0.0;
    }
    ((position - inset) / inner_width).clamp(0.0, 1.0)
}

#[derive(IntoElement)]
pub struct Slider {
    id: ElementId,
    value: f32,
    min: f32,
    max: f32,
    disabled: bool,
    /// Compact variant from `base.css` `.slider--sm`.
    small: bool,
    steps: Vec<f32>,
    on_change: Option<ChangeHandler>,
    on_drag_start: Option<DragHandler>,
    on_drag_end: Option<DragHandler>,
}

impl Slider {
    pub fn new(id: impl Into<ElementId>, value: f32, min: f32, max: f32) -> Self {
        Self {
            id: id.into(),
            value,
            min,
            max,
            disabled: false,
            small: false,
            steps: Vec::new(),
            on_change: None,
            on_drag_start: None,
            on_drag_end: None,
        }
    }

    pub fn small(mut self) -> Self {
        self.small = true;
        self
    }

    #[allow(dead_code)]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn on_change(mut self, handler: impl Fn(&f32, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }

    pub fn steps(mut self, steps: impl IntoIterator<Item = f32>) -> Self {
        self.steps = steps.into_iter().collect();
        self
    }

    pub fn on_drag_start(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_drag_start = Some(Rc::new(handler));
        self
    }

    pub fn on_drag_end(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_drag_end = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for Slider {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = active_theme(cx);
        render_track(self, &theme, window, cx)
    }
}

fn render_track(
    slider: Slider,
    theme: &ThemeVars,
    window: &mut Window,
    cx: &mut App,
) -> Stateful<gpui::Div> {
    let track_height = if slider.small {
        px(chrome::SLIDER_SM_TRACK)
    } else {
        px(chrome::SLIDER_TRACK)
    };
    let span = slider.max - slider.min;
    let fraction = if span > 0.0 {
        ((slider.value - slider.min) / span).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let (knob_w, knob_h): (Pixels, Pixels) = if slider.small {
        (px(chrome::SLIDER_SM_KNOB), px(chrome::SLIDER_SM_KNOB))
    } else {
        (
            px(chrome::SLIDER_KNOB_WIDTH),
            px(chrome::SLIDER_KNOB_HEIGHT),
        )
    };
    let knob_border = if slider.small { Pixels::ZERO } else { px(2.0) };
    let element_key = format!("{}", slider.id);
    let focus = crate::ui::primitives::control_focus(
        &element_key,
        slider.disabled || span <= 0.0,
        window,
        cx,
    );

    // HeroUI reserves room for the knob at both ends with a transparent
    // `border-x-[0.75rem]` on the track (`0.5rem` for `.slider--sm`) — half the
    // knob width — and positions the fill and thumb inside that content box, so
    // the knob never overhangs the track. This mirrors the same structure: an
    // inner box inset by half a knob carries the fill and the knob, and the
    // strip the border used to cover is painted as the fill's start cap
    // (HeroUI's `border-s-accent` when `data-fill-start` is set).
    let inset = px(content_inset(f32::from(knob_w)));
    let track: Stateful<gpui::Div> = div()
        .id(slider.id.clone())
        .track_focus(&focus)
        .focus(|style| style.shadow(crate::ui::primitives::focus_ring(theme, 2.0)))
        .relative()
        .w_full()
        .h(track_height)
        .rounded_full()
        .bg(theme.default)
        .when(fraction > 0.0, |el| {
            el.child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .h_full()
                    .w(inset)
                    // The cap sits on the pill's rounded end, so it has to
                    // carry that corner itself — the small track's knob is
                    // taller than the track, so the track cannot clip.
                    .rounded_l(track_height / 2.0)
                    .bg(theme.accent),
            )
        })
        .child(
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left(inset)
                .right(inset)
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .h_full()
                        .bg(theme.accent)
                        .w(gpui::relative(fraction)),
                )
                .child(
                    // Inside the inset box the knob is centred on the fill
                    // edge, which puts its left edge at `knob_offset` from the
                    // track's own left edge.
                    div()
                        .absolute()
                        .top((track_height - knob_h - knob_border * 2.0) / 2.0)
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_full()
                        .w(knob_w + knob_border * 2.0)
                        .h(knob_h + knob_border * 2.0)
                        .ml(-inset - knob_border)
                        .bg(if slider.small {
                            theme.foreground
                        } else {
                            theme.accent
                        })
                        .left(gpui::relative(fraction))
                        .when(!slider.small, |el| {
                            el.child(
                                div()
                                    .rounded_full()
                                    .w(knob_w)
                                    .h(knob_h)
                                    .bg(theme.accent_foreground),
                            )
                        }),
                ),
        );

    if slider.disabled || span <= 0.0 {
        return track;
    }
    let Some(handler) = slider.on_change else {
        return track;
    };

    let min = slider.min;
    let current = slider.value;
    let key_handler = handler.clone();
    let max = slider.max;
    let steps = Rc::<[f32]>::from(slider.steps);
    let apply = Rc::new({
        let handler = handler.clone();
        let steps = steps.clone();
        move |bounds: gpui::Bounds<Pixels>,
              position: gpui::Point<Pixels>,
              window: &mut Window,
              cx: &mut App| {
            let width = bounds.size.width;
            if width <= Pixels::ZERO {
                return;
            }
            let fraction = fraction_at_position(
                f32::from(width),
                f32::from(knob_w),
                f32::from(position.x - bounds.left()),
            );
            let value = snap_to_step(min + fraction * span, &steps);
            handler(&value, window, cx);
        }
    });

    let drag_start = slider.on_drag_start;
    let drag_end = slider.on_drag_end;
    let key_steps = steps.clone();
    let track = track.on_key_down(move |event: &KeyDownEvent, window, cx| {
        let step = span / 100.0;
        let next = match (event.keystroke.key.as_str(), key_steps.is_empty()) {
            ("left" | "down", false) => Some(adjacent_step(current, &key_steps, -1)),
            ("right" | "up", false) => Some(adjacent_step(current, &key_steps, 1)),
            ("pageup", false) => Some(adjacent_step(current, &key_steps, 2)),
            ("pagedown", false) => Some(adjacent_step(current, &key_steps, -2)),
            ("home", false) => key_steps.first().copied(),
            ("end", false) => key_steps.last().copied(),
            ("left" | "down", true) => Some(current - step),
            ("right" | "up", true) => Some(current + step),
            ("pageup", true) => Some(current + step * 10.0),
            ("pagedown", true) => Some(current - step * 10.0),
            ("home", true) => Some(min),
            ("end", true) => Some(max),
            _ => None,
        };
        if let Some(next) = next {
            key_handler(&next.clamp(min, max), window, cx);
            cx.stop_propagation();
        }
    });
    let drag_state_key = ElementId::Name(format!("{element_key}-drag").into());
    track.child(
        canvas(
            |bounds, _window, _cx| bounds,
            move |_bounds, bounds, window, cx| {
                let dragging = window.use_keyed_state(drag_state_key, cx, |_window, _cx| false);

                let down_state = dragging.clone();
                let down_apply = apply.clone();
                let down_start = drag_start.clone();
                window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
                    if phase != DispatchPhase::Bubble
                        || event.button != MouseButton::Left
                        || !bounds.contains(&event.position)
                    {
                        return;
                    }
                    down_state.update(cx, |active, _cx| *active = true);
                    if let Some(handler) = &down_start {
                        handler(window, cx);
                    }
                    down_apply(bounds, event.position, window, cx);
                    cx.stop_propagation();
                });

                let move_state = dragging.clone();
                let move_apply = apply.clone();
                window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                    if phase != DispatchPhase::Bubble
                        || event.pressed_button != Some(MouseButton::Left)
                        || !*move_state.read(cx)
                    {
                        return;
                    }
                    move_apply(bounds, event.position, window, cx);
                });

                window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                    if phase != DispatchPhase::Bubble
                        || event.button != MouseButton::Left
                        || !*dragging.read(cx)
                    {
                        return;
                    }
                    dragging.update(cx, |active, _cx| *active = false);
                    if let Some(handler) = &drag_end {
                        handler(window, cx);
                    }
                });
            },
        )
        .absolute()
        .inset_0(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_knob_stays_inside_the_track_at_both_ends() {
        let track = 200.0;
        let knob = chrome::SLIDER_KNOB_WIDTH;
        assert_eq!(knob_offset(track, knob, 0.0), 0.0, "flush at the minimum");
        assert_eq!(
            knob_offset(track, knob, 1.0),
            track - knob,
            "flush at the maximum"
        );
        assert_eq!(knob_offset(track, knob, 0.5), (track - knob) / 2.0);
        // Out-of-range values clamp rather than overhanging.
        assert_eq!(knob_offset(track, knob, -1.0), 0.0);
        assert_eq!(knob_offset(track, knob, 2.0), track - knob);
        // A track narrower than the knob cannot travel.
        assert_eq!(knob_offset(10.0, knob, 1.0), 0.0);
    }

    /// The renderer cannot mix a relative and an absolute length in one value,
    /// so it composes the offset out of an inset box plus a negative margin.
    /// That composition has to come out at the modelled offset, or the knob
    /// drifts off the track again.
    #[test]
    fn the_rendered_layout_reproduces_the_modelled_offset() {
        for knob in [chrome::SLIDER_KNOB_WIDTH, chrome::SLIDER_SM_KNOB] {
            for track in [40.0, 100.0, 160.0, 200.0] {
                for step in 0..=10 {
                    let fraction = step as f32 / 10.0;
                    let modelled = knob_offset(track, knob, fraction);
                    let rendered = knob_offset_from_layout(track, knob, fraction);
                    assert!(
                        (modelled - rendered).abs() < 0.001,
                        "knob {knob} track {track} at {fraction}: {modelled} vs {rendered}"
                    );
                }
            }
        }
    }

    #[test]
    fn pointer_mapping_is_the_inverse_of_the_rendered_knob_position() {
        let track = 200.0;
        let knob = chrome::SLIDER_KNOB_WIDTH;
        for fraction in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let knob_center = knob_offset_from_layout(track, knob, fraction) + knob / 2.0;
            assert_eq!(fraction_at_position(track, knob, knob_center), fraction);
        }
    }

    #[test]
    fn the_compact_variant_reserves_half_of_its_smaller_knob() {
        // `base.css` gives `.slider--sm` a 0.5rem inset for its 12px knob.
        assert_eq!(content_inset(chrome::SLIDER_SM_KNOB), 6.0);
        assert_eq!(content_inset(chrome::SLIDER_KNOB_WIDTH), 12.0);
        assert_eq!(
            knob_offset(100.0, chrome::SLIDER_SM_KNOB, 1.0),
            100.0 - chrome::SLIDER_SM_KNOB
        );
    }

    #[test]
    fn stepped_values_snap_and_move_to_adjacent_steps() {
        let steps = [0.0, 0.25, 0.5, 1.0];
        assert_eq!(snap_to_step(0.4, &steps), 0.5);
        assert_eq!(adjacent_step(0.5, &steps, -1), 0.25);
        assert_eq!(adjacent_step(0.5, &steps, 1), 1.0);
    }
}
