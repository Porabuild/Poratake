//! Surface — port of `@heroui/surface`.
//!
//! A container that applies surface-level styling. Mirrors the React API:
//! `variant` of `transparent | default | secondary | tertiary`. Upstream
//! `.surface` is only `relative text-foreground` plus each variant's
//! fill/foreground classes; the docs examples add their `flex flex-col gap-3
//! rounded-3xl p-6` skeleton through `className`, so it is not a Surface
//! default. This port keeps a minimal column skeleton with zero default
//! padding and gap so the repository `padding`/`gap` builders work; they are
//! conveniences, not upstream props. Upstream also publishes its variant
//! through `SurfaceContext`; GPUI has no ancestor context propagation, so
//! nothing here reads the surrounding surface.

use gpui::{
    div, px, AnyElement, App, IntoElement, ParentElement, Pixels, RenderOnce, Styled, Window,
};
use herogpui_theme::ActiveTheme;

/// Prominence level of a surface (`variant` prop).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SurfaceVariant {
    /// No background — for overlays and cards with a custom background.
    Transparent,
    /// `bg-surface`.
    #[default]
    Default,
    /// `bg-surface-secondary`.
    Secondary,
    /// `bg-surface-tertiary`.
    Tertiary,
}

/// HeroUI Surface.
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct Surface {
    variant: SurfaceVariant,
    padding: Pixels,
    gap: Pixels,
    children: Vec<AnyElement>,
    /// The `sx` slot, refined over the root style at the end of render.
    sx: Option<Box<gpui::StyleRefinement>>,
}

impl Surface {
    /// Creates a surface.
    pub fn new() -> Self {
        Self {
            variant: SurfaceVariant::default(),
            padding: px(0.),
            gap: px(0.),
            children: Vec::new(),
            sx: None,
        }
    }

    /// Sets the surface variant.
    pub fn variant(mut self, variant: SurfaceVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The one slot for caller-owned low-level styling: GPUI's styling methods
    /// (`bg`, `text_color`, `w`, `h`, `p`, `rounded`, `border_color`, …)
    /// applied to the surface's root element after every value the variant and
    /// the active theme chose, so they win.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        crate::util::refine_sx(&mut self.sx, style);
        self
    }

    /// Repository convenience, not an upstream prop.
    pub fn padding(mut self, padding: impl Into<Pixels>) -> Self {
        self.padding = padding.into();
        self
    }

    /// Repository convenience, not an upstream prop.
    pub fn gap(mut self, gap: impl Into<Pixels>) -> Self {
        self.gap = gap.into();
        self
    }
}

impl Default for Surface {
    fn default() -> Self {
        Self::new()
    }
}

impl ParentElement for Surface {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for Surface {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = cx.colors();
        let mut el = div()
            .flex()
            .flex_col()
            .gap(self.gap)
            .p(self.padding)
            .text_color(colors.foreground);

        el = match self.variant {
            // `.surface--transparent` is only `bg-transparent`; GPUI's default
            // div background is already transparent, so nothing extra to paint.
            SurfaceVariant::Transparent => el,
            // Each fill brings its own foreground: `.surface--secondary` is
            // `bg-surface-secondary text-surface-secondary-foreground`, and the
            // text colour was going unset.
            SurfaceVariant::Default => el
                .bg(colors.surface.background)
                .text_color(colors.surface.foreground),
            SurfaceVariant::Secondary => el
                .bg(colors.surface_secondary)
                .text_color(colors.surface_secondary_foreground()),
            SurfaceVariant::Tertiary => el
                .bg(colors.surface_tertiary)
                .text_color(colors.surface_tertiary_foreground()),
        };

        let el = el.children(self.children);
        crate::util::apply_sx(el, &self.sx)
    }
}

crate::util::impl_component_styled!(Surface);
