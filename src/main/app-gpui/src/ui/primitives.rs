//! Shared layout helpers used by app chrome. HeroUI separators and progress
//! bars come from HeroGPUI; this module keeps the focus ring, overlay motion,
//! and the editor's border-coloured tick.

use gpui::{div, prelude::*, px, Styled};
use herogpui::gpui;

use crate::theme::vars::ThemeVars;

/// The painted half of a button that carries both an icon and a label.
///
/// `Button::label` is the button's accessible name, and it *paints* that text
/// first — before every `.child(..)` — so an icon written before it in the
/// builder still lands after it on screen. `Button::content` paints in the
/// label's place but contributes no name. Passing both is the only way to get a
/// leading icon and a named button, which is why this exists rather than
/// `.child(icon).child(text)`: that order is right and the button is nameless.
///
/// `gap` is the row's own spacing and has to be passed, not inherited: the
/// button's `gap` no longer applies once the row is a single child.
pub fn icon_label(
    icon: &'static str,
    label: gpui::SharedString,
    icon_size: gpui::Pixels,
    gap: gpui::Pixels,
    icon_trailing: bool,
) -> gpui::AnyElement {
    let icon = crate::ui::icon::icon_element(icon, icon_size);
    let row = div().flex().items_center().gap(gap);
    if icon_trailing {
        row.child(label).child(icon)
    } else {
        row.child(icon).child(label)
    }
    .into_any_element()
}

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
        inset: false,
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
pub const OVERLAY_EXIT_MS: u64 = 100;
/// `slide-in-from-top-1` and friends: one spacing step, 4px.
pub const OVERLAY_ENTER_SLIDE: f32 = 4.0;

pub fn request_animation_frame(window: &gpui::Window) {
    window.request_animation_frame();
    request_immediate_animation_frame(window);
}

pub fn request_immediate_animation_frame(window: &gpui::Window) {
    #[cfg(all(windows, not(test)))]
    if let Some(hwnd) = crate::windows::window_hwnd(window) {
        crate::system::window_composition::request_animation_frame(hwnd);
    }
    #[cfg(any(not(windows), test))]
    let _ = window;
}

/// Which edge a closing surface slides towards, following the
/// placement-specific rules in `popover.css`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EnterFrom {
    /// `data-placement="bottom"` → `slide-in-from-top-1`.
    Top,
    /// `data-placement="top"` → `slide-in-from-bottom-1`. The app's menus only
    /// ever anchor below their trigger, so nothing constructs this today; it
    /// stays so a top-anchored panel slides the right way when one appears.
    #[allow(dead_code)]
    Bottom,
}

#[cfg(not(windows))]
pub fn overlay_fade_out<E>(id: impl Into<gpui::ElementId>, element: E) -> gpui::AnimationElement<E>
where
    E: gpui::IntoElement + Styled + 'static,
{
    use gpui::AnimationExt;
    use herogpui::gpui;

    element.with_animation(
        id.into(),
        gpui::Animation::new(std::time::Duration::from_millis(OVERLAY_EXIT_MS))
            .with_easing(|progress| progress),
        move |element, delta| element.opacity(1.0 - delta),
    )
}

pub fn overlay_exit<E>(
    id: impl Into<gpui::ElementId>,
    from: EnterFrom,
    element: E,
) -> gpui::AnimationElement<E>
where
    E: gpui::IntoElement + Styled + 'static,
{
    use gpui::AnimationExt;
    use herogpui::gpui;

    let travel = match from {
        EnterFrom::Top => -OVERLAY_ENTER_SLIDE,
        EnterFrom::Bottom => OVERLAY_ENTER_SLIDE,
    };
    element.with_animation(
        id.into(),
        gpui::Animation::new(std::time::Duration::from_millis(OVERLAY_EXIT_MS))
            .with_easing(ease_out()),
        move |element, delta| element.opacity(1.0 - delta).mt(px(travel * delta)),
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

pub struct HoverFade {
    pub hovered: bool,
    pub has_hovered: bool,
}

pub fn hover_is_active(state_hovered: bool, window: &gpui::Window) -> bool {
    state_hovered && window.is_window_hovered()
}

fn hover_entry(
    key: &str,
    window: &mut gpui::Window,
    cx: &mut gpui::App,
) -> (gpui::Entity<HoverFade>, bool, bool) {
    let state = window.use_keyed_state(
        gpui::ElementId::Name(format!("{key}-hover").into()),
        cx,
        |_, _| HoverFade {
            hovered: false,
            has_hovered: false,
        },
    );
    let (hovered, has_hovered) = {
        let hover = state.read(cx);
        (hover_is_active(hover.hovered, window), hover.has_hovered)
    };
    (state, hovered, has_hovered)
}

pub fn hover_flag(
    key: &str,
    window: &mut gpui::Window,
    cx: &mut gpui::App,
) -> (gpui::Entity<HoverFade>, bool) {
    let (state, hovered, _) = hover_entry(key, window, cx);
    (state, hovered)
}

/// The `on_hover` body every fading surface installs.
pub fn track_hover(state: &gpui::Entity<HoverFade>, over: bool, cx: &mut gpui::App) {
    state.update(cx, |state, cx| {
        if state.hovered != over {
            state.hovered = over;
            state.has_hovered |= over;
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
pub fn tracked_text(text: &str, tracking: gpui::Pixels) -> gpui::Div {
    let mut row = div().flex().flex_row().items_center().gap(tracking);
    for character in text.chars() {
        row = row.child(gpui::SharedString::from(character.to_string()));
    }
    row
}

/// Tailwind's `--ease-out: cubic-bezier(0, 0, 0.2, 1)`, which `button.css`
/// uses for its background transition.
pub fn ease_out() -> impl Fn(f32) -> f32 {
    cubic_bezier(0.0, 0.0, 0.2, 1.0)
}

/// `focus-ring` is `ring-2`.
pub const FOCUS_RING_WIDTH: f32 = 2.0;
/// `--ring-offset-width: 2px`, used by `focus-ring` but not by
/// `focus-field-ring`.
#[allow(dead_code)]
pub const FOCUS_RING_OFFSET: f32 = 2.0;

pub fn chrome_tick(theme: &ThemeVars) -> herogpui::Separator {
    let color = theme.border;
    herogpui::Separator::new()
        .orientation(herogpui::Orientation::Vertical)
        .mx(px(crate::ui::chrome::SEPARATOR_INSET))
        .sx(move |el| el.h(px(crate::ui::chrome::SEPARATOR_HEIGHT)).bg(color))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_exit_easing_curve_matches_css() {
        let curve = ease_out();
        assert!(curve(0.0).abs() < 1e-3);
        assert!((curve(1.0) - 1.0).abs() < 1e-3);
        let mut previous = 0.0;
        for step in 0..=20 {
            let value = curve(step as f32 / 20.0);
            assert!(value >= previous - 1e-3, "monotonic at {step}");
            previous = value;
        }
        assert!((curve(0.5) - 0.839).abs() < 0.01);
    }

    #[test]
    fn the_entrance_slide_travels_one_spacing_step_toward_its_placement() {
        assert_eq!(OVERLAY_ENTER_SLIDE, 4.0);
        assert_eq!(OVERLAY_ENTER_MS, 150);
        // A menu hangs below its trigger, so it enters from above.
        assert_eq!(EnterFrom::Top, EnterFrom::Top);
        assert_ne!(EnterFrom::Top, EnterFrom::Bottom);
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
