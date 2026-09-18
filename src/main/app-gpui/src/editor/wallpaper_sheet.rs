use std::rc::Rc;

use gpui::{
    div, linear_color_stop, linear_gradient, prelude::*, px, Animation, AnimationExt, AnyElement,
    App, ClickEvent, ElementId, SharedString, Styled, Window,
};
use herogpui::gpui;

use crate::config::schema::{CustomBackground, CustomBackgroundData};
use crate::editor::background::{BackgroundPreviews, PreviewState};
use crate::editor::options::{EditorHandlers, EditorOption};
use crate::editor::wallpaper::{self, WallpaperSettings};
use crate::editor::window::{
    BackgroundDraft, BackgroundDraftType, BACKGROUND_MAX_COLORS, BACKGROUND_MIN_COLORS,
    BACKGROUND_PALETTE,
};
use crate::theme::color::Srgba;
use crate::theme::vars::{active_theme, ThemeVars};
use crate::ui::chrome;
use crate::ui::icon::{icon_element, ICON_MD};
use crate::ui::icon_button;
use herogpui::components::{Button, Select, Size, Switch, TextField, Variant};
use herogpui::components::{Slider, SliderSize};
use herogpui::Separator;

pub const SHEET_ANIMATION_MS: u64 = 300;

pub struct SheetState<'a> {
    pub wallpaper: &'a WallpaperSettings,
    pub has_layers: bool,
    pub preset_id: &'a str,
    pub draft: Option<&'a BackgroundDraft>,
    pub previews: &'a BackgroundPreviews,
    pub preset_draft: Option<PresetDraftView<'a>>,
    pub closing: bool,
}

pub struct PresetDraftView<'a> {
    pub field: &'a gpui::Entity<herogpui::components::InputState>,
    pub name: &'a str,
}

pub fn render(
    state: &SheetState<'_>,
    handlers: &EditorHandlers,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let theme = active_theme(cx);
    let config = crate::state::state(cx).config.get();
    let wallpaper_config = config.wallpaper;
    let wallpaper = state.wallpaper;

    if let Some(draft) = state.draft {
        return sheet_shell(
            background_editor_panel(draft, handlers, &theme, window, cx),
            state.closing,
            &theme,
        );
    }

    let content = div()
        .flex()
        .flex_col()
        .w_full()
        .gap(px(chrome::WALLPAPER_SHEET_GAP))
        .child(header(handlers, &theme))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(chrome::WALLPAPER_SHEET_INNER_GAP))
                .child(preset_manager(
                    state,
                    &wallpaper_config.presets,
                    wallpaper_config.default_preset_id.as_deref(),
                    handlers,
                    &theme,
                ))
                .child(Separator::new())
                .child(backgrounds_section(
                    wallpaper,
                    &wallpaper_config.custom_backgrounds,
                    state.previews,
                    handlers,
                    &theme,
                    window,
                    cx,
                ))
                .child(Separator::new())
                .child(aspect_row(wallpaper, handlers, &theme))
                .child(Separator::new())
                .child(balance_row(wallpaper, handlers, &theme))
                .child(slider_control(
                    "wallpaper-padding",
                    "Padding",
                    wallpaper.padding,
                    0.0,
                    wallpaper::PADDING_MAX,
                    false,
                    handlers,
                    EditorOption::WallpaperPadding,
                    &theme,
                ))
                .child(slider_control(
                    "wallpaper-inset",
                    "Inset",
                    wallpaper.inset,
                    0.0,
                    wallpaper::INSET_MAX,
                    false,
                    handlers,
                    EditorOption::WallpaperInset,
                    &theme,
                ))
                .child(slider_control(
                    "wallpaper-corners",
                    "Corners",
                    wallpaper.corners,
                    0.0,
                    wallpaper::CORNERS_MAX,
                    false,
                    handlers,
                    EditorOption::WallpaperCorners,
                    &theme,
                ))
                .child(slider_control(
                    "wallpaper-shadow",
                    "Shadow",
                    wallpaper.shadow,
                    0.0,
                    wallpaper::SHADOW_MAX,
                    false,
                    handlers,
                    EditorOption::WallpaperShadow,
                    &theme,
                ))
                .child(spacing_control(
                    wallpaper,
                    state.has_layers,
                    handlers,
                    &theme,
                ))
                .child(Separator::new())
                .child(window_frames(wallpaper, handlers, &theme)),
        );
    sheet_shell(content.into_any_element(), state.closing, &theme)
}

fn sheet_shell(content: AnyElement, closing: bool, theme: &ThemeVars) -> AnyElement {
    let sheet = div()
        .id("wallpaper-sheet")
        .flex()
        .flex_col()
        .flex_none()
        .h_full()
        .w(px(chrome::WALLPAPER_SHEET_WIDTH))
        .gap(px(chrome::WALLPAPER_SHEET_GAP))
        .overflow_y_scroll()
        .border_r_1()
        .border_color(theme.border)
        .bg(theme.popover)
        .p(px(chrome::WALLPAPER_SHEET_PAD))
        .shadow_lg()
        .child(content)
        .with_animation(
            ElementId::Name(if closing {
                "wallpaper-sheet-exit".into()
            } else {
                "wallpaper-sheet-enter".into()
            }),
            Animation::new(std::time::Duration::from_millis(SHEET_ANIMATION_MS))
                .with_easing(crate::ui::primitives::cubic_bezier(0.42, 0.0, 0.58, 1.0)),
            move |sheet, delta| {
                let travel = if closing { delta } else { 1.0 - delta };
                sheet.left(px(-chrome::WALLPAPER_SHEET_WIDTH * travel))
            },
        );

    div()
        .id("wallpaper-sheet-slot")
        .relative()
        .flex_none()
        .h_full()
        .w(px(chrome::WALLPAPER_SHEET_WIDTH))
        .overflow_hidden()
        .child(sheet)
        .into_any_element()
}

/// Port of `background-editor.tsx` — authors a custom gradient (up to five
/// colors, angle, palette picker, live preview) or picks a custom image.
fn background_editor_panel(
    draft: &BackgroundDraft,
    handlers: &EditorHandlers,
    theme: &ThemeVars,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let close = handlers.option(EditorOption::WallpaperEditorClose);
    let save = handlers.option(EditorOption::WallpaperEditorSave);
    let is_gradient = draft.draft_type == BackgroundDraftType::Gradient;
    let mut panel = div()
        .flex()
        .flex_col()
        .w_full()
        .gap(px(16.0))
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(chrome::TEXT_SM))
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(theme.foreground)
                        .child(if draft.id.is_some() {
                            "Edit Background"
                        } else {
                            "New Background"
                        }),
                )
                .child(
                    icon_button::compact_sm_muted("wallpaper-editor-close", "x")
                        .on_press(move |_event, window, cx| close(window, cx)),
                ),
        )
        .child(editor_preview(draft, theme))
        .child(editor_type_toggle(draft, handlers, theme, window, cx));
    panel = if is_gradient {
        panel
            .child(editor_colors(draft, handlers, theme))
            .child(editor_angle(draft, handlers, theme))
    } else {
        panel.child(editor_image(draft, handlers, theme))
    };
    let save_disabled = !is_gradient && draft.image_url.is_none();
    panel
        .child(
            div()
                .flex()
                .flex_row()
                .gap(px(8.0))
                .child(
                    div().flex_1().child(
                        Button::new("wallpaper-editor-cancel")
                            .variant(Variant::Ghost)
                            .size(Size::Sm)
                            .label("Cancel")
                            .on_press({
                                let close = handlers.option(EditorOption::WallpaperEditorClose);
                                move |_event, window, cx| close(window, cx)
                            }),
                    ),
                )
                .child(
                    div().flex_1().child(
                        Button::new("wallpaper-editor-save")
                            .size(Size::Sm)
                            .label(if draft.id.is_some() { "Update" } else { "Save" })
                            .is_disabled(save_disabled)
                            .on_press(move |_event, window, cx| save(window, cx)),
                    ),
                ),
        )
        .into_any_element()
}

fn editor_preview(draft: &BackgroundDraft, theme: &ThemeVars) -> AnyElement {
    if draft.draft_type == BackgroundDraftType::Gradient {
        let from = Srgba::parse(
            draft
                .colors
                .first()
                .map(String::as_str)
                .unwrap_or("#000000"),
        )
        .to_hsla();
        let to =
            Srgba::parse(draft.colors.last().map(String::as_str).unwrap_or("#ffffff")).to_hsla();
        return div()
            .w_full()
            .h(px(64.0))
            .rounded(px(8.0))
            .border_1()
            .border_color(theme.border)
            .bg(linear_gradient(
                draft.angle as f32,
                linear_color_stop(from, 0.0),
                linear_color_stop(to, 1.0),
            ))
            .into_any_element();
    }
    match draft.preview.clone() {
        Some(preview) => div()
            .w_full()
            .h(px(64.0))
            .rounded(px(8.0))
            .border_1()
            .border_color(theme.border)
            .overflow_hidden()
            .child(
                gpui::img(preview)
                    .size_full()
                    .object_fit(gpui::ObjectFit::Cover),
            )
            .into_any_element(),
        None => div().into_any_element(),
    }
}

const EDITOR_TYPE_SEGMENTS: [(BackgroundDraftType, &str, &str); 2] = [
    (BackgroundDraftType::Gradient, "Gradient", "gradient"),
    (BackgroundDraftType::Image, "Image", "image"),
];

fn editor_type_segment_id(text: &str) -> String {
    format!("wallpaper-editor-tab-{text}")
}

fn activates_editor_type_segment(key: &str) -> bool {
    matches!(key, "enter" | "space")
}

fn editor_type_toggle(
    draft: &BackgroundDraft,
    handlers: &EditorHandlers,
    theme: &ThemeVars,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let row = div()
        .flex()
        .flex_col()
        .gap(px(6.0))
        .child(label("Type", theme, false));
    let mut buttons = div().flex().flex_row().gap(px(8.0));
    for (value, text, option) in EDITOR_TYPE_SEGMENTS {
        let selected = draft.draft_type == value;
        let tab = EditorOption::WallpaperEditorTab(SharedString::from(option));
        let press = handlers.option(tab.clone());
        let activate = handlers.option(tab);
        let key = editor_type_segment_id(text);
        let focus = crate::ui::primitives::control_focus(&key, false, window, cx);
        buttons = buttons.child(
            div()
                .id(ElementId::Name(SharedString::from(key)))
                .track_focus(&focus)
                .focus(|style| style.shadow(crate::ui::primitives::focus_ring(theme, 2.0)))
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(6.0))
                .border_1()
                .border_color(if selected { theme.primary } else { theme.input })
                .bg(if selected {
                    theme.primary.opacity(0.1)
                } else {
                    theme.popover
                })
                .when(!selected, |el| {
                    el.hover(|style| style.bg(theme.muted_background))
                })
                .text_size(px(chrome::TEXT_SM))
                .text_color(if selected {
                    theme.primary
                } else {
                    theme.foreground
                })
                .px(px(12.0))
                .py(px(6.0))
                .cursor_pointer()
                .on_mouse_down(gpui::MouseButton::Left, move |_event, window, cx| {
                    press(window, cx);
                })
                .on_key_down(move |event, window, cx| {
                    if !activates_editor_type_segment(event.keystroke.key.as_str()) {
                        return;
                    }
                    activate(window, cx);
                    cx.stop_propagation();
                })
                .child(text),
        );
    }
    row.child(buttons).into_any_element()
}

fn editor_colors(
    draft: &BackgroundDraft,
    handlers: &EditorHandlers,
    theme: &ThemeVars,
) -> AnyElement {
    let mut section = div().flex().flex_col().gap(px(8.0)).child(
        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .child(label("Colors", theme, false))
            .child(if draft.colors.len() < BACKGROUND_MAX_COLORS {
                let add = handlers.option(EditorOption::WallpaperEditorAddColor);
                div()
                    .id("wallpaper-editor-add-color")
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(4.0))
                    .text_size(px(chrome::TEXT_XS))
                    .text_color(theme.muted_foreground)
                    .cursor_pointer()
                    .on_mouse_down(gpui::MouseButton::Left, move |_event, window, cx| {
                        add(window, cx);
                    })
                    .child(icon_element("plus", px(12.0)))
                    .child("Add")
                    .into_any_element()
            } else {
                div().into_any_element()
            }),
    );
    let mut rows = div().flex().flex_col().gap(px(8.0));
    for (index, color) in draft.colors.iter().enumerate() {
        let active = draft.active_color == Some(index);
        let toggle = handlers.option(EditorOption::WallpaperEditorActiveColor(index));
        let mut row = div().flex().flex_row().items_center().gap(px(8.0)).child(
            div()
                .id(ElementId::Name(SharedString::from(format!(
                    "wallpaper-editor-swatch-{index}"
                ))))
                .w(px(32.0))
                .h(px(32.0))
                .rounded(px(6.0))
                .border_2()
                .border_color(if active { theme.ring } else { theme.border })
                .bg(Srgba::parse(color).to_hsla())
                .cursor_pointer()
                .on_mouse_down(gpui::MouseButton::Left, move |_event, window, cx| {
                    toggle(window, cx);
                }),
        );
        if let Some(field) = draft.fields.get(index) {
            let apply = handlers.on_option.clone();
            row = row.child(
                div()
                    .flex_1()
                    .child(
                        TextField::new(field.clone()).on_change(move |value, window, cx| {
                            apply(
                                EditorOption::WallpaperEditorColor(
                                    index,
                                    SharedString::from(value),
                                ),
                                window,
                                cx,
                            );
                        }),
                    ),
            );
        }
        if draft.colors.len() > BACKGROUND_MIN_COLORS {
            let remove = handlers.option(EditorOption::WallpaperEditorRemoveColor(index));
            row = row.child(
                icon_button::compact_sm_muted(
                    ElementId::Name(SharedString::from(format!(
                        "wallpaper-editor-remove-{index}"
                    ))),
                    "trash-2",
                )
                .on_press(move |_event, window, cx| remove(window, cx)),
            );
        }
        rows = rows.child(row);
    }
    section = section.child(rows);
    if draft.active_color.is_some() {
        let mut grid = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .gap(px(6.0))
            .rounded(px(6.0))
            .bg(theme.muted.opacity(0.5))
            .p(px(8.0));
        for palette in BACKGROUND_PALETTE {
            let selected = draft
                .active_color
                .and_then(|index| draft.colors.get(index))
                .is_some_and(|color| color == palette);
            let pick = handlers.option(EditorOption::WallpaperEditorPickColor(SharedString::from(
                palette,
            )));
            grid = grid.child(
                div()
                    .id(ElementId::Name(SharedString::from(format!(
                        "wallpaper-editor-palette-{palette}"
                    ))))
                    .w(px(24.0))
                    .h(px(24.0))
                    .rounded(px(6.0))
                    .border_1()
                    .border_color(if selected { theme.ring } else { theme.border })
                    .bg(Srgba::parse(palette).to_hsla())
                    .cursor_pointer()
                    .on_mouse_down(gpui::MouseButton::Left, move |_event, window, cx| {
                        pick(window, cx);
                    }),
            );
        }
        section = section.child(grid);
    }
    section.into_any_element()
}

fn editor_angle(
    draft: &BackgroundDraft,
    handlers: &EditorHandlers,
    theme: &ThemeVars,
) -> AnyElement {
    let apply = handlers.on_option.clone();
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
                .child(label("Angle", theme, false))
                .child(
                    div()
                        .text_size(px(chrome::TEXT_XS))
                        .text_color(theme.muted_foreground)
                        .child(format!("{}°", draft.angle.round() as i32)),
                ),
        )
        .child(
            Slider::new("wallpaper-editor-angle", draft.angle as f32)
                .min_value(0.0)
                .max_value(360.0)
                .continuous(true)
                .size(SliderSize::Sm)
                .on_change(move |value, window, cx| {
                    apply(
                        EditorOption::WallpaperEditorAngle(*value as f64),
                        window,
                        cx,
                    );
                }),
        )
        .into_any_element()
}

fn editor_image(
    draft: &BackgroundDraft,
    handlers: &EditorHandlers,
    theme: &ThemeVars,
) -> AnyElement {
    let pick = handlers.option(EditorOption::WallpaperEditorPickImage);
    let mut box_el = div()
        .id("wallpaper-editor-pick-image")
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(8.0))
        .h(px(96.0))
        .w_full()
        .rounded(px(8.0))
        .border_2()
        .border_color(theme.border)
        .overflow_hidden()
        .cursor_pointer()
        .on_mouse_down(gpui::MouseButton::Left, move |_event, window, cx| {
            pick(window, cx);
        });
    if let Some(preview) = draft.preview.clone() {
        box_el = box_el.child(
            gpui::img(preview)
                .size_full()
                .object_fit(gpui::ObjectFit::Cover),
        );
    } else {
        box_el = box_el.child(icon_element("upload", px(24.0))).child(
            div()
                .text_size(px(chrome::TEXT_XS))
                .text_color(theme.muted_foreground)
                .child("Click to select image"),
        );
    }
    let mut section = div().flex().flex_col().gap(px(12.0)).child(box_el);
    if draft.image_url.is_some() {
        let change = handlers.option(EditorOption::WallpaperEditorPickImage);
        section = section.child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(4.0))
                        .text_size(px(chrome::TEXT_XS))
                        .text_color(theme.muted_foreground)
                        .child(icon_element("image", px(12.0)))
                        .child("Image selected"),
                )
                .child(
                    div()
                        .id("wallpaper-editor-change-image")
                        .text_size(px(chrome::TEXT_XS))
                        .text_color(theme.primary)
                        .cursor_pointer()
                        .on_mouse_down(gpui::MouseButton::Left, move |_event, window, cx| {
                            change(window, cx)
                        })
                        .child("Change"),
                ),
        );
    }
    section
        .child(
            div()
                .text_size(px(chrome::TEXT_XS))
                .text_color(theme.muted_foreground)
                .child("Supports PNG, JPEG, and GIF"),
        )
        .into_any_element()
}

fn header(handlers: &EditorHandlers, theme: &ThemeVars) -> AnyElement {
    let close = handlers.option(EditorOption::Tool(crate::ui::colors::Tool::Select));
    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .child(
            div()
                .text_size(px(chrome::TEXT_SM))
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(theme.foreground)
                .child("Wallpaper"),
        )
        .child(icon_button::with_tooltip(
            "Close",
            icon_button::compact_sm("wallpaper-sheet-close", "x")
                .on_press(move |_event: &ClickEvent, window, cx| close(window, cx)),
        ))
        .into_any_element()
}

fn preset_save_panel(
    wallpaper: &WallpaperSettings,
    draft: &PresetDraftView<'_>,
    handlers: &EditorHandlers,
    theme: &ThemeVars,
) -> AnyElement {
    let cancel = handlers.option(EditorOption::WallpaperPresetDraftCancel);
    let commit = handlers.option(EditorOption::WallpaperSavePreset);
    let submit = handlers.option(EditorOption::WallpaperSavePreset);
    let can_save = wallpaper::preset_save_name(draft.name).is_some();
    div()
        .flex()
        .flex_col()
        .gap(px(12.0))
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .child(label("Save Preset", theme, false))
                .child(
                    icon_button::compact_sm_muted("wallpaper-preset-cancel", "x")
                        .on_press(move |_event, window, cx| cancel(window, cx)),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(4.0))
                .rounded(px(chrome::RADIUS_MD))
                .bg(theme.muted_background.opacity(0.5))
                .p(px(8.0))
                .text_size(px(chrome::TEXT_XS))
                .child(div().text_color(theme.muted_foreground).child("Preview:"))
                .child(
                    div()
                        .text_color(theme.foreground)
                        .child(wallpaper::preset_summary(wallpaper)),
                ),
        )
        .child(
            TextField::new(draft.field)
                .auto_focus(true)
                .full_width()
                .text_size(px(chrome::TEXT_XS))
                .placeholder("Preset name")
                .on_submit(move |value, window, cx| {
                    if wallpaper::preset_save_name(value).is_some() {
                        submit(window, cx);
                    }
                }),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .gap(px(8.0))
                .child(
                    Button::new("wallpaper-preset-cancel-button")
                        .variant(Variant::Ghost)
                        .recipe("compact")
                        .label("Cancel")
                        .grow(true)
                        .on_press({
                            let cancel = handlers.option(EditorOption::WallpaperPresetDraftCancel);
                            move |_event, window, cx| cancel(window, cx)
                        }),
                )
                .child(
                    Button::new("wallpaper-preset-save-button")
                        .recipe("compact")
                        .label("Save")
                        .is_disabled(!can_save)
                        .grow(true)
                        .on_press(move |_event, window, cx| commit(window, cx)),
                ),
        )
        .into_any_element()
}

fn preset_manager(
    state: &SheetState<'_>,
    presets: &[crate::config::schema::WallpaperPreset],
    default_id: Option<&str>,
    handlers: &EditorHandlers,
    theme: &ThemeVars,
) -> AnyElement {
    if let Some(draft) = &state.preset_draft {
        return preset_save_panel(state.wallpaper, draft, handlers, theme);
    }
    let selected_id = state.preset_id;
    let save = handlers.option(EditorOption::WallpaperPresetDraftOpen);
    let block = div().flex().flex_col().gap(px(8.0)).child(
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .child(section_label("Presets", theme, false))
            .child(
                crate::ui::rows::icon_text_button(
                    "wallpaper-preset-save",
                    "Save",
                    "save",
                    14.0,
                    8.0,
                )
                .variant(Variant::Ghost)
                .recipe("compact")
                .recipe("muted")
                .on_press(move |_event, window, cx| save(window, cx)),
            ),
    );

    if presets.is_empty() {
        return block
            .child(crate::ui::rows::description(
                "No presets saved yet. Use the Save button to create one.",
                theme,
            ))
            .into_any_element();
    }

    let items = crate::ui::rows::picker_items(presets.iter().map(|preset| {
        let label = if default_id == Some(preset.id.as_str()) {
            format!("{} (default)", preset.name)
        } else {
            preset.name.clone()
        };
        (preset.id.clone(), label)
    }));
    let value = crate::ui::rows::selected_value(&items, selected_id);
    let apply = handlers.on_option.clone();
    let mut row = div().flex().flex_row().items_center().gap(px(8.0)).child(
        Select::new("wallpaper-preset", items)
            .recipe("compact")
            .value(value.clone())
            .placeholder("Preset")
            .full_width(true)
            .on_selection_change(move |value, window, cx| {
                let Some(value) = value else { return };
                apply(
                    EditorOption::WallpaperApplyPreset(value.clone()),
                    window,
                    cx,
                );
            })
            .into_any_element(),
    );
    if !selected_id.is_empty() {
        let is_default = default_id == Some(selected_id);
        let toggle = handlers.option(EditorOption::WallpaperToggleDefaultPreset);
        let delete = handlers.option(EditorOption::WallpaperDeletePreset);
        row = row
            .child(icon_button::with_tooltip(
                if is_default {
                    "Stop using this preset for Polish"
                } else {
                    "Use this preset for Polish"
                },
                icon_button::compact_sm("wallpaper-preset-star", "star")
                    .sx(|el| {
                        el.text_color(if is_default {
                            theme.primary
                        } else {
                            theme.muted_foreground
                        })
                    })
                    .on_press(move |_event, window, cx| toggle(window, cx)),
            ))
            .child(icon_button::with_tooltip(
                "Delete preset",
                icon_button::compact_sm_muted("wallpaper-preset-delete", "trash-2")
                    .on_press(move |_event, window, cx| delete(window, cx)),
            ));
    }

    let hint_text = match default_id.and_then(|id| presets.iter().find(|preset| preset.id == id)) {
        Some(preset) => format!(
            "Polish on the capture preview copies with \"{}\".",
            preset.name
        ),
        None => "Star a preset to enable Polish on the capture preview.".to_string(),
    };

    block
        .child(row)
        .child(crate::ui::rows::description(hint_text, theme))
        .into_any_element()
}

fn backgrounds_section(
    wallpaper: &WallpaperSettings,
    customs: &[CustomBackground],
    previews: &BackgroundPreviews,
    handlers: &EditorHandlers,
    theme: &ThemeVars,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let tile =
        chrome::wallpaper_tile_size(chrome::WALLPAPER_SHEET_WIDTH, chrome::WALLPAPER_SHEET_PAD);
    let has_background = wallpaper.has_background();
    let selected_custom = selected_custom(wallpaper, customs);
    let add = handlers.option(EditorOption::WallpaperEditorOpen(None));
    let mut actions = div().flex().flex_row().items_center().gap(px(4.0));
    if let Some(custom) = selected_custom {
        let edit = handlers.option(EditorOption::WallpaperEditorOpen(Some(SharedString::from(
            custom.id.clone(),
        ))));
        actions = actions.child(icon_button::with_tooltip(
            "Edit",
            icon_button::compact_sm_muted("wallpaper-custom-edit", "pencil")
                .on_press(move |_event, window, cx| edit(window, cx)),
        ));
        let delete = handlers.option(EditorOption::WallpaperDeleteCustom(SharedString::from(
            custom.id.clone(),
        )));
        actions = actions.child(icon_button::with_tooltip(
            "Delete",
            icon_button::compact_sm_muted("wallpaper-custom-delete", "trash-2")
                .on_press(move |_event, window, cx| delete(window, cx)),
        ));
    }
    actions = actions.child(icon_button::with_tooltip(
        "Add Background",
        icon_button::compact_sm_muted("wallpaper-add-background", "plus")
            .on_press(move |_event, window, cx| add(window, cx)),
    ));

    let mut tiles: Vec<AnyElement> = Vec::new();
    if crate::system::capabilities::is_supported(
        crate::system::capabilities::Feature::DesktopWallpaper,
    ) {
        tiles.push(desktop_tile(
            wallpaper, customs, previews, tile, handlers, theme,
        ));
    }
    for (index, (id, name, _)) in crate::editor::wallpaper_svg::PRESETS.iter().enumerate() {
        let selected = wallpaper
            .gradient
            .as_ref()
            .is_some_and(|gradient| gradient.id == *id);
        let select = handlers.option(EditorOption::WallpaperGradient(SharedString::from(*id)));
        let element_id = ElementId::Integer(index as u64);
        tiles.push(match crate::editor::wallpaper_svg::render_image(id, tile) {
            Some(image) => image_tile_named(element_id, name, image, tile, selected, theme, select),
            None => icon_tile(element_id, name, "image", tile, selected, theme, select),
        });
    }
    for (index, background) in customs.iter().enumerate() {
        tiles.push(custom_tile(
            wallpaper, background, previews, index, tile, handlers, theme,
        ));
    }

    let mut section = div()
        .flex()
        .flex_col()
        .gap(px(chrome::WALLPAPER_SECTION_GAP))
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .child(section_label("Backgrounds", theme, true))
                .child(actions),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .gap(px(chrome::WALLPAPER_GRID_GAP))
                .children(tiles),
        );

    if has_background {
        section = section
            .child(slider_control(
                "wallpaper-blur",
                "Blur",
                wallpaper.background_blur,
                0.0,
                wallpaper::BLUR_MAX,
                false,
                handlers,
                EditorOption::WallpaperBlur,
                theme,
            ))
            .child(slider_control(
                "wallpaper-noise",
                "Noise",
                wallpaper.noise,
                0.0,
                wallpaper::NOISE_MAX,
                false,
                handlers,
                EditorOption::WallpaperNoise,
                theme,
            ))
            .child({
                let clear = handlers.option(EditorOption::WallpaperClear);
                let (clear_hover, clear_hovered) =
                    crate::ui::primitives::hover_flag("wallpaper-clear", window, cx);
                div()
                    .id("wallpaper-clear")
                    .text_size(px(chrome::TEXT_XS))
                    .text_color(if clear_hovered {
                        theme.foreground
                    } else {
                        theme.muted_foreground
                    })
                    .cursor_pointer()
                    .on_hover({
                        let clear_hover = clear_hover.clone();
                        move |over: &bool, _window, cx| {
                            crate::ui::primitives::track_hover(&clear_hover, *over, cx);
                        }
                    })
                    .on_mouse_down(gpui::MouseButton::Left, move |_event, window, cx| {
                        clear(window, cx);
                    })
                    .child("Clear background")
            });
    }

    section.into_any_element()
}

fn selected_custom<'a>(
    wallpaper: &WallpaperSettings,
    customs: &'a [CustomBackground],
) -> Option<&'a CustomBackground> {
    customs.iter().find(|background| match &background.data {
        CustomBackgroundData::Gradient { data } => {
            wallpaper.gradient.as_ref().is_some_and(|gradient| {
                gradient.id == background.id || gradient.id == data.gradient.id
            })
        }
        CustomBackgroundData::Image { data } => {
            wallpaper.background_image.as_deref() == Some(data.image_url.as_str())
        }
    })
}

fn desktop_tile(
    wallpaper: &WallpaperSettings,
    customs: &[CustomBackground],
    previews: &BackgroundPreviews,
    size: f32,
    handlers: &EditorHandlers,
    theme: &ThemeVars,
) -> AnyElement {
    let is_custom_image = customs.iter().any(|background| match &background.data {
        CustomBackgroundData::Image { data } => {
            wallpaper.background_image.as_deref() == Some(data.image_url.as_str())
        }
        CustomBackgroundData::Gradient { .. } => false,
    });
    let is_desktop =
        wallpaper.background_image.is_some() && wallpaper.gradient.is_none() && !is_custom_image;
    let use_desktop = handlers.option(EditorOption::WallpaperUseDesktop);
    match previews.desktop() {
        PreviewState::Ready(image) => image_tile_named(
            "wallpaper-desktop",
            if is_desktop {
                "Desktop Wallpaper (active)"
            } else {
                "Use Desktop Wallpaper"
            },
            image,
            size,
            is_desktop,
            theme,
            use_desktop,
        ),
        PreviewState::Loading => loading_tile("wallpaper-desktop", size, theme),
        PreviewState::Failed => disabled_tile(
            "wallpaper-desktop",
            "Unable to access desktop wallpaper",
            "monitor",
            size,
            theme,
        ),
    }
}

fn custom_tile(
    wallpaper: &WallpaperSettings,
    background: &CustomBackground,
    previews: &BackgroundPreviews,
    index: usize,
    size: f32,
    handlers: &EditorHandlers,
    theme: &ThemeVars,
) -> AnyElement {
    let select = handlers.option(EditorOption::WallpaperCustom(SharedString::from(
        background.id.clone(),
    )));
    match &background.data {
        CustomBackgroundData::Gradient { data } => {
            let selected = wallpaper.gradient.as_ref().is_some_and(|gradient| {
                gradient.id == background.id || gradient.id == data.gradient.id
            });
            let colors: Vec<&str> = data.gradient.colors.iter().map(String::as_str).collect();
            let pair = [
                colors.first().copied().unwrap_or("#000000"),
                colors.last().copied().unwrap_or("#ffffff"),
            ];
            gradient_tile(
                ElementId::Name(SharedString::from(format!("custom-{}", index))),
                "",
                &pair,
                data.gradient.angle,
                size,
                selected,
                theme,
                select,
            )
        }
        CustomBackgroundData::Image { data } => {
            let selected = wallpaper.background_image.as_deref() == Some(data.image_url.as_str());
            let id = ElementId::Name(SharedString::from(format!("custom-image-{}", index)));
            match previews.custom(&background.id) {
                PreviewState::Ready(image) => {
                    image_tile_named(id, "", image, size, selected, theme, select)
                }
                PreviewState::Loading => loading_tile(id, size, theme),
                PreviewState::Failed => {
                    disabled_tile(id, "This image could not be read", "image", size, theme)
                }
            }
        }
    }
}

const TILE_RING_GAP: f32 = 2.0;

enum TileInteraction {
    Click(Rc<dyn Fn(&mut Window, &mut App)>),
    Inert,
    Blocked,
}

fn tile_shell(
    id: impl Into<ElementId>,
    tooltip: &str,
    size: f32,
    selected: bool,
    theme: &ThemeVars,
    interaction: TileInteraction,
    content: AnyElement,
) -> AnyElement {
    let ring = theme.ring;
    let border = theme.border;
    let mut tile = div()
        .id(id)
        .w(px(size))
        .h(px(size))
        .flex()
        .rounded(px(chrome::WALLPAPER_TILE_RADIUS + TILE_RING_GAP * 2.0))
        .border_2()
        .border_color(if selected {
            ring
        } else {
            gpui::hsla(0.0, 0.0, 0.0, 0.0)
        })
        .p(px(TILE_RING_GAP))
        .when(!selected, |el| {
            el.hover(move |style| style.border_color(border))
        })
        .child(
            div()
                .size_full()
                .overflow_hidden()
                .rounded(px(chrome::WALLPAPER_TILE_RADIUS))
                .child(content),
        );
    match interaction {
        TileInteraction::Click(handler) => {
            tile = tile.cursor_pointer().on_mouse_down(
                gpui::MouseButton::Left,
                move |_event, window, cx| {
                    handler(window, cx);
                },
            );
        }
        TileInteraction::Inert => tile = tile.cursor_default(),
        TileInteraction::Blocked => {
            tile = tile.cursor(gpui::CursorStyle::OperationNotAllowed);
        }
    }
    if tooltip.is_empty() {
        return tile.into_any_element();
    }
    herogpui::components::Tooltip::new(SharedString::from(tooltip.to_string()))
        .child(tile)
        .into_any_element()
}

pub fn icon_tile(
    id: impl Into<ElementId>,
    tooltip: &str,
    icon: &'static str,
    size: f32,
    selected: bool,
    theme: &ThemeVars,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
) -> AnyElement {
    tile_shell(
        id,
        tooltip,
        size,
        selected,
        theme,
        TileInteraction::Click(Rc::new(on_click)),
        icon_face(theme)
            .child(icon_element(icon, px(ICON_MD)))
            .into_any_element(),
    )
}

fn icon_face(theme: &ThemeVars) -> gpui::Div {
    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(theme.muted_background)
}

fn loading_tile(id: impl Into<ElementId>, size: f32, theme: &ThemeVars) -> AnyElement {
    let id = id.into();
    let spinner = crate::ui::icon::spinner_element(
        ElementId::Name(SharedString::from(format!("{id:?}-spinner"))),
        px(ICON_MD),
    );
    tile_shell(
        id,
        "",
        size,
        false,
        theme,
        TileInteraction::Inert,
        icon_face(theme)
            .text_color(theme.muted_foreground)
            .child(spinner)
            .into_any_element(),
    )
}

fn disabled_tile(
    id: impl Into<ElementId>,
    tooltip: &str,
    icon: &'static str,
    size: f32,
    theme: &ThemeVars,
) -> AnyElement {
    tile_shell(
        id,
        tooltip,
        size,
        false,
        theme,
        TileInteraction::Blocked,
        icon_face(theme)
            .opacity(0.5)
            .child(icon_element(icon, px(ICON_MD)))
            .into_any_element(),
    )
}

pub fn image_tile(
    id: impl Into<ElementId>,
    image: std::sync::Arc<gpui::RenderImage>,
    size: f32,
    selected: bool,
    theme: &ThemeVars,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
) -> AnyElement {
    image_tile_named(id, "", image, size, selected, theme, on_click)
}

pub fn image_tile_named(
    id: impl Into<ElementId>,
    tooltip: &str,
    image: std::sync::Arc<gpui::RenderImage>,
    size: f32,
    selected: bool,
    theme: &ThemeVars,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
) -> AnyElement {
    tile_shell(
        id,
        tooltip,
        size,
        selected,
        theme,
        TileInteraction::Click(Rc::new(on_click)),
        gpui::img(image)
            .size_full()
            .object_fit(gpui::ObjectFit::Cover)
            .into_any_element(),
    )
}

pub fn gradient_tile(
    id: impl Into<ElementId>,
    name: &str,
    colors: &[&str],
    angle: f64,
    size: f32,
    selected: bool,
    theme: &ThemeVars,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
) -> AnyElement {
    let from = Srgba::parse(colors.first().copied().unwrap_or("#000000")).to_hsla();
    let to = Srgba::parse(colors.last().copied().unwrap_or("#ffffff")).to_hsla();
    tile_shell(
        id,
        name,
        size,
        selected,
        theme,
        TileInteraction::Click(Rc::new(on_click)),
        div()
            .size_full()
            .bg(linear_gradient(
                angle as f32,
                linear_color_stop(from, 0.0),
                linear_color_stop(to, 1.0),
            ))
            .into_any_element(),
    )
}

fn aspect_row(
    wallpaper: &WallpaperSettings,
    handlers: &EditorHandlers,
    theme: &ThemeVars,
) -> AnyElement {
    let apply = handlers.on_option.clone();
    let items = crate::ui::rows::picker_items(wallpaper::ASPECT_RATIOS.iter().copied());
    let value = crate::ui::rows::selected_value(&items, wallpaper.aspect_ratio.as_str());
    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .child(label("Aspect Ratio", theme, false))
        .child(
            Select::new("wallpaper-aspect", items)
                .recipe("compact")
                .value(value.clone())
                .sx(|el| el.w(px(chrome::WALLPAPER_SELECT_WIDTH)))
                .on_selection_change(move |value, window, cx| {
                    let Some(value) = value else { return };
                    apply(
                        EditorOption::WallpaperAspectRatio(value.clone()),
                        window,
                        cx,
                    );
                }),
        )
        .into_any_element()
}

fn balance_row(
    wallpaper: &WallpaperSettings,
    handlers: &EditorHandlers,
    theme: &ThemeVars,
) -> AnyElement {
    let apply = handlers.on_option.clone();
    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .child(label("Balance", theme, false))
        .child(
            Switch::new("wallpaper-balance")
                .is_selected(wallpaper.balance)
                .size(Size::Sm)
                .on_change(move |value, window, cx| {
                    apply(EditorOption::WallpaperBalance(*value), window, cx);
                }),
        )
        .into_any_element()
}

fn spacing_control(
    wallpaper: &WallpaperSettings,
    has_layers: bool,
    handlers: &EditorHandlers,
    theme: &ThemeVars,
) -> AnyElement {
    let mut block = slider_control(
        "wallpaper-spacing",
        "Spacing",
        wallpaper.spacing,
        0.0,
        wallpaper::SPACING_MAX,
        !has_layers,
        handlers,
        EditorOption::WallpaperSpacing,
        theme,
    );
    if !has_layers {
        block = div()
            .flex()
            .flex_col()
            .gap(px(chrome::WALLPAPER_SECTION_GAP))
            .child(block)
            .child(crate::ui::rows::description(
                "Drop another image to enable spacing",
                theme,
            ))
            .into_any_element();
        return block;
    }
    block
}

pub fn slider_control(
    id: &'static str,
    text: &'static str,
    value: f64,
    min: f64,
    max: f64,
    disabled: bool,
    handlers: &EditorHandlers,
    option: fn(f64) -> EditorOption,
    theme: &ThemeVars,
) -> AnyElement {
    let apply = handlers.on_option.clone();
    div()
        .flex()
        .flex_col()
        .gap(px(chrome::WALLPAPER_SECTION_GAP))
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .child(label(text, theme, disabled))
                .child(
                    div()
                        .text_size(px(chrome::TEXT_XS))
                        .text_color(theme.foreground)
                        .child(format!("{}", value.round() as i32)),
                ),
        )
        .child(
            Slider::new(id, value as f32)
                .min_value(min as f32)
                .max_value(max as f32)
                .continuous(true)
                .size(SliderSize::Sm)
                .is_disabled(disabled)
                .on_change(move |value, window, cx| {
                    apply(option(*value as f64), window, cx);
                }),
        )
        .into_any_element()
}

fn window_frames(
    wallpaper: &WallpaperSettings,
    handlers: &EditorHandlers,
    theme: &ThemeVars,
) -> AnyElement {
    let current = wallpaper.window_frame.style.as_str();
    let tiles: Vec<AnyElement> = wallpaper::WINDOW_FRAMES
        .iter()
        .map(|(value, label)| {
            let selected = current == *value;
            let select = handlers.option(EditorOption::WallpaperFrame(SharedString::from(*value)));
            frame_preview(value, label, selected, theme, select)
        })
        .collect();
    div()
        .flex()
        .flex_col()
        .gap(px(chrome::WALLPAPER_SECTION_GAP))
        .child(section_label("Window Frame", theme, true))
        .child(
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .gap(px(chrome::WALLPAPER_GRID_GAP))
                .children(tiles),
        )
        .into_any_element()
}

fn frame_preview(
    style: &'static str,
    name: &'static str,
    selected: bool,
    theme: &ThemeVars,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
) -> AnyElement {
    let inner_w = (chrome::WALLPAPER_SHEET_WIDTH
        - chrome::WALLPAPER_SHEET_PAD * 2.0
        - chrome::WALLPAPER_GRID_GAP * (chrome::WALLPAPER_FRAME_COLS as f32 - 1.0))
        / chrome::WALLPAPER_FRAME_COLS as f32;
    let handler = Rc::new(on_click);
    let frame_theme = wallpaper::FRAME_THEMES
        .iter()
        .find(|(id, _, _, _, _, _)| *id == style);
    let preview = match frame_theme {
        Some((_, title_bar, title_border, content, frame_border, control)) => {
            let is_windows = style.starts_with("windows");
            div()
                .h(px(chrome::WALLPAPER_FRAME_PREVIEW_H))
                .w_full()
                .flex()
                .flex_col()
                .overflow_hidden()
                .rounded(px(chrome::RADIUS_MD))
                .border_1()
                .border_color(Srgba::parse(frame_border).to_hsla())
                .child(
                    div()
                        .h(px(chrome::WALLPAPER_FRAME_TITLE_H))
                        .flex()
                        .flex_row()
                        .items_center()
                        .px(px(6.0))
                        .bg(Srgba::parse(title_bar).to_hsla())
                        .border_b_1()
                        .border_color(Srgba::parse(title_border).to_hsla())
                        .child(if is_windows {
                            windows_controls(Srgba::parse(control).to_hsla())
                        } else {
                            traffic_lights()
                        }),
                )
                .child(div().flex_1().bg(Srgba::parse(content).to_hsla()))
        }
        None => div()
            .relative()
            .h(px(chrome::WALLPAPER_FRAME_PREVIEW_H))
            .w_full()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(chrome::RADIUS_MD))
            .bg(theme.muted_background.opacity(0.5))
            .child(dashed_border(theme.border))
            .child(
                div()
                    .text_size(px(chrome::TEXT_XS))
                    .text_color(theme.foreground)
                    .child("No frame"),
            ),
    };

    div()
        .id(ElementId::Name(SharedString::from(format!(
            "frame-{style}"
        ))))
        .w(px(inner_w))
        .flex()
        .flex_col()
        .items_center()
        .gap(px(chrome::WALLPAPER_FRAME_GAP))
        .rounded(px(chrome::WALLPAPER_TILE_RADIUS))
        .border_1()
        .border_color(if selected {
            theme.foreground
        } else {
            gpui::hsla(0.0, 0.0, 0.0, 0.0)
        })
        .when(selected, |el| el.bg(theme.muted_background))
        .when(!selected, |el| {
            let border = theme.border;
            el.hover(move |style| style.border_color(border))
        })
        .p(px(chrome::WALLPAPER_FRAME_PAD))
        .cursor_pointer()
        .on_mouse_down(gpui::MouseButton::Left, move |_event, window, cx| {
            handler(window, cx);
        })
        .child(preview)
        .child(
            div()
                .text_size(px(chrome::TEXT_XS))
                .text_color(if selected {
                    theme.foreground
                } else {
                    theme.muted_foreground
                })
                .child(name),
        )
        .into_any_element()
}

const DASH_LENGTH: f32 = 4.0;
const DASH_GAP: f32 = 3.0;
const DASH_WIDTH: f32 = 1.0;

fn dashed_border(color: gpui::Hsla) -> AnyElement {
    gpui::canvas(
        |_, _, _| {},
        move |bounds, _: (), window, _cx| {
            let mut builder = gpui::PathBuilder::stroke(px(DASH_WIDTH));
            let inset = DASH_WIDTH / 2.0;
            let left = f32::from(bounds.origin.x) + inset;
            let top = f32::from(bounds.origin.y) + inset;
            let right = left + f32::from(bounds.size.width) - DASH_WIDTH;
            let bottom = top + f32::from(bounds.size.height) - DASH_WIDTH;
            let step = DASH_LENGTH + DASH_GAP;
            let mut dash = |from: (f32, f32), to: (f32, f32)| {
                let span = ((to.0 - from.0).powi(2) + (to.1 - from.1).powi(2)).sqrt();
                if span <= 0.0 {
                    return;
                }
                let (dx, dy) = ((to.0 - from.0) / span, (to.1 - from.1) / span);
                let mut offset = 0.0;
                while offset < span {
                    let end = (offset + DASH_LENGTH).min(span);
                    builder.move_to(gpui::point(
                        px(from.0 + dx * offset),
                        px(from.1 + dy * offset),
                    ));
                    builder.line_to(gpui::point(px(from.0 + dx * end), px(from.1 + dy * end)));
                    offset += step;
                }
            };
            dash((left, top), (right, top));
            dash((right, top), (right, bottom));
            dash((right, bottom), (left, bottom));
            dash((left, bottom), (left, top));
            if let Ok(path) = builder.build() {
                window.paint_path(path, color);
            }
        },
    )
    .absolute()
    .inset_0()
    .into_any_element()
}

fn traffic_lights() -> AnyElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(4.0))
        .child(dot("#FF5F57"))
        .child(dot("#FFBD2E"))
        .child(dot("#28C840"))
        .into_any_element()
}

fn windows_controls(color: gpui::Hsla) -> AnyElement {
    div()
        .ml_auto()
        .flex()
        .flex_row()
        .items_center()
        .h_full()
        .child(
            div()
                .w(px(12.0))
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .child(div().h(px(1.0)).w(px(6.0)).bg(color)),
        )
        .child(
            div()
                .w(px(12.0))
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .child(div().size(px(6.0)).border_1().border_color(color)),
        )
        .child(
            div()
                .w(px(12.0))
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .text_color(color)
                .child(
                    crate::ui::icon::icon("M8 8L16 16M16 8L8 16")
                        .size(px(12.0))
                        .stroke_width(2.0),
                ),
        )
        .into_any_element()
}

fn dot(hex: &str) -> AnyElement {
    div()
        .size(px(4.0))
        .rounded_full()
        .bg(Srgba::parse(hex).to_hsla())
        .into_any_element()
}

fn section_label(text: &'static str, theme: &ThemeVars, muted: bool) -> AnyElement {
    label(text, theme, muted)
}

fn label(text: impl Into<SharedString>, theme: &ThemeVars, muted: bool) -> AnyElement {
    div()
        .text_size(px(chrome::TEXT_XS))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(if muted {
            theme.muted_foreground
        } else {
            theme.foreground
        })
        .child(text.into())
        .into_any_element()
}

pub fn video_aspect_grid(
    selected: Option<(f64, f64)>,
    theme: &ThemeVars,
    on_select: impl Fn(Option<(f64, f64)>, &mut Window, &mut App) + 'static,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let handler = Rc::new(on_select);
    let buttons: Vec<AnyElement> = wallpaper::VIDEO_ASPECT_RATIOS
        .iter()
        .map(|(label, width, height)| {
            let is_auto = *width == 0.0 && *height == 0.0;
            let is_selected = match selected {
                None => is_auto,
                Some((w, h)) => {
                    (*width - w).abs() < f64::EPSILON && (*height - h).abs() < f64::EPSILON
                }
            };
            let value = if is_auto {
                None
            } else {
                Some((*width, *height))
            };
            let on_click = handler.clone();
            let inner_w = (chrome::VIDEO_SIDEBAR_WIDTH
                - chrome::VIDEO_PANEL_PAD * 2.0
                - chrome::VIDEO_ASPECT_GAP * (chrome::VIDEO_ASPECT_COLS as f32 - 1.0))
                / chrome::VIDEO_ASPECT_COLS as f32;
            let tile_key = format!("video-aspect-{label}");
            let (tile_hover, tile_hovered) =
                crate::ui::primitives::hover_flag(&tile_key, window, cx);
            let filled = is_selected || tile_hovered;
            div()
                .id(SharedString::from(tile_key))
                .w(px(inner_w))
                .rounded(px(chrome::VIDEO_ASPECT_RADIUS))
                .px(px(chrome::VIDEO_ASPECT_PAD_X))
                .py(px(chrome::VIDEO_ASPECT_PAD_Y))
                .text_size(px(chrome::TEXT_XS))
                .font_weight(gpui::FontWeight::MEDIUM)
                .bg(if filled {
                    theme.accent
                } else {
                    theme.muted_background
                })
                .text_color(if filled {
                    theme.accent_foreground
                } else {
                    theme.muted_foreground
                })
                .on_hover({
                    let tile_hover = tile_hover.clone();
                    move |over: &bool, _window, cx| {
                        crate::ui::primitives::track_hover(&tile_hover, *over, cx);
                    }
                })
                .on_mouse_down(gpui::MouseButton::Left, move |_event, window, cx| {
                    on_click(value, window, cx);
                })
                .child(*label)
                .into_any_element()
        })
        .collect();
    div()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .child(section_label("Aspect Ratio", theme, true))
        .child(
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .gap(px(chrome::VIDEO_ASPECT_GAP))
                .children(buttons),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_editor_type_segment_is_its_own_focus_stop() {
        let ids: Vec<String> = EDITOR_TYPE_SEGMENTS
            .iter()
            .map(|(_, text, _)| editor_type_segment_id(text))
            .collect();
        assert_eq!(
            ids,
            vec![
                "wallpaper-editor-tab-Gradient".to_string(),
                "wallpaper-editor-tab-Image".to_string(),
            ]
        );
    }

    #[test]
    fn editor_type_segments_carry_the_option_value_of_their_draft_type() {
        assert_eq!(
            EDITOR_TYPE_SEGMENTS
                .iter()
                .map(|(value, _, option)| (*value, *option))
                .collect::<Vec<_>>(),
            vec![
                (BackgroundDraftType::Gradient, "gradient"),
                (BackgroundDraftType::Image, "image"),
            ]
        );
    }

    #[test]
    fn editor_type_segments_activate_on_enter_and_space_only() {
        assert!(activates_editor_type_segment("enter"));
        assert!(activates_editor_type_segment("space"));
        assert!(!activates_editor_type_segment("tab"));
        assert!(!activates_editor_type_segment("escape"));
        assert!(!activates_editor_type_segment("a"));
    }
}
