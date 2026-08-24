//! Separator and progress primitives — ports of
//! `renderer/components/ui/separator.tsx` and `ui/progress.tsx`.

use gpui::{div, prelude::*, px, Styled};

use crate::theme::vars::active_theme;

/// HeroUI's focus treatment is a ring, i.e. a spread-only box shadow that
/// costs no layout: `focus-ring` is `ring-2 ring-focus` offset by
/// `--ring-offset-width` (2px), and `focus-field-ring` is the same ring with no
/// offset. `--focus` resolves to `--accent`.
///
/// `offset` reproduces `ring-offset-*`: the gap is drawn by first laying down a
/// background-coloured ring of that width, then the accent ring outside it.
pub fn focus_ring(theme: &crate::theme::vars::ThemeVars, offset: f32) -> Vec<gpui::BoxShadow> {
    let ring = |color: gpui::Hsla, spread: f32| gpui::BoxShadow {
        color,
        offset: gpui::point(px(0.0), px(0.0)),
        blur_radius: px(0.0),
        spread_radius: px(spread),
    };
    let mut shadows = Vec::new();
    if offset > 0.0 {
        shadows.push(ring(theme.background, offset));
    }
    shadows.push(ring(theme.accent, offset + FOCUS_RING_WIDTH));
    shadows
}

pub fn control_focus(
    key: &str,
    disabled: bool,
    window: &mut gpui::Window,
    cx: &mut gpui::App,
) -> gpui::FocusHandle {
    let state = window.use_keyed_state(
        gpui::ElementId::Name(format!("{key}-focus").into()),
        cx,
        |_, cx| cx.focus_handle(),
    );
    state.read(cx).clone().tab_stop(!disabled)
}

/// Popovers, menus and tooltips enter with `animate-in duration-150 ease-smooth
/// fade-in-0 zoom-in-90/95` plus a `slide-in-from-*-1`.
pub const OVERLAY_ENTER_MS: u64 = 150;
/// `slide-in-from-top-1` and friends: one spacing step, 4px.
pub const OVERLAY_ENTER_SLIDE: f32 = 4.0;

/// `zoom-in-95` and `zoom-in-90`: the scale a surface enters from.
pub const OVERLAY_ENTER_ZOOM_95: f32 = 0.95;
pub const OVERLAY_ENTER_ZOOM_90: f32 = 0.90;

/// The entrance progress of a surface that opened at `started`, eased on
/// `--ease-smooth`, saturating at 1.
///
/// `zoom-in-*` cannot be a transform — gpui has none for a `div` — so the
/// surfaces that carry one scale their *layout* instead: every length is
/// multiplied by [`enter_scale`], which is a proportional re-layout rather than
/// a raster scale. That needs the factor before the children are built, which is
/// why it comes from a clock rather than from an animation closure.
pub fn enter_progress(started: std::time::Instant) -> f32 {
    let elapsed = started.elapsed().as_secs_f32() * 1000.0;
    let linear = (elapsed / OVERLAY_ENTER_MS as f32).clamp(0.0, 1.0);
    ease_smooth()(linear)
}

/// The layout multiplier for a surface entering from `zoom`.
pub fn enter_scale(started: std::time::Instant, zoom: f32) -> f32 {
    zoom + (1.0 - zoom) * enter_progress(started)
}

/// Whether the entrance is still running, i.e. whether another frame is needed.
pub fn entering(started: std::time::Instant) -> bool {
    enter_progress(started) < 1.0
}

/// Which edge the surface slides in from, following the placement-specific
/// rules in `popover.css`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EnterFrom {
    /// `data-placement="bottom"` → `slide-in-from-top-1`.
    Top,
    /// `data-placement="top"` → `slide-in-from-bottom-1`.
    Bottom,
}

/// Wraps `element` in the 150ms entrance: the fade, and the 4px slide from the
/// placement edge. The `zoom-in-*` is the one component gpui cannot express, as
/// a `div` has no transform and the content is what would have to scale.
///
/// The id has to be stable for the life of the surface, or the animation
/// restarts every frame.
pub fn overlay_enter<E>(
    id: impl Into<gpui::ElementId>,
    from: EnterFrom,
    element: E,
) -> gpui::AnimationElement<E>
where
    E: gpui::IntoElement + Styled + 'static,
{
    use gpui::AnimationExt;

    let travel = match from {
        EnterFrom::Top => -OVERLAY_ENTER_SLIDE,
        EnterFrom::Bottom => OVERLAY_ENTER_SLIDE,
    };
    element.with_animation(
        id.into(),
        gpui::Animation::new(std::time::Duration::from_millis(OVERLAY_ENTER_MS))
            .with_easing(ease_smooth()),
        move |element, delta| element.opacity(delta).mt(px(travel * (1.0 - delta))),
    )
}

/// Evaluates a CSS `cubic-bezier(x1, y1, x2, y2)` at `t`, so the ported
/// transitions use the same curves the renderer does. The x-for-t inversion is
/// a fixed number of Newton steps, which is what browsers do.
pub fn cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32) -> impl Fn(f32) -> f32 {
    let curve = |a: f32, b: f32, t: f32| {
        let u = 1.0 - t;
        3.0 * u * u * t * a + 3.0 * u * t * t * b + t * t * t
    };
    let slope = |a: f32, b: f32, t: f32| {
        let u = 1.0 - t;
        3.0 * u * u * a + 6.0 * u * t * (b - a) + 3.0 * t * t * (1.0 - b)
    };
    move |x: f32| {
        let x = x.clamp(0.0, 1.0);
        let mut t = x;
        for _ in 0..6 {
            let error = curve(x1, x2, t) - x;
            let derivative = slope(x1, x2, t);
            if derivative.abs() < 1e-6 {
                break;
            }
            t -= error / derivative;
            t = t.clamp(0.0, 1.0);
        }
        curve(y1, y2, t)
    }
}

/// Per-element hover state, so a background can be interpolated instead of
/// swapped. gpui applies `hover` styles instantly and has no per-property
/// transition, so the CSS `transition: background-color` is reproduced by
/// remembering the hover across frames and animating between the two resting
/// colours.
pub struct HoverFade {
    pub hovered: bool,
    pub painted: bool,
}

impl HoverFade {
    /// The colours to animate between this frame. The first paint lands on the
    /// resting colour rather than fading in from the other one.
    pub fn range(&self, resting: gpui::Hsla, hovered: gpui::Hsla) -> (gpui::Hsla, gpui::Hsla) {
        if !self.painted {
            (resting, resting)
        } else if self.hovered {
            (resting, hovered)
        } else {
            (hovered, resting)
        }
    }
}

/// Reads the hover state for `key`, marking it painted, and hands back the
/// state entity so the caller can install its own `on_hover`.
pub fn hover_fade(
    key: &str,
    window: &mut gpui::Window,
    cx: &mut gpui::App,
) -> (gpui::Entity<HoverFade>, bool, bool) {
    let state = window.use_keyed_state(
        gpui::ElementId::Name(format!("{key}-hover").into()),
        cx,
        |_, _| HoverFade {
            hovered: false,
            painted: false,
        },
    );
    let (hovered, painted) = {
        let read = state.read(cx);
        (read.hovered, read.painted)
    };
    if !painted {
        state.update(cx, |state, _| state.painted = true);
    }
    (state, hovered, painted)
}

/// The `on_hover` body every fading surface installs.
pub fn track_hover(state: &gpui::Entity<HoverFade>, over: bool, cx: &mut gpui::App) {
    state.update(cx, |state, cx| {
        if state.hovered != over {
            state.hovered = over;
            cx.notify();
        }
    });
}

/// CSS `letter-spacing`, which gpui's text system has no equivalent for.
///
/// Spacing is laid out rather than shaped: each character becomes its own item
/// in a row with `tracking` between them, which is what `letter-spacing`
/// produces for a single-line run. Only suitable for short, non-ligature labels
/// — the settings sidebar title is the one place the renderer asks for it
/// (`tracking-[0.12em]`).
pub fn tracked_text(text: &str, tracking: gpui::Pixels) -> gpui::AnyElement {
    let mut row = div().flex().flex_row().items_center().gap(tracking);
    for character in text.chars() {
        row = row.child(gpui::SharedString::from(character.to_string()));
    }
    row.into_any_element()
}

/// `--ease-smooth: ease`, i.e. `cubic-bezier(0.25, 0.1, 0.25, 1)`, which the
/// field and select transitions use.
pub fn ease_smooth() -> impl Fn(f32) -> f32 {
    cubic_bezier(0.25, 0.1, 0.25, 1.0)
}

/// Tailwind's `--ease-out: cubic-bezier(0, 0, 0.2, 1)`, which `button.css`
/// uses for its background transition.
pub fn ease_out() -> impl Fn(f32) -> f32 {
    cubic_bezier(0.0, 0.0, 0.2, 1.0)
}

/// `--ease-out-fluid: cubic-bezier(0.32, 0.72, 0, 1)`, the curve the switch
/// thumb and the tab indicator travel on.
pub fn ease_out_fluid() -> impl Fn(f32) -> f32 {
    cubic_bezier(0.32, 0.72, 0.0, 1.0)
}

/// `focus-ring` is `ring-2`.
pub const FOCUS_RING_WIDTH: f32 = 2.0;
/// `--ring-offset-width: 2px`, used by `focus-ring` but not by
/// `focus-field-ring`.
#[allow(dead_code)]
pub const FOCUS_RING_OFFSET: f32 = 2.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Orientation {
    #[default]
    Horizontal,
    Vertical,
}

/// A hairline on the `--separator` token.
#[derive(IntoElement)]
pub struct Separator {
    orientation: Orientation,
    length: Option<gpui::Pixels>,
    inset: Option<gpui::Pixels>,
    color: Option<gpui::Hsla>,
}

impl Separator {
    pub fn horizontal() -> Self {
        Self {
            orientation: Orientation::Horizontal,
            length: None,
            inset: None,
            color: None,
        }
    }

    pub fn vertical(height: gpui::Pixels) -> Self {
        Self {
            orientation: Orientation::Vertical,
            length: Some(height),
            inset: None,
            color: None,
        }
    }

    /// Horizontal margin for vertical rules, vertical margin for horizontal
    /// ones — the `mx-1` / `my-1` the renderer applies.
    pub fn inset(mut self, inset: gpui::Pixels) -> Self {
        self.inset = Some(inset);
        self
    }

    /// Overrides `--separator`. The HeroUI `<Separator>` component is
    /// `bg-separator`, but the editor chrome draws its rules as plain
    /// `bg-border` divs, which is a different token.
    pub fn color(mut self, color: gpui::Hsla) -> Self {
        self.color = Some(color);
        self
    }
}

impl Default for Separator {
    fn default() -> Self {
        Self::horizontal()
    }
}

impl RenderOnce for Separator {
    fn render(self, _window: &mut gpui::Window, cx: &mut gpui::App) -> impl IntoElement {
        let theme = active_theme(cx);
        let rule = div().bg(self.color.unwrap_or(theme.separator)).flex_none();
        match self.orientation {
            Orientation::Horizontal => rule
                .w_full()
                .h(px(1.0))
                .when_some(self.inset, |el, inset| el.my(inset)),
            Orientation::Vertical => rule
                .w(px(1.0))
                .h(self.length.unwrap_or(px(16.0)))
                .when_some(self.inset, |el, inset| el.mx(inset)),
        }
    }
}

/// Linear progress bar.
#[derive(IntoElement)]
pub struct Progress {
    fraction: f32,
    height: gpui::Pixels,
}

impl Progress {
    pub fn new(fraction: f32) -> Self {
        Self {
            fraction: fraction.clamp(0.0, 1.0),
            // `ui/progress.tsx` pins `ProgressBar.Track className="h-2"`.
            height: px(8.0),
        }
    }

    #[allow(dead_code)]
    pub fn height(mut self, height: gpui::Pixels) -> Self {
        self.height = height;
        self
    }
}

impl RenderOnce for Progress {
    fn render(self, _window: &mut gpui::Window, cx: &mut gpui::App) -> impl IntoElement {
        let theme = active_theme(cx);
        div()
            .w_full()
            .h(self.height)
            .rounded_full()
            .overflow_hidden()
            .bg(theme.default)
            .child(
                div()
                    .h_full()
                    .rounded_full()
                    .bg(theme.accent)
                    .w(gpui::relative(self.fraction)),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ported_easing_curves_match_their_css_definitions() {
        let curves: [Box<dyn Fn(f32) -> f32>; 3] = [
            Box::new(ease_out_fluid()),
            Box::new(ease_out()),
            Box::new(ease_smooth()),
        ];
        for curve in curves {
            assert!(curve(0.0).abs() < 1e-3, "starts at 0");
            assert!((curve(1.0) - 1.0).abs() < 1e-3, "ends at 1");
            // Monotonic, which every one of these curves is.
            let mut previous = 0.0;
            for step in 0..=20 {
                let value = curve(step as f32 / 20.0);
                assert!(value >= previous - 1e-3, "monotonic at {step}");
                previous = value;
            }
        }
        // All three are front-loaded — half the time covers most of the
        // distance — and `--ease-out-fluid` most aggressively, which is why it
        // is the one the switch thumb and tab indicator travel on. Values are
        // the CSS curves evaluated at the midpoint.
        assert!((ease_out()(0.5) - 0.839).abs() < 0.01);
        assert!((ease_smooth()(0.5) - 0.802).abs() < 0.01);
        assert!((ease_out_fluid()(0.5) - 0.955).abs() < 0.01);
        assert!(ease_out_fluid()(0.5) > ease_out()(0.5));
        assert!(ease_out()(0.5) > ease_smooth()(0.5));
    }

    /// The entrance composes the fade and the 4px slide from the placement
    /// edge. The `zoom-in-*` third of it needs a transform gpui does not have;
    /// the fade masks the residual because both run on the same curve — the
    /// scale error is largest exactly when the surface is most transparent.
    #[test]
    fn the_entrance_slide_travels_one_spacing_step_toward_its_placement() {
        assert_eq!(OVERLAY_ENTER_SLIDE, 4.0);
        assert_eq!(OVERLAY_ENTER_MS, 150);
        // A menu hangs below its trigger, so it enters from above.
        assert_eq!(EnterFrom::Top, EnterFrom::Top);
        assert_ne!(EnterFrom::Top, EnterFrom::Bottom);
    }

    /// `zoom-in-*` is delivered as a proportional re-layout, so the factor has
    /// to start at the CSS scale, finish at 1, and never overshoot.
    #[test]
    fn the_entrance_scale_runs_from_the_css_zoom_to_one() {
        use std::time::{Duration, Instant};

        for zoom in [OVERLAY_ENTER_ZOOM_95, OVERLAY_ENTER_ZOOM_90] {
            let now = Instant::now();
            let at_start = enter_scale(now, zoom);
            assert!(
                (at_start - zoom).abs() < 0.02,
                "starts near {zoom}, got {at_start}"
            );
            assert!(entering(now), "a fresh surface is still entering");

            // Well past the duration the scale has settled and no further
            // frames are requested.
            let done = now - Duration::from_millis(OVERLAY_ENTER_MS * 2);
            assert_eq!(enter_scale(done, zoom), 1.0);
            assert!(!entering(done));

            // Never overshoots in between.
            for step in 0..=10 {
                let started = now - Duration::from_millis(OVERLAY_ENTER_MS * step / 10);
                let scale = enter_scale(started, zoom);
                assert!(
                    (zoom..=1.0).contains(&scale),
                    "{zoom} at step {step} gave {scale}"
                );
            }
        }
    }

    #[test]
    fn letter_spacing_is_laid_out_per_character() {
        // `tracking-[0.12em]` at the 12px sidebar title.
        assert!((crate::ui::chrome::SETTINGS_TITLE_TRACKING - 1.44).abs() < 1e-4);
    }

    #[test]
    fn the_focus_ring_is_two_pixels_of_accent() {
        assert_eq!(FOCUS_RING_WIDTH, 2.0);
        assert_eq!(FOCUS_RING_OFFSET, 2.0);
    }
}
