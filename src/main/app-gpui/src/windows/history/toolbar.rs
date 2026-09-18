use gpui::{div, prelude::*, px, AnyElement, Context, ElementId, SharedString, Styled};
use herogpui::gpui;

use crate::theme::vars::ThemeVars;
use crate::ui::chrome;
use crate::ui::icon_button;
use crate::ui::toolbar;
use crate::windows::history::model::{HistoryFilter, HistoryLayout, HistorySortOrder};
use crate::windows::history::HistoryWindow;
use herogpui::components::Variant;

const TAB_HEIGHT: f32 = 20.0;
const TAB_PAD_X: f32 = 6.0;
const HEADER_ACTION_PAD_X: f32 = 8.0;

pub fn header(has_items: bool, theme: &ThemeVars, cx: &mut Context<HistoryWindow>) -> AnyElement {
    let mut actions = div()
        .flex()
        .items_center()
        .gap(px(4.0))
        .h(px(chrome::HISTORY_ACTION_SIZE));

    if has_items {
        actions = actions.child(
            crate::ui::rows::icon_text_button(
                "history-clear-all",
                "Clear All",
                "trash-2",
                chrome::HISTORY_CHIP_ICON,
                chrome::HISTORY_CHIP_ICON_GAP,
            )
            .variant(Variant::Ghost)
            .recipe("compact")
            .recipe("muted")
            .padding_x(px(HEADER_ACTION_PAD_X))
            .on_press(cx.listener(|this, _event, _window, cx| this.clear_all(cx))),
        );
    }

    actions = actions.child(toolbar::tooltip_button(
        icon_button::compact_muted("history-open-settings", "settings"),
        "Settings",
        cx,
        |this, window, cx| this.open_settings(window, cx),
    ));

    div()
        .flex()
        .items_center()
        .justify_between()
        .px(px(chrome::HISTORY_HEADER_PX))
        .py(px(chrome::HISTORY_HEADER_PY))
        .border_b_1()
        .border_color(theme.border)
        .child(
            div()
                .text_size(px(chrome::HISTORY_TITLE_SIZE))
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(theme.foreground)
                .child("History"),
        )
        .child(actions)
        .into_any_element()
}

pub fn toolbar(
    filter: HistoryFilter,
    order: HistorySortOrder,
    layout: HistoryLayout,
    theme: &ThemeVars,
    cx: &mut Context<HistoryWindow>,
) -> AnyElement {
    let mut filters = div().flex().items_center().gap(px(2.0));

    for option in HistoryFilter::ALL {
        let foreground = theme.foreground;
        let mut button = icon_button::chip(
            ElementId::Name(SharedString::from(format!(
                "history-filter-{}",
                option.as_str()
            ))),
            filter == option,
        )
        .label(option.label())
        .height(px(TAB_HEIGHT))
        .padding_x(px(TAB_PAD_X))
        .sx(move |el| el.text_color(foreground))
        .on_press(cx.listener(move |this, _event, _window, cx| {
            this.set_filter(option, cx);
        }));
        if let Some(icon) = option.icon() {
            let label = SharedString::from(option.label());
            button = button.content(move |_| {
                crate::ui::primitives::icon_label(
                    icon,
                    label.clone(),
                    px(chrome::HISTORY_CHIP_ICON),
                    px(chrome::HISTORY_CHIP_ICON_GAP),
                    false,
                )
            });
        }
        filters = filters.child(button);
    }

    div()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(chrome::TITLE_BAR_GAP))
        .px(px(chrome::HISTORY_TOOLBAR_PX))
        .py(px(chrome::HISTORY_TOOLBAR_PY))
        .child(filters)
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(2.0))
                .child(toolbar::tooltip_button(
                    icon_button::chip_icon("history-sort", "arrow-up-down"),
                    order.tooltip(),
                    cx,
                    |this, _window, cx| this.toggle_sort_order(cx),
                ))
                .child(toolbar::tooltip_button(
                    icon_button::chip_icon("history-layout", layout.toggle_icon()),
                    layout.toggle_tooltip(),
                    cx,
                    |this, _window, cx| this.toggle_layout(cx),
                )),
        )
        .into_any_element()
}

pub fn empty_state(
    icon_size: f32,
    title: impl Into<SharedString>,
    subtitle: Option<&'static str>,
    theme: &ThemeVars,
) -> AnyElement {
    let mut column = div()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(8.0))
        .size_full()
        .text_color(theme.muted_foreground)
        .child(crate::ui::icon::icon_element("image-off", px(icon_size)))
        .child(
            div()
                .text_size(px(chrome::HISTORY_TITLE_SIZE))
                .child(title.into()),
        );

    if let Some(subtitle) = subtitle {
        column = column.child(
            div()
                .text_size(px(chrome::ONBOARDING_HINT_SIZE))
                .child(subtitle),
        );
    }

    column.into_any_element()
}
