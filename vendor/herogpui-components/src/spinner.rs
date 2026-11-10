//! Spinner — port of `@heroui/spinner` (v3).
//!
//! `size` is `sm | md | lg | xl` and `color` is
//! `current | accent | success | warning | danger`, where `current` inherits
//! the surrounding text color (used inside a pending `Button`). A caller can
//! also name the diameter in pixels (`size_px`) or replace the arc with its
//! own glyph (`glyph`); both keep the rotation, the reduced-motion
//! suppression and the `role="status"` root.

use std::time::Duration;

use gpui::{
    prelude::*, px, svg, Animation, AnimationExt, AnyElement, App, IntoElement, RenderOnce, Svg,
    Window,
};
use herogpui_core::Color;
use herogpui_theme::ActiveTheme;

use crate::a11y::A11y as _;
use crate::icons;

/// Spinner diameter (`size` prop).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SpinnerSize {
    /// Small.
    Sm,
    #[default]
    /// Medium.
    Md,
    /// Large.
    Lg,
    /// Extra large.
    Xl,
}

impl SpinnerSize {
    /// Every spinner size, in declaration order.
    pub const ALL: [SpinnerSize; 4] = [
        SpinnerSize::Sm,
        SpinnerSize::Md,
        SpinnerSize::Lg,
        SpinnerSize::Xl,
    ];

    /// The spinner's diameter in pixels.
    pub fn px(self) -> gpui::Pixels {
        match self {
            SpinnerSize::Sm => px(16.0),
            SpinnerSize::Md => px(24.0),
            SpinnerSize::Lg => px(32.0),
            SpinnerSize::Xl => px(40.0),
        }
    }

    /// A human-readable label for this size.
    pub fn label(self) -> &'static str {
        match self {
            SpinnerSize::Sm => "Sm",
            SpinnerSize::Md => "Md",
            SpinnerSize::Lg => "Lg",
            SpinnerSize::Xl => "Xl",
        }
    }
}

impl From<herogpui_core::Size> for SpinnerSize {
    fn from(size: herogpui_core::Size) -> Self {
        match size {
            herogpui_core::Size::Sm => SpinnerSize::Sm,
            herogpui_core::Size::Md => SpinnerSize::Md,
            herogpui_core::Size::Lg => SpinnerSize::Lg,
        }
    }
}

/// A rotating arc spinner, animated on the GPU.
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct Spinner {
    id: gpui::ElementId,
    size: SpinnerSize,
    /// Explicit diameter from [`Spinner::size_px`], winning over `size` when
    /// set.
    size_px: Option<gpui::Pixels>,
    color: Color,
    /// Set by `color="current"`: the resolved colour of the surrounding text.
    current_color: Option<gpui::Hsla>,
    /// One full turn, in milliseconds. HeroUI's default `animate-spin-fast`
    /// token is 750ms; the local setter also gives the gallery a deterministic
    /// equivalent of its speed utility examples.
    duration_ms: u64,
    /// Caller glyph replacing the arc, from [`Spinner::glyph`].
    glyph: Option<AnyElement>,
    /// The `sx` slot, refined over the root style at the end of render.
    sx: Option<Box<gpui::StyleRefinement>>,
}

impl Spinner {
    /// Creates a spinner with the given id.
    pub fn new(id: impl Into<gpui::ElementId>) -> Self {
        Self {
            id: id.into(),
            size: SpinnerSize::default(),
            size_px: None,
            color: Color::Accent,
            current_color: None,
            duration_ms: 750,
            glyph: None,
            sx: None,
        }
    }

    /// How long one full turn takes. v3 sets it with an animation utility.
    pub fn duration_ms(mut self, ms: u64) -> Self {
        self.duration_ms = ms.max(1);
        self
    }

    /// Sets the spinner size.
    pub fn size(mut self, size: impl Into<SpinnerSize>) -> Self {
        self.size = size.into();
        self
    }

    /// Overrides the diameter `size` derives. HeroUI names the arc's box with
    /// Tailwind size utilities, so a caller needing an in-between diameter
    /// (12, 14, 20px…) names it in pixels instead. The explicit diameter wins
    /// whether it is set before or after [`Spinner::size`].
    pub fn size_px(mut self, diameter: impl Into<gpui::Pixels>) -> Self {
        self.size_px = Some(diameter.into());
        self
    }

    /// Replaces the arc glyph with a caller element. The spinner keeps owning
    /// the box and the motion: the resolved diameter sizes the container the
    /// glyph centers in, the resolved colour (`color`/`current_color`) is
    /// applied to the glyph the same way the arc's svg is coloured, and the
    /// same repeated rotation — with the same reduced-motion suppression and
    /// the same `role="status"` root — wraps the caller's element.
    ///
    /// Pass the `svg()` element itself rather than a div wrapper: gpui can
    /// transform only svgs, so an svg nested deeper would sit still inside
    /// the animated container.
    pub fn glyph(mut self, glyph: impl IntoElement) -> Self {
        self.glyph = Some(glyph.into_any_element());
        self
    }

    /// Sets the color role.
    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self.current_color = None;
        self
    }

    /// `color="current"`. gpui svgs do not inherit `text_color`, so the caller
    /// passes the surrounding text colour explicitly.
    pub fn current_color(mut self, color: gpui::Hsla) -> Self {
        self.current_color = Some(color);
        self
    }

    /// The one slot for caller-owned low-level styling: GPUI's styling methods
    /// (`bg`, `text_color`, `w`, `h`, `p`, `rounded`, `border_color`, …)
    /// applied to the spinner's root element after every value the size, the
    /// colour and the active theme chose, so they win.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        crate::util::refine_sx(&mut self.sx, style);
        self
    }

    /// The rendered diameter: the explicit [`Spinner::size_px`] override when
    /// set, the documented [`SpinnerSize`] step otherwise.
    fn diameter(&self) -> gpui::Pixels {
        self.size_px.unwrap_or_else(|| self.size.px())
    }
}

/// The rotation's normalized turn fraction, clamped against the easing's
/// non-finite edge.
fn spin_fraction(delta: f32) -> f32 {
    if delta.is_finite() {
        delta.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// Rotates the caller's svg glyph to `t` of a full turn. gpui can transform
/// only svgs (`Svg::with_transformation` is an inherent method), so any other
/// glyph is returned unchanged and the animation keeps scheduling its frames
/// on the executor clock exactly as for the arc.
fn rotate_glyph(glyph: AnyElement, t: f32) -> AnyElement {
    let mut glyph = glyph;
    let Some(shell) = glyph.downcast_mut::<Svg>() else {
        return glyph;
    };
    // No API hands the svg back out by value and the transformation field is
    // private, so swap a fresh svg into the shell and keep the caller's.
    let caller = std::mem::replace(shell, svg());
    caller
        .with_transformation(gpui::Transformation::rotate(gpui::percentage(t)))
        .into_any_element()
}

impl RenderOnce for Spinner {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let color = self.current_color.unwrap_or(match self.color {
            Color::Default => cx.colors().muted,
            other => cx.role(other).color,
        });
        let diameter = self.diameter();

        let glyph = match self.glyph {
            None => {
                let spinner = svg()
                    .size(diameter)
                    .flex_shrink_0()
                    .path(icons::SPINNER)
                    .text_color(color);
                if ActiveTheme::reduce_motion(cx) {
                    crate::util::apply_sx(spinner, &self.sx).into_any_element()
                } else {
                    // `with_animation` hands back an `AnimationElement`, which has no
                    // style of its own to refine, so the slot lands on the svg the
                    // rotation wraps.
                    crate::util::apply_sx(spinner, &self.sx)
                        .with_animation(
                            self.id.clone(),
                            Animation::new(Duration::from_millis(self.duration_ms)).repeat(),
                            |svg, delta| {
                                svg.with_transformation(gpui::Transformation::rotate(
                                    gpui::percentage(spin_fraction(delta)),
                                ))
                            },
                        )
                        .into_any_element()
                }
            }
            Some(mut custom) => {
                // gpui svgs do not inherit the container's `text_color`, so
                // the resolved colour reaches a caller svg the same way it
                // reaches the arc: on the svg's own style. Every other glyph
                // (text, divs) inherits it from the container below.
                if let Some(svg) = custom.downcast_mut::<Svg>() {
                    svg.style().text.color = Some(color);
                }
                let spinning = if ActiveTheme::reduce_motion(cx) {
                    custom
                } else {
                    custom
                        .with_animation(
                            self.id.clone(),
                            Animation::new(Duration::from_millis(self.duration_ms)).repeat(),
                            |glyph, delta| rotate_glyph(glyph, spin_fraction(delta)),
                        )
                        .into_any_element()
                };
                let container = gpui::div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(diameter)
                    .flex_shrink_0()
                    .text_color(color)
                    .child(spinning);
                // The slot lands on the spinner-owned container, the same
                // "box around the glyph" the arc's svg occupies.
                crate::util::apply_sx(container, &self.sx).into_any_element()
            }
        };

        // The node sits on a box *around* the glyph rather than on the glyph
        // itself, which is upstream's own anatomy:
        // `@heroui/react/dist/components/spinner/spinner.js` imports no
        // `react-aria-components` primitive at all and hard-codes
        // `role: "status"` with `"aria-label": "Loading"` on its `dom.span`
        // root, giving the `SpinnerPrimitive` svg inside it `aria-hidden: true`.
        // (`Spinner` is a separate v3 export from `ProgressCircle`, which goes
        // through `useProgressBar`; the two are not the same component.)
        //
        // It also has to be a separate element here: the rotation is applied
        // by `Svg::with_transformation`, an inherent method on `Svg` that a
        // `Stateful<Svg>` no longer exposes, so an id on the glyph and the
        // animation cannot both survive. The `aria-hidden` half needs no
        // builder — the glyph has no id, and an element with no id and no role
        // produces no AccessKit node at all (`gpui-pre`'s
        // `window/a11y.rs`), which is the stronger form of the same thing.
        //
        // Stated after the layout chain, not spliced into it:
        // `.shots/design_audit.py` reads sizes and gaps out of builder chains
        // with character-windowed regexes.
        let root = gpui::div().flex().flex_shrink_0().child(glyph);
        root.id(self.id).a11y_named(
            crate::a11y::Role::Status,
            &crate::a11y::Name::labelled(crate::i18n::ui_string(
                crate::i18n::UiString::Loading,
                cx,
            )),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::Spinner;

    #[test]
    fn default_speed_matches_heroui_spin_fast_token() {
        assert_eq!(Spinner::new("spinner").duration_ms, 750);
    }
}

crate::util::impl_component_styled!(Spinner);
