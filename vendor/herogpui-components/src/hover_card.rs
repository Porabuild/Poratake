//! `HoverCard` — a rich preview card opened by hovering or focusing a trigger
//! (HeroGPUI extension; HeroUI v3 has no hover card, and React Aria has no
//! hook for one).
//!
//! The behaviour follows Radix's `HoverCard` (`@radix-ui/react-hover-card`),
//! the reference gpui-kit's `hover_card.rs` also ports:
//!
//! - **Opening.** The pointer entering the trigger opens the card after
//!   `open_delay` (700ms, Radix's default); keyboard-visible focus inside the
//!   trigger opens it at once, the way the port's [`crate::Tooltip`] answers
//!   keyboard focus.
//! - **Staying open.** Leaving the trigger starts `close_delay` (300ms); the
//!   pointer reaching the card within that time cancels the close, so the card
//!   can be read and its links pressed. Leaving the card starts the same
//!   delay again.
//! - **Closing.** Escape (while the card is the topmost overlay, wherever the
//!   focus is), a press outside the card, or the keyboard focus leaving the
//!   trigger close it.
//!
//! The delay timers use the Tooltip's scheme: every hover transition bumps a
//! generation in keyed state, and a timer that wakes after a newer transition
//! was recorded is stale and does nothing, so a fast pass over a row of
//! triggers opens none of them. The timings are builders rather than theme
//! tokens: the Tooltip tokens (`tooltip_delay_ms`, 1500ms) are HeroUI's for a
//! tip, which is far slower than a card preview wants.
//!
//! **Accessibility.** The card reports `Role::Group`, named by [`HoverCard::label`]
//! when one is given. It is deliberately not a tooltip: a tooltip's content is
//! a plain-text description of its trigger, while a card holds structured and
//! interactive content. Radix renders the content as a plain element and
//! documents the card as a pointer enhancement; the trigger (the caller's
//! element) keeps its own role and name, so nothing the card shows should be
//! the only way to reach an action.
//!
//! ```
//! use gpui::{div, IntoElement, ParentElement};
//! use herogpui_components::HoverCard;
//!
//! let card = HoverCard::new("profile-card")
//!     .label("Profile of Jane Doe")
//!     .content(|_window, _cx| {
//!         div().child("Jane Doe").child("Design engineer").into_any_element()
//!     })
//!     .child(div().child("@jane"));
//! ```

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use gpui::{
    div, prelude::FluentBuilder as _, px, AnyElement, App, Bounds, ElementId, InteractiveElement,
    IntoElement, ParentElement, Pixels, RenderOnce, SharedString, StatefulInteractiveElement,
    Styled, Window,
};
use herogpui_core::{element_id, Placement};
use herogpui_theme::ActiveTheme;

use crate::a11y::{self, A11y as _};
use crate::{anim, util};

/// The default wait before a hovered trigger opens its card (Radix's
/// `openDelay`).
pub const HOVER_CARD_OPEN_DELAY_MS: u64 = 700;
/// The default wait before a card the pointer left closes (Radix's
/// `closeDelay`).
pub const HOVER_CARD_CLOSE_DELAY_MS: u64 = 300;

type OpenChange = Arc<dyn Fn(&bool, &mut Window, &mut App) + 'static>;
type Content = Rc<dyn Fn(&mut Window, &mut App) -> AnyElement + 'static>;

/// What survives between frames: which surfaces the pointer is over, and the
/// generation that stales pending timers.
#[derive(Default)]
struct HoverCardState {
    generation: u64,
    over_trigger: bool,
    over_card: bool,
    /// Whether the keyboard focus sat inside the trigger last frame.
    was_focused: bool,
}

/// A card that previews content while its trigger is hovered or focused. See
/// the [module docs](self).
#[must_use]
#[derive(IntoElement)]
pub struct HoverCard {
    id: ElementId,
    children: Vec<AnyElement>,
    content: Option<Content>,
    label: Option<SharedString>,
    placement: Placement,
    offset: Pixels,
    open_delay: u64,
    close_delay: u64,
    is_open: Option<bool>,
    default_open: bool,
    is_disabled: bool,
    width: Pixels,
    radius: Option<Pixels>,
    on_open_change: Option<OpenChange>,
    sx: Option<Box<gpui::StyleRefinement>>,
}

impl HoverCard {
    /// A hover card keyed by `id`; its children are the trigger. Give every
    /// instance its own id.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            children: Vec::new(),
            content: None,
            label: None,
            placement: Placement::Bottom,
            offset: px(8.),
            open_delay: HOVER_CARD_OPEN_DELAY_MS,
            close_delay: HOVER_CARD_CLOSE_DELAY_MS,
            is_open: None,
            default_open: false,
            is_disabled: false,
            width: px(256.),
            radius: None,
            on_open_change: None,
            sx: None,
        }
    }

    /// The card's body. The closure runs only while the card is on screen.
    pub fn content(
        mut self,
        render: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
    ) -> Self {
        self.content = Some(Rc::new(render));
        self
    }

    /// The card's accessible name.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Where the card sits relative to the trigger; `Bottom` by default. It
    /// flips when the viewport has no room, like a popover.
    pub fn placement(mut self, placement: Placement) -> Self {
        self.placement = placement;
        self
    }

    /// The gap between trigger and card; 8px by default.
    pub fn offset(mut self, offset: impl Into<Pixels>) -> Self {
        self.offset = offset.into();
        self
    }

    /// Milliseconds a hovered trigger waits before opening the card.
    pub fn open_delay(mut self, ms: u64) -> Self {
        self.open_delay = ms;
        self
    }

    /// Milliseconds the card waits, once the pointer left both the trigger
    /// and the card, before closing.
    pub fn close_delay(mut self, ms: u64) -> Self {
        self.close_delay = ms;
        self
    }

    /// The controlled open state. Hover, focus, Escape and outside presses
    /// then only report through [`HoverCard::on_open_change`].
    pub fn is_open(mut self, open: bool) -> Self {
        self.is_open = Some(open);
        self
    }

    /// The initial open state of an uncontrolled card.
    pub fn default_open(mut self, open: bool) -> Self {
        self.default_open = open;
        self
    }

    /// A disabled card never opens; the trigger renders unchanged.
    pub fn is_disabled(mut self, disabled: bool) -> Self {
        self.is_disabled = disabled;
        self
    }

    /// The card's width; 256px by default (Radix's `w-64` example).
    pub fn width(mut self, width: impl Into<Pixels>) -> Self {
        self.width = width.into();
        self
    }

    /// The card's corner radius, in place of the `container_radius` helper.
    pub fn radius(mut self, radius: impl Into<Pixels>) -> Self {
        self.radius = Some(radius.into());
        self
    }

    /// Reports every open or close the card asks for.
    pub fn on_open_change(mut self, f: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_open_change = Some(Arc::new(f));
        self
    }

    /// Caller-owned styling refined over the card panel after the theme's
    /// values.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        util::refine_sx(&mut self.sx, style);
        self
    }
}

impl ParentElement for HoverCard {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

/// The open-state writer every path shares.
#[derive(Clone)]
struct Opener {
    open: bool,
    own: Option<gpui::Entity<bool>>,
    on_open_change: Option<OpenChange>,
}

impl Opener {
    fn set(&self, open: bool, window: &mut Window, cx: &mut App) {
        let current = self.own.as_ref().map_or(self.open, |own| *own.read(cx));
        if current == open {
            return;
        }
        if let Some(own) = &self.own {
            own.update(cx, |value, cx| {
                *value = open;
                cx.notify();
            });
        }
        if let Some(cb) = &self.on_open_change {
            cb(&open, window, cx);
        }
    }
}

/// Starts a timer that sets the card to `open` after `ms`, unless a newer
/// hover transition was recorded meanwhile or, for a close, the pointer is
/// back over the trigger or the card.
fn schedule(
    state: &gpui::Entity<HoverCardState>,
    opener: &Opener,
    open: bool,
    ms: u64,
    window: &mut Window,
    cx: &mut App,
) {
    let generation = state.update(cx, |s, _| {
        s.generation += 1;
        s.generation
    });
    if ms == 0 {
        opener.set(open, window, cx);
        return;
    }
    let weak = state.downgrade();
    let opener = opener.clone();
    window
        .spawn(cx, async move |cx| {
            cx.background_executor()
                .timer(Duration::from_millis(ms))
                .await;
            let _ = cx.update(|window, cx| {
                let Some(state) = weak.upgrade() else {
                    return;
                };
                let s = state.read(cx);
                let hovered = s.over_trigger || s.over_card;
                if s.generation != generation || (!open && hovered) {
                    return;
                }
                opener.set(open, window, cx);
            });
        })
        .detach();
}

impl RenderOnce for HoverCard {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let base = self.id.clone();
        if self.is_disabled {
            return div()
                .id(base)
                .flex()
                .children(self.children)
                .into_any_element();
        }
        let state = window.use_keyed_state(element_id::scoped(&base, "hover"), cx, |_, _| {
            HoverCardState::default()
        });
        let (open, own) = util::controlled(
            window,
            cx,
            element_id::scoped(&base, "open"),
            self.is_open,
            self.default_open,
        );
        let opener = Opener {
            open,
            own,
            on_open_change: self.on_open_change.clone(),
        };
        let trigger_bounds = window
            .use_keyed_state(element_id::scoped(&base, "trigger-bounds"), cx, |_, _| {
                Rc::new(Cell::new(None::<Bounds<Pixels>>))
            })
            .read(cx)
            .clone();
        // Not a tab stop: the caller's trigger is the stop, and this handle
        // only answers whether the focus is inside it.
        let wrap_focus = window
            .use_keyed_state(element_id::scoped(&base, "wrap-focus"), cx, |_, cx| {
                cx.focus_handle()
            })
            .read(cx)
            .clone();

        // Keyboard focus opens at once and its departure closes, observed at
        // render time the way the Tooltip observes it: a focus move always
        // repaints the window.
        let focused = wrap_focus.contains_focused(window, cx);
        if focused != state.read(cx).was_focused {
            state.update(cx, |s, _| s.was_focused = focused);
            let keyboard = focused && util::focus_visible(cx);
            let opener = opener.clone();
            let state = state.clone();
            window.defer(cx, move |window, cx| {
                if keyboard {
                    schedule(&state, &opener, true, 0, window, cx);
                } else if !focused {
                    let hovered = {
                        let s = state.read(cx);
                        s.over_trigger || s.over_card
                    };
                    if !hovered {
                        schedule(&state, &opener, false, 0, window, cx);
                    }
                }
            });
        }

        let (phase, token) =
            util::overlay_scope(window, cx, element_id::scoped(&base, "phase"), open, true);
        let dismiss = util::shared({
            let opener = opener.clone();
            let state = state.clone();
            move |window: &mut Window, cx: &mut App| {
                state.update(cx, |s, _| s.generation += 1);
                opener.set(false, window, cx);
                util::DismissResult::Handled
            }
        });
        // The pointer may be over the card while the focus is anywhere, so
        // Escape is captured at the document level, as the Tooltip's is.
        {
            let dismiss = dismiss.clone();
            util::capture_escape(&token, move |window, cx| dismiss(window, cx), cx);
        }

        let probe = trigger_bounds.clone();
        let hover_state = state.clone();
        let hover_opener = opener.clone();
        let (open_delay, close_delay) = (self.open_delay, self.close_delay);
        let mut root = div()
            .id(base.clone())
            .track_focus(&wrap_focus)
            .relative()
            .flex()
            .children(self.children)
            .child(
                gpui::canvas(move |bounds, _, _| probe.set(Some(bounds)), |_, _, _, _| {})
                    .absolute()
                    .inset_0(),
            )
            .on_hover(move |over, window, cx| {
                let over = *over;
                hover_state.update(cx, |s, _| s.over_trigger = over);
                if over {
                    schedule(&hover_state, &hover_opener, true, open_delay, window, cx);
                } else {
                    schedule(&hover_state, &hover_opener, false, close_delay, window, cx);
                }
            });
        root = util::dismiss_on_escape_with_token(root, token.clone(), {
            let dismiss = dismiss.clone();
            move |window, cx| dismiss(window, cx)
        });

        if phase == util::OverlayPhase::Closed {
            return root.into_any_element();
        }

        let body = self.content.as_ref().map(|render| render(window, cx));
        let colors = cx.colors().clone();
        let layout = cx.layout().clone();
        let radius = self.radius.unwrap_or_else(|| util::container_radius(cx));
        let card_state = state.clone();
        let card_opener = opener;
        let card = div()
            .id(element_id::scoped(&base, "card"))
            .a11y_named(a11y::Role::Group, &a11y::Name::maybe(self.label.clone()))
            .relative()
            .flex()
            .flex_col()
            .gap(px(8.))
            .w(self.width)
            .p(px(16.))
            .bg(colors.overlay.background)
            .text_color(colors.surface.foreground)
            .text_size(px(14.))
            .line_height(px(20.))
            .rounded(radius)
            .when_some(layout.overlay_hairline, |el, hairline| {
                el.border(layout.border_width).border_color(hairline)
            })
            .shadow(layout.overlay_shadow)
            .occlude()
            .on_hover(move |over, window, cx| {
                let over = *over;
                card_state.update(cx, |s, _| s.over_card = over);
                if over {
                    // Cancels a close the trigger's departure scheduled.
                    card_state.update(cx, |s, _| s.generation += 1);
                } else {
                    schedule(&card_state, &card_opener, false, close_delay, window, cx);
                }
            })
            .debug_selector({
                let name = format!("{}-card", selector_base(&base));
                move || name
            })
            .children(body);
        let card = util::apply_sx(card, &self.sx);
        let card = util::dismiss_on_press_outside_with_token(card, token, move |window, cx| {
            dismiss(window, cx)
        });

        let (slide_x, slide_y) = crate::popover::placement_entry_offset(self.placement);
        let zoom = anim::ZoomBox::panel(px(16.), radius)
            .padding_x(px(16.))
            .sized(self.width);
        let zoom = anim::ZoomBox {
            slide_x: (slide_x != 0.0).then(|| px(slide_x)),
            slide_y: (slide_y != 0.0).then(|| px(slide_y)),
            ..zoom
        };
        let card = if phase == util::OverlayPhase::Exiting {
            anim::exiting(
                card,
                element_id::scoped(&base, "card-out"),
                zoom,
                anim::Motion::LIST_OUT,
                cx,
            )
        } else {
            anim::entering_zoom(
                card,
                element_id::scoped(&base, "card-in"),
                zoom,
                anim::Motion::POPOVER_IN,
                cx,
            )
        };
        root.child(util::floating(
            crate::popover::popover_with_resolved_placement(
                trigger_bounds,
                self.placement,
                self.offset,
                None,
                card,
            ),
        ))
        .into_any_element()
    }
}

/// The debug-selector spelling of an id: its name when it has one.
fn selector_base(id: &ElementId) -> String {
    match id {
        ElementId::Name(name) => name.to_string(),
        other => format!("{other:?}"),
    }
}

crate::util::impl_component_styled!(HoverCard);
