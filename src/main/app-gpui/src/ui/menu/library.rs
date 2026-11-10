use std::collections::HashMap;
use std::rc::Rc;

use gpui::{div, prelude::*, px, AnyElement, App, SharedString, Window};
use herogpui::components::{InteractiveState, MenuItem as LibraryItem};
use herogpui::gpui;

use super::model::{MenuEntry, MenuItem};
use crate::ui::icon::icon_element;

pub struct MenuItems {
    pub items: Vec<LibraryItem>,
    pub entries: HashMap<SharedString, MenuItem>,
    pub disabled: Vec<SharedString>,
    pub selected: Vec<SharedString>,
    pub has_indicators: bool,
}

impl MenuItems {
    pub fn new(entries: &[MenuEntry]) -> Self {
        let mut result = Self {
            items: Vec::new(),
            entries: HashMap::new(),
            disabled: Vec::new(),
            selected: Vec::new(),
            has_indicators: false,
        };
        result.items = result.convert(entries, "item");
        result
    }

    fn convert(&mut self, entries: &[MenuEntry], prefix: &str) -> Vec<LibraryItem> {
        entries
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                let item = match entry {
                    MenuEntry::Separator => return LibraryItem::Separator,
                    MenuEntry::Label(label) => return LibraryItem::SectionLabel(label.clone()),
                    MenuEntry::Item(item) => item,
                };
                let key = SharedString::from(format!("{prefix}-{index}"));
                if !item.is_interactive() {
                    self.disabled.push(key.clone());
                }
                if item.toggle == Some(true) || item.radio == Some(true) {
                    self.selected.push(key.clone());
                }
                self.has_indicators |= item.toggle.is_some() || item.radio.is_some() || item.inset;
                let mut converted = LibraryItem::new(key.clone(), item.label.clone());
                if item.danger {
                    converted = converted.danger();
                }
                if let Some(shortcut) = &item.shortcut {
                    converted = converted.shortcut(shortcut.clone());
                }
                converted = converted.submenu(self.convert(&item.submenu, &key));
                self.entries.insert(key, item.clone());
                converted
            })
            .collect()
    }
}

pub type MenuEntries = Rc<HashMap<SharedString, MenuItem>>;

pub fn item_content(
    entries: MenuEntries,
    compact: bool,
) -> impl Fn(&SharedString, InteractiveState) -> AnyElement + 'static {
    move |key, _| match entries.get(key) {
        Some(item) => ItemContent {
            item: item.clone(),
            compact,
        }
        .into_any_element(),
        None => div().into_any_element(),
    }
}

pub fn item_action(
    entries: MenuEntries,
) -> impl Fn(&SharedString, &mut Window, &mut App) + 'static {
    move |key, window, cx| {
        let Some(item) = entries.get(key).filter(|item| item.is_interactive()) else {
            return;
        };
        if let Some(action) = &item.action {
            action(window, cx);
        }
    }
}

#[derive(IntoElement)]
pub struct ItemContent {
    pub item: MenuItem,
    pub compact: bool,
}

impl RenderOnce for ItemContent {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let item = self.item;
        let mut content = div()
            .flex()
            .items_center()
            .gap(if self.compact { px(8.0) } else { px(12.0) })
            .w_full();
        if let Some(leading) = &item.leading {
            content = content.child(div().flex_shrink_0().child(leading(cx)));
        } else if let Some(icon) = &item.icon {
            content = content.child(div().flex_shrink_0().child(icon_element(icon, px(14.0))));
        }
        content = content.child(div().flex_1().min_w_0().truncate().child(item.label));
        if item.trailing_check {
            content = content.child(icon_element("check", px(14.0)));
        }
        content
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    #[test]
    fn duplicate_labels_keep_distinct_nested_actions() {
        let selected = Rc::new(Cell::new(0));
        let first = selected.clone();
        let second = selected.clone();
        let entries = vec![
            MenuEntry::Item(MenuItem::new("Same").on_select(move |_, _| first.set(1))),
            MenuEntry::Item(MenuItem::new("Parent").submenu(vec![MenuEntry::Item(
                MenuItem::new("Same").on_select(move |_, _| second.set(2)),
            )])),
        ];
        let converted = MenuItems::new(&entries);
        assert_eq!(converted.entries.len(), 3);
        assert!(converted.entries["item-0"].action.is_some());
        assert!(converted.entries["item-1-0"].action.is_some());
        assert!(converted.disabled.is_empty());
    }

    #[test]
    fn disabled_rows_and_radio_state_survive_nested_conversion() {
        let entries = vec![
            MenuEntry::Label("Section".into()),
            MenuEntry::Separator,
            MenuEntry::Item(MenuItem::new("Parent").submenu(vec![
                MenuEntry::Item(MenuItem::new("Read only")),
                MenuEntry::Item(MenuItem::new("Disabled").disabled(true).on_select(|_, _| {})),
                MenuEntry::Item(MenuItem::new("Selected").radio(true).on_select(|_, _| {})),
            ])),
        ];
        let converted = MenuItems::new(&entries);
        assert!(matches!(converted.items[0], LibraryItem::SectionLabel(_)));
        assert!(matches!(converted.items[1], LibraryItem::Separator));
        assert_eq!(
            converted.disabled,
            vec![SharedString::from("item-2-0"), "item-2-1".into()]
        );
        assert_eq!(converted.selected, vec![SharedString::from("item-2-2")]);
        assert!(converted.has_indicators);
    }
}
