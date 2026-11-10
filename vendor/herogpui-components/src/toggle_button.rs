//! ToggleButton & ToggleButtonGroup — port of `@heroui/toggle-button` v3.
//!
//! Toggle between selected/unselected. Supports all button variants, sizes,
//! icon-only, controlled `isSelected` and group selection modes.

use gpui::{
    div, prelude::*, px, AnyElement, App, ClickEvent, ElementId, IntoElement, ParentElement,
    Pixels, RenderOnce, SharedString, Styled, Window,
};
use herogpui_core::{element_id, Orientation as SelectionOrientation, SelectionMode, Size};
use herogpui_theme::ActiveTheme;

use crate::a11y::{self, A11y as _};

// ---------------------------------------------------------------------------
// ToggleButton
// ---------------------------------------------------------------------------

/// Visual variant of a toggle button.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ToggleVariant {
    /// Filled when unselected (the default).
    #[default]
    Default,
    /// Transparent when unselected.
    Ghost,
}

#[derive(Clone, Default)]
struct ToggleGroupFocusState {
    last_key: Option<SharedString>,
    was_inside: bool,
    restore_on_entry: bool,
    edge_exit: bool,
}

/// A button that toggles between selected and unselected.
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct ToggleButton {
    id: ElementId,
    /// Selection key inside a group. Defaults to the element id, so a group can
    /// namespace its ids without breaking selection.
    key: Option<SharedString>,
    label: Option<SharedString>,
    /// `aria-label` — the explicit accessible name, which wins over the
    /// label-derived name exactly as the HTML name computation ranks it.
    aria_label: Option<SharedString>,
    /// v3's `children`-as-a-function: handed the interactive state, `isSelected`
    /// included, and drawn in place of the label.
    content: Option<std::sync::Arc<dyn Fn(crate::util::InteractiveState) -> AnyElement + 'static>>,
    variant: ToggleVariant,
    size: Size,
    /// Whether the child explicitly set `size`. HeroUI's group context only
    /// supplies a size when the child did not override it.
    size_explicit: bool,
    /// `isSelected` — `None` leaves the button holding the state, seeded from
    /// `defaultSelected`.
    is_selected: Option<bool>,
    default_selected: bool,
    is_icon_only: bool,
    /// Supplied by a toggle group so it can navigate its typed children
    /// without falling through to the window-wide tab order.
    group_focus_handle: Option<gpui::FocusHandle>,
    /// Set by [`ToggleButtonGroup`] when it selects one member at a time.
    /// `useToggleButtonGroupItem` swaps the member's role from `button` to
    /// `radio` and its `aria-pressed` for `aria-checked` in exactly that case.
    group_single_selection: bool,
    /// Set by [`ToggleButtonGroup`]: which end of the group this member is,
    /// and whether the group stacks. `.toggle-button-group .toggle-button` is
    /// `rounded-none` with the outer radius on the first and last member.
    group_edge: Option<(crate::button::GroupEdge, bool)>,
    is_disabled: bool,
    disabled_explicit: bool,
    children: Vec<AnyElement>,
    /// Set by [`ToggleButton::hover_bg`]: the fill the hover fade eases *to*.
    hover_bg: Option<gpui::Hsla>,
    /// The corner radius, in place of the owning helper. Group edges, the
    /// hover fade and the press scale still apply.
    radius: Option<Pixels>,
    /// `Arc` for the same reason as `on_change`: the pointer and the keyboard
    /// each hold it.
    on_press: Option<std::sync::Arc<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>>,
    /// `Arc` rather than `Box`: the handler is bound twice, once for the
    /// pointer and once for Enter and Space.
    on_change: Option<std::sync::Arc<dyn Fn(&bool, &mut Window, &mut App) + 'static>>,
    /// The `sx` slot, refined over the root style at the end of render.
    sx: Option<Box<gpui::StyleRefinement>>,
}

impl ToggleButton {
    /// Overrides the selection key, which otherwise mirrors the element id.
    pub fn key(mut self, key: impl Into<SharedString>) -> Self {
        self.key = Some(key.into());
        self
    }

    /// The member's key inside a [`ToggleButtonGroup`].
    pub fn selection_key(&self) -> SharedString {
        self.key.clone().unwrap_or_else(|| match &self.id {
            ElementId::Name(name) => name.clone(),
            other => other.to_string().into(),
        })
    }

    /// Creates a toggle button with the given element id.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            content: None,
            id: id.into(),
            key: None,
            label: None,
            aria_label: None,
            variant: ToggleVariant::Default,
            size: Size::Md,
            size_explicit: false,
            is_selected: None,
            default_selected: false,
            is_icon_only: false,
            group_focus_handle: None,
            group_single_selection: false,
            group_edge: None,
            is_disabled: false,
            disabled_explicit: false,
            children: Vec::new(),
            on_press: None,
            on_change: None,
            sx: None,
            hover_bg: None,
            radius: None,
        }
    }

    /// v3's render function for the button's children, handed `isHovered`,
    /// `isPressed`, `isFocused`, `isFocusVisible` and `isSelected`. The hover and
    /// the press are a frame behind the pointer -- gpui reports both to a
    /// handler, not to the render that draws them.
    pub fn content(
        mut self,
        render: impl Fn(crate::util::InteractiveState) -> AnyElement + 'static,
    ) -> Self {
        self.content = Some(std::sync::Arc::new(render));
        self
    }

    /// Sets the text label.
    pub fn label(mut self, l: impl Into<SharedString>) -> Self {
        self.label = Some(l.into());
        self
    }

    /// `aria-label` — the explicit accessible name, which wins over the
    /// label-derived name exactly as the HTML name computation ranks it.
    pub fn aria_label(mut self, label: impl Into<SharedString>) -> Self {
        self.aria_label = Some(label.into());
        self
    }

    /// Sets the variant.
    pub fn variant(mut self, v: ToggleVariant) -> Self {
        self.variant = v;
        self
    }

    /// Sets the size; a size set here takes precedence over one inherited from a group.
    pub fn size(mut self, s: Size) -> Self {
        self.size = s;
        self.size_explicit = true;
        self
    }

    /// The fill the hover fade eases to, in place of the variant's hover
    /// colour. The fade runs from the resting background — the `sx` background
    /// when one is set, the variant's resting colour otherwise — so this is the
    /// Button contract, on a toggle. v3 has no such prop.
    pub fn hover_bg(mut self, color: impl Into<gpui::Hsla>) -> Self {
        self.hover_bg = Some(color.into());
        self
    }

    /// The corner radius, in place of the owning `control_radius` helper.
    /// Group edges, the hover fade and the press scale still apply. Not a v3
    /// prop; the removed v2 `radius` prop is prohibited and this is a
    /// per-component repository extension.
    pub fn radius(mut self, radius: impl Into<Pixels>) -> Self {
        self.radius = Some(radius.into());
        self
    }

    /// The one slot for caller-owned low-level styling: GPUI's styling methods
    /// (`bg`, `text_color`, `w`, `h`, `p`, `rounded`, `border_color`, …)
    /// applied to the button's root element after every value the variant, the
    /// size and the active theme chose, so they win.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        crate::util::refine_sx(&mut self.sx, style);
        self
    }

    fn group_size(mut self, size: Size) -> Self {
        if !self.size_explicit {
            self.size = size;
        }
        self
    }

    /// Sets the selected state (controlled).
    pub fn is_selected(mut self, v: bool) -> Self {
        self.is_selected = Some(v);
        self
    }

    /// `defaultSelected` — the uncontrolled initial state.
    ///
    /// Only consulted when `isSelected` is not supplied; the button then owns
    /// the state and toggles itself. A group always drives selection, so this
    /// is for a standalone toggle.
    pub fn default_selected(mut self, v: bool) -> Self {
        self.default_selected = v;
        self
    }

    /// Sets whether the button is icon-only.
    pub fn is_icon_only(mut self, v: bool) -> Self {
        self.is_icon_only = v;
        self
    }

    /// Joins this toggle to a group edge. Internal: a caller reaches it by
    /// putting the toggle in a [`ToggleButtonGroup`].
    pub(crate) fn group_edge(mut self, edge: crate::button::GroupEdge, vertical: bool) -> Self {
        self.group_edge = Some((edge, vertical));
        self
    }

    fn group_focus_handle(mut self, handle: gpui::FocusHandle) -> Self {
        self.group_focus_handle = Some(handle);
        self
    }

    fn group_managed(mut self) -> Self {
        self.on_change = None;
        self
    }

    /// `useToggleButtonGroupItem` reads the group's selection mode, so the
    /// member has to be told which one it is in.
    fn group_selection_mode(mut self, mode: SelectionMode) -> Self {
        self.group_single_selection = mode == SelectionMode::Single;
        self
    }

    fn group_on_press(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        let child = self.on_press.take();
        self.on_change = None;
        self.on_press = Some(std::sync::Arc::new(move |event, window, cx| {
            handler(event, window, cx);
            if let Some(child) = &child {
                child(event, window, cx);
            }
        }));
        self
    }

    /// Sets whether the button is disabled; takes precedence over the group's disabled state.
    pub fn is_disabled(mut self, v: bool) -> Self {
        self.is_disabled = v;
        self.disabled_explicit = true;
        self
    }

    fn group_disabled(mut self, v: bool) -> Self {
        if !self.disabled_explicit {
            self.is_disabled = v;
        }
        self
    }

    /// Appends a child element.
    pub fn child(mut self, el: impl IntoElement) -> Self {
        self.children.push(el.into_any_element());
        self
    }

    /// `onChange` — reports the selection the press moves to.
    ///
    /// Fires alongside [`ToggleButton::on_press`]; use whichever shape suits.
    pub fn on_change(mut self, f: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(std::sync::Arc::new(f));
        self
    }

    /// Sets the handler called when the button is pressed.
    pub fn on_press(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_press = Some(std::sync::Arc::new(f));
        self
    }
}

impl ParentElement for ToggleButton {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for ToggleButton {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // `controlled` takes `cx` mutably, so it precedes the theme tokens.
        let (is_selected, own) = crate::util::controlled(
            window,
            cx,
            element_id::scoped(&self.id, "selected"),
            self.is_selected,
            self.default_selected,
        );

        // `.toggle-button:focus-visible` is `status-focused`.
        let focus_handle = self.group_focus_handle.clone().unwrap_or_else(|| {
            crate::util::tab_stop_handle(element_id::scoped(&self.id, "focus"), window, cx)
        });
        // One keyed slot owns hover and press state for both render-prop and
        // plain toggles, like Button: the hover fade reads the hover and
        // press bits it records — including the keyboard press — and the
        // press ramp reads the same slot instead of keying a second one.
        let interaction = Some(crate::util::interaction(
            element_id::scoped(&self.id, "interaction"),
            window,
            cx,
        ));
        let colors = cx.colors().clone();
        let sem = colors.accent;
        let disabled_opacity = cx.layout().disabled_opacity;
        let radius = self
            .radius
            .unwrap_or_else(|| crate::util::control_radius(cx));
        let is_grouped = self.group_edge.is_some();

        let sx_background = crate::util::sx_background(&self.sx);
        let sx_corners = crate::util::sx_radius(&self.sx);
        let fade = (!self.is_disabled)
            .then(|| {
                let idle = if is_selected {
                    sem.soft()
                } else {
                    match self.variant {
                        ToggleVariant::Default => colors.default.color,
                        ToggleVariant::Ghost => gpui::transparent_black(),
                    }
                };
                let hover = if is_selected {
                    colors.accent.soft_hover()
                } else {
                    match self.variant {
                        ToggleVariant::Default => colors.default.hover(),
                        ToggleVariant::Ghost => colors.default.color,
                    }
                };
                (idle, hover)
            })
            .and_then(|pair| crate::util::fade_endpoints(Some(pair), sx_background, self.hover_bg));

        // `useToggleButton` is `useButton` plus `aria-pressed`. Inside a
        // single-selection group, `useToggleButtonGroupItem` overwrites both:
        // `role = 'radio'`, `aria-checked`, and `delete aria-pressed`.
        let single = self.group_single_selection;
        let name = a11y::Name::maybe(self.aria_label.clone().or(self.label.clone()));
        let mut el = div()
            .id(self.id.clone())
            .flex()
            .items_center()
            .justify_center()
            // `.toggle-button` is not `overflow-hidden` -- `Button`, whose box
            // this one mirrors, is not either. The clip that used to be here
            // only kept the hover fade's fill inside the rounded box on the
            // patched renderer; the fill carries the same `group_radius_any`
            // corners as the box now, and dropping the clip is what lets the
            // focus ring below be the overlay, which hangs outside the box.
            .whitespace_nowrap()
            .flex_shrink_0()
            .font_weight(gpui::FontWeight::MEDIUM)
            .when(is_selected, |e| {
                let e = if fade.is_none() { e.bg(sem.soft()) } else { e };
                e.text_color(sem.soft_foreground(colors.foreground))
            })
            .when(!is_selected, |e| match self.variant {
                ToggleVariant::Default => {
                    let e = if fade.is_none() {
                        e.bg(colors.default.color)
                    } else {
                        e
                    };
                    e.text_color(colors.foreground)
                }
                ToggleVariant::Ghost => {
                    let e = if fade.is_none() {
                        e.bg(gpui::transparent_black())
                    } else {
                        e
                    };
                    e.text_color(colors.default.foreground)
                }
            });

        // sizing — kept in locals so the press geometry below scales exactly
        // what was applied here.
        // `.toggle-button` is `h-10 md:h-9` with `--sm` at `h-9 md:h-8` and
        // `--lg` at `h-11 md:h-10`: 32 / 36 / 40 on a desktop, the same pair as
        // `.button`. This had them a step too tall.
        let (height, pad_x, gap, press_scale) = match self.size {
            Size::Sm => (px(32.), px(12.), px(8.), crate::anim::PRESSED_SCALE_SUBTLE),
            Size::Md => (px(36.), px(16.), px(8.), crate::anim::PRESSED_SCALE),
            Size::Lg => (px(40.), px(16.), px(8.), crate::anim::PRESSED_SCALE_FIRM),
        };
        let (text, line) = match self.size {
            Size::Sm | Size::Md => (px(14.), px(20.)),
            Size::Lg => (px(16.), px(24.)),
        };
        el = el.h(height).text_size(text).line_height(line);
        el = if self.is_icon_only {
            el.w(height)
        } else {
            el.px(pad_x).gap(gap)
        };

        el = crate::button::group_radius_any(el, self.group_edge, radius);
        el = crate::util::round_sx_corners(el, &sx_corners);

        // The pressed endpoint both press paths resolve: the hover colour —
        // v3's `:active` fill on a toggle is the same colour `:hover` paints.
        // The hover fade eases to it while the interaction slot's pressed
        // bit is set; the reduced-motion and no-fade paths paint it outright.
        let hover_bg = if is_selected {
            colors.accent.soft_hover()
        } else {
            match self.variant {
                ToggleVariant::Default => colors.default.hover(),
                ToggleVariant::Ghost => colors.default.color,
            }
        };
        if let Some(colors) = fade {
            let edge = self.group_edge;
            el = crate::anim::hover_fade_with_press(
                el,
                element_id::scoped(&self.id, "fade"),
                colors,
                // v3's `:active` fill is the `:hover` colour — the fade's own
                // resolved endpoint, so a `hover_bg` override presses the
                // colour hover actually paints.
                colors.1,
                interaction
                    .as_ref()
                    .expect("a toggle always creates its interaction slot"),
                move |fill| {
                    crate::util::round_sx_corners(
                        crate::button::group_radius_any(fill, edge, radius),
                        &sx_corners,
                    )
                },
                window,
                cx,
            );
        }

        if self.is_disabled {
            el = el.opacity(disabled_opacity);
        }

        if let Some(render) = self.content.clone() {
            let (is_hovered, is_pressed) = interaction
                .as_ref()
                .map(|slot| *slot.read(cx))
                .unwrap_or_default();
            let focused = focus_handle.is_focused(window);
            el = el.child(render(crate::util::InteractiveState {
                is_hovered,
                is_pressed,
                is_focused: focused,
                is_focus_visible: focused && crate::util::focus_visible(cx),
                is_selected,
                is_disabled: self.is_disabled,
                is_pending: false,
                is_indeterminate: false,
            }));
        } else if let Some(label) = self.label {
            el = el.child(label.to_string());
        }
        // The interaction tracking belongs on the press slot, like Button:
        // key events dispatch along the focus path, which runs through the
        // slot the focus handle below tracks.
        el = el.children(self.children);

        let (skin_style, slot_style) =
            crate::util::split_press_style(&self.sx.clone().unwrap_or_default());
        el.style().refine(&skin_style);
        let sx_size = crate::util::sx_pixel_size(&self.sx);
        // The press wrap (or the disabled dimming, which must cover the label
        // above the skin too) comes after every visual child: children added
        // after `pressed` land on the slot and fight the skin for width.
        if !self.is_disabled {
            el = crate::util::cursor_interactive(el, cx);
            if fade.is_none() {
                el = el.hover(move |s| s.bg(hover_bg));
            }
            // v3 documents ToggleButton's pressed state as including the same
            // size-specific scale. Group members suppress it so the attached
            // control never opens gaps between buttons while pressed.
            //
            // Standalone, the press rides `pressed_with_background_ramp`:
            // `toggle-button.css` declares the press as a transition
            // (`transform 250ms var(--ease-smooth), background-color 100ms
            // var(--ease-out)`). The hover fade owns the colour whenever it
            // is mounted — its pressed endpoint is the same `hover_bg` — so
            // the ramp keeps only the transform track, the shape CloseButton's
            // press takes; the grouped `.active` fill applies only where no
            // fade owns the colour.
            if is_grouped {
                el = el.when(fade.is_none(), |e| {
                    e.active(move |style| style.bg(hover_bg))
                });
            } else {
                el = crate::anim::pressed_with_background_ramp(
                    el,
                    crate::anim::PressBox {
                        height: sx_size.height.unwrap_or(height),
                        padding_x: (!self.is_icon_only).then_some(pad_x),
                        width: sx_size
                            .width
                            .or_else(|| self.is_icon_only.then_some(height)),
                        min_width: None,
                        text_size: text,
                        line_height: line,
                        gap,
                        radius,
                        shrink_x: true,
                        scale: press_scale,
                    },
                    None,
                    crate::anim::BUTTON_PRESS,
                    interaction.as_ref(),
                    window,
                    cx,
                );
            }
        }

        el = el.map(|e| {
            if single {
                e.a11y_named(a11y::Role::RadioButton, &name)
                    .a11y_checked(is_selected, false)
            } else {
                e.a11y_named(a11y::Role::Button, &name)
                    .a11y_pressed(is_selected)
            }
        });
        el = el.key_context("Button");

        // A disabled toggle mounts no fade for the slot to feed, so it
        // tracks nothing — same gate Button's `interactive` applies.
        if !self.is_disabled {
            if let Some(slot) = &interaction {
                el = crate::util::track_interaction(el, slot);
            }
        }

        if !self.is_disabled
            && (self.on_press.is_some() || self.on_change.is_some() || own.is_some())
        {
            let on_press = self.on_press;
            let on_change = self.on_change;
            let next = !is_selected;
            el = el.on_click(move |ev, w, cx| {
                // Uncontrolled: flip our own copy, or a standalone toggle could
                // never change.
                if let Some(held) = &own {
                    held.update(cx, |v, cx| {
                        *v = next;
                        cx.notify();
                    });
                }
                if let Some(cb) = &on_change {
                    cb(&next, w, cx);
                }
                if let Some(cb) = &on_press {
                    cb(ev, w, cx);
                }
            });
        }

        if self.is_disabled {
            el.style().refine(&slot_style);
            return el;
        }
        // The ring is the overlay form wherever the four corners resolve to
        // one radius, as `Button` does it: an overlay is crisp and concentric
        // where a spread shadow keeps the element's own corner. A grouped
        // `Start` or `End` member rounds only its outer edge and an `sx`
        // refinement can break the symmetry of any member, and the overlay's
        // outer band is built from a scalar, so those keep the shadow ring,
        // which dilates whatever per-corner shape the element already has.
        let el = match crate::button::uniform_ring_radius(self.group_edge, radius, &sx_corners) {
            Some(ring_radius) => crate::util::ring_overlay_if_focused(
                el.track_focus(&focus_handle),
                &focus_handle,
                !is_grouped,
                ring_radius,
                Vec::new(),
                window,
                cx,
            ),
            None => crate::util::ring_if_focused(
                el.track_focus(&focus_handle),
                &focus_handle,
                !is_grouped,
                Vec::new(),
                window,
                cx,
            ),
        };
        let mut el = el;
        el.style().refine(&slot_style);
        crate::util::record_focus_bounds(el, &focus_handle, window, cx)
    }
}

// ---------------------------------------------------------------------------
// ToggleButtonGroup
// ---------------------------------------------------------------------------

/// A group of toggle buttons that share selection.
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct ToggleButtonGroup {
    id: ElementId,
    selected: Vec<SharedString>,
    default_selected: Vec<SharedString>,
    /// `Vec` alone cannot distinguish controlled empty from uncontrolled.
    is_controlled: bool,
    selection_mode: SelectionMode,
    size: Size,
    is_disabled: bool,
    is_detached: bool,
    /// Whether a `ToggleButtonGroup.Separator` sits before each member after
    /// the first. v3 composes it as a child, and hides it when detached.
    separators: bool,
    is_vertical: bool,
    disallow_empty_selection: bool,
    full_width: bool,
    children: Vec<ToggleButton>,
    on_change: Option<std::sync::Arc<dyn Fn(&[SharedString], &mut Window, &mut App) + 'static>>,
    /// The `sx` slot, refined over the root style at the end of render.
    sx: Option<Box<gpui::StyleRefinement>>,
}

impl ToggleButtonGroup {
    /// `orientation` — lays the group out along the given axis.
    pub fn orientation(mut self, orientation: SelectionOrientation) -> Self {
        self.is_vertical = orientation == SelectionOrientation::Vertical;
        self
    }

    /// `disallowEmptySelection` — keeps at least one member selected.
    pub fn disallow_empty_selection(mut self, v: bool) -> Self {
        self.disallow_empty_selection = v;
        self
    }

    /// `onSelectionChange` — the v3 name for [`ToggleButtonGroup::on_change`].
    pub fn on_selection_change(
        self,
        handler: impl Fn(&[SharedString], &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change(handler)
    }

    /// Creates an empty group with the given element id.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            selected: Vec::new(),
            default_selected: Vec::new(),
            is_controlled: false,
            selection_mode: SelectionMode::Single,
            size: Size::Md,
            is_disabled: false,
            is_detached: false,
            separators: false,
            is_vertical: false,
            disallow_empty_selection: false,
            full_width: false,
            children: Vec::new(),
            on_change: None,
            sx: None,
        }
    }

    /// Sets whether one or several buttons can be selected.
    pub fn selection_mode(mut self, m: SelectionMode) -> Self {
        self.selection_mode = m;
        self
    }

    /// `size` — inherited by children that do not set their own size.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// `isDisabled` — disables every child, matching React Aria's group state.
    pub fn is_disabled(mut self, v: bool) -> Self {
        self.is_disabled = v;
        self
    }

    /// `ToggleButtonGroup.Separator` — the hairline between members.
    pub fn separators(mut self, v: bool) -> Self {
        self.separators = v;
        self
    }

    /// Sets whether members are spaced apart instead of joined (detached members have a 4px gap and no separators).
    pub fn is_detached(mut self, v: bool) -> Self {
        self.is_detached = v;
        self
    }

    /// Sets whether the group fills the width of its parent.
    pub fn full_width(mut self, v: bool) -> Self {
        self.full_width = v;
        self
    }

    /// The one slot for caller-owned low-level styling: GPUI's styling methods
    /// (`bg`, `text_color`, `w`, `h`, `p`, `rounded`, `border_color`, …)
    /// applied to the group's root element after every value the orientation and
    /// the active theme chose, so they win. It restyles the row the members sit
    /// in; each member keeps its own `sx` slot.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        crate::util::refine_sx(&mut self.sx, style);
        self
    }

    /// Sets the selected keys (controlled).
    pub fn selected_keys(
        mut self,
        keys: impl IntoIterator<Item = impl Into<SharedString>>,
    ) -> Self {
        self.selected = keys.into_iter().map(Into::into).collect();
        self.is_controlled = true;
        self
    }

    /// `defaultSelectedKeys` — seeds the group's own selection state.
    pub fn default_selected_keys(
        mut self,
        keys: impl IntoIterator<Item = impl Into<SharedString>>,
    ) -> Self {
        self.default_selected = keys.into_iter().map(Into::into).collect();
        self
    }

    /// Sets the handler called with the selected keys when the selection changes.
    pub fn on_change(
        mut self,
        f: impl Fn(&[SharedString], &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(std::sync::Arc::new(f));
        self
    }

    /// Appends a toggle button to the group.
    pub fn child_toggle(mut self, btn: ToggleButton) -> Self {
        self.children.push(btn);
        self
    }
}

impl RenderOnce for ToggleButtonGroup {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let (selected, selection_own) = crate::util::controlled(
            window,
            cx,
            element_id::scoped(&self.id, "selected"),
            self.is_controlled.then_some(self.selected),
            self.default_selected,
        );
        // `.toggle-button-group` is `inline-flex items-center justify-center
        // gap-0`; `--detached` is `gap-1` and restores each member's full
        // radius.
        let gap = if self.is_detached { px(4.) } else { px(0.) };
        // `useToggleButtonGroup` starts from `useToolbar` — whose role is
        // `toolbar` and which always reports `aria-orientation` — and replaces
        // the role with `radiogroup` when the group selects one member.
        let orientation = if self.is_vertical {
            SelectionOrientation::Vertical
        } else {
            SelectionOrientation::Horizontal
        };
        let group_role = if self.selection_mode == SelectionMode::Single {
            a11y::Role::RadioGroup
        } else {
            a11y::Role::Toolbar
        };
        let mut row = div()
            .id(self.id.clone())
            .a11y(group_role)
            .a11y_orientation(orientation)
            .flex()
            .items_center()
            .justify_center()
            .when(self.is_vertical, |r| r.flex_col())
            .when(!self.is_vertical, |r| r.flex_row())
            .gap(gap)
            .when(self.full_width, |r| r.w_full());
        let total = self.children.len();
        let separators = self.separators && !self.is_detached;
        let is_vertical = self.is_vertical;
        let full_width = self.full_width;
        // `.toggle-button-group__separator` is `bg-current opacity-15`, 1px by
        // half the member, one pixel before its leading edge. This used to be a
        // 20px line in `--separator` with a 2px margin, which is neither the
        // colour nor the geometry v3 draws.
        let separator_color = cx.colors().foreground.alpha(0.15);
        let separator_radius = crate::util::hairline_radius(cx);

        let mode = self.selection_mode;
        let disallow_empty = self.disallow_empty_selection;

        let mut children = self
            .children
            .into_iter()
            .map(|button| {
                button
                    .group_managed()
                    .group_selection_mode(mode)
                    .group_disabled(self.is_disabled)
                    .group_size(self.size)
            })
            .collect::<Vec<_>>();
        let members = children
            .iter()
            .filter(|button| !button.is_disabled)
            .map(|button| {
                (
                    button.selection_key(),
                    crate::util::tab_stop_handle(
                        element_id::scoped(
                            &element_id::scoped(
                                &element_id::scoped(&self.id, "member"),
                                format!("{:?}", button.id),
                            ),
                            "focus",
                        ),
                        window,
                        cx,
                    ),
                )
            })
            .collect::<Vec<_>>();
        let focus_state =
            window.use_keyed_state(element_id::scoped(&self.id, "focus-state"), cx, |_, _| {
                ToggleGroupFocusState::default()
            });
        let current = members
            .iter()
            .position(|(_, handle)| handle.is_focused(window));
        let snapshot = focus_state.read(cx).clone();
        if let Some(current) = current {
            let mut effective = current;
            if !snapshot.was_inside && snapshot.restore_on_entry {
                if let Some(last) = &snapshot.last_key {
                    if let Some(restored) = members.iter().position(|(key, _)| key == last) {
                        effective = restored;
                        if restored != current {
                            window.focus(&members[restored].1, cx);
                        }
                    }
                }
            }
            let key = members[effective].0.clone();
            focus_state.update(cx, |state, _| {
                state.last_key = Some(key);
                state.was_inside = true;
                state.restore_on_entry = false;
                state.edge_exit = false;
            });
        } else if snapshot.was_inside {
            focus_state.update(cx, |state, _| {
                state.was_inside = false;
                state.restore_on_entry = !state.edge_exit;
                state.edge_exit = false;
            });
        } else if snapshot.edge_exit {
            focus_state.update(cx, |state, _| state.edge_exit = false);
        }

        let vertical = self.is_vertical;
        let key_focuses = members
            .iter()
            .map(|(_, handle)| handle.clone())
            .collect::<Vec<_>>();
        let key_focus_state = focus_state.clone();
        row = row.on_key_down(move |event: &gpui::KeyDownEvent, window, cx| {
            let movement = match (vertical, event.keystroke.key.as_str()) {
                (false, "right") | (true, "down") => Some("next"),
                (false, "left") | (true, "up") => Some("prev"),
                _ => None,
            };

            if let Some(movement) = movement {
                // Pinned `useToolbar` omits the nested group's handler when an
                // ancestor toolbar owns the arrows. GPUI bubbles key handlers,
                // so mark the exit and let that ancestor handle this key.
                if window
                    .context_stack()
                    .iter()
                    .any(|context| context.contains("Toolbar"))
                {
                    key_focus_state.update(cx, |state, _| state.edge_exit = true);
                    window.refresh();
                    return;
                }
                // Cross-axis arrows return above unconsumed. The group owns
                // every key on its own axis, including one that cannot move at
                // an edge, matching the pinned `useToolbar` handler.
                cx.stop_propagation();
                let Some(index) = key_focuses
                    .iter()
                    .position(|handle| handle.is_focused(window))
                else {
                    return;
                };
                let next = if movement == "next" {
                    index
                        .checked_add(1)
                        .filter(|next| *next < key_focuses.len())
                } else {
                    index.checked_sub(1)
                };
                if let Some(next) = next {
                    window.focus(&key_focuses[next], cx);
                }
                return;
            }

            if matches!(
                event.keystroke.key.as_str(),
                "up" | "down" | "left" | "right"
            ) {
                // A cross-axis arrow may belong to an enclosing toolbar. If it
                // moves focus out, do not restore the inner group's last item.
                key_focus_state.update(cx, |state, _| state.edge_exit = true);
                window.refresh();
                return;
            }

            if event.keystroke.key != "tab" {
                return;
            }
            // `useToolbar` moves to the edge and deliberately leaves Tab
            // unconsumed. The app root then performs its ordinary one step,
            // which exits the whole group instead of walking another member.
            let edge = if event.keystroke.modifiers.shift {
                key_focuses.first()
            } else {
                key_focuses.last()
            };
            if let Some(edge) = edge {
                window.focus(edge, cx);
            }
        });

        let mut member_focuses = members.into_iter().map(|(_, handle)| handle);
        for (i, btn) in children.drain(..).enumerate() {
            // Reflect the group's selection into the child, and let the child
            // report the next selection back through the group's callback.
            let key = btn.selection_key();
            let is_selected = selected.iter().any(|k| k == &key);
            let mut btn = if btn.is_disabled {
                btn.is_selected(is_selected)
            } else {
                btn.group_focus_handle(
                    member_focuses
                        .next()
                        .expect("every enabled toggle has a group focus handle"),
                )
                .is_selected(is_selected)
            };

            if self.on_change.is_some() || selection_own.is_some() {
                let on_change = self.on_change.clone();
                let own = selection_own.clone();
                let current = selected.clone();
                let key = key.clone();
                btn = btn.group_on_press(move |_, window, cx| {
                    let next =
                        crate::selection::next_selection(&current, &key, mode, disallow_empty);
                    if let Some(held) = &own {
                        let held_next = next.clone();
                        held.update(cx, |value, cx| {
                            *value = held_next;
                            cx.notify();
                        });
                    }
                    if let Some(change) = &on_change {
                        change(&next, window, cx);
                    }
                });
            }

            // The edge decides which corners stay round, so it has to reach the
            // `ToggleButton` before it becomes an element.
            let edge = if self.is_detached || total <= 1 {
                crate::button::GroupEdge::Only
            } else if i == 0 {
                crate::button::GroupEdge::Start
            } else if i + 1 == total {
                crate::button::GroupEdge::End
            } else {
                crate::button::GroupEdge::Middle
            };
            let mut slot = div()
                .relative()
                .child(btn.group_edge(edge, is_vertical))
                .when(full_width, |sl| sl.flex_1());
            if separators && i > 0 {
                slot = slot.child(
                    div()
                        .absolute()
                        .bg(separator_color)
                        .rounded(separator_radius)
                        .map(|sep| {
                            if is_vertical {
                                sep.left(gpui::relative(0.25))
                                    .top(px(-1.))
                                    .w(gpui::relative(0.5))
                                    .h(px(1.))
                            } else {
                                sep.left(px(-1.))
                                    .top(gpui::relative(0.25))
                                    .w(px(1.))
                                    .h(gpui::relative(0.5))
                            }
                        }),
                );
            }
            row = row.child(slot);
        }

        row = crate::util::apply_sx(row, &self.sx);
        row
    }
}

/// Separator element for ToggleButtonGroup — visual divider.
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct ToggleSeparator;

impl RenderOnce for ToggleSeparator {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        div().w(px(1.)).h(px(20.)).bg(cx.colors().separator)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_key_defaults_to_the_element_id() {
        // A group namespaces its child ids, so selection has to fall back to
        // the id when no explicit key is given.
        let plain = ToggleButton::new("bold");
        assert_eq!(plain.selection_key().as_ref(), "bold");
        let keyed = ToggleButton::new("grp-bold").key("bold");
        assert_eq!(keyed.selection_key().as_ref(), "bold");
    }

    #[test]
    fn group_separators_are_explicit_composition() {
        assert!(
            !ToggleButtonGroup::new("plain").separators,
            "v3 groups without a Separator child must not synthesize dividers"
        );
        assert!(
            ToggleButtonGroup::new("separated")
                .separators(true)
                .separators
        );
    }
}

crate::util::impl_component_styled!(ToggleButton, ToggleButtonGroup);
