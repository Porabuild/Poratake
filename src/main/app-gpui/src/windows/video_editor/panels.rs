use gpui::{div, prelude::*, px, AnyElement, Context, SharedString, Styled};

use crate::theme::vars::ThemeVars;
use crate::ui::button::{Button, ButtonSize, ButtonVariant};
use crate::ui::menu::MenuHandle;
use crate::windows::video_editor::data_editor;
use crate::windows::video_editor::model::{FocusPoint, VideoEditorState};
use crate::windows::video_editor::panel_kit as kit;
use crate::windows::video_editor::sidebar::SidebarTab;
use crate::windows::video_editor::styles::{
    self, CameraStyle, CursorStyle, KeyboardStyle, SubtitleStyle,
};
use crate::windows::video_editor::VideoEditorWindow;

pub fn render(
    tab: SidebarTab,
    // Reading the owning entity back out of the context panics while it is
    // being updated, so the panels that need more than `state` take the view
    // itself.
    view: &VideoEditorWindow,
    state: &VideoEditorState,
    has_cursor_data: bool,
    has_camera: bool,
    has_keyboard: bool,
    has_mic: bool,
    is_exporting: bool,
    export_progress: f32,
    menu: &MenuHandle,
    theme: &ThemeVars,
    window: &mut gpui::Window,
    cx: &mut Context<VideoEditorWindow>,
) -> AnyElement {
    match tab {
        SidebarTab::Cursor => cursor_panel(state, has_cursor_data, menu, theme, cx),
        SidebarTab::Zoom => zoom_panel(view, state, has_cursor_data, theme, cx),
        SidebarTab::Drawing => drawing_panel(view, state, menu, theme, cx),
        SidebarTab::Camera => camera_panel(state, has_camera, theme, cx),
        SidebarTab::Audio => audio_panel(view, state, has_keyboard, menu, theme, cx),
        SidebarTab::Wallpaper => wallpaper_panel(state, theme, window, cx),
        SidebarTab::Keyboard => keyboard_panel(state, has_keyboard, theme, cx),
        SidebarTab::Subtitle => subtitle_panel(view, state, has_mic, theme, cx),
        SidebarTab::FirstFrame => first_frame_panel(state, theme, cx),
        SidebarTab::Export => {
            export_panel(view, state, is_exporting, export_progress, menu, theme, cx)
        }
    }
}

fn cursor_panel(
    state: &VideoEditorState,
    has_cursor_data: bool,
    menu: &MenuHandle,
    theme: &ThemeVars,
    cx: &mut Context<VideoEditorWindow>,
) -> AnyElement {
    if !has_cursor_data {
        return kit::panel(vec![
            kit::header(
                "Cursor Data",
                "No cursor data available for this video",
                None,
                theme,
                cx,
                |_, _, _| {},
            ),
            kit::note(
                "You can add cursor data manually by importing a JSON file or creating it in the editor.",
                theme,
            ),
            kit::tertiary_button(
                "cursor-import-data",
                "Import from File",
                "file-up",
                false,
                theme,
                cx,
                |this, cx| this.import_cursor_data(cx),
            ),
            kit::tertiary_button(
                "cursor-edit-data",
                "Create Manually",
                "edit-3",
                false,
                theme,
                cx,
                |this, cx| this.open_data_editor(data_editor::DataKind::Cursor, cx),
            ),
            format_card(
                "Cursor Data Format",
                "Cursor data is a JSON file containing mouse movement events with normalized coordinates (0-1). Each event has a timestamp, x/y position, and event type (move, down, up, scroll).",
                theme,
            ),
        ]);
    }

    let style = state.cursor_style.clone();
    let enabled = style.enabled;
    let mut children = vec![kit::header(
        "Cursor Overlay",
        "Show cursor in your video",
        Some(enabled),
        theme,
        cx,
        |this, value, cx| {
            this.update_cursor(cx, move |style| style.enabled = value);
        },
    )];

    if !enabled {
        children.push(kit::note(
            "Cursor overlay is disabled. Enable it to show cursor in your video.",
            theme,
        ));
        return kit::panel(children);
    }

    let has_custom = style.custom_cursor_image.is_some();
    if has_custom {
        children.push(kit::field(
            "Custom Cursor",
            kit::tertiary_button(
                "cursor-remove-custom",
                "Remove",
                "x",
                false,
                theme,
                cx,
                |this, cx| this.clear_custom_cursor(cx),
            ),
            theme,
        ));
        children.push(kit::hint("Using custom cursor image", theme));
    } else {
        children.push(kit::field(
            "Custom Cursor",
            kit::tertiary_button(
                "cursor-upload-custom",
                "Upload Custom Cursor",
                "upload",
                false,
                theme,
                cx,
                |this, cx| this.pick_custom_cursor(cx),
            ),
            theme,
        ));
        children.push(kit::hint(
            "Upload a PNG, SVG, or other image to use as cursor",
            theme,
        ));
    }

    children.push(kit::slider_row(
        "cursor-size",
        "Size",
        style.size,
        styles::CURSOR_SIZE_MIN,
        styles::CURSOR_SIZE_MAX,
        format!("{}%", style.size.round() as i32),
        theme,
        cx,
        |this, value, cx| this.update_cursor(cx, move |style| style.size = value.round()),
    ));
    children.push(kit::slider_row(
        "cursor-smoothing",
        "Smoothing",
        style.smoothing,
        0.0,
        1.0,
        smoothing_label(style.smoothing).to_string(),
        theme,
        cx,
        |this, value, cx| this.update_cursor(cx, move |style| style.smoothing = value),
    ));
    children.push(kit::hint(
        "Reduces cursor shake for smoother movement",
        theme,
    ));

    children.push(kit::switch_row(
        "cursor-motion-blur",
        "Motion Blur",
        style.motion_blur,
        theme,
        cx,
        |this, value, cx| this.update_cursor(cx, move |style| style.motion_blur = value),
    ));
    children.push(kit::hint("Blur the cursor along its movement", theme));
    if style.motion_blur {
        children.push(kit::slider_row(
            "cursor-blur-strength",
            "Blur Strength",
            style.motion_blur_strength,
            0.0,
            1.0,
            format!("{}%", (style.motion_blur_strength * 100.0).round() as i32),
            theme,
            cx,
            |this, value, cx| {
                this.update_cursor(cx, move |style| style.motion_blur_strength = value)
            },
        ));
    }

    if !has_custom {
        children.push(kit::select_row(
            "cursor-color",
            "Color",
            &style.color,
            &styles::CURSOR_COLORS,
            menu,
            theme,
            cx,
            |this, value, cx| this.update_cursor(cx, move |style| style.color = value.clone()),
        ));
        children.push(kit::select_row(
            "cursor-border",
            "Border",
            &style.border_color,
            &styles::CURSOR_BORDERS,
            menu,
            theme,
            cx,
            |this, value, cx| {
                this.update_cursor(cx, move |style| style.border_color = value.clone())
            },
        ));
    }

    children.push(kit::switch_row(
        "cursor-hide-on-idle",
        "Hide When Idle",
        style.hide_on_idle,
        theme,
        cx,
        |this, value, cx| this.update_cursor(cx, move |style| style.hide_on_idle = value),
    ));
    children.push(kit::hint("Fade out cursor when not moving", theme));
    if style.hide_on_idle {
        children.push(kit::slider_row(
            "cursor-idle-timeout",
            "Timeout",
            style.hide_on_idle_timeout,
            0.5,
            5.0,
            format!("{}s", style.hide_on_idle_timeout),
            theme,
            cx,
            |this, value, cx| {
                this.update_cursor(cx, move |style| style.hide_on_idle_timeout = value)
            },
        ));
    }

    children.push(kit::separator(theme));
    children.push(kit::label("Cursor Data", theme));
    children.push(
        div()
            .flex()
            .flex_row()
            .gap(px(8.0))
            .child(kit::tertiary_button(
                "cursor-edit-data-panel",
                "Edit",
                "edit-3",
                false,
                theme,
                cx,
                |this, cx| this.open_data_editor(data_editor::DataKind::Cursor, cx),
            ))
            .child(kit::tertiary_button(
                "cursor-import-data-panel",
                "Import",
                "file-up",
                false,
                theme,
                cx,
                |this, cx| this.import_cursor_data(cx),
            ))
            .into_any_element(),
    );
    children.push(kit::reset_button("cursor-reset", theme, cx, |this, cx| {
        this.update_cursor(cx, |style| *style = CursorStyle::default());
    }));

    kit::panel(children)
}

fn format_card(title: &'static str, body: &'static str, theme: &ThemeVars) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .rounded(px(6.0))
        .border_1()
        .border_color(theme.border)
        .bg(theme.muted_background)
        .p(px(12.0))
        .child(
            div()
                .text_size(px(crate::ui::chrome::TEXT_SM))
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(theme.foreground)
                .child(title),
        )
        .child(kit::hint(body, theme))
        .into_any_element()
}

fn smoothing_label(value: f64) -> &'static str {
    if value == 0.0 {
        "Off"
    } else if value <= 0.3 {
        "Low"
    } else if value <= 0.6 {
        "Medium"
    } else {
        "High"
    }
}

fn zoom_panel(
    view: &VideoEditorWindow,
    state: &VideoEditorState,
    has_cursor_data: bool,
    theme: &ThemeVars,
    cx: &mut Context<VideoEditorWindow>,
) -> AnyElement {
    let settings = state.zoom_settings.clone();
    let selected = view.selected_zoom_segment().cloned();

    let mut children = vec![
        kit::header(
            "Auto Zoom",
            "Highlight clicks, drags and scrolls with zoom",
            None,
            theme,
            cx,
            |_, _, _| {},
        ),
        kit::tertiary_button(
            "zoom-auto-generate",
            "Generate from Interactions",
            "mouse-pointer-click",
            !has_cursor_data,
            theme,
            cx,
            |this, cx| this.generate_auto_zoom(cx),
        ),
    ];
    if !has_cursor_data {
        children.push(kit::hint("No cursor data recorded for this video", theme));
    }
    children.push(kit::separator(theme));

    match selected {
        None => {
            children.push(kit::note(
                "Select a zoom segment on the timeline to edit its settings",
                theme,
            ));
        }
        Some(segment) => {
            children.push(kit::header(
                "Zoom Settings",
                "Configure settings for the selected zoom segment",
                None,
                theme,
                cx,
                |_, _, _| {},
            ));
            children.push(kit::tab_row(
                "zoom-target-mode",
                "Zoom Target",
                segment.target_mode.as_deref().unwrap_or("cursor"),
                &ZOOM_TARGET_MODES,
                theme,
                cx,
                |this, value, cx| this.set_zoom_target_mode(value.to_string(), cx),
            ));

            if segment.target_mode.as_deref() == Some("manual") {
                let focus = segment.focus_point.unwrap_or(FocusPoint { x: 0.5, y: 0.5 });
                children.push(focus_picker(view, focus, theme, cx));
            } else {
                children.push(kit::hint(
                    "Zoom follows cursor position and movement",
                    theme,
                ));
            }

            children.push(kit::slider_row(
                "zoom-level",
                "Zoom Level",
                segment.zoom_level,
                styles::ZOOM_LEVEL_MIN,
                styles::ZOOM_LEVEL_MAX,
                format!("{}%", (segment.zoom_level * 100.0).round() as i32),
                theme,
                cx,
                |this, value, cx| {
                    this.update_selected_zoom(cx, move |segment| segment.zoom_level = value)
                },
            ));
            children.push(kit::hint(
                "Magnification level (100% = no zoom, 300% = 3x)",
                theme,
            ));

            let speed = segment
                .transition_in_duration
                .or(segment.transition_out_duration)
                .unwrap_or(settings.transition_in_duration);
            children.push(kit::slider_row(
                "zoom-speed",
                "Zoom Speed",
                speed,
                styles::ZOOM_SPEED_MIN,
                styles::ZOOM_SPEED_MAX,
                zoom_speed_label(speed).to_string(),
                theme,
                cx,
                |this, value, cx| {
                    this.update_selected_zoom(cx, move |segment| {
                        segment.transition_in_duration = Some(value);
                        segment.transition_out_duration = Some(value);
                    })
                },
            ));
            children.push(kit::hint("Duration of zoom in/out transitions", theme));

            children.push(kit::slider_row(
                "zoom-follow-smoothness",
                "Smooth Follow",
                settings.follow_smoothness,
                0.08,
                0.8,
                zoom_speed_label(settings.follow_smoothness).to_string(),
                theme,
                cx,
                |this, value, cx| {
                    this.update_zoom_settings(cx, move |settings| {
                        settings.follow_smoothness = value
                    })
                },
            ));
            children.push(kit::slider_row(
                "zoom-look-ahead",
                "Look Ahead",
                settings.look_ahead,
                0.0,
                0.3,
                format!("{}ms", (settings.look_ahead * 1000.0).round() as i64),
                theme,
                cx,
                |this, value, cx| {
                    this.update_zoom_settings(cx, move |settings| settings.look_ahead = value)
                },
            ));
            children.push(kit::reset_named(
                "zoom-reset-level",
                "Reset zoom level",
                theme,
                cx,
                |this, cx| {
                    this.update_selected_zoom(cx, |segment| segment.zoom_level = 1.2);
                },
            ));
            children.push(kit::reset_named(
                "zoom-reset-speed",
                "Reset zoom speed",
                theme,
                cx,
                |this, cx| {
                    this.update_selected_zoom(cx, |segment| {
                        segment.transition_in_duration = None;
                        segment.transition_out_duration = None;
                    });
                },
            ));
        }
    }

    kit::panel(children)
}

fn zoom_speed_label(value: f64) -> &'static str {
    if value <= 0.3 {
        "Very Fast"
    } else if value <= 0.6 {
        "Fast"
    } else if value <= 1.0 {
        "Medium"
    } else if value <= 1.5 {
        "Slow"
    } else {
        "Very Slow"
    }
}

const ZOOM_TARGET_MODES: [(&str, &str); 2] = [("cursor", "Cursor"), ("manual", "Manual")];

/// Port of `manual-zoom-preview.tsx`: the frame under the playhead with a
/// draggable crosshair marking where a manual zoom frames.
fn focus_picker(
    view: &VideoEditorWindow,
    focus: FocusPoint,
    theme: &ThemeVars,
    cx: &mut Context<VideoEditorWindow>,
) -> AnyElement {
    let frame = view.preview_image();
    let bounds: std::rc::Rc<std::cell::RefCell<Option<gpui::Bounds<gpui::Pixels>>>> =
        std::rc::Rc::new(std::cell::RefCell::new(None));

    let mut surface = div()
        .id("zoom-focus-picker")
        .relative()
        .w_full()
        .h(px(120.0))
        .rounded(px(6.0))
        .overflow_hidden()
        .border_1()
        .border_color(theme.border)
        .bg(theme.surface)
        .cursor_crosshair();

    if let Some(image) = frame {
        surface = surface.child(
            gpui::img(image)
                .absolute()
                .inset_0()
                .size_full()
                .object_fit(gpui::ObjectFit::Contain),
        );
    } else {
        surface = surface.child(
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(11.0))
                .text_color(theme.muted_foreground)
                .child("No frame yet"),
        );
    }

    let recorder = bounds.clone();
    surface = surface.child(
        gpui::canvas(
            move |laid_out, _window, _cx| {
                *recorder.borrow_mut() = Some(laid_out);
            },
            |_, (), _, _| {},
        )
        .absolute()
        .inset_0(),
    );

    // The crosshair marks the focus point; dragging anywhere moves it.
    surface = surface.child(
        div()
            .absolute()
            .left(gpui::relative(focus.x as f32))
            .top(gpui::relative(focus.y as f32))
            .child(
                div()
                    .size(px(14.0))
                    .ml(px(-7.0))
                    .mt(px(-7.0))
                    .rounded_full()
                    .border_2()
                    .border_color(crate::ui::colors::white(0.9))
                    .bg(theme.accent.opacity(0.6)),
            ),
    );

    let pick = {
        let bounds = bounds.clone();
        move |this: &mut VideoEditorWindow,
              position: gpui::Point<gpui::Pixels>,
              cx: &mut Context<VideoEditorWindow>| {
            let Some(area) = *bounds.borrow() else {
                return;
            };
            if area.size.width <= px(0.0) || area.size.height <= px(0.0) {
                return;
            }
            let x = f32::from(position.x - area.left()) / f32::from(area.size.width);
            let y = f32::from(position.y - area.top()) / f32::from(area.size.height);
            this.set_zoom_focus_point(x as f64, y as f64, cx);
        }
    };

    surface
        .on_mouse_down(gpui::MouseButton::Left, {
            let pick = pick.clone();
            cx.listener(move |this, event: &gpui::MouseDownEvent, _window, cx| {
                pick(this, event.position, cx);
            })
        })
        .on_mouse_move(
            cx.listener(move |this, event: &gpui::MouseMoveEvent, _window, cx| {
                if event.dragging() {
                    pick(this, event.position, cx);
                }
            }),
        )
        .into_any_element()
}

#[allow(dead_code)]
fn segment_row(title: &str, trailing: &str, theme: &ThemeVars) -> AnyElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .rounded(px(6.0))
        .border_1()
        .border_color(theme.border)
        .bg(theme.muted_background)
        .px(px(8.0))
        .py(px(6.0))
        .text_size(px(11.0))
        .child(div().text_color(theme.foreground).child(title.to_string()))
        .child(
            div()
                .text_color(theme.muted_foreground)
                .child(trailing.to_string()),
        )
        .into_any_element()
}

fn drawing_panel(
    view: &VideoEditorWindow,
    state: &VideoEditorState,
    menu: &MenuHandle,
    theme: &ThemeVars,
    cx: &mut Context<VideoEditorWindow>,
) -> AnyElement {
    let (tools, selected_id) = {
        let editor = view;
        (
            editor.drawing_tools.clone(),
            editor.selected_clip.clone().map(|id| id.to_string()),
        )
    };
    let selected = selected_id.as_deref().and_then(|id| {
        state
            .drawing_segments
            .iter()
            .find(|segment| segment.id == id)
    });
    let mut children = vec![
        kit::header(
            "Drawing",
            "Draw annotations directly on the video preview",
            None,
            theme,
            cx,
            |_, _, _| {},
        ),
        drawing_tool_grid(&tools.active_tool, theme, cx),
        kit::label("Style", theme),
    ];
    children.extend(drawing_style_rows(&tools, menu, theme, cx));
    children.push(kit::separator(theme));
    match selected {
        Some(_) => {
            children.push(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(kit::label("Selected Drawing", theme))
                    .child(
                        Button::new("drawing-delete")
                            .variant(ButtonVariant::Ghost)
                            .size(ButtonSize::IconXs)
                            .icon("trash-2")
                            .foreground(theme.destructive)
                            .on_click(cx.listener(|this, _event, _window, cx| {
                                this.delete_selected_drawing(cx)
                            })),
                    )
                    .into_any_element(),
            );
        }
        None => children.push(kit::note(
            "Select a drawing segment on the timeline to edit it.",
            theme,
        )),
    }
    kit::panel(children)
}

fn drawing_tool_grid(
    active: &str,
    theme: &ThemeVars,
    cx: &mut Context<VideoEditorWindow>,
) -> AnyElement {
    let buttons: Vec<AnyElement> = crate::ui::colors::VIDEO_DRAWING_TOOLS
        .iter()
        .map(|tool| {
            let selected = tool.id() == active;
            let id = tool.id();
            Button::new(SharedString::from(format!("drawing-tool-{id}")))
                .variant(if selected {
                    ButtonVariant::Tertiary
                } else {
                    ButtonVariant::Ghost
                })
                .size(ButtonSize::IconSm)
                .icon(tool.icon())
                .tooltip(tool.label())
                .on_click(cx.listener(move |this, _event, _window, cx| {
                    this.update_drawing_tools(cx, |tools| tools.active_tool = id.to_string());
                }))
                .into_any_element()
        })
        .collect();
    let _ = theme;
    div()
        .flex()
        .flex_row()
        .flex_wrap()
        .gap(px(crate::ui::chrome::DRAWING_TOOL_GRID_GAP))
        .children(buttons)
        .into_any_element()
}

fn drawing_style_rows(
    tools: &styles::DrawingToolSettings,
    menu: &MenuHandle,
    theme: &ThemeVars,
    cx: &mut Context<VideoEditorWindow>,
) -> Vec<AnyElement> {
    use crate::editor::options::{
        ARROW_STYLES, FONT_FAMILIES, FONT_SIZES, HIGHLIGHT_OPACITIES, NUMBER_SIZES,
        NUMBER_START_VALUES, NUMBER_STYLES, REDACT_INTENSITIES, REDACT_STYLES, SHAPE_FILL_MODES,
    };
    let tool = tools.active_tool.as_str();
    let mut rows = Vec::new();
    let color = if tool == "highlight" {
        tools.highlight_color.as_str()
    } else {
        tools.selected_color.as_str()
    };
    rows.push(kit::select_row(
        "drawing-color",
        if tool == "highlight" {
            "Highlight"
        } else {
            "Color"
        },
        color,
        if tool == "highlight" {
            &[
                ("#FFFF00", "Yellow"),
                ("#00FF00", "Green"),
                ("#FF69B4", "Pink"),
                ("#00BFFF", "Blue"),
                ("#FFA500", "Orange"),
            ]
        } else {
            &[
                ("#FF3B30", "Red"),
                ("#f97316", "Orange"),
                ("#f59e0b", "Amber"),
                ("#22c55e", "Green"),
                ("#3b82f6", "Blue"),
                ("#8b5cf6", "Violet"),
                ("#000000", "Black"),
                ("#ffffff", "White"),
            ]
        },
        menu,
        theme,
        cx,
        move |this, value, cx| {
            this.update_drawing_tools(cx, |tools| {
                if tools.active_tool == "highlight" {
                    tools.highlight_color = value.clone();
                } else {
                    tools.selected_color = value.clone();
                }
            });
        },
    ));
    if matches!(tool, "pen" | "rectangle" | "circle" | "line" | "arrow") {
        rows.push(kit::slider_row(
            "drawing-thickness",
            "Thickness",
            tools.stroke_width,
            crate::ui::chrome::DRAWING_STROKE_MIN,
            crate::ui::chrome::DRAWING_STROKE_MAX,
            format!("{}", tools.stroke_width.round() as i32),
            theme,
            cx,
            |this, value, cx| {
                this.update_drawing_tools(cx, |tools| tools.stroke_width = value.round())
            },
        ));
    }
    if tool == "arrow" {
        rows.push(kit::select_row(
            "drawing-arrow",
            "Arrow",
            &tools.arrow_style,
            &ARROW_STYLES,
            menu,
            theme,
            cx,
            |this, value, cx| {
                this.update_drawing_tools(cx, |tools| tools.arrow_style = value.clone())
            },
        ));
    }
    if tool == "highlight" {
        let opacity_options: [(&str, &str); 5] = [
            ("0.2", "20%"),
            ("0.3", "30%"),
            ("0.4", "40%"),
            ("0.5", "50%"),
            ("0.6", "60%"),
        ];
        let current = format!("{}", tools.highlight_opacity);
        let current = HIGHLIGHT_OPACITIES
            .iter()
            .find(|value| (**value - tools.highlight_opacity).abs() < 0.01)
            .map(|value| format!("{value}"))
            .unwrap_or(current);
        rows.push(kit::select_row(
            "drawing-highlight-opacity",
            "Opacity",
            &current,
            &opacity_options,
            menu,
            theme,
            cx,
            |this, value, cx| {
                if let Ok(parsed) = value.parse::<f64>() {
                    this.update_drawing_tools(cx, |tools| tools.highlight_opacity = parsed);
                }
            },
        ));
    }
    if matches!(tool, "rectangle" | "circle") {
        rows.push(kit::tab_row(
            "drawing-fill",
            "Fill",
            &tools.shape_fill_mode,
            &SHAPE_FILL_MODES,
            theme,
            cx,
            |this, value, cx| {
                this.update_drawing_tools(cx, |tools| tools.shape_fill_mode = value.clone())
            },
        ));
    }
    if tool == "number" {
        rows.push(kit::select_row(
            "drawing-number-style",
            "Number",
            &tools.number_style,
            &NUMBER_STYLES,
            menu,
            theme,
            cx,
            |this, value, cx| {
                this.update_drawing_tools(cx, |tools| tools.number_style = value.clone())
            },
        ));
        rows.push(kit::tab_row(
            "drawing-number-size",
            "Size",
            &tools.number_size,
            &NUMBER_SIZES,
            theme,
            cx,
            |this, value, cx| {
                this.update_drawing_tools(cx, |tools| tools.number_size = value.clone())
            },
        ));
        let start = format!("{}", tools.number_start_value as i32);
        let starts: Vec<(&'static str, &'static str)> = NUMBER_START_VALUES
            .iter()
            .map(|value| {
                let label: &'static str = match *value as i32 {
                    1 => "1",
                    2 => "2",
                    3 => "3",
                    4 => "4",
                    5 => "5",
                    6 => "6",
                    7 => "7",
                    8 => "8",
                    9 => "9",
                    10 => "10",
                    _ => "1",
                };
                (label, label)
            })
            .collect();
        rows.push(kit::select_row(
            "drawing-number-start",
            "Start",
            &start,
            &starts,
            menu,
            theme,
            cx,
            |this, value, cx| {
                if let Ok(parsed) = value.parse::<f64>() {
                    this.update_drawing_tools(cx, |tools| tools.number_start_value = parsed);
                }
            },
        ));
    }
    if tool == "text" {
        let size = format!("{}", tools.text_font_size as i32);
        let sizes: Vec<(&'static str, &'static str)> = FONT_SIZES
            .iter()
            .map(|value| {
                let label: &'static str = match *value as i32 {
                    12 => "12",
                    16 => "16",
                    20 => "20",
                    24 => "24",
                    28 => "28",
                    32 => "32",
                    40 => "40",
                    48 => "48",
                    64 => "64",
                    72 => "72",
                    84 => "84",
                    92 => "92",
                    _ => "24",
                };
                (label, label)
            })
            .collect();
        rows.push(kit::select_row(
            "drawing-text-size",
            "Text",
            &size,
            &sizes,
            menu,
            theme,
            cx,
            |this, value, cx| {
                if let Ok(parsed) = value.parse::<f64>() {
                    this.update_drawing_tools(cx, |tools| tools.text_font_size = parsed);
                }
            },
        ));
        rows.push(kit::select_row(
            "drawing-text-font",
            "Font",
            &tools.text_font_family,
            &FONT_FAMILIES,
            menu,
            theme,
            cx,
            |this, value, cx| {
                this.update_drawing_tools(cx, |tools| tools.text_font_family = value.clone())
            },
        ));
        rows.push(kit::switch_row(
            "drawing-text-background",
            "Background",
            tools.text_background,
            theme,
            cx,
            |this, value, cx| this.update_drawing_tools(cx, |tools| tools.text_background = value),
        ));
    }
    if tool == "redact" {
        let redact: [(&str, &str); 3] = REDACT_STYLES.map(|(value, label, _)| (value, label));
        rows.push(kit::select_row(
            "drawing-redact-style",
            "Redact",
            &tools.redact_style,
            &redact,
            menu,
            theme,
            cx,
            |this, value, cx| {
                this.update_drawing_tools(cx, |tools| tools.redact_style = value.clone())
            },
        ));
        rows.push(kit::slider_row(
            "drawing-redact-intensity",
            "Intensity",
            tools.redact_intensity,
            *REDACT_INTENSITIES.first().unwrap_or(&1.0),
            *REDACT_INTENSITIES.last().unwrap_or(&10.0),
            format!("{}", tools.redact_intensity.round() as i32),
            theme,
            cx,
            |this, value, cx| {
                this.update_drawing_tools(cx, |tools| tools.redact_intensity = value.round())
            },
        ));
    }
    rows
}

fn camera_panel(
    state: &VideoEditorState,
    has_camera: bool,
    theme: &ThemeVars,
    cx: &mut Context<VideoEditorWindow>,
) -> AnyElement {
    if !has_camera {
        return kit::empty_state(
            "No camera recording available for this video.\nEnable camera during recording to use camera overlay.",
            theme,
        );
    }

    let style = state.camera_style.clone();
    let mut children = vec![
        kit::header(
            "Camera Overlay",
            "Customize camera appearance in your video",
            None,
            theme,
            cx,
            |_, _, _| {},
        ),
        kit::switch_row(
            "camera-visible",
            "Show Camera",
            style.visible,
            theme,
            cx,
            |this, value, cx| this.update_camera(cx, move |style| style.visible = value),
        ),
    ];

    if !style.visible {
        return kit::panel(children);
    }

    children.extend([
        kit::switch_row(
            "camera-mirrored",
            "Mirror",
            style.mirrored,
            theme,
            cx,
            |this, value, cx| this.update_camera(cx, move |style| style.mirrored = value),
        ),
        kit::field(
            "Position",
            camera_position_grid(&style.position, theme, cx),
            theme,
        ),
        kit::tab_row(
            "camera-size",
            "Size",
            &style.size,
            &styles::SIZE_OPTIONS,
            theme,
            cx,
            |this, value, cx| this.update_camera(cx, move |style| style.size = value.clone()),
        ),
        kit::tab_row(
            "camera-shape",
            "Shape",
            &style.shape,
            &styles::CAMERA_SHAPES,
            theme,
            cx,
            |this, value, cx| this.update_camera(cx, move |style| style.shape = value.clone()),
        ),
        kit::slider_row(
            "camera-padding",
            "Edge Padding",
            style.padding,
            0.0,
            styles::CAMERA_PADDING_MAX,
            format!("{}", style.padding.round() as i32),
            theme,
            cx,
            |this, value, cx| this.update_camera(cx, move |style| style.padding = value.round()),
        ),
        kit::slider_row(
            "camera-corners",
            "Radius",
            style.border_radius,
            0.0,
            styles::CAMERA_RADIUS_MAX,
            format!("{}", style.border_radius.round() as i32),
            theme,
            cx,
            |this, value, cx| {
                this.update_camera(cx, move |style| style.border_radius = value.round())
            },
        ),
        kit::slider_row(
            "camera-shadow",
            "Shadow",
            style.shadow,
            0.0,
            styles::CAMERA_SHADOW_MAX,
            format!("{}", style.shadow.round() as i32),
            theme,
            cx,
            |this, value, cx| this.update_camera(cx, move |style| style.shadow = value.round()),
        ),
        kit::reset_button("camera-reset", theme, cx, |this, cx| {
            this.update_camera(cx, |style| *style = CameraStyle::default());
        }),
    ]);

    kit::panel(children)
}

fn camera_position_grid(
    value: &str,
    theme: &ThemeVars,
    cx: &mut Context<VideoEditorWindow>,
) -> AnyElement {
    let mut rows = Vec::new();
    for (row_index, row) in styles::CAMERA_POSITION_GRID.iter().enumerate() {
        let cells: Vec<AnyElement> = row
            .iter()
            .enumerate()
            .map(|(col_index, position)| {
                let selected = *position == value;
                let position = (*position).to_string();
                div()
                    .id(SharedString::from(format!(
                        "camera-pos-{row_index}-{col_index}"
                    )))
                    .h(px(24.0))
                    .flex_1()
                    .rounded(px(8.0))
                    .bg(if selected {
                        theme.accent
                    } else {
                        theme.default
                    })
                    .cursor_pointer()
                    .on_mouse_down(gpui::MouseButton::Left, {
                        cx.listener(move |this, _event, _window, cx| {
                            let value = position.clone();
                            this.update_camera(cx, move |style| style.position = value);
                        })
                    })
                    .into_any_element()
            })
            .collect();
        rows.push(
            div()
                .flex()
                .flex_row()
                .gap(px(4.0))
                .children(cells)
                .into_any_element(),
        );
    }
    div()
        .flex()
        .flex_col()
        .gap(px(4.0))
        .children(rows)
        .into_any_element()
}

fn audio_panel(
    view: &VideoEditorWindow,
    state: &VideoEditorState,
    has_keyboard: bool,
    menu: &MenuHandle,
    theme: &ThemeVars,
    cx: &mut Context<VideoEditorWindow>,
) -> AnyElement {
    let style = state.audio_style.clone();
    let mut children = vec![div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
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
                        .child("Audio Tracks"),
                )
                .child(
                    div()
                        .text_size(px(crate::ui::chrome::SETTINGS_HEADER_DESC))
                        .text_color(theme.muted_foreground)
                        .child("Manage audio tracks in your project"),
                ),
        )
        .child(
            Button::new("audio-add-music")
                .variant(ButtonVariant::Ghost)
                .size(ButtonSize::IconXs)
                .icon("plus")
                .tooltip("Add music")
                .on_click(cx.listener(|this, _event, _window, cx| this.add_music_track(cx))),
        )
        .into_any_element()];

    let groups = music_groups(&state.music_tracks);
    if groups.is_empty() {
        children.push(kit::hint("No audio tracks.", theme));
    }
    for track in groups {
        children.push(music_row(track, menu, theme, cx));
    }

    if has_keyboard {
        children.push(kit::switch_row(
            "audio-keyboard-enabled",
            "Keyboard Sound",
            style.keyboard_sound_enabled,
            theme,
            cx,
            |this, value, cx| {
                this.update_audio(cx, move |style| style.keyboard_sound_enabled = value)
            },
        ));
        if style.keyboard_sound_enabled {
            let demo = view.is_keyboard_demo_playing();
            children.push(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .child(kit::select_row(
                        "audio-keyboard-type",
                        "Keyboard sound",
                        &style.keyboard_sound_type,
                        &styles::KEYBOARD_SOUND_TYPES,
                        menu,
                        theme,
                        cx,
                        |this, value, cx| {
                            this.update_audio(cx, move |style| {
                                style.keyboard_sound_type = value.clone()
                            })
                        },
                    ))
                    .child(
                        Button::new("audio-keyboard-demo")
                            .variant(ButtonVariant::Tertiary)
                            .size(ButtonSize::IconXs)
                            .icon(if demo { "square" } else { "play" })
                            .tooltip(if demo { "Stop demo" } else { "Play demo" })
                            .on_click(cx.listener(move |this, _event, _window, cx| {
                                if this.is_keyboard_demo_playing() {
                                    this.stop_keyboard_demo(cx);
                                } else {
                                    this.play_keyboard_demo(cx);
                                }
                            })),
                    )
                    .into_any_element(),
            );
            children.push(kit::slider_row(
                "audio-keyboard-volume",
                "Volume",
                style.keyboard_sound_volume,
                0.0,
                1.0,
                kit::percent(style.keyboard_sound_volume),
                theme,
                cx,
                |this, value, cx| {
                    this.update_audio(cx, move |style| style.keyboard_sound_volume = value)
                },
            ));
        }
    }

    kit::panel(children)
}

fn music_groups(
    tracks: &[crate::windows::video_editor::model::MusicTrack],
) -> Vec<&crate::windows::video_editor::model::MusicTrack> {
    let mut seen = Vec::new();
    let mut groups = Vec::new();
    for track in tracks {
        let key = if track.group_id.is_empty() {
            track.id.as_str()
        } else {
            track.group_id.as_str()
        };
        if seen.contains(&key) {
            continue;
        }
        seen.push(key);
        groups.push(track);
    }
    groups
}

/// One music track: its name, an enable toggle, a volume slider and a remove
/// button, mirroring the rows in `audio-settings-panel.tsx`.
fn music_row(
    track: &crate::windows::video_editor::model::MusicTrack,
    menu: &MenuHandle,
    theme: &ThemeVars,
    cx: &mut Context<VideoEditorWindow>,
) -> AnyElement {
    let id = SharedString::from(track.id.clone());
    let volume = track.volume;
    let speed = track.speed;
    let removable = track.source == "music";
    let mut body = div()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .rounded(px(6.0))
        .border_1()
        .border_color(theme.border)
        .p(px(12.0))
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .gap(px(8.0))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.0))
                        .min_w_0()
                        .child(crate::ui::icon::icon_element("music", px(16.0)))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_size(px(crate::ui::chrome::TEXT_SM))
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .child(SharedString::from(track.name.clone())),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(4.0))
                        .child(
                            crate::ui::switch::Switch::new(
                                SharedString::from(format!("music-enabled-{id}")),
                                track.enabled,
                            )
                            .size(crate::ui::switch::SwitchSize::Sm)
                            .on_change(cx.listener({
                                let id = id.clone();
                                move |this, value: &bool, _window, cx| {
                                    this.set_music_enabled(id.clone(), *value, cx)
                                }
                            })),
                        )
                        .when(removable, |el| {
                            el.child(
                                Button::new(SharedString::from(format!("music-remove-{id}")))
                                    .variant(ButtonVariant::Ghost)
                                    .size(ButtonSize::IconXs)
                                    .icon("trash-2")
                                    .on_click(cx.listener({
                                        let id = id.clone();
                                        move |this, _event, _window, cx| {
                                            this.remove_music_track(id.clone(), cx)
                                        }
                                    })),
                            )
                        }),
                ),
        );

    if track.enabled {
        let speed_value = format_music_speed(speed);
        body = body
            .child(kit::slider_row(
                "music-volume",
                "Volume",
                volume,
                0.0,
                1.0,
                kit::percent(volume),
                theme,
                cx,
                {
                    let id = id.clone();
                    move |this, value, cx| this.set_music_volume(id.clone(), value, cx)
                },
            ))
            .child(kit::select_row(
                "music-speed",
                "Speed",
                &speed_value,
                &styles::MUSIC_SPEEDS,
                menu,
                theme,
                cx,
                {
                    let id = id.clone();
                    move |this, value, cx| {
                        if let Ok(speed) = value.parse::<f64>() {
                            this.set_music_speed(id.clone(), speed, cx);
                        }
                    }
                },
            ));
    }

    body.into_any_element()
}

fn format_music_speed(speed: f64) -> String {
    if (speed - speed.round()).abs() < 0.001 {
        format!("{}", speed.round() as i32)
    } else {
        format!("{speed}")
    }
}

fn wallpaper_panel(
    state: &VideoEditorState,
    theme: &ThemeVars,
    window: &mut gpui::Window,
    cx: &mut Context<VideoEditorWindow>,
) -> AnyElement {
    let wallpaper = state.wallpaper.clone();
    let enabled = wallpaper.enabled;
    let is_ios = state.recording_type.as_deref() == Some("ios-device");
    let mut children = vec![
        kit::header(
            "Wallpaper",
            "Add background and styling to your video",
            None,
            theme,
            cx,
            |_, _, _| {},
        ),
        video_backgrounds(&wallpaper, theme, cx),
    ];

    if is_ios && enabled {
        children.push(kit::switch_row(
            "wallpaper-device-frame",
            "Device Frame",
            wallpaper.device_frame,
            theme,
            cx,
            |this, value, cx| {
                this.update_wallpaper(cx, move |wallpaper| wallpaper.device_frame = value)
            },
        ));
        children.push(kit::separator(theme));
    }

    if enabled {
        children.push(kit::separator(theme));
        let selected = crate::video::composition::wallpaper::aspect_ratio(&wallpaper)
            .map(|ratio| (ratio.width, ratio.height));
        children.push(crate::editor::wallpaper_sheet::video_aspect_grid(
            selected,
            theme,
            {
                let entity = cx.entity().downgrade();
                move |value, _window, cx| {
                    if let Some(entity) = entity.upgrade() {
                        entity.update(cx, |this, cx| {
                            this.update_wallpaper(cx, |wallpaper| {
                                wallpaper.aspect_ratio = value.map(|(width, height)| {
                                    let name = if width == 0.0 && height == 0.0 {
                                        "Free".to_string()
                                    } else {
                                        format!("{}:{}", width as i32, height as i32)
                                    };
                                    serde_json::json!({
                                        "name": name,
                                        "width": width,
                                        "height": height,
                                    })
                                });
                            });
                        });
                    }
                }
            },
            window,
            cx,
        ));
        children.push(kit::separator(theme));
        children.push(kit::slider_row(
            "wallpaper-padding",
            "Padding",
            wallpaper.padding,
            0.0,
            crate::editor::wallpaper::VIDEO_PADDING_MAX,
            format!("{}", wallpaper.padding.round() as i32),
            theme,
            cx,
            |this, value, cx| {
                this.update_wallpaper(cx, move |wallpaper| wallpaper.padding = value.round())
            },
        ));
        if !wallpaper.device_frame {
            children.push(kit::slider_row(
                "wallpaper-corners",
                "Corners",
                wallpaper.corners,
                0.0,
                crate::editor::wallpaper::VIDEO_CORNERS_MAX,
                format!("{}", wallpaper.corners.round() as i32),
                theme,
                cx,
                |this, value, cx| {
                    this.update_wallpaper(cx, move |wallpaper| wallpaper.corners = value.round())
                },
            ));
        }
        children.push(kit::slider_row(
            "wallpaper-shadow",
            "Shadow",
            wallpaper.shadow,
            0.0,
            crate::editor::wallpaper::VIDEO_SHADOW_MAX,
            format!("{}", wallpaper.shadow.round() as i32),
            theme,
            cx,
            |this, value, cx| {
                this.update_wallpaper(cx, move |wallpaper| wallpaper.shadow = value.round())
            },
        ));
    }

    kit::panel(children)
}

fn video_backgrounds(
    wallpaper: &crate::windows::video_editor::styles::VideoWallpaperSettings,
    theme: &ThemeVars,
    cx: &mut Context<VideoEditorWindow>,
) -> AnyElement {
    use crate::config::schema::CustomBackgroundData;
    use crate::editor::wallpaper;
    use crate::editor::wallpaper_sheet::{gradient_tile, icon_tile};
    use crate::ui::button::{Button, ButtonSize, ButtonVariant};
    use crate::ui::chrome;

    let tile = chrome::wallpaper_tile_size(chrome::VIDEO_SIDEBAR_WIDTH, chrome::VIDEO_PANEL_PAD);
    let config = crate::state::state(cx).config.get();
    let customs = config.wallpaper.custom_backgrounds;
    let current_gradient_id = wallpaper
        .gradient
        .as_ref()
        .and_then(|value| value.get("id"))
        .and_then(|value| value.as_str())
        .map(str::to_string);
    let no_wallpaper = !wallpaper.enabled;
    let entity = cx.entity().downgrade();
    let disable = entity.clone();
    let mut tiles = vec![icon_tile(
        "video-wallpaper-none",
        "No Wallpaper",
        "ban",
        tile,
        no_wallpaper,
        theme,
        move |_, cx| {
            if let Some(entity) = disable.upgrade() {
                entity.update(cx, |this, cx| {
                    this.update_wallpaper(cx, |wallpaper| wallpaper.enabled = false);
                });
            }
        },
    )];

    let desktop = entity.clone();
    let is_desktop = wallpaper.enabled
        && wallpaper.background_image.is_some()
        && wallpaper.gradient.is_none()
        && !customs.iter().any(|background| match &background.data {
            CustomBackgroundData::Image { data } => {
                wallpaper.background_image.as_deref() == Some(data.image_url.as_str())
            }
            CustomBackgroundData::Gradient { .. } => false,
        });
    tiles.push(icon_tile(
        "video-wallpaper-desktop",
        "Use Desktop Wallpaper",
        "monitor",
        tile,
        is_desktop,
        theme,
        move |_, cx| {
            if let Some(entity) = desktop.upgrade() {
                entity.update(cx, |this, cx| {
                    let source = crate::editor::background::desktop_wallpaper(
                        &crate::state::state(cx).daemon,
                    );
                    this.update_wallpaper(cx, |wallpaper| {
                        if let Some(source) = source.clone() {
                            wallpaper.enabled = true;
                            wallpaper.background_image = Some(source);
                            wallpaper.gradient = None;
                            if wallpaper.padding == 0.0 {
                                wallpaper.padding = 50.0;
                            }
                        }
                    });
                });
            }
        },
    ));

    for (index, (id, name, colors, angle)) in wallpaper::SVG_PRESETS.iter().enumerate() {
        let selected = wallpaper.enabled && current_gradient_id.as_deref() == Some(*id);
        let entity = entity.clone();
        let preset_id = (*id).to_string();
        let preset_colors: Vec<String> = colors.iter().map(|color| (*color).to_string()).collect();
        let preset_angle = *angle;
        tiles.push(gradient_tile(
            gpui::ElementId::Integer(2000 + index as u64),
            name,
            colors,
            *angle,
            tile,
            selected,
            theme,
            move |_, cx| {
                if let Some(entity) = entity.upgrade() {
                    entity.update(cx, |this, cx| {
                        this.update_wallpaper(cx, |wallpaper| {
                            wallpaper.enabled = true;
                            wallpaper.gradient = Some(serde_json::json!({
                                "id": preset_id,
                                "colors": preset_colors,
                                "angle": preset_angle,
                            }));
                            wallpaper.background_image = None;
                            if wallpaper.padding == 0.0 {
                                wallpaper.padding = 50.0;
                            }
                        });
                    });
                }
            },
        ));
    }

    for (index, background) in customs.iter().enumerate() {
        let entity = entity.clone();
        let background = background.clone();
        match &background.data {
            CustomBackgroundData::Gradient { data } => {
                let selected = wallpaper.enabled
                    && current_gradient_id.as_deref() == Some(data.gradient.id.as_str());
                let colors: Vec<&str> = data.gradient.colors.iter().map(String::as_str).collect();
                let pair = [
                    colors.first().copied().unwrap_or("#000000"),
                    colors.last().copied().unwrap_or("#ffffff"),
                ];
                let gradient = data.gradient.clone();
                tiles.push(gradient_tile(
                    gpui::ElementId::Name(gpui::SharedString::from(format!(
                        "video-custom-{index}"
                    ))),
                    background.id.as_str(),
                    &pair,
                    gradient.angle,
                    tile,
                    selected,
                    theme,
                    move |_, cx| {
                        if let Some(entity) = entity.upgrade() {
                            entity.update(cx, |this, cx| {
                                this.update_wallpaper(cx, |wallpaper| {
                                    wallpaper.enabled = true;
                                    wallpaper.gradient = serde_json::to_value(&gradient).ok();
                                    wallpaper.background_image = None;
                                    if wallpaper.padding == 0.0 {
                                        wallpaper.padding = 50.0;
                                    }
                                });
                            });
                        }
                    },
                ));
            }
            CustomBackgroundData::Image { data } => {
                let selected = wallpaper.enabled
                    && wallpaper.background_image.as_deref() == Some(data.image_url.as_str());
                let path = data.image_url.clone();
                tiles.push(icon_tile(
                    gpui::ElementId::Name(gpui::SharedString::from(format!(
                        "video-custom-image-{index}"
                    ))),
                    "Custom image",
                    "image",
                    tile,
                    selected,
                    theme,
                    move |_, cx| {
                        if let Some(entity) = entity.upgrade() {
                            entity.update(cx, |this, cx| {
                                this.update_wallpaper(cx, |wallpaper| {
                                    wallpaper.enabled = true;
                                    wallpaper.background_image = Some(path.clone());
                                    wallpaper.gradient = None;
                                    if wallpaper.padding == 0.0 {
                                        wallpaper.padding = 50.0;
                                    }
                                });
                            });
                        }
                    },
                ));
            }
        }
    }

    let add = entity.clone();
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
                .child(kit::label("Backgrounds", theme))
                .child(
                    Button::new("video-wallpaper-add")
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::IconXs)
                        .icon("plus")
                        .foreground(theme.muted_foreground)
                        .tooltip("Add Background")
                        .on_click(move |_event, _window, cx| {
                            if let Some(entity) = add.upgrade() {
                                entity.update(cx, |this, cx| {
                                    if let Some(path) = crate::editor::background::pick_image() {
                                        this.update_wallpaper(cx, |wallpaper| {
                                            wallpaper.enabled = true;
                                            wallpaper.background_image = Some(path);
                                            wallpaper.gradient = None;
                                            if wallpaper.padding == 0.0 {
                                                wallpaper.padding = 50.0;
                                            }
                                        });
                                    }
                                });
                            }
                        }),
                ),
        )
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

fn keyboard_panel(
    state: &VideoEditorState,
    has_keyboard: bool,
    theme: &ThemeVars,
    cx: &mut Context<VideoEditorWindow>,
) -> AnyElement {
    if !has_keyboard {
        return kit::empty_state("No keyboard data available for this video.", theme);
    }

    let style = state.keyboard_style.clone();
    let mut children = vec![
        kit::header(
            "Keyboard Overlay",
            "Display key presses during playback",
            None,
            theme,
            cx,
            |_, _, _| {},
        ),
        kit::switch_row(
            "keyboard-visible",
            "Show Keys",
            style.visible,
            theme,
            cx,
            |this, value, cx| this.update_keyboard(cx, move |style| style.visible = value),
        ),
    ];

    if style.visible {
        children.push(kit::tab_row(
            "keyboard-font-size",
            "Size",
            &style.font_size,
            &styles::SIZE_OPTIONS,
            theme,
            cx,
            |this, value, cx| {
                this.update_keyboard(cx, move |style| style.font_size = value.clone())
            },
        ));
        children.push(kit::reset_button(
            "keyboard-reset",
            theme,
            cx,
            |this, cx| {
                this.update_keyboard(cx, |style| *style = KeyboardStyle::default());
            },
        ));
    }

    kit::panel(children)
}

fn subtitle_panel(
    view: &VideoEditorWindow,
    state: &VideoEditorState,
    has_mic: bool,
    theme: &ThemeVars,
    cx: &mut Context<VideoEditorWindow>,
) -> AnyElement {
    let (has_subtitles, count, is_transcribing, model) = {
        let editor = view;
        (
            editor.has_subtitles(),
            editor.subtitle_count(),
            editor.is_transcribing(),
            editor.transcription_model().to_string(),
        )
    };

    if !has_subtitles {
        let description = if has_mic {
            "Generate subtitles from microphone audio or import manually"
        } else {
            "No microphone audio available. Import subtitles manually."
        };
        let mut children = vec![kit::header(
            "Subtitles",
            description,
            None,
            theme,
            cx,
            |_, _, _| {},
        )];
        if has_mic {
            children.push(kit::tab_row(
                "subtitle-model",
                "Model",
                &model,
                &WHISPER_MODELS,
                theme,
                cx,
                |this, value, cx| this.set_transcription_model(value.to_string(), cx),
            ));
            let prompt_field = view.prompt_field.clone();
            children.push(kit::field(
                "Custom Prompt (optional)",
                prompt_field.into_any_element(),
                theme,
            ));
            children.push(kit::hint(
                "Add context like speaker names, technical terms, or topics",
                theme,
            ));
            children.push(kit::tertiary_button(
                "subtitle-generate",
                if is_transcribing {
                    "Generating..."
                } else {
                    "Generate Subtitles"
                },
                "subtitles",
                is_transcribing,
                theme,
                cx,
                |this, cx| this.generate_subtitles(cx),
            ));
            children.push(kit::separator(theme));
        }
        children.push(kit::note(
            if has_mic {
                "You can also add subtitles manually by importing a JSON/SRT file or creating them in the editor."
            } else {
                "You can add subtitles manually by importing a JSON/SRT file or creating them in the editor."
            },
            theme,
        ));
        children.push(kit::tertiary_button(
            "subtitle-import",
            "Import from File",
            "file-up",
            false,
            theme,
            cx,
            |this, cx| this.import_subtitles(cx),
        ));
        children.push(kit::tertiary_button(
            "subtitle-create",
            "Create Manually",
            "edit-3",
            false,
            theme,
            cx,
            |this, cx| this.open_data_editor(data_editor::DataKind::Subtitle, cx),
        ));
        children.push(format_card(
            "Subtitle Data Format",
            "Subtitle data is a JSON file containing segments with start/end times (in seconds) and text content. You can also import standard SRT files.",
            theme,
        ));
        return kit::panel(children);
    }

    let style = state.subtitle_style.clone();
    let mut children = vec![kit::header(
        "Subtitles",
        "Show subtitles in your video",
        Some(style.visible),
        theme,
        cx,
        |this, value, cx| this.update_subtitle(cx, move |style| style.visible = value),
    )];

    if !style.visible {
        children.push(kit::note(
            "Subtitles are disabled. Enable them to show subtitles in your video.",
            theme,
        ));
        return kit::panel(children);
    }

    children.extend([
        kit::tab_row(
            "subtitle-font-size",
            "Size",
            &style.font_size,
            &styles::SIZE_OPTIONS,
            theme,
            cx,
            |this, value, cx| {
                this.update_subtitle(cx, move |style| style.font_size = value.clone())
            },
        ),
        kit::tab_row(
            "subtitle-position",
            "Position",
            &style.position,
            &styles::SUBTITLE_POSITIONS,
            theme,
            cx,
            |this, value, cx| this.update_subtitle(cx, move |style| style.position = value.clone()),
        ),
        kit::tab_row(
            "subtitle-background",
            "Background",
            &style.background_color,
            &styles::SUBTITLE_BACKGROUNDS,
            theme,
            cx,
            |this, value, cx| {
                this.update_subtitle(cx, move |style| style.background_color = value.clone())
            },
        ),
    ]);
    children.push(kit::separator(theme));
    children.push(kit::label("Subtitle Data", theme));
    children.push(kit::hint(format!("{count} segments"), theme));
    children.push(
        div()
            .flex()
            .flex_row()
            .gap(px(8.0))
            .child(kit::tertiary_button(
                "subtitle-edit-data",
                "Edit",
                "edit-3",
                false,
                theme,
                cx,
                |this, cx| this.open_data_editor(data_editor::DataKind::Subtitle, cx),
            ))
            .child(kit::tertiary_button(
                "subtitle-import-data",
                "Import",
                "file-up",
                false,
                theme,
                cx,
                |this, cx| this.import_subtitles(cx),
            ))
            .into_any_element(),
    );
    if has_mic {
        children.push(
            div()
                .flex()
                .flex_row()
                .gap(px(8.0))
                .child(kit::tertiary_button(
                    "subtitle-regenerate",
                    "Regenerate",
                    "subtitles",
                    is_transcribing,
                    theme,
                    cx,
                    |this, cx| this.generate_subtitles(cx),
                ))
                .child(kit::tertiary_button(
                    "subtitle-delete",
                    "Delete",
                    "trash-2",
                    is_transcribing,
                    theme,
                    cx,
                    |this, cx| this.delete_subtitles(cx),
                ))
                .into_any_element(),
        );
    } else {
        children.push(kit::tertiary_button(
            "subtitle-delete",
            "Delete Subtitles",
            "trash-2",
            false,
            theme,
            cx,
            |this, cx| this.delete_subtitles(cx),
        ));
    }
    children.push(kit::reset_button(
        "subtitle-reset",
        theme,
        cx,
        |this, cx| {
            this.update_subtitle(cx, |style| *style = SubtitleStyle::default());
        },
    ));

    kit::panel(children)
}

/// `WHISPER_MODELS` in `types/subtitle.ts`.
const WHISPER_MODELS: [(&str, &str); 3] =
    [("base", "Base"), ("small", "Small"), ("medium", "Medium")];

fn first_frame_panel(
    state: &VideoEditorState,
    theme: &ThemeVars,
    cx: &mut Context<VideoEditorWindow>,
) -> AnyElement {
    let settings = state.first_frame.clone();
    let mut children = vec![kit::header(
        "First Frame",
        "Add a thumbnail image shown as the first frame of your video",
        None,
        theme,
        cx,
        |_, _, _| {},
    )];

    match settings.image_data.as_deref() {
        None => {
            children.push(kit::tertiary_button(
                "first-frame-upload",
                "Upload Image",
                "frame",
                false,
                theme,
                cx,
                |this, cx| this.pick_first_frame(cx),
            ));
        }
        Some(path) => {
            children.push(
                div()
                    .w_full()
                    .rounded(px(8.0))
                    .border_1()
                    .border_color(theme.border)
                    .overflow_hidden()
                    .child(
                        gpui::img(std::path::PathBuf::from(path))
                            .w_full()
                            .h(px(144.0))
                            .object_fit(gpui::ObjectFit::Cover),
                    )
                    .into_any_element(),
            );
            children.push(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(8.0))
                    .child(kit::tertiary_button(
                        "first-frame-replace",
                        "Replace",
                        "frame",
                        false,
                        theme,
                        cx,
                        |this, cx| this.pick_first_frame(cx),
                    ))
                    .child(
                        Button::new("first-frame-remove")
                            .variant(ButtonVariant::Tertiary)
                            .size(ButtonSize::IconXs)
                            .icon("trash-2")
                            .on_click(
                                cx.listener(|this, _event, _window, cx| this.clear_first_frame(cx)),
                            ),
                    )
                    .into_any_element(),
            );
            children.push(kit::tab_row(
                "first-frame-fit",
                "Fit Mode",
                &settings.fit,
                &styles::FIRST_FRAME_FITS,
                theme,
                cx,
                |this, value, cx| {
                    this.update_first_frame(cx, move |settings| settings.fit = value.clone())
                },
            ));
        }
    }

    kit::panel(children)
}

fn export_panel(
    view: &VideoEditorWindow,
    state: &VideoEditorState,
    is_exporting: bool,
    export_progress: f32,
    menu: &MenuHandle,
    theme: &ThemeVars,
    cx: &mut Context<VideoEditorWindow>,
) -> AnyElement {
    let settings = state.export_settings.clone();
    let is_gif = settings.format == "gif";
    let resolutions: Vec<(&'static str, &'static str)> = styles::EXPORT_RESOLUTIONS
        .iter()
        .copied()
        .filter(|(value, _)| {
            if is_gif {
                styles::GIF_RESOLUTIONS.contains(value)
            } else {
                styles::MP4_RESOLUTIONS.contains(value)
            }
        })
        .collect();
    let frame_rates: Vec<(&'static str, &'static str)> = styles::EXPORT_FRAME_RATES
        .iter()
        .copied()
        .filter(|(value, _)| {
            if is_gif {
                styles::GIF_FRAME_RATES.contains(value)
            } else {
                styles::MP4_FRAME_RATES.contains(value)
            }
        })
        .collect();

    let settings_children = vec![
        kit::header(
            "Export Settings",
            "Configure video export options",
            None,
            theme,
            cx,
            |_, _, _| {},
        ),
        kit::select_row(
            "export-format",
            "Format",
            &settings.format,
            &styles::EXPORT_FORMATS,
            menu,
            theme,
            cx,
            |this, value, cx| {
                this.update_export(cx, move |settings| settings.format = value.clone())
            },
        ),
        kit::select_row(
            "export-resolution",
            "Resolution",
            &settings.resolution,
            &resolutions,
            menu,
            theme,
            cx,
            |this, value, cx| {
                this.update_export(cx, move |settings| settings.resolution = value.clone())
            },
        ),
        kit::select_row(
            "export-quality",
            "Compression",
            &settings.quality_preset,
            &styles::EXPORT_QUALITY_PRESETS,
            menu,
            theme,
            cx,
            |this, value, cx| {
                this.update_export(cx, move |settings| settings.quality_preset = value.clone())
            },
        ),
        kit::select_row(
            "export-frame-rate",
            "Frame Rate",
            &settings.frame_rate,
            &frame_rates,
            menu,
            theme,
            cx,
            |this, value, cx| {
                this.update_export(cx, move |settings| settings.frame_rate = value.clone())
            },
        ),
    ];

    let (upload_to_cloud, cloud_upload, uploaded_url) = {
        let editor = view;
        (
            editor.upload_to_cloud,
            editor.cloud_upload,
            editor.uploaded_url.clone(),
        )
    };
    let cloud_configured = crate::cloud::is_configured(&crate::state::state(cx).config.get().cloud);
    let mut footer = vec![kit::switch_row(
        "export-open-in-finder",
        "Reveal in Finder after export",
        settings.open_in_finder,
        theme,
        cx,
        |this, value, cx| this.update_export(cx, move |settings| settings.open_in_finder = value),
    )];
    footer.push(kit::switch_row(
        "export-upload-cloud",
        "Upload to cloud after export",
        cloud_configured && upload_to_cloud,
        theme,
        cx,
        |this, value, cx| this.set_upload_to_cloud(value, cx),
    ));
    if !cloud_configured {
        footer.push(kit::hint(
            "Configure cloud storage in Settings to enable.",
            theme,
        ));
    }
    match cloud_upload {
        crate::cloud::UploadState::Uploading => {
            footer.push(kit::hint("Uploading to cloud...", theme));
            footer.push(kit::tertiary_button(
                "export-cloud-cancel",
                "Cancel",
                "x",
                false,
                theme,
                cx,
                |this, cx| this.cancel_cloud_upload(cx),
            ));
        }
        crate::cloud::UploadState::Error => {
            footer.push(kit::hint("Cloud upload failed. Please try again.", theme));
        }
        crate::cloud::UploadState::Success => {
            if let Some(url) = uploaded_url {
                footer.push(kit::hint("Uploaded to cloud", theme));
                footer.push(kit::hint(url, theme));
                footer.push(
                    div()
                        .flex()
                        .flex_row()
                        .gap(px(8.0))
                        .child(kit::tertiary_button(
                            "export-cloud-copy",
                            "Copy",
                            "copy",
                            false,
                            theme,
                            cx,
                            |this, cx| this.copy_uploaded_url(cx),
                        ))
                        .child(kit::tertiary_button(
                            "export-cloud-open",
                            "Open",
                            "external-link",
                            false,
                            theme,
                            cx,
                            |this, cx| this.open_uploaded_url(cx),
                        ))
                        .into_any_element(),
                );
            }
        }
        crate::cloud::UploadState::Idle => {}
    }

    if is_exporting {
        footer.push(
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
                        .child(kit::hint("Exporting...", theme))
                        .child(kit::hint(
                            format!("{}%", (export_progress * 100.0).round() as i32),
                            theme,
                        )),
                )
                .child(crate::ui::primitives::Progress::new(export_progress).height(px(6.0)))
                .into_any_element(),
        );
        footer.push(kit::tertiary_button(
            "export-cancel",
            "Cancel",
            "x",
            false,
            theme,
            cx,
            |this, cx| this.cancel_export(cx),
        ));
    } else {
        footer.push(kit::tertiary_button(
            "export-start",
            if is_gif { "Export GIF" } else { "Export Video" },
            "download",
            false,
            theme,
            cx,
            |this, cx| this.start_export(cx),
        ));
    }

    div()
        .id("video-export-panel")
        .flex()
        .flex_col()
        .size_full()
        .child(
            div()
                .id("video-export-settings")
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .gap(px(crate::ui::chrome::VIDEO_PANEL_GAP))
                .overflow_y_scroll()
                .p(px(crate::ui::chrome::VIDEO_PANEL_PAD))
                .children(settings_children),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(12.0))
                .border_t_1()
                .border_color(theme.border)
                .p(px(crate::ui::chrome::VIDEO_PANEL_PAD))
                .children(footer),
        )
        .into_any_element()
}
