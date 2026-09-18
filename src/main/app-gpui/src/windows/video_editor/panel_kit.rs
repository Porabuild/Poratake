//! The shared row vocabulary the video editor's side panels are built from —
//! `SettingsPanelHeader`, `Label`, the small selects, sliders and switches in
//! `renderer/components/video-editor/components`. The row atoms live in
//! `ui::rows` and are shared with the settings window; this module keeps the
//! panel layouts and the `VideoEditorWindow`-bound wiring on top of them.

use gpui::{div, prelude::*, px, AnyElement, Context, ElementId, SharedString, Styled};

use crate::theme::vars::ThemeVars;
use crate::ui::menu::MenuHandle;
use crate::ui::rows;
use crate::windows::video_editor::styles;
use crate::windows::video_editor::VideoEditorWindow;
use herogpui::components::SliderSize;
use herogpui::components::{Button, PickerItem, Select, Size, TabItem, Tabs, TabsVariant, Variant};

pub use crate::ui::rows::{error, hint, label, note};

pub fn scoped_id(key: &'static str, scope: Option<&str>) -> ElementId {
    match scope {
        Some(scope) => SharedString::from(format!("{key}-{scope}")).into(),
        None => SharedString::from(key).into(),
    }
}

pub fn panel(scroll: &gpui::ScrollHandle, children: Vec<AnyElement>) -> AnyElement {
    div()
        .relative()
        .size_full()
        .min_w_0()
        .child(
            div()
                .id("video-panel")
                .track_scroll(scroll)
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
                ),
        )
        .child(crate::windows::scrollbars::app_vertical(
            "video-panel-scrollbar",
            scroll,
        ))
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
            rows::title_desc_stack(
                title,
                description,
                herogpui::gpui::FontWeight::MEDIUM,
                theme,
            )
            .flex_1()
            .min_w_0(),
        );

    if let Some(enabled) = toggle {
        row = row.child(
            rows::switch(
                SharedString::from(format!("panel-toggle-{title}")),
                enabled,
                cx,
                on_toggle,
            )
            .size(Size::Sm),
        );
    }

    row.into_any_element()
}

pub fn empty_state(lines: &[&str], theme: &ThemeVars) -> AnyElement {
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
                .flex()
                .flex_col()
                .text_center()
                .text_size(px(crate::ui::chrome::TEXT_SM))
                .text_color(theme.muted_foreground)
                .children(
                    lines
                        .iter()
                        .map(|line| div().w_full().child(SharedString::from(line.to_string()))),
                ),
        )
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

pub fn setting_row(
    text: impl Into<SharedString>,
    control: AnyElement,
    theme: &ThemeVars,
) -> AnyElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .gap(px(8.0))
        .child(
            div()
                .flex_shrink_0()
                .text_size(px(crate::ui::chrome::TEXT_XS))
                .text_color(theme.muted_foreground)
                .child(text.into()),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .child(control),
        )
        .into_any_element()
}

pub fn switch_row(
    id: &'static str,
    text: &'static str,
    description: Option<&'static str>,
    checked: bool,
    theme: &ThemeVars,
    cx: &mut Context<VideoEditorWindow>,
    on_change: impl Fn(&mut VideoEditorWindow, bool, &mut Context<VideoEditorWindow>) + 'static,
) -> AnyElement {
    let left = match description {
        Some(description) => {
            rows::title_desc_stack(text, description, herogpui::gpui::FontWeight::MEDIUM, theme)
                .flex_1()
                .min_w_0()
                .into_any_element()
        }
        None => label(text, theme),
    };
    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .gap(px(8.0))
        .child(left)
        .child(rows::switch(id, checked, cx, on_change).size(Size::Sm))
        .into_any_element()
}

fn select_control(
    id: ElementId,
    value: &str,
    items: Vec<PickerItem>,
    cx: &mut Context<VideoEditorWindow>,
    on_change: impl Fn(&mut VideoEditorWindow, String, &mut Context<VideoEditorWindow>) + 'static,
) -> Select {
    let value = rows::selected_value(&items, value);
    Select::new(id, items)
        .recipe("compact")
        .value(value)
        .full_width(true)
        .on_selection_change(
            cx.listener(move |this, value: &Option<SharedString>, _window, cx| {
                let Some(value) = value else { return };
                on_change(this, value.to_string(), cx);
            }),
        )
}

#[allow(clippy::too_many_arguments)]
pub fn select_row(
    key: &'static str,
    scope: Option<&str>,
    text: &'static str,
    value: &str,
    options: &[(&'static str, &'static str)],
    _menu: &MenuHandle,
    theme: &ThemeVars,
    cx: &mut Context<VideoEditorWindow>,
    on_change: impl Fn(&mut VideoEditorWindow, String, &mut Context<VideoEditorWindow>) + 'static,
) -> AnyElement {
    let items = rows::picker_items(options.iter().copied());
    field(
        text,
        select_control(scoped_id(key, scope), value, items, cx, on_change).into_any_element(),
        theme,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn color_select_row(
    key: &'static str,
    text: &'static str,
    value: &str,
    options: &[(&'static str, &'static str)],
    theme: &ThemeVars,
    cx: &mut Context<VideoEditorWindow>,
    on_change: impl Fn(&mut VideoEditorWindow, String, &mut Context<VideoEditorWindow>) + 'static,
) -> AnyElement {
    let items = rows::picker_items(options.iter().copied());
    let selected_swatch = options
        .iter()
        .any(|(option, _)| *option == value)
        .then(|| crate::theme::color::Srgba::parse(value).to_hsla());
    field(
        text,
        select_control(scoped_id(key, None), value, items, cx, on_change)
            .placeholder("Custom")
            .item_leading(|key, _selected| {
                Some(
                    color_swatch(crate::theme::color::Srgba::parse(key).to_hsla())
                        .into_any_element(),
                )
            })
            .value_content(move |selection| {
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .text_size(px(crate::ui::chrome::TEXT_XS))
                    .children(selected_swatch.map(color_swatch))
                    .child(selection.default_children)
                    .into_any_element()
            })
            .into_any_element(),
        theme,
    )
}

const SWATCH_BORDER: &str = "#d1d5db";

fn color_swatch(color: gpui::Hsla) -> gpui::Div {
    div()
        .size(px(12.0))
        .flex_shrink_0()
        .rounded_full()
        .border_1()
        .border_color(crate::theme::color::Srgba::parse(SWATCH_BORDER).to_hsla())
        .bg(color)
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

fn slider(
    id: ElementId,
    key: &'static str,
    value: f64,
    min: f64,
    max: f64,
    cx: &mut Context<VideoEditorWindow>,
    on_change: impl Fn(&mut VideoEditorWindow, f64, &mut Context<VideoEditorWindow>) + 'static,
) -> herogpui::components::Slider {
    let step = styles::slider_step(key);
    debug_assert!(
        step.is_some(),
        "{key} has no entry in styles::VIDEO_SLIDER_STEPS"
    );
    let drag_view = cx.entity().downgrade();
    let drop_view = drag_view.clone();
    rows::slider_control(id, value, min, max, step.unwrap_or_default(), cx, on_change)
        .size(SliderSize::Sm)
        .on_drag_start(move |_window, cx| {
            let _ = drag_view.update(cx, |this, _cx| this.begin_slider_gesture());
        })
        .on_drag_end(move |_window, cx| {
            let _ = drop_view.update(cx, |this, cx| this.end_slider_gesture(cx));
        })
}

#[allow(clippy::too_many_arguments)]
pub fn slider_row(
    key: &'static str,
    scope: Option<&str>,
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
        .child(slider(
            scoped_id(key, scope),
            key,
            value,
            min,
            max,
            cx,
            on_change,
        ))
        .into_any_element()
}

#[allow(clippy::too_many_arguments)]
pub fn slider_inline_row(
    key: &'static str,
    scope: Option<&str>,
    text: Option<&'static str>,
    value: f64,
    min: f64,
    max: f64,
    display: String,
    theme: &ThemeVars,
    cx: &mut Context<VideoEditorWindow>,
    on_change: impl Fn(&mut VideoEditorWindow, f64, &mut Context<VideoEditorWindow>) + 'static,
) -> AnyElement {
    let mut row = div().flex().flex_row().items_center().gap(px(12.0));
    if let Some(text) = text {
        row = row.child(
            div()
                .w(px(48.0))
                .flex_shrink_0()
                .text_size(px(crate::ui::chrome::TEXT_XS))
                .text_color(theme.muted_foreground)
                .child(text),
        );
    }
    row.child(div().flex_1().min_w_0().child(slider(
        scoped_id(key, scope),
        key,
        value,
        min,
        max,
        cx,
        on_change,
    )))
    .child(
        div()
            .w(px(32.0))
            .flex_shrink_0()
            .text_right()
            .text_size(px(crate::ui::chrome::TEXT_XS))
            .text_color(theme.muted_foreground)
            .child(display),
    )
    .into_any_element()
}

#[allow(clippy::too_many_arguments)]
pub fn select_inline_row(
    key: &'static str,
    scope: Option<&str>,
    text: &'static str,
    value: &str,
    options: &[(&'static str, &'static str)],
    theme: &ThemeVars,
    cx: &mut Context<VideoEditorWindow>,
    on_change: impl Fn(&mut VideoEditorWindow, String, &mut Context<VideoEditorWindow>) + 'static,
) -> AnyElement {
    let items = rows::picker_items(options.iter().copied());
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(12.0))
        .child(
            div()
                .w(px(48.0))
                .flex_shrink_0()
                .text_size(px(crate::ui::chrome::TEXT_XS))
                .text_color(theme.muted_foreground)
                .child(text),
        )
        .child(div().flex_1().min_w_0().child(select_control(
            scoped_id(key, scope),
            value,
            items,
            cx,
            on_change,
        )))
        .into_any_element()
}

#[allow(clippy::too_many_arguments)]
pub fn select_setting_row(
    key: &'static str,
    text: &'static str,
    value: &str,
    options: &[(&'static str, &'static str)],
    theme: &ThemeVars,
    cx: &mut Context<VideoEditorWindow>,
    on_change: impl Fn(&mut VideoEditorWindow, String, &mut Context<VideoEditorWindow>) + 'static,
) -> AnyElement {
    let items = rows::picker_items(options.iter().copied());
    setting_row(
        text,
        div()
            .w(px(128.0))
            .child(select_control(
                scoped_id(key, None),
                value,
                items,
                cx,
                on_change,
            ))
            .into_any_element(),
        theme,
    )
}

pub fn tabs_setting_row(
    key: &'static str,
    text: &'static str,
    value: &str,
    options: &[(&'static str, &'static str)],
    theme: &ThemeVars,
    cx: &mut Context<VideoEditorWindow>,
    on_change: impl Fn(&mut VideoEditorWindow, String, &mut Context<VideoEditorWindow>) + 'static,
) -> AnyElement {
    setting_row(
        text,
        div()
            .w(px(128.0))
            .child(
                Tabs::new(
                    key,
                    options
                        .iter()
                        .map(|(value, label)| TabItem::new(*value, *label))
                        .collect(),
                    value,
                )
                .variant(TabsVariant::Secondary)
                .selected_key(value)
                .full_width(true)
                .on_selection_change(cx.listener(
                    move |this, value: &SharedString, _window, cx| {
                        on_change(this, value.to_string(), cx);
                    },
                )),
            )
            .into_any_element(),
        theme,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn slider_setting_row(
    key: &'static str,
    text: &'static str,
    value: f64,
    min: f64,
    max: f64,
    display: String,
    theme: &ThemeVars,
    cx: &mut Context<VideoEditorWindow>,
    on_change: impl Fn(&mut VideoEditorWindow, f64, &mut Context<VideoEditorWindow>) + 'static,
) -> AnyElement {
    setting_row(
        text,
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .child(div().w(px(128.0)).child(slider(
                scoped_id(key, None),
                key,
                value,
                min,
                max,
                cx,
                on_change,
            )))
            .child(
                div()
                    .w(px(20.0))
                    .flex_shrink_0()
                    .text_right()
                    .text_size(px(crate::ui::chrome::TEXT_XS))
                    .text_color(theme.muted_foreground)
                    .child(display),
            )
            .into_any_element(),
        theme,
    )
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
    label: impl Into<SharedString>,
    icon: &'static str,
    disabled: bool,
    theme: &ThemeVars,
    cx: &mut Context<VideoEditorWindow>,
    on_click: impl Fn(&mut VideoEditorWindow, &mut Context<VideoEditorWindow>) + 'static,
) -> AnyElement {
    let _ = theme;
    rows::icon_text_button(id, label, icon, 14.0, 8.0)
        .variant(Variant::Tertiary)
        .recipe("compact")
        .is_disabled(disabled)
        .full_width(true)
        .on_press(cx.listener(move |this, _event, _window, cx| on_click(this, cx)))
        .into_any_element()
}

pub fn tertiary_text_button(
    id: &'static str,
    label: impl Into<SharedString>,
    disabled: bool,
    danger: bool,
    cx: &mut Context<VideoEditorWindow>,
    on_click: impl Fn(&mut VideoEditorWindow, &mut Context<VideoEditorWindow>) + 'static,
) -> AnyElement {
    let mut button = Button::new(id)
        .variant(Variant::Tertiary)
        .recipe("compact")
        .label(label)
        .is_disabled(disabled)
        .full_width(true);
    if danger {
        button = button.recipe("danger-text");
    }
    button
        .on_press(cx.listener(move |this, _event, _window, cx| on_click(this, cx)))
        .into_any_element()
}

pub fn tertiary_spinner_button(
    id: &'static str,
    label: impl Into<SharedString>,
    cx: &mut Context<VideoEditorWindow>,
    on_click: impl Fn(&mut VideoEditorWindow, &mut Context<VideoEditorWindow>) + 'static,
) -> AnyElement {
    let label = label.into();
    Button::new(id)
        .variant(Variant::Tertiary)
        .recipe("compact")
        .label(label.clone())
        .is_disabled(true)
        .full_width(true)
        .content(move |_| {
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .child(crate::ui::icon::spinner_element(
                    "video-panel-spinner",
                    px(14.0),
                ))
                .child(label.clone())
                .into_any_element()
        })
        .on_press(cx.listener(move |this, _event, _window, cx| on_click(this, cx)))
        .into_any_element()
}

/// A hairline rule between groups inside a panel.
pub fn separator(_theme: &ThemeVars) -> AnyElement {
    herogpui::Separator::new().my(px(4.0)).into_any_element()
}

pub const LABELLED_SEPARATOR_RULE_HEIGHT: f32 = 1.0;
pub const LABELLED_SEPARATOR_PADDING_TOP: f32 = 16.0;
pub const LABELLED_SEPARATOR_LABEL_TOP: f32 = -10.0;
pub const LABELLED_SEPARATOR_LABEL_LINE: f32 = 16.0;
pub const LABELLED_SEPARATOR_LABEL_PADDING_X: f32 = 8.0;

pub fn labelled_separator(text: &'static str, theme: &ThemeVars) -> AnyElement {
    div()
        .relative()
        .w_full()
        .my(px(4.0))
        .border_t(px(LABELLED_SEPARATOR_RULE_HEIGHT))
        .border_color(theme.border)
        .pt(px(LABELLED_SEPARATOR_PADDING_TOP))
        .child(
            div()
                .absolute()
                .top(px(LABELLED_SEPARATOR_LABEL_TOP))
                .left_0()
                .right_0()
                .flex()
                .justify_center()
                .child(
                    div()
                        .px(px(LABELLED_SEPARATOR_LABEL_PADDING_X))
                        .bg(theme.muted_background)
                        .text_size(px(crate::ui::chrome::TEXT_XS))
                        .line_height(px(LABELLED_SEPARATOR_LABEL_LINE))
                        .text_color(theme.muted_foreground)
                        .child(text),
                ),
        )
        .into_any_element()
}

pub fn data_editor_section(
    title: &'static str,
    buttons: AnyElement,
    summary: Option<SharedString>,
    theme: &ThemeVars,
) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .child(label(title, theme))
        .child(buttons)
        .children(summary.map(|summary| hint(summary, theme)))
        .into_any_element()
}

pub fn hint_inline(text: impl Into<SharedString>, theme: &ThemeVars) -> AnyElement {
    div()
        .flex_none()
        .text_size(px(crate::ui::chrome::TEXT_XS))
        .text_color(theme.muted_foreground)
        .child(text.into())
        .into_any_element()
}

pub fn progress_label(text: impl Into<SharedString>, theme: &ThemeVars) -> AnyElement {
    div()
        .flex_none()
        .text_size(px(crate::ui::chrome::TEXT_SM))
        .font_weight(herogpui::gpui::FontWeight::MEDIUM)
        .text_color(theme.foreground)
        .child(text.into())
        .into_any_element()
}

pub fn percent(value: f64) -> String {
    format!("{}%", (value * 100.0).round() as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_labelled_separator_floats_its_label_above_the_rule() {
        let label_top = LABELLED_SEPARATOR_LABEL_TOP;
        let label_bottom = label_top + LABELLED_SEPARATOR_LABEL_LINE;
        let label_centre = label_top + LABELLED_SEPARATOR_LABEL_LINE / 2.0;
        assert!(label_top < 0.0);
        assert!(label_bottom > LABELLED_SEPARATOR_RULE_HEIGHT);
        assert!(label_centre < 0.0);
        assert!(label_bottom < LABELLED_SEPARATOR_PADDING_TOP);
    }

    #[test]
    fn the_labelled_separator_rule_sits_above_its_padding() {
        assert_eq!(LABELLED_SEPARATOR_RULE_HEIGHT, 1.0);
        assert_eq!(LABELLED_SEPARATOR_PADDING_TOP, 16.0);
        assert_eq!(LABELLED_SEPARATOR_LABEL_PADDING_X, 8.0);
        assert_eq!(LABELLED_SEPARATOR_LABEL_LINE, 16.0);
    }
}
