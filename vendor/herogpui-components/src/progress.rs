//! ProgressBar / ProgressCircle — port of `@heroui/progress`.

use gpui::{
    prelude::*, px, AnimationExt, App, IntoElement, Pixels, RenderOnce, SharedString, Styled,
    Window,
};
use herogpui_core::{element_id, Color, Size};
use herogpui_theme::ActiveTheme;

use crate::a11y::A11y as _;

#[derive(Clone)]
struct ProgressBarMotion {
    target: f32,
    generation: usize,
    from: f32,
    width: std::rc::Rc<std::cell::Cell<f32>>,
}

impl ProgressBarMotion {
    fn retarget(&mut self, target: f32, animate: bool) -> bool {
        let mut changed = false;
        if (self.target - target).abs() > f32::EPSILON {
            self.target = target;
            self.generation = self.generation.wrapping_add(1);
            self.from = self.width.get();
            changed = true;
        }
        if !animate && (self.width.get() - target).abs() > f32::EPSILON {
            self.from = target;
            self.width.set(target);
            changed = true;
        }
        changed
    }
}

struct ProgressBarMotionFrame {
    /// The ProgressBar's own id, so two bars animate on separate timelines.
    id: gpui::ElementId,
    generation: usize,
    from: f32,
    to: f32,
    width: std::rc::Rc<std::cell::Cell<f32>>,
    animate: bool,
}

impl ProgressBarMotionFrame {
    fn render(self, fill: gpui::Div) -> gpui::AnyElement {
        if !self.animate {
            self.width.set(self.to);
            return fill.w(gpui::relative(self.to)).into_any_element();
        }

        let width = self.width;
        let from = self.from;
        let to = self.to;
        fill.with_animation(
            element_id::indexed(&self.id, "fill-width", self.generation),
            gpui::Animation::new(std::time::Duration::from_millis(
                crate::anim::PROGRESS_BAR_FILL_MS,
            ))
            .with_easing(|t| crate::anim::Curve::Out.at(t)),
            move |fill, delta| {
                let next = from + (to - from) * delta;
                width.set(next);
                fill.w(gpui::relative(next))
            },
        )
        .into_any_element()
    }
}

fn progress_bar_motion(
    id: &gpui::ElementId,
    target: f32,
    animate: bool,
    window: &mut Window,
    cx: &mut App,
) -> ProgressBarMotionFrame {
    let state = window.use_keyed_state(element_id::scoped(id, "fill-motion"), cx, |_, _| {
        ProgressBarMotion {
            target,
            generation: 0,
            from: target,
            width: std::rc::Rc::new(std::cell::Cell::new(target)),
        }
    });
    let mut current = state.read(cx).clone();
    if current.retarget(target, animate) {
        state.update(cx, |stored, _| *stored = current.clone());
    }
    let should_animate =
        animate && current.generation != 0 && (current.width.get() - target).abs() > f32::EPSILON;
    ProgressBarMotionFrame {
        id: id.clone(),
        generation: current.generation,
        from: current.from,
        to: target,
        width: current.width,
        animate: should_animate,
    }
}

/// Linear progress bar.
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct ProgressBar {
    id: gpui::ElementId,
    value: f32,
    min_value: f32,
    max_value: f32,
    size: Size,
    color: Color,
    is_indeterminate: bool,
    label: Option<String>,
    show_value: bool,
    value_label: Option<SharedString>,
    /// `ProgressBar.ValueLabel`'s render props: the closure is handed
    /// `percentage`, `valueText`, and `isIndeterminate`.
    value_content: Option<std::sync::Arc<dyn Fn(f32, &str, bool) -> gpui::AnyElement + 'static>>,
    /// `formatOptions` — how the generated value label is written.
    format: Option<herogpui_core::NumberFormat>,
    /// The `sx` slot, refined over the root style at the end of render.
    sx: Option<Box<gpui::StyleRefinement>>,
    /// The corner radius, in place of the size-step's radius. The track and
    /// every fill segment paint it.
    radius: Option<Pixels>,
    /// `useMeter` is `useProgressBar` with one thing changed — the role — and
    /// [`crate::meter::Meter`] delegates its whole rendering here, so the role
    /// travels with the delegation rather than being duplicated.
    a11y_role: crate::a11y::Role,
}

impl ProgressBar {
    /// Reports this bar as a meter rather than a progress indicator.
    ///
    /// Upstream's `role="meter progressbar"` is a pair: `meter` with
    /// `progressbar` as a fallback for browsers that do not implement it.
    /// AccessKit roles are a single enum, so the port reports the half that is
    /// true.
    pub(crate) fn as_meter(mut self) -> Self {
        self.a11y_role = crate::a11y::Role::Meter;
        self
    }

    /// Creates a progress bar with the given element id.
    pub fn new(id: impl Into<gpui::ElementId>) -> Self {
        Self {
            id: id.into(),
            value: 0.0,
            min_value: 0.0,
            max_value: 100.0,
            size: Size::Md,
            color: Color::Accent,
            is_indeterminate: false,
            label: None,
            show_value: false,
            value_label: None,
            value_content: None,
            format: None,
            sx: None,
            radius: None,
            a11y_role: crate::a11y::Role::ProgressIndicator,
        }
    }

    /// Hands a captured refinement down from a component that renders no root
    /// of its own: [`crate::meter::Meter`] delegates its whole element tree to
    /// this bar, so the bar's root is the meter's root.
    pub(crate) fn sx_refinement(mut self, sx: Box<gpui::StyleRefinement>) -> Self {
        self.sx = Some(sx);
        self
    }

    /// Sets the current value (v3 `value`).
    pub fn value(mut self, v: f32) -> Self {
        self.value = v;
        self
    }

    /// Sets the minimum value (v3 `minValue`).
    pub fn min_value(mut self, v: f32) -> Self {
        self.min_value = v;
        self
    }

    /// Sets the maximum value (v3 `maxValue`).
    pub fn max_value(mut self, v: f32) -> Self {
        self.max_value = v;
        self
    }

    /// `isIndeterminate` — an unbounded operation; the bar sweeps instead of
    /// filling to a fraction.
    pub fn is_indeterminate(mut self, v: bool) -> Self {
        self.is_indeterminate = v;
        self
    }

    /// `ProgressBar.ValueLabel`'s render function — handed `percentage`,
    /// `valueText`, and `isIndeterminate`.
    pub fn value_content(
        mut self,
        render: impl Fn(f32, &str, bool) -> gpui::AnyElement + 'static,
    ) -> Self {
        self.value_content = Some(std::sync::Arc::new(render));
        self
    }

    /// `valueLabel` — replaces the generated percentage.
    pub fn value_label(mut self, text: impl Into<SharedString>) -> Self {
        self.value_label = Some(text.into());
        self
    }

    /// `formatOptions` — v3 defaults to `{style: "percent"}`.
    pub fn format_options(mut self, format: herogpui_core::NumberFormat) -> Self {
        self.format = Some(format);
        self
    }

    /// Sets the size (v3 `size`).
    pub fn size(mut self, s: Size) -> Self {
        self.size = s;
        self
    }

    /// Sets the color (v3 `color`).
    pub fn color(mut self, c: Color) -> Self {
        self.color = c;
        self
    }

    /// The corner radius, in place of the size-step's radius. Both the track
    /// and the fill segments that run inside it follow the override. Not a v3
    /// prop; the removed v2 `radius` prop is prohibited and this is a
    /// per-component repository extension.
    pub fn radius(mut self, radius: impl Into<Pixels>) -> Self {
        self.radius = Some(radius.into());
        self
    }

    /// The one slot for caller-owned low-level styling: GPUI's styling methods
    /// (`bg`, `text_color`, `w`, `h`, `p`, `rounded`, `border_color`, …)
    /// applied to the bar's root element after every value the size and the
    /// active theme chose, so they win.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        crate::util::refine_sx(&mut self.sx, style);
        self
    }

    /// Label rendered above the track (`label` + `showValueLabel`).
    pub fn label(mut self, l: impl Into<String>) -> Self {
        self.label = Some(l.into());
        self
    }

    /// Sets whether the value label is shown.
    pub fn show_value_label(mut self, v: bool) -> Self {
        self.show_value = v;
        self
    }
}

impl RenderOnce for ProgressBar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = cx.colors();
        let progress_fill_color = if self.color == Color::Default {
            colors.default.foreground
        } else {
            cx.role(self.color).color
        };
        let progress_track_color = colors.default.color;
        let text_color = colors.foreground;
        let (h, radius) = match self.size {
            Size::Sm => (px(4.), crate::util::micro_radius(cx)),
            Size::Md => (px(8.), crate::util::hairline_radius(cx)),
            Size::Lg => (px(12.), crate::util::mark_radius(cx)),
        };
        // The size step stays the fallback; an instance radius replaces it for
        // the track and every fill segment below.
        let radius = self.radius.unwrap_or(radius);
        // Clamp once at entry so the fill, percentage and every formatted
        // label use the same value, matching React Aria's clamp-before-format
        // behavior. Guarded because
        // `f32::clamp` panics when min > max, which `fraction_of` tolerates.
        let value = if self.min_value <= self.max_value {
            self.value.clamp(self.min_value, self.max_value)
        } else {
            self.value
        };
        let fraction = fraction_of(self.value, self.min_value, self.max_value);
        // `useProgressBar` formats the value label whether or not anything
        // displays it, because `aria-valuetext` carries it either way. An
        // indeterminate bar has neither.
        let announced_value_text: Option<SharedString> = (!self.is_indeterminate).then(|| {
            self.value_label.clone().unwrap_or_else(|| {
                let format = self
                    .format
                    .clone()
                    .unwrap_or_else(herogpui_core::NumberFormat::percent);
                let n = if format.style == herogpui_core::NumberStyle::Percent {
                    fraction as f64
                } else {
                    value as f64
                };
                SharedString::from(format.format(n))
            })
        });
        let fill_motion = progress_bar_motion(
            &self.id,
            if self.is_indeterminate { 0.4 } else { fraction },
            !self.is_indeterminate && !ActiveTheme::reduce_motion(cx),
            window,
            cx,
        );

        // `useProgressBar` keeps `aria-valuemin`/`aria-valuemax` in every
        // case and drops `aria-valuenow`/`aria-valuetext` when indeterminate.
        let range = if self.is_indeterminate {
            crate::a11y::Range::indeterminate(self.min_value as f64, self.max_value as f64)
        } else {
            crate::a11y::Range::new(
                self.min_value as f64,
                self.max_value as f64,
                self.value as f64,
            )
            .text(announced_value_text.clone())
        };
        // The accessibility calls come after the layout chain deliberately:
        // `design_audit.py` reads this wrapper's gap through a pattern
        // anchored on `gpui::div()` followed by its layout calls.
        let mut el = gpui::div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .w_full()
            .id(self.id.clone())
            .a11y_named(
                self.a11y_role,
                &crate::a11y::Name::maybe(self.label.clone().map(SharedString::from)),
            )
            .a11y_range(&range);

        // `.progress-bar__output` / `.meter__output` is the value beside the
        // label, in the row above the track.
        if self.label.is_some() || self.show_value {
            // The same text the node announces; an indeterminate bar shows
            // nothing where the value would be.
            let value_text = announced_value_text.unwrap_or_default();
            let percentage = if self.is_indeterminate {
                0.0
            } else {
                fraction * 100.
            };
            el = el.child(
                gpui::div()
                    .flex()
                    .justify_between()
                    .text_size(px(14.))
                    .line_height(px(20.))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(text_color)
                    .child(self.label.clone().unwrap_or_default())
                    .when(self.show_value, |l| {
                        // `percentage` is 0-100, with 0 standing in for v3's
                        // undefined indeterminate percentage. The output
                        // wrapper carries HeroUI's tabular-nums feature while
                        // leaving the label on the normal font metrics.
                        let output = match &self.value_content {
                            Some(render) => render(percentage, &value_text, self.is_indeterminate),
                            None => value_text.to_string().into_any_element(),
                        };
                        l.child(
                            gpui::div()
                                .font_features(crate::util::tabular_font_features())
                                .child(output),
                        )
                    }),
            );
        }

        // `.progress-bar__track` / `.meter__track`, with
        // `.progress-bar__fill` / `.meter__fill` inside it.
        let track = gpui::div()
            .w_full()
            .h(h)
            .overflow_hidden()
            .rounded(radius)
            .bg(progress_track_color);

        // Indeterminate bars sweep a 40% segment; reduced motion leaves that
        // same segment static so the state is still legible.
        let track = if self.is_indeterminate && !ActiveTheme::reduce_motion(cx) {
            let indeterminate_id = element_id::scoped(&self.id, "indeterminate");
            track
                .child(
                    gpui::div()
                        .relative()
                        .h_full()
                        .w(gpui::relative(0.4))
                        .rounded(radius)
                        .bg(progress_fill_color)
                        .with_animation(
                            indeterminate_id,
                            gpui::Animation::new(std::time::Duration::from_millis(
                                crate::anim::PROGRESS_BAR_INDETERMINATE_MS,
                            ))
                            .with_easing(crate::anim::progress_bar_indeterminate_ease())
                            .repeat(),
                            |el, delta| el.left(gpui::relative(delta * 1.8 - 0.4)),
                        ),
                )
                .into_any_element()
        } else if self.is_indeterminate {
            track
                .child(
                    gpui::div()
                        .h_full()
                        .rounded(radius)
                        .bg(progress_fill_color)
                        .w(gpui::relative(0.4)),
                )
                .into_any_element()
        } else {
            let indicator = gpui::div().h_full().rounded(radius).bg(progress_fill_color);
            track
                .child(fill_motion.render(indicator))
                .into_any_element()
        };

        el = el.child(track);
        crate::util::apply_sx(el, &self.sx)
    }
}

/// Normalises `value` into `0.0..=1.0` across the v3 `minValue`/`maxValue`
/// range, guarding against an empty or inverted range.
fn fraction_of(value: f32, min: f32, max: f32) -> f32 {
    let span = max - min;
    if span.abs() < f32::EPSILON {
        return 0.0;
    }
    ((value - min) / span).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::ProgressBarMotion;
    use std::cell::Cell;
    use std::rc::Rc;

    fn close(left: f32, right: f32) -> bool {
        (left - right).abs() < f32::EPSILON
    }

    #[test]
    fn progress_bar_motion_reverses_from_current_width_and_snaps_without_motion() {
        let width = Rc::new(Cell::new(0.25));
        let mut motion = ProgressBarMotion {
            target: 0.25,
            generation: 0,
            from: 0.25,
            width: width.clone(),
        };

        assert!(motion.retarget(0.75, true));
        assert_eq!(motion.generation, 1);
        assert!(close(motion.from, 0.25));
        assert!(close(width.get(), 0.25));

        width.set(0.5);
        assert!(motion.retarget(0.1, true));
        assert_eq!(motion.generation, 2);
        assert!(close(motion.from, 0.5));
        assert!(close(width.get(), 0.5));

        assert!(motion.retarget(0.9, false));
        assert_eq!(motion.generation, 3);
        assert!(close(motion.from, 0.9));
        assert!(close(width.get(), 0.9));
    }

    #[test]
    fn indeterminate_circle_rotates_track_and_fill_as_one_surface() {
        let source = include_str!("progress.rs")
            .rsplit("/// Circular progress ring")
            .next()
            .expect("the ProgressCircle implementation is always present");
        assert!(source.contains("let progress_visual = gpui::div()"));
        assert!(source.contains(".child(track).child(arc)"));
        assert!(source.contains("progress_visual\n                .with_animation("));
        assert!(source.contains("rotation.set(crate::anim::progress_circle_spin_turn(delta))"));
    }

    #[test]
    fn determinate_circle_interpolates_the_retained_arc_fraction() {
        let source = include_str!("progress.rs")
            .rsplit("/// Circular progress ring")
            .next()
            .expect("the ProgressCircle implementation is always present");
        assert!(source.contains("PROGRESS_CIRCLE_FILL_MS"));
        assert!(source.contains("\"fill-fraction\""));
        assert!(source.contains("let paint_fraction = fraction_motion.value()"));
        assert!(source.contains("let fraction = paint_fraction.get()"));
        assert!(source.contains("fraction_motion.animates(reduce_motion)"));
    }

    #[test]
    fn progress_output_uses_shared_tabular_numeric_features() {
        let source = include_str!("progress.rs")
            .split("/// Circular progress ring")
            .next()
            .expect("the ProgressBar implementation is always present");
        assert!(source.contains("tabular_font_features"));
        assert!(source.contains("font_features(crate::util::tabular_font_features())"));
    }

    #[test]
    fn indeterminate_bar_animation_is_instance_scoped() {
        let source = include_str!("progress.rs");
        assert!(
            source.contains("element_id::scoped(&self.id, \"indeterminate\")"),
            "each indeterminate bar needs an animation id under its own component id"
        );
        assert!(
            !source.contains("\"progress-bar-indeterminate\""),
            "a shared literal animation id lets sibling bars collide"
        );
    }
}

/// Circular progress ring (`ProgressCircle`).
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct ProgressCircle {
    value: f32,
    min_value: f32,
    max_value: f32,
    color: Color,
    size_px: Pixels,
    is_indeterminate: bool,
    show_value: bool,
    /// `ProgressCircle.ValueLabel`'s render props: `percentage`, `valueText`,
    /// and `isIndeterminate`.
    value_content: Option<std::sync::Arc<dyn Fn(f32, &str, bool) -> gpui::AnyElement + 'static>>,
    /// `formatOptions` — how the generated value label is written.
    format: Option<herogpui_core::NumberFormat>,
    id: Option<gpui::ElementId>,
    /// The `sx` slot, refined over the root style at the end of render.
    sx: Option<Box<gpui::StyleRefinement>>,
}

impl ProgressCircle {
    /// Creates a progress circle.
    pub fn new() -> Self {
        Self {
            value: 0.0,
            min_value: 0.0,
            max_value: 100.0,
            color: Color::Accent,
            size_px: px(28.),
            is_indeterminate: false,
            show_value: false,
            value_content: None,
            format: None,
            id: None,
            sx: None,
        }
    }

    /// Names this ring so it can report `role="progressbar"`. Unnamed rings
    /// produce no AccessKit node — a constant id would fold every instance
    /// into one.
    pub fn id(mut self, id: impl Into<gpui::ElementId>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// `ProgressCircle.ValueLabel`'s render function — handed `percentage`
    /// (0-100), `valueText`, and `isIndeterminate`.
    pub fn value_content(
        mut self,
        render: impl Fn(f32, &str, bool) -> gpui::AnyElement + 'static,
    ) -> Self {
        self.value_content = Some(std::sync::Arc::new(render));
        self
    }

    /// Sets the current value (v3 `value`).
    pub fn value(mut self, v: f32) -> Self {
        self.value = v;
        self
    }

    /// Sets the minimum value (v3 `minValue`).
    pub fn min_value(mut self, v: f32) -> Self {
        self.min_value = v;
        self
    }

    /// Sets the maximum value (v3 `maxValue`).
    pub fn max_value(mut self, v: f32) -> Self {
        self.max_value = v;
        self
    }

    /// `isIndeterminate` — spins the arc instead of sweeping to a fraction.
    pub fn is_indeterminate(mut self, v: bool) -> Self {
        self.is_indeterminate = v;
        self
    }

    /// Sets the color (v3 `color`).
    pub fn color(mut self, c: Color) -> Self {
        self.color = c;
        self
    }

    /// `size` — the ring's diameter: 20 / 28 / 36px for `sm` / `md` / `lg`.
    ///
    /// v3 documents the three-step scale, not a pixel value.
    pub fn size(mut self, s: Size) -> Self {
        self.size_px = match s {
            Size::Sm => px(20.),
            Size::Md => px(28.),
            Size::Lg => px(36.),
        };
        self
    }

    /// The one slot for caller-owned low-level styling: GPUI's styling methods
    /// (`bg`, `text_color`, `w`, `h`, `p`, `rounded`, `border_color`, …)
    /// applied to the ring's root element after every value the size and the
    /// active theme chose, so they win.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        crate::util::refine_sx(&mut self.sx, style);
        self
    }

    /// Sets whether the value label is shown.
    pub fn show_value_label(mut self, v: bool) -> Self {
        self.show_value = v;
        self
    }

    /// `formatOptions` — v3 defaults to `{style: "percent"}`.
    pub fn format_options(mut self, format: herogpui_core::NumberFormat) -> Self {
        self.format = Some(format);
        self
    }
}

impl Default for ProgressCircle {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for ProgressCircle {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // A value change moves the SVG stroke-dashoffset over 300ms. The
        // canvas arc has no retained dash property, so keep the live fraction
        // in the same keyed tween and let the paint callback read it. This
        // preserves reversal from the frame actually on screen and lets
        // reduced motion settle without scheduling a frame.
        // Clamp once at entry, like the bar: a non-percent label formats the
        // clamped value, and the guard keeps `f32::clamp` from panicking on
        // the inverted range `fraction_of` tolerates.
        let value = if self.min_value <= self.max_value {
            self.value.clamp(self.min_value, self.max_value)
        } else {
            self.value
        };
        let fraction = if self.is_indeterminate {
            // An indeterminate ring shows a fixed quarter arc.
            0.25
        } else {
            fraction_of(self.value, self.min_value, self.max_value)
        };
        // An id-less ring still needs per-instance motion state. The old
        // constant fallback made every anonymous ring share one tween slot --
        // one `Rc<Cell>` fraction -- so an indeterminate sibling's `settle()`
        // pinned the shared cell at the quarter arc and every determinate
        // ring painted 0.25 while its fill animation restarted each frame.
        // The fallback key is the ring's own arc signature plus the fraction
        // itself, so rings that differ only in `value` keep separate
        // timelines, and `last_fraction` remembers what this signature last
        // painted to seed a new fraction's tween -- an id-less value change
        // still eases instead of snapping.
        let signature_id = self.id.is_none().then(|| {
            let ring = gpui::ElementId::Name("progress-circle".into());
            let ring = element_id::scoped(
                &ring,
                if self.is_indeterminate {
                    "indeterminate"
                } else {
                    "determinate"
                },
            );
            let ring = element_id::scoped(&ring, self.min_value.to_bits().to_string());
            let ring = element_id::scoped(&ring, self.max_value.to_bits().to_string());
            element_id::scoped(&ring, f32::from(self.size_px).to_bits().to_string())
        });
        let motion_id = match &self.id {
            Some(id) => id.clone(),
            None => element_id::scoped(
                signature_id
                    .as_ref()
                    .expect("the signature exists exactly when the id does not"),
                fraction.to_bits().to_string(),
            ),
        };
        let continuity = signature_id.as_ref().map(|signature| {
            window.use_keyed_state(
                element_id::scoped(signature, "last-fraction"),
                cx,
                |_, _| std::cell::Cell::new(f32::NAN),
            )
        });
        let seed = continuity
            .as_ref()
            .map(|held| held.read(cx).get())
            .filter(|last| !last.is_nan());
        let reduce_motion = ActiveTheme::reduce_motion(cx);
        let mut fraction_motion = crate::anim::Tween::keyed_with_initial(
            &motion_id,
            "fill-fraction",
            fraction,
            seed,
            window,
            cx,
        );
        if self.is_indeterminate {
            fraction_motion.settle();
        } else {
            fraction_motion.snap_if_reduced(reduce_motion);
        }
        let animate_fraction = !self.is_indeterminate && fraction_motion.animates(reduce_motion);
        let paint_fraction = fraction_motion.value();
        if let Some(held) = &continuity {
            held.update(cx, |cell, _| cell.set(paint_fraction.get()));
        }
        let colors = cx.colors();
        let arc_color = if self.color == Color::Default {
            colors.default.foreground
        } else {
            cx.role(self.color).color
        };
        // v3 uses stroke-width 4 in a 36-unit viewBox.
        let stroke_w = self.size_px / 9.;
        let spins = self.is_indeterminate && !reduce_motion;
        let rotation = std::rc::Rc::new(std::cell::Cell::new(0.0f32));
        let paint_rotation = rotation.clone();

        let arc = gpui::canvas(
            move |bounds, _, _| bounds,
            move |bounds, _, window, _| {
                let fraction = paint_fraction.get();
                if fraction <= 0.0 {
                    return;
                }
                let mut builder = gpui::PathBuilder::stroke(stroke_w);
                let center = bounds.center();
                let radius = (bounds.size.width.min(bounds.size.height) / 2.) - stroke_w / 2.;
                // CSS positive rotation is clockwise in screen coordinates;
                // this path's mathematical angle increases counter-clockwise.
                let start = std::f32::consts::FRAC_PI_2 - paint_rotation.get();
                let sweep = std::f32::consts::TAU * fraction;
                let steps = ((sweep / 0.05).ceil() as usize).max(2);
                for i in 0..=steps {
                    let a = start - sweep * i as f32 / steps as f32;
                    let p = gpui::point(center.x + radius * a.cos(), center.y - radius * a.sin());
                    if i == 0 {
                        builder.move_to(p);
                    } else {
                        builder.line_to(p);
                    }
                }
                if let Ok(path) = builder.build() {
                    window.paint_path(path, arc_color);
                }
                // v3's SVG uses stroke-linecap="round". GPUI's public stroke
                // builder uses butt caps, so complete the same geometry with
                // a filled disc at each endpoint.
                let cap_radius = stroke_w / 2.;
                for angle in [start, start - sweep] {
                    let cap_center = gpui::point(
                        center.x + radius * angle.cos(),
                        center.y - radius * angle.sin(),
                    );
                    crate::util::paint_disc(cap_center, cap_radius, arc_color, window);
                }
            },
        )
        .absolute()
        .inset_0();

        let id = self
            .id
            .as_ref()
            .or(signature_id.as_ref())
            .expect("a ring always has a caller id or an arc signature");
        let spin_id = element_id::scoped(id, "spin");
        let track = gpui::div()
            .absolute()
            .inset_0()
            .rounded_full()
            .border(stroke_w)
            .border_color(colors.default.color);

        // HeroUI animates the SVG track wrapper, so the track and the quarter
        // fill arc rotate as one surface. Rotating only the arc leaves a
        // stationary ring behind it and makes the indeterminate state visibly
        // different from the pinned `progress-circle__track` animation.
        let progress_visual = gpui::div().absolute().inset_0().child(track).child(arc);
        let progress_visual = if spins {
            progress_visual
                .with_animation(
                    spin_id,
                    gpui::Animation::new(std::time::Duration::from_millis(
                        crate::anim::PROGRESS_CIRCLE_SPIN_MS,
                    ))
                    .repeat(),
                    move |visual, delta| {
                        rotation.set(crate::anim::progress_circle_spin_turn(delta));
                        visual
                    },
                )
                .into_any_element()
        } else if animate_fraction {
            let from = fraction_motion.from();
            let to = fraction_motion.target();
            let value = fraction_motion.value();
            progress_visual
                .with_animation(
                    element_id::indexed(&motion_id, "fill-fraction", fraction_motion.generation()),
                    gpui::Animation::new(std::time::Duration::from_millis(
                        crate::anim::PROGRESS_CIRCLE_FILL_MS,
                    ))
                    .with_easing(crate::anim::ease_out()),
                    move |visual, delta| {
                        value.set(from + (to - from) * delta);
                        visual
                    },
                )
                .into_any_element()
        } else {
            fraction_motion.settle();
            progress_visual.into_any_element()
        };

        let el = gpui::div()
            .relative()
            .flex()
            .items_center()
            .justify_center()
            .size(self.size_px)
            // `.progress-circle__track` and `.progress-circle__fill-circle`
            // share one animated SVG wrapper in v3. The canvas-backed port
            // keeps the same stacking and rotates both layers together.
            .child(progress_visual)
            .when(self.show_value, |el| {
                let format = self
                    .format
                    .clone()
                    .unwrap_or_else(herogpui_core::NumberFormat::percent);
                // The indeterminate quarter arc is geometry, not a value:
                // React Aria generates no value label for indeterminate
                // progress, so report an empty text and a 0% percentage,
                // like the bar does. The arc keeps drawing regardless.
                let (percentage, value_text) = if self.is_indeterminate {
                    (0.0, String::new())
                } else {
                    let n = if format.style == herogpui_core::NumberStyle::Percent {
                        fraction as f64
                    } else {
                        value as f64
                    };
                    (fraction * 100., format.format(n))
                };
                match &self.value_content {
                    Some(render) => {
                        el.child(render(percentage, &value_text, self.is_indeterminate))
                    }
                    None => el.child(
                        gpui::div()
                            .text_size(px(12.))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(colors.foreground)
                            .child(value_text),
                    ),
                }
            });
        let el = crate::util::apply_sx(el, &self.sx);
        match self.id {
            Some(id) => {
                let range = if self.is_indeterminate {
                    crate::a11y::Range::indeterminate(self.min_value as f64, self.max_value as f64)
                } else {
                    crate::a11y::Range::new(
                        self.min_value as f64,
                        self.max_value as f64,
                        self.value as f64,
                    )
                };
                el.id(id)
                    .a11y(crate::a11y::Role::ProgressIndicator)
                    .a11y_range(&range)
                    .into_any_element()
            }
            None => el.into_any_element(),
        }
    }
}

crate::util::impl_component_styled!(ProgressBar, ProgressCircle);
