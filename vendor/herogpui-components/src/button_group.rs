//! ButtonGroup — port of `@heroui/button-group` (v3).
//!
//! Joined buttons sharing one outer radius. The pinned root hands `variant`,
//! `size`, `isDisabled` and `fullWidth` to its direct `Button` children through
//! context as *defaults* (`button.tsx` merges `prop ?? context.prop`), so a
//! member's own explicit value — including `isDisabled={false}`,
//! `fullWidth={false}` and any Button variant, `outline` included — always
//! wins. Members reached through [`ButtonGroup::button`] are those typed direct
//! children.

use gpui::{
    div, prelude::*, px, AnyElement, App, ElementId, IntoElement, ParentElement, RenderOnce,
    Styled, Window,
};
use herogpui_core::{Orientation, Size, Variant};

use crate::{
    a11y::{self, A11y as _},
    button::Button,
    util,
};

/// The variant whose foreground the separator drawn inside a member's slot
/// takes. v3 composes `ButtonGroup.Separator` as a child of the member that
/// follows the seam and paints it `bg-current`, so the hairline inherits that
/// member's currentColor — its own variant once
/// [`Button::group_defaults`](crate::button::Button::group_defaults) has
/// resolved it — rather than one group-wide colour. A type-erased child
/// receives no context in v3 either, so its slot falls back to the group's
/// variant.
pub(crate) fn separator_variant(member: Option<Variant>, group: Variant) -> Variant {
    member.unwrap_or(group)
}

/// HeroUI ButtonGroup.
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct ButtonGroup {
    variant: Variant,
    size: Size,
    /// Whether a `ButtonGroup.Separator` is drawn before each member after the
    /// first. v3 composes it as a child of the member that follows it, so this
    /// is the slot flag rather than a documented prop. Defaults to none, as a
    /// group without `Separator` children draws no dividers.
    separators: bool,
    is_disabled: bool,
    orientation: Orientation,
    full_width: bool,
    /// Buttons added through [`ButtonGroup::button`], which inherit the
    /// group's `variant`, `size`, `isDisabled` and `fullWidth` unless they set
    /// their own.
    buttons: Vec<Button>,
    children: Vec<AnyElement>,
    id: Option<ElementId>,
    /// The `sx` slot, refined over the root style at the end of render.
    sx: Option<Box<gpui::StyleRefinement>>,
}

impl ButtonGroup {
    /// `isDisabled` — disables every member.
    pub fn is_disabled(mut self, v: bool) -> Self {
        self.is_disabled = v;
        self
    }

    /// `orientation` — a vertical group stacks its buttons.
    pub fn orientation(mut self, orientation: Orientation) -> Self {
        self.orientation = orientation;
        self
    }

    /// Creates an empty button group.
    pub fn new() -> Self {
        Self {
            variant: Variant::Primary,
            size: Size::Md,
            separators: false,
            is_disabled: false,
            orientation: Orientation::Horizontal,
            full_width: false,
            buttons: Vec::new(),
            children: Vec::new(),
            id: None,
            sx: None,
        }
    }

    /// Names this group so it can report `role="group"`. Unnamed groups
    /// produce no AccessKit node — a constant id would fold every instance
    /// into one.
    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// The variant every member inherits unless that button sets its own.
    pub fn variant(mut self, variant: Variant) -> Self {
        self.variant = variant;
        self
    }

    /// Sets the size inherited by the group's buttons.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// Sets whether the group fills the available width.
    pub fn full_width(mut self, v: bool) -> Self {
        self.full_width = v;
        self
    }

    /// Adds a button that inherits the group's `variant` and `size`.
    pub fn button(mut self, button: Button) -> Self {
        self.buttons.push(button);
        self
    }

    /// `ButtonGroup.Separator` — the hairline before each member after the
    /// first.
    ///
    /// v3 composes it as a child of whichever member should show one, and
    /// `ButtonGroupRoot` synthesizes none of its own, so this port spells the
    /// composition as a flag. Defaults to false: a group only draws dividers
    /// when its example composes them.
    pub fn separators(mut self, v: bool) -> Self {
        self.separators = v;
        self
    }

    /// The one slot for caller-owned low-level styling: GPUI's styling methods
    /// (`bg`, `text_color`, `w`, `h`, `p`, `rounded`, `border_color`, …)
    /// applied to the group's root element after every value the orientation
    /// and the full-width layout chose, so they win.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        util::refine_sx(&mut self.sx, style);
        self
    }
}

impl Default for ButtonGroup {
    fn default() -> Self {
        Self::new()
    }
}

impl ParentElement for ButtonGroup {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for ButtonGroup {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let vertical = !self.orientation.is_horizontal();
        // `.button-group` is `inline-flex items-center justify-center gap-0`
        // and nothing else -- no border, no radius, no background. Each member
        // keeps its own fill, and the outer corners come from the first and last
        // of them. This used to draw a bordered, radius-clipped box with the
        // members overlapped by -1px, which is not a shape v3 has.
        // A vertical column that kept `items-center` would leave every member
        // at its own content width, stacking mixed-width labels into a
        // staircase instead of one joined control, so the column stretches its
        // members' cross axis and each slot below stretches the member inside
        // it. Members with a definite width (icon-only) keep it, exactly as
        // they keep `w-fit` under v3's `items-center`.
        let mut el = div().flex().justify_center();
        el = if vertical {
            el.flex_col().items_stretch()
        } else {
            el.flex_row().items_center()
        };
        if self.full_width {
            el = el.w_full();
        }

        let variant = self.variant;
        let size = self.size;
        let disabled = self.is_disabled;
        let inherited: Vec<Button> = self.buttons;
        let extra: Vec<AnyElement> = self.children;
        let total = inherited.len() + extra.len();

        // `.button-group__separator` is `bg-current opacity-15`, 1px by 50% of
        // the member, sitting one pixel before its leading edge. `bg-current`
        // resolves per member — `separator_variant` below decides which.
        let separator_radius = util::hairline_radius(cx);
        let separators = self.separators;

        let edge = move |i: usize| {
            if total <= 1 {
                crate::button::GroupEdge::Only
            } else if i == 0 {
                crate::button::GroupEdge::Start
            } else if i + 1 == total {
                crate::button::GroupEdge::End
            } else {
                crate::button::GroupEdge::Middle
            }
        };

        // The edge and each member's resolved width have to reach the `Button`
        // before it is erased to an `AnyElement`, so the members are built
        // index-first. The width is v3's `finalFullWidth = fullWidth ??
        // context.fullWidth`: only a member that resolves to full width takes
        // a stretch slot, which is what frees an explicit child
        // `full_width(false)` from the group's equal division. Each slot also
        // carries the member's resolved variant — the colour its separator
        // inherits — or `None` for a type-erased child, which receives no
        // context in v3 either.
        let mut members: Vec<(bool, Option<Variant>, AnyElement)> = Vec::with_capacity(total);
        for (i, b) in inherited.into_iter().enumerate() {
            let b = b.group_defaults(variant, size, disabled, self.full_width);
            let member_full = b.is_full_width();
            let slot_variant = b.resolved_variant();
            let el = b.group_edge(edge(i), vertical).into_any_element();
            members.push((member_full, Some(slot_variant), el));
        }
        members.extend(extra.into_iter().map(|child| (false, None, child)));

        let mut wrapped: Vec<gpui::Div> = Vec::with_capacity(total);
        for (i, (member_full, slot_variant, child)) in members.into_iter().enumerate() {
            let mut slot = div().relative().child(child);
            // Names the laid-out slot for behaviour tests (`debug_bounds`);
            // a no-op outside test-support.
            slot = slot.debug_selector(move || format!("button-group-slot-{i}"));
            if vertical {
                // The root's stretch gives the slot the column width; this
                // hands it on to a member whose own width is still auto.
                slot = slot.flex().flex_col().items_stretch();
            } else if member_full {
                slot = slot.flex_1();
            }
            if separators && i > 0 {
                let separator_color =
                    crate::button::button_foreground(separator_variant(slot_variant, variant), cx)
                        .alpha(0.15);
                slot = slot.child(
                    div()
                        .absolute()
                        .bg(separator_color)
                        .rounded(separator_radius)
                        .map(|s| {
                            if vertical {
                                s.left(gpui::relative(0.25))
                                    .top(px(-1.))
                                    .w(gpui::relative(0.5))
                                    .h(px(1.))
                            } else {
                                s.left(px(-1.))
                                    .top(gpui::relative(0.25))
                                    .w(px(1.))
                                    .h(gpui::relative(0.5))
                            }
                        }),
                );
            }
            wrapped.push(slot);
        }
        el = el.children(wrapped);
        el = util::apply_sx(el, &self.sx);
        match self.id {
            Some(id) => el.id(id).a11y(a11y::Role::Group).into_any_element(),
            None => el.into_any_element(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn separators_are_explicit_composition() {
        assert!(
            !ButtonGroup::new().separators,
            "v3 groups without a Separator child must not synthesize dividers"
        );
        assert!(ButtonGroup::new().separators(true).separators);
    }

    #[test]
    fn outline_variant_reaches_grouped_buttons() {
        assert_eq!(
            ButtonGroup::new().variant(Variant::Outline).variant,
            Variant::Outline,
            "pinned ButtonGroup source accepts Button's outline variant"
        );
    }

    /// v3's separator is `bg-current`, so the hairline composed into a
    /// member's slot takes that member's resolved variant foreground — the
    /// very property `Button::group_defaults` computes — and a type-erased
    /// child's slot falls back to the group variant.
    #[test]
    fn separator_variant_follows_its_member_slot() {
        assert_eq!(
            separator_variant(Some(Variant::Danger), Variant::Secondary),
            Variant::Danger,
            "a typed member's resolved variant owns its separator colour"
        );
        assert_eq!(
            separator_variant(Some(Variant::Outline), Variant::Primary),
            Variant::Outline
        );
        assert_eq!(
            separator_variant(None, Variant::Secondary),
            Variant::Secondary,
            "a type-erased child receives no context and falls back to the group variant"
        );
    }
}

crate::util::impl_component_styled!(ButtonGroup);
