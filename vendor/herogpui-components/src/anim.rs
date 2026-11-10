//! Motion primitives shared by every animated component.
//!
//! HeroUI v3 drives animation from data attributes: overlays fade in on
//! `[data-entering]`, buttons scale on `[data-pressed]`, and everything is
//! suppressed when the user asks for reduced motion — with no opt-in required
//! from the caller.
//!
//! ## Hover slots never request their own frame
//!
//! [`hover_fade`] and `field_chrome_ramp` keep a copy of the pointer state in
//! keyed state, because a colour ramp needs to know which endpoint it is easing
//! towards and gpui hands that out only through a listener. Those listeners
//! record the new value and stop; they must never call `cx.notify()`.
//!
//! gpui reconciles an `on_hover` listener during *paint* whenever the element's
//! interactive state is new, deferring a call so the listener catches up with a
//! pointer that was already inside (`Interactivity::paint` in pinned gpui-pre
//! 0.3.3). That reconciliation is not a user event, so a listener that notifies
//! turns every such frame into a request for another one. It settles only if
//! the element keeps the same id across frames -- and a control whose id comes
//! from a caller-owned state entity does not, if the caller rebuilds that
//! entity in `render`. The result is an unbounded re-render that spins a real
//! app at full CPU and hangs a headless test, where `App::flush_effects` draws
//! every dirty window until none is left.
//!
//! What repaints on a genuine crossing instead is gpui's own hover machinery:
//! when an element carries a hover style, `Interactivity::paint` installs a
//! capture-phase `MouseMoveEvent` handler that notifies the view. That handler
//! is the *only* repaint on offer -- `Window::dispatch_mouse_event` refreshes
//! for an active drag and nothing else -- and it is gated on `hover_style`
//! specifically: a `mouse_cursor`-only element installs the handler but leaves
//! its `hover_state` `None` and so notifies nothing.
//!
//! So the rule is **whoever owns the element's `on_hover` owns its gpui hover
//! style**, and these two helpers own both rather than trusting a caller to
//! supply the second. [`hover_fade`] takes the immediate `hover_border`
//! endpoint that its callers used to apply themselves; `field_chrome_ramp`
//! sets an empty refinement, because every endpoint it has is interpolated and
//! a style swap would snap the colour. When an `Interaction` slot is passed,
//! `util::track_interaction` owns the listener and its notify, and that
//! caller keeps its own hover style. A caller that sets a second hover style on
//! the same element trips gpui's own `debug_assert!("hover style already set")`,
//! so the ownership rule cannot be broken silently.
//!
//! This module is the gpui equivalent. Components call [`entering`] instead of
//! reaching for `with_animation` directly, so the reduced-motion check and the
//! duration/easing live in exactly one place.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use gpui::prelude::FluentBuilder;
use gpui::{
    px, AnimationExt, AnyElement, App, ElementId, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, StyleRefinement, Styled, Window,
};
use herogpui_core::element_id;
use herogpui_theme::ActiveTheme;

/// `[data-entering]` duration for the common case — most overlays are
/// `duration-150`. Panels and `Autocomplete` are 250; see [`Motion`].
pub const ENTERING_MS: u64 = 150;

/// How long `.button`'s fill takes to change, read from its own declaration:
/// `background-color 100ms var(--ease-out)`.
///
/// 150ms is the commonest duration across v3's sheets (88 declarations to
/// 100ms's 33), but the button states its own, and this is the button's.
pub const TRANSITION_MS: u64 = 100;

/// Toast cards translate for 350ms but fade on their own 150ms opacity track.
/// The separate constant keeps that stylesheet timing visible to the shared
/// motion readers without conflating it with the placement motion itself.
pub const TOAST_OPACITY_MS: u64 = 150;

/// `.accordion__trigger`'s opacity/box-shadow transition duration. Accordion
/// has its own 150ms declaration; it must not inherit the button's 100ms hover
/// token when it uses the shared colour-fill machinery.
pub const ACCORDION_TRIGGER_HOVER_MS: u64 = 150;

/// How long a press takes: `transform 250ms var(--ease-smooth)`.
///
/// The instant [`pressed`] still arrives in one frame — gpui's `active` is a
/// style swap with no timeline — but `pressed_with_background_ramp` rides
/// this pinned duration on the components whose stylesheets declare it.
pub const PRESS_MS: u64 = 250;

/// `progress-bar-indeterminate`: one sweep every 1.5 seconds.
pub const PROGRESS_BAR_INDETERMINATE_MS: u64 = 1500;
/// `.progress-bar__fill` width transition duration.
pub const PROGRESS_BAR_FILL_MS: u64 = 300;

/// v3's indeterminate ProgressBar curve.
pub fn progress_bar_indeterminate_ease() -> impl Fn(f32) -> f32 {
    |t| cubic_bezier(0.65, 0.0, 0.35, 1.0, t)
}

/// `@keyframes progress-circle-spin`: one linear turn per second.
pub const PROGRESS_CIRCLE_SPIN_MS: u64 = 1000;
/// `.progress-circle__fill-circle` value changes use the pinned 300ms
/// ease-out stroke transition. The canvas-backed port applies the same
/// timeline to its retained arc fraction.
pub const PROGRESS_CIRCLE_FILL_MS: u64 = 300;

/// Rotation for one `progress-circle-spin` iteration, in radians.
pub fn progress_circle_spin_turn(delta: f32) -> f32 {
    delta.clamp(0.0, 1.0) * std::f32::consts::TAU
}

/// Evaluates a CSS `cubic-bezier(x1, y1, x2, y2)` at `t`.
///
/// v3 names its curves in `--ease-*` tokens and gpui takes an arbitrary easing
/// function, so the real curves can be used rather than approximated by
/// whichever of gpui's two built-ins looks closest.
pub(crate) fn cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32, t: f32) -> f32 {
    // A cubic Bezier from (0,0) to (1,1); `t` is the x we want a y for, so the
    // curve parameter has to be solved for first.
    let bez = |a: f32, b: f32, u: f32| {
        let v = 1.0 - u;
        3.0 * v * v * u * a + 3.0 * v * u * u * b + u * u * u
    };
    let mut lo = 0.0f32;
    let mut hi = 1.0f32;
    let mut u = t;
    // Bisection: monotonic in x, and 24 halvings is well under a pixel.
    for _ in 0..24 {
        let x = bez(x1, x2, u);
        if x < t {
            lo = u;
        } else {
            hi = u;
        }
        u = (lo + hi) * 0.5;
    }
    bez(y1, y2, u)
}

/// v3's `--ease-out` — Tailwind's `ease-out`, `cubic-bezier(0, 0, 0.2, 1)`.
pub fn ease_out() -> impl Fn(f32) -> f32 {
    |t| cubic_bezier(0.0, 0.0, 0.2, 1.0, t)
}

/// One of v3's `--ease-*` curves.
///
/// Named rather than passed as a closure so a [`Motion`] stays `Copy` and can be
/// a `const`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Curve {
    /// `--ease-out`, Tailwind's `ease-out`: `cubic-bezier(0, 0, 0.2, 1)`.
    Out,
    /// `--ease-smooth`, CSS `ease`: `cubic-bezier(0.25, 0.1, 0.25, 1)`.
    Smooth,
    /// `--ease-out-quad`: `cubic-bezier(0.25, 0.46, 0.45, 0.94)`.
    OutQuad,
    /// `--ease-out-fluid`: `cubic-bezier(0.32, 0.72, 0, 1)`.
    OutFluid,
    /// `--ease-out-quart`: `cubic-bezier(0.165, 0.84, 0.44, 1)` — the
    /// close button's transform curve.
    OutQuart,
    /// Constant speed, no easing.
    Linear,
}

impl Curve {
    /// Evaluates the curve at progress `t` (0 to 1), returning the eased progress.
    pub fn at(self, t: f32) -> f32 {
        match self {
            Curve::Out => cubic_bezier(0.0, 0.0, 0.2, 1.0, t),
            Curve::Smooth => cubic_bezier(0.25, 0.1, 0.25, 1.0, t),
            Curve::OutQuad => cubic_bezier(0.25, 0.46, 0.45, 0.94, t),
            Curve::OutFluid => cubic_bezier(0.32, 0.72, 0.0, 1.0, t),
            Curve::OutQuart => cubic_bezier(0.165, 0.84, 0.44, 1.0, t),
            Curve::Linear => t,
        }
    }
}

/// The duration, scale and curve v3 declares for one overlay's transition.
///
/// v3 does **not** animate every overlay the same way, which is what reading the
/// guide rather than the stylesheets had suggested. Each surface names its own
/// `duration-*`, `ease-*` and `zoom-*`, and a modal panel even *shrinks* in from
/// 105% rather than growing from 90%. The constants below are transcribed one
/// per group, and `anim_audit.py` checks them against the CSS.
#[derive(Clone, Copy, Debug)]
pub struct Motion {
    /// Duration in milliseconds.
    pub ms: u64,
    /// The scale the animation starts at (entering) or ends at (exiting).
    /// `1.0` means no scaling — a fade alone.
    pub scale: f32,
    /// Easing curve.
    pub curve: Curve,
}

impl Motion {
    /// `duration-250 ease-out-quad zoom-in-105` — `Modal` and `AlertDialog`
    /// panels, which settle *down* onto the page.
    pub const PANEL_IN: Motion = Motion {
        ms: 250,
        scale: 1.05,
        curve: Curve::OutQuad,
    };
    /// `duration-100 ease-out-quad zoom-out-95`.
    pub const PANEL_OUT: Motion = Motion {
        ms: 100,
        scale: 0.95,
        curve: Curve::OutQuad,
    };

    /// `duration-150 ease-out fade-in-0` — the backdrop behind a panel, which
    /// only fades.
    pub const BACKDROP_IN: Motion = Motion {
        ms: 150,
        scale: 1.0,
        curve: Curve::Out,
    };
    /// `duration-100 ease-out fade-out-0`.
    pub const BACKDROP_OUT: Motion = Motion {
        ms: 100,
        scale: 1.0,
        curve: Curve::Out,
    };

    /// `duration-150 ease-smooth zoom-in-90` — `Popover`, `Dropdown`, `Tooltip`.
    pub const POPOVER_IN: Motion = Motion {
        ms: 150,
        scale: 0.90,
        curve: Curve::Smooth,
    };
    /// `duration-150 ease-smooth zoom-in-95` — `Select`, `ComboBox`, the date
    /// and colour pickers, which start closer to full size.
    pub const LIST_IN: Motion = Motion {
        ms: 150,
        scale: 0.95,
        curve: Curve::Smooth,
    };
    /// `duration-100 ease-smooth zoom-out-95` — the exit both share.
    pub const LIST_OUT: Motion = Motion {
        ms: 100,
        scale: 0.95,
        curve: Curve::Smooth,
    };

    /// `.disclosure__content` is a *transition*, not an `animate-in`: `height
    /// 200ms ease-out-quad, opacity 200ms ease-out`. The collapsible panel
    /// helper measures its natural child extent and drives both properties at
    /// these curves.
    pub const DISCLOSURE: Motion = Motion {
        ms: 200,
        scale: 1.0,
        curve: Curve::OutQuad,
    };

    /// `field-error.css` expands the error row over 350ms with the smooth
    /// curve, independently of its shorter 150ms opacity transition.
    pub const FIELD_ERROR_HEIGHT: Motion = Motion {
        ms: 350,
        scale: 1.0,
        curve: Curve::Smooth,
    };
    /// Opacity transition of the field error row: 150ms with the `Out` curve.
    pub const FIELD_ERROR_OPACITY: Motion = Motion {
        ms: 150,
        scale: 1.0,
        curve: Curve::Out,
    };

    /// `translate 250ms cubic-bezier(0.32, 0.72, 0, 1)` — the drawer's slide,
    /// which `drawer.css` gives its own `--drawer-enter-*` tokens.
    pub const DRAWER_IN: Motion = Motion {
        ms: 250,
        scale: 1.0,
        curve: Curve::OutFluid,
    };
    /// `--drawer-exit-duration: 200ms`, same curve.
    pub const DRAWER_OUT: Motion = Motion {
        ms: 200,
        scale: 1.0,
        curve: Curve::OutFluid,
    };

    /// Drawer-specific backdrop fade from `drawer.css`: it follows the
    /// panel's fluid enter curve instead of the shared modal backdrop token.
    pub const DRAWER_BACKDROP_IN: Motion = Motion {
        ms: 250,
        scale: 1.0,
        curve: Curve::OutFluid,
    };
    /// Drawer-specific backdrop exit from `drawer.css`.
    pub const DRAWER_BACKDROP_OUT: Motion = Motion {
        ms: 200,
        scale: 1.0,
        curve: Curve::OutFluid,
    };

    /// `duration-250 ease-out-fluid zoom-in-95` — `Autocomplete` alone.
    pub const FLUID_IN: Motion = Motion {
        ms: 250,
        scale: 0.95,
        curve: Curve::OutFluid,
    };
    /// `duration-100 ease-out-quad zoom-out-95`.
    pub const FLUID_OUT: Motion = Motion {
        ms: 100,
        scale: 0.95,
        curve: Curve::OutQuad,
    };

    /// `350ms ease-out-fluid` — Toast's placement-aware card translation and
    /// fade when a new notification enters the queue.
    pub const TOAST_IN: Motion = Motion {
        ms: 350,
        scale: 1.0,
        curve: Curve::OutFluid,
    };
    /// `350ms ease-out-fluid` — the frontmost Toast card leaves toward the
    /// placement edge while its opacity settles to zero.
    pub const TOAST_OUT: Motion = Motion {
        ms: 350,
        scale: 1.0,
        curve: Curve::OutFluid,
    };
    /// `200ms ease-out-fluid` and `scale(.96)` — a non-frontmost Toast card
    /// retires in an expanded/collapsed stack without leaving its slot.
    pub const TOAST_STACK_OUT: Motion = Motion {
        ms: 200,
        scale: 0.96,
        curve: Curve::OutFluid,
    };
}

/// v3's `--ease-smooth`, which is CSS `ease`: `cubic-bezier(0.25, 0.1, 0.25, 1)`.
pub fn ease_smooth() -> impl Fn(f32) -> f32 {
    |t| cubic_bezier(0.25, 0.1, 0.25, 1.0, t)
}

/// Tailwind's default transition timing — `cubic-bezier(0.4, 0, 0.2, 1)` — the
/// curve a `transition-all duration-*` utility runs when the rule names no
/// `--ease-*` token. v3's checkmark undraw rides it: selected, its rule is
/// `stroke-dashoffset 150ms linear 15ms`; unselecting falls back to the base
/// `transition-all duration-200`.
pub(crate) fn tailwind_default_ease() -> impl Fn(f32) -> f32 {
    |t| cubic_bezier(0.4, 0.0, 0.2, 1.0, t)
}

/// The transition on v3's accordion and disclosure indicators. Both stylesheets
/// keep one down-chevron in the DOM and rotate it 180 degrees when expanded;
/// the two components share this keyed implementation so a quick reversal
/// resumes from the frame that was actually painted. The bare Tailwind
/// `transition` utility uses the pinned default curve, rather than the named
/// `ease-smooth` token used by several other HeroUI transitions.
#[allow(dead_code)]
pub(crate) const INDICATOR_ROTATION_MS: u64 = 250;

/// HeroUI's calendar year-picker indicator uses the same transition timing
/// as its trigger heading color and rotates the down chevron through one
/// quarter turn when the picker opens.
pub(crate) const YEAR_PICKER_INDICATOR_MS: u64 = 150;
pub(crate) const YEAR_PICKER_INDICATOR_ANGLE: f32 = std::f32::consts::FRAC_PI_2;

/// Render a 16px indicator SVG with the same state-driven rotation used by
/// HeroUI's `.accordion__indicator` and `.disclosure__indicator` rules.
///
/// `svg` must already carry its path and visual styling. The wrapper animates
/// only the SVG transform, so its layout box stays fixed while the angle moves
/// from the live frame to the new expanded endpoint. Reduced motion still
/// paints the endpoint immediately and does not schedule a frame.
pub(crate) fn rotating_indicator(
    id: &ElementId,
    expanded: bool,
    svg: gpui::Svg,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    // Keep the chevron duration tied to the disclosure motion token so an
    // application theme can audit one source of truth for both the panel and
    // its indicator (the stock value is `Motion::DISCLOSURE.ms` = 250ms).
    rotating_indicator_with_duration(id, expanded, svg, Motion::DISCLOSURE.ms, window, cx)
}

/// Render a rotating SVG indicator with an owner-specific transition length.
///
/// HeroUI's disclosure indicators use the bare 250ms transition, while the
/// field/picker indicators use the same transform with a 150ms duration. Keep
/// the keyed/live-frame behavior shared so both families reverse from the
/// frame that was actually painted.
pub(crate) fn rotating_indicator_with_duration(
    id: &ElementId,
    expanded: bool,
    svg: gpui::Svg,
    duration_ms: u64,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    rotating_indicator_with_angle(
        id,
        expanded,
        svg,
        duration_ms,
        std::f32::consts::PI,
        window,
        cx,
    )
}

/// Render an indicator with an owner-specific rotation angle.
///
/// Most HeroUI disclosure-like indicators turn a down chevron through 180°.
/// Calendar year-picker indicators are the exception: the pinned stylesheet
/// turns the same glyph through 90° over 150ms. Keeping the angle in this
/// shared helper preserves keyed reversal and reduced-motion behavior without
/// making Calendar and RangeCalendar invent separate animation state.
pub(crate) fn rotating_indicator_with_angle(
    id: &ElementId,
    expanded: bool,
    svg: gpui::Svg,
    duration_ms: u64,
    angle: f32,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    rotating_indicator_with_angle_easing(
        id,
        expanded,
        svg,
        duration_ms,
        angle,
        IndicatorEasing::TailwindDefault,
        window,
        cx,
    )
}

/// Render an indicator with HeroUI's explicit `--ease-out` curve.
///
/// Calendar year-picker CSS names this curve directly, unlike the bare
/// transition used by Accordion and Disclosure. Its keyed state is shared
/// with the general angle helper, so interrupted open/close motion still
/// resumes from the painted frame.
pub(crate) fn rotating_indicator_with_angle_ease_out(
    id: &ElementId,
    expanded: bool,
    svg: gpui::Svg,
    duration_ms: u64,
    angle: f32,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    rotating_indicator_with_angle_easing(
        id,
        expanded,
        svg,
        duration_ms,
        angle,
        IndicatorEasing::EaseOut,
        window,
        cx,
    )
}

#[derive(Clone, Copy)]
enum IndicatorEasing {
    TailwindDefault,
    EaseOut,
}

#[allow(clippy::too_many_arguments)] // the two easing wrappers pass their parameters straight through
fn rotating_indicator_with_angle_easing(
    id: &ElementId,
    expanded: bool,
    svg: gpui::Svg,
    duration_ms: u64,
    angle: f32,
    easing: IndicatorEasing,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let reduce_motion = ActiveTheme::reduce_motion(cx);
    let target = if expanded { 1.0 } else { 0.0 };
    let mut rotation = Tween::keyed(id, "indicator-rotation", target, window, cx);
    rotation.snap_if_reduced(reduce_motion);

    if !rotation.animates(reduce_motion) {
        rotation.settle();
        return svg
            .with_transformation(gpui::Transformation::rotate(gpui::radians(
                rotation.target() * angle,
            )))
            .into_any_element();
    }

    let from = rotation.from();
    let to = rotation.target();
    let value = rotation.value();
    let animation = gpui::Animation::new(Duration::from_millis(duration_ms));
    let animation = match easing {
        IndicatorEasing::TailwindDefault => animation.with_easing(tailwind_default_ease()),
        IndicatorEasing::EaseOut => animation.with_easing(ease_out()),
    };
    svg.with_animation(
        element_id::indexed(id, "indicator-rotation", rotation.generation()),
        animation,
        move |svg, delta| {
            let progress = from + (to - from) * delta;
            value.set(progress);
            svg.with_transformation(gpui::Transformation::rotate(gpui::radians(
                progress * angle,
            )))
        },
    )
    .into_any_element()
}

/// The keyed tween bookkeeping the checkbox's motion slots share: the last
/// target, the generation that advanced with it, the snapshot a new animation
/// starts from, and the live value an interrupted one resumes from.
///
/// `target`/`from`/`generation` live in the keyed state; `value` is an
/// `Rc<Cell<_>>` the animation closure writes, so the snapshot a later
/// generation takes is the frame actually on screen.
#[derive(Clone)]
pub(crate) struct Tween<T: Copy + PartialEq + 'static> {
    target: T,
    generation: usize,
    from: T,
    value: Rc<Cell<T>>,
}

impl<T: Copy + PartialEq + 'static> Tween<T> {
    fn settled(value: T) -> Self {
        Self {
            target: value,
            generation: 0,
            from: value,
            value: Rc::new(Cell::new(value)),
        }
    }

    /// Reads this slot's keyed state for `target`, advancing the generation —
    /// and re-snapshotting the rendered value as the new start — when the
    /// target changed.
    pub(crate) fn keyed(
        id: &ElementId,
        tag: &'static str,
        target: T,
        window: &mut Window,
        cx: &mut App,
    ) -> Self {
        Self::keyed_with_initial(id, tag, target, None, window, cx)
    }

    /// Reads a keyed slot, optionally seeding its first frame from an
    /// explicit value. The seed is used only when the slot is first created;
    /// later target changes still resume from the live value written by the
    /// previous animation generation.
    pub(crate) fn keyed_with_initial(
        id: &ElementId,
        tag: &'static str,
        target: T,
        initial_from: Option<T>,
        window: &mut Window,
        cx: &mut App,
    ) -> Self {
        let state =
            window.use_keyed_state(element_id::scoped(id, tag), cx, |_, _| match initial_from {
                Some(from) if from != target => Self {
                    target,
                    generation: 1,
                    from,
                    value: Rc::new(Cell::new(from)),
                },
                _ => Self::settled(target),
            });
        let mut current = state.read(cx).clone();
        if current.target != target {
            current.target = target;
            current.generation = current.generation.wrapping_add(1);
            current.from = current.value.get();
            state.update(cx, |stored, _| *stored = current.clone());
        }
        current
    }

    /// Lands the value on the target under reduced motion — the state still
    /// applies, with no animation mounted.
    pub(crate) fn snap_if_reduced(&mut self, reduce_motion: bool) {
        if reduce_motion && self.value.get() != self.target {
            self.from = self.target;
            self.value.set(self.target);
        }
    }

    /// Whether this frame mounts an animation: a real change happened (the
    /// generation moved), motion is allowed, and the live value is short of
    /// the target — which also keeps a finished tween from re-mounting at
    /// rest.
    pub(crate) fn animates(&self, reduce_motion: bool) -> bool {
        self.generation != 0 && !reduce_motion && self.value.get() != self.target
    }

    /// Lands exactly on the target, the state a settled tween paints.
    pub(crate) fn settle(&self) {
        self.value.set(self.target);
    }

    pub(crate) fn target(&self) -> T {
        self.target
    }

    pub(crate) fn from(&self) -> T {
        self.from
    }

    pub(crate) fn generation(&self) -> usize {
        self.generation
    }

    /// The live value, shared with the keyed state and the animation closure.
    pub(crate) fn value(&self) -> Rc<Cell<T>> {
        Rc::clone(&self.value)
    }
}

/// The scale v3 applies to a pressed control (`transform: scale(0.97)`).
pub const PRESSED_SCALE: f32 = 0.97;

/// The other scales v3 presses with: a menu row and a pagination link squeeze
/// less than a button, a calendar cell and a radio control more, and a range
/// calendar cell most.
pub const PRESSED_SCALE_SUBTLE: f32 = 0.98;

/// `list-box-item.css`'s pressed transform: option rows ease to 98% over
/// 250ms with the pinned quart curve. Keep this beside the shared press ramp
/// so ListBox and Select cannot drift into different row motion.
pub(crate) const LIST_ITEM_PRESS: PressTiming = PressTiming {
    transform_ms: 250,
    transform: Curve::OutQuart,
    background: None,
};
/// Pressed scale of 0.96, used for large buttons and pagination links.
pub const PRESSED_SCALE_FIRM: f32 = 0.96;
/// Pressed scale of 0.95, used for smaller controls such as calendar cells and radio controls.
pub const PRESSED_SCALE_DEEP: f32 = 0.95;
/// Pressed scale of 0.9, used for range calendar cells.
pub const PRESSED_SCALE_RANGE: f32 = 0.9;

/// The inset that shrinks a control of `height` by a scale about its
/// centre.
pub fn pressed_inset(height: gpui::Pixels) -> gpui::Pixels {
    inset_for(height, PRESSED_SCALE)
}

/// The inset that shrinks `height` by `scale`, centred.
fn inset_for(height: gpui::Pixels, scale: f32) -> gpui::Pixels {
    px(f32::from(height) * (1.0 - scale) / 2.0)
}

#[cfg(test)]
fn shrink(value: gpui::Pixels, by: gpui::Pixels) -> gpui::Pixels {
    px((f32::from(value) - f32::from(by)).max(0.0))
}

/// `value` scaled by `scale`.
fn scaled_by(value: gpui::Pixels, scale: f32) -> gpui::Pixels {
    px(f32::from(value) * scale)
}

/// Scale the skin's resolved pixel corners independently. Unsupported Rems
/// retain the existing PressBox fallback; callers keep them on the root.
pub(crate) fn pressed_corners(
    corners: &gpui::CornersRefinement<gpui::AbsoluteLength>,
    fallback: gpui::Pixels,
    scale: f32,
) -> gpui::Corners<Option<gpui::Pixels>> {
    let scale_corner = |corner| {
        let radius = match corner {
            Some(gpui::AbsoluteLength::Pixels(radius)) => radius,
            _ => fallback,
        };
        Some(scaled_by(radius, scale))
    };
    gpui::Corners {
        top_left: scale_corner(corners.top_left),
        top_right: scale_corner(corners.top_right),
        bottom_right: scale_corner(corners.bottom_right),
        bottom_left: scale_corner(corners.bottom_left),
    }
}

/// Carry the resting corner shape onto the stable press slot.
///
/// `pressed_with_optional_background` wraps a button's painted skin in a
/// footprint-preserving slot.  The slot is the element that owns the focus
/// ring, so leaving its corners at the default zero radius turns a rounded
/// button's ring into a square.  Keep partial group corners partial; only a
/// completely unspecified refinement needs the PressBox fallback.
fn resting_slot_corners(
    corners: &gpui::CornersRefinement<gpui::AbsoluteLength>,
    fallback: gpui::Pixels,
) -> gpui::Corners<Option<gpui::Pixels>> {
    let any_specified = corners.top_left.is_some()
        || corners.top_right.is_some()
        || corners.bottom_right.is_some()
        || corners.bottom_left.is_some();
    let resolve = |corner| match corner {
        Some(gpui::AbsoluteLength::Pixels(radius)) => Some(radius),
        Some(_) => Some(fallback),
        None if any_specified => None,
        None => Some(fallback),
    };
    gpui::Corners {
        top_left: resolve(corners.top_left),
        top_right: resolve(corners.top_right),
        bottom_right: resolve(corners.bottom_right),
        bottom_left: resolve(corners.bottom_left),
    }
}

/// Everything a pressed control scales down.
#[derive(Clone, Copy, Debug)]
pub struct PressBox {
    /// Resting height.
    pub height: gpui::Pixels,
    /// Horizontal padding for a control that sizes to its content, or `None`
    /// for one with a fixed width.
    pub padding_x: Option<gpui::Pixels>,
    /// Fixed width, for a square icon-only control.
    pub width: Option<gpui::Pixels>,
    /// Minimum width, which has to scale too or it pins the box at full size.
    pub min_width: Option<gpui::Pixels>,
    /// Font size.
    pub text_size: gpui::Pixels,
    /// Line height.
    pub line_height: gpui::Pixels,
    /// Gap between children.
    pub gap: gpui::Pixels,
    /// Corner radius.
    pub radius: gpui::Pixels,
    /// How far the press scales. v3 uses 0.97 for a button, 0.98 for a menu row,
    /// 0.96 and 0.95 for the smaller controls, so it is per control rather than
    /// one constant.
    pub scale: f32,
    /// False for a full-width control, whose width is its parent's: a
    /// horizontal margin there would overflow rather than inset.
    pub shrink_x: bool,
}

/// Applies v3's `[data-pressed]` press.
///
/// v3 presses with `transform: scale(s)` about the centre, and gpui 0.2.2 has
/// no paint transform — so the pressed element becomes the painted *skin*
/// inside a stable slot root: the slot keeps the resting footprint, and the
/// skin downscales about the centre through fractional absolute insets. The
/// scale therefore never reflows the slot's neighbours, and the skin's own
/// content (label, padding) is carried along by its box.
///
/// Returns `el` untouched under reduced motion.
pub fn pressed(el: gpui::Stateful<gpui::Div>, b: PressBox, cx: &App) -> gpui::Stateful<gpui::Div> {
    pressed_with_optional_background(el, b, None, cx)
}

/// Applies the same press geometry and an active-state background in one
/// refinement, for controls whose CSS changes both on `[data-pressed]`.
pub fn pressed_with_background(
    el: gpui::Stateful<gpui::Div>,
    b: PressBox,
    background: gpui::Hsla,
    cx: &App,
) -> gpui::Stateful<gpui::Div> {
    pressed_with_optional_background(el, b, Some(background), cx)
}

#[derive(gpui::IntoElement)]
struct PressSlotContent {
    id: ElementId,
    child: AnyElement,
    height: gpui::Pixels,
    frozen: Option<bool>,
}

impl gpui::RenderOnce for PressSlotContent {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let width = window
            .use_keyed_state(element_id::scoped(&self.id, "rest-width"), cx, |_, _| {
                Rc::new(Cell::new(None::<f32>))
            })
            .read(cx)
            .clone();
        let interaction = self.frozen.is_none().then(|| {
            crate::util::interaction(element_id::scoped(&self.id, "instant-press"), window, cx)
        });
        let frozen = self
            .frozen
            .unwrap_or_else(|| interaction.as_ref().is_some_and(|state| state.read(cx).1));
        let recorded = width.clone();
        let mut content = gpui::div()
            .id(self.id)
            .relative()
            .flex()
            .flex_shrink_0()
            .items_center()
            .justify_center()
            .min_h(self.height)
            .when(frozen, |el| {
                el.when_some(width.get(), |el, width| el.w(px(width)))
            })
            .child(self.child)
            .child(
                gpui::canvas(
                    |_, _, _| {},
                    move |bounds, _, _, _| {
                        if !frozen {
                            recorded.set(Some(f32::from(bounds.size.width)));
                        }
                    },
                )
                .absolute()
                .inset_0(),
            );
        if let Some(interaction) = interaction {
            content = crate::util::track_interaction(content, &interaction);
        }
        content
    }
}

fn pressed_with_optional_background(
    mut el: gpui::Stateful<gpui::Div>,
    b: PressBox,
    background: Option<gpui::Hsla>,
    cx: &App,
) -> gpui::Stateful<gpui::Div> {
    if ActiveTheme::reduce_motion(cx) {
        return match background {
            Some(background) => el.active(move |style| style.bg(background)),
            None => el,
        };
    }
    // The skin is sized by all four fractional insets — `(1 - s) / 2` of each
    // slot axis is exactly the gap a scale of `s` leaves on that side — with
    // explicit `Auto` extents overriding any way the caller sized the skin
    // (an explicit `h`, `w_full`, `min_h`). Inset sizing needs no percentage
    // resolution, which matters because the slot's height is only a minimum:
    // a percentage height against it would not resolve and the bottom edge
    // would stay put.
    let inset = gpui::DefiniteLength::Fraction((1.0 - b.scale) / 2.0);
    let pressed_min_height = scaled_by(b.height, b.scale);
    let pressed_radius = scaled_by(b.radius, b.scale);
    let corners = pressed_corners(&el.style().corner_radii, b.radius, b.scale);

    // `active` state only exists for elements with a hitbox, and a hitbox is
    // only inserted for elements that track focus, set a cursor, or listen to
    // the mouse. Skins whose caller keeps every handler on the slot (the
    // calendar cells) would press invisibly; arm a no-op listener so the
    // press registers. `on_mouse_down` appends, so a caller's own listener
    // is untouched.
    let el = el.on_mouse_down(gpui::MouseButton::Left, |_, _, _| {});
    let mut el = el.active(move |s: StyleRefinement| {
        let s = match background {
            Some(background) => s.bg(background),
            None => s,
        };
        crate::util::round_sx_corners(
            s.absolute()
                .left(inset)
                .right(inset)
                .top(inset)
                .bottom(inset)
                .w(gpui::Length::Auto)
                .h(gpui::Length::Auto)
                .min_h(pressed_min_height)
                .rounded(pressed_radius),
            &corners,
        )
    });

    let Some(id) = el.interactivity().element_id.clone() else {
        // No identity to anchor the slot's resting width on; skip the scale
        // rather than risk a reflow.
        return el;
    };
    let slot_corners = resting_slot_corners(&el.style().corner_radii, b.radius);
    press_slot(el, id, b, &slot_corners, None)
}

/// Wraps a built skin in the stable press slot: the resting footprint that
/// keeps the caller's id, hit-testing, focus and focus-ring geometry while
/// the skin inside carries the press.
fn press_slot(
    el: impl IntoElement,
    id: ElementId,
    b: PressBox,
    slot_corners: &gpui::Corners<Option<gpui::Pixels>>,
    frozen: Option<bool>,
) -> gpui::Stateful<gpui::Div> {
    // The slot keeps the resting footprint: fixed where the caller gave us a
    // width or asked for full width, and otherwise the skin's resting width
    // recorded while it hugged the slot at rest. Callers must add every
    // visual child to the skin *before* pressing — children added after land
    // on the slot and fight the skin for its width.
    let mut slot = gpui::div()
        .id(element_id::scoped(&id, "press-slot"))
        .relative()
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        // A minimum, not a fixed height: a tall content row grows the slot
        // past the control minimum instead of being clipped to it.
        .min_h(b.height);
    slot = crate::util::round_sx_corners(slot, slot_corners);
    match b.width {
        Some(width) => slot.w(width).child(el),
        None if !b.shrink_x => slot.w_full().child(el),
        None => slot.child(PressSlotContent {
            id: element_id::scoped(&id, "content-fit"),
            child: el.into_any_element(),
            height: b.height,
            frozen,
        }),
    }
}

/// Resting corner radii resolved against a fallback, ready to be rescaled per
/// animation frame by [`scale_corners`].
fn resting_corner_radii(
    el: &mut gpui::Stateful<gpui::Div>,
    fallback: gpui::Pixels,
) -> gpui::Corners<Option<gpui::Pixels>> {
    pressed_corners(&el.style().corner_radii, fallback, 1.0)
}

/// The corners a scale `s` paints: every resolved radius shrinks about the
/// centre with the box.
fn scale_corners(
    corners: &gpui::Corners<Option<gpui::Pixels>>,
    s: f32,
) -> gpui::Corners<Option<gpui::Pixels>> {
    let scale = |radius: Option<gpui::Pixels>| radius.map(|radius| scaled_by(radius, s));
    gpui::Corners {
        top_left: scale(corners.top_left),
        top_right: scale(corners.top_right),
        bottom_right: scale(corners.bottom_right),
        bottom_left: scale(corners.bottom_left),
    }
}

/// Lays the skin out at scale `s`: the four fractional insets, the `Auto`
/// extents that let them rule the box, and the rescaled minimum height and
/// corners. Shared verbatim by the settled and animated paint paths so the
/// ramp's frames land exactly where the instant press did.
fn skin_at_scale(
    skin: gpui::Stateful<gpui::Div>,
    s: f32,
    b: &PressBox,
    resting: &gpui::Corners<Option<gpui::Pixels>>,
) -> gpui::Stateful<gpui::Div> {
    let inset = gpui::DefiniteLength::Fraction((1.0 - s) / 2.0);
    crate::util::round_sx_corners(
        skin.absolute()
            .left(inset)
            .right(inset)
            .top(inset)
            .bottom(inset)
            .w(gpui::Length::Auto)
            .h(gpui::Length::Auto)
            .min_h(scaled_by(b.height, s))
            .rounded(scaled_by(b.radius, s)),
        &scale_corners(resting, s),
    )
}

/// One component's pinned press transition, read from its stylesheet's
/// `transition` block: the `transform` track and the `background-color` track
/// carry separate durations and easings, exactly as the cascade interpolates
/// them independently.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PressTiming {
    /// The `transform` duration and easing.
    pub transform_ms: u64,
    pub transform: Curve,
    /// The `background-color` duration and easing, or `None` when the pressed
    /// state changes no background — the track then has no endpoints to ease
    /// between.
    pub background: Option<(u64, Curve)>,
}

/// `button.css` and `toggle-button.css`, lines 11-15 / 13-17:
/// `transform 250ms var(--ease-smooth), background-color 100ms
/// var(--ease-out), box-shadow 100ms var(--ease-out)`. The pressed state
/// changes no box-shadow, so only the first two tracks have endpoints here.
pub(crate) const BUTTON_PRESS: PressTiming = PressTiming {
    transform_ms: PRESS_MS,
    transform: Curve::Smooth,
    background: Some((100, Curve::Out)),
};

/// `close-button.css`, lines 14-19: `transform 250ms var(--ease-out-quart),
/// color 150ms var(--ease-out), background-color 100ms var(--ease-out),
/// box-shadow 150ms var(--ease-out)`. Its pressed state declares only
/// `transform: scale(0.93)` — the colour tracks have no pressed endpoints.
pub(crate) const CLOSE_BUTTON_PRESS: PressTiming = PressTiming {
    transform_ms: 250,
    transform: Curve::OutQuart,
    background: None,
};

/// Applies a component's `[data-pressed]` press as the interpolated ramp its
/// stylesheet's `transition` block declares, instead of the one-frame swap
/// [`pressed_with_background`] paints.
///
/// The geometry and the fill ride separate timelines — `transform` and
/// `background-color` interpolate on their own pinned durations and easings —
/// the way the cascade runs them as independent transitions. Each track keeps
/// a [`Tween`] keyed by the button's own element id, so sibling instances
/// never share a timeline, and a release mid-press re-snapshots the frame
/// actually on screen as the new start rather than restarting from rest.
///
/// The skin stays the listener-free visual child of the stable press slot:
/// the animation ids above it change per generation, and everything that
/// owns state — the slot's id, hit-testing, focus and focus ring — never
/// moves. Reduced motion snaps both tracks to their endpoints, matching
/// `motion-reduce:transition-none`, which removes the timing but keeps the
/// pressed property values.
///
/// `endpoints` is the colour track's `(resting, pressed)` pair, resolved by
/// the caller from its variant; `None` for a component whose pressed state
/// changes no background. `interaction` supplies the pressed bit when the
/// caller already tracks one (a `content` closure's slot); otherwise a
/// per-instance slot is created here and `util::track_interaction` is wired
/// onto the returned slot — the same handlers that keep a render prop's
/// `isPressed` current, including the keyboard press and the release outside
/// the control's bounds.
pub(crate) fn pressed_with_background_ramp(
    mut el: gpui::Stateful<gpui::Div>,
    b: PressBox,
    endpoints: Option<(gpui::Hsla, gpui::Hsla)>,
    timing: PressTiming,
    interaction: Option<&crate::util::Interaction>,
    window: &mut Window,
    cx: &mut App,
) -> gpui::Stateful<gpui::Div> {
    // A full-width control has a stable slot and a painted skin with the same
    // inline extent. Without this, the slot centers the skin at its intrinsic
    // min-content width: list rows become narrow, long labels never wrap, and
    // absolute indicators/focus rings appear to move when row padding changes.
    // Content-sized controls keep their intrinsic width and use the recorder
    // below instead.
    if !b.shrink_x {
        el = el.w_full();
    }
    let slot_corners = resting_slot_corners(&el.style().corner_radii, b.radius);
    let Some(id) = el.interactivity().element_id.clone() else {
        // No identity to key the tweens or the slot's resting width on; skip
        // the scale rather than risk a reflow, like the instant press.
        return el;
    };

    // The pressed bit a frame behind the pointer, exactly like a render
    // prop's: a handler stashes it in the keyed slot and this render reads it.
    let (tracked, pressed) = match interaction {
        Some(slot) => (None, slot.read(cx).1),
        None => {
            let tracked =
                crate::util::interaction(element_id::scoped(&id, "press-track"), window, cx);
            let pressed = tracked.read(cx).1;
            (Some(tracked), pressed)
        }
    };

    let (resting, pressed_bg) = endpoints.unwrap_or_default();
    let reduce = ActiveTheme::reduce_motion(cx);
    let mut scale_tween = Tween::keyed(
        &id,
        "press-transform",
        if pressed { b.scale } else { 1.0 },
        window,
        cx,
    );
    let mut color_tween = endpoints.map(|_| {
        Tween::keyed(
            &id,
            "press-bg",
            if pressed { pressed_bg } else { resting },
            window,
            cx,
        )
    });
    scale_tween.snap_if_reduced(reduce);
    if let Some(tween) = &mut color_tween {
        tween.snap_if_reduced(reduce);
    }

    let resting_corners = resting_corner_radii(&mut el, b.radius);
    let animates = scale_tween.animates(reduce)
        || color_tween
            .as_ref()
            .is_some_and(|tween| tween.animates(reduce));

    let skin = if animates {
        // Each track mounts keyed by its own generation: an interrupted flip
        // re-keys only the track that turned around, and re-renders while it
        // runs paint the same delta against the same shared skin.
        // `with_animation` keeps its own frames coming until it settles, and
        // every painted value is written back into the tween, which is where
        // a later generation resumes from.
        let transform_ms = timing.transform_ms;
        let transform = timing.transform;
        let scale_from = scale_tween.from();
        let scale_to = scale_tween.target();
        let scale_value = scale_tween.value();
        let transform_id = element_id::indexed(&id, "press-transform", scale_tween.generation());
        let geometry = move |delta: f32| {
            let s = if delta >= 1.0 {
                scale_to
            } else {
                scale_from + (scale_to - scale_from) * delta
            };
            scale_value.set(s);
            s
        };
        match color_tween {
            Some(tween) => {
                let (color_from, color_to) = (tween.from(), tween.target());
                let color_value = tween.value();
                let (bg_ms, bg_curve) = timing.background.unwrap_or((100, Curve::Out));
                let animated = el
                    .with_animation(
                        element_id::indexed(&id, "press-bg", tween.generation()),
                        gpui::Animation::new(Duration::from_millis(bg_ms))
                            .with_easing(move |t| bg_curve.at(t)),
                        move |skin, delta| {
                            let color = if delta >= 1.0 {
                                color_to
                            } else {
                                herogpui_core::mix_oklab(color_from, color_to, delta)
                            };
                            color_value.set(color);
                            skin.bg(color)
                        },
                    )
                    .with_animation(
                        transform_id,
                        transform_animation(transform_ms, transform),
                        move |el, delta| {
                            let s = geometry(delta);
                            el.map_element(|skin| skin_at_scale(skin, s, &b, &resting_corners))
                        },
                    );
                animated.into_any_element()
            }
            None => {
                let animated = el.with_animation(
                    transform_id,
                    transform_animation(transform_ms, transform),
                    move |skin, delta| {
                        let s = geometry(delta);
                        skin_at_scale(skin, s, &b, &resting_corners)
                    },
                );
                animated.into_any_element()
            }
        }
    } else {
        // Settled: the tween's painted value is the state. At rest the skin
        // stays exactly as the caller built it; pressed it takes the final
        // geometry and fill with no animation mounted.
        let s = scale_tween.value().get();
        let mut skin = el;
        #[allow(clippy::float_cmp)] // untouched is exactly the identity scale
        if s != 1.0 {
            skin = skin_at_scale(skin, s, &b, &resting_corners);
            // The pressed fill only exists when the caller named endpoints; a
            // transform-only press (CloseButton) leaves the skin's own
            // background — its stylesheet changes no colour on `:active`.
            if pressed && endpoints.is_some() {
                skin = skin.bg(pressed_bg);
            }
        }
        skin.into_any_element()
    };

    let frozen = animates || (scale_tween.value().get() - 1.0).abs() > f32::EPSILON;
    let slot = press_slot(skin, id, b, &slot_corners, Some(frozen));
    match interaction {
        Some(_) => slot,
        None => crate::util::track_interaction(
            slot,
            tracked
                .as_ref()
                .expect("a press ramp without an interaction slot creates one"),
        ),
    }
}

/// The `transform` track's pinned duration and curve, as an [`gpui::Animation`].
fn transform_animation(ms: u64, curve: Curve) -> gpui::Animation {
    gpui::Animation::new(Duration::from_millis(ms)).with_easing(move |t| curve.at(t))
}

/// `[data-exiting]` duration. Every overlay in v3 leaves in `duration-100`.
pub const EXITING_MS: u64 = 100;

/// Everything an entering overlay grows from `ZOOM_FROM` to full size.
///
/// Every field is optional because the overlays differ in what they know about
/// themselves: a `Modal` has a width, a `Popover` only its padding, type and
/// corner radius. Whatever is supplied is scaled; whatever is not keeps its
/// size, so a panel sized by its content grows by its chrome alone.
#[derive(Clone, Copy, Debug, Default)]
pub struct ZoomBox {
    /// Width, if known.
    pub width: Option<gpui::Pixels>,
    /// Height, if known.
    pub height: Option<gpui::Pixels>,
    /// Horizontal padding, if known.
    pub padding_x: Option<gpui::Pixels>,
    /// Vertical padding, if known.
    pub padding_y: Option<gpui::Pixels>,
    /// Top padding, if known; applied after `padding_y`.
    pub padding_top: Option<gpui::Pixels>,
    /// Bottom padding, if known; applied after `padding_y`.
    pub padding_bottom: Option<gpui::Pixels>,
    /// Gap between children, if known.
    pub gap: Option<gpui::Pixels>,
    /// Font size, if known.
    pub text_size: Option<gpui::Pixels>,
    /// Line height, if known.
    pub line_height: Option<gpui::Pixels>,
    /// Corner radius, if known.
    pub radius: Option<gpui::Pixels>,
    /// Optional placement-relative entry offset. A positive value starts on
    /// the corresponding physical side and eases back to zero with the panel.
    pub slide_x: Option<gpui::Pixels>,
    /// Vertical counterpart of `slide_x`.
    pub slide_y: Option<gpui::Pixels>,
}

impl ZoomBox {
    /// The box for a floating panel: its padding and corner radius, with no
    /// fixed extent.
    pub fn panel(padding_y: gpui::Pixels, radius: gpui::Pixels) -> Self {
        Self {
            padding_y: Some(padding_y),
            radius: Some(radius),
            ..Default::default()
        }
    }

    /// Sets the horizontal padding.
    pub fn padding_x(mut self, padding_x: gpui::Pixels) -> Self {
        self.padding_x = Some(padding_x);
        self
    }

    /// Adds a fixed width, for a panel that has one.
    pub fn sized(mut self, width: gpui::Pixels) -> Self {
        self.width = Some(width);
        self
    }

    /// Adds the panel's type size, which grows with the box.
    pub fn text(mut self, text_size: gpui::Pixels) -> Self {
        self.text_size = Some(text_size);
        self
    }
}

fn lerp(value: gpui::Pixels, factor: f32) -> gpui::Pixels {
    px(f32::from(value) * factor)
}

/// v3's `[data-entering]` in full: `zoom-in-90 fade-in-0 duration-200`.
///
/// gpui 0.2.2 has no transform for a div, so the zoom is reproduced the same
/// way [`pressed`] reproduces `scale(0.97)` — by growing the metrics the panel
/// is made of, including its **type size**, which gpui accepts fractionally.
/// What a real `scale()` would also carry, and this does not, is a child whose
/// size the caller fixed: an icon or an image inside the panel keeps its size
/// while the chrome around it grows.
///
/// Returns `el` untouched under reduced motion.
pub fn entering_zoom<E>(
    el: E,
    id: impl Into<ElementId>,
    b: ZoomBox,
    m: Motion,
    cx: &App,
) -> AnyElement
where
    E: IntoElement + Styled + 'static,
{
    if ActiveTheme::reduce_motion(cx) {
        return el.into_any_element();
    }

    el.with_animation(
        id.into(),
        gpui::Animation::new(Duration::from_millis(m.ms)).with_easing(move |t| m.curve.at(t)),
        move |el, delta| {
            // `scale` may be above 1.0: a modal panel settles down from 105%.
            let f = m.scale + (1.0 - m.scale) * delta;
            let mut el = el.opacity(delta);
            if let Some(w) = b.width {
                el = el.w(lerp(w, f));
            }
            if let Some(h) = b.height {
                el = el.h(lerp(h, f));
            }
            if let Some(p) = b.padding_x {
                el = el.px(lerp(p, f));
            }
            if let Some(p) = b.padding_y {
                el = el.py(lerp(p, f));
            }
            if let Some(p) = b.padding_top {
                el = el.pt(lerp(p, f));
            }
            if let Some(p) = b.padding_bottom {
                el = el.pb(lerp(p, f));
            }
            if let Some(g) = b.gap {
                el = el.gap(lerp(g, f));
            }
            if let Some(t) = b.text_size {
                el = el.text_size(lerp(t, f));
            }
            if let Some(l) = b.line_height {
                el = el.line_height(lerp(l, f));
            }
            if let Some(r) = b.radius {
                el = el.rounded(lerp(r, f));
            }
            if b.slide_x.is_some() || b.slide_y.is_some() {
                el = el.relative();
            }
            if let Some(x) = b.slide_x {
                el = el.left(lerp(x, 1.0 - delta));
            }
            if let Some(y) = b.slide_y {
                el = el.top(lerp(y, 1.0 - delta));
            }
            el
        },
    )
    .into_any_element()
}

/// v3's `[data-exiting]`: `animate-out zoom-out-95 fade-out duration-150`.
///
/// The mirror of [`entering_zoom`] — the panel shrinks to `ZOOM_TO` and fades
/// as it leaves. It only has anything to animate because the component keeps
/// rendering for [`EXITING_MS`] after `isOpen` goes false; see
/// [`crate::util::overlay_phase`].
///
/// Returns `el` untouched under reduced motion, which is also what makes the
/// panel disappear immediately: with nothing to animate, the extra frames are
/// invisible.
pub fn exiting<E>(el: E, id: impl Into<ElementId>, b: ZoomBox, m: Motion, cx: &App) -> AnyElement
where
    E: IntoElement + Styled + 'static,
{
    if ActiveTheme::reduce_motion(cx) {
        return el.into_any_element();
    }

    el.with_animation(
        id.into(),
        gpui::Animation::new(Duration::from_millis(m.ms)).with_easing(move |t| m.curve.at(t)),
        move |el, delta| {
            // `delta` runs 0 -> 1 over the exit, so the scale runs 1 -> m.scale.
            let f = 1.0 - (1.0 - m.scale) * delta;
            let mut el = el.opacity(1.0 - delta);
            if let Some(w) = b.width {
                el = el.w(lerp(w, f));
            }
            if let Some(h) = b.height {
                el = el.h(lerp(h, f));
            }
            if let Some(p) = b.padding_x {
                el = el.px(lerp(p, f));
            }
            if let Some(p) = b.padding_y {
                el = el.py(lerp(p, f));
            }
            if let Some(p) = b.padding_top {
                el = el.pt(lerp(p, f));
            }
            if let Some(p) = b.padding_bottom {
                el = el.pb(lerp(p, f));
            }
            if let Some(g) = b.gap {
                el = el.gap(lerp(g, f));
            }
            if let Some(t) = b.text_size {
                el = el.text_size(lerp(t, f));
            }
            if let Some(l) = b.line_height {
                el = el.line_height(lerp(l, f));
            }
            if let Some(r) = b.radius {
                el = el.rounded(lerp(r, f));
            }
            el
        },
    )
    .into_any_element()
}

/// v3's `transition-colors`: the background eases between two colours instead
/// of switching on the frame the pointer arrives.
///
/// gpui has no property transitions — `hover` swaps the style outright — so the
/// element keeps its own hover flag and a keyed colour tween, and each target
/// change starts a fresh animation that interpolates in OKLab, resuming from
/// the value on screen when the pointer turns around mid-flight. `colors` is
/// the `(idle, hovered)` pair — the two ends; everything else about the element
/// is untouched.
///
/// `interaction` is the hover source when the element already records one: a
/// `content` closure's `isHovered` needs the same enter/leave that drives this
/// fade, and gpui allows exactly one `on_hover` per element, so when the caller
/// wired `util::track_interaction` the fade reads the slot it keeps rather than
/// binding a second listener. `None` leaves the fade owning its own listener.
/// Either way exactly one `on_hover` is bound.
///
/// **The animated colour lives on an absolutely-positioned child fill, not the
/// element itself.** gpui keys element state by the *full* element-id path, and
/// `with_animation` restarts by changing its id — if the animation wrapped the
/// element, the id change would shift the element's path and reset every
/// listener latch on it, so hover-out was silently lost the moment the fade
/// wrapper appeared (and the button stayed in its hover colour). Here the
/// element keeps a constant id — a stable path, working hover listeners — and
/// only the fill's animation id moves; the fill has no listeners or hitbox to
/// lose. `round_corners` shapes the fill like the element itself (a group
/// member's corners are partial).
///
/// **Call it before adding any children.** The fill is appended as the last
/// child, so it paints over anything already on the element — a label, icon
/// or indicator added beforehand is hidden under the hover colour. Children
/// added after the call paint on top of the fill, which is the order every
/// caller wants: fill first, content over it.
///
/// Returns the element with the endpoint painted outright under reduced
/// motion, so the state is still visible without motion.
#[allow(clippy::too_many_arguments)] // one parameter per endpoint the fade owns
pub fn hover_fade(
    el: gpui::Stateful<gpui::Div>,
    id: impl Into<ElementId>,
    colors: (gpui::Hsla, gpui::Hsla),
    interaction: Option<&crate::util::Interaction>,
    hover_border: Option<gpui::Hsla>,
    round_corners: impl Fn(gpui::Div) -> gpui::Div,
    window: &mut Window,
    cx: &mut App,
) -> gpui::Stateful<gpui::Div> {
    hover_fade_with_duration_and_easing(
        el,
        id,
        colors,
        interaction,
        hover_border,
        round_corners,
        None,
        HoverFadeEasing::EaseOut,
        window,
        cx,
    )
}

/// `hover_fade` with an owner-specific duration. Most HeroUI controls read
/// the theme's shared hover token; components whose stylesheet declares a
/// different transition (Accordion's 150ms trigger) use this seam so the
/// parity implementation does not silently inherit the button's 100ms.
#[allow(clippy::too_many_arguments)] // the seam adds one parameter to the stock fade
pub(crate) fn hover_fade_with_duration(
    el: gpui::Stateful<gpui::Div>,
    id: impl Into<ElementId>,
    colors: (gpui::Hsla, gpui::Hsla),
    interaction: Option<&crate::util::Interaction>,
    hover_border: Option<gpui::Hsla>,
    round_corners: impl Fn(gpui::Div) -> gpui::Div,
    duration_override_ms: Option<u64>,
    window: &mut Window,
    cx: &mut App,
) -> gpui::Stateful<gpui::Div> {
    hover_fade_with_duration_and_easing(
        el,
        id,
        colors,
        interaction,
        hover_border,
        round_corners,
        duration_override_ms,
        HoverFadeEasing::EaseOut,
        window,
        cx,
    )
}

/// `hover_fade_with_duration` with the component's named HeroUI easing curve.
/// The stock helper uses `--ease-out`; NumberField's group is one of the v3
/// surfaces that explicitly names `--ease-smooth` for its background transition.
#[allow(clippy::too_many_arguments)] // the seam adds one parameter to the stock fade
pub(crate) fn hover_fade_with_duration_and_easing(
    el: gpui::Stateful<gpui::Div>,
    id: impl Into<ElementId>,
    colors: (gpui::Hsla, gpui::Hsla),
    interaction: Option<&crate::util::Interaction>,
    hover_border: Option<gpui::Hsla>,
    round_corners: impl Fn(gpui::Div) -> gpui::Div,
    duration_override_ms: Option<u64>,
    easing: HoverFadeEasing,
    window: &mut Window,
    cx: &mut App,
) -> gpui::Stateful<gpui::Div> {
    hover_fade_with_duration_and_easing_suppressed(
        el,
        id,
        colors,
        None,
        interaction,
        hover_border,
        false,
        round_corners,
        duration_override_ms,
        easing,
        window,
        cx,
    )
}

/// `hover_fade` for a control whose held press is a third colour endpoint.
/// While the interaction slot's pressed bit is set the fill eases to `pressed`
/// — v3's `[data-pressed]` background — instead of the hover colour, so a
/// held press never paints a second layer under the fill. The caller always
/// has the slot: `util::track_interaction` keeps it current for the whole
/// press, pointer- or keyboard-driven.
#[allow(clippy::too_many_arguments)] // one parameter per endpoint the fade owns
pub(crate) fn hover_fade_with_press(
    el: gpui::Stateful<gpui::Div>,
    id: impl Into<ElementId>,
    colors: (gpui::Hsla, gpui::Hsla),
    pressed: gpui::Hsla,
    interaction: &crate::util::Interaction,
    round_corners: impl Fn(gpui::Div) -> gpui::Div,
    window: &mut Window,
    cx: &mut App,
) -> gpui::Stateful<gpui::Div> {
    hover_fade_with_duration_and_easing_suppressed(
        el,
        id,
        colors,
        Some(pressed),
        Some(interaction),
        None,
        false,
        round_corners,
        None,
        HoverFadeEasing::EaseOut,
        window,
        cx,
    )
}

/// `hover_fade_with_duration_and_easing` with a render-time suppression flag.
/// Composite triggers use this when a nested affordance owns the pointer: the
/// parent fill eases back to its resting endpoint while the nested control is
/// hovered, then resumes the normal target when the pointer leaves it.
///
/// `pressed` is the third endpoint a held press resolves to, ahead of the
/// hover colour: it needs the interaction slot, whose pressed bit is what a
/// keyboard press flips as well as a pointer one.
#[allow(clippy::too_many_arguments)] // the seam adds one parameter to the stock fade
pub(crate) fn hover_fade_with_duration_and_easing_suppressed(
    el: gpui::Stateful<gpui::Div>,
    id: impl Into<ElementId>,
    colors: (gpui::Hsla, gpui::Hsla),
    pressed: Option<gpui::Hsla>,
    interaction: Option<&crate::util::Interaction>,
    hover_border: Option<gpui::Hsla>,
    suppressed: bool,
    round_corners: impl Fn(gpui::Div) -> gpui::Div,
    duration_override_ms: Option<u64>,
    easing: HoverFadeEasing,
    window: &mut Window,
    cx: &mut App,
) -> gpui::Stateful<gpui::Div> {
    let (idle, hovered) = colors;
    let id = id.into();
    let duration = match duration_override_ms {
        None => hover_fade_duration(cx),
        Some(ms) => hover_fade_duration_with_override(cx, Some(ms)),
    };
    let state = window.use_keyed_state(id.clone(), cx, |_, _| HoverFade::default());
    let mut current = *state.read(cx);

    // One hover listener for the whole element. `util::track_interaction` owns
    // `on_hover` when the interaction slot exists; the fade then reads the
    // hover bit it keeps. `relative()` makes the element the containing block
    // the fill stretches across.
    let mut el = el;
    if duration.is_some() {
        el = el.relative();
    }
    el = match interaction {
        Some(slot) => {
            debug_assert!(
                hover_border.is_none(),
                "the interaction slot's owner keeps its own gpui hover style; \
                 `hover_border` is only read by the listener-owned branch"
            );
            // The slot is the hover source; `suppressed` is a render-time
            // flag, applied where the target is resolved below rather than
            // stored.
            let hovered_now = slot.read(cx).0;
            if hovered_now != current.hovered {
                current.hovered = hovered_now;
                state.update(cx, |s, _| *s = current);
            }
            el
        }
        None => {
            // This branch owns the element's `on_hover`, so it also owns its
            // gpui hover style -- the listener below records the pointer
            // without notifying, and this is what makes gpui notify on a real
            // crossing. See the hover-slot note in the module documentation.
            // `hover_border` carries the caller's immediate border endpoint;
            // the fill itself is the interpolated child, never a style swap.
            let el = el.hover(move |style| match hover_border {
                Some(color) => style.border_color(color),
                None => style,
            });
            let held = state.clone();
            el.on_hover(move |over: &bool, _, cx| {
                // The raw pointer bit, stored without `suppressed` (a
                // render-time flag) and without `cx.notify()` -- see the
                // hover-slot note in the module documentation above.
                let over = *over;
                held.update(cx, |s, _| {
                    if s.hovered != over {
                        s.hovered = over;
                    }
                });
            })
        }
    };

    // A held press outranks the hover wash: v3's `:active` background is the
    // stronger state, so the fill eases to the pressed endpoint while the
    // interaction slot's pressed bit is set.
    let is_hovered = current.hovered && !suppressed;
    let pressed_now = interaction.is_some_and(|slot| slot.read(cx).1);
    debug_assert!(
        pressed.is_none() || interaction.is_some(),
        "a pressed endpoint needs the interaction slot's pressed bit"
    );
    let target = match pressed {
        Some(pressed) if pressed_now => pressed,
        _ if is_hovered => hovered,
        _ => idle,
    };

    // The fill's colour lives in a `Tween` keyed beside the element: its
    // generation advances on every target change and the value cell holds
    // the frame actually on screen, so a mid-flight reversal resumes from
    // the painted colour — the way CSS reverses a transition — instead of
    // snapping to the far endpoint a from/to pair derived from the hovered
    // bit would pick.
    let mut tween = Tween::keyed(&id, "fade-color", target, window, cx);

    // First render: the resting colour on the element IS the state, and
    // there is nothing to ease from yet.
    if tween.generation() == 0 {
        return el.bg(target);
    }
    // Reduced motion and a zero-duration theme still need an immediate
    // endpoint. Use the same listener/state path as the animated case instead
    // of installing a second `.hover` refinement on a caller that already owns
    // one for its border. GPUI deliberately rejects duplicate hover styles.
    // The tween still lands on the target so a later animated generation
    // resumes from the colour actually painted.
    let Some(duration) = duration else {
        tween.snap_if_reduced(true);
        return el.bg(target);
    };
    // Once the fill is mounted it is the only painter of this colour. The
    // element under it stays transparent: a translucent endpoint (a soft or
    // ghost fill) painted on both layers composites twice, and the settled
    // colour -- resting as well as hovered -- would read darker than the
    // token for as long as the element lives.
    el = el.bg(gpui::transparent_black());
    // The fill sits under everything the caller adds afterwards: it exactly
    // covers the rounded element and carries only the colour transition, so the
    // element's own state — and its hover listeners — survive the id change.
    let fill = if tween.animates(false) {
        let (from, to) = (tween.from(), tween.target());
        let value = tween.value();
        round_corners(gpui::div().absolute().inset_0())
            .with_animation(
                element_id::indexed(&id, "fade", tween.generation()),
                gpui::Animation::new(duration).with_easing(move |t| easing.at(t)),
                move |fill, delta| {
                    let color = if delta >= 1.0 {
                        to
                    } else {
                        herogpui_core::mix_oklab(from, to, delta)
                    };
                    value.set(color);
                    fill.bg(color)
                },
            )
            .into_any_element()
    } else {
        tween.settle();
        round_corners(gpui::div().absolute().inset_0())
            .bg(target)
            .into_any_element()
    };
    el.child(fill)
}

#[derive(Clone, Copy)]
pub(crate) enum HoverFadeEasing {
    EaseOut,
    EaseSmooth,
}

impl HoverFadeEasing {
    fn at(self, t: f32) -> f32 {
        match self {
            Self::EaseOut => ease_out()(t),
            Self::EaseSmooth => ease_smooth()(t),
        }
    }
}

/// The duration [`hover_fade`] eases over, or `None` when it must resolve
/// immediately: reduced motion, or a theme that set `hover_fade_ms` to zero.
fn hover_fade_duration(cx: &App) -> Option<Duration> {
    hover_fade_duration_with_override(cx, None)
}

fn hover_fade_duration_with_override(
    cx: &App,
    duration_override_ms: Option<u64>,
) -> Option<Duration> {
    if ActiveTheme::reduce_motion(cx) {
        return None;
    }
    // A theme value of zero is the public global opt-out and must still win
    // over a component's stylesheet-specific default.
    let configured = cx.layout().hover_fade_ms;
    let ms = if configured == 0 {
        0
    } else {
        duration_override_ms.unwrap_or(configured)
    };
    (ms > 0).then(|| Duration::from_millis(ms))
}

/// The hover flag [`hover_fade`] keeps per element. The fill's restart
/// generation lives in the colour tween's own keyed state, so a press or a
/// reversal bumps that instead.
#[derive(Clone, Copy, Debug, Default)]
struct HoverFade {
    hovered: bool,
}

// ---------------------------------------------------------------------------
// Field chrome — the shell transition the field-family sheets share
// ---------------------------------------------------------------------------

/// How long a v3 field shell takes to change its fill and border colour, and
/// on which curve. `.input-otp__slot` (lines 28-32) and `.number-field__group`
/// (lines 39-43) declare the identical block — `background-color 150ms
/// var(--ease-smooth), border-color 150ms var(--ease-smooth), box-shadow
/// 150ms var(--ease-out)` with `motion-reduce:transition-none` after it — so
/// one constant quartet keeps both ports on the pinned timings.
pub(crate) const FIELD_CHROME_COLOR_MS: u64 = 150;
pub(crate) const FIELD_CHROME_COLOR_CURVE: Curve = Curve::Smooth;
pub(crate) const FIELD_CHROME_SHADOW_MS: u64 = 150;
pub(crate) const FIELD_CHROME_SHADOW_CURVE: Curve = Curve::Out;

/// A state ring as the two shadows [`focus_ring_shadows`] paint: the ring
/// itself and — for a theme with a `ring-offset` width — the
/// background-coloured ring carving the gap.
///
/// [`focus_ring_shadows`]: crate::util::focus_ring_shadows
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FieldRing {
    /// The ring's colour and spread.
    pub ring: (gpui::Hsla, f32),
    /// The offset-gap ring's colour and spread; spread zero when the theme
    /// sets no offset.
    pub gap: (gpui::Hsla, f32),
}

impl FieldRing {
    /// The no-ring endpoint: zero spreads, transparent colours — the shape CSS
    /// interpolates a missing box shadow from, and what `mix_oklab` treats as
    /// "the other colour's hue at reduced alpha".
    pub const NONE: Self = Self {
        ring: (gpui::hsla(0., 0., 0., 0.), 0.0),
        gap: (gpui::hsla(0., 0., 0., 0.), 0.0),
    };

    /// One ring with no offset gap — the invalid danger ring's shape.
    pub(crate) fn solid(color: gpui::Hsla, spread: f32) -> Self {
        Self {
            ring: (color, spread),
            gap: Self::NONE.ring,
        }
    }

    /// The same shape with every colour at zero alpha and every spread at
    /// zero: where a ring fades out to.
    fn faded(self) -> Self {
        Self {
            ring: (herogpui_core::with_alpha(self.ring.0, 0.0), 0.0),
            gap: (herogpui_core::with_alpha(self.gap.0, 0.0), 0.0),
        }
    }

    /// The shadows one frame of this ring paints — the settled form of
    /// `focus_ring_shadows`: one-pixel blur (a zero blur integrates over
    /// nothing in gpui's shadow shader), largest first.
    fn shadows(self) -> Vec<gpui::BoxShadow> {
        let mut shadows = vec![ring_shadow(self.ring)];
        if self.gap.1 > 0.0 {
            shadows.push(ring_shadow(self.gap));
        }
        shadows
    }

    /// CSS box-shadow interpolation: each shadow's colour and spread ease
    /// between the endpoints, `mix_oklab` being the pinned colour math.
    fn mix(self, to: Self, delta: f32) -> Self {
        let lerp = |a: f32, b: f32| a + (b - a) * delta;
        Self {
            ring: (
                herogpui_core::mix_oklab(self.ring.0, to.ring.0, delta),
                lerp(self.ring.1, to.ring.1),
            ),
            gap: (
                herogpui_core::mix_oklab(self.gap.0, to.gap.0, delta),
                lerp(self.gap.1, to.gap.1),
            ),
        }
    }
}

fn ring_shadow((color, spread): (gpui::Hsla, f32)) -> gpui::BoxShadow {
    gpui::BoxShadow {
        color,
        offset: gpui::point(px(0.), px(0.)),
        blur_radius: px(1.),
        spread_radius: px(spread),
        inset: false,
    }
}

/// The shared keyboard focus ring — `status-focused-field`, `ring-2
/// ring-focus` with no offset — as a chrome-ramp endpoint: the shape
/// [`focus_ring_shadows`] paints, including the offset-gap ring for a theme
/// that sets a `ring-offset` width.
///
/// [`focus_ring_shadows`]: crate::util::focus_ring_shadows
pub(crate) fn focus_ring_endpoint(cx: &App) -> FieldRing {
    let colors = cx.colors();
    let gap = cx.layout().ring_offset_width;
    let mut ring = FieldRing::solid(colors.focus, 2.0 + f32::from(gap));
    if gap > px(0.) {
        ring.gap = (colors.background, f32::from(gap));
    }
    ring
}

/// The invalid danger ring — the shared chrome helper's focused-invalid
/// treatment, `status-invalid-field`'s 2px ring — as a chrome-ramp endpoint.
pub(crate) fn danger_ring_endpoint(cx: &App) -> FieldRing {
    FieldRing::solid(cx.colors().danger.color, 2.0)
}

/// One resolved field-shell chrome state: the fill, the border colour, the
/// border-box width and the state ring. This is the shape the field-family
/// ramp interpolates — the endpoints a caller resolves from its variant and
/// flags, exactly what [`apply_field_chrome`] would paint for the same state.
///
/// [`apply_field_chrome`]: crate::util::apply_field_chrome
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FieldChrome {
    /// The state fill: `--field-background`, `--field-focus` or a hover mix.
    pub bg: gpui::Hsla,
    /// The border colour. The default theme's `--field-border` is
    /// transparent, which is what makes the port's invalid outline fade in
    /// and out rather than crossfade.
    pub border: gpui::Hsla,
    /// The painted border-box width. Widths are geometry and the pinned
    /// transition names only the colours, so this snaps while they ease.
    pub border_width: gpui::Pixels,
    /// The keyboard focus ring or invalid danger ring, or `None` — which
    /// still gives the `box-shadow` track an endpoint to ease through.
    pub ring: Option<FieldRing>,
}

/// Interpolates a field shell's chrome the way the pinned `transition` block
/// runs it, instead of snapping each state's endpoints in one frame.
///
/// The caller paints the shell's settled chrome through the shared helpers —
/// `apply_field_chrome` or `with_focus_ring`, the same paint Input and
/// TextArea take — and resolves `idle`, that chrome as endpoints, plus
/// `hovered`, the chrome while the pointer rests on the shell. Hover is the
/// one bit a render cannot ask for, so this arms the element's hover listener
/// against a keyed slot, exactly like [`hover_fade`]. A shell whose tracks are
/// all still in their first generation has never transitioned: its painted
/// chrome is the state and nothing mounts — CSS transitions do not run on
/// load. Past that first flip the interpolating fill, border and ring mount
/// as listener-free absolutely-positioned children that own the shell's
/// border and state ring for good, painting their settled endpoints whenever
/// nothing is in flight.
///
/// The technique is the press ramp's: three [`Tween`]s keyed on the shell's
/// own id keep the last target, the generation and the frame actually on
/// screen, so an interrupted flip resumes from the painted frame, and the
/// animation ids the layers change per generation never touch the element
/// that owns state — its id, hit-testing, focus and listeners. Reduced motion
/// snaps all three tracks to their endpoints, matching
/// `motion-reduce:transition-none`, which removes the timing but keeps the
/// state's property values.
///
/// `base_shadows` is the shell's constant shadow list (`--field-shadow`); the
/// state ring rides on top of it because `shadow()` replaces rather than
/// adds. `ring_escapes_clip` is for a shell whose own `overflow-hidden` (or
/// an ancestor's) would clip an outset ring painted by a child — including a
/// deferred one, which stays masked — so no ring layer mounts inside it at
/// all: the third return value hands the caller the hollow
/// `ring_overlay_in` geometry, its opacity driven by the ring tween's spread
/// the way the shadow alpha used to, for the caller to hang on its
/// non-clipping carrier (`util::field_ring_carrier`). A shadow could never
/// serve there: gpui paints a spread shadow as a filled shape, so the
/// background-coloured gap ring would cover the whole shell interior. The
/// in-flow ring (`ring_escapes_clip` false) stays on shadows.
///
/// Returns the refined shell, whether the ramp is painting the state ring
/// this frame, and — for a clip-escaping shell — the ring overlay element
/// for the caller's carrier. The ramp is not painting the ring on a shell
/// whose ring has never been part of a tracked state -- a pristine shell, or
/// one that never takes a ring -- and then whatever ring the caller painted
/// is the state. A caller whose ring is an overlay on a wrapper reads the
/// flag to decide whether to paint one.
#[allow(clippy::too_many_arguments)] // one parameter per chrome track, ring and shell option
pub(crate) fn field_chrome_ramp<E>(
    mut el: E,
    id: &ElementId,
    idle: FieldChrome,
    hovered: Option<FieldChrome>,
    base_shadows: Vec<gpui::BoxShadow>,
    radius: gpui::Pixels,
    ring_escapes_clip: bool,
    window: &mut Window,
    cx: &mut App,
) -> (E, bool, Option<AnyElement>)
where
    E: InteractiveElement + Styled + ParentElement,
{
    // Hover is a question about the last frame's pointer, so it lives in a
    // keyed slot the handler writes and this render reads.
    let slot = window.use_keyed_state(element_id::scoped(id, "chrome-hover"), cx, |_, _| false);
    let is_hovered = *slot.read(cx) && hovered.is_some();
    // This helper owns the element's `on_hover`, so it owns its gpui hover
    // style too. The refinement is deliberately empty: every chrome endpoint
    // here is interpolated by the tweens below, and a style swap would snap
    // the colour gpui is meant only to notify about. Setting it is what makes
    // a real pointer crossing repaint, which is why the listener records
    // without notifying. See the hover-slot note in the module documentation.
    el = el.hover(|style| style);
    el.interactivity().on_hover({
        move |over: &bool, _, cx| {
            slot.update(cx, |hovered, _| {
                if *hovered != *over {
                    *hovered = *over;
                    // Recording the pointer must not itself ask for a frame
                    // -- see the hover-slot note in the module documentation
                    // above.
                }
            });
        }
    });

    let (target, other_ring) = if is_hovered {
        (hovered.unwrap_or(idle), idle.ring)
    } else {
        (idle, hovered.and_then(|hovered| hovered.ring))
    };
    // Whether the state ring is one of this shell's tracked endpoints. When it
    // is, the ring layer owns the shell's shadow list for as long as the
    // layers are mounted; when it never is, whatever ring the caller painted
    // stays on the shell untouched.
    let ring_tracked = idle.ring.is_some() || target.ring.is_some();
    let reduce = ActiveTheme::reduce_motion(cx);
    let mut bg = Tween::keyed(id, "chrome-bg", target.bg, window, cx);
    let mut border = Tween::keyed(id, "chrome-border", target.border, window, cx);
    // A missing ring target still eases: it fades from the other endpoint's
    // shape — transparent, spread zero — the way CSS interpolates an absent
    // box shadow.
    let ring_target = target
        .ring
        .or_else(|| other_ring.map(|ring| ring.faded()))
        .unwrap_or(FieldRing::NONE);
    let mut ring = Tween::keyed(id, "chrome-ring", ring_target, window, cx);
    bg.snap_if_reduced(reduce);
    border.snap_if_reduced(reduce);
    ring.snap_if_reduced(reduce);

    // Pristine: every track still sits in its first generation, so nothing has
    // ever transitioned and the chrome the caller painted through the shared
    // helpers IS the state — CSS transitions do not run on load. Nothing
    // mounts, and the shell keeps every property it was given.
    let pristine = bg.generation() == 0 && border.generation() == 0 && ring.generation() == 0;
    if pristine {
        return (el, false, None);
    }

    // The containing block the layers stretch across. The shell's own paint
    // stays underneath, but the two properties the layers now own must stop
    // being cast by the shell: its border (the border layer repaints it, and
    // a semi-transparent frame over an instant one would read too dark) and —
    // when the ring is tracked — its shadow list, which drops to the field's
    // constant base while the ring layer carries the state ring.
    el = el.relative().border(px(0.));
    if ring_tracked {
        el = if base_shadows.is_empty() {
            el
        } else {
            el.shadow(base_shadows)
        };
    }

    let (bg_from, bg_to) = (bg.from(), bg.target());
    let bg_value = bg.value();
    let (border_from, border_to) = (border.from(), border.target());
    let border_value = border.value();
    // Border widths are geometry: the layer paints the wider endpoint's box
    // so a fading outline keeps its shape while its colour eases out.
    let border_width = idle.border_width.max(target.border_width);
    let (ring_from, ring_to) = (ring.from(), ring.target());
    let ring_value = ring.value();

    let fill = if bg.animates(reduce) {
        bg_value.set(bg_from);
        gpui::div()
            .absolute()
            .inset_0()
            .rounded(radius)
            .with_animation(
                element_id::indexed(id, "chrome-bg", bg.generation()),
                gpui::Animation::new(Duration::from_millis(FIELD_CHROME_COLOR_MS))
                    .with_easing(|t| FIELD_CHROME_COLOR_CURVE.at(t)),
                move |fill, delta| {
                    let next = if delta >= 1.0 {
                        bg_to
                    } else {
                        herogpui_core::mix_oklab(bg_from, bg_to, delta)
                    };
                    bg_value.set(next);
                    fill.bg(next)
                },
            )
            .into_any_element()
    } else {
        bg.settle();
        gpui::div()
            .absolute()
            .inset_0()
            .rounded(radius)
            .bg(bg_to)
            .into_any_element()
    };
    let border_layer = if border.animates(reduce) {
        border_value.set(border_from);
        gpui::div()
            .absolute()
            .inset_0()
            .rounded(radius)
            .with_animation(
                element_id::indexed(id, "chrome-border", border.generation()),
                gpui::Animation::new(Duration::from_millis(FIELD_CHROME_COLOR_MS))
                    .with_easing(|t| FIELD_CHROME_COLOR_CURVE.at(t)),
                move |layer, delta| {
                    let next = if delta >= 1.0 {
                        border_to
                    } else {
                        herogpui_core::mix_oklab(border_from, border_to, delta)
                    };
                    border_value.set(next);
                    layer.border(border_width).border_color(next)
                },
            )
            .into_any_element()
    } else {
        border.settle();
        gpui::div()
            .absolute()
            .inset_0()
            .rounded(radius)
            .border(border_width)
            .border_color(border_to)
            .into_any_element()
    };
    // The clip-escaping ring cannot be a child of the shell at all: the
    // shell's own `overflow-hidden` (or any ancestor's) cuts an outset ring
    // painted inside it, and a deferred child is still masked. The hollow
    // overlay is handed back for the caller to hang on its non-clipping
    // carrier — `util::field_ring_carrier` — where the settled ring already
    // lives. A shadow could never serve here either: gpui paints a spread
    // shadow as a filled shape, so the background-coloured gap ring would
    // cover the whole shell interior. The overlay's opacity is driven by the
    // ring tween's spread the way the shadow alpha used to, resuming from
    // the painted frame through the same `ring_value`.
    let mut escaped_ring: Option<AnyElement> = None;
    let ring_layer: Option<AnyElement> = if ring.generation() == 0 {
        // The ring has never been part of this shell's state: the caller's
        // painted ring (if any) is the state.
        None
    } else if ring_escapes_clip {
        let visible = if ring_to.ring.1 > 0.0 {
            ring_to
        } else {
            ring_from
        };
        let full_spread = visible.ring.1;
        let overlay_offset = visible.gap.1 > 0.0;
        let overlay_color = visible.ring.0;
        let opacity = move |ring: FieldRing| {
            if full_spread > 0.0 {
                (ring.ring.1 / full_spread).clamp(0.0, 1.0)
            } else {
                0.0
            }
        };
        escaped_ring = Some(if ring.animates(reduce) {
            ring_value.set(ring_from);
            crate::util::ring_overlay_in(radius, overlay_offset, overlay_color, cx)
                .with_animation(
                    element_id::indexed(id, "chrome-ring", ring.generation()),
                    gpui::Animation::new(Duration::from_millis(FIELD_CHROME_SHADOW_MS))
                        .with_easing(|t| FIELD_CHROME_SHADOW_CURVE.at(t)),
                    move |layer, delta| {
                        let next = if delta >= 1.0 {
                            ring_to
                        } else {
                            ring_from.mix(ring_to, delta)
                        };
                        ring_value.set(next);
                        layer.opacity(opacity(next))
                    },
                )
                .into_any_element()
        } else {
            ring.settle();
            crate::util::ring_overlay_in(radius, overlay_offset, overlay_color, cx)
                .opacity(opacity(ring_to))
                .into_any_element()
        });
        None
    } else if ring.animates(reduce) {
        ring_value.set(ring_from);
        Some(
            gpui::div()
                .absolute()
                .inset_0()
                .rounded(radius)
                .with_animation(
                    element_id::indexed(id, "chrome-ring", ring.generation()),
                    gpui::Animation::new(Duration::from_millis(FIELD_CHROME_SHADOW_MS))
                        .with_easing(|t| FIELD_CHROME_SHADOW_CURVE.at(t)),
                    move |layer, delta| {
                        let next = if delta >= 1.0 {
                            ring_to
                        } else {
                            ring_from.mix(ring_to, delta)
                        };
                        ring_value.set(next);
                        layer.shadow(next.shadows())
                    },
                )
                .into_any_element(),
        )
    } else {
        ring.settle();
        Some(
            gpui::div()
                .absolute()
                .inset_0()
                .rounded(radius)
                .shadow(ring_to.shadows())
                .into_any_element(),
        )
    };
    let ring_painted = ring_layer.is_some() || escaped_ring.is_some();
    el = el.child(fill).child(border_layer);
    if let Some(ring_layer) = ring_layer {
        el = el.child(ring_layer);
    }
    (el, ring_painted, escaped_ring)
}

/// v3's `@keyframes caret-blink`: opaque at 0/70/100%, transparent at 20/50%.
///
/// Reproduced as a repeating 1s animation over the same stops, so a text caret
/// blinks the way it does on the web instead of sitting solid.
pub fn caret_blink<E>(el: E, id: impl Into<ElementId>, cx: &App) -> AnyElement
where
    E: IntoElement + Styled + 'static,
{
    if ActiveTheme::reduce_motion(cx) {
        return el.into_any_element();
    }

    el.with_animation(
        id.into(),
        gpui::Animation::new(Duration::from_millis(1000)).repeat(),
        |el, delta| el.opacity(caret_opacity(delta)),
    )
    .into_any_element()
}

/// The `caret-blink` keyframe curve, linear between its stops.
fn caret_opacity(delta: f32) -> f32 {
    match delta {
        d if d < 0.20 => 1.0 - (d / 0.20),
        d if d < 0.50 => 0.0,
        d if d < 0.70 => (d - 0.50) / 0.20,
        _ => 1.0,
    }
}

/// Applies the v3 overlay entry animation: a 200ms ease-out fade.
///
/// The fade alone, for a panel with no metrics worth growing. Prefer
/// [`entering_zoom`], which adds v3's `zoom-in-90`. Returns `el` untouched when
/// the app has reduced motion enabled.
pub fn entering<E>(el: E, id: impl Into<ElementId>, m: Motion, cx: &App) -> AnyElement
where
    E: IntoElement + Styled + 'static,
{
    if ActiveTheme::reduce_motion(cx) {
        return el.into_any_element();
    }

    el.with_animation(
        id.into(),
        gpui::Animation::new(Duration::from_millis(m.ms)).with_easing(move |t| m.curve.at(t)),
        Styled::opacity,
    )
    .into_any_element()
}

/// Render a measured Accordion/Disclosure panel with the pinned height and
/// opacity transition.
///
/// `panel` owns the component's id and accessibility semantics; `body` is
/// wrapped in a non-shrinking measurement slot so its natural height remains
/// available even while the outer panel is clipped to an animated height. The
/// body stays mounted through the 200ms closing phase, which lets focus and
/// layout settle the same way the React Aria panel does before it becomes
/// hidden. A reopened panel cancels the stale exit timer and retargets both
/// tweens from their live frame.
pub(crate) fn collapsible_panel(
    id: &ElementId,
    is_open: bool,
    panel: gpui::Stateful<gpui::Div>,
    body: AnyElement,
    window: &mut Window,
    cx: &mut App,
) -> Option<AnyElement> {
    collapsible_panel_with_timings(
        id,
        is_open,
        panel,
        body,
        Motion::DISCLOSURE,
        Motion::DISCLOSURE,
        window,
        cx,
    )
}

/// Render a validation error row with v3's independent height and opacity
/// timelines. The message is retained in keyed state while the row exits so
/// the height tween has a real measured body instead of collapsing before the
/// animation begins. A quick revalidation updates that retained body and
/// retargets both tweens from their current painted values.
pub(crate) fn field_error_panel(
    id: &ElementId,
    message: Option<gpui::SharedString>,
    window: &mut Window,
    cx: &mut App,
) -> Option<AnyElement> {
    let retained =
        window.use_keyed_state(element_id::scoped(id, "field-error-message"), cx, |_, _| {
            None::<gpui::SharedString>
        });
    let is_open = message.is_some();
    if let Some(message) = message {
        if retained.read(cx).as_ref() != Some(&message) {
            retained.update(cx, |stored, cx| {
                *stored = Some(message);
                cx.notify();
            });
        }
    }
    let message = retained.read(cx).clone().unwrap_or_default();
    let panel_id = element_id::scoped(id, "field-error-panel");
    let panel = gpui::div()
        .id(panel_id.clone())
        .flex_shrink_0()
        .px(px(4.))
        .text_size(px(12.))
        .line_height(px(16.))
        .text_color(cx.colors().danger.color);
    let body = gpui::div()
        .flex_shrink_0()
        .child(message.to_string())
        .into_any_element();
    collapsible_panel_with_timings(
        &panel_id,
        is_open,
        panel,
        body,
        Motion::FIELD_ERROR_HEIGHT,
        Motion::FIELD_ERROR_OPACITY,
        window,
        cx,
    )
}

#[allow(clippy::too_many_arguments)] // the two motion tracks are the caller's paired contract
fn collapsible_panel_with_timings(
    id: &ElementId,
    is_open: bool,
    panel: gpui::Stateful<gpui::Div>,
    body: AnyElement,
    height_motion: Motion,
    opacity_motion: Motion,
    window: &mut Window,
    cx: &mut App,
) -> Option<AnyElement> {
    let reduce_motion = ActiveTheme::reduce_motion(cx);
    // Remember whether this panel has ever been visited. A default-open panel
    // has no opening transition in React Aria (its initial effect sets the
    // height to `auto`), while a panel opened after starting closed animates
    // from zero. This state is created even while the panel is closed so the
    // first later open is distinguishable from a default-open render.
    let lifecycle = window.use_keyed_state(element_id::scoped(id, "lifecycle"), cx, |_, _| {
        CollapsibleLifecycle::default()
    });
    let lifecycle_snapshot = *lifecycle.read(cx);
    let default_open = lifecycle_snapshot.default_open;
    if !lifecycle_snapshot.seen {
        lifecycle.update(cx, |state, _| {
            state.seen = true;
            state.default_open = is_open;
        });
    }

    let exit_ms = height_motion.ms.max(opacity_motion.ms);
    let phase = crate::util::panel_phase(
        window,
        cx,
        element_id::scoped(id, "phase"),
        is_open,
        !reduce_motion,
        exit_ms,
    );
    if phase == crate::util::OverlayPhase::Closed {
        return None;
    }

    let measured = window.use_keyed_state(element_id::scoped(id, "natural-height"), cx, |_, _| {
        Rc::new(Cell::new(None::<gpui::Pixels>))
    });
    let measured_for_listener = measured.clone();
    let body = gpui::div()
        .flex_shrink_0()
        .on_children_prepainted(move |bounds, _, cx| {
            let next = bounds.first().map_or(px(0.), |bound| bound.size.height);
            measured_for_listener.update(cx, |height, cx| {
                if height.get() != Some(next) {
                    height.set(Some(next));
                    cx.notify();
                }
            });
        })
        .child(body);

    let natural_height = measured.read(cx).get();
    // The first open frame must remain intrinsically sized so content is
    // immediately usable and its natural extent can be measured. Skipping
    // the height tween until that measurement exists also prevents an
    // initial default-open panel from animating from an invented zero height.
    if natural_height.is_none() {
        let panel = panel.flex_shrink_0().child(body);
        return Some(
            panel
                .opacity(if is_open { 1.0 } else { 0.0 })
                .into_any_element(),
        );
    }
    let target_height = if is_open {
        natural_height.unwrap_or(px(0.))
    } else {
        px(0.)
    };
    let initial_open = is_open && !default_open && natural_height.is_some();
    let mut height = Tween::keyed_with_initial(
        id,
        "panel-height",
        target_height,
        initial_open.then_some(px(0.)),
        window,
        cx,
    );
    let mut opacity = Tween::keyed_with_initial(
        id,
        "panel-opacity",
        if is_open { 1.0 } else { 0.0 },
        initial_open.then_some(0.0),
        window,
        cx,
    );
    height.snap_if_reduced(reduce_motion);
    opacity.snap_if_reduced(reduce_motion);

    let panel = panel.flex_shrink_0().overflow_hidden().child(body);
    let animate_height = height.animates(reduce_motion);
    let animate_opacity = opacity.animates(reduce_motion);
    if !animate_height && !animate_opacity {
        height.settle();
        opacity.settle();
        return Some(
            panel
                .h(height.target())
                .opacity(opacity.target())
                .into_any_element(),
        );
    }

    let height_from = height.from();
    let height_to = height.target();
    let height_value = height.value();
    let opacity_from = opacity.from();
    let opacity_to = opacity.target();
    let opacity_value = opacity.value();
    let animation_id = element_id::scoped(
        id,
        format!(
            "panel-motion-{}-{}",
            height.generation(),
            opacity.generation()
        ),
    );
    let duration_ms = height_motion.ms.max(opacity_motion.ms).max(1);
    let panel = panel.with_animation(
        animation_id,
        gpui::Animation::new(Duration::from_millis(duration_ms)),
        move |panel, delta| {
            let height_delta = height_motion
                .curve
                .at((delta * duration_ms as f32 / height_motion.ms.max(1) as f32).min(1.0));
            let opacity_delta = opacity_motion
                .curve
                .at((delta * duration_ms as f32 / opacity_motion.ms.max(1) as f32).min(1.0));
            let next_height = height_from + (height_to - height_from) * height_delta;
            let next_opacity = opacity_from + (opacity_to - opacity_from) * opacity_delta;
            height_value.set(next_height);
            opacity_value.set(next_opacity);
            panel.h(next_height).opacity(next_opacity)
        },
    );
    Some(panel.into_any_element())
}

#[derive(Clone, Copy, Debug, Default)]
struct CollapsibleLifecycle {
    seen: bool,
    default_open: bool,
}

/// Like [`entering`] but for content that also slides in — used by `Drawer`,
/// which enters from a window edge.
///
/// `travel` is the distance in pixels the panel covers; it is applied as a
/// margin that relaxes to zero.
pub fn entering_from<E>(
    el: E,
    id: impl Into<ElementId>,
    edge: Edge,
    travel: gpui::Pixels,
    m: Motion,
    cx: &App,
) -> AnyElement
where
    E: IntoElement + Styled + 'static,
{
    if ActiveTheme::reduce_motion(cx) {
        return el.into_any_element();
    }

    el.with_animation(
        id.into(),
        gpui::Animation::new(Duration::from_millis(m.ms)).with_easing(move |t| m.curve.at(t)),
        move |el, delta| {
            let remaining = travel * (1.0 - delta);
            let el = el.opacity(delta);
            match edge {
                Edge::Left => el.ml(-remaining),
                Edge::Right => el.mr(-remaining),
                Edge::Top => el.mt(-remaining),
                Edge::Bottom => el.mb(-remaining),
            }
        },
    )
    .into_any_element()
}

/// The mirror of [`entering_from`]: the panel slides back out to `edge`.
///
/// v3's drawer uses `slide-out-to-*` here, at the shorter exit duration.
pub fn exiting_to<E>(
    el: E,
    id: impl Into<ElementId>,
    edge: Edge,
    travel: gpui::Pixels,
    m: Motion,
    cx: &App,
) -> AnyElement
where
    E: IntoElement + Styled + 'static,
{
    if ActiveTheme::reduce_motion(cx) {
        return el.into_any_element();
    }

    el.with_animation(
        id.into(),
        gpui::Animation::new(Duration::from_millis(m.ms)).with_easing(move |t| m.curve.at(t)),
        move |el, delta| {
            let gone = travel * delta;
            let el = el.opacity(1.0 - delta);
            match edge {
                Edge::Left => el.ml(-gone),
                Edge::Right => el.mr(-gone),
                Edge::Top => el.mt(-gone),
                Edge::Bottom => el.mb(-gone),
            }
        },
    )
    .into_any_element()
}

/// Which window edge a sliding panel enters from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    /// The left edge.
    Left,
    /// The right edge.
    Right,
    /// The top edge.
    Top,
    /// The bottom edge.
    Bottom,
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::px;
    use herogpui_theme::{set_reduce_motion, set_theme, Theme, ThemeProvider};

    #[test]
    fn press_scales_resolved_corners_without_erasing_partial_overrides() {
        let mut skin = crate::util::round_sx_corners(
            gpui::div().rounded(px(2.)),
            &gpui::Corners {
                top_left: Some(px(12.)),
                bottom_right: Some(px(0.)),
                ..Default::default()
            },
        );
        let corners = pressed_corners(&skin.style().corner_radii, px(8.), 0.5);
        assert_eq!(
            corners,
            gpui::Corners {
                top_left: Some(px(6.)),
                top_right: Some(px(1.)),
                bottom_right: Some(px(0.)),
                bottom_left: Some(px(1.)),
            }
        );
        let mut pressed = crate::util::round_sx_corners(gpui::div().rounded(px(4.)), &corners);
        assert_eq!(pressed.style().corner_radii.top_left, Some(px(6.).into()));
        assert_eq!(
            pressed.style().corner_radii.bottom_right,
            Some(px(0.).into())
        );
        let unsupported = gpui::CornersRefinement {
            top_left: Some(gpui::rems(1.).into()),
            ..Default::default()
        };
        assert_eq!(
            pressed_corners(&unsupported, px(8.), 0.5),
            gpui::Corners::all(px(4.)).map(|radius| Some(*radius))
        );
    }

    #[test]
    fn stable_press_slot_keeps_button_radius_and_group_seams() {
        let full = gpui::CornersRefinement {
            top_left: Some(gpui::px(12.).into()),
            top_right: Some(gpui::px(12.).into()),
            bottom_right: Some(gpui::px(12.).into()),
            bottom_left: Some(gpui::px(12.).into()),
        };
        assert_eq!(
            resting_slot_corners(&full, px(8.)),
            gpui::Corners {
                top_left: Some(px(12.)),
                top_right: Some(px(12.)),
                bottom_right: Some(px(12.)),
                bottom_left: Some(px(12.)),
            }
        );

        let group_start = gpui::CornersRefinement {
            top_left: Some(gpui::px(12.).into()),
            bottom_left: Some(gpui::px(12.).into()),
            ..Default::default()
        };
        let corners = resting_slot_corners(&group_start, px(8.));
        assert_eq!(corners.top_left, Some(px(12.)));
        assert_eq!(corners.bottom_left, Some(px(12.)));
        assert_eq!(corners.top_right, None);
        assert_eq!(corners.bottom_right, None);
    }

    /// `hover_fade_ms` is public configuration: the stock theme keeps the
    /// button's `100ms`, a zero resolves like reduced motion, and any other
    /// value reaches the helper the fade reads.
    #[gpui::test]
    fn hover_fade_duration_follows_the_theme_and_zero_resolves_immediately(
        cx: &mut gpui::TestAppContext,
    ) {
        cx.update(ThemeProvider::init);
        cx.update(|cx| {
            assert_eq!(
                hover_fade_duration(cx),
                Some(Duration::from_millis(TRANSITION_MS)),
                "the stock theme uses the button's own 100ms"
            );
            assert_eq!(
                hover_fade_duration_with_override(cx, Some(ACCORDION_TRIGGER_HOVER_MS)),
                Some(Duration::from_millis(ACCORDION_TRIGGER_HOVER_MS)),
                "Accordion keeps its pinned 150ms declaration"
            );
        });
        cx.update(|cx| {
            set_theme(
                Theme::builder("instant", Theme::light())
                    .hover_fade_ms(0)
                    .build(),
                cx,
            );
        });
        cx.update(|cx| {
            assert_eq!(hover_fade_duration(cx), None, "zero resolves immediately");
            assert_eq!(
                hover_fade_duration_with_override(cx, Some(ACCORDION_TRIGGER_HOVER_MS)),
                None,
                "the theme-wide zero opt-out still suppresses owner overrides"
            );
        });
        cx.update(|cx| {
            set_theme(
                Theme::builder("slow", Theme::light())
                    .hover_fade_ms(250)
                    .build(),
                cx,
            );
        });
        cx.update(|cx| {
            assert_eq!(hover_fade_duration(cx), Some(Duration::from_millis(250)));
        });
        cx.update(|cx| set_reduce_motion(true, cx));
        cx.update(|cx| {
            assert_eq!(
                hover_fade_duration(cx),
                None,
                "reduced motion resolves immediately even with a non-zero token"
            );
        });
        cx.update(|cx| set_reduce_motion(false, cx));
    }

    #[test]
    fn press_inset_matches_the_scale() {
        // scale(0.97) on a 40px control moves each edge in by 1.5% of 40.
        // `1.0 - 0.97` is not exact in f32, so compare with a tolerance.
        assert!((f32::from(pressed_inset(px(40.))) - 0.6).abs() < 1e-4);
        assert!((f32::from(pressed_inset(px(32.))) - 0.48).abs() < 1e-4);
    }

    #[test]
    fn press_preserves_the_outer_footprint() {
        // The margin the box gains is exactly what its height gives up, so a
        // press never moves a neighbour.
        for h in [32.0f32, 40.0, 48.0] {
            let inset = f32::from(pressed_inset(px(h)));
            let shrunk = f32::from(shrink(px(h), pressed_inset(px(h)) + pressed_inset(px(h))));
            assert!(
                (shrunk + inset * 2.0 - h).abs() < 1e-4,
                "footprint changed at {h}"
            );
        }
    }

    #[test]
    fn cubic_bezier_pins_its_endpoints_and_rises() {
        let out = ease_out();
        assert!(out(0.0).abs() < 1e-3);
        assert!((out(1.0) - 1.0).abs() < 1e-3);
        // ease-out leads: it is ahead of linear through the middle.
        assert!(out(0.5) > 0.5, "ease-out should lead at the midpoint");
        // and it never goes backwards
        let mut prev = 0.0;
        for i in 0..=20 {
            let v = out(i as f32 / 20.0);
            assert!(v >= prev - 1e-4, "ease-out dipped at {i}");
            prev = v;
        }
        let smooth = ease_smooth();
        assert!(smooth(0.0).abs() < 1e-3);
        assert!((smooth(1.0) - 1.0).abs() < 1e-3);
    }

    #[test]
    #[allow(clippy::float_cmp)] // the scales are declared as exact identities
    fn drawer_backdrop_uses_its_own_fluid_timing() {
        assert_eq!(Motion::DRAWER_BACKDROP_IN.ms, 250);
        assert_eq!(Motion::DRAWER_BACKDROP_OUT.ms, 200);
        assert_eq!(Motion::DRAWER_BACKDROP_IN.curve, Curve::OutFluid);
        assert_eq!(Motion::DRAWER_BACKDROP_OUT.curve, Curve::OutFluid);
        assert_eq!(Motion::DRAWER_BACKDROP_IN.scale, 1.0);
        assert_eq!(Motion::DRAWER_BACKDROP_OUT.scale, 1.0);
    }

    #[test]
    #[allow(clippy::float_cmp)] // the scales are declared as exact identities
    fn field_error_uses_independent_height_and_opacity_timelines() {
        assert_eq!(Motion::FIELD_ERROR_HEIGHT.ms, 350);
        assert_eq!(Motion::FIELD_ERROR_HEIGHT.curve, Curve::Smooth);
        assert_eq!(Motion::FIELD_ERROR_OPACITY.ms, 150);
        assert_eq!(Motion::FIELD_ERROR_OPACITY.curve, Curve::Out);
        assert_eq!(Motion::FIELD_ERROR_HEIGHT.scale, 1.0);
        assert_eq!(Motion::FIELD_ERROR_OPACITY.scale, 1.0);
    }

    #[test]
    #[allow(clippy::float_cmp)] // the keyframe values are meant to be exact
    fn caret_blink_matches_its_keyframes() {
        // 0%, 70% and 100% are opaque; 20% and 50% are transparent.
        let at = |t: f32| caret_opacity(t);
        for (t, want) in [
            (0.0, 1.0),
            (0.20, 0.0),
            (0.35, 0.0),
            (0.50, 0.0),
            (0.70, 1.0),
            (1.0, 1.0),
        ] {
            assert!((at(t) - want).abs() < 1e-6, "{t} should be {want}");
        }
        // and it ramps rather than jumping
        assert!((caret_opacity(0.10) - 0.5).abs() < 1e-6);
        assert!((caret_opacity(0.60) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn progress_circle_spin_is_one_linear_turn() {
        assert!(progress_circle_spin_turn(0.0).abs() < 1e-6);
        assert!((progress_circle_spin_turn(0.25) - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
        assert!((progress_circle_spin_turn(1.0) - std::f32::consts::TAU).abs() < 1e-6);

        let mut previous = 0.0;
        for step in 0..=20 {
            let rotation = progress_circle_spin_turn(step as f32 / 20.0);
            assert!(rotation >= previous, "spin moved backwards at step {step}");
            previous = rotation;
        }
    }

    #[test]
    #[allow(clippy::float_cmp)] // clamped to exactly zero, not to near-zero
    fn shrink_never_goes_negative() {
        assert!(f32::from(shrink(px(1.), px(4.))).abs() < 1e-6);
    }
}
