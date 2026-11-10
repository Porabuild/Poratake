use gpui::{div, prelude::*, px, AnyElement, App, SharedString, Window};
use herogpui::components::{Button, ListBox, ListBoxItem, PickerItem, Popover, Select, Switch};
use herogpui::gpui;
use herogpui::{Placement, SelectionMode, Size, Variant};

use super::options::{self, EditorHandlers, EditorOption};
use super::tool_options::{number_badge, ToolOptionsState};
use crate::theme::vars::{active_theme, ThemeVars};
use crate::ui::colors::Tool;
use crate::ui::icon::{chevron_element, icon_element};

fn setting_row(label: &'static str, control: AnyElement, theme: &ThemeVars) -> AnyElement {
    div()
        .w_full()
        .border_t_1()
        .border_color(theme.border)
        .px(px(8.))
        .py(px(6.))
        .flex()
        .items_center()
        .justify_between()
        .text_size(px(12.))
        .line_height(px(16.))
        .child(div().text_color(theme.muted_foreground).child(label))
        .child(control)
        .into_any_element()
}

fn select(
    id: &'static str,
    label: &'static str,
    width: f32,
    choices: Vec<(SharedString, SharedString, EditorOption)>,
    value: SharedString,
    handlers: &EditorHandlers,
    window: &Window,
) -> Select {
    let minimum_width = choices
        .iter()
        .map(|(_, label, _)| {
            super::toolbar_select::text_width(label, px(12.), window)
                + crate::ui::chrome::TOOL_POPOVER_PANEL_PAD * 2.0
                + crate::ui::chrome::TOOL_POPOVER_ROW_PAD
                + crate::ui::chrome::SELECT_INDICATOR_GUTTER
        })
        .fold(width, f32::max)
        .ceil();
    let current_label = choices
        .iter()
        .find(|(key, _, _)| *key == value)
        .map(|(_, label, _)| label.clone())
        .unwrap_or_else(|| value.clone());
    let items = choices
        .iter()
        .map(|(key, label, _)| PickerItem::new(key.clone(), label.clone()))
        .collect();
    let on_option = handlers.on_option.clone();
    Select::new(id, items)
        .recipe("tool-popover")
        .aria_label(label)
        .panel_min_width(px(minimum_width))
        .indicator(|selected| {
            if selected {
                icon_element("check", px(10.))
            } else {
                div().into_any_element()
            }
        })
        .value(Some(value))
        .value_content(move |_| {
            div()
                .flex_1()
                .text_center()
                .font_weight(gpui::FontWeight::NORMAL)
                .line_height(px(16.))
                .child(current_label.clone())
                .into_any_element()
        })
        .sx(move |el| el.w(px(width)))
        .on_selection_change(move |key, window, cx| {
            if let Some((_, _, option)) = choices
                .iter()
                .find(|(choice, _, _)| Some(choice) == key.as_ref())
            {
                on_option(option.clone(), window, cx);
            }
        })
}

pub(super) fn render(
    id: &'static str,
    state: &ToolOptionsState,
    handlers: &EditorHandlers,
    content: AnyElement,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let theme = active_theme(cx);
    let open_state = window.use_keyed_state((gpui::ElementId::from(id), "open"), cx, |_, _| false);
    let open = *open_state.read(cx);
    let (label, width, selected, rows): (_, _, SharedString, Vec<_>) = match state.tool {
        Tool::Number => (
            "Number options",
            192.,
            state.number_style.clone(),
            options::NUMBER_STYLES
                .iter()
                .map(|(key, label)| (*key, *label))
                .collect(),
        ),
        Tool::Text => (
            "Text options",
            160.,
            state.text_font_family.clone(),
            options::FONT_FAMILIES
                .iter()
                .map(|(key, label)| (*key, *label))
                .collect(),
        ),
        Tool::Redact => (
            "Redaction options",
            160.,
            state.redact_style.clone(),
            options::REDACT_STYLES
                .iter()
                .map(|(key, label, _)| (*key, *label))
                .collect(),
        ),
        _ => return div().into_any_element(),
    };
    let tool = state.tool;
    let row_theme = theme.clone();
    let on_option = handlers.on_option.clone();
    let list = ListBox::new(
        (gpui::ElementId::from(id), "choices"),
        rows.iter()
            .map(|(key, label)| ListBoxItem::new(*key, *label))
            .collect(),
    )
    .selection_mode(SelectionMode::Single)
    .selected_key(selected)
    .disallow_empty_selection(true)
    .padding(px(4.))
    .gap(px(4.))
    .row_padding_x(px(8.))
    .indicator(|selected| {
        if selected {
            icon_element("check", px(10.))
        } else {
            div().into_any_element()
        }
    })
    .item_content(move |key, _| {
        let label = rows
            .iter()
            .find(|(value, _)| *value == key.as_ref())
            .map(|(_, label)| *label)
            .unwrap_or_default();
        let mut row = div()
            .flex()
            .flex_1()
            .min_w_0()
            .items_center()
            .gap(px(12.))
            .font_weight(if tool == Tool::Text {
                gpui::FontWeight::MEDIUM
            } else {
                gpui::FontWeight::NORMAL
            });
        match tool {
            Tool::Number => {
                row = row
                    .child(number_badge(
                        options::number_preview_glyph(key),
                        20.,
                        &row_theme,
                    ))
                    .child(label)
            }
            Tool::Text => {
                row = row
                    .font_family(super::text_render::ui_family(key))
                    .child(div().w(px(64.)).child(label))
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(row_theme.muted_foreground)
                            .child("Aa"),
                    )
            }
            Tool::Redact => {
                let icon = options::REDACT_STYLES
                    .iter()
                    .find(|(value, _, _)| *value == key.as_ref())
                    .map(|(_, _, icon)| *icon)
                    .unwrap_or("square");
                row = row.child(icon_element(icon, px(16.))).child(label);
            }
            _ => {}
        }
        let probe = format!("tool-choice-{}-{key}", tool.id());
        row.debug_selector(move || probe.clone()).into_any_element()
    })
    .on_selection_change(move |keys, window, cx| {
        let Some(key) = keys.iter().next() else {
            return;
        };
        let option = match tool {
            Tool::Number => EditorOption::NumberStyle(key.clone()),
            Tool::Text => EditorOption::TextFontFamily(key.clone()),
            Tool::Redact => EditorOption::RedactStyle(key.clone()),
            _ => return,
        };
        on_option(option, window, cx);
    });
    let mut body = div()
        .debug_selector(move || format!("{id}-body"))
        .w_full()
        .flex()
        .flex_col()
        .child(list);
    match tool {
        Tool::Number => {
            let choices = options::NUMBER_START_VALUES
                .iter()
                .map(|value| {
                    (
                        value.to_string().into(),
                        options::number_display_value(*value, &state.number_style).into(),
                        EditorOption::NumberStartValue(*value),
                    )
                })
                .collect();
            body = body.child(setting_row(
                "Starting:",
                select(
                    "number-start",
                    "Starting number",
                    56.,
                    choices,
                    state.number_start_value.to_string().into(),
                    handlers,
                    window,
                )
                .into_any_element(),
                &theme,
            ));
            let choices = options::NUMBER_SIZES
                .iter()
                .map(|(key, label)| {
                    (
                        (*key).into(),
                        (*label).into(),
                        EditorOption::NumberSize((*key).into()),
                    )
                })
                .collect();
            body = body.child(setting_row(
                "Size:",
                select(
                    "number-size",
                    "Number size",
                    96.,
                    choices,
                    state.number_size.clone(),
                    handlers,
                    window,
                )
                .into_any_element(),
                &theme,
            ));
        }
        Tool::Text => {
            let choices = options::FONT_SIZES
                .iter()
                .map(|value| {
                    (
                        value.to_string().into(),
                        value.to_string().into(),
                        EditorOption::TextFontSize(*value),
                    )
                })
                .collect();
            body = body.child(setting_row(
                "Size:",
                select(
                    "text-size",
                    "Font size",
                    64.,
                    choices,
                    state.text_font_size.to_string().into(),
                    handlers,
                    window,
                )
                .value_content({
                    let family = super::text_render::ui_family(&state.text_font_family);
                    let value = state.text_font_size.to_string();
                    move |_| {
                        div()
                            .flex_1()
                            .text_center()
                            .font_family(family)
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .line_height(px(16.))
                            .child(value.clone())
                            .into_any_element()
                    }
                })
                .item_content({
                    let family = super::text_render::ui_family(&state.text_font_family);
                    move |key, _, _, _| {
                        div()
                            .flex_1()
                            .font_family(family)
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .line_height(px(16.))
                            .child(key.clone())
                            .into_any_element()
                    }
                })
                .into_any_element(),
                &theme,
            ));
            let on_option = handlers.on_option.clone();
            body = body.child(setting_row(
                "Background:",
                Switch::new("text-background")
                    .size(Size::Sm)
                    .control_scale(0.75)
                    .aria_label("Text background")
                    .is_selected(state.text_background)
                    .on_change(move |value, window, cx| {
                        on_option(EditorOption::TextBackground(*value), window, cx)
                    })
                    .into_any_element(),
                &theme,
            ));
        }
        Tool::Redact if state.redact_style.as_ref() != "blackout" => {
            let choices = options::REDACT_INTENSITIES
                .iter()
                .map(|value| {
                    (
                        value.to_string().into(),
                        value.to_string().into(),
                        EditorOption::RedactIntensity(*value),
                    )
                })
                .collect();
            body = body.child(setting_row(
                "Intensity:",
                select(
                    "redact-intensity",
                    "Redaction intensity",
                    56.,
                    choices,
                    state.redact_intensity.to_string().into(),
                    handlers,
                    window,
                )
                .into_any_element(),
                &theme,
            ));
        }
        _ => {}
    }
    let change = std::rc::Rc::new(move |value: &bool, window: &mut Window, cx: &mut App| {
        open_state.update(cx, |open, cx| {
            *open = *value;
            cx.notify();
        });
        if *value {
            let held = open_state.clone();
            window.on_next_frame(move |window, cx| {
                if *held.read(cx) {
                    window.focus_next(cx);
                }
            });
        }
    });
    let press_change = change.clone();
    let trigger = Button::new((gpui::ElementId::from(id), "button"))
        .aria_label(label)
        .on_press(move |_, window, cx| {
            press_change(&!open, window, cx);
            cx.stop_propagation();
        })
        .variant(Variant::Secondary)
        .height(px(28.))
        .padding_x(px(8.))
        .radius(px(crate::ui::chrome::TOOL_OPTION_RADIUS))
        .child(content)
        .child(
            div()
                .text_color(theme.muted_foreground)
                .child(chevron_element(px(14.), open)),
        );
    let popover = Popover::new(trigger)
        .id(id)
        .aria_label(label)
        .panel_width(px(width))
        .placement(Placement::Bottom)
        .padding(px(0.))
        .radius(px(6.))
        .is_open(open)
        .on_open_change(move |value, window, cx| change(value, window, cx))
        .child(body)
        .into_any_element();
    div()
        .flex()
        .debug_selector(move || id.into())
        .child(popover)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use herogpui::test::TestWindowExt;
    use std::sync::Arc;

    #[herogpui::test]
    fn tool_popover_keeps_live_values_and_nested_escape_ownership(cx: &mut gpui::TestAppContext) {
        let scratch = tempfile::tempdir().unwrap();
        let store = Arc::new(
            crate::config::store::ConfigStore::load_at(scratch.path().join("config.json")).unwrap(),
        );
        store.update(|config| config.editor.last_tool = "number".into());
        cx.update(|cx| {
            crate::state::set_test_state(cx, store);
            crate::editor::actions::init_bindings(cx);
            herogpui::theme::set_reduce_motion(true, cx);
        });
        let path = scratch.path().join("capture.png");
        image::RgbaImage::from_pixel(600, 400, image::Rgba([10, 20, 30, 255]))
            .save(&path)
            .unwrap();
        let (editor, cx) = cx.add_window_view(|window, cx| {
            super::super::window::EditorWindow::from_file(path.to_str().unwrap(), window, cx)
        });
        cx.settle();
        cx.click("tool-option-number");
        cx.update(|window, cx| window.simulate_next_frame(cx));
        cx.settle();
        cx.press("h");
        assert_eq!(editor.read_with(cx, |editor, _| editor.tool), Tool::Number);
        cx.press("down enter");
        assert_eq!(
            editor.read_with(cx, |editor, _| editor.number_style.clone()),
            "alpha-upper"
        );
        let body = cx.expect("tool-option-number-body");
        assert_eq!(body.size, gpui::size(px(192.), px(238.)));
        cx.click("tool-choice-number-roman");
        assert_eq!(
            editor.read_with(cx, |editor, _| editor.number_style.clone()),
            "roman"
        );
        assert!(cx.debug_bounds("tool-choice-number-numeric").is_some());
        cx.click("select-trigger-Name(\"number-start\")");
        cx.press("down enter");
        assert_eq!(
            editor.read_with(cx, |editor, _| editor.number_start_value),
            2.
        );
        assert!(cx.debug_bounds("tool-choice-number-numeric").is_some());
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(110));
        cx.settle();
        cx.click("select-trigger-Name(\"number-size\")");
        cx.press("escape");
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(110));
        cx.settle();
        assert!(cx.debug_bounds("tool-choice-number-numeric").is_some());
        cx.press("escape");
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(110));
        cx.settle();
        assert!(cx.debug_bounds("tool-choice-number-numeric").is_none());
        cx.press("enter");
        assert!(cx.find("tool-choice-number-numeric").is_some());
        cx.press("escape");
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(110));
        cx.settle();
        cx.update(|_, cx| {
            editor.update(cx, |editor, cx| {
                editor.tool = Tool::Text;
                cx.notify();
            })
        });
        cx.click("tool-option-text");
        let text = cx.expect("tool-option-text-body");
        assert_eq!(text.size, gpui::size(px(160.), px(190.)));
        cx.press("escape");
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(110));
        cx.settle();
        cx.update(|_, cx| {
            editor.update(cx, |editor, cx| {
                editor.tool = Tool::Redact;
                cx.notify();
            })
        });
        cx.click("tool-option-redact");
        cx.click("tool-choice-redact-blackout");
        let blackout = cx.expect("tool-option-redact-body");
        assert_eq!(blackout.size.height, px(124.));
        cx.click("tool-choice-redact-pixelate");
        assert_eq!(cx.expect("tool-option-redact-body").size.height, px(161.));
    }
}
