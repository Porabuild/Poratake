//! Skeleton — port of `@heroui/skeleton` (v3).
//!
//! `animationType` selects between the sweeping shimmer, a pulse, and no
//! motion; it defaults to the theme's `--skeleton-animation` token.

use std::time::Duration;

use gpui::{
    div, prelude::*, px, Animation, AnimationExt, AnyElement, App, ElementId, IntoElement,
    ParentElement, Pixels, RenderOnce, Styled, Window,
};
use herogpui_theme::{ActiveTheme, SkeletonAnimation};

/// Loading placeholder (`Skeleton`).
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct Skeleton {
    id: ElementId,
    w: Option<Pixels>,
    h: Option<Pixels>,
    /// `animationType`. `None` defers to `--skeleton-animation`.
    animation_type: Option<SkeletonAnimation>,
    /// The corner radius, in place of the owning `hairline_radius` helper.
    radius: Option<Pixels>,
    children: Vec<AnyElement>,
    /// The `sx` slot, refined over the root style at the end of render.
    sx: Option<Box<gpui::StyleRefinement>>,
}

impl Skeleton {
    /// Creates a skeleton.
    pub fn new() -> Self {
        Self {
            id: "skeleton".into(),
            w: None,
            // A leaf keeps the pinned 24px fallback. A composed Skeleton
            // sizes itself from its children unless the caller supplies an
            // explicit height, which is how the upstream parent shimmer can
            // cover a profile/card/list shape without collapsing it to one
            // line.
            h: None,
            animation_type: None,
            radius: None,
            children: Vec::new(),
            sx: None,
        }
    }

    /// Distinct id per skeleton; required when several animate on one page.
    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = id.into();
        self
    }

    /// Sets the width.
    pub fn w(mut self, v: impl Into<Pixels>) -> Self {
        self.w = Some(v.into());
        self
    }

    /// Sets the height.
    pub fn h(mut self, v: impl Into<Pixels>) -> Self {
        self.h = Some(v.into());
        self
    }

    /// Sets the animation type.
    pub fn animation_type(mut self, animation: SkeletonAnimation) -> Self {
        self.animation_type = Some(animation);
        self
    }

    /// The corner radius, in place of the owning `hairline_radius` helper. Not
    /// a v3 prop; the removed v2 `radius` prop is prohibited and this is a
    /// per-component repository extension.
    pub fn radius(mut self, radius: impl Into<Pixels>) -> Self {
        self.radius = Some(radius.into());
        self
    }

    /// The one slot for caller-owned low-level styling: GPUI's styling methods
    /// (`bg`, `text_color`, `w`, `h`, `p`, `rounded`, `border_color`, …)
    /// applied to the skeleton's root element after every value the component
    /// and the active theme chose, so they win.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        crate::util::refine_sx(&mut self.sx, style);
        self
    }
}

impl Default for Skeleton {
    fn default() -> Self {
        Self::new()
    }
}

impl ParentElement for Skeleton {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for Skeleton {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = cx.colors();
        let has_children = !self.children.is_empty();
        // `.skeleton` is `bg-surface-tertiary/70`, not the solid token: the
        // placeholder is meant to read as a tint of whatever it sits on. This
        // painted it opaque, so every skeleton came out the full tertiary fill
        // -- (234,234,235) on the light page against v3's (237,237,238).
        let base_color = colors.surface_tertiary.alpha(0.7);
        // Reduced motion collapses every animation type to `None`.
        let animation = if ActiveTheme::reduce_motion(cx) {
            SkeletonAnimation::None
        } else {
            self.animation_type
                .unwrap_or(cx.layout().skeleton_animation)
        };

        // Hoisted so the shimmer band below can share the resolved value:
        // vanilla GPUI clips `overflow_hidden()` to the rectangle, so the
        // band carries this same radius (see `util::inner_fill_radius`).
        let base_radius = match self.radius {
            Some(radius) => radius,
            None => crate::util::hairline_radius(cx),
        };
        let base = div()
            .bg(base_color)
            .rounded(base_radius)
            .overflow_hidden()
            .when_some(self.w, |el, w| el.w(w))
            .when_some(
                self.h.or_else(|| (!has_children).then(|| px(24.))),
                |el, h| el.h(h),
            )
            // HeroUI keeps composed child skeletons visible and suppresses
            // their individual shimmer in the gallery with
            // `animationType="none"`; the parent owns the synchronized band.
            .when(has_children, |el| el.children(self.children));

        match animation {
            SkeletonAnimation::None => crate::util::apply_sx(base, &self.sx).into_any_element(),
            // `with_animation` hands back an `AnimationElement`, which has no
            // style of its own to refine, so the slot lands on the box the
            // animation wraps: the pulse keeps driving opacity, everything
            // else the caller set holds.
            // Tailwind's `animate-pulse`: opacity 1 → .5 at 50% → 1 over
            // 2s, `cubic-bezier(0.4, 0, 0.6, 1)` easing each half of the
            // cycle — not the old 1.6s sine between .3 and .8.
            SkeletonAnimation::Pulse => crate::util::apply_sx(base, &self.sx)
                .with_animation(
                    self.id,
                    Animation::new(Duration::from_millis(2000)).repeat(),
                    |el, delta| {
                        let (from, to, t) = if delta < 0.5 {
                            (1.0, 0.5, delta * 2.0)
                        } else {
                            (0.5, 1.0, (delta - 0.5) * 2.0)
                        };
                        let eased = crate::anim::cubic_bezier(0.4, 0.0, 0.6, 1.0, t);
                        el.opacity(from + (to - from) * eased)
                    },
                )
                .into_any_element(),
            // v3's `::after` is `inset-0` with
            // `bg-linear-to-r from-transparent via-surface-tertiary
            // to-transparent`, travelling `-translate-x-full` →
            // `translateX(200%)` over 2s linear, infinite — three box widths
            // of travel. GPUI gradients take two stops, so the symmetric band
            // is two half-width children: transparent→surface-tertiary, then
            // surface-tertiary→transparent. The band is full-width and its
            // middle is the opaque token, so it reads as a sheen over the
            // tinted base rather than a hard-edged band of the page colour.
            SkeletonAnimation::Shimmer => {
                let surface = colors.surface_tertiary;
                let half = |from: gpui::Hsla, to: gpui::Hsla| {
                    div()
                        .h_full()
                        .w(gpui::relative(0.5))
                        .bg(gpui::linear_gradient(
                            90.0,
                            gpui::linear_color_stop(from, 0.0),
                            gpui::linear_color_stop(to, 1.0),
                        ))
                };
                let clear = surface.alpha(0.0);
                // The band sweeps edge to edge inside a clip container inset
                // horizontally by the base radius: vanilla GPUI clips
                // `overflow_hidden()` to the rectangle, so without the inset
                // the band's square ends would paint through the base's
                // rounded corners at each end of its travel. The radius-wide
                // strips the sheen never reaches are visually negligible —
                // the CSS clip v3 gets from `overflow: hidden` +
                // `border-radius`.
                let band = div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left(gpui::relative(-1.0))
                    .w_full()
                    .flex()
                    .child(half(clear, surface))
                    .child(half(surface, clear))
                    .with_animation(
                        self.id,
                        Animation::new(Duration::from_millis(2000)).repeat(),
                        |el, delta| el.left(gpui::relative(-1.0 + 3.0 * delta)),
                    );
                let clip = div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left(base_radius)
                    .right(base_radius)
                    .overflow_hidden()
                    .child(band);
                crate::util::apply_sx(base.child(clip), &self.sx).into_any_element()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    fn implementation() -> String {
        let source = include_str!("skeleton.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("the implementation section is always present");
        source.split_whitespace().collect()
    }

    #[test]
    fn composed_skeletons_keep_children_visible_for_a_parent_shimmer() {
        let source = include_str!("skeleton.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("the implementation section is always present");
        assert!(source.contains("self.h.or_else(|| (!has_children).then(|| px(24.)))"));
        assert!(source.contains(".when(has_children, |el| el.children(self.children))"));
        assert!(
            !source.contains("div().opacity(0.).children(self.children)"),
            "composed Skeleton children must remain visible beneath the parent shimmer"
        );
    }

    /// v3's shimmer `::after` is a full-box band, `transparent →
    /// surface-tertiary → transparent`, travelling -100% → +200% over 2s
    /// linear — three box widths, symmetric, in the tertiary token (which
    /// stays light in dark theme), never the page `background`.
    #[test]
    fn the_shimmer_band_matches_v3s_sheen() {
        let compact = implementation();
        assert!(
            compact.contains("letsurface=colors.surface_tertiary;"),
            "the sheen must be surface-tertiary, not the page background"
        );
        assert!(
            !compact.contains("colors.background"),
            "a background-coloured band is a dark smear in dark theme"
        );
        assert!(
            compact.contains(".w_full()")
                && compact.contains(".child(half(clear,surface))")
                && compact.contains(".child(half(surface,clear))"),
            "the symmetric gradient is two half-width children"
        );
        assert!(
            compact.contains("Animation::new(Duration::from_millis(2000)).repeat()")
                && compact.contains("el.left(gpui::relative(-1.0+3.0*delta))"),
            "the band must travel -100% to +200% of the box over 2s"
        );
    }

    /// Vanilla GPUI clips `overflow_hidden()` to the rectangle, so a band
    /// sweeping the full box paints square pixels through the base's rounded
    /// corners. The band therefore rides inside a clip container inset
    /// horizontally by the base radius — the sheen never reaches the corner
    /// columns, and the band's own rounding is no longer needed.
    #[test]
    fn the_shimmer_band_clears_the_rounded_corners() {
        let compact = implementation();
        assert!(
            compact.contains(".left(base_radius).right(base_radius).overflow_hidden()"),
            "the band must sweep inside a clip inset by the base radius"
        );
        assert!(
            !compact.contains(".flex().rounded(base_radius)"),
            "the band's own rounding is redundant inside the inset clip"
        );
    }

    /// Tailwind `animate-pulse`: 2s `cubic-bezier(0.4,0,0.6,1)`, opacity
    /// 1 → .5 at the midpoint → 1 — not a sine between other extremes.
    #[test]
    fn the_pulse_matches_tailwinds_animate_pulse() {
        let compact = implementation();
        assert!(
            compact.contains("crate::anim::cubic_bezier(0.4,0.0,0.6,1.0,t)"),
            "the pulse easing must be Tailwind's cubic-bezier(0.4,0,0.6,1)"
        );
        assert!(
            compact.contains("(1.0,0.5,delta*2.0)")
                && compact.contains("(0.5,1.0,(delta-0.5)*2.0)"),
            "opacity must go 1 → .5 → 1 across the 2s cycle"
        );
        assert!(
            compact.contains("Animation::new(Duration::from_millis(2000)).repeat()"),
            "animate-pulse runs on a 2s cycle"
        );
        assert!(
            !compact.contains("0.55+0.25*t"),
            "the old 1.6s sine pulse must be gone"
        );
    }
}

crate::util::impl_component_styled!(Skeleton);
