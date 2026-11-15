//! `ContextMenu` — a right-click menu over an area (HeroGPUI extension;
//! HeroUI v3 has no context menu, only the trigger-anchored `Dropdown`).
//!
//! It reuses the Dropdown's [`Menu`] panel unchanged — rows, sections,
//! submenus, typeahead, keyboard navigation, disabled keys, the enter/exit
//! motion, and the topmost-overlay Escape / outside-press arbitration — and
//! only replaces the trigger: a secondary-button press inside the wrapped
//! area opens the menu with its top-left corner at the pointer, flipping to
//! stay inside the window. Choosing an item, Escape or a press outside the
//! panel closes it.
//!
//! The keyboard opens it too: Shift+F10 or the ContextMenu key (`menu` on
//! Windows and Linux, `contextmenu` on the web) while the focus is anywhere
//! inside the area opens the menu below the focused HeroGPUI control when
//! its bounds were painted, or at the area's top-left corner otherwise. The
//! first item receives focus, and dismissing the menu returns it to the
//! previous control. The area owns a focus handle outside the tab order: a
//! primary press on a part of the area that does not take the focus itself
//! focuses the area, so an area with no focusable content is reachable too.
//!
//! The panel takes the same styling surface as a [`crate::Dropdown`]'s:
//! [`ContextMenu::recipe`] names a [`herogpui_theme::ComponentThemes::menu`]
//! overlay (row fill, panel width, padding, radius and the rest of
//! `MenuStyle`), and [`ContextMenu::item_content`] and
//! [`ContextMenu::item_start_content`] draw the rows' label and leading
//! element.
//!
//! ```
//! use herogpui_components::{ContextMenu, MenuItem};
//! use gpui::{div, prelude::*, px};
//!
//! let menu = ContextMenu::new(
//!     "canvas-menu",
//!     div().size(px(240.)).child("Right-click here"),
//!     vec![
//!         MenuItem::new("copy", "Copy").shortcut("⌘C"),
//!         MenuItem::new("paste", "Paste").shortcut("⌘V"),
//!         MenuItem::Separator,
//!         MenuItem::new("delete", "Delete").danger(),
//!     ],
//! )
//! .on_action(|key, _window, _cx| println!("chose {key}"));
//! ```

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    px, size, AnyElement, App, Bounds, InteractiveElement, IntoElement, MouseButton, ParentElement,
    Pixels, RenderOnce, SharedString, Styled, Window,
};
use herogpui_core::{element_id, Placement};

use crate::{Menu, MenuItem};

type Callback<T> = Arc<dyn Fn(&T, &mut Window, &mut App) + 'static>;
type RowContent<T> = Box<dyn Fn(&SharedString, crate::util::InteractiveState) -> T + 'static>;

/// A secondary-click menu over `child`. See the [module docs](self).
#[must_use = "a component does nothing until it is rendered: add it as a child or return it from `render`"]
#[derive(IntoElement)]
pub struct ContextMenu {
    id: gpui::ElementId,
    child: AnyElement,
    items: Vec<MenuItem>,
    disabled_keys: Vec<SharedString>,
    is_disabled: bool,
    on_action: Option<Callback<SharedString>>,
    on_open_change: Option<Callback<bool>>,
    recipes: Vec<SharedString>,
    item_content: Option<RowContent<AnyElement>>,
    item_start_content: Option<RowContent<Option<AnyElement>>>,
}

impl ContextMenu {
    /// A context menu with `items` over the `child` area. `id` keys the
    /// menu's state; give every instance its own.
    pub fn new(
        id: impl Into<gpui::ElementId>,
        child: impl IntoElement,
        items: Vec<MenuItem>,
    ) -> Self {
        Self {
            id: id.into(),
            child: child.into_any_element(),
            items,
            disabled_keys: Vec::new(),
            is_disabled: false,
            on_action: None,
            on_open_change: None,
            recipes: Vec::new(),
            item_content: None,
            item_start_content: None,
        }
    }

    /// Named theme overlay from [`herogpui_theme::ComponentThemes::menu`],
    /// forwarded onto the painted [`Menu`] as [`crate::Dropdown::recipe`]
    /// forwards it. Stackable; a missing name adds no override.
    pub fn recipe(mut self, name: impl Into<SharedString>) -> Self {
        self.recipes.push(name.into());
        self
    }

    /// Replaces each item's label, forwarded onto the painted [`Menu`]. See
    /// [`Menu::item_content`].
    pub fn item_content(
        mut self,
        render: impl Fn(&SharedString, crate::util::InteractiveState) -> AnyElement + 'static,
    ) -> Self {
        self.item_content = Some(Box::new(render));
        self
    }

    /// The leading element of each row, forwarded onto the painted [`Menu`].
    /// See [`Menu::item_start_content`].
    pub fn item_start_content(
        mut self,
        render: impl Fn(&SharedString, crate::util::InteractiveState) -> Option<AnyElement> + 'static,
    ) -> Self {
        self.item_start_content = Some(Box::new(render));
        self
    }

    /// Runs with the chosen item's key; the menu then closes.
    pub fn on_action(mut self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_action = Some(Arc::new(f));
        self
    }

    /// Reports every open and close.
    pub fn on_open_change(mut self, f: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_open_change = Some(Arc::new(f));
        self
    }

    /// Items that render dimmed and cannot be chosen or focused.
    pub fn disabled_keys(
        mut self,
        keys: impl IntoIterator<Item = impl Into<SharedString>>,
    ) -> Self {
        self.disabled_keys = keys.into_iter().map(Into::into).collect();
        self
    }

    /// A disabled context menu ignores secondary clicks; the area still
    /// renders and receives every other event.
    pub fn is_disabled(mut self, v: bool) -> Self {
        self.is_disabled = v;
        self
    }
}

fn set_open(
    own: &Option<gpui::Entity<bool>>,
    on_open_change: &Option<Callback<bool>>,
    open: bool,
    window: &mut Window,
    cx: &mut App,
) {
    if let Some(held) = own {
        held.update(cx, |v, cx| {
            *v = open;
            cx.notify();
        });
    }
    if let Some(cb) = on_open_change {
        cb(&open, window, cx);
    }
}

/// Shift+F10, or the ContextMenu key, with no other modifier.
fn opens_context_menu(keystroke: &gpui::Keystroke) -> bool {
    let m = &keystroke.modifiers;
    if m.control || m.alt || m.platform || m.function {
        return false;
    }
    match keystroke.key.as_str() {
        "f10" => m.shift,
        "menu" | "contextmenu" => !m.shift,
        _ => false,
    }
}

impl RenderOnce for ContextMenu {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let base = self.id.clone();
        let (is_open, own) =
            crate::util::controlled(window, cx, element_id::scoped(&base, "open"), None, false);
        let (phase, overlay_token) = crate::util::overlay_scope(
            window,
            cx,
            element_id::scoped(&base, "phase"),
            is_open,
            true,
        );
        // The pointer position, as a zero-size anchor the shared popover
        // positioner places the panel against (offset 0, so the panel's
        // corner sits on the pointer).
        let anchor = window
            .use_keyed_state(element_id::scoped(&base, "anchor"), cx, |_, _| {
                Rc::new(Cell::new(None::<Bounds<Pixels>>))
            })
            .read(cx)
            .clone();
        let focus_first =
            window.use_keyed_state(element_id::scoped(&base, "focus-first"), cx, |_, _| {
                None::<crate::list_core::OpenFocus>
            });

        let area_focus = window
            .use_keyed_state(element_id::scoped(&base, "area-focus"), cx, |_, cx| {
                cx.focus_handle().tab_stop(false)
            })
            .read(cx)
            .clone();
        // The area's own bounds, the fallback keyboard-open anchor.
        let area_bounds = window
            .use_keyed_state(element_id::scoped(&base, "area-bounds"), cx, |_, _| {
                Rc::new(Cell::new(None::<Bounds<Pixels>>))
            })
            .read(cx)
            .clone();
        // Where the focus was when the keyboard opened the menu.
        let restore_focus = window
            .use_keyed_state(element_id::scoped(&base, "restore-focus"), cx, |_, _| {
                Rc::new(std::cell::RefCell::new(None::<gpui::FocusHandle>))
            })
            .read(cx)
            .clone();

        let bounds_probe = area_bounds.clone();
        let mut area = gpui::div()
            .id(element_id::scoped(&base, "area"))
            .relative()
            .track_focus(&area_focus)
            .child(self.child)
            .child(
                gpui::canvas(
                    move |bounds, _, _| bounds_probe.set(Some(bounds)),
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0(),
            );
        if !self.is_disabled {
            let focus = area_focus;
            area = area.on_mouse_down(MouseButton::Left, move |_, window, cx| {
                if !focus.contains_focused(window, cx) {
                    window.focus(&focus, cx);
                }
            });
            let anchor_key = anchor.clone();
            let own_key = own.clone();
            let on_open_change_key = self.on_open_change.clone();
            let focus_first_key = focus_first.clone();
            let restore_key = restore_focus.clone();
            area = area.on_key_down(move |event, window, cx| {
                if is_open || !opens_context_menu(&event.keystroke) {
                    return;
                }
                let Some(bounds) = area_bounds.get() else {
                    return;
                };
                let target = crate::util::focused_element_bounds(window, cx)
                    .filter(|focused| bounds.intersects(focused))
                    .unwrap_or(Bounds::new(bounds.origin, size(px(0.), px(0.))));
                anchor_key.set(Some(target));
                *restore_key.borrow_mut() = window.focused(cx);
                focus_first_key.update(cx, |focus, _| {
                    *focus = Some(crate::list_core::OpenFocus::First);
                });
                set_open(&own_key, &on_open_change_key, true, window, cx);
                cx.stop_propagation();
            });
            let anchor = anchor.clone();
            let own = own.clone();
            let on_open_change = self.on_open_change.clone();
            let pointer_focus_first = focus_first.clone();
            let pointer_restore = restore_focus.clone();
            area = area.on_mouse_down(MouseButton::Right, move |event, window, cx| {
                anchor.set(Some(Bounds::new(event.position, size(px(0.), px(0.)))));
                pointer_focus_first.update(cx, |focus, _| {
                    *focus = Some(crate::list_core::OpenFocus::None);
                });
                pointer_restore.borrow_mut().take();
                set_open(&own, &on_open_change, true, window, cx);
                cx.stop_propagation();
            });
        }

        let mut root = gpui::div().relative().child(area);
        if phase != crate::util::OverlayPhase::Closed {
            let mut menu = Menu::new(element_id::scoped(&base, "menu-content"), self.items)
                .id(element_id::scoped(&base, "menu"))
                .dropdown_composition()
                .focus_first(focus_first)
                .exiting(phase == crate::util::OverlayPhase::Exiting)
                .disabled_keys(self.disabled_keys)
                .overlay_token(overlay_token);
            for recipe in self.recipes {
                menu = menu.recipe(recipe);
            }
            if let Some(render) = self.item_content {
                menu = menu.item_content(render);
            }
            if let Some(render) = self.item_start_content {
                menu = menu.item_start_content(render);
            }
            if let Some(on_action) = self.on_action {
                menu = menu.on_action(move |key, window, cx| on_action(key, window, cx));
            }
            let on_open_change = self.on_open_change;
            menu = menu.on_dismiss(move |_refocus, window, cx| {
                set_open(&own, &on_open_change, false, window, cx);
                // A keyboard open took the focus into the menu; hand it back,
                // an Enter pick included: pinned gpui activates an element on
                // key up only when the key went down on it too, so the
                // restored element does not see the pick's release as a press.
                let restore = restore_focus.borrow_mut().take();
                if let Some(handle) = restore {
                    window.focus(&handle, cx);
                }
            });
            root = root.child(crate::util::floating(
                crate::popover::popover_with_resolved_placement(
                    anchor,
                    Placement::BottomStart,
                    px(0.),
                    None,
                    menu.panel_debug_label("context-menu"),
                ),
            ));
        }
        root
    }
}
