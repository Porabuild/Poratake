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
    point, px, Bounds, DisplayId, Pixels, Point, Size, TitlebarOptions, WindowBackgroundAppearance,
    WindowBounds, WindowKind, WindowOptions,
};
use herogpui::gpui;

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

pub struct PopupWindowConfig {
    pub focus: bool,
    pub show: bool,
    pub movable: bool,
    pub resizable: bool,
    pub display_id: Option<DisplayId>,
    pub background: WindowBackgroundAppearance,
}

impl Default for PopupWindowConfig {
    fn default() -> Self {
        Self {
            focus: true,
            show: true,
            movable: false,
            resizable: false,
            display_id: None,
            background: WindowBackgroundAppearance::Transparent,
        }
    }
}

pub fn popup_window_options(bounds: Bounds<Pixels>, config: PopupWindowConfig) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: None,
        focus: config.focus,
        show: config.show,
        kind: WindowKind::PopUp,
        is_movable: config.movable,
        is_resizable: config.resizable,
        is_minimizable: false,
        display_id: config.display_id,
        window_background: config.background,
        ..Default::default()
    }
}
