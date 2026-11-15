//! Badge — port of `@heroui/badge`.

use gpui::{
    point, prelude::*, px, AbsoluteLength, AnyElement, App, Bounds, DefiniteLength, Display, Edges,
    Element, GlobalElementId, Hsla, InspectorElementId, IntoElement, LayoutId, Length,
    ParentElement, Pixels, Position, RenderOnce, Style, Styled, Window,
};
use herogpui_core::{Color, Size};
use herogpui_theme::{ActiveTheme, ThemeColors};

/// Where the badge is anchored on its child (`placement`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BadgePlacement {
    /// Top-left corner of the child.
    TopLeft,
    /// Top-right corner of the child.
    #[default]
    TopRight,
    /// Bottom-left corner of the child.
    BottomLeft,
    /// Bottom-right corner of the child.
    BottomRight,
}

/// Visual style of a badge (`variant`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BadgeVariant {
    /// Filled with the badge color.
    #[default]
    Primary,
    /// Filled with `default`, colored text.
    Secondary,
    /// Filled with `--{role}-soft` (the default role mixes at 50%), labelled
    /// in the soft foreground.
    Soft,
}

impl BadgeVariant {
    /// Every badge variant, in display order.
    pub const ALL: [BadgeVariant; 3] = [
        BadgeVariant::Primary,
        BadgeVariant::Secondary,
        BadgeVariant::Soft,
    ];

    /// The human-readable name of this variant.
    pub fn label(self) -> &'static str {
        match self {
            BadgeVariant::Primary => "Primary",
            BadgeVariant::Secondary => "Secondary",
            BadgeVariant::Soft => "Soft",
        }
    }
}

/// v3's `Badge.Anchor` — the positioning wrapper (`.badge-anchor`) that owns
/// the anchored element and the [`Badge`] pointing at it:
///
/// ```
/// # use gpui::{prelude::*, px, Window};
/// # use herogpui_components::{Badge, BadgeAnchor, BadgeLabel};
/// # struct Demo;
/// # impl Render for Demo {
/// #     fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
/// #         let avatar = gpui::div().w(px(40.)).h(px(40.));
/// BadgeAnchor::new()
///     .child(avatar)
///     .child(Badge::new().child(BadgeLabel::new().child("5")))
/// #     }
/// # }
/// # let mut tcx = gpui::TestAppContext::single();
/// # tcx.update(herogpui_theme::ThemeProvider::init);
/// # let _ = tcx.add_window_view(|_, _| Demo);
/// ```
///
/// `.badge-anchor` is `relative inline-flex shrink-0`. GPUI 0.2.2 has no
/// inline-flex, so the wrapper hugs its content only inside a flex parent,
/// and the ported `flex_shrink_0` keeps it from compressing in an overflowing
/// row. The badge positions itself against this wrapper.
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct BadgeAnchor {
    children: Vec<AnyElement>,
    /// The `sx` slot, refined over the root style at the end of render.
    sx: Option<Box<gpui::StyleRefinement>>,
}

impl BadgeAnchor {
    /// Creates an empty badge anchor.
    pub fn new() -> Self {
        Self {
            children: Vec::new(),
            sx: None,
        }
    }

    /// The one slot for caller-owned low-level styling: GPUI's styling methods
    /// (`bg`, `text_color`, `w`, `h`, `p`, `rounded`, `border_color`, …)
    /// applied to the anchor's root element after every value the anchor and
    /// the active theme chose, so they win.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        crate::util::refine_sx(&mut self.sx, style);
        self
    }
}

impl Default for BadgeAnchor {
    fn default() -> Self {
        Self::new()
    }
}

impl ParentElement for BadgeAnchor {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for BadgeAnchor {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let el = gpui::div()
            .relative()
            .flex()
            .flex_shrink_0()
            .debug_selector(|| "badge-anchor".to_owned())
            .children(self.children);
        crate::util::apply_sx(el, &self.sx)
    }
}

/// v3's `Badge.Label` — the badge's text slot. `.badge__label` is `px-0.5`,
/// the only horizontal padding in v3's badge sheet; the badge itself has
/// none.
///
/// v3's root auto-wraps plain string and number children in this part. GPUI
/// elements carry no runtime type a parent can intercept, so the port cannot
/// reproduce that auto-wrap: plain [`Badge`] children draw inside the badge
/// without the label padding, and this explicit part is the seam that carries
/// the pinned padding.
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct BadgeLabel {
    children: Vec<AnyElement>,
    /// The `sx` slot, refined over the root style at the end of render.
    sx: Option<Box<gpui::StyleRefinement>>,
}

impl BadgeLabel {
    /// Creates an empty badge label.
    pub fn new() -> Self {
        Self {
            children: Vec::new(),
            sx: None,
        }
    }

    /// The one slot for caller-owned low-level styling: GPUI's styling methods
    /// (`bg`, `text_color`, `w`, `h`, `p`, `rounded`, `border_color`, …)
    /// applied to the label's root element after every value the badge and the
    /// active theme chose, so they win.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        crate::util::refine_sx(&mut self.sx, style);
        self
    }
}

impl Default for BadgeLabel {
    fn default() -> Self {
        Self::new()
    }
}

impl ParentElement for BadgeLabel {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for BadgeLabel {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let el = gpui::div()
            .debug_selector(|| "badge-label".to_owned())
            .px(px(2.))
            .children(self.children);
        crate::util::apply_sx(el, &self.sx)
    }
}

/// HeroUI Badge — v3's `Badge.Root`, the indicator positioned against a
/// [`BadgeAnchor`]. Its [`ParentElement`] children are the badge's own
/// content: text goes through [`BadgeLabel`], and a badge with no children
/// renders as a dot — the dot is the omitted label, not a separate mode.
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct Badge {
    color: Color,
    variant: BadgeVariant,
    size: Size,
    placement: BadgePlacement,
    children: Vec<AnyElement>,
    /// The label's font size; unset keeps the size-step's font. The
    /// fractional leading scales with it.
    text_size: Option<Pixels>,
    /// The badge's corner radius, in place of the size step's radius.
    radius: Option<Pixels>,
    /// The `sx` slot, refined over the root style at the end of render.
    sx: Option<Box<gpui::StyleRefinement>>,
}

impl Badge {
    /// Creates a badge with the default color, `Primary` variant, medium size and top-right placement.
    pub fn new() -> Self {
        // v3's table gives `color` a default of `"default"`, which is the
        // gray `.badge--default`; the seed used to be `Danger`.
        Self {
            color: Color::Default,
            variant: BadgeVariant::Primary,
            size: Size::Md,
            placement: BadgePlacement::TopRight,
            children: Vec::new(),
            text_size: None,
            radius: None,
            sx: None,
        }
    }

    /// Sets the badge color (`color`).
    pub fn color(mut self, c: Color) -> Self {
        self.color = c;
        self
    }

    /// Sets the visual variant (`variant`).
    pub fn variant(mut self, v: BadgeVariant) -> Self {
        self.variant = v;
        self
    }

    /// Sets the size (`size`).
    pub fn size(mut self, s: Size) -> Self {
        self.size = s;
        self
    }

    /// Sets where the badge is anchored on its child (`placement`).
    pub fn placement(mut self, p: BadgePlacement) -> Self {
        self.placement = p;
        self
    }

    /// The badge's corner radius, in place of the size step's radius. The step
    /// (`Sm` → `small_radius`, `Md` → `control_radius`, `Lg` → `soft_radius`)
    /// stays the fallback, so a badge with no override keeps its step. Not a
    /// v3 prop; the removed v2 `radius` prop is prohibited and this is a
    /// per-component repository extension.
    pub fn radius(mut self, radius: impl Into<Pixels>) -> Self {
        self.radius = Some(radius.into());
        self
    }

    /// The label's font size; unset keeps the size-step's font. The
    /// fractional leading scales with the font.
    pub fn text_size(mut self, size: impl Into<Pixels>) -> Self {
        self.text_size = Some(size.into());
        self
    }

    /// The one slot for caller-owned low-level styling: GPUI's styling methods
    /// (`bg`, `text_color`, `w`, `h`, `p`, `rounded`, `border_color`, …)
    /// applied to the badge's root element after every value the variant, the
    /// color and the active theme chose, so they win.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        crate::util::refine_sx(&mut self.sx, style);
        self
    }
}

impl Default for Badge {
    fn default() -> Self {
        Self::new()
    }
}

impl ParentElement for Badge {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

/// Resolves one badge's paint pair against v3.2.4's `badge.css` cascade: the
/// base rule, the `.badge--{color}` foreground classes, and the compound
/// variant×color rules. Every badge carries the page-background ring, so
/// unlike a chip this always paints a fill.
fn paint(colors: &ThemeColors, variant: BadgeVariant, color: Color) -> (Hsla, Hsla) {
    let role = colors.role(color);
    let muted_foreground = || {
        if color == Color::Default {
            colors.default.foreground
        } else {
            role.soft_foreground(colors.foreground)
        }
    };
    match variant {
        BadgeVariant::Primary => (role.color, role.foreground),
        BadgeVariant::Secondary => (colors.default.color, muted_foreground()),
        BadgeVariant::Soft => (role.soft(), muted_foreground()),
    }
}

/// The placement node around one anchored [`Badge`]. v3's placement class is
/// two declarations — the `top/right/bottom/left: 0` pin at the anchor's
/// corner and `transform: translate(±25%, ±25%)`, a shift by a quarter of the
/// badge's *own* box outward on each participating axis. gpui-pre 0.3.3 has
/// no div-level transform (SVG only), and a percentage inset would resolve
/// against the containing block rather than the badge, so the two declarations
/// split across two nodes: this absolute wrapper carries the corner pin in
/// layout, and prepaint reads the badge's laid-out box and prepaints it
/// shifted by its quarter-box translate — the measure-then-offset pass
/// gpui's `Anchored` and the port's `PopoverArrow` and `PopoverPositioner`
/// run.
struct PlacedBadge {
    placement: BadgePlacement,
    badge: AnyElement,
}

impl Element for PlacedBadge {
    type RequestLayoutState = LayoutId;
    type PrepaintState = ();

    fn id(&self) -> Option<gpui::ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let badge = self.badge.request_layout(window, cx);
        // The placement class's corner pin (`top-0 right-0` and friends),
        // carried here so the badge node is positioned at the anchor corner
        // before the translate is applied in prepaint.
        let pinned = || Length::Definite(DefiniteLength::Absolute(AbsoluteLength::Pixels(px(0.))));
        let mut inset = Edges::<Length>::auto();
        match self.placement {
            BadgePlacement::TopRight => {
                inset.top = pinned();
                inset.right = pinned();
            }
            BadgePlacement::TopLeft => {
                inset.top = pinned();
                inset.left = pinned();
            }
            BadgePlacement::BottomRight => {
                inset.bottom = pinned();
                inset.right = pinned();
            }
            BadgePlacement::BottomLeft => {
                inset.bottom = pinned();
                inset.left = pinned();
            }
        }
        let layout = window.request_layout(
            Style {
                position: Position::Absolute,
                display: Display::Flex,
                inset,
                ..Style::default()
            },
            [badge],
            cx,
        );
        (layout, badge)
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        badge: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        // `translate(±25%, ±25%)` resolves against the badge's own box, and
        // the box is final by prepaint time: a grown label overhangs a
        // quarter of its grown box, exactly like the CSS transform. The raw
        // fraction is applied as-is — a 42px box overhangs 10.5px, the same
        // arithmetic a browser paints.
        let size = window.layout_bounds(*badge).size;
        let (dx, dy) = match self.placement {
            BadgePlacement::TopRight => (size.width / 4., -(size.height / 4.)),
            BadgePlacement::TopLeft => (-(size.width / 4.), -(size.height / 4.)),
            BadgePlacement::BottomRight => (size.width / 4., size.height / 4.),
            BadgePlacement::BottomLeft => (-(size.width / 4.), size.height / 4.),
        };
        window.with_element_offset(point(dx, dy), |window| {
            self.badge.prepaint(window, cx);
        });
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.badge.paint(window, cx);
    }
}

impl IntoElement for PlacedBadge {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl RenderOnce for Badge {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = cx.colors();

        // `.badge` is `min-h-7 min-w-7 rounded-3xl text-xs leading-[1.34]`,
        // `--lg` is `min-h-8 min-w-8 rounded-2xl text-sm leading-[1.43]` and
        // `--sm` is `min-h-4 min-w-4 rounded-xl text-[10px] leading-[1.34]`.
        // The radius is a *step per size*, not a pill: a large badge is a
        // rounded rectangle, which `rounded_full` could not draw, and its box
        // was 24px where v3 asks for 32. The leadings are Tailwind's unitless
        // multipliers, resolved against the badge's own text size.
        let (size_px, font, radius, leading) = match self.size {
            Size::Sm => (
                px(16.),
                px(10.),
                crate::util::small_radius(cx),
                DefiniteLength::Fraction(1.34),
            ),
            Size::Md => (
                px(28.),
                px(12.),
                crate::util::control_radius(cx),
                DefiniteLength::Fraction(1.34),
            ),
            Size::Lg => (
                px(32.),
                px(14.),
                crate::util::soft_radius(cx),
                DefiniteLength::Fraction(1.43),
            ),
        };
        // The size step stays the fallback; an instance radius replaces it.
        let radius = self.radius.unwrap_or(radius);
        let font = self.text_size.unwrap_or(font);

        // Each placement class pins the badge at its anchor corner with
        // `top/right/bottom/left: 0` and then translates itself `±25%` of
        // its own box outward — an overhang of a quarter of the badge: 4px
        // sm, 7px md, 8px lg at the min box. The pin sits on the badge here;
        // the translate is [`PlacedBadge`]'s, applied in prepaint from the
        // badge's laid-out box.
        let (top, bottom, left, right) = match self.placement {
            BadgePlacement::TopRight => (Some(px(0.)), None, None, Some(px(0.))),
            BadgePlacement::TopLeft => (Some(px(0.)), None, Some(px(0.)), None),
            BadgePlacement::BottomRight => (None, Some(px(0.)), None, Some(px(0.))),
            BadgePlacement::BottomLeft => (None, Some(px(0.)), Some(px(0.)), None),
        };

        let (bg, fg) = paint(colors, self.variant, self.color);

        let badge = gpui::div()
            .absolute()
            .debug_selector(|| "badge".to_owned())
            // `min-h`/`min-w`, not a fixed box: a badge with a longer label
            // grows sideways rather than clipping.
            .min_w(size_px)
            .min_h(size_px)
            .gap(px(2.))
            .rounded(radius)
            .bg(bg)
            .text_color(fg)
            .text_size(font)
            .line_height(leading)
            .font_weight(gpui::FontWeight::MEDIUM)
            .flex()
            // `shrink-0` is pinned on `.badge` itself, not only the anchor.
            // It is inert here — every placement class makes the badge
            // absolute, out of the flex flow — but it is v3's declared value.
            .flex_shrink_0()
            .items_center()
            .justify_center()
            // v3 rings every anchored badge against the page background with
            // `border: 1px solid var(--background)`; there is no prop, because
            // without it the badge and its anchor bleed together.
            .border_1()
            .border_color(colors.background)
            .when_some(top, |b, t| b.top(t))
            .when_some(bottom, |b, v| b.bottom(v))
            .when_some(left, |b, l| b.left(l))
            .when_some(right, |b, r| b.right(r));

        // v3's root renders its children in the badge itself; with no
        // children — the omitted label — the badge is a dot, a circle at the
        // badge size.
        let badge = if self.children.is_empty() {
            badge.size(size_px).max_w(size_px)
        } else {
            badge.children(self.children)
        };
        PlacedBadge {
            placement: self.placement,
            badge: crate::util::apply_sx(badge, &self.sx).into_any_element(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pure variant×color paint matrix of `badge.css`: the base rule,
    /// the `.badge--{color}` foreground classes, and the compound
    /// `.badge--{variant}.badge--{color}` cells, over both appearances. The
    /// headless test window cannot sample a fill, so the cascade is pinned
    /// here instead.
    #[test]
    fn paint_matrix_matches_the_badge_css_cascade() {
        for colors in [ThemeColors::light(), ThemeColors::dark()] {
            for color in Color::ALL {
                let role = colors.role(color);

                // `.badge--primary.badge--{color}` fills with the role itself
                // and labels in the role foreground.
                assert_eq!(
                    paint(&colors, BadgeVariant::Primary, color),
                    (role.color, role.foreground),
                    "primary×{color:?} must fill with the role and label in its foreground"
                );

                // Secondary keeps the base `--badge-bg: var(--default)`
                // whatever the colour; only the label changes, to the
                // soft foreground (`.badge--default` labels in
                // `--default-foreground`).
                assert_eq!(
                    paint(&colors, BadgeVariant::Secondary, color).0,
                    colors.default.color,
                    "secondary×{color:?} must keep the default fill"
                );
                assert_eq!(
                    paint(&colors, BadgeVariant::Secondary, color).1,
                    if color == Color::Default {
                        colors.default.foreground
                    } else {
                        role.soft_foreground(colors.foreground)
                    },
                    "secondary×{color:?} must label in the colour's soft foreground"
                );

                // Soft fills with `--{color}-soft`, lighter than the role
                // itself, and labels in the soft foreground.
                assert_eq!(
                    paint(&colors, BadgeVariant::Soft, color),
                    (role.soft(), role.soft_foreground(colors.foreground)),
                    "soft×{color:?} must fill with the soft mix and label in the soft foreground"
                );
                assert!(
                    role.soft() != role.color,
                    "the soft fill of {color:?} must not equal the solid fill"
                );
            }
        }

        // The roles are distinct fills: no colour may borrow another's
        // primary background.
        let colors = ThemeColors::light();
        for (a, b) in [
            (Color::Default, Color::Accent),
            (Color::Accent, Color::Success),
            (Color::Success, Color::Warning),
            (Color::Warning, Color::Danger),
        ] {
            assert_ne!(
                colors.role(a).color,
                colors.role(b).color,
                "the {a:?} and {b:?} roles must not share a fill"
            );
        }
    }
}

crate::util::impl_component_styled!(BadgeAnchor, BadgeLabel, Badge);
