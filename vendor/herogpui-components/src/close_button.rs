//! CloseButton — port of `@heroui/close-button`.
//!
//! A button for dismissing dialogs, modals and inline content. Mirrors the
//! React API: `variant`, `isDisabled`, `onPress` and a custom-icon slot that
//! replaces the default close glyph.

use gpui::{
    div, prelude::*, px, AnyElement, App, ClickEvent, ElementId, InteractiveElement, IntoElement,
    ParentElement, Pixels, RenderOnce, Styled, Window,
};
use herogpui_core::element_id;
use herogpui_theme::ActiveTheme;

use crate::a11y::{self, A11y as _};
use crate::icons;

/// Visual variant of a close button. React exposes a single `default` variant.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CloseButtonVariant {
    #[default]
    /// The default close button style.
    Default,
}

/// A press handler. `Arc` rather than `Box` because it is bound twice: the
/// pointer's `on_click` and the keyboard's Enter/Space both run it.
type OnPress = std::sync::Arc<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// HeroUI CloseButton.
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct CloseButton {
    id: ElementId,
    is_disabled: bool,
    aria_label: Option<gpui::SharedString>,
    /// Replaces the default close glyph (`children` in React).
    icon: Option<AnyElement>,
    /// v3's `children`-as-a-function: handed the interactive state and drawn in
    /// place of the default content.
    content: Option<std::sync::Arc<dyn Fn(crate::util::InteractiveState) -> AnyElement + 'static>>,

    on_press: Option<OnPress>,
    /// The `sx` slot, refined over the root style at the end of render.
    sx: Option<Box<gpui::StyleRefinement>>,
    /// Set by [`CloseButton::hover_bg`]: the fill the hover fade eases *to*.
    /// Additive — unset, the fade behaves exactly as it did before.
    hover_bg: Option<gpui::Hsla>,
    /// The corner radius, in place of the owning `small_radius` helper. The
    /// pressed box scales it along with the box.
    radius: Option<Pixels>,
}

impl CloseButton {
    /// Creates a close button with the given id.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            is_disabled: false,
            aria_label: None,
            icon: None,
            content: None,
            on_press: None,
            sx: None,
            hover_bg: None,
            radius: None,
        }
    }

    /// Accessible name, overriding the localized default close label.
    pub fn aria_label(mut self, label: impl Into<gpui::SharedString>) -> Self {
        self.aria_label = Some(label.into());
        self
    }

    /// The one slot for caller-owned low-level styling: GPUI's styling methods
    /// (`bg`, `text_color`, `w`, `h`, `p`, `rounded`, `border_color`, …)
    /// applied to the close button's root element after every value the active
    /// theme chose, so they win.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        crate::util::refine_sx(&mut self.sx, style);
        self
    }

    /// The fill the hover fade eases to, in place of `--default-hover`.
    ///
    /// The fade runs from the resting background — the `sx` background when one
    /// is set, the close button's own `--default` otherwise — to `color`, over
    /// the same `transition-colors` timing the stock button uses. v3 has no
    /// such prop; on the web this is `className="hover:bg-…"`.
    pub fn hover_bg(mut self, color: impl Into<gpui::Hsla>) -> Self {
        self.hover_bg = Some(color.into());
        self
    }

    /// Sets whether the button is disabled.
    pub fn is_disabled(mut self, v: bool) -> Self {
        self.is_disabled = v;
        self
    }

    /// The corner radius, in place of the owning `small_radius` helper. The
    /// box is the button's whole shape, so the pressed box scales the same
    /// value rather than snapping back to the helper. Not a v3 prop; the
    /// removed v2 `radius` prop is prohibited and this is a per-component
    /// repository extension.
    pub fn radius(mut self, radius: impl Into<Pixels>) -> Self {
        self.radius = Some(radius.into());
        self
    }

    /// Supplies a custom icon in place of the default close glyph.
    /// v3's render function for the button's children, handed `isHovered`,
    /// `isPressed`, `isFocused` and `isDisabled`. The hover and press are a
    /// frame behind the pointer, because gpui reports both to a handler.
    pub fn content(
        mut self,
        render: impl Fn(crate::util::InteractiveState) -> AnyElement + 'static,
    ) -> Self {
        self.content = Some(std::sync::Arc::new(render));
        self
    }

    /// Replaces the default close icon.
    pub fn icon(mut self, icon: impl IntoElement) -> Self {
        self.icon = Some(icon.into_any_element());
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

    pub(crate) fn then_press(
        mut self,
        callback: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        crate::util::chain_press(&mut self.on_press, callback);
        self
    }
}

impl RenderOnce for CloseButton {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // `.close-button:focus-visible` is `status-focused`. The handle has to be
        // read before the theme tokens: `use_keyed_state` takes `cx` mutably.
        let focus_handle =
            crate::util::tab_stop_handle(element_id::scoped(&self.id, "focus"), window, cx);
        let interaction = self.content.as_ref().map(|_| {
            crate::util::interaction(element_id::scoped(&self.id, "interaction"), window, cx)
        });
        if self.is_disabled {
            if let Some(slot) = &interaction {
                if *slot.read(cx) != (false, false) {
                    slot.update(cx, |state, _| *state = (false, false));
                }
            }
        }

        let colors = cx.colors().clone();
        // The box is the button's whole shape, so the press-scale derivation
        // below multiplies this resolved value rather than the helper's.
        let radius = self.radius.unwrap_or_else(|| crate::util::small_radius(cx));
        let sx_corners = crate::util::sx_radius(&self.sx);
        let disabled_opacity = cx.layout().disabled_opacity;
        // `.close-button` is `h-6 p-1` with a `size-4` glyph.
        let (box_size, icon_size) = (px(24.), px(16.));
        // The `sx` slot refines the root, so its background is the resting
        // value the fade must hold; an explicit `hover_bg` eases from it.
        let sx_background = crate::util::sx_background(&self.sx);
        let idle_bg = sx_background.unwrap_or(colors.default.color);
        let hover_end = self.hover_bg.unwrap_or(colors.default.hover());
        let fade = (!self.is_disabled)
            .then_some((colors.default.color, colors.default.hover()))
            .and_then(|pair| crate::util::fade_endpoints(Some(pair), sx_background, self.hover_bg));

        let name = a11y::Name::labelled(
            self.aria_label
                .clone()
                .unwrap_or_else(|| crate::i18n::ui_string(crate::i18n::UiString::Close, cx)),
        );
        let (skin_style, slot_style) =
            crate::util::split_press_style(&self.sx.clone().unwrap_or_default());
        let sx_size = crate::util::sx_pixel_size(&self.sx);
        let foreground = self
            .sx
            .as_ref()
            .and_then(|style| style.text.color)
            .unwrap_or(colors.muted);
        let mut el = div()
            .id(self.id.clone())
            // The icon has no text: use the explicit accessible name, then
            // the localized default, matching v3.2.7's label precedence.
            .debug_selector({
                let id = self.id.clone();
                move || format!("{id:?}")
            })
            .flex()
            .items_center()
            .justify_center()
            .flex_shrink_0()
            .size(box_size)
            .p(px(4.))
            .rounded(radius)
            .map(|el| crate::util::round_sx_corners(el, &sx_corners))
            .when(fade.is_none(), |e| e.bg(idle_bg))
            .text_color(foreground);

        if let Some(fade_colors) = fade {
            el = crate::anim::hover_fade(
                el,
                element_id::scoped(&self.id, "fade"),
                fade_colors,
                interaction.as_ref(),
                None,
                move |fill| crate::util::round_sx_corners(fill.rounded(radius), &sx_corners),
                window,
                cx,
            );
        }

        if self.is_disabled {
            el = el.opacity(disabled_opacity);
        } else if fade.is_none() {
            el = el.hover(move |s| s.bg(hover_end));
        }

        el.style().refine(&skin_style);
        el = match (self.content.clone(), self.icon) {
            (Some(render), _) => {
                let (is_hovered, is_pressed) = interaction
                    .as_ref()
                    .map(|slot| *slot.read(cx))
                    .unwrap_or_default();
                let is_focused = !self.is_disabled && focus_handle.is_focused(window);
                el.child(render(crate::util::InteractiveState {
                    is_hovered,
                    is_pressed,
                    is_focused,
                    is_focus_visible: is_focused && crate::util::focus_visible(cx),
                    is_selected: false,
                    is_disabled: self.is_disabled,
                    is_pending: false,
                    is_indeterminate: false,
                }))
            }
            (None, Some(icon)) => el.child(icon),
            (None, None) => el.child(
                gpui::svg()
                    .debug_selector({
                        let id = self.id.clone();
                        move || format!("{id:?}-icon")
                    })
                    .size(icon_size)
                    .flex_shrink_0()
                    // `.close-button svg` is `-mx-0.5 my-0.5` (2px at the 16px
                    // root). Symmetric margins on a centred 16px child in the
                    // 16px `p-1` content box cancel, but they are representable
                    // on `svg()` through Styled and belong on the default glyph.
                    .mx(px(-2.))
                    .my(px(2.))
                    .path(icons::CLOSE)
                    .text_color(foreground),
            ),
        };
        if !self.is_disabled {
            // `.close-button--default:active, &[data-pressed="true"]` is
            // `transform: scale(0.93)` about `origin-center`, riding
            // `transition: transform 250ms var(--ease-out-quart)`. The pinned
            // sheet's pressed state changes no background, so this is the one
            // animated track. gpui has no div-level scale, so the press
            // shrinks the 24px skin about its centre inside a stable slot —
            // the same geometry `anim::pressed` uses — and the ramp eases it
            // the way the stylesheet's transition does. Reduced motion snaps
            // to the scaled endpoint, matching
            // `motion-reduce:transition-none`, which removes the timing but
            // keeps the transform. The wrap comes after every visual child:
            // children added after it land on the slot and fight the skin for
            // its width.
            const PRESS_SCALE: f32 = 0.93;
            let pressed_radius = px(f32::from(radius) * PRESS_SCALE);
            debug_assert!(pressed_radius <= radius);
            // Reapply the caller's corner refinement on the skin immediately
            // before the press wrapper. This keeps the painted skin, hover
            // fill and the press slot on the same resolved shape.
            el = crate::util::round_sx_corners(el, &sx_corners);
            el = crate::anim::pressed_with_background_ramp(
                el,
                crate::anim::PressBox {
                    height: sx_size.height.unwrap_or(box_size),
                    padding_x: None,
                    width: Some(sx_size.width.unwrap_or(box_size)),
                    min_width: None,
                    text_size: icon_size,
                    line_height: icon_size,
                    gap: px(0.),
                    radius,
                    shrink_x: true,
                    scale: PRESS_SCALE,
                },
                None,
                crate::anim::CLOSE_BUTTON_PRESS,
                interaction.as_ref(),
                window,
                cx,
            );
        }

        if !self.is_disabled {
            // The interaction tracking (hover, mouse/keyboard press) belongs
            // on the press slot, like Button: the handlers see the whole
            // resting footprint, and the skin below is its listener-free
            // visual child. The ramp wired its own tracking already unless a
            // `content` closure's slot exists.
            el = crate::util::cursor_interactive(el, cx);
            if let Some(slot) = &interaction {
                el = crate::util::track_interaction(el, slot);
            }
        }

        if let Some(on_press) = self.on_press {
            if !self.is_disabled {
                el = el.on_click(move |ev: &ClickEvent, window, cx| on_press(ev, window, cx));
            }
        }

        el = el
            .a11y_named(a11y::Role::Button, &name)
            .key_context("Button");
        el.style().refine(&slot_style);
        if self.is_disabled {
            return el;
        }
        // `.close-button:focus-visible` is `status-focused`: the ring and the
        // focus tracking land on the press slot — the element the ramp
        // returned — so keyboard activation and pointer activation answer on
        // the same element and the ring draws around the resting footprint.
        // The press slot is both the element the ramp rounded to `radius` and
        // the one that takes the focus, so the overlay goes there: it stays
        // concentric with the resting footprint while the skin inside it
        // scales under a press.
        let el = crate::util::ring_overlay_if_focused(
            el.track_focus(&focus_handle),
            &focus_handle,
            true,
            radius,
            Vec::new(),
            window,
            cx,
        );
        crate::util::record_focus_bounds(el, &focus_handle, window, cx)
    }
}

crate::util::impl_component_styled!(CloseButton);
