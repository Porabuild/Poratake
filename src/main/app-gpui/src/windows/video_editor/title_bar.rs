use gpui::{div, prelude::*, px, AnyElement, Context, SharedString, Styled, Window};

use crate::system::accelerator;
use crate::theme::vars::ThemeVars;
use crate::ui::button::{Button, ButtonSize, ButtonVariant};
use crate::ui::chrome;
use crate::ui::primitives::Progress;
use crate::windows::video_editor::VideoEditorWindow;

pub const TITLE_BAR_HEIGHT: f32 = chrome::TITLE_BAR_HEIGHT;

pub struct TitleBarState {
    pub file_name: SharedString,
    pub project_path: Option<SharedString>,
    pub can_undo: bool,
    pub can_redo: bool,
    pub is_sidebar_open: bool,
    pub is_exporting: bool,
    pub export_progress: f32,
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

    if let Some(path) = &state.project_path {
        actions = actions.child(
            Button::new("video-project-path")
                .variant(ButtonVariant::Ghost)
                .size(ButtonSize::IconXs)
                .icon("folder-open")
                .tooltip(path.clone())
                .foreground(theme.muted_foreground)
                .on_click(cx.listener(|this, _event, _window, cx| this.reveal_project(cx))),
        );
    }

    if state.is_exporting {
        actions = actions.child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .rounded_full()
                .bg(theme.default)
                .px(px(8.0))
                .py(px(4.0))
                .child(
                    div()
                        .w(px(64.0))
                        .child(Progress::new(state.export_progress)),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(theme.muted_foreground)
                        .child(format!("{}%", (state.export_progress * 100.0) as i32)),
                )
                .child(
                    Button::new("video-cancel-export")
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::IconXs)
                        .icon("x")
                        .tooltip("Cancel export")
                        .on_click(cx.listener(|this, _event, _window, cx| this.cancel_export(cx))),
                ),
        );
    }

    actions = actions
        .child(
            Button::new("video-undo")
                .variant(ButtonVariant::Ghost)
                .size(ButtonSize::IconXs)
                .icon("rotate-ccw")
                .disabled(!state.can_undo)
                .tooltip(format!(
                    "Undo ({})",
                    accelerator::display("CommandOrControl+Z")
                ))
                .on_click(cx.listener(|this, _event, _window, cx| this.undo(cx))),
        )
        .child(
            Button::new("video-redo")
                .variant(ButtonVariant::Ghost)
                .size(ButtonSize::IconXs)
                .icon("rotate-cw")
                .disabled(!state.can_redo)
                .tooltip(format!(
                    "Redo ({})",
                    accelerator::display("CommandOrControl+Shift+Z")
                ))
                .on_click(cx.listener(|this, _event, _window, cx| this.redo(cx))),
        )
        .child(
            Button::new("video-reset")
                .variant(ButtonVariant::Ghost)
                .size(ButtonSize::IconXs)
                .icon("refresh-ccw")
                .tooltip("Reset to Defaults")
                .on_click(cx.listener(|this, _event, _window, cx| this.confirm_reset(cx))),
        )
        .child(
            Button::new("video-delete")
                .variant(ButtonVariant::Ghost)
                .size(ButtonSize::IconXs)
                .icon("trash-2")
                .tooltip(format!(
                    "Delete Video ({})",
                    accelerator::display("CommandOrControl+Backspace")
                ))
                .on_click(
                    cx.listener(|this, _event, window, cx| this.delete_recording(window, cx)),
                ),
        )
        .child(
            Button::new("video-toggle-sidebar")
                .variant(ButtonVariant::Ghost)
                .size(ButtonSize::IconXs)
                .icon(if state.is_sidebar_open {
                    "panel-right-close"
                } else {
                    "panel-right-open"
                })
                .tooltip(if state.is_sidebar_open {
                    "Hide Sidebar"
                } else {
                    "Show Sidebar"
                })
                .on_click(cx.listener(|this, _event, _window, cx| this.toggle_sidebar(cx))),
        );

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
            chrome::VIDEO_TRAFFIC_LIGHT_PAD + chrome::TITLE_BAR_PADDING_X
        ));
    }

    let mut bar = div()
        .flex()
        .flex_row()
        .items_center()
        .h(px(TITLE_BAR_HEIGHT))
        .w_full()
        .flex_none()
        .bg(theme.card)
        .child(
            name.child(
                div()
                    .truncate()
                    .text_size(px(chrome::VIDEO_FILENAME_SIZE))
                    .child(state.file_name.clone()),
            ),
        )
        .child(actions);
    if !chrome::is_macos() {
        bar = bar.child(crate::ui::window_controls::render(window, cx, theme));
    }
    bar.into_any_element()
}
