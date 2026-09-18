use gpui::{div, prelude::*, px, AnyElement, Context, SharedString, Styled, Window};
use herogpui::gpui;

use crate::system::accelerator;
use crate::theme::vars::ThemeVars;
use crate::ui::chrome;
use crate::ui::icon::icon_element;
use crate::ui::icon_button;
use crate::ui::toolbar;
use crate::windows::video_editor::VideoEditorWindow;
use herogpui::components::{Button, Size, Tooltip, TooltipPlacement, Variant};

pub const TITLE_BAR_HEIGHT: f32 = chrome::TITLE_BAR_HEIGHT;
const RING_SIZE: f32 = 16.0;
const RING_STROKE: f32 = 2.0;
const EXPORT_POPOVER_WIDTH: f32 = 256.0;
const PROJECT_POPOVER_WIDTH: f32 = 320.0;
pub const EXPORT_POPOVER_ID: &str = "video-export-indicator";
pub const PROJECT_POPOVER_ID: &str = "video-project-path";

pub struct TitleBarState {
    pub file_name: SharedString,
    pub project_path: Option<SharedString>,
    pub can_undo: bool,
    pub can_redo: bool,
    pub is_sidebar_open: bool,
    pub is_exporting: bool,
    pub export_progress: f32,
    pub export_completed: bool,
    pub menu: crate::ui::menu::MenuHandle,
}

fn tooltip_button_below<V: 'static>(
    button: Button,
    tooltip: impl Into<SharedString>,
    cx: &mut Context<V>,
    on_click: impl Fn(&mut V, &mut Window, &mut Context<V>) + 'static,
) -> AnyElement {
    Tooltip::new(tooltip)
        .placement(TooltipPlacement::Bottom)
        .child(
            button.on_press(cx.listener(move |this, _event, window, cx| {
                on_click(this, window, cx);
            })),
        )
        .into_any_element()
}

fn progress_ring(progress: f32, theme: &ThemeVars) -> AnyElement {
    let track = theme.muted_foreground.opacity(0.3);
    let indicator = theme.foreground;
    let fraction = progress.clamp(0.0, 1.0);
    gpui::canvas(
        |_bounds, _window, _cx| (),
        move |bounds, (), window, _cx| {
            let radius = (RING_SIZE - RING_STROKE) / 2.0;
            let center = gpui::point(
                bounds.origin.x + px(RING_SIZE / 2.0),
                bounds.origin.y + px(RING_SIZE / 2.0),
            );
            paint_arc(window, center, radius, 0.0, 1.0, track);
            if fraction > 0.0 {
                paint_arc(window, center, radius, 0.0, fraction, indicator);
            }
        },
    )
    .size(px(RING_SIZE))
    .into_any_element()
}

fn paint_arc(
    window: &mut Window,
    center: gpui::Point<gpui::Pixels>,
    radius: f32,
    from: f32,
    to: f32,
    color: gpui::Hsla,
) {
    const SEGMENTS: usize = 48;
    let steps = ((to - from) * SEGMENTS as f32).ceil().max(1.0) as usize;
    let mut builder = gpui::PathBuilder::stroke(px(RING_STROKE));
    if let gpui::PathStyle::Stroke(options) = &mut builder.style {
        *options = gpui::StrokeOptions::default()
            .with_line_width(options.line_width)
            .with_line_cap(lyon::path::LineCap::Round)
            .with_line_join(lyon::path::LineJoin::Round);
    }
    for step in 0..=steps {
        let fraction = from + (to - from) * (step as f32 / steps as f32);
        let angle = fraction * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
        let point = gpui::point(
            center.x + px(radius * angle.cos()),
            center.y + px(radius * angle.sin()),
        );
        match step {
            0 => builder.move_to(point),
            _ => builder.line_to(point),
        }
    }
    if let Ok(path) = builder.build() {
        window.paint_path(path, color);
    }
}

fn completion_badge(size: f32, theme: &ThemeVars) -> AnyElement {
    div()
        .size(px(size))
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .bg(theme.primary)
        .text_color(theme.primary_foreground)
        .child(icon_element("check", px(size * 0.625)))
        .into_any_element()
}

fn export_indicator(
    state: &TitleBarState,
    theme: &ThemeVars,
    cx: &mut Context<VideoEditorWindow>,
) -> AnyElement {
    let complete = state.export_completed;
    let progress = state.export_progress;
    let content: AnyElement = match complete {
        true => completion_badge(RING_SIZE, theme),
        false => progress_ring(progress, theme),
    };
    div()
        .relative()
        .flex()
        .items_center()
        .child(
            Button::new(EXPORT_POPOVER_ID)
                .variant(Variant::Ghost)
                .size(Size::Sm)
                .is_icon_only(true)
                .child(div().size(px(RING_SIZE)).child(content))
                .on_press(cx.listener(|this, _event, window, cx| {
                    this.toggle_export_popover(window, cx);
                })),
        )
        .child(state.menu.render_dropdown(EXPORT_POPOVER_ID))
        .into_any_element()
}

pub fn export_popover(
    complete: bool,
    progress: f32,
    elapsed: SharedString,
    remaining: SharedString,
    theme: &ThemeVars,
    on_cancel: impl Fn(&mut Window, &mut gpui::App) + 'static,
) -> gpui::Div {
    let card = div()
        .flex()
        .flex_col()
        .gap(px(12.0))
        .w(px(EXPORT_POPOVER_WIDTH))
        .p(px(12.0));
    if complete {
        return card.child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .child(completion_badge(20.0, theme))
                .child(
                    div()
                        .text_size(px(chrome::TEXT_SM))
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .child("Export Complete"),
                ),
        );
    }
    card.child(
        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_size(px(chrome::TEXT_SM))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .child("Exporting..."),
            )
            .child(
                div()
                    .flex_none()
                    .text_size(px(chrome::TEXT_XS))
                    .text_color(theme.muted_foreground)
                    .child(format!("{}%", (progress * 100.0).round() as i32)),
            ),
    )
    .child(herogpui::ProgressBar::new("video-export-popover-progress").value(progress * 100.0))
    .child(
        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .text_size(px(chrome::TEXT_XS))
            .text_color(theme.muted_foreground)
            .child(div().flex_none().child(elapsed))
            .child(div().flex_none().child(remaining)),
    )
    .child(
        crate::ui::rows::icon_text_button("video-export-popover-cancel", "Cancel", "x", 14.0, 6.0)
            .variant(Variant::Tertiary)
            .recipe("compact")
            .full_width(true)
            .on_press(move |_event, window, cx| on_cancel(window, cx)),
    )
}

pub fn project_popover(
    path: SharedString,
    rename_field: gpui::Entity<herogpui::components::InputState>,
    rename_error: Option<SharedString>,
    copied: bool,
    theme: &ThemeVars,
    on_rename: impl Fn(&str, &mut Window, &mut gpui::App) + 'static,
    on_copy: impl Fn(&mut Window, &mut gpui::App) + 'static,
    on_reveal: impl Fn(&mut Window, &mut gpui::App) + 'static,
) -> gpui::Div {
    let submit_field = rename_field.clone();
    let submit = std::rc::Rc::new(on_rename);
    let pressed = submit.clone();
    div()
        .flex()
        .flex_col()
        .gap(px(12.0))
        .w(px(PROJECT_POPOVER_WIDTH))
        .p(px(12.0))
        .child(
            div()
                .text_size(px(chrome::TEXT_SM))
                .font_weight(gpui::FontWeight::MEDIUM)
                .child("Project Name"),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .gap(px(8.0))
                .child(
                    div().flex_1().min_w_0().child(
                        herogpui::components::TextField::new(rename_field)
                            .recipe("compact")
                            .on_submit({
                                let submit = submit.clone();
                                move |value, window, cx| submit(value, window, cx)
                            }),
                    ),
                )
                .child(
                    Button::new("video-project-rename-save")
                        .variant(Variant::Primary)
                        .recipe("compact")
                        .label("Save")
                        .on_press(move |_event, window, cx| {
                            let value = submit_field.read(cx).value().to_string();
                            pressed(&value, window, cx);
                        }),
                ),
        )
        .children(rename_error.map(|error| crate::ui::rows::error(error, theme)))
        .child(
            div()
                .text_size(px(chrome::TEXT_SM))
                .font_weight(gpui::FontWeight::MEDIUM)
                .child("Project Path"),
        )
        .child(
            div()
                .w_full()
                .min_w_0()
                .truncate()
                .rounded(px(chrome::RADIUS_MD))
                .border_1()
                .border_color(theme.field_border)
                .bg(theme.field_background)
                .px(px(8.0))
                .py(px(6.0))
                .text_size(px(chrome::TEXT_XS))
                .text_color(theme.muted_foreground)
                .child(path),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .gap(px(8.0))
                .child(
                    div().flex_1().min_w_0().child(
                        crate::ui::rows::icon_text_button(
                            "video-project-copy-path",
                            "Copy Path",
                            if copied { "check" } else { "copy" },
                            14.0,
                            6.0,
                        )
                        .variant(Variant::Tertiary)
                        .recipe("compact")
                        .full_width(true)
                        .on_press(move |_event, window, cx| on_copy(window, cx)),
                    ),
                )
                .child(
                    div().flex_1().min_w_0().child(
                        crate::ui::rows::icon_text_button(
                            "video-project-show-original",
                            "Show Original",
                            "folder-open",
                            14.0,
                            6.0,
                        )
                        .variant(Variant::Tertiary)
                        .recipe("compact")
                        .full_width(true)
                        .on_press(move |_event, window, cx| on_reveal(window, cx)),
                    ),
                ),
        )
}

pub fn render(
    state: &TitleBarState,
    theme: &ThemeVars,
    window: &mut Window,
    cx: &mut Context<VideoEditorWindow>,
) -> AnyElement {
    let mut actions = div()
        .flex()
        .flex_row()
        .flex_shrink_0()
        .items_center()
        .justify_end()
        .gap(px(chrome::TITLE_BAR_GAP))
        .mr(px(chrome::TITLE_BAR_PADDING_X));

    if state.project_path.is_some() {
        actions = actions.child(
            div()
                .relative()
                .flex()
                .items_center()
                .child(
                    Tooltip::new("Project Info")
                        .placement(TooltipPlacement::Bottom)
                        .child(
                            icon_button::compact_muted(PROJECT_POPOVER_ID, "folder-open").on_press(
                                cx.listener(|this, _event, window, cx| {
                                    this.toggle_project_popover(window, cx);
                                }),
                            ),
                        ),
                )
                .child(state.menu.render_dropdown(PROJECT_POPOVER_ID)),
        );
    }

    if state.is_exporting || state.export_completed {
        actions = actions.child(export_indicator(state, theme, cx));
    }

    actions = actions
        .child(toolbar::tooltip_button(
            icon_button::compact("video-undo", "rotate-ccw").is_disabled(!state.can_undo),
            format!("Undo ({})", accelerator::display("CommandOrControl+Z")),
            cx,
            |this, _window, cx| this.undo(cx),
        ))
        .child(toolbar::tooltip_button(
            icon_button::compact("video-redo", "rotate-cw").is_disabled(!state.can_redo),
            format!(
                "Redo ({})",
                accelerator::display("CommandOrControl+Shift+Z")
            ),
            cx,
            |this, _window, cx| this.redo(cx),
        ))
        .child(tooltip_button_below(
            icon_button::compact("video-reset", "refresh-ccw"),
            "Reset to Defaults",
            cx,
            |this, _window, cx| this.confirm_reset(cx),
        ))
        .child(tooltip_button_below(
            icon_button::compact("video-delete", "trash-2"),
            format!(
                "Delete Video ({})",
                accelerator::display("CommandOrControl+Backspace")
            ),
            cx,
            |this, window, cx| this.delete_recording(window, cx),
        ))
        .child(tooltip_button_below(
            icon_button::compact(
                "video-toggle-sidebar",
                if state.is_sidebar_open {
                    "panel-right-close"
                } else {
                    "panel-right-open"
                },
            ),
            if state.is_sidebar_open {
                "Hide Sidebar"
            } else {
                "Show Sidebar"
            },
            cx,
            |this, _window, cx| this.toggle_sidebar(cx),
        ));

    let mut name = crate::ui::window_controls::drag_area("video-title-drag")
        .flex()
        .flex_row()
        .items_center()
        .flex_1()
        .min_w_0()
        .pl(px(chrome::TITLE_BAR_PADDING_X))
        .text_color(theme.muted_foreground);
    if chrome::is_macos() {
        name = name.pl(px(
            chrome::MACOS_TITLE_LEADING_INSET + chrome::TITLE_BAR_PADDING_X
        ));
    }
    name = name.child(
        div()
            .id("video-title")
            .w(px(0.0))
            .h_full()
            .flex_none()
            .debug_selector(|| "video-title".to_string()),
    );

    let name_content = div()
        .flex_1()
        .min_w_0()
        .truncate()
        .text_size(px(chrome::VIDEO_FILENAME_SIZE))
        .child(state.file_name.clone());

    let mut bar = div()
        .flex()
        .flex_row()
        .items_center()
        .h(px(TITLE_BAR_HEIGHT))
        .w_full()
        .flex_none()
        .bg(theme.card)
        .child(name.child(name_content))
        .child(actions);
    if !chrome::is_macos() {
        bar = bar.child(crate::ui::window_controls::render(window, cx, theme));
    }
    bar.into_any_element()
}
