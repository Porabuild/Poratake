use gpui::{
    div, prelude::*, px, size, App, Bounds, Context, Render, Window, WindowBounds, WindowKind,
    WindowOptions,
};

/// Poratake is a tray-first app with no main window, but the Windows backend
/// quits the process when its last window closes. This hidden 1x1 window keeps
/// the app alive between visible surfaces.
pub struct KeepAlive;

impl KeepAlive {
    pub fn open(cx: &mut App) {
        let bounds = Bounds {
            origin: gpui::point(px(-32000.0), px(-32000.0)),
            size: size(px(1.0), px(1.0)),
        };
        let opened = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: None,
                focus: false,
                show: false,
                kind: WindowKind::PopUp,
                is_movable: false,
                is_resizable: false,
                is_minimizable: false,
                ..Default::default()
            },
            |_, cx| cx.new(|_| Self),
        );
        if let Err(error) = opened {
            eprintln!("[keepalive] failed to open anchor window: {error}");
        }
    }
}

impl Render for KeepAlive {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}
