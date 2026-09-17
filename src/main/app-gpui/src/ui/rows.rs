use gpui::{div, prelude::*, px, AnyElement, Context, Div, SharedString, Styled};
use herogpui::components::{Button, PickerItem, Slider, Switch};
use herogpui::gpui;

use crate::theme::vars::ThemeVars;
use crate::ui::chrome;

pub fn label(text: impl Into<SharedString>, theme: &ThemeVars) -> AnyElement {
    label_weighted(text, gpui::FontWeight::MEDIUM, theme)
}

pub fn label_weighted(
    text: impl Into<SharedString>,
    weight: gpui::FontWeight,
    theme: &ThemeVars,
) -> AnyElement {
    div()
        .text_size(px(chrome::TEXT_SM))
        .font_weight(weight)
        .text_color(theme.foreground)
        .child(text.into())
        .into_any_element()
}

pub fn description(text: impl Into<SharedString>, theme: &ThemeVars) -> AnyElement {
    div()
        .text_size(px(chrome::TEXT_XS))
        .text_color(theme.muted_foreground)
        .child(text.into())
        .into_any_element()
}

pub fn note(text: impl Into<SharedString>, theme: &ThemeVars) -> AnyElement {
    div()
        .w_full()
        .min_w_0()
        .text_size(px(chrome::TEXT_SM))
        .text_color(theme.muted_foreground)
        .child(text.into())
        .into_any_element()
}

pub fn hint(text: impl Into<SharedString>, theme: &ThemeVars) -> AnyElement {
    div()
        .w_full()
        .min_w_0()
        .text_size(px(chrome::TEXT_XS))
        .text_color(theme.muted_foreground)
        .child(text.into())
        .into_any_element()
}

pub fn error(text: impl Into<SharedString>, theme: &ThemeVars) -> AnyElement {
    div()
        .w_full()
        .min_w_0()
        .flex()
        .justify_center()
        .text_size(px(chrome::TEXT_XS))
        .text_color(theme.destructive)
        .child(text.into())
        .into_any_element()
}

pub fn title_desc_stack(
    title: impl Into<SharedString>,
    desc: impl Into<SharedString>,
    weight: gpui::FontWeight,
    theme: &ThemeVars,
) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(2.0))
        .child(label_weighted(title, weight, theme))
        .child(hint(desc, theme))
}

pub fn switch<V: 'static>(
    id: impl Into<gpui::ElementId>,
    checked: bool,
    cx: &mut Context<V>,
    on_change: impl Fn(&mut V, bool, &mut Context<V>) + 'static,
) -> Switch {
    Switch::new(id).is_selected(checked).on_change(cx.listener(
        move |this, value: &bool, _window, cx| {
            on_change(this, *value, cx);
        },
    ))
}

pub fn picker_items<K: Into<SharedString>, L: Into<SharedString>>(
    options: impl IntoIterator<Item = (K, L)>,
) -> Vec<PickerItem> {
    options
        .into_iter()
        .map(|(value, label)| PickerItem::new(value, label))
        .collect()
}

pub fn selected_value(items: &[PickerItem], current: &str) -> Option<SharedString> {
    items
        .iter()
        .any(|item| item.key().as_str() == current)
        .then(|| SharedString::from(current))
}

pub fn slider_control<V: 'static>(
    id: impl Into<gpui::ElementId>,
    value: f64,
    min: f64,
    max: f64,
    step: f64,
    cx: &mut Context<V>,
    on_change: impl Fn(&mut V, f64, &mut Context<V>) + 'static,
) -> Slider {
    Slider::new(id, value as f32)
        .min_value(min as f32)
        .max_value(max as f32)
        .continuous(true)
        .on_change(cx.listener(move |this, value: &f32, _window, cx| {
            let next = if step > 0.0 {
                (*value as f64 / step).round() * step
            } else {
                *value as f64
            };
            on_change(this, next, cx);
        }))
}

pub fn icon_text_button(
    id: impl Into<gpui::ElementId>,
    label: impl Into<SharedString>,
    icon: &'static str,
    icon_size: f32,
    gap: f32,
) -> Button {
    let label = label.into();
    let content_label = label.clone();
    Button::new(id).label(label).content(move |_| {
        crate::ui::primitives::icon_label(
            icon,
            content_label.clone(),
            px(icon_size),
            px(gap),
            false,
        )
    })
}
