use gpui::{div, prelude::*, px, AnyElement, Context, MouseDownEvent, Styled, Window};
use herogpui::gpui;

use crate::theme::vars::ThemeVars;
use crate::windows::video_editor::VideoEditorWindow;

pub const HANDLE_HEIGHT: f32 = 6.0;

pub fn resize_handle(
    resizing: bool,
    theme: &ThemeVars,
    window: &mut Window,
    cx: &mut Context<VideoEditorWindow>,
) -> AnyElement {
    let (hover, hovered) = crate::ui::primitives::hover_flag("video-timeline-resize", window, cx);
    div()
        .id("video-timeline-resize")
        .h(px(HANDLE_HEIGHT))
        .w_full()
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .cursor_ns_resize()
        .bg(if resizing {
            theme.primary.opacity(0.4)
        } else if hovered {
            theme.primary.opacity(0.2)
        } else {
            gpui::transparent_black()
        })
        .child(div().h(px(2.0)).w(px(32.0)).rounded_full().bg(if resizing {
            theme.primary
        } else if hovered {
            theme.muted_foreground.opacity(0.5)
        } else {
            theme.muted_foreground.opacity(0.3)
        }))
        .on_hover(move |over: &bool, _window, cx| {
            crate::ui::primitives::track_hover(&hover, *over, cx);
        })
        .on_mouse_down(
            gpui::MouseButton::Left,
            cx.listener(|this, event: &MouseDownEvent, _window, cx| {
                cx.stop_propagation();
                this.begin_timeline_resize(f32::from(event.position.y), cx);
            }),
        )
        .into_any_element()
}
