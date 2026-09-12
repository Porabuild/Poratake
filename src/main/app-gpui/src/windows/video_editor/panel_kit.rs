//! The shared row vocabulary the video editor's side panels are built from —
//! `SettingsPanelHeader`, `Label`, the small selects, sliders and switches in
//! `renderer/components/video-editor/components/`.

use gpui::{div, prelude::*, px, AnyElement, Context, SharedString, Styled};
use herogpui::gpui;

use crate::theme::vars::ThemeVars;
use crate::ui::menu::MenuHandle;
use crate::windows::video_editor::VideoEditorWindow;
use herogpui::components::{
    Button, PickerItem, Select, Size, Switch, TabItem, Tabs, TabsVariant, Variant,
};
use herogpui::components::{Slider, SliderSize};

pub fn panel(children: Vec<AnyElement>) -> AnyElement {
    div()
        .id("video-panel")
        .flex()
        .flex_col()
        .gap(px(crate::ui::chrome::VIDEO_PANEL_GAP))
        .size_full()
        .min_w_0()
        .overflow_y_scroll()
        .p(px(crate::ui::chrome::VIDEO_PANEL_PAD))
        .children(
            children
                .into_iter()
                .map(|child| div().w_full().flex_shrink_0().child(child)),
        )
        .into_any_element()
}

pub fn header(
    title: &'static str,
    description: &'static str,
    toggle: Option<bool>,
    theme: &ThemeVars,
    cx: &mut Context<VideoEditorWindow>,
    on_toggle: impl Fn(&mut VideoEditorWindow, bool, &mut Context<VideoEditorWindow>) + 'static,
) -> AnyElement {
    let mut row = div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .gap(px(8.0))
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .gap(px(2.0))
                .child(
                    div()
                        .text_size(px(crate::ui::chrome::SETTINGS_HEADER_TITLE))
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(theme.foreground)
                        .child(title),
                )
                .child(
                    div()
                        .w_full()
                        .text_size(px(crate::ui::chrome::SETTINGS_HEADER_DESC))
                        .text_color(theme.muted_foreground)
                        .child(description),
                ),
        );

    if let Some(enabled) = toggle {
        row = row.child(
            Switch::new(SharedString::from(format!("panel-toggle-{title}")))
                .is_selected(enabled)
                .size(Size::Sm)
                .on_change(cx.listener(move |this, value: &bool, _window, cx| {
                    on_toggle(this, *value, cx);
                })),
        );
    }

    row.into_any_element()
}

pub fn note(text: impl Into<SharedString>, theme: &ThemeVars) -> AnyElement {
    div()
        .w_full()
        .min_w_0()
        .text_size(px(crate::ui::chrome::TEXT_SM))
        .text_color(theme.muted_foreground)
        .child(text.into())
        .into_any_element()
}

pub fn hint(text: impl Into<SharedString>, theme: &ThemeVars) -> AnyElement {
    div()
        .w_full()
        .min_w_0()
        .text_size(px(crate::ui::chrome::TEXT_XS))
        .text_color(theme.muted_foreground)
        .child(text.into())
        .into_any_element()
}

pub fn empty_state(message: impl Into<SharedString>, theme: &ThemeVars) -> AnyElement {
    div()
        .flex()
        .size_full()
        .items_center()
        .justify_center()
        .p(px(crate::ui::chrome::VIDEO_PANEL_PAD))
        .child(
            div()
                .w_full()
                .min_w_0()
                .text_center()
                .text_size(px(crate::ui::chrome::TEXT_SM))
                .text_color(theme.muted_foreground)
                .child(message.into()),
        )
        .into_any_element()
}

/// `<Label className="text-sm">`, which HeroUI renders `font-medium`.
pub fn label(text: impl Into<SharedString>, theme: &ThemeVars) -> AnyElement {
    div()
        .text_size(px(crate::ui::chrome::TEXT_SM))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(theme.foreground)
        .child(text.into())
        .into_any_element()
}

pub fn field(text: impl Into<SharedString>, control: AnyElement, theme: &ThemeVars) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .child(label(text, theme))
        .child(control)
        .into_any_element()
}

pub fn switch_row(
    id: &'static str,
    text: &'static str,
    checked: bool,
    theme: &ThemeVars,
    cx: &mut Context<VideoEditorWindow>,
    on_change: impl Fn(&mut VideoEditorWindow, bool, &mut Context<VideoEditorWindow>) + 'static,
) -> AnyElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .gap(px(8.0))
        .child(label(text, theme))
        .child(
            Switch::new(id)
                .is_selected(checked)
                .size(Size::Sm)
                .on_change(cx.listener(move |this, value: &bool, _window, cx| {
                    on_change(this, *value, cx);
                })),
        )
        .into_any_element()
}

pub fn select_row(
    id: &'static str,
    text: &'static str,
    value: &str,
    options: &[(&'static str, &'static str)],
    _menu: &MenuHandle,
    theme: &ThemeVars,
    cx: &mut Context<VideoEditorWindow>,
    on_change: impl Fn(&mut VideoEditorWindow, String, &mut Context<VideoEditorWindow>) + 'static,
) -> AnyElement {
    let items: Vec<PickerItem> = options
        .iter()
        .map(|(value, label)| PickerItem::new(*value, *label))
        .collect();
    let value = items
        .iter()
        .any(|item| item.key().as_str() == value)
        .then(|| SharedString::from(value));
    field(
        text,
        Select::new(id, items)
            .recipe("compact")
            .value(value.clone())
            .full_width(true)
            .on_selection_change(cx.listener(
                move |this, value: &Option<SharedString>, _window, cx| {
                    let Some(value) = value else { return };
                    on_change(this, value.to_string(), cx);
                },
            ))
            .into_any_element(),
        theme,
    )
}

/// A segmented control — the renderer's `TabSelector`, used for the small
/// enumerations (sizes, shapes, positions) instead of a dropdown.
pub fn tab_row(
    id: &'static str,
    text: &'static str,
    value: &str,
    options: &[(&'static str, &'static str)],
    theme: &ThemeVars,
    cx: &mut Context<VideoEditorWindow>,
    on_change: impl Fn(&mut VideoEditorWindow, String, &mut Context<VideoEditorWindow>) + 'static,
) -> AnyElement {
    field(
        text,
        Tabs::new(
            id,
            options
                .iter()
                .map(|(value, label)| TabItem::new(*value, *label))
                .collect(),
            value,
        )
        .variant(TabsVariant::Secondary)
        .selected_key(value)
        .full_width(true)
        .on_selection_change(cx.listener(move |this, value: &SharedString, _window, cx| {
            on_change(this, value.to_string(), cx);
        }))
        .into_any_element(),
        theme,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn slider_row(
    id: &'static str,
    text: &'static str,
    value: f64,
    min: f64,
    max: f64,
    display: String,
    theme: &ThemeVars,
    cx: &mut Context<VideoEditorWindow>,
    on_change: impl Fn(&mut VideoEditorWindow, f64, &mut Context<VideoEditorWindow>) + 'static,
) -> AnyElement {
    let drag_view = cx.entity().downgrade();
    let drop_view = drag_view.clone();
    div()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .child(label(text, theme))
                .child(
                    div()
                        .text_size(px(crate::ui::chrome::TEXT_XS))
                        .text_color(theme.muted_foreground)
                        .child(display),
                ),
        )
        .child(
            Slider::new(id, value as f32)
                .min_value(min as f32)
                .max_value(max as f32)
                .continuous(true)
                .size(SliderSize::Sm)
                .on_drag_start(move |_window, cx| {
                    let _ = drag_view.update(cx, |this, _cx| this.begin_slider_gesture());
                })
                .on_drag_end(move |_window, cx| {
                    let _ = drop_view.update(cx, |this, cx| this.end_slider_gesture(cx));
                })
                .on_change(cx.listener(move |this, value: &f32, _window, cx| {
                    on_change(this, *value as f64, cx);
                })),
        )
        .into_any_element()
}

pub fn reset_button(
    id: &'static str,
    theme: &ThemeVars,
    cx: &mut Context<VideoEditorWindow>,
    on_click: impl Fn(&mut VideoEditorWindow, &mut Context<VideoEditorWindow>) + 'static,
) -> AnyElement {
    reset_named(id, "Reset to defaults", theme, cx, on_click)
}

pub fn reset_named(
    id: &'static str,
    label: &'static str,
    _theme: &ThemeVars,
    cx: &mut Context<VideoEditorWindow>,
    on_click: impl Fn(&mut VideoEditorWindow, &mut Context<VideoEditorWindow>) + 'static,
) -> AnyElement {
    Button::new(id)
        .variant(Variant::Ghost)
        .recipe("compact")
        .recipe("muted")
        .label(label)
        .full_width(true)
        .on_press(cx.listener(move |this, _event, _window, cx| on_click(this, cx)))
        .into_any_element()
}

/// The panel's primary action, sitting above the reset row.
pub fn tertiary_button(
    id: &'static str,
    label: &'static str,
    icon: &'static str,
    disabled: bool,
    theme: &ThemeVars,
    cx: &mut Context<VideoEditorWindow>,
    on_click: impl Fn(&mut VideoEditorWindow, &mut Context<VideoEditorWindow>) + 'static,
) -> AnyElement {
    let _ = theme;
    Button::new(id)
        .label(label)
        .content(move |_| {
            crate::ui::primitives::icon_label(icon, label.into(), px(14.0), px(8.0), false)
        })
        .variant(Variant::Tertiary)
        .recipe("compact")
        .is_disabled(disabled)
        .full_width(true)
        .on_press(cx.listener(move |this, _event, _window, cx| on_click(this, cx)))
        .into_any_element()
}

#[allow(dead_code)]
pub fn action_button(
    id: &'static str,
    label: &'static str,
    icon: &'static str,
    theme: &ThemeVars,
    cx: &mut Context<VideoEditorWindow>,
    on_click: impl Fn(&mut VideoEditorWindow, &mut Context<VideoEditorWindow>) + 'static,
) -> AnyElement {
    let _ = theme;
    Button::new(id)
        .label(label)
        .content(move |_| {
            crate::ui::primitives::icon_label(icon, label.into(), px(16.0), px(8.0), false)
        })
        .variant(Variant::Primary)
        .size(Size::Sm)
        .full_width(true)
        .on_press(cx.listener(move |this, _event, _window, cx| on_click(this, cx)))
        .into_any_element()
}

/// A secondary action, for the panel's supporting buttons.
#[allow(dead_code)]
pub fn secondary_button(
    id: &'static str,
    label: &'static str,
    icon: &'static str,
    theme: &ThemeVars,
    cx: &mut Context<VideoEditorWindow>,
    on_click: impl Fn(&mut VideoEditorWindow, &mut Context<VideoEditorWindow>) + 'static,
) -> AnyElement {
    let _ = theme;
    Button::new(id)
        .label(label)
        .content(move |_| {
            crate::ui::primitives::icon_label(icon, label.into(), px(14.0), px(8.0), false)
        })
        .variant(Variant::Secondary)
        .recipe("compact")
        .full_width(true)
        .on_press(cx.listener(move |this, _event, _window, cx| on_click(this, cx)))
        .into_any_element()
}

/// A hairline rule between groups inside a panel.
pub fn separator(theme: &ThemeVars) -> AnyElement {
    div()
        .h(px(1.0))
        .w_full()
        .my(px(4.0))
        .bg(theme.border)
        .into_any_element()
}

pub fn percent(value: f64) -> String {
    format!("{}%", (value * 100.0).round() as i32)
}

#[allow(dead_code)]
pub fn seconds(value: f64) -> String {
    format!("{:.1}s", value)
}
