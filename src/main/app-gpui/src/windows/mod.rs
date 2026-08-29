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
pub mod tray_menu;
pub mod video_editor;

use gpui::{
    point, px, Bounds, Pixels, Point, Size, TitlebarOptions, WindowBackgroundAppearance,
    WindowBounds, WindowOptions,
};

#[cfg(all(windows, not(test)))]
pub(crate) fn window_hwnd(window: &gpui::Window) -> Option<windows::Win32::Foundation::HWND> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let handle = HasWindowHandle::window_handle(window).ok()?;
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return None;
    };
    Some(windows::Win32::Foundation::HWND(
        handle.hwnd.get() as *mut core::ffi::c_void
    ))
}

#[cfg(all(windows, test))]
pub(crate) fn window_hwnd(_window: &gpui::Window) -> Option<windows::Win32::Foundation::HWND> {
    None
}

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
