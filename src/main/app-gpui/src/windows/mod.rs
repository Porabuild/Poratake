pub mod capture_preview;
pub mod history;
pub mod keepalive;
pub mod onboarding;
pub mod pin;
pub mod recording_control;
pub mod registry;
pub mod settings;
mod smoke;
pub mod toast;
pub mod video_editor;

use gpui::{
    point, px, Bounds, Pixels, Point, Size, TitlebarOptions, WindowBackgroundAppearance,
    WindowBounds, WindowOptions,
};

pub fn app_window_options(bounds: Bounds<Pixels>, min_size: Option<Size<Pixels>>) -> WindowOptions {
    app_window_options_with_lights(bounds, min_size, point(px(12.0), px(11.0)))
}

pub fn app_window_options_with_lights(
    bounds: Bounds<Pixels>,
    min_size: Option<Size<Pixels>>,
    traffic_lights: Point<Pixels>,
) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitlebarOptions {
            title: Some("Poratake".into()),
            appears_transparent: true,
            traffic_light_position: Some(traffic_lights),
        }),
        window_min_size: min_size,
        window_background: WindowBackgroundAppearance::Opaque,
        ..Default::default()
    }
}
