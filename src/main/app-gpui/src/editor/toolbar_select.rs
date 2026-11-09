use std::rc::Rc;

use gpui::{div, prelude::*, px, AnyElement, App};
use herogpui::components::{PickerItem, Select};
use herogpui::gpui;
use herogpui::theme::ActiveTheme as _;

use crate::ui::icon::icon_element;
use crate::ui::menu::{MenuEntry, MenuItem};

pub fn render(
    id: &'static str,
    label: &'static str,
    entries: Vec<MenuEntry>,
    content: impl Fn() -> AnyElement + 'static,
    show_labels: bool,
    window: &mut gpui::Window,
    cx: &mut App,
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
    let style = cx.components().select.resolve(&["toolbar".into()]);
    let text_size = style.row_text_size.unwrap_or(px(14.0));
    let padding = f32::from(style.panel_padding.unwrap_or(px(6.0)))
        + f32::from(style.row_padding_x.unwrap_or(px(10.0)));
    let mut panel_width = f32::from(style.panel_min_width.unwrap_or(px(0.0)));
    for item in entries.iter() {
        let leading_width = item
            .leading
            .as_ref()
            .map(|draw| {
                let mut element = draw(cx);
                f32::from(
                    element
                        .layout_as_root(gpui::AvailableSpace::min_size(), window, cx)
                        .width,
                )
            })
            .unwrap_or(0.0);
        let label_width = if show_labels || item.leading.is_none() {
            let run = gpui::TextRun {
                len: item.label.len(),
                font: window.text_style().font(),
                color: gpui::black(),
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            f32::from(
                window
                    .text_system()
                    .shape_line(item.label.clone(), text_size, &[run], None)
                    .width,
            )
        } else {
            0.0
        };
        let gap = if leading_width > 0.0 && label_width > 0.0 {
            12.0
        } else {
            0.0
        };
        panel_width = panel_width.max(leading_width + gap + label_width + padding * 2.0);
    }
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
        .panel_min_width(px(panel_width))
        .value(selected)
        .value_content(move |_| content())
        .indicator(|selected| {
            if selected {
                return icon_element("check", px(10.0));
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
            div()
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
