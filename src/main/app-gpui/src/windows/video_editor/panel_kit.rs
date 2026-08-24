//! The shared row vocabulary the video editor's side panels are built from —
//! `SettingsPanelHeader`, `Label`, the small selects, sliders and switches in
//! `renderer/components/video-editor/components/`.

use gpui::{div, prelude::*, px, AnyElement, Context, SharedString, Styled};

use crate::theme::vars::ThemeVars;
use crate::ui::button::{Button, ButtonSize, ButtonVariant};
use crate::ui::menu::MenuHandle;
use crate::ui::select::{Select, SelectOption};
use crate::ui::slider::Slider;
use crate::ui::switch::{Switch, SwitchSize};
use crate::ui::tabs::{TabItem, Tabs};
use crate::windows::video_editor::VideoEditorWindow;

pub fn panel(children: Vec<AnyElement>) -> AnyElement {
    div()
        .id("video-panel")
        .flex()
        .flex_col()
        .gap(px(crate::ui::chrome::VIDEO_PANEL_GAP))
        .size_full()
        .overflow_y_scroll()
        .p(px(crate::ui::chrome::VIDEO_PANEL_PAD))
        .children(children)
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
                        .text_size(px(crate::ui::chrome::SETTINGS_HEADER_DESC))
                        .text_color(theme.muted_foreground)
                        .child(description),
                ),
        );

    if let Some(enabled) = toggle {
        row = row.child(
            Switch::new(SharedString::from(format!("panel-toggle-{title}")), enabled)
                .size(SwitchSize::Sm)
                .on_change(cx.listener(move |this, value: &bool, _window, cx| {
                    on_toggle(this, *value, cx);
                })),
        );
    }

    row.into_any_element()
}

pub fn note(text: impl Into<SharedString>, theme: &ThemeVars) -> AnyElement {
    div()
        .text_size(px(crate::ui::chrome::TEXT_SM))
        .text_color(theme.muted_foreground)
        .child(text.into())
        .into_any_element()
}

pub fn hint(text: impl Into<SharedString>, theme: &ThemeVars) -> AnyElement {
    div()
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
            Switch::new(id, checked)
                .size(SwitchSize::Sm)
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
    menu: &MenuHandle,
    theme: &ThemeVars,
    cx: &mut Context<VideoEditorWindow>,
    on_change: impl Fn(&mut VideoEditorWindow, String, &mut Context<VideoEditorWindow>) + 'static,
) -> AnyElement {
    field(
        text,
        Select::new(id, menu.clone())
            .selected(value.to_string())
            .options(
                options
                    .iter()
                    .map(|(value, label)| SelectOption::new(*value, *label))
                    .collect(),
            )
            .full_width()
            .on_select(cx.listener(move |this, value: &SharedString, _window, cx| {
                on_change(this, value.to_string(), cx);
            }))
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
        Tabs::new(id)
            .items(
                options
                    .iter()
                    .map(|(value, label)| TabItem::new(*value, *label))
                    .collect(),
            )
            .selected(value.to_string())
            .full_width()
            .on_select(cx.listener(move |this, value: &SharedString, _window, cx| {
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
            Slider::new(id, value as f32, min as f32, max as f32)
                .small()
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
    theme: &ThemeVars,
    cx: &mut Context<VideoEditorWindow>,
    on_click: impl Fn(&mut VideoEditorWindow, &mut Context<VideoEditorWindow>) + 'static,
) -> AnyElement {
    Button::new(id)
        .variant(ButtonVariant::Ghost)
        .size(ButtonSize::Xs)
        .full_width()
        .label(label)
        .foreground(theme.muted_foreground)
        .on_click(cx.listener(move |this, _event, _window, cx| on_click(this, cx)))
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
        .variant(ButtonVariant::Tertiary)
        .size(ButtonSize::Xs)
        .full_width()
        .icon(icon)
        .label(label)
        .disabled(disabled)
        .on_click(cx.listener(move |this, _event, _window, cx| on_click(this, cx)))
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
        .variant(ButtonVariant::Primary)
        .size(ButtonSize::Sm)
        .full_width()
        .icon(icon)
        .label(label)
        .on_click(cx.listener(move |this, _event, _window, cx| on_click(this, cx)))
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
        .variant(ButtonVariant::Secondary)
        .size(ButtonSize::Xs)
        .full_width()
        .icon(icon)
        .label(label)
        .on_click(cx.listener(move |this, _event, _window, cx| on_click(this, cx)))
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
