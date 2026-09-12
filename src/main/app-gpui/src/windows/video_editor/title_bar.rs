use gpui::{div, prelude::*, px, AnyElement, Context, SharedString, Styled, Window};
use herogpui::gpui;

use crate::system::accelerator;
use crate::theme::vars::ThemeVars;
use crate::ui::chrome;
use crate::ui::icon_button;
use crate::windows::video_editor::VideoEditorWindow;
use herogpui::components::{Button, Size, Variant};

pub const TITLE_BAR_HEIGHT: f32 = chrome::TITLE_BAR_HEIGHT;

pub struct TitleBarState {
    pub file_name: SharedString,
    pub project_path: Option<SharedString>,
    pub can_undo: bool,
    pub can_redo: bool,
    pub is_sidebar_open: bool,
    pub is_exporting: bool,
    pub export_progress: f32,
    pub renaming: bool,
    pub rename_field: gpui::Entity<herogpui::components::InputState>,
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
        actions = actions.child(icon_button::with_tooltip(
            path.clone(),
            icon_button::compact_muted("video-project-path", "folder-open")
                .on_press(cx.listener(|this, _event, _window, cx| this.reveal_project(cx))),
        ));
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
                    div().w(px(64.0)).child(
                        herogpui::ProgressBar::new("video-export-progress")
                            .value(state.export_progress * 100.0)
                            .sx(|el| el.h(px(8.0))),
                    ),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(theme.muted_foreground)
                        .child(format!("{}%", (state.export_progress * 100.0) as i32)),
                )
                .child(icon_button::with_tooltip(
                    "Cancel export",
                    icon_button::compact("video-cancel-export", "x")
                        .on_press(cx.listener(|this, _event, _window, cx| this.cancel_export(cx))),
                )),
        );
    }

    actions = actions
        .child(icon_button::with_tooltip(
            format!("Undo ({})", accelerator::display("CommandOrControl+Z")),
            icon_button::compact("video-undo", "rotate-ccw")
                .is_disabled(!state.can_undo)
                .on_press(cx.listener(|this, _event, _window, cx| this.undo(cx))),
        ))
        .child(icon_button::with_tooltip(
            format!(
                "Redo ({})",
                accelerator::display("CommandOrControl+Shift+Z")
            ),
            icon_button::compact("video-redo", "rotate-cw")
                .is_disabled(!state.can_redo)
                .on_press(cx.listener(|this, _event, _window, cx| this.redo(cx))),
        ))
        .child(icon_button::with_tooltip(
            "Reset to Defaults",
            icon_button::compact("video-reset", "refresh-ccw")
                .on_press(cx.listener(|this, _event, _window, cx| this.confirm_reset(cx))),
        ))
        .child(icon_button::with_tooltip(
            format!(
                "Delete Video ({})",
                accelerator::display("CommandOrControl+Backspace")
            ),
            icon_button::compact("video-delete", "trash-2").on_press(
                cx.listener(|this, _event, window, cx| this.delete_recording(window, cx)),
            ),
        ))
        .child(icon_button::with_tooltip(
            if state.is_sidebar_open {
                "Hide Sidebar"
            } else {
                "Show Sidebar"
            },
            icon_button::compact(
                "video-toggle-sidebar",
                if state.is_sidebar_open {
                    "panel-right-close"
                } else {
                    "panel-right-open"
                },
            )
            .on_press(cx.listener(|this, _event, _window, cx| this.toggle_sidebar(cx))),
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

    let name_content = if state.renaming {
        let rename_owner = cx.entity().downgrade();
        let cancel_owner = rename_owner.clone();
        div()
            .w(px(220.0))
            .on_key_down(move |event, _window, cx| {
                if event.keystroke.key.as_str() != "escape" {
                    return;
                }
                let _ = cancel_owner.update(cx, |this, cx| {
                    this.renaming = false;
                    cx.notify();
                });
                cx.stop_propagation();
            })
            .child(
                herogpui::components::TextField::new(state.rename_field.clone())
                    .recipe("compact")
                    .is_bare(true)
                    .on_submit(move |value, window, cx| {
                        let value = value.to_string();
                        let _ = rename_owner.update(cx, |this, cx| {
                            this.rename_project(&value, window, cx);
                        });
                    }),
            )
            .into_any_element()
    } else {
        icon_button::with_tooltip(
            "Rename project",
            Button::new("video-rename-project")
                .child(
                    div()
                        .text_size(px(chrome::VIDEO_FILENAME_SIZE))
                        .child(state.file_name.clone())
                        .into_any_element(),
                )
                .variant(Variant::Ghost)
                .size(Size::Sm)
                .on_press(cx.listener(|this, _event, window, cx| {
                    this.begin_rename(window, cx);
                })),
        )
    };

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
