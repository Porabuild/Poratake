use std::collections::HashMap;

use gpui::{div, prelude::*, px, AnyElement, App, SharedString, Window};
use herogpui::components::{MenuItem as LibraryItem, PickerItem, Select, Size, Switch};
use herogpui::gpui;

use super::model::{MenuEntry, MenuItem};
use crate::theme::vars::active_theme;
use crate::ui::icon::icon_element;

const ROW_SELECT_HEIGHT: f32 = 24.0;
const ROW_SELECT_MIN_WIDTH: f32 = 56.0;
const ROW_SELECT_PADDING_X: f32 = 8.0;
const ROW_SELECT_PADDING_Y: f32 = 2.0;
const ROW_SELECT_TEXT_SIZE: f32 = 12.0;

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
                converted = if hosts_row_select(item) {
                    converted.is_interactive(true)
                } else {
                    converted.submenu(self.convert(&item.submenu, &key))
                };
                self.entries.insert(key, item.clone());
                converted
            })
            .collect()
    }
}

#[derive(IntoElement)]
pub struct ItemContent {
    pub key: SharedString,
    pub item: MenuItem,
    pub compact: bool,
}

impl RenderOnce for ItemContent {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = active_theme(cx);
        let item = self.item;
        let mut content = div()
            .flex()
            .items_center()
            .gap(if self.compact { px(8.0) } else { px(12.0) })
            .w_full();
        if let Some(row_label) = item.row_label.clone() {
            content = content.justify_between().gap_2().child(
                div()
                    .text_size(px(12.0))
                    .text_color(theme.muted_foreground)
                    .child(row_label),
            );
            if let Some(checked) = item.trailing_switch {
                return content.child(
                    Switch::new(SharedString::from(format!("{}-switch", self.key)))
                        .is_selected(checked)
                        .size(Size::Sm)
                        .is_disabled(item.disabled)
                        .on_change(move |_, window, cx| {
                            cx.stop_propagation();
                            if let Some(action) = &item.action {
                                action(window, cx);
                            }
                        }),
                );
            }
            return content.child(row_select(&self.key, &item));
        }
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

fn hosts_row_select(item: &MenuItem) -> bool {
    item.row_label.is_some() && item.trailing_switch.is_none() && !item.submenu.is_empty()
}

fn row_select(key: &SharedString, item: &MenuItem) -> AnyElement {
    let options: Vec<MenuItem> = item
        .submenu
        .iter()
        .filter_map(|entry| match entry {
            MenuEntry::Item(option) => Some(option.clone()),
            _ => None,
        })
        .collect();
    let items: Vec<PickerItem> = options
        .iter()
        .enumerate()
        .map(|(index, option)| PickerItem::new(index.to_string(), option.label.clone()))
        .collect();
    let selected = options
        .iter()
        .position(|option| option.trailing_check)
        .or_else(|| options.iter().position(|option| option.label == item.label))
        .map(|index| SharedString::from(index.to_string()));
    let actions: Vec<Option<super::model::MenuAction>> =
        options.into_iter().map(|option| option.action).collect();
    div()
        .flex()
        .flex_shrink_0()
        .justify_end()
        .min_w(px(ROW_SELECT_MIN_WIDTH))
        .child(
            Select::new(SharedString::from(format!("{key}-select")), items)
                .recipe("compact")
                .full_width(true)
                .height(px(ROW_SELECT_HEIGHT))
                .padding_x(px(ROW_SELECT_PADDING_X))
                .padding_y(px(ROW_SELECT_PADDING_Y))
                .trigger_text_size(px(ROW_SELECT_TEXT_SIZE))
                .radius(px(crate::ui::chrome::RADIUS_3XL))
                .default_value(selected)
                .on_selection_change(move |value, window, cx| {
                    let Some(action) = value
                        .as_ref()
                        .and_then(|value| value.parse::<usize>().ok())
                        .and_then(|index| actions.get(index))
                        .and_then(|action| action.as_ref())
                    else {
                        return;
                    };
                    action(window, cx);
                }),
        )
        .into_any_element()
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
    fn a_row_with_a_select_stays_inline_instead_of_opening_a_submenu() {
        let options = vec![
            MenuEntry::Item(MenuItem::new("1").trailing_check(true).on_select(|_, _| {})),
            MenuEntry::Item(MenuItem::new("2").on_select(|_, _| {})),
        ];
        let entries = vec![MenuEntry::Item(
            MenuItem::new("1").row("Starting:").submenu(options),
        )];
        let converted = MenuItems::new(&entries);

        assert_eq!(converted.items.len(), 1, "no submenu rows are emitted");
        assert!(
            matches!(&converted.items[0], LibraryItem::Item { submenu, is_interactive, .. }
                if submenu.is_empty() && *is_interactive),
            "the row hands its interaction to the hosted select"
        );
        assert!(converted.disabled.is_empty());
        assert!(hosts_row_select(&converted.entries["item-0"]));
    }

    #[test]
    fn a_switch_row_keeps_the_row_interaction() {
        let entries = vec![MenuEntry::Item(
            MenuItem::new("")
                .row("Background:")
                .trailing_switch(false)
                .on_select(|_, _| {}),
        )];
        let converted = MenuItems::new(&entries);
        assert!(!hosts_row_select(&converted.entries["item-0"]));
    }

    #[test]
    fn the_inline_select_reads_the_reference_pill() {
        assert_eq!(ROW_SELECT_HEIGHT, 24.0, "h-6");
        assert_eq!(ROW_SELECT_MIN_WIDTH, 56.0, "w-14");
        assert_eq!(
            ROW_SELECT_HEIGHT,
            20.0 + 2.0 * ROW_SELECT_PADDING_Y,
            "the trigger's line box fills the pill exactly"
        );
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
