//! `MenuBar` — a horizontal application menu bar (HeroGPUI extension; HeroUI
//! v3 has no menu bar, only the trigger-anchored `Dropdown`).
//!
//! Each top-level item opens the Dropdown's [`Menu`] panel unchanged — rows,
//! sections, submenus, typeahead, keyboard navigation, disabled keys, the
//! enter/exit motion and the topmost-overlay Escape / outside-press
//! arbitration — below its trigger. The bar adds the menubar behaviour on
//! top:
//!
//! - **Roving focus.** One trigger is in the tab order. Left and Right move
//!   the focus between the top-level items (wrapping, skipping disabled
//!   ones), Home and End jump to the first and last.
//! - **Opening.** A press, Enter, Space or Down opens a trigger's menu; the
//!   keyboard paths focus its first item.
//! - **Switching.** While a menu is open, Left and Right open the previous or
//!   next top-level menu instead (Right on an item with a submenu opens the
//!   submenu first, as in the Dropdown), and hovering another trigger switches
//!   to its menu.
//! - **Closing.** Escape, choosing an item, a press outside, or pressing the
//!   open trigger again closes the menu, and the focus returns to its trigger.
//!
//! ```
//! use herogpui_components::{MenuBar, MenuBarMenu, MenuItem};
//!
//! let bar = MenuBar::new(
//!     "app-menu",
//!     vec![
//!         MenuBarMenu::new(
//!             "file",
//!             "File",
//!             vec![MenuItem::new("new", "New").shortcut("⌘N"), MenuItem::new("open", "Open")],
//!         ),
//!         MenuBarMenu::new("edit", "Edit", vec![MenuItem::new("undo", "Undo")]),
//!     ],
//! )
//! .on_action(|menu, item, _window, _cx| println!("{menu} → {item}"));
//! ```

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    div, prelude::FluentBuilder as _, px, App, Bounds, InteractiveElement, IntoElement,
    MouseButton, ParentElement, Pixels, RenderOnce, SharedString, StatefulInteractiveElement,
    Styled, Window,
};
use herogpui_core::{element_id, Placement};
use herogpui_theme::ActiveTheme;

use crate::a11y::{self, A11y as _};
use crate::{Menu, MenuItem};

type ActionCallback = Arc<dyn Fn(&SharedString, &SharedString, &mut Window, &mut App) + 'static>;
type OpenCallback = Arc<dyn Fn(&Option<SharedString>, &mut Window, &mut App) + 'static>;

/// One top-level item of a [`MenuBar`] and the menu it opens.
#[must_use = "builder methods return a new value; pass it on to its component"]
pub struct MenuBarMenu {
    key: SharedString,
    label: SharedString,
    items: Vec<MenuItem>,
    disabled_keys: Vec<SharedString>,
    is_disabled: bool,
}

impl MenuBarMenu {
    /// A top-level item `label` whose menu holds `items`. `key` identifies
    /// it in [`MenuBar::on_action`] and [`MenuBar::on_open_change`].
    pub fn new(
        key: impl Into<SharedString>,
        label: impl Into<SharedString>,
        items: Vec<MenuItem>,
    ) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            items,
            disabled_keys: Vec::new(),
            is_disabled: false,
        }
    }

    /// Items of this menu that render dimmed and cannot be chosen or focused.
    pub fn disabled_keys(
        mut self,
        keys: impl IntoIterator<Item = impl Into<SharedString>>,
    ) -> Self {
        self.disabled_keys = keys.into_iter().map(Into::into).collect();
        self
    }

    /// A disabled top-level item renders dimmed, never opens, and is skipped
    /// by the arrow keys.
    pub fn is_disabled(mut self, v: bool) -> Self {
        self.is_disabled = v;
        self
    }
}

/// A horizontal bar of menus. See the [module docs](self).
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct MenuBar {
    id: gpui::ElementId,
    menus: Vec<MenuBarMenu>,
    on_action: Option<ActionCallback>,
    on_open_change: Option<OpenCallback>,
}

impl MenuBar {
    /// A menu bar over `menus`. `id` keys the bar's state; give every
    /// instance its own.
    pub fn new(id: impl Into<gpui::ElementId>, menus: Vec<MenuBarMenu>) -> Self {
        Self {
            id: id.into(),
            menus,
            on_action: None,
            on_open_change: None,
        }
    }

    /// Runs with the open menu's key and the chosen item's key; the menu
    /// then closes.
    pub fn on_action(
        mut self,
        f: impl Fn(&SharedString, &SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_action = Some(Arc::new(f));
        self
    }

    /// Reports the key of the menu that opened, or `None` when the bar
    /// closed. Switching between menus reports the new key.
    pub fn on_open_change(
        mut self,
        f: impl Fn(&Option<SharedString>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_open_change = Some(Arc::new(f));
        self
    }
}

/// Everything the event closures share, cloned into each.
#[derive(Clone)]
struct BarState {
    open: gpui::Entity<Option<usize>>,
    active: gpui::Entity<usize>,
    focus_first: gpui::Entity<Option<crate::list_core::OpenFocus>>,
    handles: Rc<Vec<gpui::FocusHandle>>,
    keys: Rc<Vec<SharedString>>,
    enabled: Rc<Vec<bool>>,
    on_open_change: Option<OpenCallback>,
}

impl BarState {
    /// Opens menu `ix` (or closes the bar with `None`), reporting the change.
    fn set_open(&self, ix: Option<usize>, keyboard: bool, window: &mut Window, cx: &mut App) {
        if *self.open.read(cx) == ix {
            return;
        }
        self.focus_first.update(cx, |v, _| {
            *v = Some(if keyboard {
                crate::list_core::OpenFocus::First
            } else {
                crate::list_core::OpenFocus::None
            });
        });
        self.open.update(cx, |v, cx| {
            *v = ix;
            cx.notify();
        });
        if let Some(ix) = ix {
            self.set_active(ix, cx);
        }
        if let Some(cb) = &self.on_open_change {
            cb(&ix.map(|ix| self.keys[ix].clone()), window, cx);
        }
    }

    fn set_active(&self, ix: usize, cx: &mut App) {
        self.active.update(cx, |v, cx| {
            if *v != ix {
                *v = ix;
                cx.notify();
            }
        });
    }

    /// The next enabled index from `from` in direction `step` (wrapping).
    fn step(&self, from: usize, forward: bool) -> Option<usize> {
        let n = self.enabled.len();
        (1..=n)
            .map(|d| {
                if forward {
                    (from + d) % n
                } else {
                    (from + n - d % n) % n
                }
            })
            .find(|&ix| self.enabled[ix])
    }

    fn focus_trigger(&self, ix: usize, window: &mut Window, cx: &mut App) {
        self.set_active(ix, cx);
        window.focus(&self.handles[ix], cx);
    }
}

impl RenderOnce for MenuBar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let base = self.id.clone();
        let count = self.menus.len();
        let open =
            window.use_keyed_state(element_id::scoped(&base, "open"), cx, |_, _| None::<usize>);
        let active = window.use_keyed_state(element_id::scoped(&base, "active"), cx, |_, _| 0);
        let focus_first =
            window.use_keyed_state(element_id::scoped(&base, "focus-first"), cx, |_, _| {
                None::<crate::list_core::OpenFocus>
            });
        let mut handles = Vec::with_capacity(count);
        let mut trigger_bounds = Vec::with_capacity(count);
        for ix in 0..count {
            let part = format!("trigger-{ix}");
            handles.push(
                window
                    .use_keyed_state(
                        element_id::scoped(&base, format!("{part}-focus")),
                        cx,
                        |_, cx| cx.focus_handle(),
                    )
                    .read(cx)
                    .clone(),
            );
            trigger_bounds.push(
                window
                    .use_keyed_state(
                        element_id::scoped(&base, format!("{part}-bounds")),
                        cx,
                        |_, _| Rc::new(Cell::new(None::<Bounds<Pixels>>)),
                    )
                    .read(cx)
                    .clone(),
            );
        }
        let enabled: Vec<bool> = self.menus.iter().map(|m| !m.is_disabled).collect();
        // A menu that went away or became disabled closes: the stored state
        // forgets it too (so hover and Left/Right rove instead of reopening),
        // and the caller hears about it after this render.
        let stored_open = *open.read(cx);
        let open_ix = stored_open.filter(|&ix| enabled.get(ix).copied().unwrap_or(false));
        if stored_open.is_some() && open_ix.is_none() {
            open.update(cx, |v, _| *v = None);
            if let Some(cb) = self.on_open_change.clone() {
                window.defer(cx, move |window, cx| cb(&None, window, cx));
            }
        }
        let active_ix = {
            let at = *active.read(cx);
            if enabled.get(at).copied().unwrap_or(false) {
                at
            } else {
                enabled.iter().position(|e| *e).unwrap_or(0)
            }
        };
        let (phase, overlay_token) = crate::util::overlay_scope(
            window,
            cx,
            element_id::scoped(&base, "phase"),
            open_ix.is_some(),
            true,
        );
        // The last open menu stays mounted through its exit motion.
        let shown =
            window.use_keyed_state(element_id::scoped(&base, "shown"), cx, |_, _| None::<usize>);
        if open_ix.is_some() && *shown.read(cx) != open_ix {
            shown.update(cx, |v, _| *v = open_ix);
        }
        let shown_ix = open_ix.or(*shown.read(cx));

        let state = BarState {
            open,
            active,
            focus_first: focus_first.clone(),
            handles: Rc::new(handles),
            keys: Rc::new(self.menus.iter().map(|m| m.key.clone()).collect()),
            enabled: Rc::new(enabled),
            on_open_change: self.on_open_change.clone(),
        };

        let colors = cx.colors().clone();
        let radius = crate::util::control_radius(cx);
        let focus_visible = crate::util::focus_visible(cx);
        let cursor = crate::util::interactive_cursor(cx);

        let mut bar = div()
            .id(element_id::scoped(&base, "bar"))
            .a11y(a11y::Role::MenuBar)
            .flex()
            .flex_row()
            .items_center()
            .gap(px(2.))
            .p(px(4.))
            .rounded(radius + px(4.))
            .bg(colors.surface.background)
            .text_sm();

        for (ix, menu) in self.menus.iter().enumerate() {
            let handle = state.handles[ix].clone().tab_stop(ix == active_ix);
            let is_open = open_ix == Some(ix);
            let disabled = menu.is_disabled;
            let focused = handle.is_focused(window);
            let bounds_probe = trigger_bounds[ix].clone();
            let mut trigger = div()
                .id(element_id::indexed(&base, "trigger", ix))
                .a11y_named(
                    a11y::Role::MenuItem,
                    &a11y::Name::labelled(menu.label.clone()),
                )
                .a11y_expanded(is_open)
                .relative()
                .flex()
                .items_center()
                .h(px(28.))
                .px(px(10.))
                .rounded(radius)
                .text_color(colors.foreground)
                .child(menu.label.clone())
                .child(
                    gpui::canvas(
                        move |bounds, _, _| bounds_probe.set(Some(bounds)),
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .inset_0(),
                )
                .debug_selector({
                    let name = format!("menu-bar-trigger-{}", menu.key);
                    move || name
                });
            if disabled {
                trigger = trigger.opacity(0.5);
            } else {
                trigger = trigger
                    .track_focus(&handle)
                    .cursor(cursor)
                    .hover(|s| s.bg(colors.default.color))
                    .when(is_open, |t| t.bg(colors.default.color))
                    .when(focused && focus_visible, |t| {
                        t.child(crate::util::focus_ring_overlay(radius, false, cx))
                    });
                trigger = crate::util::record_focus_bounds(trigger, &handle, window, cx);
                // Only a closed trigger sees its press: while its menu is
                // open, the press lands outside the menu panel, whose
                // capture-phase outside-press dismissal closes the menu and
                // stops the event before it reaches this listener.
                let press = state.clone();
                trigger = trigger.on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    crate::util::set_focus_visible(false, cx);
                    press.focus_trigger(ix, window, cx);
                    press.set_open(Some(ix), false, window, cx);
                    cx.stop_propagation();
                });
                let hover = state.clone();
                trigger = trigger.on_hover(move |hovered, window, cx| {
                    let open_now = *hover.open.read(cx);
                    if *hovered && open_now.is_some() && open_now != Some(ix) {
                        hover.focus_trigger(ix, window, cx);
                        hover.set_open(Some(ix), false, window, cx);
                    }
                });
                let keys = state.clone();
                trigger = trigger.on_key_down(move |event, window, cx| {
                    let key = event.keystroke.key.as_str();
                    if !matches!(key, "enter" | "space" | "down") {
                        return;
                    }
                    crate::util::set_focus_visible(true, cx);
                    keys.set_open(Some(ix), true, window, cx);
                    cx.stop_propagation();
                });
            }
            bar = bar.child(trigger);
        }

        let mut menus = self.menus;
        let nav = state.clone();
        let mut root = div()
            .relative()
            .flex()
            .child(bar)
            .on_key_down(move |event, window, cx| {
                let m = &event.keystroke.modifiers;
                if m.control || m.alt || m.platform || m.shift {
                    return;
                }
                let key = event.keystroke.key.as_str();
                let open_now = *nav.open.read(cx);
                let from =
                    open_now.or_else(|| nav.handles.iter().position(|h| h.is_focused(window)));
                let Some(from) = from else {
                    return;
                };
                let target = match key {
                    "right" => nav.step(from, true),
                    "left" => nav.step(from, false),
                    "home" if open_now.is_none() => nav.enabled.iter().position(|e| *e),
                    "end" if open_now.is_none() => nav.enabled.iter().rposition(|e| *e),
                    _ => return,
                };
                let Some(target) = target else {
                    return;
                };
                crate::util::set_focus_visible(true, cx);
                if open_now.is_some() {
                    nav.set_open(Some(target), true, window, cx);
                    // The switched-to menu takes the focus as it mounts.
                    nav.set_active(target, cx);
                } else {
                    nav.focus_trigger(target, window, cx);
                }
                cx.stop_propagation();
            });

        if let (Some(ix), true) = (shown_ix, phase != crate::util::OverlayPhase::Closed) {
            if ix < menus.len() {
                let spec = menus.swap_remove(ix);
                let menu_key = spec.key.clone();
                let mut menu = Menu::new(
                    element_id::scoped(&base, format!("menu-content-{}", spec.key)),
                    spec.items,
                )
                .id(element_id::scoped(&base, format!("menu-{}", spec.key)))
                .dropdown_composition()
                .focus_first(focus_first)
                .exiting(phase == crate::util::OverlayPhase::Exiting)
                .disabled_keys(spec.disabled_keys)
                .overlay_token(overlay_token);
                if let Some(on_action) = self.on_action {
                    menu = menu
                        .on_action(move |key, window, cx| on_action(&menu_key, key, window, cx));
                }
                let dismiss = state;
                menu = menu.on_dismiss(move |_refocus, window, cx| {
                    let was = *dismiss.open.read(cx);
                    dismiss.set_open(None, false, window, cx);
                    // Pinned gpui activates an element on key up only when
                    // the key also went down on it, so returning the focus to
                    // the trigger inside an Enter pick does not reopen it.
                    if let Some(was) = was {
                        dismiss.focus_trigger(was, window, cx);
                    }
                });
                root = root.child(crate::util::floating(
                    crate::popover::popover_with_resolved_placement(
                        trigger_bounds[ix].clone(),
                        Placement::BottomStart,
                        px(4.),
                        None,
                        menu.panel_debug_label("menu-bar-menu"),
                    ),
                ));
            }
        }
        root
    }
}
