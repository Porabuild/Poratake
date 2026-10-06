use std::cell::Cell;
use std::rc::Rc;

use gpui::{div, prelude::*, px, App, Context, FocusHandle, Pixels, Render, Window};
use herogpui::components::Menu;
use herogpui::gpui;

use super::library::{item_action, item_content, MenuItems};
use super::model::MenuEntry;
use crate::ui::icon::icon_element;

pub(super) const ACCENT_RECIPE: &str = "accent";

pub type DismissHandler = Rc<dyn Fn(&mut Window, &mut App)>;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum MenuEntrance {
    #[default]
    Overlay,
    Instant,
}

pub struct MenuView {
    entries: Vec<MenuEntry>,
    on_dismiss: DismissHandler,
    min_width: Option<Pixels>,
    max_width: Option<Pixels>,
    max_height: Option<Pixels>,
    compact: bool,
    neutral_highlight: bool,
    entrance: MenuEntrance,
    exiting: Rc<Cell<bool>>,
    focus_handle: FocusHandle,
}

impl MenuView {
    pub fn new(
        entries: Vec<MenuEntry>,
        on_dismiss: DismissHandler,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            entries,
            on_dismiss,
            min_width: None,
            max_width: None,
            max_height: None,
            compact: false,
            neutral_highlight: false,
            entrance: MenuEntrance::default(),
            exiting: Rc::new(Cell::new(false)),
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn min_width(mut self, width: Pixels) -> Self {
        self.min_width = Some(width);
        self
    }

    pub fn max_width(mut self, width: Pixels) -> Self {
        self.max_width = Some(width);
        self
    }

    pub fn compact(mut self, compact: bool) -> Self {
        self.compact = compact;
        self
    }

    pub fn neutral_highlight(mut self, neutral: bool) -> Self {
        self.neutral_highlight = neutral;
        self
    }

    pub fn entrance(mut self, entrance: MenuEntrance) -> Self {
        self.entrance = entrance;
        self
    }

    pub fn exit_flag(mut self, exiting: Rc<Cell<bool>>) -> Self {
        self.exiting = exiting;
        self
    }

    pub fn max_height(mut self, height: Pixels) -> Self {
        self.max_height = Some(height);
        self
    }

    pub fn focus_handle(&self) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for MenuView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let converted = MenuItems::new(&self.entries);
        let entries = Rc::new(converted.entries);
        let indicators = entries.clone();
        let dismiss = self.on_dismiss.clone();
        let compact = self.compact;
        let mut menu = Menu::new("menu", converted.items)
            .focus_handle(self.focus_handle.clone())
            .animate_entry(self.entrance == MenuEntrance::Overlay)
            .exiting(self.exiting.get())
            .disabled_keys(converted.disabled)
            .selected_keys(converted.selected)
            .on_dismiss(move |_, window, cx| dismiss(window, cx))
            .on_action(item_action(entries.clone()))
            .item_content(item_content(entries, compact));
        if compact {
            menu = menu.recipe("compact");
        }
        menu = menu.recipe(if self.neutral_highlight {
            "neutral"
        } else {
            ACCENT_RECIPE
        });
        if let Some(min_width) = self.min_width {
            menu = menu.panel_min_width(min_width);
        }
        if let Some(max_width) = self.max_width {
            menu = menu.panel_max_width(max_width);
        }
        if let Some(max_height) = self.max_height {
            menu = menu.panel_max_height(max_height);
        }
        if converted.has_indicators {
            menu = menu.indicator_content(move |key, _, _| {
                let item = indicators.get(key);
                match item {
                    Some(item) if item.toggle == Some(true) => icon_element("check", px(14.0)),
                    Some(item) if item.radio == Some(true) => icon_element("circle", px(8.0)),
                    _ => div().into_any_element(),
                }
            });
        }
        menu
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::menu::MenuItem;
    use std::cell::Cell;

    #[herogpui::test]
    fn keyboard_actions_skip_disabled_rows_and_dismiss(cx: &mut gpui::TestAppContext) {
        cx.update(herogpui::init);
        let selected = Rc::new(Cell::new(0));
        let dismissed = Rc::new(Cell::new(0));
        let first = selected.clone();
        let second = selected.clone();
        let closed = dismissed.clone();
        let window = cx.add_window(|window, cx| {
            let menu = MenuView::new(
                vec![
                    MenuEntry::Item(
                        MenuItem::new("Disabled")
                            .disabled(true)
                            .on_select(move |_, _| first.set(1)),
                    ),
                    MenuEntry::Item(MenuItem::new("Enabled").on_select(move |_, _| second.set(2))),
                ],
                Rc::new(move |_, _| closed.set(closed.get() + 1)),
                cx,
            )
            .entrance(MenuEntrance::Instant);
            window.focus(&menu.focus_handle(), cx);
            menu
        });
        cx.refresh().unwrap();
        cx.run_until_parked();
        cx.simulate_keystrokes(window.into(), "home enter");
        assert_eq!(selected.get(), 2);
        assert_eq!(dismissed.get(), 1);
    }

    #[herogpui::test]
    fn submenu_keyboard_action_keeps_its_own_key(cx: &mut gpui::TestAppContext) {
        cx.update(herogpui::init);
        let selected = Rc::new(Cell::new(0));
        let first = selected.clone();
        let nested = selected.clone();
        let dismissed = Rc::new(Cell::new(0));
        let closed = dismissed.clone();
        let window = cx.add_window(|window, cx| {
            let menu = MenuView::new(
                vec![
                    MenuEntry::Item(MenuItem::new("Same").on_select(move |_, _| first.set(1))),
                    MenuEntry::Item(MenuItem::new("Parent").submenu(vec![MenuEntry::Item(
                        MenuItem::new("Same").on_select(move |_, _| nested.set(2)),
                    )])),
                ],
                Rc::new(move |_, _| closed.set(closed.get() + 1)),
                cx,
            )
            .entrance(MenuEntrance::Instant);
            window.focus(&menu.focus_handle(), cx);
            menu
        });
        cx.refresh().unwrap();
        cx.run_until_parked();
        cx.simulate_keystrokes(window.into(), "end right");
        cx.run_until_parked();
        cx.simulate_keystrokes(window.into(), "enter");
        assert_eq!(selected.get(), 2);
        assert_eq!(dismissed.get(), 1);
    }

    #[herogpui::test]
    fn escape_dismisses_without_running_an_action(cx: &mut gpui::TestAppContext) {
        cx.update(herogpui::init);
        let selected = Rc::new(Cell::new(false));
        let action = selected.clone();
        let dismissed = Rc::new(Cell::new(false));
        let closed = dismissed.clone();
        let window = cx.add_window(|window, cx| {
            let menu = MenuView::new(
                vec![MenuEntry::Item(
                    MenuItem::new("Action").on_select(move |_, _| action.set(true)),
                )],
                Rc::new(move |_, _| closed.set(true)),
                cx,
            )
            .entrance(MenuEntrance::Instant);
            window.focus(&menu.focus_handle(), cx);
            menu
        });
        cx.refresh().unwrap();
        cx.run_until_parked();
        cx.simulate_keystrokes(window.into(), "escape");
        assert!(dismissed.get());
        assert!(!selected.get());
    }
}
