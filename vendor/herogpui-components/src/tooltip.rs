//! Tooltip — port of `@heroui/tooltip` (v3).
//!
//! The tip is state-driven rather than a pure hover style, because v3's `delay`
//! and `closeDelay` need to know *when* the hover began. State lives in a
//! per-tooltip [`Window::use_keyed_state`] entity, so callers still write a
//! plain builder with no entity to thread through.

use std::time::Duration;

use gpui::{
    prelude::*, px, AnyElement, App, ElementId, IntoElement, ParentElement, Pixels, RenderOnce,
    SharedString, StatefulInteractiveElement, Styled, Window,
};
use herogpui_core::{element_id, PlacementAlign};
use herogpui_theme::ActiveTheme;

use crate::{
    a11y::{self, A11y as _},
    anim, icons, util,
};

/// Where the tip sits relative to its trigger.
///
/// Shares the one placement vocabulary with the popovers and pickers — the
/// full 22-value React Aria union. The logical `start`/`end` aliases resolve
/// to the same pixels as their `left`/`right` spellings because this port has
/// no RTL mode.
pub use herogpui_core::Placement as TooltipPlacement;

/// The arrow points back at the trigger, so it faces opposite the tip.
fn arrow_rotation(placement: TooltipPlacement) -> f32 {
    if placement.is_above() {
        // The asset's apex is at the bottom, so it points down unrotated:
        // a tip above the trigger needs no rotation at all.
        0.
    } else if placement.is_side() {
        if placement.is_start_side() {
            -std::f32::consts::FRAC_PI_2
        } else {
            std::f32::consts::FRAC_PI_2
        }
    } else {
        std::f32::consts::PI
    }
}

/// HeroUI's `slide-in-from-*` entry offset for the physical side. Aligned
/// top/bottom placements share the same motion as their centered form.
fn entry_offset(placement: TooltipPlacement) -> (f32, f32) {
    if placement.is_above() {
        (0.0, 4.0)
    } else if placement.is_side() {
        if placement.is_start_side() {
            (4.0, 0.0)
        } else {
            (-4.0, 0.0)
        }
    } else {
        (0.0, -4.0)
    }
}

/// GPUI's pinned text wrapper breaks normal prose at spaces but has no
/// `overflow-wrap: anywhere` style. HeroUI applies that rule to tooltip text
/// so a long URL or token still fits the 320px cap. Zero-width break
/// opportunities preserve the visible and accessible text while allowing the
/// existing normal wrapper to split an unbroken token when it reaches the cap.
fn tooltip_text_with_break_opportunities(text: &str) -> String {
    let mut chars = text.chars().peekable();
    let mut result = String::with_capacity(text.len());
    while let Some(ch) = chars.next() {
        result.push(ch);
        if !ch.is_whitespace()
            && ch != '\u{200b}'
            && chars
                .peek()
                .is_some_and(|next| !next.is_whitespace() && *next != '\u{200b}')
        {
            result.push('\u{200b}');
        }
    }
    result
}

fn tooltip_display_content(text: &str, natural_width: Pixels) -> (String, bool) {
    let needs_breaks = natural_width > px(320.);
    let display = if needs_breaks {
        tooltip_text_with_break_opportunities(text)
    } else {
        text.to_owned()
    };
    (display, needs_breaks)
}

/// Hover state for one tooltip.
///
/// `generation` is bumped on every hover transition; a timer that fires after a
/// newer transition has been recorded is stale and must not flip the tip. That
/// is what keeps a fast pass over a row of triggers from opening all of them.
///
/// `focus_dismissed` is what Escape trips for a *focus-opened* tip. The focus
/// gate (`contains_focused && focus_visible`) is not something Escape may
/// clear — `focus_visible` is app-wide state every focus ring reads — so the
/// dismissal is remembered per tooltip instead, and dropped on either edge of
/// the focus session. A dismissal therefore lasts only for the current focus:
/// the next keyboard focus shows the tip again.
pub struct TooltipHover {
    open: bool,
    generation: u64,
    focus_dismissed: bool,
    focus_open: bool,
    was_focused: bool,
}

impl TooltipHover {
    fn new() -> Self {
        Self {
            open: false,
            generation: 0,
            focus_dismissed: false,
            focus_open: false,
            was_focused: false,
        }
    }

    /// A closed tip, for a caller that needs the same seed the component uses.
    ///
    /// The state lives in `Window::use_keyed_state` under the tooltip's id, and
    /// a test (or any caller that wants to read the flag) has to hand that call
    /// the identical initialiser or it seeds a different slot.
    pub fn closed() -> Self {
        Self::new()
    }

    /// Whether the tip is currently shown.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Whether keyboard-visible focus opened the tip in this focus session.
    pub fn is_focus_open(&self) -> bool {
        self.focus_open && !self.focus_dismissed
    }

    fn close(&mut self, dismiss_focus: bool) -> bool {
        self.generation += 1;
        let was_open = self.open || self.is_focus_open();
        self.open = false;
        if dismiss_focus {
            self.focus_dismissed = true;
        }
        was_open
    }
}

#[derive(Default)]
struct TooltipManager {
    entries: Vec<gpui::WeakEntity<TooltipHover>>,
    warmed_up: bool,
    cooldown_generation: u64,
}

impl gpui::Global for TooltipManager {}

fn ensure_tooltip_manager(cx: &mut App) {
    if cx.try_global::<TooltipManager>().is_none() {
        cx.set_global(TooltipManager::default());
    }
}

fn prepare_tooltip_open(current: &gpui::WeakEntity<TooltipHover>, cx: &mut App) -> bool {
    ensure_tooltip_manager(cx);
    let (warmed_up, others) = cx.update_global::<TooltipManager, _>(|manager, _| {
        manager.entries.retain(|entry| entry.upgrade().is_some());
        let others = manager
            .entries
            .iter()
            .filter(|entry| *entry != current)
            .filter_map(gpui::WeakEntity::upgrade)
            .collect::<Vec<_>>();
        manager.entries.retain(|entry| entry == current);
        if manager.entries.is_empty() {
            manager.entries.push(current.clone());
        }
        (manager.warmed_up, others)
    });
    // Entity updates run after the global borrow is released. `current` may
    // itself be mid-update when a hover timer calls this helper.
    for other in others {
        other.update(cx, |state, cx| {
            if state.close(true) {
                cx.notify();
            }
        });
    }
    warmed_up
}

fn mark_tooltip_open(cx: &mut App) {
    cx.update_global::<TooltipManager, _>(|manager, _| {
        manager.warmed_up = true;
        manager.cooldown_generation += 1;
    });
}

fn start_tooltip_cooldown(
    current: &gpui::WeakEntity<TooltipHover>,
    close_delay: u64,
    cx: &mut App,
) {
    ensure_tooltip_manager(cx);
    let generation = cx.update_global::<TooltipManager, _>(|manager, _| {
        if !manager.warmed_up || !manager.entries.iter().any(|entry| entry == current) {
            return None;
        }
        manager.cooldown_generation += 1;
        Some(manager.cooldown_generation)
    });
    let Some(generation) = generation else {
        return;
    };
    let cooldown = cx.layout().tooltip_cooldown_ms.max(close_delay);
    cx.spawn(async move |cx: &mut gpui::AsyncApp| {
        cx.background_executor()
            .timer(Duration::from_millis(cooldown))
            .await;
        cx.update_global::<TooltipManager, _>(|manager, _| {
            if manager.cooldown_generation == generation {
                manager.warmed_up = false;
                manager.entries.clear();
            }
        });
    })
    .detach();
}

/// `trigger` — what reveals the tip.
///
/// v3's default is `hover`, and React Aria shows a hovered tooltip on keyboard
/// focus as well, so `Hover` means "either". `Focus` is the narrower one: the
/// pointer does nothing and only focus opens it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TooltipTrigger {
    #[default]
    /// Shown while the trigger is hovered.
    Hover,
    /// Shown while the trigger is focused.
    Focus,
}

impl TooltipTrigger {
    /// Every trigger mode, in declaration order.
    pub const ALL: [TooltipTrigger; 2] = [TooltipTrigger::Hover, TooltipTrigger::Focus];

    /// A human-readable label for this trigger mode.
    pub fn label(self) -> &'static str {
        match self {
            TooltipTrigger::Hover => "Hover",
            TooltipTrigger::Focus => "Focus",
        }
    }
}

/// HeroUI Tooltip: wraps a trigger and reveals a tip on hover.
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct Tooltip {
    id: Option<ElementId>,
    content: SharedString,
    is_disabled: bool,
    placement: TooltipPlacement,
    show_arrow: bool,
    offset: Option<Pixels>,
    should_skip_animation: bool,
    delay: Option<u64>,
    close_delay: Option<u64>,
    trigger: TooltipTrigger,
    /// A caller-drawn tip body, in place of the measured single line.
    #[allow(clippy::type_complexity)]
    body: Option<Box<dyn Fn(&mut Window, &mut App) -> AnyElement + 'static>>,
    children: Vec<AnyElement>,
    /// The corner radius, in place of the owning `small_radius` helper.
    radius: Option<Pixels>,
    /// The `sx` slot, refined over the root style at the end of render.
    sx: Option<Box<gpui::StyleRefinement>>,
}

impl Tooltip {
    /// Creates a tooltip with the given content.
    pub fn new(content: impl Into<SharedString>) -> Self {
        Self {
            id: None,
            content: content.into(),
            is_disabled: false,
            placement: TooltipPlacement::Top,
            show_arrow: false,
            offset: None,
            should_skip_animation: false,
            delay: None,
            close_delay: None,
            trigger: TooltipTrigger::default(),
            body: None,
            children: Vec::new(),
            radius: None,
            sx: None,
        }
    }

    /// Distinguishes this tooltip's hover state from its neighbours'.
    ///
    /// The default key is the tip text, which is unique on most pages; set an
    /// id when two tooltips on one screen share the same content.
    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Draws the tip's body yourself, in place of the text handed to
    /// [`Tooltip::new`].
    ///
    /// v3's `Tooltip` takes children, so a tip is free to compose a small
    /// table, a key/value list or a swatch legend. This port shapes the tip's
    /// single line to reproduce CSS `max-content` capped at 320px, which only
    /// a string can go through; an element body skips that measurement and
    /// takes its own intrinsic width under the same 320px cap instead.
    ///
    /// The string from [`Tooltip::new`] is still required and still used: it
    /// remains the tip's accessible name and the default hover key, so a rich
    /// tip cannot ship without something a screen reader can read. Pass the
    /// text the body conveys.
    ///
    /// The closure runs only while the tip is on screen, never for a closed
    /// tooltip, and it runs again on each frame of the reveal.
    ///
    /// ```
    /// # use herogpui_components::tooltip::Tooltip;
    /// # use gpui::{div, IntoElement, ParentElement};
    /// Tooltip::new("Tokens: name, kind, scope")
    ///     .body(|_, _| {
    ///         div()
    ///             .child("name — the identifier")
    ///             .child("kind — the token class")
    ///             .into_any_element()
    ///     })
    ///     .child(div().child("tokens"));
    /// ```
    pub fn body(mut self, render: impl Fn(&mut Window, &mut App) -> AnyElement + 'static) -> Self {
        self.body = Some(Box::new(render));
        self
    }

    /// `isDisabled` — suppresses the tip entirely.
    pub fn is_disabled(mut self, v: bool) -> Self {
        self.is_disabled = v;
        self
    }

    /// Sets where the tooltip sits relative to its trigger.
    pub fn placement(mut self, p: TooltipPlacement) -> Self {
        self.placement = p;
        self
    }

    /// `showArrow` — draws the arrow indicator pointing at the trigger.
    pub fn show_arrow(mut self, v: bool) -> Self {
        self.show_arrow = v;
        self
    }

    /// `offset` — distance from the trigger. Defaults to 3px, or 7px with an
    /// arrow, matching v3.
    pub fn offset(mut self, offset: impl Into<Pixels>) -> Self {
        self.offset = Some(offset.into());
        self
    }

    /// `shouldSkipAnimation` — reveal without the entry animation.
    ///
    /// v3 uses this when moving quickly between neighbouring triggers, where
    /// re-animating each tip reads as flicker.
    pub fn should_skip_animation(mut self, v: bool) -> Self {
        self.should_skip_animation = v;
        self
    }

    /// `trigger` — `hover` (the default, which also answers keyboard focus) or
    /// `focus`, which the pointer cannot open.
    pub fn trigger(mut self, trigger: TooltipTrigger) -> Self {
        self.trigger = trigger;
        self
    }

    /// `delay` — milliseconds to wait before showing. Defaults to the
    /// `--tooltip-delay` theme token.
    pub fn delay(mut self, ms: u64) -> Self {
        self.delay = Some(ms);
        self
    }

    /// `closeDelay` — milliseconds to wait before hiding. Defaults to the
    /// `--tooltip-close-delay` theme token.
    pub fn close_delay(mut self, ms: u64) -> Self {
        self.close_delay = Some(ms);
        self
    }

    /// The corner radius, in place of the owning `small_radius` helper. The
    /// tip's entry zoom interpolates the same value, so both follow the
    /// override. Not a v3 prop; the removed v2 `radius` prop is prohibited
    /// and this is a per-component repository extension.
    pub fn radius(mut self, radius: impl Into<Pixels>) -> Self {
        self.radius = Some(radius.into());
        self
    }

    /// The one slot for caller-owned low-level styling: GPUI's styling methods
    /// (`bg`, `text_color`, `w`, `h`, `p`, `rounded`, `border_color`, …)
    /// applied to the tooltip's root element — the wrapper the trigger and the
    /// floating tip sit in — after every value the placement and the active
    /// theme chose, so they win.
    pub fn sx(mut self, style: impl FnOnce(gpui::Div) -> gpui::Div) -> Self {
        util::refine_sx(&mut self.sx, style);
        self
    }
}

impl ParentElement for Tooltip {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for Tooltip {
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        if self.is_disabled {
            // A disabled tooltip renders its trigger and nothing else.
            return util::apply_sx(gpui::div().flex().children(self.children), &self.sx)
                .into_any_element();
        }

        let key = self
            .id
            .clone()
            .unwrap_or_else(|| ElementId::Name(self.content.clone()));
        // The state entity has to be created before the theme tokens are read;
        // `use_keyed_state` takes `cx` mutably and would conflict with them.
        let state = window.use_keyed_state(key.clone(), cx, |_, _| TooltipHover::new());
        let current_tooltip = state.downgrade();
        let (delay, close_delay) = {
            let layout = cx.layout();
            (
                self.delay.unwrap_or(layout.tooltip_delay_ms),
                self.close_delay.unwrap_or(layout.tooltip_close_delay_ms),
            )
        };
        // React Aria explicitly removes the Trigger wrapper's tab index: the
        // caller's trigger is the stop, and this handle only reports whether a
        // descendant currently owns focus.
        let wrap_focus =
            window.use_keyed_state(element_id::scoped(&key, "wrap-focus"), cx, |_, cx| {
                cx.focus_handle()
            });
        let wrap_handle = wrap_focus.read(cx).clone();
        let focus_held = wrap_handle.contains_focused(window, cx);
        // Escape's dismissal is per focus *session*: once the focus leaves the
        // trigger, the latch is dropped, so the next focus is a fresh one and
        // shows the tip again. Clearing here rather than on the next open is
        // what makes a dismissal not permanent without ever touching the
        // app-wide `focus_visible`.
        if focus_held != state.read(cx).was_focused {
            let keyboard_focus = focus_held && util::focus_visible(cx);
            let leaving_keyboard_focus = state.read(cx).is_focus_open() && !focus_held;
            state.update(cx, |s, cx| {
                let closed = leaving_keyboard_focus && s.close(false);
                s.was_focused = focus_held;
                s.focus_open = keyboard_focus;
                // Either edge ends the previous dismissal session. Clearing
                // on arrival matters when hover was dismissed before focus.
                s.focus_dismissed = false;
                if keyboard_focus {
                    // An immediate focus open replaces a pending hover warmup,
                    // just as React Stately clears its global warmup timeout.
                    s.generation += 1;
                }
                if closed {
                    cx.notify();
                }
            });
            if keyboard_focus {
                let _ = prepare_tooltip_open(&current_tooltip, cx);
                mark_tooltip_open(cx);
            } else if leaving_keyboard_focus {
                start_tooltip_cooldown(&current_tooltip, close_delay, cx);
            }
        }
        let state_snapshot = state.read(cx);
        let focus_open = state_snapshot.is_focus_open();
        let hover_open = state_snapshot.is_open();
        // `trigger="focus"` takes the pointer out of it; `hover` is both, which
        // is React Aria's behaviour for the default.
        let open = match self.trigger {
            TooltipTrigger::Hover => hover_open || focus_open,
            TooltipTrigger::Focus => focus_open,
        };

        let hover_state = state.clone();
        let dismiss_current = current_tooltip.clone();
        let dismiss_tooltip = util::shared(move |cx: &mut App| {
            state.update(cx, |s, cx| {
                if s.close(true) {
                    cx.notify();
                }
            });
            start_tooltip_cooldown(&dismiss_current, close_delay, cx);
            util::DismissResult::Handled
        });
        // Press dismissal belongs to the trigger. The tip is a sibling here;
        // v3 portals it outside the trigger, so pressing the surface itself
        // must not trip `shouldCloseOnPress`.
        let trigger = gpui::div()
            .flex()
            .children(self.children)
            .capture_any_mouse_down({
                let dismiss_tooltip = dismiss_tooltip.clone();
                move |_, _, cx| {
                    dismiss_tooltip(cx);
                }
            })
            .on_key_down({
                let dismiss_tooltip = dismiss_tooltip.clone();
                move |_, _, cx| {
                    // RAC wires `onKeyDown: onPressStart` on the trigger: any
                    // key dismisses an already-open tooltip immediately.
                    dismiss_tooltip(cx);
                }
            });
        let hover_enabled = self.trigger == TooltipTrigger::Hover;
        let mut wrapper = gpui::div()
            // `on_hover` needs a stateful element, so the wrapper carries the id.
            .id(key.clone())
            .track_focus(&wrap_handle)
            .relative()
            .flex()
            .child(trigger)
            .on_hover(move |over, _window, cx: &mut App| {
                if !hover_enabled {
                    return;
                }
                let over = *over;
                let current = hover_state.downgrade();
                let warmed_up = over && prepare_tooltip_open(&current, cx);
                if !over {
                    // GPUI dispatches sibling hover listeners in reverse paint
                    // order. An outgoing tooltip may run after the incoming
                    // one opened, so only the manager's current entry may cool.
                    start_tooltip_cooldown(&current, close_delay, cx);
                }
                let wait = if over {
                    if warmed_up { 0 } else { delay }
                } else {
                    close_delay
                };
                let generation = hover_state.update(cx, |s, _| {
                    s.generation += 1;
                    s.generation
                });

                if wait == 0 {
                    hover_state.update(cx, |s, cx| {
                        if over {
                            mark_tooltip_open(cx);
                            s.open = true;
                            cx.notify();
                        } else if s.close(true) {
                            cx.notify();
                        }
                    });
                    return;
                }

                let weak = hover_state.downgrade();
                cx.spawn(async move |cx: &mut gpui::AsyncApp| {
                    cx.background_executor()
                        .timer(Duration::from_millis(wait))
                        .await;
                    if let Some(state) = weak.upgrade() {
                        state.update(cx, |s, cx| {
                            // A newer hover transition supersedes this timer.
                            if s.generation == generation {
                                if over {
                                    mark_tooltip_open(cx);
                                    if !s.open {
                                        s.open = true;
                                        cx.notify();
                                    }
                                } else if s.close(true) {
                                    cx.notify();
                                }
                            }
                        });
                    }
                })
                .detach();
            });
        // React Aria hides a tooltip on Escape, which reaches here from the
        // focused trigger inside the wrapper. The hover flag alone is not
        // enough: a `trigger="focus"` tip reads the focus gate and never
        // looks at `open`, so Escape has to trip `focus_dismissed` as well.
        // The latch is per focus session — it is dropped when the focus
        // leaves (see the render gate) — so the next focus shows the tip
        // again, and `focus_visible` is deliberately left untouched.
        let (phase, overlay_token) = util::overlay_scope(
            window,
            cx,
            element_id::scoped(&key, "tip-phase"),
            open,
            true,
        );
        let captured_dismiss = dismiss_tooltip.clone();
        util::capture_escape(&overlay_token, move |_window, cx| captured_dismiss(cx), cx);
        wrapper = util::dismiss_on_escape_with_token(wrapper, overlay_token, move |_window, cx| {
            dismiss_tooltip(cx)
        });

        // A tooltip leaves the way every other overlay does: `overlay_scope`
        // keeps it for its exit run, which is what `[data-exiting]` needs to
        // have something to play and gives Escape a stack position.
        //
        // The tip — and the max-content line shaping it is sized from — is
        // only built while it is visible: `shape_line` is the most expensive
        // call in this render, and a closed tooltip has no surface to size.
        if phase != util::OverlayPhase::Closed {
            // The caller's body is built first: it takes `cx` mutably, and the
            // theme reads below hold it borrowed for the rest of this block.
            let body = self.body.take().map(|render| render(window, cx));
            let colors = cx.colors();
            let layout = cx.layout();
            // v3 pushes the tip further out when the arrow needs room.
            let offset = self
                .offset
                .unwrap_or(if self.show_arrow { px(7.) } else { px(3.) });
            // The entry zoom interpolates the tip's own radius, so one
            // binding feeds both the painted shape and the animation.
            let radius = self.radius.unwrap_or_else(|| util::small_radius(cx));
            // CSS gives an absolutely positioned tooltip max-content width capped
            // at 320px. GPUI otherwise resolves normal wrapping to min-content,
            // making even "With an arrow" one word wide, so shape the single line
            // and pin the same max-content result explicitly.
            let content = self.content.clone();
            // A caller-drawn body replaces the measured line, and with it the
            // whole `max-content` reconstruction the string path performs: an
            // element resolves its own intrinsic width the way CSS would, so
            // the tip only has to impose the same 320px cap on it. The string
            // stays the tip's accessible name in both shapes.
            let mut tip = match body {
                Some(body) => {
                    gpui::div()
                        .id(element_id::scoped(&key, "tip"))
                        .a11y_named(a11y::Role::Tooltip, &a11y::Name::labelled(content))
                        .relative()
                        // `.tooltip` is `p-2` all round.
                        .p(px(8.))
                        .max_w(px(320.))
                        .rounded(radius)
                        .bg(colors.overlay.background)
                        .text_color(colors.overlay.foreground)
                        .text_size(px(12.))
                        .line_height(px(16.))
                        .when_some(layout.overlay_hairline, |el, hairline| {
                            el.border(layout.border_width).border_color(hairline)
                        })
                        .shadow(layout.overlay_shadow.clone())
                        .child(body)
                }
                None => {
                    let raw_run = gpui::TextRun {
                        len: content.len(),
                        font: window.text_style().font(),
                        color: gpui::black(),
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    };
                    let hairline_width = if layout.overlay_hairline.is_some() {
                        layout.border_width * 2.
                    } else {
                        px(0.)
                    };
                    // `overflow-wrap: anywhere` only takes effect when the natural
                    // line would exceed the 320px cap.  Inserting a zero-width break
                    // after every character unconditionally makes short placements
                    // such as the `Left` tooltip wrap its final letter because GPUI's
                    // line wrapper treats the opportunity as a legal split even when
                    // the unbroken word would fit.  Measure the natural text first,
                    // then add opportunities only for content that actually needs the
                    // cap.
                    let raw_line =
                        window
                            .text_system()
                            .shape_line(content.clone(), px(12.), &[raw_run], None);
                    let natural_width = raw_line.width + px(16.) + hairline_width;
                    let (display, needs_breaks) =
                        tooltip_display_content(content.as_ref(), natural_width);
                    let display_content: SharedString = display.into();
                    let run = gpui::TextRun {
                        len: display_content.len(),
                        font: window.text_style().font(),
                        color: gpui::black(),
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    };
                    let line = if display_content == content {
                        raw_line
                    } else {
                        window.text_system().shape_line(
                            display_content.clone(),
                            px(12.),
                            &[run],
                            None,
                        )
                    };
                    let intrinsic_width = line.width + px(16.) + hairline_width;
                    let tooltip_width = if intrinsic_width < px(320.) {
                        intrinsic_width
                    } else {
                        px(320.)
                    };

                    gpui::div()
                    // `tooltip/tooltip.js` renders the RAC `Tooltip`, and
                    // `react-aria/dist/private/tooltip/useTooltip.js` is a single
                    // `role: 'tooltip'`. Upstream leaves the tip unnamed and
                    // points the *trigger*'s `aria-describedby` at it; with no id
                    // graph the port names the tip with its own content instead,
                    // which is the text that describedby would have resolved to.
                    .id(element_id::scoped(&key, "tip"))
                    .a11y_named(a11y::Role::Tooltip, &a11y::Name::labelled(content))
                    // The placement anchor lives on an outer absolute wrapper
                    // below. Keeping the painted surface relative lets the entry
                    // slide use top/left without replacing that anchor.
                    .relative()
                    // `.tooltip` is `p-2` all round, not a wider-than-tall pill.
                    .p(px(8.))
                    .w(tooltip_width)
                    .rounded(radius)
                    .bg(colors.overlay.background)
                    .text_color(colors.overlay.foreground)
                    .text_size(px(12.))
                    .line_height(px(16.))
                    // GPUI's normal wrapper can round a max-content width down by
                    // a glyph fraction and split the last letter of a short
                    // placement label (for example, `Left`).  Short tooltips have
                    // already been measured to fit, so keep that line intact;
                    // long capped content still uses normal wrapping at the
                    // inserted zero-width opportunities above.
                    .when(!needs_breaks, |el| el.whitespace_nowrap())
                    .when_some(layout.overlay_hairline, |el, hairline| {
                        el.border(layout.border_width).border_color(hairline)
                    })
                    .shadow(layout.overlay_shadow.clone())
                    .child(display_content)
                }
            };

            if self.show_arrow {
                // The arrow leaf pins to the tip's resolved side; the
                // placement's cross-axis alignment flushes it to that edge or
                // centres it by stretching, mirroring the anchor below.
                let mut arrow = gpui::div().absolute().child(
                    gpui::svg()
                        .size(px(12.))
                        .path(icons::TOOLTIP_ARROW)
                        // svg() never inherits text colour; the arrow has to be
                        // tinted to match the tip body explicitly.
                        .text_color(colors.overlay.background)
                        .with_transformation(gpui::Transformation::rotate(gpui::radians(
                            arrow_rotation(self.placement),
                        ))),
                );
                arrow = if self.placement.is_side() {
                    let base = if self.placement.is_start_side() {
                        arrow.left_full()
                    } else {
                        arrow.right_full()
                    };
                    match self.placement.align() {
                        PlacementAlign::Start => base.top(px(0.)),
                        PlacementAlign::End => base.bottom(px(0.)),
                        PlacementAlign::Center => {
                            base.top(px(0.)).bottom(px(0.)).flex().items_center()
                        }
                    }
                } else {
                    let base = if self.placement.is_above() {
                        arrow.top_full()
                    } else {
                        arrow.bottom_full()
                    };
                    match self.placement.align() {
                        PlacementAlign::Start => base.left(px(0.)),
                        PlacementAlign::End => base.right(px(0.)),
                        PlacementAlign::Center => {
                            base.left(px(0.)).right(px(0.)).flex().justify_center()
                        }
                    }
                };
                tip = tip.child(arrow);
            }

            // `absolute` does not lift the tip above later siblings in the page,
            // so it has to paint last.
            let (slide_x, slide_y) = entry_offset(self.placement);
            let zoom = anim::ZoomBox::panel(px(8.), radius).padding_x(px(8.));
            let zoom = anim::ZoomBox {
                slide_x: (slide_x != 0.0).then(|| px(slide_x)),
                slide_y: (slide_y != 0.0).then(|| px(slide_y)),
                ..zoom
            };
            let animated = if self.should_skip_animation {
                tip.into_any_element()
            } else if phase == util::OverlayPhase::Exiting {
                anim::exiting(
                    tip,
                    element_id::scoped(&key, "tip-out"),
                    zoom,
                    anim::Motion::LIST_OUT,
                    cx,
                )
            } else {
                // `tooltip.css` is `duration-150 ease-smooth zoom-in-90` — the
                // same zoom as a popover, not a slide.
                anim::entering_zoom(
                    tip,
                    element_id::scoped(&key, "tip"),
                    zoom,
                    anim::Motion::POPOVER_IN,
                    cx,
                )
            };
            // Keep the placement anchor outside the animated surface. The
            // inner `ZoomBox` can then apply its four-pixel relative slide
            // without clobbering the anchor's absolute side constraint.
            let mut anchor = gpui::div().absolute();
            anchor = if self.placement.is_side() {
                let base = if self.placement.is_start_side() {
                    anchor.right_full().mr(offset)
                } else {
                    anchor.left_full().ml(offset)
                };
                match self.placement.align() {
                    PlacementAlign::Start => base.top_0(),
                    PlacementAlign::End => base.bottom_0(),
                    PlacementAlign::Center => base.top_0().bottom_0().flex().items_center(),
                }
            } else {
                let base = if self.placement.is_above() {
                    anchor.bottom_full().mb(offset)
                } else {
                    anchor.top_full().mt(offset)
                };
                match self.placement.align() {
                    PlacementAlign::Start => base.left_0(),
                    PlacementAlign::End => base.right_0(),
                    PlacementAlign::Center => base.left_0().right_0().flex().justify_center(),
                }
            };
            wrapper = wrapper.child(util::floating(anchor.child(animated)));
        }

        util::apply_sx(wrapper, &self.sx).into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        arrow_rotation, entry_offset, tooltip_display_content,
        tooltip_text_with_break_opportunities, TooltipPlacement,
    };

    #[test]
    fn tooltip_text_adds_breaks_without_changing_whitespace() {
        assert_eq!(
            tooltip_text_with_break_opportunities("longtoken"),
            "l\u{200b}o\u{200b}n\u{200b}g\u{200b}t\u{200b}o\u{200b}k\u{200b}e\u{200b}n"
        );
        assert_eq!(
            tooltip_text_with_break_opportunities("two words\nnext"),
            "t\u{200b}w\u{200b}o w\u{200b}o\u{200b}r\u{200b}d\u{200b}s\nn\u{200b}e\u{200b}x\u{200b}t"
        );
    }

    #[test]
    fn short_tooltips_keep_their_label_while_long_content_gets_breaks() {
        assert_eq!(
            tooltip_display_content("Left", gpui::px(40.)),
            ("Left".to_owned(), false)
        );
        let (long, needs_breaks) = tooltip_display_content("longtoken", gpui::px(321.));
        assert!(needs_breaks);
        assert!(long.contains('\u{200b}'));
    }

    #[test]
    fn entry_offsets_follow_the_tooltip_side() {
        assert_eq!(entry_offset(TooltipPlacement::Top), (0.0, 4.0));
        assert_eq!(entry_offset(TooltipPlacement::TopStart), (0.0, 4.0));
        assert_eq!(entry_offset(TooltipPlacement::TopLeft), (0.0, 4.0));
        assert_eq!(entry_offset(TooltipPlacement::TopEnd), (0.0, 4.0));
        assert_eq!(entry_offset(TooltipPlacement::TopRight), (0.0, 4.0));
        assert_eq!(entry_offset(TooltipPlacement::Bottom), (0.0, -4.0));
        assert_eq!(entry_offset(TooltipPlacement::BottomStart), (0.0, -4.0));
        assert_eq!(entry_offset(TooltipPlacement::BottomLeft), (0.0, -4.0));
        assert_eq!(entry_offset(TooltipPlacement::BottomEnd), (0.0, -4.0));
        assert_eq!(entry_offset(TooltipPlacement::BottomRight), (0.0, -4.0));
        assert_eq!(entry_offset(TooltipPlacement::Left), (4.0, 0.0));
        assert_eq!(entry_offset(TooltipPlacement::LeftTop), (4.0, 0.0));
        assert_eq!(entry_offset(TooltipPlacement::LeftBottom), (4.0, 0.0));
        assert_eq!(entry_offset(TooltipPlacement::Start), (4.0, 0.0));
        assert_eq!(entry_offset(TooltipPlacement::StartTop), (4.0, 0.0));
        assert_eq!(entry_offset(TooltipPlacement::StartBottom), (4.0, 0.0));
        assert_eq!(entry_offset(TooltipPlacement::Right), (-4.0, 0.0));
        assert_eq!(entry_offset(TooltipPlacement::RightTop), (-4.0, 0.0));
        assert_eq!(entry_offset(TooltipPlacement::RightBottom), (-4.0, 0.0));
        assert_eq!(entry_offset(TooltipPlacement::End), (-4.0, 0.0));
        assert_eq!(entry_offset(TooltipPlacement::EndTop), (-4.0, 0.0));
        assert_eq!(entry_offset(TooltipPlacement::EndBottom), (-4.0, 0.0));
    }

    #[test]
    #[allow(clippy::float_cmp)] // the rotations are quarter-turn constants
    fn arrow_rotations_face_the_tooltip_side() {
        assert_eq!(arrow_rotation(TooltipPlacement::Top), 0.);
        assert_eq!(arrow_rotation(TooltipPlacement::TopRight), 0.);
        assert_eq!(
            arrow_rotation(TooltipPlacement::Bottom),
            std::f32::consts::PI
        );
        assert_eq!(
            arrow_rotation(TooltipPlacement::BottomLeft),
            std::f32::consts::PI
        );
        assert_eq!(
            arrow_rotation(TooltipPlacement::Left),
            -std::f32::consts::FRAC_PI_2
        );
        assert_eq!(
            arrow_rotation(TooltipPlacement::StartBottom),
            -std::f32::consts::FRAC_PI_2
        );
        assert_eq!(
            arrow_rotation(TooltipPlacement::Right),
            std::f32::consts::FRAC_PI_2
        );
        assert_eq!(
            arrow_rotation(TooltipPlacement::EndTop),
            std::f32::consts::FRAC_PI_2
        );
    }

    #[test]
    fn placement_list_includes_the_supported_aligned_edges() {
        assert_eq!(TooltipPlacement::ALL.len(), 22);
        for placement in [
            TooltipPlacement::TopStart,
            TooltipPlacement::TopLeft,
            TooltipPlacement::TopEnd,
            TooltipPlacement::TopRight,
            TooltipPlacement::BottomStart,
            TooltipPlacement::BottomLeft,
            TooltipPlacement::BottomEnd,
            TooltipPlacement::BottomRight,
            TooltipPlacement::LeftTop,
            TooltipPlacement::LeftBottom,
            TooltipPlacement::RightTop,
            TooltipPlacement::RightBottom,
            TooltipPlacement::Start,
            TooltipPlacement::StartTop,
            TooltipPlacement::StartBottom,
            TooltipPlacement::End,
            TooltipPlacement::EndTop,
            TooltipPlacement::EndBottom,
        ] {
            assert!(TooltipPlacement::ALL.contains(&placement));
        }
    }
}

crate::util::impl_component_styled!(Tooltip);
