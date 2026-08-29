use std::rc::Rc;

use gpui::{
    div, linear_color_stop, linear_gradient, prelude::*, px, Animation, AnimationExt, AnyElement,
    App, ClickEvent, ElementId, SharedString, Styled, Window,
};

use crate::config::schema::{CustomBackground, CustomBackgroundData};
use crate::editor::options::{EditorHandlers, EditorOption};
use crate::editor::wallpaper::{self, WallpaperSettings};
use crate::theme::color::Srgba;
use crate::theme::vars::{active_theme, ThemeVars};
use crate::ui::button::{Button, ButtonSize, ButtonVariant};
use crate::ui::chrome;
use crate::ui::icon::{icon_element, ICON_MD};
use crate::ui::menu::MenuHandle;
use crate::ui::primitives::Separator;
use crate::ui::select::{Select, SelectOption};
use crate::ui::slider::Slider;
use crate::ui::switch::{Switch, SwitchSize};

pub fn render(
    wallpaper: &WallpaperSettings,
    has_layers: bool,
    preset_id: &str,
    menu: &MenuHandle,
    handlers: &EditorHandlers,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let theme = active_theme(cx);
    let config = crate::state::state(cx).config.get();
    let wallpaper_config = config.wallpaper;

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
        .child(header(handlers, &theme))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(chrome::WALLPAPER_SHEET_INNER_GAP))
                .child(preset_manager(
                    preset_id,
                    &wallpaper_config.presets,
                    wallpaper_config.default_preset_id.as_deref(),
                    menu,
                    handlers,
                    &theme,
                ))
                .child(Separator::horizontal())
                .child(backgrounds_section(
                    wallpaper,
                    &wallpaper_config.custom_backgrounds,
                    handlers,
                    &theme,
                    window,
                    cx,
                ))
                .child(Separator::horizontal())
                .child(aspect_row(wallpaper, menu, handlers, &theme))
                .child(Separator::horizontal())
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
                .child(spacing_control(wallpaper, has_layers, handlers, &theme))
                .child(Separator::horizontal())
                .child(window_frames(wallpaper, handlers, &theme)),
        )
        .with_animation(
            ElementId::Name("wallpaper-sheet-enter".into()),
            Animation::new(std::time::Duration::from_millis(300))
                .with_easing(crate::ui::primitives::cubic_bezier(0.42, 0.0, 0.58, 1.0)),
            |sheet, delta| {
                sheet
                    .opacity(delta)
                    .left(px(-chrome::WALLPAPER_SHEET_WIDTH * (1.0 - delta)))
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
        .child(
            Button::new("wallpaper-sheet-close")
                .variant(ButtonVariant::Ghost)
                .size(ButtonSize::IconXs)
                .icon("x")
                .tooltip("Close")
                .on_click(move |_event: &ClickEvent, window, cx| close(window, cx)),
        )
        .into_any_element()
}

fn preset_manager(
    selected_id: &str,
    presets: &[crate::config::schema::WallpaperPreset],
    default_id: Option<&str>,
    menu: &MenuHandle,
    handlers: &EditorHandlers,
    theme: &ThemeVars,
) -> AnyElement {
    let save = handlers.option(EditorOption::WallpaperSavePreset);
    let block = div().flex().flex_col().gap(px(8.0)).child(
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .child(section_label("Presets", theme, false))
            .child(
                Button::new("wallpaper-preset-save")
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Xs)
                    .icon("save")
                    .label("Save")
                    .foreground(theme.muted_foreground)
                    .on_click(move |_event, window, cx| save(window, cx)),
            ),
    );

    if presets.is_empty() {
        return block
            .child(hint(
                "No presets saved yet. Use the Save button to create one.",
                theme,
            ))
            .into_any_element();
    }

    let options: Vec<SelectOption> = presets
        .iter()
        .map(|preset| {
            let label = if default_id == Some(preset.id.as_str()) {
                format!("{} (default)", preset.name)
            } else {
                preset.name.clone()
            };
            SelectOption::new(preset.id.clone(), label)
        })
        .collect();
    let apply = handlers.on_option.clone();
    let mut row = div().flex().flex_row().items_center().gap(px(8.0)).child(
        Select::new("wallpaper-preset", menu.clone())
            .selected(selected_id.to_string())
            .placeholder("Preset")
            .options(options)
            .small()
            .full_width()
            .on_select(move |value, window, cx| {
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
            .child(
                Button::new("wallpaper-preset-star")
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::IconXs)
                    .icon("star")
                    // `text-primary` when this preset is the Polish default,
                    // and `--primary` is the operating system's accent.
                    .foreground(if is_default {
                        theme.primary
                    } else {
                        theme.muted_foreground
                    })
                    .tooltip(if is_default {
                        "Stop using this preset for Polish"
                    } else {
                        "Use this preset for Polish"
                    })
                    .on_click(move |_event, window, cx| toggle(window, cx)),
            )
            .child(
                Button::new("wallpaper-preset-delete")
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::IconXs)
                    .icon("trash-2")
                    .foreground(theme.muted_foreground)
                    .tooltip("Delete preset")
                    .on_click(move |_event, window, cx| delete(window, cx)),
            );
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
        .child(hint(hint_text, theme))
        .into_any_element()
}

fn backgrounds_section(
    wallpaper: &WallpaperSettings,
    customs: &[CustomBackground],
    handlers: &EditorHandlers,
    theme: &ThemeVars,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let tile =
        chrome::wallpaper_tile_size(chrome::WALLPAPER_SHEET_WIDTH, chrome::WALLPAPER_SHEET_PAD);
    let has_background = wallpaper.has_background();
    let selected_custom = selected_custom(wallpaper, customs);
    let add = handlers.option(EditorOption::WallpaperPickImage);
    let mut actions = div().flex().flex_row().items_center().gap(px(4.0));
    if let Some(custom) = selected_custom {
        let delete = handlers.option(EditorOption::WallpaperDeleteCustom(SharedString::from(
            custom.id.clone(),
        )));
        actions = actions.child(
            Button::new("wallpaper-custom-delete")
                .variant(ButtonVariant::Ghost)
                .size(ButtonSize::IconXs)
                .icon("trash-2")
                .foreground(theme.muted_foreground)
                .tooltip("Delete")
                .on_click(move |_event, window, cx| delete(window, cx)),
        );
    }
    actions = actions.child(
        Button::new("wallpaper-add-background")
            .variant(ButtonVariant::Ghost)
            .size(ButtonSize::IconXs)
            .icon("plus")
            .foreground(theme.muted_foreground)
            .tooltip("Add Background")
            .on_click(move |_event, window, cx| add(window, cx)),
    );

    let mut tiles: Vec<AnyElement> = Vec::new();
    tiles.push(desktop_tile(wallpaper, customs, tile, handlers, theme));
    for (index, (id, name, colors, angle)) in wallpaper::SVG_PRESETS.iter().enumerate() {
        let selected = wallpaper
            .gradient
            .as_ref()
            .is_some_and(|gradient| gradient.id == *id);
        let select = handlers.option(EditorOption::WallpaperGradient(SharedString::from(*id)));
        tiles.push(gradient_tile(
            ElementId::Integer(index as u64),
            name,
            colors,
            *angle,
            tile,
            selected,
            theme,
            select,
        ));
    }
    for (index, background) in customs.iter().enumerate() {
        tiles.push(custom_tile(
            wallpaper, background, index, tile, handlers, theme,
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
                // Gated hover flag instead of a `.hover()` style, which gpui
                // paints against the window's last mouse position and so
                // survives the pointer leaving the window.
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
    icon_tile(
        "wallpaper-desktop",
        "Use Desktop Wallpaper",
        "monitor",
        size,
        is_desktop,
        theme,
        use_desktop,
    )
}

fn custom_tile(
    wallpaper: &WallpaperSettings,
    background: &CustomBackground,
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
                background.id.as_str(),
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
            icon_tile(
                ElementId::Name(SharedString::from(format!("custom-image-{}", index))),
                "Custom image",
                "image",
                size,
                selected,
                theme,
                select,
            )
        }
    }
}

pub fn icon_tile(
    id: impl Into<ElementId>,
    _tooltip: &str,
    icon: &'static str,
    size: f32,
    selected: bool,
    theme: &ThemeVars,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
) -> AnyElement {
    let handler = Rc::new(on_click);
    div()
        .id(id)
        .w(px(size))
        .h(px(size))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(chrome::WALLPAPER_TILE_RADIUS))
        .bg(theme.muted_background)
        .when(selected, |el| el.border_2().border_color(theme.ring))
        .when(!selected, |el| {
            el.border_2().border_color(theme.muted_background)
        })
        .cursor_pointer()
        .on_mouse_down(gpui::MouseButton::Left, move |_event, window, cx| {
            handler(window, cx);
        })
        .child(icon_element(icon, px(ICON_MD)))
        .into_any_element()
}

pub fn gradient_tile(
    id: impl Into<ElementId>,
    _name: &str,
    colors: &[&str],
    angle: f64,
    size: f32,
    selected: bool,
    theme: &ThemeVars,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
) -> AnyElement {
    let from = Srgba::parse(colors.first().copied().unwrap_or("#000000")).to_hsla();
    let to = Srgba::parse(colors.last().copied().unwrap_or("#ffffff")).to_hsla();
    let handler = Rc::new(on_click);
    div()
        .id(id)
        .w(px(size))
        .h(px(size))
        .rounded(px(chrome::WALLPAPER_TILE_RADIUS))
        .bg(linear_gradient(
            angle as f32,
            linear_color_stop(from, 0.0),
            linear_color_stop(to, 1.0),
        ))
        .when(selected, |el| el.border_2().border_color(theme.ring))
        .when(!selected, |el| {
            el.border_2().border_color(gpui::hsla(0.0, 0.0, 0.0, 0.0))
        })
        .cursor_pointer()
        .on_mouse_down(gpui::MouseButton::Left, move |_event, window, cx| {
            handler(window, cx);
        })
        .into_any_element()
}

fn aspect_row(
    wallpaper: &WallpaperSettings,
    menu: &MenuHandle,
    handlers: &EditorHandlers,
    theme: &ThemeVars,
) -> AnyElement {
    let apply = handlers.on_option.clone();
    let options = wallpaper::ASPECT_RATIOS
        .iter()
        .map(|(value, label)| SelectOption::new(*value, *label))
        .collect();
    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .child(label("Aspect Ratio", theme, false))
        .child(
            Select::new("wallpaper-aspect", menu.clone())
                .selected(wallpaper.aspect_ratio.clone())
                .options(options)
                .small()
                .width(px(chrome::WALLPAPER_SELECT_WIDTH))
                .on_select(move |value, window, cx| {
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
            Switch::new("wallpaper-balance", wallpaper.balance)
                .size(SwitchSize::Sm)
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
            .child(hint("Drop another image to enable spacing", theme))
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
            Slider::new(id, value as f32, min as f32, max as f32)
                .small()
                .disabled(disabled)
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
                .rounded(px(6.0))
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
            .h(px(chrome::WALLPAPER_FRAME_PREVIEW_H))
            .w_full()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(6.0))
            .border_1()
            .border_color(theme.border)
            .bg(theme.muted_background.opacity(0.5))
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

fn hint(text: impl Into<SharedString>, theme: &ThemeVars) -> AnyElement {
    div()
        .text_size(px(chrome::TEXT_XS))
        .text_color(theme.muted_foreground)
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
            // Gated hover flag instead of a `.hover()` style, which gpui
            // paints against the window's last mouse position and so survives
            // the pointer leaving the window.
            let tile_key = format!("video-aspect-{label}");
            let (tile_hover, tile_hovered) =
                crate::ui::primitives::hover_flag(&tile_key, window, cx);
            // Unselected tiles hover to `bg-accent text-accent-foreground`.
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
