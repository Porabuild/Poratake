use gpui::{div, prelude::*, px, AnyElement, Context, ElementId, SharedString, Styled};

use crate::theme::vars::ThemeVars;
use crate::ui::button::{Button, ButtonSize, ButtonVariant};
use crate::ui::chrome;
use crate::windows::history::model::{HistoryFilter, HistoryLayout, HistorySortOrder};
use crate::windows::history::HistoryWindow;

pub fn header(has_items: bool, theme: &ThemeVars, cx: &mut Context<HistoryWindow>) -> AnyElement {
    let mut actions = div()
        .flex()
        .items_center()
        .gap(px(4.0))
        .h(px(chrome::HISTORY_ACTION_SIZE));

    if has_items {
        actions = actions.child(
            Button::new("history-clear-all")
                .variant(ButtonVariant::Ghost)
                .size(ButtonSize::Xs)
                .icon("trash-2")
                .icon_size(px(chrome::HISTORY_CHIP_ICON))
                .gap(px(chrome::HISTORY_CHIP_ICON_GAP))
                .padding_x(px(chrome::HISTORY_CHIP_PAD_X))
                .label("Clear All")
                .radius(px(chrome::RADIUS_3XL))
                .foreground(theme.muted_foreground)
                .on_click(cx.listener(|this, _event, _window, cx| this.clear_all(cx))),
        );
    }

    actions = actions.child(
        Button::new("history-open-settings")
            .variant(ButtonVariant::Ghost)
            .size(ButtonSize::IconXs)
            .icon("settings")
            .icon_size(px(chrome::TOOL_BUTTON_ICON))
            .tooltip("Settings")
            .radius(px(chrome::RADIUS_3XL))
            .foreground(theme.muted_foreground)
            .on_click(cx.listener(|this, _event, window, cx| this.open_settings(window, cx))),
    );

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
        let mut button = Button::new(ElementId::Name(SharedString::from(format!(
            "history-filter-{}",
            option.as_str()
        ))))
        .variant(ButtonVariant::Ghost)
        .selected(filter == option)
        .size(ButtonSize::Xs)
        .height(px(chrome::HISTORY_CHIP_HEIGHT))
        .padding_x(px(chrome::HISTORY_CHIP_PAD_X))
        .gap(px(chrome::HISTORY_CHIP_ICON_GAP))
        .icon_size(px(chrome::HISTORY_CHIP_ICON))
        .label(option.label())
        .radius(px(chrome::RADIUS_3XL))
        .on_click(cx.listener(move |this, _event, _window, cx| {
            this.set_filter(option, cx);
        }));
        if let Some(icon) = option.icon() {
            button = button.icon(icon);
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
                .child(
                    Button::new("history-sort")
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::IconXs)
                        .height(px(chrome::HISTORY_CHIP_HEIGHT))
                        .icon_size(px(chrome::HISTORY_TOOL_ICON))
                        .icon("arrow-up-down")
                        .tooltip(order.tooltip())
                        .radius(px(chrome::RADIUS_3XL))
                        .foreground(theme.muted_foreground)
                        .on_click(
                            cx.listener(|this, _event, _window, cx| this.toggle_sort_order(cx)),
                        ),
                )
                .child(
                    Button::new("history-layout")
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::IconXs)
                        .height(px(chrome::HISTORY_CHIP_HEIGHT))
                        .icon_size(px(chrome::HISTORY_TOOL_ICON))
                        .icon(layout.toggle_icon())
                        .tooltip(layout.toggle_tooltip())
                        .radius(px(chrome::RADIUS_3XL))
                        .foreground(theme.muted_foreground)
                        .on_click(cx.listener(|this, _event, _window, cx| this.toggle_layout(cx))),
                ),
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
