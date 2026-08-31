//! Port of `renderer/components/settings/setting-item-renderer.tsx` — one row
//! per registry item, rendered from the item's control shape.

use gpui::{div, prelude::*, px, AnyElement, Context, SharedString, Styled};

use crate::theme::vars::ThemeVars;
use crate::ui::button::{Button, ButtonSize, ButtonVariant};
use crate::ui::chrome;
use crate::ui::select::{Select, SelectOption};
use crate::ui::slider::Slider;
use crate::ui::switch::Switch;
use crate::windows::settings::registry::{Control, Item, PathKind};
use crate::windows::settings::SettingsWindow;

/// `SettingsSelect` is `w-40 shrink-0`.
const CONTROL_WIDTH: f32 = 160.0;
#[allow(dead_code)]
const INPUT_WIDTH: f32 = 280.0;
/// `space-y-3` between the label row, the slider and the description.
const STACK_GAP: f32 = 12.0;
/// `py-2` on the stacked rows (slider, input, path picker, headers).
const STACK_PAD_Y: f32 = 8.0;

/// Key under which the naming-pattern token list records whether it is shown.
/// Shares `extras_open` with the page-level disclosures.
const NAMING_TOKENS_KEY: &str = "storage.namingPattern.tokens";

impl SettingsWindow {
    pub(crate) fn render_setting(
        &mut self,
        item: &Item,
        theme: &ThemeVars,
        compact: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match &item.control {
            Control::NamingPattern => return self.naming_pattern_row(item, theme, cx),
            Control::RestHeaders => return self.rest_headers_row(item, theme, cx),
            Control::Input { hint, .. } => {
                // `<div className="grid gap-2 py-2">`: the label sits above a
                // full-width input, with the hint below it.
                let hint = *hint;
                let field = self.text_field_for(item, cx);
                return div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .py(px(STACK_PAD_Y))
                    .child(field_label(item.label, theme))
                    .child(field)
                    .when_some(hint, |el, hint| {
                        el.child(
                            div()
                                .text_size(px(chrome::TEXT_XS))
                                .text_color(theme.muted_foreground)
                                .child(hint),
                        )
                    })
                    .into_any_element();
            }
            _ => {}
        }

        let menu = self.menu.clone();
        let control: AnyElement = match &item.control {
            Control::Switch { get, set, disabled } => {
                let set = *set;
                let requires_accessibility = item.id == "screenshot.hideDesktopIcons";
                Switch::new(
                    SharedString::from(format!("{}-switch", item.id)),
                    get(self.config()),
                )
                .disabled(disabled.is_some_and(|predicate| predicate(self.config())))
                .on_change(cx.listener(move |this, value: &bool, _window, cx| {
                    let value = *value;
                    if value
                        && requires_accessibility
                        && !crate::system::permissions::accessibility_granted()
                    {
                        crate::system::permissions::open_accessibility_preferences();
                        return;
                    }
                    this.mutate(cx, move |config| set(config, value));
                }))
                .into_any_element()
            }
            Control::Select { options, get, set } => {
                let set = *set;
                Select::new(SharedString::from(format!("{}-select", item.id)), menu)
                    .selected(get(self.config()))
                    .options(
                        options
                            .resolve()
                            .into_iter()
                            .map(|(value, label)| SelectOption::new(value, label))
                            .collect(),
                    )
                    .width(px(CONTROL_WIDTH))
                    .on_select(cx.listener(move |this, value: &SharedString, _window, cx| {
                        let value = value.to_string();
                        this.mutate(cx, move |config| set(config, &value));
                    }))
                    .into_any_element()
            }
            Control::Slider {
                min,
                max,
                step,
                get,
                set,
            } => {
                // `<div className="space-y-3 py-2">`: the label and its value
                // share a row, the slider spans the full width beneath them,
                // and the description sits under that.
                let (set, step) = (*set, *step);
                let value = get(self.config());
                return div()
                    .flex()
                    .flex_col()
                    .gap(px(STACK_GAP))
                    .py(px(STACK_PAD_Y))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .child(field_label(item.label, theme))
                            .child(
                                div()
                                    .text_size(px(chrome::TEXT_SM))
                                    .text_color(theme.muted_foreground)
                                    .child(format!("{}", value.round() as i64)),
                            ),
                    )
                    .child(
                        Slider::new(
                            SharedString::from(format!("{}-slider", item.id)),
                            value as f32,
                            *min as f32,
                            *max as f32,
                        )
                        .on_change(cx.listener(
                            move |this, value: &f32, _window, cx| {
                                let snapped = if step > 0.0 {
                                    (*value as f64 / step).round() * step
                                } else {
                                    *value as f64
                                };
                                this.mutate(cx, move |config| set(config, snapped));
                            },
                        )),
                    )
                    .child(
                        div()
                            .text_size(px(chrome::TEXT_XS))
                            .text_color(theme.muted_foreground)
                            .child(item.description),
                    )
                    .into_any_element();
            }
            Control::Shortcut {
                get,
                set,
                single_key,
            } => {
                let set = *set;
                let value = get(self.config());
                let recording = self.is_recording_shortcut(item.id);
                crate::ui::shortcut_input::render(
                    item.id,
                    &value,
                    *single_key,
                    recording,
                    theme,
                    cx,
                    move |this, next, cx| {
                        this.mutate(cx, move |config| set(config, &next));
                    },
                )
            }
            // Returns its own stacked block -- label above field -- so it must
            // not go through `labelled`, which would lay the two out side by
            // side and add a second, registry-supplied label.
            Control::PathPicker { kind } => {
                return self.path_picker_row(item, *kind, theme, cx);
            }
            Control::CloudTestConnection => {
                let status = self.cloud_test_status();
                let mut row = div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(12.0))
                    .py(px(STACK_PAD_Y))
                    .child(
                        Button::new(SharedString::from(format!("{}-test", item.id)))
                            .variant(ButtonVariant::Outline)
                            .size(ButtonSize::Md)
                            .when_some(status.icon, |button, icon| {
                                button
                                    .icon(icon)
                                    .icon_size(px(chrome::TOOL_BUTTON_ICON))
                                    .icon_spinning(status.spinning)
                            })
                            .label("Test Connection")
                            .disabled(status.disabled)
                            .on_click(cx.listener(|this, _event, _window, cx| {
                                this.test_cloud_connection(cx)
                            })),
                    );
                if let Some((message, tone)) = status.message {
                    row = row.child(
                        div()
                            .text_size(px(chrome::TEXT_SM))
                            .text_color(tone)
                            .child(message),
                    );
                }
                if status.unconfigured {
                    row = row.child(
                        div()
                            .text_size(px(chrome::TEXT_XS))
                            .text_color(theme.muted_foreground)
                            .child("Fill in all required fields first"),
                    );
                }
                return row.into_any_element();
            }
            // Both render a stacked block -- label, description, full-width
            // select, then the test controls -- so neither goes through
            // `labelled`.
            Control::MicrophoneDevice => return self.microphone_row(item, theme, cx),
            Control::CameraDevice => return self.camera_row(item, theme, cx),
            Control::Input { .. } | Control::NamingPattern | Control::RestHeaders => {
                unreachable!("handled above")
            }
        };

        labelled(item, theme, control, compact)
    }

    /// `<div className="space-y-3 py-2">` with a `space-y-0.5` label block, the
    /// select, then a `flex items-center gap-3` row holding the test button and
    /// the level meter, and a hint paragraph while the test runs.
    fn microphone_row(
        &mut self,
        item: &Item,
        theme: &ThemeVars,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let select = self.device_select(
            "devices-microphone",
            crate::system::devices::DeviceKind::Microphone,
            cx,
        );
        let testing = crate::system::device_test::mic_active();
        let level = crate::system::device_test::level();

        let mut block = device_block(item, theme).child(select).child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(12.0))
                .child(
                    Button::new("devices-mic-test")
                        .variant(ButtonVariant::Secondary)
                        .size(ButtonSize::Sm)
                        .min_width(px(DEVICE_TEST_BUTTON_WIDTH))
                        .label(if testing { "Stop Test" } else { "Mic Test" })
                        .on_click(cx.listener(|this, _event, _window, cx| {
                            this.toggle_mic_test(cx);
                        })),
                )
                .child(level_meter(level, testing, theme)),
        );
        if testing {
            block = block.child(
                div()
                    .text_size(px(chrome::TEXT_XS))
                    .text_color(theme.muted_foreground)
                    .child(
                        "Speak into your microphone \u{2014} the meter should react to your voice",
                    ),
            );
        }
        block.into_any_element()
    }

    fn camera_row(&mut self, item: &Item, theme: &ThemeVars, cx: &mut Context<Self>) -> AnyElement {
        let select = self.device_select(
            "devices-camera",
            crate::system::devices::DeviceKind::Camera,
            cx,
        );
        let testing = crate::system::device_test::camera_active();
        let flipped = self.config().recording.camera.flipped;

        device_block(item, theme)
            .child(select)
            // `<div className="flex items-center justify-between gap-4">` with
            // its own label and description -- not a registry row.
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .gap(px(16.0))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .child(
                                div()
                                    .text_size(px(14.0))
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .text_color(theme.foreground)
                                    .child("Mirror camera"),
                            )
                            .child(
                                div()
                                    .text_size(px(chrome::TEXT_XS))
                                    .text_color(theme.muted_foreground)
                                    .child(
                                        "Flip the camera horizontally in previews and recordings",
                                    ),
                            ),
                    )
                    .child(
                        crate::ui::switch::Switch::new("devices-camera-mirror", flipped).on_change(
                            cx.listener(|this, checked: &bool, _window, cx| {
                                let flipped = *checked;
                                this.mutate(cx, move |config| {
                                    config.recording.camera.flipped = flipped;
                                });
                            }),
                        ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(12.0))
                    .child(
                        Button::new("devices-camera-test")
                            .variant(ButtonVariant::Secondary)
                            .size(ButtonSize::Sm)
                            .min_width(px(DEVICE_TEST_BUTTON_WIDTH))
                            .label(if testing { "Stop Test" } else { "Test Video" })
                            .on_click(cx.listener(|this, _event, _window, cx| {
                                this.toggle_camera_test(cx);
                            })),
                    )
                    .when(testing, |el| {
                        el.child(
                            div()
                                .text_size(px(chrome::TEXT_XS))
                                .text_color(theme.muted_foreground)
                                .child(
                                    "The camera preview opens in a floating window \u{2014} the \
                                     same one shown while recording",
                                ),
                        )
                    }),
            )
            .into_any_element()
    }

    fn device_select(
        &mut self,
        id: &'static str,
        kind: crate::system::devices::DeviceKind,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        use crate::system::devices::DeviceKind;

        let lists = self.device_lists(cx);
        let (devices, selected) = match kind {
            DeviceKind::Microphone => (
                lists.microphones.clone(),
                self.config().recording.selected_mic_id.clone(),
            ),
            DeviceKind::Camera => (
                lists.cameras.clone(),
                self.config().recording.camera.selected_device_id.clone(),
            ),
        };

        let options: Vec<SelectOption> =
            crate::system::devices::options_with_selection(&devices, selected.as_deref())
                .into_iter()
                .map(|(value, label)| SelectOption::new(value, label))
                .collect();
        let labels: std::collections::HashMap<String, String> = devices
            .iter()
            .map(|d| (d.id.clone(), d.label.clone()))
            .collect();

        Select::new(SharedString::from(id), self.menu.clone())
            .selected(selected.unwrap_or_default())
            .options(options)
            .full_width()
            .placeholder("System Default")
            .on_select(cx.listener(move |this, value: &SharedString, _window, cx| {
                let id = value.to_string();
                let selected = (!id.is_empty()).then_some(id.clone());
                let label = selected.as_ref().and_then(|id| labels.get(id).cloned());
                this.mutate(cx, move |config| match kind {
                    DeviceKind::Microphone => {
                        config.recording.selected_mic_id = selected.clone();
                        config.recording.selected_mic_name = label.clone();
                    }
                    DeviceKind::Camera => {
                        config.recording.camera.selected_device_id = selected.clone();
                        config.recording.camera.selected_device_name = label.clone();
                    }
                });
            }))
            .into_any_element()
    }

    fn path_picker_row(
        &mut self,
        item: &Item,
        kind: PathKind,
        theme: &ThemeVars,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let current = self.path_for(kind);
        let custom = !current.is_empty();
        // `<div className="space-y-2"><Label>Save Location</Label><div
        // className="flex gap-2">…`: the label sits *above* the field, and the
        // label text is hard-coded rather than taken from the registry. Laying
        // it out beside the field instead crushes it to nothing, because the
        // field row is `w-full`.
        let mut row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .w_full()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .rounded(px(6.0))
                    .bg(theme.field_background)
                    .px(px(10.0))
                    .py(px(7.0))
                    .text_size(px(12.0))
                    .text_color(if custom {
                        theme.field_foreground
                    } else {
                        theme.field_placeholder
                    })
                    .child(if custom {
                        current
                    } else {
                        "Default location".to_string()
                    }),
            )
            .child(
                Button::new(SharedString::from(format!("{}-browse", item.id)))
                    .variant(ButtonVariant::Tertiary)
                    .size(ButtonSize::Icon)
                    .icon("folder-open")
                    .tooltip("Choose folder")
                    .on_click(cx.listener(move |this, _event, _window, cx| {
                        this.pick_path(kind, cx);
                    })),
            );
        // The reset button only exists while a custom path is set.
        if custom {
            row = row.child(
                Button::new(SharedString::from(format!("{}-reset", item.id)))
                    .variant(ButtonVariant::Tertiary)
                    .size(ButtonSize::Icon)
                    .icon("rotate-ccw")
                    .tooltip("Reset to default")
                    .on_click(cx.listener(move |this, _event, _window, cx| {
                        this.reset_path(kind, cx);
                    })),
            );
        }

        div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(
                div()
                    .text_size(px(14.0))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(theme.foreground)
                    .child("Save Location"),
            )
            .child(row)
            .into_any_element()
    }

    fn naming_pattern_row(
        &mut self,
        // The row hard-codes its own label, exactly as `setting-item-renderer`
        // does, so nothing here comes from the registry entry.
        _item: &Item,
        theme: &ThemeVars,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let field = self.naming_pattern_field(cx);
        let pattern = self.config().storage.naming_pattern.clone();
        let error = crate::editor::filename::validate_naming_pattern(&pattern);
        let preview = crate::editor::filename::generate_filename(
            &pattern,
            "Screenshot",
            "png",
            chrono::Local::now(),
        );

        let mut tokens = div().flex().flex_wrap().gap(px(6.0));
        for token in crate::editor::filename::available_tokens(chrono::Local::now()) {
            let insert = token.token;
            tokens = tokens.child(
                Button::new(SharedString::from(format!("naming-token-{insert}")))
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Xs)
                    .radius(px(6.0))
                    .label(insert)
                    .tooltip(format!("{} \u{2192} {}", token.description, token.example))
                    .foreground(theme.muted_foreground)
                    .on_click(cx.listener(move |this, _event, _window, cx| {
                        this.append_naming_token(insert, cx);
                    })),
            );
        }

        // `<div className="space-y-2">` with a hard-coded label beside a help
        // icon -- no description line, and the token list lives behind the icon
        // rather than on the page. Clicking the icon reveals this shell's
        // clickable chips, so the default view is Electron's.
        let tokens_open = self.extras_open.contains(NAMING_TOKENS_KEY);
        let mut block = div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_size(px(14.0))
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(theme.foreground)
                            .child("Naming Pattern"),
                    )
                    .child(
                        Button::new("naming-pattern-help")
                            .variant(ButtonVariant::Ghost)
                            .size(ButtonSize::IconXs)
                            .icon("help-circle")
                            .tooltip("Available tokens")
                            .foreground(theme.muted_foreground)
                            .on_click(cx.listener(|this, _event, _window, cx| {
                                if !this.extras_open.remove(NAMING_TOKENS_KEY) {
                                    this.extras_open.insert(NAMING_TOKENS_KEY);
                                }
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .child(div().flex_1().min_w_0().child(field))
                    .child(
                        Button::new("naming-pattern-reset")
                            .variant(ButtonVariant::Tertiary)
                            .size(ButtonSize::Icon)
                            .icon("rotate-ccw")
                            .tooltip("Reset to default")
                            .on_click(cx.listener(|this, _event, _window, cx| {
                                this.reset_naming_pattern(cx);
                            })),
                    ),
            );
        if tokens_open {
            block = block.child(tokens);
        }
        // `{patternError && <p className="text-sm text-destructive">}` is its own
        // line, and the preview line is always shown.
        if let Some(message) = error {
            block = block.child(
                div()
                    .text_size(px(chrome::TEXT_SM))
                    .text_color(theme.danger)
                    .child(message.to_string()),
            );
        }
        block
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(4.0))
                    .text_size(px(chrome::TEXT_SM))
                    .text_color(theme.muted_foreground)
                    .child("Preview:")
                    .child(
                        // `<code className="rounded bg-muted px-1.5 py-0.5 text-xs">`.
                        div()
                            .rounded(px(crate::ui::chrome::RADIUS_MD))
                            .bg(theme.muted_background)
                            .px(px(6.0))
                            .py(px(2.0))
                            .text_size(px(chrome::TEXT_XS))
                            .font_family(crate::ui::colors::MONO_FONT)
                            .child(preview),
                    ),
            )
            .into_any_element()
    }

    fn rest_headers_row(
        &mut self,
        item: &Item,
        theme: &ThemeVars,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let count = self.config().cloud.rest.headers.len();
        let mut list = div().flex().flex_col().gap(px(6.0));
        for index in 0..count {
            let (key_field, value_field) = self.rest_header_fields(index, cx);
            list = list.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.0))
                    .child(div().flex_1().child(key_field))
                    .child(div().flex_1().child(value_field))
                    .child(
                        Button::new(SharedString::from(format!("rest-header-remove-{index}")))
                            .variant(ButtonVariant::Ghost)
                            .size(ButtonSize::IconXs)
                            .icon("trash-2")
                            .foreground(theme.danger)
                            .tooltip("Remove header")
                            .on_click(cx.listener(move |this, _event, _window, cx| {
                                this.remove_rest_header(index, cx);
                            })),
                    ),
            );
        }

        stacked(item, theme)
            .child(list)
            .child(
                Button::new("rest-header-add")
                    .variant(ButtonVariant::Secondary)
                    .size(ButtonSize::Xs)
                    .icon("plus")
                    .label("Add header")
                    .on_click(cx.listener(|this, _event, _window, cx| this.add_rest_header(cx))),
            )
            .into_any_element()
    }
}

/// The `<Label className="text-sm">` every settings row leads with.
fn field_label(text: &'static str, theme: &ThemeVars) -> AnyElement {
    div()
        .text_size(px(chrome::TEXT_SM))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(theme.foreground)
        .child(text)
        .into_any_element()
}

/// `<div className="flex items-center justify-between gap-4">`. The vertical
/// rhythm comes from the page (`space-y-4`), not from the row.
/// Every row's label is `<Label>`, i.e. `font-medium` -- except a shortcut row,
/// which passes `className="text-sm font-normal"` and so overrides it.
fn label_weight(item: &Item) -> gpui::FontWeight {
    if matches!(item.control, Control::Shortcut { .. }) {
        gpui::FontWeight::NORMAL
    } else {
        gpui::FontWeight::MEDIUM
    }
}

fn labelled(item: &Item, theme: &ThemeVars, control: AnyElement, compact: bool) -> AnyElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .gap(px(16.0))
        .when(compact, |el| el.min_h(px(40.0)).py(px(4.0)))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .text_size(px(14.0))
                        .font_weight(label_weight(item))
                        .text_color(theme.foreground)
                        .child(item.label),
                )
                .when(!compact, |el| {
                    el.child(
                        div()
                            .text_size(px(12.0))
                            .text_color(theme.muted_foreground)
                            .child(item.description),
                    )
                }),
        )
        .child(control)
        .into_any_element()
}

fn stacked(item: &Item, theme: &ThemeVars) -> gpui::Div {
    div().flex().flex_col().gap(px(8.0)).child(
        div()
            .flex()
            .flex_col()
            .gap(px(2.0))
            .child(
                div()
                    .text_size(px(14.0))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(theme.foreground)
                    .child(item.label),
            )
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(theme.muted_foreground)
                    .child(item.description),
            ),
    )
}

/// `className="w-24 shrink-0"` on both test buttons.
const DEVICE_TEST_BUTTON_WIDTH: f32 = 96.0;

/// `<div className="space-y-3 py-2">` with the `space-y-0.5` label block both
/// device settings open with.
fn device_block(item: &Item, theme: &ThemeVars) -> gpui::Div {
    div().flex().flex_col().gap(px(12.0)).py(px(8.0)).child(
        div()
            .flex()
            .flex_col()
            .gap(px(2.0))
            .child(
                div()
                    .text_size(px(14.0))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(theme.foreground)
                    .child(item.label),
            )
            .child(
                div()
                    .text_size(px(chrome::TEXT_XS))
                    .text_color(theme.muted_foreground)
                    .child(item.description),
            ),
    )
}

/// `level-meter.tsx`: 32 segments, `h-5 flex-1 gap-0.5`, each `rounded-sm` and
/// either `bg-primary` or `bg-muted`.
pub(crate) const LEVEL_METER_SEGMENTS: usize = 32;

pub(crate) fn filled_segments(level: f32, active: bool) -> usize {
    if !active {
        return 0;
    }
    ((level * LEVEL_METER_SEGMENTS as f32).round() as usize).min(LEVEL_METER_SEGMENTS)
}

fn level_meter(level: f32, active: bool, theme: &ThemeVars) -> gpui::Div {
    let filled = filled_segments(level, active);
    let mut meter = div()
        .flex()
        .flex_row()
        .items_center()
        .h(px(20.0))
        .flex_1()
        .gap(px(2.0));
    for index in 0..LEVEL_METER_SEGMENTS {
        meter = meter.child(
            div()
                .h_full()
                .flex_1()
                .rounded(px(crate::ui::chrome::RADIUS_SM))
                .bg(if index < filled {
                    theme.primary
                } else {
                    theme.muted_background
                }),
        );
    }
    meter
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `filled = active ? Math.round(level * SEGMENT_COUNT) : 0`. The clamp is
    /// what keeps a level the daemon reports above 1.0 from asking for more
    /// segments than the meter has.
    #[test]
    fn the_level_meter_fills_the_way_the_react_one_does() {
        assert_eq!(filled_segments(0.0, true), 0);
        assert_eq!(filled_segments(0.5, true), 16);
        assert_eq!(filled_segments(1.0, true), LEVEL_METER_SEGMENTS);
        assert_eq!(filled_segments(2.0, true), LEVEL_METER_SEGMENTS);
        // Rounding, not truncation: 0.51 * 32 = 16.32 -> 16, 0.52 * 32 = 16.64 -> 17.
        assert_eq!(filled_segments(0.51, true), 16);
        assert_eq!(filled_segments(0.52, true), 17);
        // Inactive reads empty no matter what the last level was.
        assert_eq!(filled_segments(0.9, false), 0);
    }

    /// `shortcut-input.tsx` renders `<Label className="text-sm font-normal">`,
    /// overriding `Label`'s own `font-medium`; every other row keeps it. Both
    /// sources are read here so a change to either fails a test rather than
    /// only showing up in a screenshot.
    #[test]
    fn only_shortcut_rows_drop_to_the_normal_weight() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .expect("repository root")
            .to_path_buf();
        let shortcut_input = std::fs::read_to_string(
            root.join("src/renderer/components/settings/shortcut-input.tsx"),
        )
        .expect("read shortcut-input.tsx");
        assert!(
            shortcut_input.contains(r#"<Label className="text-sm font-normal">"#),
            "shortcut-input.tsx no longer overrides the label weight"
        );
        let label = std::fs::read_to_string(root.join("src/renderer/components/ui/label.tsx"))
            .expect("read label.tsx");
        assert!(
            label.contains("text-sm font-medium"),
            "`Label` no longer defaults to font-medium"
        );

        let items = crate::windows::settings::registry::items();
        let mut shortcuts = 0;
        let mut others = 0;
        for item in &items {
            if matches!(item.control, Control::Shortcut { .. }) {
                shortcuts += 1;
                assert_eq!(
                    label_weight(item),
                    gpui::FontWeight::NORMAL,
                    "{} is a shortcut row",
                    item.id
                );
            } else {
                others += 1;
                assert_eq!(
                    label_weight(item),
                    gpui::FontWeight::MEDIUM,
                    "{} is not a shortcut row",
                    item.id
                );
            }
        }
        assert!(
            shortcuts > 5 && others > 5,
            "{shortcuts} shortcuts, {others} others"
        );
    }
}
