//! InputGroup — port of `@heroui/input-group` (v3).
//!
//! Combines a field with adjacent addons and controls behind one shared piece
//! of field chrome, so a prefix label, the input itself and a trailing button
//! read as a single control.

use gpui::prelude::FluentBuilder;
use gpui::{
    div, px, AnyElement, App, ElementId, InteractiveElement, IntoElement, MouseButton,
    ParentElement, Pixels, RenderOnce, SharedString, Styled, Window,
};
use herogpui_core::{element_id, FieldVariant};
use herogpui_theme::ActiveTheme;

use crate::{
    a11y::{self, A11y as _},
    util,
};

/// A static, non-interactive segment of an [`InputGroup`] — the `$` before an
/// amount, or a `.com` suffix.
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct InputAddon {
    text: SharedString,
    /// The `sx` slot, refined over the root style at the end of render.
    sx: Option<Box<gpui::StyleRefinement>>,
}

impl InputAddon {
    /// Creates an addon showing the given text.
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            text: text.into(),
            sx: None,
        }
    }

    /// The one slot for caller-owned low-level styling: GPUI's styling methods
    /// (`bg`, `text_color`, `w`, `h`, `p`, `rounded`, `border_color`, …)
    /// applied to the addon's root element after every value the active theme
    /// chose, so they win.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        util::refine_sx(&mut self.sx, style);
        self
    }
}

impl RenderOnce for InputAddon {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        // `.input-group__prefix` / `__suffix`: `px-3`, transparent, and drawn in
        // `--field-placeholder`. The slot itself carries the inset in v3, so
        // bare icon addons sit the same 12px in as `InputAddon` text.
        let el = div()
            .flex()
            .items_center()
            .flex_shrink_0()
            .text_color(cx.colors().field.placeholder)
            .child(self.text.to_string());
        util::apply_sx(el, &self.sx)
    }
}

/// HeroUI InputGroup.
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct InputGroup {
    variant: FieldVariant,
    full_width: bool,
    /// Optional group geometry/chrome overrides; defaults are the stock box.
    field: util::FieldBox,
    /// The group box's hover endpoint, in place of the variant's token.
    group_hover_bg: Option<gpui::Hsla>,
    /// The family the held field is drawn and measured with; unset keeps the
    /// field's own setting.
    font_family: Option<SharedString>,
    /// The corner radius used by the group's shared chrome. The held field is
    /// transparent and unrounded so this remains the only visible outline.
    radius: Option<Pixels>,
    is_disabled: bool,
    is_invalid: bool,
    is_required: bool,
    label: Option<SharedString>,
    description: Option<SharedString>,
    error_message: Option<SharedString>,
    /// `InputGroup.Prefix` — the leading addon.
    prefix: Option<AnyElement>,
    /// `InputGroup.Suffix` — the trailing addon.
    suffix: Option<AnyElement>,
    /// `InputGroup.Input` / `InputGroup.TextArea` — held rather than rendered
    /// so the group can strip its chrome and tell it which sides an addon
    /// occupies.
    input: Option<crate::input::Input>,
    /// Whether the held field came from [`InputGroup::text_area`], so the
    /// group can play the pinned `:has([data-slot="input-group-textarea"])`
    /// rules: top alignment, auto height, 8px addon top padding — and the
    /// `querySelector("input")` exception to root-click focusing.
    is_textarea: bool,
    children: Vec<AnyElement>,
    id: Option<ElementId>,
    /// The `sx` slot, refined over the root style at the end of render.
    sx: Option<Box<gpui::StyleRefinement>>,
}

impl InputGroup {
    /// Creates an empty input group.
    pub fn new() -> Self {
        Self {
            variant: FieldVariant::Primary,
            full_width: false,
            field: util::FieldBox::default(),
            group_hover_bg: None,
            font_family: None,
            radius: None,
            is_disabled: false,
            is_invalid: false,
            is_required: false,
            label: None,
            description: None,
            error_message: None,
            prefix: None,
            suffix: None,
            input: None,
            is_textarea: false,
            children: Vec::new(),
            id: None,
            sx: None,
        }
    }

    /// Names this group so it can report `role="group"`. Unnamed groups
    /// produce no AccessKit node.
    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Sets the field variant.
    pub fn variant(mut self, variant: FieldVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Sets whether the group fills the available width.
    pub fn full_width(mut self, v: bool) -> Self {
        self.full_width = v;
        self
    }

    /// Replaces the group's 36px minimum height and propagates the explicit
    /// height to the held single-line field; a textarea group stays
    /// content-sized.
    pub fn height(mut self, h: impl Into<Pixels>) -> Self {
        self.field.height = Some(h.into());
        self
    }

    /// Replaces the held field's `px-3` inset on every side without an addon;
    /// a side with a prefix or suffix keeps that addon's own padding.
    pub fn padding_x(mut self, p: impl Into<Pixels>) -> Self {
        self.field.padding_x = Some(p.into());
        self
    }

    /// Renders the group with no background, border, field shadow, focus ring
    /// or hover fill, for a caller painting around it.
    pub fn is_bare(mut self, v: bool) -> Self {
        self.field.is_bare = v;
        self.field.is_bare_is_set = true;
        self
    }

    /// The group box's fill while hovered, in place of `--field-hover`
    /// (`--default-hover` on the secondary variant). The 150ms ease-smooth
    /// fade and the border hover are unchanged; a focused, invalid, disabled
    /// or bare group does not hover, so the override never reaches those
    /// states. Not a v3 prop: v3 tints the group with a class.
    pub fn group_hover_bg(mut self, color: impl Into<gpui::Hsla>) -> Self {
        self.group_hover_bg = Some(color.into());
        self
    }

    /// Shows or hides only the group's visual focus ring. The held field
    /// remains focusable and editable when set to `false`.
    pub fn focus_ring(mut self, v: bool) -> Self {
        self.field.focus_ring = Some(v);
        self
    }

    /// The family the held field is drawn and measured with; unset keeps the
    /// field's own setting.
    pub fn font_family(mut self, family: impl Into<SharedString>) -> Self {
        self.font_family = Some(family.into());
        self
    }

    /// The corner radius of the group box, in place of the owning
    /// `field_radius` helper. The held field remains transparent and
    /// unrounded. Not a v3 prop; the removed v2
    /// `radius` prop is prohibited and this is a per-component repository
    /// extension.
    ///
    /// The group's chrome uses the resolved radius; the inner field keeps the
    /// upstream `rounded-none` grouped-input contract.
    pub fn radius(mut self, radius: impl Into<Pixels>) -> Self {
        self.radius = Some(radius.into());
        self
    }

    /// Sets whether the group is disabled.
    pub fn is_disabled(mut self, v: bool) -> Self {
        self.is_disabled = v;
        self
    }

    /// `isRequired` — marks the group's label as required. v3's examples get
    /// this from the `TextField` around the group.
    pub fn is_required(mut self, v: bool) -> Self {
        self.is_required = v;
        self
    }

    /// Sets whether the group is invalid.
    pub fn is_invalid(mut self, v: bool) -> Self {
        self.is_invalid = v;
        self
    }

    /// Sets the label text.
    pub fn label(mut self, text: impl Into<SharedString>) -> Self {
        self.label = Some(text.into());
        self
    }

    /// Sets the description text.
    pub fn description(mut self, text: impl Into<SharedString>) -> Self {
        self.description = Some(text.into());
        self
    }

    /// Sets the error message text.
    pub fn error_message(mut self, text: impl Into<SharedString>) -> Self {
        self.error_message = Some(text.into());
        self
    }

    /// `InputGroup.Prefix` — content before the field.
    pub fn prefix(mut self, el: impl IntoElement) -> Self {
        self.prefix = Some(el.into_any_element());
        self
    }

    /// `InputGroup.Suffix` — content after the field.
    pub fn suffix(mut self, el: impl IntoElement) -> Self {
        self.suffix = Some(el.into_any_element());
        self
    }

    /// `InputGroup.Input` — the field itself.
    ///
    /// Taken as an [`crate::input::Input`] rather than an element so the group
    /// can strip its chrome: v3's group paints the box, and the inner input is
    /// transparent and flush against the addons. Passing one as a plain child
    /// instead leaves a second, smaller field drawn inside the group.
    pub fn input(mut self, input: crate::input::Input) -> Self {
        self.input = Some(input);
        self.is_textarea = false;
        self
    }

    /// `InputGroup.TextArea` — a multi-line field in the same shared chrome.
    pub fn text_area(mut self, text_area: crate::textarea::TextArea) -> Self {
        self.input = Some(text_area.into_group_input());
        self.is_textarea = true;
        self
    }

    /// The one slot for caller-owned low-level styling: GPUI's styling methods
    /// (`bg`, `text_color`, `w`, `h`, `p`, `rounded`, `border_color`, …)
    /// applied to the group's root element — the column holding the label,
    /// the group box and the message — after every value the variant and the
    /// active theme chose, so they win. The group box's own chrome stays with
    /// the variant.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        util::refine_sx(&mut self.sx, style);
        self
    }
}

impl Default for InputGroup {
    fn default() -> Self {
        Self::new()
    }
}

impl ParentElement for InputGroup {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for InputGroup {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // `.input-group` rings on `focus-within`, and what is inside it is a
        // real `Input` or `TextArea`, so their state is where the focus is.
        // A `TextArea` is converted to an `Input` by `text_area`, so there is one
        // slot to ask.
        let focus_within = self
            .input
            .as_ref()
            .is_some_and(|input| input.state_focus(cx).is_focused(window));
        // The hover fade owns a keyed animation state, so the render below
        // needs to borrow the app mutably after reading the theme snapshot.
        // Keep the values local and stable for the complete field shell.
        let colors = cx.colors().clone();
        let layout = cx.layout().clone();
        let is_invalid = self.is_invalid || self.error_message.is_some();
        let (is_disabled, is_textarea) = (self.is_disabled, self.is_textarea);
        let field_box = self.field;
        // Resolve the radius passed to the group's shared field chrome.
        let radius = self.radius.unwrap_or_else(|| util::field_radius(cx));
        // A textarea group grows with its content and ignores the height
        // override; every other group defaults to the 36px row.
        let explicit_height = if is_textarea { None } else { field_box.height };
        // The held field's state entity names this instance's probes; two
        // groups sharing one state would share the probes and the field both.
        let entity = self
            .input
            .as_ref()
            .map(|input| input.state().entity_id().as_u64());
        // NamedInteger keeps the held field's entity identity in the element
        // path without flattening it into a formatted string (which can make
        // two scoped paths collide on a separator). An input-less group uses
        // the zero seed only for its listener-free fallback surface.
        let group_id = ElementId::named_usize("input-group", entity.unwrap_or(0) as usize);

        // `.input-group` is `inline-flex min-h-9 items-center` with no padding
        // of its own: the prefix, the input and the suffix each carry `px-3`,
        // which is what keeps the addons flush with the field's edges. With a
        // textarea inside, the pinned `:has` rule switches the group to
        // `items-start` with `height: auto`, so the box grows downward around
        // the multi-line field instead of centring it in a row.
        let mut group = div()
            .id(group_id.clone())
            .flex()
            .flex_row()
            .map(|g| {
                if is_textarea {
                    g.items_start()
                } else {
                    g.items_center()
                }
            })
            .min_h(explicit_height.unwrap_or(util::FIELD_HEIGHT))
            .when_some(explicit_height, |g, h| g.h(h))
            .text_size(util::FIELD_TEXT)
            .line_height(px(20.))
            .text_color(colors.field.foreground);
        if let Some(entity) = entity {
            group = group.debug_selector(move || format!("input-group-{entity}-group"));
        }

        // v3 rings the *group* on `focus-within`, so the state comes from the
        // field inside it.
        if !field_box.is_bare {
            // The held field carries the same value, so the group box and the
            // box it holds keep one corner.
            // The group box is a plain flex row that never clips, so the
            // ring rides as an overlay child and keeps a corner concentric
            // with the box's own rather than the squarer one a spread
            // shadow re-uses.
            group = util::apply_field_chrome_overlay(
                group,
                self.variant,
                is_invalid,
                focus_within,
                field_box.focus_ring.unwrap_or(true),
                Some(radius),
                cx,
            );

            // HeroUI's focus-within and invalid rules both use the field-focus
            // endpoint.  The shared chrome supplies the ring/border; this
            // explicit fill keeps the group surface in sync for both variants
            // (secondary uses its neutral default endpoint).
            if focus_within || is_invalid {
                group = group.bg(match self.variant {
                    FieldVariant::Primary => colors.field.focus(),
                    FieldVariant::Secondary => colors.default.color,
                });
            }
        }
        if self.full_width {
            group = group.w_full();
        }
        // `status-disabled` is one dim over the whole group box, so the held
        // field, the addon slots and any composed children dim exactly once;
        // the propagated field skips its own coat (`group_dim`), and the
        // folded label dims itself beside the box. v3's `pointer-events:
        // none` cannot come with it — an arbitrary child of a disabled group
        // only dims here, it does not go inert.
        if is_disabled {
            group = group.opacity(layout.disabled_opacity);
        }

        // `.input-group:hover:not(:focus-within)` is `bg-field-hover` plus
        // `--field-border-hover`, and `.input-group--secondary` swaps only the
        // fill for `--input-group-bg-hover: var(--default-hover)`. The
        // refinement is baked off while the focus is inside, and a focus
        // change repaints through a re-render, so the suppressed hover never
        // paints over the focused chrome. v3's `status-disabled` is
        // `pointer-events: none` first, so a disabled group hovers never.
        if !field_box.is_bare && !focus_within && !is_invalid && !is_disabled {
            let hover_bg = self.group_hover_bg.unwrap_or(match self.variant {
                FieldVariant::Primary => colors.field.hover(),
                FieldVariant::Secondary => colors.default.hover(),
            });
            let hover_border = colors.field.border_hover();
            // Keep the group identity and focus listeners stable while only
            // the hover surface interpolates over HeroUI's 150ms ease-smooth
            // transition. The border endpoint remains an immediate refinement.
            group = crate::anim::hover_fade_with_duration_and_easing(
                group,
                element_id::scoped(&group_id, "hover-fade"),
                (
                    match self.variant {
                        FieldVariant::Primary => colors.field.background,
                        FieldVariant::Secondary => colors.default.color,
                    },
                    hover_bg,
                ),
                None,
                Some(hover_border),
                |fill| fill.rounded(radius),
                Some(150),
                crate::anim::HoverFadeEasing::EaseSmooth,
                window,
                cx,
            );
        }

        // v3.2.4 `InputGroupRoot.handleClick`: a click on the group outside
        // the contained input focuses the input (`target !== input &&
        // !input.contains(target)` -> `input.focus()`), so clicking the
        // prefix or the suffix starts typing in the field instead of
        // blurring it. The handler sits on this root box — no wrapper above
        // the field — and bubble order runs the deeper element first, so the
        // dispatch has already settled which handle the press lands on:
        // a click inside the input finds the field holding the focus (its
        // own mouse-down placed the caret) and must not write a second focus
        // behind it, while a click on a focusable suffix (a Button) finds
        // the button holding it and hands the focus to the field — the
        // button's click still fires on mouse-up, because gpui decides
        // clicks by hover, not focus, so the pinned order "action, then the
        // field" holds. That check is the focused handle itself, not
        // `default_prevented`: the input's own focus transfer prevented
        // default too, and skipping on that alone would exempt the very
        // suffix button v3 hands the focus past. `prevent_default` runs on
        // both arms — it is what keeps the press from falling through to a
        // focusable ancestor, which would otherwise take the focus the
        // moment the dispatch reaches the app focus root. Two pinned
        // exceptions keep the handler off the box: `querySelector("input")`
        // finds nothing in a textarea-only group, so a click there focuses
        // nothing; and a disabled field — the group's own flag, propagated
        // below, or the field's — never takes a browser focus.
        let field_disabled = self
            .input
            .as_ref()
            .is_some_and(|input| input.builder_is_disabled());
        if !is_textarea && !is_disabled && !field_disabled {
            if let Some(focus) = self.input.as_ref().map(|input| input.state_focus(cx)) {
                group = group.on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    if !window.focused(cx).is_some_and(|held| held == focus) {
                        window.focus(&focus, cx);
                    }
                    window.prevent_default();
                });
            }
        }

        // Order matters: prefix, field, suffix, then anything else the caller
        // put in. The field is told which sides an addon occupies so it can
        // drop that padding. Group-disabled reaches the field itself — the
        // propagated field stops tracking focus and answering keys, exactly
        // as a browser's disabled `<input>` does, and skips its own dim
        // because the box above already carries it.
        let addon_slot = |el: AnyElement, name: &'static str| -> AnyElement {
            // Prefix/suffix are real slots in v3: their side border belongs to
            // the slot, not to the input.  A zero-width field border keeps the
            // stock theme invisible while custom bordered fields retain the
            // seam. The wrapper also gives arbitrary prefix/suffix children
            // the same placeholder colour and vertical alignment as InputAddon.
            let mut slot = div()
                .flex_shrink_0()
                .when(!is_textarea, |slot| slot.h_full())
                .flex()
                .items_center()
                .justify_center()
                .px(px(12.))
                .text_color(colors.field.placeholder)
                .when(name == "prefix", |slot| {
                    slot.border_r(layout.field_border_width)
                        .border_color(colors.field.border)
                })
                .when(name == "suffix", |slot| {
                    slot.border_l(layout.field_border_width)
                        .border_color(colors.field.border)
                });
            // `:has([data-slot="input-group-textarea"])` top-aligns the
            // addons and gives each `padding-top: 0.5rem`, so addon text starts
            // level with the textarea's first line.
            if is_textarea {
                slot = slot.items_start().pt(px(8.));
            }
            if let Some(entity) = entity {
                slot = slot.debug_selector(move || format!("input-group-{entity}-{name}"));
            }
            slot.child(el).into_any_element()
        };
        let (has_prefix, has_suffix) = (self.prefix.is_some(), self.suffix.is_some());
        if let Some(prefix) = self.prefix {
            group = group.child(addon_slot(prefix, "prefix"));
        }
        if let Some(input) = self.input {
            let input = if is_disabled {
                input.is_disabled(true).group_dim(true)
            } else {
                input
            };
            let input = input.in_group(has_prefix, has_suffix);
            let input = match explicit_height {
                Some(height) => input.height(height),
                None => input,
            };
            let input = match field_box.padding_x {
                Some(padding_x) => input.group_padding_x(padding_x),
                None => input,
            };
            let input = match self.font_family.clone() {
                Some(family) => input.font_family(family),
                None => input,
            };
            let input = match self.radius {
                Some(radius) => input.radius(radius),
                None => input,
            };
            let input = match field_box.focus_ring {
                Some(show) => input.focus_ring(show),
                None => input,
            };
            group = group.child(input.is_bare(field_box.is_bare));
        }
        if let Some(suffix) = self.suffix {
            group = group.child(addon_slot(suffix, "suffix"));
        }
        group = group.children(self.children);

        // `.input-group` wrapper is `gap-1`, like every other field.
        let mut root = div().flex().flex_col().gap(px(4.));
        if self.full_width {
            // `.input-group--full-width` is `w-full`, and the wrapper above
            // the group has to carry the width too: gpui resolves a
            // percentage against the parent, and a content-sized parent
            // stretches nothing.
            root = root.w_full();
        }
        if let Some(label) = self.label {
            root = root.child(
                crate::field::Label::new(label)
                    .is_invalid(is_invalid)
                    .is_required(self.is_required)
                    .is_disabled(is_disabled),
            );
        }
        root = root.child(group);

        let error = is_invalid.then(|| self.error_message.clone()).flatten();
        if let Some(error) = crate::anim::field_error_panel(&group_id, error, window, cx) {
            root = root.child(error);
        } else if let Some(description) = self.description {
            root = root.child(crate::field::Description::new(description));
        }

        root = util::apply_sx(root, &self.sx);
        match self.id {
            Some(id) => root.id(id).a11y(a11y::Role::Group).into_any_element(),
            None => root.into_any_element(),
        }
    }
}

#[cfg(test)]
mod tests {
    // The pinned `.input-group--secondary:hover` fill is
    // `--input-group-bg-hover: var(--default-hover)`, which
    // `RoleColor::hover()` computes; `soft_hover()` is a different, lighter
    // token. The two accessors differ by one word and the wrong one still
    // looks plausible on screen, so the check is mechanical: the source must
    // name the pinned accessor, and the wrong one must not appear at all.
    #[test]
    fn secondary_hover_uses_the_pinned_default_hover_token() {
        // Scan the implementation only; this test module's own text mentions
        // the wrong accessor to forbid it.
        let source = include_str!("input_group.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("the implementation section is always present");
        assert!(
            source.contains("FieldVariant::Secondary => colors.default.hover()"),
            "the secondary group hover must read `colors.default.hover()` \
             (pinned `--input-group-bg-hover: var(--default-hover)`)"
        );
        assert!(
            !source.contains("soft_hover()"),
            "`soft_hover()` is not the pinned hover token for the group and \
             must not come back"
        );
    }

    // v3's `status-disabled` is `pointer-events: none` before it is an
    // opacity, so a disabled group paints the hover refinement never.
    #[test]
    fn the_disabled_group_paints_no_hover() {
        let source = include_str!("input_group.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("the implementation section is always present");
        assert!(
            source.contains(
                "if !field_box.is_bare && !focus_within && !is_invalid && !is_disabled {"
            ),
            "the hover refinement must be gated off while the group is \
             disabled, not only while the focus is inside"
        );
    }

    #[test]
    fn focus_and_invalid_use_the_pinned_field_focus_fill() {
        let source = include_str!("input_group.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("the implementation section is always present");
        assert!(
            source.contains("if focus_within || is_invalid")
                && source.contains("FieldVariant::Primary => colors.field.focus()")
                && source.contains("FieldVariant::Secondary => colors.default.color"),
            "focus-within and invalid group states must paint the HeroUI field-focus \
             endpoint for both variants"
        );
    }

    #[test]
    fn group_hover_uses_the_pinned_smooth_transition() {
        let source = include_str!("input_group.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("the implementation section is always present");
        assert!(
            source.contains("hover_fade_with_duration_and_easing")
                && source.contains("Some(150)")
                && source.contains("HoverFadeEasing::EaseSmooth"),
            "InputGroup hover must animate with HeroUI's 150ms ease-smooth \
             transition"
        );
    }

    #[test]
    fn addon_slots_keep_the_pinned_side_seams() {
        let source = include_str!("input_group.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("the implementation section is always present");
        assert!(
            source.contains("slot.border_r(layout.field_border_width)")
                && source.contains("slot.border_l(layout.field_border_width)"),
            "prefix and suffix slots must own their respective field-border seams"
        );
    }

    // One dim per box: the group carries the whole `status-disabled` opacity
    // and the propagated field skips its own coat, so the opacity must not
    // nest (`group_dim`), and the dim must sit on the group box so composed
    // children dim with it.
    #[test]
    fn the_disabled_dim_covers_the_box_once() {
        let source = include_str!("input_group.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("the implementation section is always present");
        assert!(
            source.contains("input.is_disabled(true).group_dim(true)"),
            "the propagated field must be told the group already dims the box"
        );
        assert!(
            source.contains("group = group.opacity(layout.disabled_opacity)"),
            "the one dim must sit on the group box so composed children dim \
             with it"
        );
    }
}

crate::util::impl_component_styled!(InputAddon, InputGroup);
