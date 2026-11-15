//! Separator — port of `@heroui/separator` (v3, formerly `Divider`).
//!
//! `variant` pairs with the surrounding [`Surface`](crate::surface::Surface)
//! prominence so the line stays visible as the container gets more prominent.

use gpui::{
    div, AnyElement, App, ElementId, InteractiveElement, IntoElement, ParentElement, Pixels,
    RenderOnce, Styled, Window,
};
use herogpui_core::Orientation;
use herogpui_theme::ActiveTheme;

/// Visual variant of a separator.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SeparatorVariant {
    /// `--separator`
    #[default]
    Default,
    /// `color-mix(in oklab, surface 85%, surface-foreground 15%)`
    Secondary,
    /// `color-mix(in oklab, surface 81%, surface-foreground 19%)`
    Tertiary,
}

impl SeparatorVariant {
    /// Every separator variant, in declaration order.
    pub const ALL: [SeparatorVariant; 3] = [
        SeparatorVariant::Default,
        SeparatorVariant::Secondary,
        SeparatorVariant::Tertiary,
    ];

    /// A human-readable label for this variant.
    pub fn label(self) -> &'static str {
        match self {
            SeparatorVariant::Default => "Default",
            SeparatorVariant::Secondary => "Secondary",
            SeparatorVariant::Tertiary => "Tertiary",
        }
    }
}

/// HeroUI Separator.
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct Separator {
    orientation: Orientation,
    variant: SeparatorVariant,
    inset_y: Pixels,
    inset_x: Pixels,
    /// Set by [`Toolbar::separator`](crate::toolbar::Toolbar::separator) for
    /// `.toolbar`'s own descendant rules, which halve whichever separator
    /// crosses the bar's flow and centre it. See [`Separator::in_toolbar`].
    in_toolbar: bool,
    id: Option<ElementId>,
    /// The `sx` slot, refined over the root style at the end of render.
    sx: Option<Box<gpui::StyleRefinement>>,
}

impl Separator {
    /// Creates a horizontal separator.
    pub fn new() -> Self {
        Self {
            orientation: Orientation::Horizontal,
            variant: SeparatorVariant::default(),
            inset_y: gpui::px(0.),
            inset_x: gpui::px(0.),
            in_toolbar: false,
            id: None,
            sx: None,
        }
    }

    /// Names this instance. AccessKit 0.24 has no `Role::Separator`, so a
    /// named separator still produces no accessibility node — the id is here
    /// so a later AccessKit bump can claim the role without a public-API
    /// change.
    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Applies `.toolbar`'s descendant rules for a separator inside a bar:
    /// `.separator--vertical` becomes `h-1/2 self-center` and
    /// `.separator--horizontal` becomes `w-1/2 justify-self-center`, so the
    /// rule crossing the bar's flow is half its cross size and centred rather
    /// than running the bar's whole edge.
    ///
    /// v3 spells this as a descendant selector, so *any* separator inside a
    /// toolbar picks it up. A [`Toolbar`](crate::toolbar::Toolbar) holds
    /// type-erased children and cannot reach into one to restyle it, so the
    /// bar builds its own separators instead — reach for
    /// [`Toolbar::separator`](crate::toolbar::Toolbar::separator) rather than
    /// passing a hand-built `Separator` as a child.
    pub(crate) fn in_toolbar(mut self) -> Self {
        self.in_toolbar = true;
        self
    }

    /// Sets the separator orientation.
    pub fn orientation(mut self, orientation: Orientation) -> Self {
        self.orientation = orientation;
        self
    }

    /// Sets the separator variant.
    pub fn variant(mut self, variant: SeparatorVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The one slot for caller-owned low-level styling: GPUI's styling methods
    /// (`bg`, `text_color`, `w`, `h`, `p`, `rounded`, `border_color`, …)
    /// applied to the separator's root element after every value the variant
    /// and the active theme chose, so they win.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        crate::util::refine_sx(&mut self.sx, style);
        self
    }

    /// Vertical inset. gpui has no `className`, so the margin v3 sets with
    /// `my-*` is a builder here.
    pub fn my(mut self, v: impl Into<Pixels>) -> Self {
        self.inset_y = v.into();
        self
    }

    /// Horizontal inset — the `mx-*` counterpart of [`Separator::my`].
    pub fn mx(mut self, v: impl Into<Pixels>) -> Self {
        self.inset_x = v.into();
        self
    }
}

impl Default for Separator {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for Separator {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = cx.colors();
        let weight = cx.layout().border_width;
        let color = match self.variant {
            SeparatorVariant::Default => colors.separator,
            SeparatorVariant::Secondary => colors.separator_secondary(),
            SeparatorVariant::Tertiary => colors.separator_tertiary(),
        };

        // `separator.tsx` renders a childless RAC `Separator`: v3 has no
        // content-bearing mode (v3.2.6 deleted the never-applied
        // `.separator__container` / `__line` / `__content` rules), so the
        // "With Content" example places separators *between* blocks.
        let radius = crate::util::hairline_radius(cx);

        if self.in_toolbar {
            // `.toolbar` halves the rule that crosses its flow and centres it:
            // an 18px tick in a 36px bar, not a line down its whole edge.
            //
            // The half-length box is positioned inside a transparent full-size
            // slot rather than sized directly, because a percentage
            // main/cross size against a bar whose own size comes from its
            // controls has nothing definite to resolve against. This is the
            // same construction `ButtonGroup` draws its member separators
            // with, and the only one in this codebase proven to land on the
            // measured 25%/50% geometry.
            let slot = div()
                .relative()
                .my(self.inset_y)
                .mx(self.inset_x)
                .flex_shrink_0()
                .debug_selector(|| "toolbar-separator".to_owned());
            let mark = div()
                .absolute()
                .rounded(radius)
                .bg(color)
                .debug_selector(|| "toolbar-separator-mark".to_owned());
            let slot = match self.orientation {
                Orientation::Horizontal => slot.w_full().h(weight).child(
                    mark.left(gpui::relative(0.25))
                        .w(gpui::relative(0.5))
                        .h(weight),
                ),
                Orientation::Vertical => slot.self_stretch().min_h(gpui::px(8.)).w(weight).child(
                    mark.top(gpui::relative(0.25))
                        .h(gpui::relative(0.5))
                        .w(weight),
                ),
            };
            return finish_separator(crate::util::apply_sx(slot, &self.sx), self.id);
        }

        let el = div()
            .my(self.inset_y)
            .mx(self.inset_x)
            .flex_shrink_0()
            .rounded(radius)
            .bg(color);

        let el = match self.orientation {
            Orientation::Horizontal => el.w_full().h(weight),
            // `.separator--vertical` is `min-h-2`: a vertical rule between
            // two inline items still draws when its row is shorter.
            Orientation::Vertical => el.h_full().min_h(gpui::px(8.)).w(weight),
        };
        finish_separator(crate::util::apply_sx(el, &self.sx), self.id)
    }
}

fn finish_separator(el: gpui::Div, id: Option<ElementId>) -> AnyElement {
    match id {
        Some(id) => el.id(id).into_any_element(),
        None => el.into_any_element(),
    }
}

crate::util::impl_component_styled!(Separator);
