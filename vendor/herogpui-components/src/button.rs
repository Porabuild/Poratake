//! Button — port of `@heroui/button` (v3).
//!
//! v3 replaced v2's `variant` x `color` matrix with a single emphasis scale:
//! `primary | secondary | tertiary | outline | ghost | danger | danger-soft`.
//! There is no `color` or `radius` prop, `isLoading` became `isPending`, and
//! v2's `startContent`/`endContent` slots are gone: icons are ordered
//! [`ParentElement`] children around the label.

use gpui::{
    div, prelude::*, AnyElement, App, ClickEvent, Div, ElementId, InteractiveElement, IntoElement,
    ParentElement, Pixels, Refineable, RenderOnce, SharedString, Stateful, Styled, Window,
};
use herogpui_core::{element_id, Size, Variant};
use herogpui_theme::ActiveTheme;

use crate::a11y::{self, A11y as _};
use crate::util;

/// A press handler. `Arc` rather than `Box` because it is bound twice: the
/// pointer's `on_click` and the keyboard's Enter/Space both run it.
type OnPress = std::sync::Arc<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// Which edge of a [`crate::button_group::ButtonGroup`] a button sits on.
///
/// `.button-group .button` is `rounded-none`; the first member takes
/// `rounded-s-3xl` and the last `rounded-e-3xl`, so a joined group has one
/// outer radius rather than a rounded box per member. The press scale is also
/// off inside a group (`.button-group .button:active { transform: none }`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GroupEdge {
    /// First member: the leading corners are round.
    Start,
    /// Between two others: square on both ends.
    Middle,
    /// Last member: the trailing corners are round.
    End,
    /// The only member, so it keeps the full radius.
    Only,
}

/// HeroUI Button.
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    label: Option<SharedString>,
    /// `aria-label` — the explicit accessible name, which wins over the
    /// label-derived name exactly as the HTML name computation ranks it.
    aria_label: Option<SharedString>,
    /// v3's `children`-as-a-function: handed `{isHovered, isPressed, isFocused,
    /// isFocusVisible, isDisabled, isPending}` and drawn in place of the label.
    content: Option<std::sync::Arc<dyn Fn(util::InteractiveState) -> AnyElement + 'static>>,
    variant: Variant,
    variant_is_set: bool,
    size: Size,
    size_is_set: bool,
    full_width: bool,
    /// Set by [`Button::full_width`]. ButtonGroup context supplies width as a
    /// *default* (`button.tsx`: `finalFullWidth = fullWidth ??
    /// context.fullWidth`), so this flag is what keeps an explicit child
    /// `full_width(false)` from being overwritten by a full-width group.
    full_width_is_set: bool,
    is_icon_only: bool,
    /// Set by [`crate::button_group::ButtonGroup`]: which end of the group this
    /// button is, and whether the group stacks.
    group_edge: Option<(GroupEdge, bool)>,
    is_disabled: bool,
    is_disabled_is_set: bool,
    is_pending: bool,
    children: Vec<AnyElement>,
    on_press: Option<OnPress>,
    /// The `sx` slot, refined over the root style at the end of render.
    sx: Option<Box<gpui::StyleRefinement>>,
    /// Set by [`Button::hover_bg`]: the fill the hover fade eases *to*, in
    /// place of the variant's hover colour. Additive — unset, the fade behaves
    /// exactly as it did before the builder existed.
    hover_bg: Option<gpui::Hsla>,
    /// The corner radius, in place of `--radius-3xl` (capped). Group edges and
    /// the press scale still apply.
    radius: Option<Pixels>,
    /// Set by [`Button::width`]: the fixed pixel width.
    width: Option<Pixels>,
    /// Set by [`Button::min_width`]: the width floor.
    min_width: Option<Pixels>,
    /// Set by [`Button::height`]: the fixed pixel height.
    height: Option<Pixels>,
    /// Set by [`Button::padding_x`]: the horizontal inset.
    padding_x: Option<Pixels>,
    /// Set by [`Button::text_size`]: the label's font size.
    text_size: Option<Pixels>,
    /// Set by [`Button::font_weight`]: the label's font weight.
    font_weight: Option<gpui::FontWeight>,
    /// Set by [`Button::grow`]: `flex-1` plus `min-w-0`.
    grow: bool,
    recipes: Vec<SharedString>,
}

impl Button {
    /// Creates a button with the given id.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            label: None,
            aria_label: None,
            content: None,
            variant: Variant::Primary,
            variant_is_set: false,
            size: Size::Md,
            size_is_set: false,
            full_width: false,
            full_width_is_set: false,
            is_icon_only: false,
            group_edge: None,
            is_disabled: false,
            is_disabled_is_set: false,
            is_pending: false,
            children: Vec::new(),
            on_press: None,
            sx: None,
            hover_bg: None,
            radius: None,
            width: None,
            min_width: None,
            height: None,
            padding_x: None,
            text_size: None,
            font_weight: None,
            grow: false,
            recipes: Vec::new(),
        }
    }

    /// v3's render function for a button's children, handed `isHovered`,
    /// `isPressed`, `isFocused`, `isFocusVisible`, `isDisabled` and `isPending`.
    ///
    /// The hover and the press are a frame behind the pointer: gpui reports both
    /// to a handler, so the render that draws them can only read what the last
    /// frame recorded. The button's own hover and press styling does not go
    /// through this -- it is applied by gpui in the same frame.
    pub fn content(
        mut self,
        render: impl Fn(util::InteractiveState) -> AnyElement + 'static,
    ) -> Self {
        self.content = Some(std::sync::Arc::new(render));
        self
    }

    /// Sets the text label.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// `aria-label` — the explicit accessible name, which wins over the
    /// label-derived name exactly as the HTML name computation ranks it.
    pub fn aria_label(mut self, label: impl Into<SharedString>) -> Self {
        self.aria_label = Some(label.into());
        self
    }

    /// The visual style.
    ///
    /// Setting this at the call site suppresses
    /// [`herogpui_theme::ButtonStyle::variant`] from
    /// every recipe on the button — the instance is the more specific source,
    /// so it wins, the same way [`Button::hover_bg`] outranks a recipe's
    /// `hover_bg`. The surprise is what goes with the variant: each one
    /// derives its own hover shade, so
    /// `.variant(Variant::Primary).recipe("accented")` keeps *primary's* hover
    /// even when the recipe was written to change it through its variant. A
    /// recipe that must change the hover names it with
    /// [`herogpui_theme::ButtonStyle::hover_bg`], which is honoured whatever variant is in
    /// force; a hover that should follow a whole role everywhere belongs on
    /// the role instead, through
    /// [`herogpui_theme::ThemeBuilder::role_hover`].
    pub fn variant(mut self, variant: Variant) -> Self {
        self.variant = variant;
        self.variant_is_set = true;
        self
    }

    /// Sets the button size.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self.size_is_set = true;
        self
    }

    /// Fills the parent with `w-full`. An explicit [`Button::width`] is the
    /// more specific source and wins when both are set.
    pub fn full_width(mut self, v: bool) -> Self {
        self.full_width = v;
        self.full_width_is_set = true;
        self
    }

    /// Sets whether the button shows only an icon.
    pub fn is_icon_only(mut self, v: bool) -> Self {
        self.is_icon_only = v;
        self
    }

    /// The button's fixed pixel width, in place of the content-fit ladder.
    ///
    /// Beats [`Button::full_width`] when both are set: `full_width` fills the
    /// parent, a pixel width fixes the box, and the pixel width is the more
    /// specific source. It also replaces the icon-only square. The pressed
    /// skin keeps the fixed box instead of snapping back to the ladder, and a
    /// matching `sx` width still refines the root last.
    ///
    /// Not a v3 prop; a per-component repository extension like
    /// [`Button::radius`].
    pub fn width(mut self, w: impl Into<Pixels>) -> Self {
        self.width = Some(w.into());
        self
    }

    /// The button's width floor, under the content-fit ladder, an explicit
    /// [`Button::width`] and `grow`'s zero floor alike.
    ///
    /// Not a v3 prop; a per-component repository extension like
    /// [`Button::radius`].
    pub fn min_width(mut self, w: impl Into<Pixels>) -> Self {
        self.min_width = Some(w.into());
        self
    }

    /// The button's fixed pixel height, in place of the size ladder's control
    /// height. Beats what [`Button::size`] derives; a matching `sx` height
    /// still refines the root last, and the pressed skin keeps the fixed box.
    ///
    /// Not a v3 prop; a per-component repository extension like
    /// [`Button::radius`].
    pub fn height(mut self, h: impl Into<Pixels>) -> Self {
        self.height = Some(h.into());
        self
    }

    /// The button's horizontal inset, in place of the size ladder's `px-4`
    /// (`px-3` on `--sm`). Beats what [`Button::size`] derives and feeds the
    /// pressed skin's inset geometry; a matching `sx` padding still refines
    /// the root last.
    ///
    /// Not a v3 prop; a per-component repository extension like
    /// [`Button::radius`].
    pub fn padding_x(mut self, p: impl Into<Pixels>) -> Self {
        self.padding_x = Some(p.into());
        self
    }

    /// The label's font size; unset keeps the size ladder's pair (`text-sm`,
    /// stepping to `text-base` on `--lg`). Beats what [`Button::size`]
    /// derives. A Tailwind step keeps its paired leading through
    /// `util::leading_for`; other sizes keep the size step's leading, the
    /// same convention [`crate::chip::Chip::text_size`] records.
    ///
    /// Not a v3 prop; a per-component repository extension like
    /// [`Button::radius`].
    pub fn text_size(mut self, size: impl Into<Pixels>) -> Self {
        self.text_size = Some(size.into());
        self
    }

    /// The label's font weight, in place of `.button`'s `font-medium`. Beats
    /// what [`Button::size`] derives; a matching `sx` weight still refines the
    /// root last.
    ///
    /// Not a v3 prop; a per-component repository extension like
    /// [`Button::radius`].
    pub fn font_weight(mut self, weight: gpui::FontWeight) -> Self {
        self.font_weight = Some(weight);
        self
    }

    /// Fills the row's free width: v3's `flex-1` plus `min-w-0`, the pair a
    /// caller otherwise has to reach for `sx` to spell. The button shares the
    /// free width of its flex parent instead of overflowing it, and may
    /// compress below its own content width — exactly what the `min-w-0` half
    /// is for.
    ///
    /// Coexists with [`Button::full_width`]: `full_width` pins the box to
    /// 100% of the parent, `grow` shares whatever is left over after the
    /// siblings.
    ///
    /// Not a v3 prop; a per-component repository extension like
    /// [`Button::radius`].
    pub fn grow(mut self, v: bool) -> Self {
        self.grow = v;
        self
    }

    /// The one slot for caller-owned low-level styling: GPUI's styling methods
    /// (`bg`, `text_color`, `w`, `h`, `p`, `rounded`, `border_color`, …)
    /// applied to the button's root element after every value the variant and
    /// the active theme chose, so they win. An overridden background also
    /// replaces the hover fade's endpoints and an overridden pixel size the
    /// press geometry, so the override holds across states.
    pub fn sx(mut self, style: impl FnOnce(Div) -> Div) -> Self {
        util::refine_sx(&mut self.sx, style);
        self
    }

    /// The fill the hover fade eases *to*, in place of the variant's own hover
    /// colour.
    ///
    /// The escape hatch for a caller-owned surface: `sx`'s background replaces
    /// both of the fade's endpoints, because a fill that eased back to the
    /// variant colour would paint over the override — so an `sx` background
    /// alone is a button whose hover does not move. Naming the hover colour
    /// restores the transition: the fade runs from the resting background (the
    /// `sx` background when one is set, the variant's resting colour
    /// otherwise) to `color`, over the same `transition-colors` timing every
    /// other button uses. The press state is unaffected either way — v3's
    /// `:active` is the opacity step [`apply_button_variant`] applies, not a
    /// third colour.
    ///
    /// v3 has no such prop; on the web this is `className="hover:bg-…"`.
    pub fn hover_bg(mut self, color: impl Into<gpui::Hsla>) -> Self {
        self.hover_bg = Some(color.into());
        self
    }

    /// The corner radius, in place of `--radius-3xl` (capped). Group edges and
    /// the press scale still apply. Not a v3 prop; the removed v2 `radius`
    /// prop is prohibited and this is a per-component repository extension.
    pub fn radius(mut self, radius: impl Into<Pixels>) -> Self {
        self.radius = Some(radius.into());
        self
    }

    /// Named theme overlay from [`herogpui_theme::ComponentThemes::button`].
    /// Stackable; a missing name adds no override.
    pub fn recipe(mut self, name: impl Into<SharedString>) -> Self {
        self.recipes.push(name.into());
        self
    }

    /// Joins this button to a group edge. Internal: a caller reaches it by
    /// putting the button in a [`crate::button_group::ButtonGroup`].
    pub(crate) fn group_edge(mut self, edge: GroupEdge, vertical: bool) -> Self {
        self.group_edge = Some((edge, vertical));
        self
    }

    /// Applies ButtonGroup context values only where the child did not set its
    /// own prop, matching React's direct-child context precedence.
    pub(crate) fn group_defaults(
        mut self,
        variant: Variant,
        size: Size,
        is_disabled: bool,
        full_width: bool,
    ) -> Self {
        if !self.variant_is_set {
            self.variant = variant;
            self.variant_is_set = true;
        }
        if !self.size_is_set {
            self.size = size;
            self.size_is_set = true;
        }
        if !self.is_disabled_is_set {
            self.is_disabled = is_disabled;
        }
        if !self.full_width_is_set {
            self.full_width = full_width;
        }
        self
    }

    /// The member's resolved width after [`Self::group_defaults`]: an explicit
    /// child value when one was set, the group's `fullWidth` otherwise.
    pub(crate) fn is_full_width(&self) -> bool {
        self.full_width
    }

    /// The member's resolved variant after [`Self::group_defaults`]: an
    /// explicit child value when one was set, the group's otherwise.
    /// ButtonGroup reads it for the member's `bg-current` separator colour.
    pub(crate) fn resolved_variant(&self) -> Variant {
        self.variant
    }

    /// Sets whether the button is disabled.
    pub fn is_disabled(mut self, v: bool) -> Self {
        self.is_disabled = v;
        self.is_disabled_is_set = true;
        self
    }

    /// `isPending` — blocks presses and hover while retaining the tab stop and focus ring.
    /// A `content` closure receives the pending state and owns any loading indicator.
    pub fn is_pending(mut self, v: bool) -> Self {
        self.is_pending = v;
        self
    }

    /// Sets the handler run when the button is pressed.
    pub fn on_press(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_press = Some(std::sync::Arc::new(handler));
        self
    }
}

impl Button {
    pub(crate) fn then_press(
        mut self,
        callback: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        util::chain_press(&mut self.on_press, callback);
        self
    }
}

impl ParentElement for Button {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

/// Paints a button's fill, border, text and interaction states for `variant`.
///
/// Shared with `ButtonGroup`, which propagates the same variant to its members.
pub fn apply_button_variant(
    el: Stateful<Div>,
    variant: Variant,
    interactive: bool,
    cx: &App,
) -> Stateful<Div> {
    apply_variant(el, variant, interactive, true, cx)
}

/// The background pair `variant` eases between on hover, or `None` when the
/// variant has no background to ease.
///
/// Used by [`Button`] to run v3's `transition-colors` through
/// [`crate::anim::hover_fade`] instead of swapping the fill on one frame.
pub fn button_hover_colors(variant: Variant, cx: &App) -> Option<(gpui::Hsla, gpui::Hsla)> {
    let colors = cx.colors();
    match variant {
        Variant::Primary => Some((colors.accent.color, colors.accent.hover())),
        Variant::Secondary => Some((colors.default.color, colors.default.hover())),
        Variant::Tertiary => Some((colors.default.color, colors.default.hover())),
        Variant::Outline => Some((gpui::transparent_black(), colors.default.color.alpha(0.6))),
        Variant::Ghost => Some((gpui::transparent_black(), colors.default.color)),
        Variant::Danger => Some((colors.danger.color, colors.danger.hover())),
        Variant::DangerSoft => Some((colors.danger.soft(), colors.danger.soft_hover())),
    }
}

/// The pinned `--button-bg-pressed` endpoint for each variant. HeroUI changes
/// the background on press; it does not dim the whole button with opacity.
fn button_pressed_background(variant: Variant, cx: &App) -> gpui::Hsla {
    let colors = cx.colors();
    match variant {
        Variant::Primary => colors.accent.hover(),
        Variant::Secondary | Variant::Tertiary => colors.default.hover(),
        Variant::Outline | Variant::Ghost => colors.default.color,
        Variant::Danger => colors.danger.hover(),
        Variant::DangerSoft => colors.danger.soft_hover(),
    }
}

/// [`apply_button_variant`], with `hover_bg` off when the caller is going to
/// animate the background itself.
fn apply_variant(
    el: Stateful<Div>,
    variant: Variant,
    interactive: bool,
    hover_bg: bool,
    cx: &App,
) -> Stateful<Div> {
    let colors = cx.colors();
    let layout = cx.layout();

    match variant {
        Variant::Primary => {
            let base = colors.accent;
            let el = el.text_color(base.foreground);
            let el = if hover_bg { el.bg(base.color) } else { el };
            if interactive {
                el.when(hover_bg, |e| {
                    e.hover(move |s| s.bg(base.hover()))
                        .active(move |s| s.bg(base.hover()))
                })
            } else {
                el
            }
        }
        // `secondary` is the neutral filled style: v3 maps the removed
        // `bg-secondary` token to `bg-default`.
        Variant::Secondary => {
            let base = colors.default;
            let el = el.text_color(colors.accent.soft_foreground(colors.foreground));
            let el = if hover_bg { el.bg(base.color) } else { el };
            if interactive {
                el.when(hover_bg, |e| {
                    e.hover(move |s| s.bg(base.hover()))
                        .active(move |s| s.bg(base.hover()))
                })
            } else {
                el
            }
        }
        Variant::Tertiary => {
            let base = colors.default;
            let fg = colors.foreground;
            let el = if hover_bg {
                el.bg(base.color).text_color(fg)
            } else {
                el.text_color(fg)
            };
            if interactive {
                el.when(hover_bg, |e| {
                    e.hover(move |s| s.bg(base.hover()))
                        .active(move |s| s.bg(base.hover()))
                })
            } else {
                el
            }
        }
        Variant::Outline => {
            let base = colors.default;
            let el = el
                .border(layout.border_width)
                .border_color(colors.border)
                .text_color(base.foreground);
            if interactive {
                el.when(hover_bg, |e| {
                    e.hover(move |s| s.bg(base.color.alpha(0.6)))
                        .active(move |s| s.bg(base.color))
                })
            } else {
                el
            }
        }
        Variant::Ghost => {
            let base = colors.default;
            let el = el.text_color(base.foreground);
            if interactive {
                el.when(hover_bg, |e| {
                    e.hover(move |s| s.bg(base.color))
                        .active(move |s| s.bg(base.color))
                })
            } else {
                el
            }
        }
        Variant::Danger => {
            let base = colors.danger;
            let el = el.text_color(base.foreground);
            let el = if hover_bg { el.bg(base.color) } else { el };
            if interactive {
                el.when(hover_bg, |e| {
                    e.hover(move |s| s.bg(base.hover()))
                        .active(move |s| s.bg(base.hover()))
                })
            } else {
                el
            }
        }
        Variant::DangerSoft => {
            let base = colors.danger;
            let el = el.text_color(base.soft_foreground(colors.foreground));
            let el = if hover_bg { el.bg(base.soft()) } else { el };
            if interactive {
                el.when(hover_bg, |e| {
                    e.hover(move |s| s.bg(base.soft_hover()))
                        .active(move |s| s.bg(base.soft_hover()))
                })
            } else {
                el
            }
        }
    }
}

/// Button's own type and spacing ladder, from `button.css`.
///
/// Only three things move across the sizes. `.button` sets `px-4 gap-2 text-sm`
/// for every size; `.button--sm` narrows the padding to `px-3` and `.button--lg`
/// steps the type up to `text-base` — neither touches the gap, and `--sm` does
/// not touch the type. Reading a generic sm/md/lg ladder instead made the small
/// button's label a step too small and the large button's padding and gap a
/// step too wide.
fn button_metrics(size: Size) -> ButtonMetrics {
    let (text, line_height) = match size {
        // `text-sm` / `text-base`, with Tailwind's paired line heights.
        Size::Sm | Size::Md => (gpui::px(14.), gpui::px(20.)),
        Size::Lg => (gpui::px(16.), gpui::px(24.)),
    };
    ButtonMetrics {
        text,
        line_height,
        // `px-3` on `--sm`, `px-4` everywhere else.
        padding_x: match size {
            Size::Sm => gpui::px(12.),
            Size::Md | Size::Lg => gpui::px(16.),
        },
        // `gap-2`, never overridden.
        gap: gpui::px(8.),
    }
}

struct ButtonMetrics {
    text: Pixels,
    line_height: Pixels,
    padding_x: Pixels,
    gap: Pixels,
}

/// The text colour `variant` paints, for child svgs that cannot inherit
/// `text_color` from their parent.
pub fn button_foreground(variant: Variant, cx: &App) -> gpui::Hsla {
    let colors = cx.colors();
    match variant {
        Variant::Primary => colors.accent.foreground,
        Variant::Secondary => colors.accent.soft_foreground(colors.foreground),
        Variant::Tertiary => colors.foreground,
        Variant::Outline | Variant::Ghost => colors.default.foreground,
        Variant::Danger => colors.danger.foreground,
        Variant::DangerSoft => colors.danger.soft_foreground(colors.foreground),
    }
}

/// [`group_radius`] for any styled element — `ToggleButtonGroup` merges its
/// members' corners the same way `.button-group` does.
pub(crate) fn group_radius_any<T: Styled>(
    el: T,
    edge: Option<(GroupEdge, bool)>,
    radius: Pixels,
) -> T {
    let Some((edge, vertical)) = edge else {
        return el.rounded(radius);
    };
    match (edge, vertical) {
        (GroupEdge::Only, _) => el.rounded(radius),
        (GroupEdge::Start, false) => el.rounded_tl(radius).rounded_bl(radius),
        (GroupEdge::End, false) => el.rounded_tr(radius).rounded_br(radius),
        (GroupEdge::Start, true) => el.rounded_tl(radius).rounded_tr(radius),
        (GroupEdge::End, true) => el.rounded_bl(radius).rounded_br(radius),
        (GroupEdge::Middle, _) => el,
    }
}

/// The border sides an outline group member drops, in gpui's per-side order.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) struct CollapsedSides {
    pub left: bool,
    pub right: bool,
    pub top: bool,
    pub bottom: bool,
}

/// Which borders an outline member's group position collapses, read off the
/// pinned `button-group.css`: the horizontal sheet rows are
/// `:first-child { border-e-0 }`, `:last-child { border-s-0 }` and a middle
/// member (`:not(:first-child):not(:last-child)`) `border-x-0`; the vertical
/// sheet mirrors them into the block axis with `border-b-0`, `border-t-0` and
/// `border-y-0`. A lone member is `:first-child:last-child`, so both edge
/// rules apply at once and its whole stacking-axis border collapses. Pure so
/// every GroupEdge x orientation case can be table-tested against the pinned
/// stylesheet.
pub(crate) fn collapsed_border_sides(edge: GroupEdge, vertical: bool) -> CollapsedSides {
    match (edge, vertical) {
        (GroupEdge::Start, false) => CollapsedSides {
            right: true,
            ..Default::default()
        },
        (GroupEdge::End, false) => CollapsedSides {
            left: true,
            ..Default::default()
        },
        (GroupEdge::Middle | GroupEdge::Only, false) => CollapsedSides {
            left: true,
            right: true,
            ..Default::default()
        },
        (GroupEdge::Start, true) => CollapsedSides {
            bottom: true,
            ..Default::default()
        },
        (GroupEdge::End, true) => CollapsedSides {
            top: true,
            ..Default::default()
        },
        (GroupEdge::Middle | GroupEdge::Only, true) => CollapsedSides {
            top: true,
            bottom: true,
            ..Default::default()
        },
    }
}

/// Zeroes exactly the collapsed sides of an already-bordered element.
fn apply_collapsed_sides<T: Styled>(el: T, sides: CollapsedSides) -> T {
    let el = if sides.left { el.border_l_0() } else { el };
    let el = if sides.right { el.border_r_0() } else { el };
    let el = if sides.top { el.border_t_0() } else { el };
    if sides.bottom {
        el.border_b_0()
    } else {
        el
    }
}

/// Applies `radius` to only the corners a group edge leaves round.
fn group_radius(
    el: Stateful<Div>,
    edge: Option<(GroupEdge, bool)>,
    radius: Pixels,
) -> Stateful<Div> {
    let Some((edge, vertical)) = edge else {
        return el.rounded(radius);
    };
    match (edge, vertical) {
        (GroupEdge::Only, _) => el.rounded(radius),
        // Horizontal: the start edge rounds its left corners, the end edge its
        // right ones. Vertical: top and bottom.
        (GroupEdge::Start, false) => el.rounded_tl(radius).rounded_bl(radius),
        (GroupEdge::End, false) => el.rounded_tr(radius).rounded_br(radius),
        (GroupEdge::Start, true) => el.rounded_tl(radius).rounded_tr(radius),
        (GroupEdge::End, true) => el.rounded_bl(radius).rounded_br(radius),
        (GroupEdge::Middle, _) => el,
    }
}

/// The one radius a focus-ring overlay can be drawn at, when there is one.
///
/// `util::focus_ring_overlay` builds its bands from a scalar radius, so it can
/// only stand in for the shadow ring on a button -- or a `ToggleButton`, which
/// groups the same way -- whose four corners resolve to the same value. That is the ungrouped button, a lone member (`Only`), and a
/// `Middle` member, whose corners are all square; a `Start` or `End` member
/// rounds one side and keeps the other flush against its neighbour, and an `sx`
/// refinement can break the symmetry of any of them. Those cases return `None`
/// and keep the spread-shadow ring, which dilates whatever per-corner shape the
/// element already has.
pub(crate) fn uniform_ring_radius(
    edge: Option<(GroupEdge, bool)>,
    radius: Pixels,
    sx_corners: &gpui::Corners<Option<Pixels>>,
) -> Option<Pixels> {
    let base = match edge {
        None | Some((GroupEdge::Only, _)) => radius,
        Some((GroupEdge::Middle, _)) => gpui::px(0.),
        Some((GroupEdge::Start | GroupEdge::End, _)) => return None,
    };
    let resolved = [
        sx_corners.top_left,
        sx_corners.top_right,
        sx_corners.bottom_right,
        sx_corners.bottom_left,
    ]
    .map(|corner| corner.unwrap_or(base));
    resolved
        .iter()
        .all(|corner| *corner == resolved[0])
        .then_some(resolved[0])
}

impl RenderOnce for Button {
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // The handle that says whether this button holds the focus.
        // `use_keyed_state` takes `cx` mutably, so it precedes the tokens.
        let focus_handle = util::tab_stop_handle(element_id::scoped(&self.id, "focus"), window, cx);
        // One keyed slot owns hover and press state for both render-prop and
        // plain buttons. The hover fade and press ramp read it, while the
        // final tracker installs the single event layer on the stable slot.
        // Keeping the slot for plain buttons also lets their CSS press
        // transition run without layering a second listener onto the fade.
        let interaction = Some(util::interaction(
            element_id::scoped(&self.id, "interaction"),
            window,
            cx,
        ));
        let layout = cx.layout();
        let button_theme = cx.theme().components.button.resolve(&self.recipes);
        if !self.variant_is_set {
            if let Some(variant) = button_theme.variant {
                self.variant = variant;
            }
        }
        if !self.size_is_set {
            if let Some(size) = button_theme.size {
                self.size = size;
            }
        }
        let explicit_radius = self.radius;
        self.radius = self.radius.or(button_theme.radius);
        if self.hover_bg.is_none() {
            self.hover_bg = button_theme
                .hover_bg
                .map(|color| color.resolve(cx.colors()));
        }
        let theme_style = button_theme.style.map(Box::new);
        let theme_bg = button_theme
            .background
            .map(|color| color.resolve(cx.colors()));
        let theme_fg = button_theme
            .foreground
            .map(|color| color.resolve(cx.colors()));
        let theme_hover_fg = button_theme
            .hover_foreground
            .map(|color| color.resolve(cx.colors()));
        let theme_pressed_bg = button_theme
            .pressed_bg
            .map(|color| color.resolve(cx.colors()));
        let theme_disabled_fg = button_theme
            .disabled_foreground
            .map(|color| color.resolve(cx.colors()));
        // Copied out: `hover_fade` below takes `&mut App`, and holding the
        // `layout` borrow across it would be a second borrow of `cx`.
        let disabled_opacity = layout.disabled_opacity;
        let focusable = !self.is_disabled;
        let interactive = focusable && !self.is_pending;
        if !interactive {
            if let Some(slot) = &interaction {
                if *slot.read(cx) != (false, false) {
                    slot.update(cx, |state, _| *state = (false, false));
                }
            }
        }
        // v3's `transition-colors`: the fill eases rather than switching on the
        // frame the pointer arrives. The variant then leaves the background
        // alone so the two do not fight over it. `fade_endpoints` resolves
        // which pair it eases: `hover_bg` names the hover end and the resting
        // background (the `sx` one, else the variant's) becomes the other,
        // while an `sx` background on its own replaces *both* endpoints —
        // the fill the fade draws would otherwise paint the variant colour
        // back over the override.
        let sx_background = util::sx_background(&self.sx).or(theme_bg);
        let mut resolved_style = theme_style.unwrap_or_default();
        if let Some(height) = self.height {
            resolved_style.size.height = Some(height.into());
        }
        if let Some(width) = self.width {
            resolved_style.size.width = Some(width.into());
        }
        if let Some(padding) = self.padding_x {
            resolved_style.padding.left = Some(padding.into());
            resolved_style.padding.right = Some(padding.into());
        }
        if let Some(text) = self.text_size {
            resolved_style.text.font_size = Some(text.into());
        }
        if let Some(weight) = self.font_weight {
            resolved_style.text.font_weight = Some(weight);
        }
        if self.full_width && self.width.is_none() {
            resolved_style.size.width = Some(gpui::relative(1.).into());
        }
        if self.grow {
            resolved_style.flex_grow = Some(1.);
            resolved_style.flex_shrink = Some(1.);
            resolved_style.flex_basis = Some(gpui::px(0.).into());
            resolved_style.min_size.width = Some(gpui::px(0.).into());
        }
        if let Some(width) = self.min_width {
            resolved_style.min_size.width = Some(width.into());
        }
        if explicit_radius.is_some() {
            resolved_style.corner_radii = Default::default();
        }
        if let Some(text) = self.text_size {
            resolved_style.text.line_height = Some(gpui::DefiniteLength::from(
                util::leading_for(text).unwrap_or_else(|| button_metrics(self.size).line_height),
            ));
        }
        if let Some(style) = &self.sx {
            resolved_style.refine(style);
        }
        let sx_size = util::sx_pixel_size(&Some(resolved_style.clone()));
        let relative_width = matches!(
            resolved_style.size.width,
            Some(gpui::Length::Definite(gpui::DefiniteLength::Fraction(_)))
        );
        let (skin_style, slot_style) = util::split_press_style(&resolved_style);
        let sx_corners = util::sx_radius(&Some(resolved_style.clone()));
        // The resting box, the hover fade's fill and the press box all take
        // the same resolved corner, so it is resolved once.
        let radius = self.radius.unwrap_or_else(|| util::control_radius(cx));
        let fade = interactive
            .then(|| button_hover_colors(self.variant, cx))
            .and_then(|variant| util::fade_endpoints(variant, sx_background, self.hover_bg));
        // The pressed endpoint both press paths resolve: the theme's
        // `pressed_bg` when a recipe names one, else the variant's
        // `--button-bg-pressed`. The hover fade eases to it while the
        // interaction slot's pressed bit is set; the reduced-motion press
        // refinement paints it outright.
        let pressed_bg = theme_pressed_bg.unwrap_or(button_pressed_background(self.variant, cx));

        // The size ladder is the default; the instance text and padding
        // builders replace their rung. An overridden size re-pairs its leading
        // through `util::leading_for` when the value is a Tailwind step, and
        // keeps the ladder's leading otherwise — the convention Chip's
        // `text_size` records.
        let derived = button_metrics(self.size);
        let metrics = ButtonMetrics {
            text: self.text_size.unwrap_or(derived.text),
            line_height: self
                .text_size
                .and_then(util::leading_for)
                .unwrap_or(derived.line_height),
            padding_x: self.padding_x.unwrap_or(derived.padding_x),
            gap: derived.gap,
        };
        // The resolved resting height: an instance builder beats the size
        // ladder, and the matching `sx` height still refines the root last.
        let height = self.height.unwrap_or_else(|| self.size.control_height());
        // RAC's `Button` renders a native `<button>`, so upstream's role is
        // implicit and its accessible name comes from the rendered children.
        // A gpui text child carries no id, so it contributes no node and no
        // name (see `a11y`), which is why the label is restated here.
        let name = a11y::Name::maybe(self.aria_label.clone().or(self.label.clone()));
        let mut el = div()
            .id(self.id.clone())
            .flex()
            .flex_row()
            .items_center()
            .justify_center()
            .flex_shrink_0()
            // `button.css` declares no `overflow`: a label too long for the
            // button spills, it is not cut. Clipping it here also gave the row
            // an automatic minimum size of zero, which let the label collapse
            // instead of overflowing.
            .whitespace_nowrap()
            .font_weight(self.font_weight.unwrap_or(gpui::FontWeight::MEDIUM))
            .map(|e| group_radius(e, self.group_edge, radius))
            .map(|e| util::round_sx_corners(e, &sx_corners))
            .text_size(metrics.text)
            .line_height(metrics.line_height)
            .h(height);

        el = if self.is_icon_only {
            el.w(self.size.icon_control_size())
        } else {
            el.px(metrics.padding_x).gap(metrics.gap)
        };

        // An explicit pixel width is the more specific source: it wins over
        // `full_width` and the icon-only square alike.
        if let Some(width) = self.width {
            el = el.w(width);
        } else if self.full_width {
            el = el.w_full();
        }

        if self.grow {
            // `flex-1` plus `min-w-0` on the skin: inside a press slot it must
            // fill that slot, and without one it is the row item itself. The
            // same pair goes onto the slot further down, so the caller's row
            // stretches whichever element it actually lays out.
            el = el.flex_1().min_w(gpui::px(0.));
        }

        if let Some(min_width) = self.min_width {
            el = el.min_w(min_width);
        }

        el = apply_variant(el, self.variant, interactive, fade.is_none(), cx);

        // `button-group.css` collapses the borders an outline member shows
        // toward its neighbours so a seam is the one composed separator
        // hairline rather than two borders. `collapsed_border_sides` holds
        // the per-case mapping; outside a group the full border stays.
        if self.variant == Variant::Outline {
            if let Some((edge, vertical)) = self.group_edge {
                el = apply_collapsed_sides(el, collapsed_border_sides(edge, vertical));
            }
        }

        // The fade's animated layer is glued under everything that follows: the
        // colour transition lives on an inset fill *inside* the button, so the
        // button's own element id — and with it the hover listener latch — never
        // moves when the fill's animation restarts (see `anim::hover_fade`).
        // The interaction slot is always present: `track_interaction` owns
        // `on_hover`, and the fade reads the hover and press bits the slot
        // records — including the keyboard press — instead of binding a second
        // listener or a competing `.active` background.
        if let Some(colors) = fade {
            let edge = self.group_edge;
            el = crate::anim::hover_fade_with_press(
                el,
                element_id::scoped(&self.id, "fade"),
                colors,
                pressed_bg,
                interaction
                    .as_ref()
                    .expect("a button always creates its interaction slot"),
                move |fill| {
                    util::round_sx_corners(group_radius_any(fill, edge, radius), &sx_corners)
                },
                window,
                cx,
            );
        }

        el.style().refine(&skin_style);
        let (hovered, pressed) = interaction
            .as_ref()
            .map(|slot| *slot.read(cx))
            .unwrap_or_default();
        let foreground = if !interactive {
            theme_disabled_fg.or(theme_fg)
        } else if pressed {
            button_theme
                .pressed_foreground
                .map(|color| color.resolve(cx.colors()))
                .or(theme_hover_fg)
                .or(theme_fg)
        } else if hovered {
            theme_hover_fg.or(theme_fg)
        } else {
            theme_fg
        };
        if let Some(foreground) = foreground {
            el = el.text_color(foreground);
        }
        if let Some(color) = self.sx.as_ref().and_then(|style| style.text.color) {
            el = el.text_color(color);
        }

        if let Some(render) = self.content.clone() {
            let (is_hovered, is_pressed) = if interactive {
                interaction
                    .as_ref()
                    .map(|slot| *slot.read(cx))
                    .unwrap_or_default()
            } else {
                (false, false)
            };
            let focused = focusable && focus_handle.is_focused(window);
            el = el.child(render(util::InteractiveState {
                is_hovered,
                is_pressed,
                is_focused: focused,
                is_focus_visible: focused && util::focus_visible(cx),
                is_selected: false,
                is_disabled: self.is_disabled,
                is_pending: self.is_pending,
                is_indeterminate: false,
            }));
        } else if let Some(label) = self.label {
            el = el.child(label.to_string());
        }
        el = el.children(self.children);

        // v3's `[data-pressed]` press ramp. Applied last so the press geometry
        // sits on top of whatever the variant did to padding.
        //
        // `button.css` declares the press as a transition
        // (`transform 250ms var(--ease-smooth), background-color 100ms
        // var(--ease-out)`), so the skin rides
        // `pressed_with_background_ramp`: the colour track eases between the
        // same resting fill the hover fade holds and the variant's
        // `--button-bg-pressed` endpoint.
        if interactive && self.group_edge.is_none() {
            let press_scale = match self.size {
                Size::Sm => crate::anim::PRESSED_SCALE_SUBTLE,
                Size::Md => crate::anim::PRESSED_SCALE,
                Size::Lg => crate::anim::PRESSED_SCALE_FIRM,
            };
            let press_box = crate::anim::PressBox {
                // An `sx` pixel size keeps the press footprint at the
                // overridden box instead of snapping back to the ladder; an
                // instance builder sits between the two.
                height: sx_size
                    .height
                    .or(self.height)
                    .unwrap_or_else(|| self.size.control_height()),
                padding_x: (!self.is_icon_only).then_some(metrics.padding_x),
                width: sx_size
                    .width
                    .or(self.width)
                    .or_else(|| self.is_icon_only.then(|| self.size.icon_control_size())),
                // v3's `.button` is `w-fit` with no minimum, so a press has
                // no floor to scale; a caller's `min_width` rides on the skin
                // itself, which the press refinement never strips.
                min_width: None,
                text_size: metrics.text,
                line_height: metrics.line_height,
                gap: metrics.gap,
                radius,
                shrink_x: !relative_width
                    && !self.full_width
                    && !self.grow
                    && resolved_style.flex_grow.unwrap_or(0.) == 0.,
                scale: press_scale,
            };
            if ActiveTheme::reduce_motion(cx) {
                el = crate::anim::pressed_with_background(el, press_box, pressed_bg, cx);
            } else {
                // The hover fade owns the colour whenever it is mounted —
                // its pressed endpoint is the same `pressed_bg` — so the
                // ramp keeps only the transform track, the shape
                // CloseButton's press takes.
                el = crate::anim::pressed_with_background_ramp(
                    el,
                    press_box,
                    None,
                    crate::anim::BUTTON_PRESS,
                    interaction.as_ref(),
                    window,
                    cx,
                );
            }
        }

        // When the press wrapper is present, `el` is now the stable press
        // slot — the element the caller's row actually lays out — so the
        // stretch pair lands here too. (On paths without a wrapper this
        // re-states what the skin above already carries.)
        if self.grow {
            el = el.flex_1().min_w(gpui::px(0.));
        }

        if let Some(on_press) = self.on_press {
            if interactive {
                // gpui fires a *focused* element's click listeners on Enter and
                // Space with `ClickEvent::Keyboard`, which is React Aria's press
                // exactly -- so this one binding answers the pointer and the
                // keyboard, and the focus handle above is what switched the
                // second half on.
                el = el.on_click(move |ev: &ClickEvent, window, cx| on_press(ev, window, cx));
            }
        }

        // The interaction tracking (hover, mouse/keyboard press bits the
        // `content` closure reads) belongs on the press slot: key events
        // dispatch along the focus path, which runs through the slot — the
        // skin is its child.
        if interactive {
            if let Some(slot) = &interaction {
                el = util::track_interaction(el, slot);
            }
        }

        // `.button:focus-visible` is `status-focused`: a 2px ring, offset from
        // the button by another in the background colour. A disabled button is
        // not a tab stop, which is what `pointer-events-none` amounts to here.
        // The focus tracking lands on the press slot (the element `pressed`
        // returns) so keyboard activation and pointer activation answer on the
        // same element, and the ring draws around the resting footprint. The
        // pending/disabled dimming covers the label, which lives above the
        // skin.
        //
        // The ring is the overlay form wherever the button's four corners
        // resolve to one radius, because an overlay is crisp and concentric
        // where a spread shadow keeps the element's own corner. A grouped
        // member that rounds only the corners on its outer edge has no single
        // radius an overlay could take -- one bordered div carries one
        // `rounded()` per corner but the overlay's outer band is built from a
        // scalar -- so those stay on the shadow ring, which dilates whatever
        // per-corner shape the element already has.
        if focusable {
            el = match uniform_ring_radius(self.group_edge, radius, &sx_corners) {
                Some(ring_radius) => util::ring_overlay_if_focused(
                    el.track_focus(&focus_handle),
                    &focus_handle,
                    true,
                    ring_radius,
                    Vec::new(),
                    window,
                    cx,
                ),
                None => util::ring_if_focused(
                    el.track_focus(&focus_handle),
                    &focus_handle,
                    true,
                    Vec::new(),
                    window,
                    cx,
                ),
            };
        }

        if self.is_disabled || self.is_pending {
            el = el.opacity(disabled_opacity);
        }

        el.style().refine(&slot_style);
        el = el
            .a11y_named(a11y::Role::Button, &name)
            .key_context("Button");
        if focusable {
            el = util::record_focus_bounds(el, &focus_handle, window, cx);
        }
        el.into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_corners_refine_group_edges_without_rounding_unnamed_seams() {
        let sx = gpui::Corners {
            top_right: Some(gpui::px(12.)),
            ..Default::default()
        };
        let mut skin = group_radius_any(div(), Some((GroupEdge::Start, false)), gpui::px(2.));
        skin = util::round_sx_corners(skin, &sx);
        let corners = &skin.style().corner_radii;
        assert_eq!(corners.top_left, Some(gpui::px(2.).into()));
        assert_eq!(corners.top_right, Some(gpui::px(12.).into()));
        assert_eq!(corners.bottom_left, Some(gpui::px(2.).into()));
        assert_eq!(
            corners.bottom_right, None,
            "the unmentioned attached edge stays square"
        );
    }

    /// Stand-ins for the variant's own pair and the two overrides. Distinct
    /// values so every assertion below names which one it expected, rather
    /// than comparing a colour against itself.
    const VARIANT_IDLE: gpui::Hsla = gpui::Hsla {
        h: 0.0,
        s: 0.5,
        l: 0.5,
        a: 1.0,
    };
    const VARIANT_HOVER: gpui::Hsla = gpui::Hsla {
        h: 0.1,
        s: 0.5,
        l: 0.5,
        a: 1.0,
    };
    const SX: gpui::Hsla = gpui::Hsla {
        h: 0.2,
        s: 0.5,
        l: 0.5,
        a: 1.0,
    };
    const CUSTOM_HOVER: gpui::Hsla = gpui::Hsla {
        h: 0.3,
        s: 0.5,
        l: 0.5,
        a: 1.0,
    };

    /// (a) A caller-owned surface *and* a hover colour: the fade eases between
    /// exactly those two, so the button no longer sits frozen on its override.
    #[test]
    fn hover_bg_eases_from_the_sx_background() {
        let endpoints = util::fade_endpoints(
            Some((VARIANT_IDLE, VARIANT_HOVER)),
            Some(SX),
            Some(CUSTOM_HOVER),
        );

        assert_eq!(
            endpoints,
            Some((SX, CUSTOM_HOVER)),
            "the fade must rest on the sx background and ease to the named hover colour"
        );
        let (idle, hovered) = endpoints.unwrap();
        assert_ne!(
            idle, hovered,
            "the fade must not be frozen once hover_bg is set"
        );
    }

    /// (b) The behaviour `hover_bg` is an escape hatch from: an `sx`
    /// background alone still pins both endpoints, so nothing repaints the
    /// variant colour over the override.
    #[test]
    fn sx_background_alone_still_freezes_both_endpoints() {
        assert_eq!(
            util::fade_endpoints(Some((VARIANT_IDLE, VARIANT_HOVER)), Some(SX), None),
            Some((SX, SX)),
            "an sx background with no hover_bg must hold across hover"
        );
    }

    /// (c) No `sx`: the fade keeps the variant's resting colour and only the
    /// hover end is replaced.
    #[test]
    fn hover_bg_without_sx_eases_from_the_variant_resting_colour() {
        let variant = (VARIANT_IDLE, VARIANT_HOVER);

        assert_eq!(
            util::fade_endpoints(Some(variant), None, Some(CUSTOM_HOVER)),
            Some((variant.0, CUSTOM_HOVER)),
            "the resting end must stay the variant's own colour"
        );
    }

    /// With neither override the resolution is the identity, which is what
    /// keeps every existing button pixel-identical.
    #[test]
    fn no_override_passes_the_variant_pair_through() {
        let variant = (VARIANT_IDLE, VARIANT_HOVER);

        assert_eq!(
            util::fade_endpoints(Some(variant), None, None),
            Some(variant)
        );
        assert_eq!(
            util::fade_endpoints(None, None, None),
            None,
            "a variant with no background to ease must stay unfaded"
        );
    }

    #[test]
    fn group_defaults_preserve_explicit_child_props() {
        let button = Button::new("override")
            .variant(Variant::Outline)
            .size(Size::Lg)
            .is_disabled(false)
            .full_width(false)
            .group_defaults(Variant::Secondary, Size::Sm, true, true);

        assert_eq!(button.variant, Variant::Outline);
        assert_eq!(button.size, Size::Lg);
        assert!(!button.is_disabled);
        assert!(!button.is_full_width());
    }

    #[test]
    fn group_defaults_fill_unset_child_props() {
        let button =
            Button::new("inherited").group_defaults(Variant::Secondary, Size::Sm, true, true);

        assert_eq!(button.variant, Variant::Secondary);
        assert_eq!(button.size, Size::Sm);
        assert!(button.is_disabled);
        assert!(button.is_full_width());
    }

    /// `button-group.css` outline collapse, one row per GroupEdge x
    /// orientation, each naming the pinned selector that demands it.
    #[test]
    fn outline_collapse_table_matches_pinned_css() {
        let cases = [
            (
                GroupEdge::Start,
                false,
                CollapsedSides { right: true, ..Default::default() },
                ".button-group--horizontal .button--outline:first-child { border-e-0 }",
            ),
            (
                GroupEdge::End,
                false,
                CollapsedSides { left: true, ..Default::default() },
                ".button-group--horizontal .button--outline:last-child { border-s-0 }",
            ),
            (
                GroupEdge::Middle,
                false,
                CollapsedSides { left: true, right: true, ..Default::default() },
                ".button-group--horizontal .button--outline:not(:first-child):not(:last-child) { border-x-0 }",
            ),
            (
                GroupEdge::Start,
                true,
                CollapsedSides { bottom: true, ..Default::default() },
                ".button-group--vertical .button--outline:first-child { border-b-0 }",
            ),
            (
                GroupEdge::End,
                true,
                CollapsedSides { top: true, ..Default::default() },
                ".button-group--vertical .button--outline:last-child { border-t-0 }",
            ),
            (
                GroupEdge::Middle,
                true,
                CollapsedSides { top: true, bottom: true, ..Default::default() },
                ".button-group--vertical .button--outline:not(:first-child):not(:last-child) { border-y-0 }",
            ),
        ];
        for (edge, vertical, expected, selector) in cases {
            assert_eq!(
                collapsed_border_sides(edge, vertical),
                expected,
                "`{selector}` must collapse exactly these borders"
            );
        }

        // A lone member is `:first-child:last-child`, so both edge rules
        // apply at once and its whole stacking-axis border collapses.
        assert_eq!(
            collapsed_border_sides(GroupEdge::Only, false),
            CollapsedSides {
                left: true,
                right: true,
                ..Default::default()
            },
            "a lone horizontal outline member matches :first-child:last-child, \
             so border-e-0 and border-s-0 both apply"
        );
        assert_eq!(
            collapsed_border_sides(GroupEdge::Only, true),
            CollapsedSides {
                top: true,
                bottom: true,
                ..Default::default()
            },
            "a lone vertical outline member matches :first-child:last-child, \
             so border-b-0 and border-t-0 both apply"
        );
    }

    /// `button.tsx`: `finalFullWidth = fullWidth ?? context.fullWidth` — the
    /// child value wins in both directions, and an unset child inherits the
    /// context in both directions.
    #[test]
    fn group_defaults_full_width_precedence() {
        let inherit_false =
            Button::new("inherit-false").group_defaults(Variant::Primary, Size::Md, false, false);
        let inherit_true =
            Button::new("inherit-true").group_defaults(Variant::Primary, Size::Md, false, true);
        let override_false = Button::new("override-false")
            .full_width(false)
            .group_defaults(Variant::Primary, Size::Md, false, true);
        let override_true = Button::new("override-true")
            .full_width(true)
            .group_defaults(Variant::Primary, Size::Md, false, false);

        assert!(!inherit_false.is_full_width());
        assert!(inherit_true.is_full_width());
        assert!(
            !override_false.is_full_width(),
            "an explicit child fullWidth=false must survive a full-width group context"
        );
        assert!(
            override_true.is_full_width(),
            "an explicit child fullWidth=true must survive a non-full group context"
        );
    }
}

crate::util::impl_component_styled!(Button);
