use std::rc::Rc;

use gpui::{div, prelude::*, px, AnyElement};
use herogpui::components::{PickerItem, Select};
use herogpui::gpui;

use crate::ui::icon::icon_element;
use crate::ui::menu::{MenuEntry, MenuItem};

pub fn render(
    id: &'static str,
    label: &'static str,
    entries: Vec<MenuEntry>,
    content: impl Fn() -> AnyElement + 'static,
    show_labels: bool,
) -> AnyElement {
    let entries: Rc<Vec<MenuItem>> = Rc::new(
        entries
            .into_iter()
            .filter_map(|entry| match entry {
                MenuEntry::Item(item) => Some(item),
                _ => None,
            })
            .collect(),
    );
    let items = entries
        .iter()
        .enumerate()
        .map(|(index, item)| PickerItem::new(index.to_string(), item.label.clone()))
        .collect();
    let selected = entries
        .iter()
        .position(|item| item.trailing_check)
        .map(|index| index.to_string().into());
    let rows = entries.clone();
    Select::new(id, items)
        .placement(herogpui::Placement::Bottom)
        .recipe("toolbar")
        .aria_label(label)
        .fit_panel_width(true)
        .value(selected)
        .value_content(move |_| content())
        .indicator(move |selected| {
            if selected {
                return div()
                    .size(px(10.))
                    .debug_selector(move || format!("{id}-selected-indicator"))
                    .child(icon_element("check", px(10.0)))
                    .into_any_element();
            }
            div().into_any_element()
        })
        .item_content(move |key, _, _, cx| {
            let item = key.parse::<usize>().ok().and_then(|index| rows.get(index));
            let Some(item) = item else {
                return div().into_any_element();
            };
            let Some(draw) = &item.leading else {
                return div().flex_1().child(item.label.clone()).into_any_element();
            };
            let probe = format!("{id}-content-{key}");
            div()
                .debug_selector(move || probe.clone())
                .flex_1()
                .h(px(16.))
                .flex()
                .items_center()
                .gap(px(12.0))
                .child(draw(cx))
                .when(show_labels, |row| {
                    row.child(div().flex_1().child(item.label.clone()))
                })
                .into_any_element()
        })
        .on_selection_change(move |key, window, cx| {
            let item = key
                .as_ref()
                .and_then(|key| key.parse::<usize>().ok())
                .and_then(|index| entries.get(index));
            if let Some(action) = item.and_then(|item| item.action.as_ref()) {
                action(window, cx);
            }
        })
        .into_any_element()
}
