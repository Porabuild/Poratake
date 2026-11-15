//! Link — port of `@heroui/link` (v3).
//!
//! Mirrors the React API: `href`, `isDisabled`, `autoFocus`, `onPress`, and
//! the documented `render` function. Links draw with the `--link` token
//! (which defaults to `--foreground`) — v3 removed the `color` prop — and
//! `link.css` keeps the text colour fixed across every state: hover and press
//! change only the underline decoration (`decoration-muted/50`, then
//! `decoration-muted`), never the text. `href` opens through the OS handler
//! (`App::open_url`), so v3's anchor-only `target` / `rel` / `download` have
//! no meaning here and are not offered.

use gpui::{
    div, prelude::*, px, App, ClickEvent, ElementId, Hsla, InteractiveElement, IntoElement, Pixels,
    RenderOnce, SharedString, StyleRefinement, Styled, UnderlineStyle, Window,
};
use herogpui_core::element_id;
use herogpui_theme::ActiveTheme;

use crate::a11y::{self, A11y as _};

/// A press handler. `Arc` rather than `Box` because it is bound twice: the
/// pointer's `on_click` and the keyboard's Enter/Space both run it.
type OnPress = std::sync::Arc<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// v3's `render` — a function of the link's interactive render-props state.
type Render = std::sync::Arc<dyn Fn(crate::util::InteractiveState) -> gpui::AnyElement + 'static>;

/// The underline v3 turns on for hover and press: `decoration-[1.5px]`, with
/// only the decoration colour differing between the two states.
fn underline(color: Hsla) -> UnderlineStyle {
    UnderlineStyle {
        thickness: px(1.5),
        color: Some(color),
        wavy: false,
    }
}

/// The pinned `.link__icon` slot: a centered, muted 0.75em icon. The
/// childless default-arrow margin is deliberately not part of this wrapper.
fn icon_slot(
    icon: gpui::AnyElement,
    id: ElementId,
    is_focus_visible: bool,
    link_color: Hsla,
) -> gpui::AnyElement {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .size(px(12.))
        .flex_shrink_0()
        .text_color(link_color)
        .opacity(if is_focus_visible { 1.0 } else { 0.6 })
        .hover(|s| s.opacity(1.0))
        .active(|s| s.opacity(1.0))
        .child(icon)
        .into_any_element()
}

/// HeroUI Link.
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct Link {
    id: ElementId,
    label: Option<SharedString>,
    href: Option<String>,
    is_disabled: bool,
    /// `autoFocus` — take focus on the first render.
    auto_focus: bool,
    /// `Link.Icon` — the glyph v3 composes beside the label. `None` when the
    /// caller composes no icon at all.
    icon: Option<gpui::AnyElement>,
    /// Whether the icon comes first. v3 gets this from where `Link.Icon` sits
    /// among the link's children.
    icon_first: bool,
    /// `render` — draws the link's content in place of the label and icon,
    /// handed the interactive state v3 passes its render functions.
    render: Option<Render>,
    on_press: Option<OnPress>,
    /// The root box's corner radius, in place of the owning `small_radius`
    /// helper.
    radius: Option<Pixels>,
    /// The `sx` slot, refined over the root style at the end of render.
    sx: Option<Box<StyleRefinement>>,
}

impl Link {
    /// Creates a link with the given id.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            label: None,
            href: None,
            is_disabled: false,
            auto_focus: false,
            icon: None,
            icon_first: false,
            render: None,
            on_press: None,
            radius: None,
            sx: None,
        }
    }

    /// `Link.Icon` (`.link__icon`) with a caller-supplied child — the
    /// arbitrary-children path, never v3's childless `<Link.Icon />`: upstream
    /// derives `data-default-icon` from `!children`, and the pinned `ms-1
    /// pb-1.5` applies only to that built-in arrow, which this port does not
    /// draw. `.link` has no gap of its own, so a custom icon sits flush.
    pub fn icon(mut self, el: impl IntoElement) -> Self {
        self.icon = Some(el.into_any_element());
        self
    }

    /// Puts the icon before the label, which v3 does by ordering the children.
    pub fn icon_first(mut self, v: bool) -> Self {
        self.icon_first = v;
        self
    }

    /// Sets the text label.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Sets the link target.
    pub fn href(mut self, href: impl Into<String>) -> Self {
        self.href = Some(href.into());
        self
    }

    /// Sets whether the link is disabled.
    pub fn is_disabled(mut self, v: bool) -> Self {
        self.is_disabled = v;
        self
    }

    /// `autoFocus` — take focus on the first render.
    ///
    /// A link is not otherwise a focus target here, so this also makes it one.
    pub fn auto_focus(mut self, v: bool) -> Self {
        self.auto_focus = v;
        self
    }

    /// `render` — v3's render function receives the DOM props plus the link's
    /// interactive state and renders whatever element it returns. GPUI has no
    /// DOM props to spread onto a caller-built element, so the closure
    /// receives the interactive half alone
    /// (`{isHovered, isPressed, isFocused, isFocusVisible, isDisabled}`) and
    /// draws the content; the root keeps the `href`, `onPress`, focus, and
    /// disabled wiring either way.
    pub fn render(
        mut self,
        render: impl Fn(crate::util::InteractiveState) -> gpui::AnyElement + 'static,
    ) -> Self {
        self.render = Some(std::sync::Arc::new(render));
        self
    }

    /// `onPress` — extra behaviour, in addition to opening `href`.
    pub fn on_press(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_press = Some(std::sync::Arc::new(handler));
        self
    }

    /// The root box's corner radius, in place of the owning `small_radius`
    /// helper (`.link` is `rounded-xl`). The link paints no fill of its own,
    /// so the corner shows only against a caller-supplied background or its
    /// focus ring. Not a v3 prop; the removed v2 `radius` prop is prohibited
    /// and this is a per-component repository extension.
    pub fn radius(mut self, radius: impl Into<Pixels>) -> Self {
        self.radius = Some(radius.into());
        self
    }

    /// The one slot for caller-owned low-level styling: GPUI's styling methods
    /// (`bg`, `text_color`, `w`, `h`, `p`, `rounded`, `border_color`, …)
    /// applied to the link's root element after every value the states and the
    /// active theme chose, so they win.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        crate::util::refine_sx(&mut self.sx, style);
        self
    }
}

impl RenderOnce for Link {
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // `.link:focus-visible` is `status-focused`, and `track_focus` is what
        // puts the link in the tab order. A disabled link must leave that order
        // like every other disabled control in this port — `track_focus` gates
        // on interactivity, and so does the ring — which is what
        // `pointer-events-none` with nothing to move to amounts to here.
        let interactive = !self.is_disabled;
        // `focus_once` takes `cx` mutably, so it runs before the tokens.
        let focus =
            crate::util::tab_stop_handle(element_id::scoped(&self.id, "link-focus"), window, cx);
        // `autoFocus` needs a focus target, and a link is only one while it is
        // interactive: a disabled link is skipped by Tab, so it must not grab
        // the focus on its first frame either.
        if self.auto_focus && interactive {
            crate::util::focus_once(
                window,
                cx,
                element_id::scoped(&self.id, "link-autofocus"),
                &focus,
            );
        }

        // Tokens are `Copy`, so take them before the `cx`-mutating state calls
        // below.
        let link_color = cx.colors().link;
        let disabled_opacity = cx.layout().disabled_opacity;
        let hover_decoration = cx.colors().muted.alpha(0.5);
        let pressed_decoration = cx.colors().muted;

        // One hover/press slot per link, for a `render` closure. The tracking
        // handlers cost a listener and a frame of lag; without the closure
        // nothing observes the states, so nothing tracks them.
        let interaction = if self.render.is_some() {
            Some(crate::util::interaction(
                element_id::scoped(&self.id, "link-interaction"),
                window,
                cx,
            ))
        } else {
            None
        };
        if let Some(slot) = &interaction {
            if !interactive && *slot.read(cx) != (false, false) {
                slot.update(cx, |state, _| *state = (false, false));
            }
        }

        // The resting corner is resolved once: the box rounds to it and the
        // focus ring overlay has to be concentric with that same shape.
        let link_radius = self.radius.unwrap_or_else(|| crate::util::small_radius(cx));

        // RAC's `Link` renders a native `<a>`, whose role is `link`;
        // `useLink` only adds the explicit role when the element is not an
        // anchor. The rendered text carries no element id and so contributes
        // no name, which is why the label is restated on the node.
        let mut el = div()
            .id(self.id.clone())
            .a11y_named(a11y::Role::Link, &a11y::Name::maybe(self.label.clone()))
            .flex()
            .items_center()
            .w_auto()
            // `.link` is `font-medium text-link` — the docs' Global CSS
            // snippet still says `font-semibold`, but the stylesheet is the
            // contract, and the text colour never changes state.
            .text_color(link_color)
            .font_weight(gpui::FontWeight::MEDIUM)
            .rounded(link_radius);
        if interactive {
            el = crate::util::ring_overlay_if_focused(
                el.track_focus(&focus),
                &focus,
                true,
                link_radius,
                Vec::new(),
                window,
                cx,
            );
        }

        if self.is_disabled {
            // `.link[aria-disabled="true"]` is `status-disabled`: the disabled
            // opacity, no pointer reach, and no tab stop.
            el = el.opacity(disabled_opacity);
        } else {
            // `&:hover` draws `underline decoration-muted/50` and `&:active`
            // takes the decoration to full `decoration-muted`; neither touches
            // the text colour. gpui panics on a second `hover` call, so each
            // closure owns its state's whole underline.
            el = el
                .cursor(crate::util::interactive_cursor(cx))
                .hover(move |mut s: StyleRefinement| {
                    s.text_style().underline = Some(underline(hover_decoration));
                    s
                })
                .active(move |mut s: StyleRefinement| {
                    s.text_style().underline = Some(underline(pressed_decoration));
                    s
                });
        }

        // v3 orders `Link.Icon` among the children, so the icon can lead or
        // trail the label. A `render` closure replaces that content and is
        // handed the state the slot tracked one frame ago.
        let icon_focus_visible =
            interactive && focus.is_focused(window) && crate::util::focus_visible(cx);
        if let Some(render) = &self.render {
            let (is_hovered, is_pressed) = interaction
                .as_ref()
                .map_or((false, false), |slot| *slot.read(cx));
            let focused = interactive && focus.is_focused(window);
            let state = crate::util::InteractiveState {
                is_hovered,
                is_pressed,
                is_focused: focused,
                is_focus_visible: focused && crate::util::focus_visible(cx),
                is_disabled: self.is_disabled,
                ..Default::default()
            };
            el = el.child(render(state));
        } else {
            // `.link` has no gap, and the pinned `ms-1 pb-1.5` belongs only
            // to `[data-default-icon="true"]` — the built-in arrow drawn by a
            // childless `<Link.Icon />`, which this port does not render. A
            // caller icon sits flush against the label on either side.
            if self.icon_first {
                if let Some(icon) = self.icon.take() {
                    el = el.child(icon_slot(
                        icon,
                        element_id::scoped(&self.id, "link-icon"),
                        icon_focus_visible,
                        link_color,
                    ));
                }
            }
            if let Some(label) = self.label.clone() {
                el = el.child(label.to_string());
            }
            if !self.icon_first {
                if let Some(icon) = self.icon.take() {
                    el = el.child(icon_slot(
                        icon,
                        element_id::scoped(&self.id, "link-icon"),
                        icon_focus_visible,
                        link_color,
                    ));
                }
            }
        }
        if let Some(slot) = &interaction {
            el = crate::util::track_interaction(el, slot);
        }

        if !self.is_disabled {
            let href = self.href.clone();
            let user_click = self.on_press;
            el = el.on_click(move |ev: &ClickEvent, window, cx| {
                if let Some(f) = &user_click {
                    f(ev, window, cx);
                }
                if let Some(url) = &href {
                    cx.open_url(url);
                }
            });
        }

        el = crate::util::apply_sx(el, &self.sx);
        if interactive {
            el = crate::util::record_focus_bounds(el, &focus, window, cx);
        }
        el
    }
}

crate::util::impl_component_styled!(Link);
