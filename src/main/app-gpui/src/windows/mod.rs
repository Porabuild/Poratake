#[cfg(test)]
mod a11y_tests;
pub mod capture_preview;
pub mod history;
pub mod keepalive;
pub mod onboarding;
pub mod pin;
pub mod recording_control;
pub mod registry;
#[cfg(any(target_os = "macos", test, all(windows, feature = "parity")))]
pub mod scroll_capture;
pub mod scrollbars;
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

#[cfg(target_os = "linux")]
pub fn open_wayland_layer_or_fallback<V: 'static + gpui::Render>(
    cx: &mut gpui::App,
    layer_options: gpui::layer_shell::LayerShellOptions,
    fallback: impl Fn() -> WindowOptions,
    build: impl FnOnce(&mut gpui::Window, &mut gpui::App) -> gpui::Entity<V>,
) -> anyhow::Result<(gpui::WindowHandle<V>, bool)> {
    if crate::system::linux_session::current()
        != crate::system::linux_session::LinuxSession::Wayland
    {
        return cx
            .open_window(fallback(), build)
            .map(|handle| (handle, false));
    }
    let mut build = Some(build);
    let mut primary = fallback();
    primary.kind = WindowKind::LayerShell(layer_options);
    if let Some(bounds) = primary.window_bounds {
        primary.window_bounds = Some(WindowBounds::Windowed(bounds.get_bounds()));
    }
    match cx.open_window(primary, |window, cx| {
        (build.take().expect("window builder"))(window, cx)
    }) {
        Ok(handle) => Ok((handle, true)),
        Err(error) => {
            if let Some(build) = build.take() {
                cx.open_window(fallback(), build)
                    .map(|handle| (handle, false))
            } else {
                Err(error)
            }
        }
    }
}

#[cfg(all(target_os = "macos", not(test)))]
pub(crate) fn remove_popup_frame(window: &gpui::Window) {
    use cocoa::appkit::NSWindowStyleMask;
    use cocoa::base::nil;
    use objc::{msg_send, sel, sel_impl};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let Ok(handle) = HasWindowHandle::window_handle(window) else {
        return;
    };
    let RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
        return;
    };
    unsafe {
        let view = appkit.ns_view.as_ptr() as cocoa::base::id;
        let native_window: cocoa::base::id = msg_send![view, window];
        if native_window == nil {
            return;
        }
        let style: NSWindowStyleMask = msg_send![native_window, styleMask];
        let style = style
            & !(NSWindowStyleMask::NSTitledWindowMask
                | NSWindowStyleMask::NSFullSizeContentViewWindowMask);
        let _: () = msg_send![native_window, setStyleMask: style];
        let _: () = msg_send![native_window, setHasShadow: false];
    }
}

#[cfg(any(test, not(target_os = "macos")))]
pub(crate) fn remove_popup_frame(_window: &gpui::Window) {}

#[cfg(all(target_os = "macos", not(test)))]
fn macos_window_and_primary_height(window: &gpui::Window) -> Option<(cocoa::base::id, f64)> {
    use cocoa::appkit::NSScreen;
    use cocoa::base::nil;
    use cocoa::foundation::{NSArray, NSRect};
    use objc::{msg_send, sel, sel_impl};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let handle = HasWindowHandle::window_handle(window).ok()?;
    let RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
        return None;
    };
    unsafe {
        let view = appkit.ns_view.as_ptr() as cocoa::base::id;
        let native_window: cocoa::base::id = msg_send![view, window];
        let screens = NSScreen::screens(nil);
        if native_window == nil || NSArray::count(screens) == 0 {
            return None;
        }
        let primary = screens.objectAtIndex(0);
        let primary_frame: NSRect = msg_send![primary, frame];
        Some((native_window, primary_frame.size.height))
    }
}

pub(crate) fn desktop_window_bounds(window: &gpui::Window) -> Bounds<Pixels> {
    #[cfg(all(target_os = "macos", not(test)))]
    if let Some((native_window, primary_height)) = macos_window_and_primary_height(window) {
        use cocoa::foundation::NSRect;
        use objc::{msg_send, sel, sel_impl};

        let frame: NSRect = unsafe { msg_send![native_window, frame] };
        return Bounds::new(
            point(
                px(frame.origin.x as f32),
                px((primary_height - frame.origin.y - frame.size.height) as f32),
            ),
            gpui::size(px(frame.size.width as f32), px(frame.size.height as f32)),
        );
    }
    window.bounds()
}

#[cfg(all(target_os = "macos", not(test)))]
pub(crate) fn set_macos_window_bounds(window: &gpui::Window, bounds: Bounds<Pixels>) -> bool {
    use cocoa::foundation::{NSPoint, NSRect, NSSize};
    use objc::{msg_send, sel, sel_impl};

    let Some((native_window, primary_height)) = macos_window_and_primary_height(window) else {
        return false;
    };
    unsafe {
        let frame = NSRect::new(
            NSPoint::new(
                f64::from(f32::from(bounds.origin.x)),
                primary_height - f64::from(f32::from(bounds.origin.y + bounds.size.height)),
            ),
            NSSize::new(
                f64::from(f32::from(bounds.size.width)),
                f64::from(f32::from(bounds.size.height)),
            ),
        );
        let _: () = msg_send![native_window, setFrame: frame display: true];
        true
    }
}

#[cfg(all(target_os = "macos", not(test)))]
fn pin_macos_window_contents(window: &gpui::Window, bottom_aligned: bool) {
    use cocoa::base::{id, nil, BOOL, YES};
    use objc::{msg_send, sel, sel_impl};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    #[link(name = "QuartzCore", kind = "framework")]
    unsafe extern "C" {
        static kCAGravityTop: id;
        static kCAGravityBottom: id;
    }

    let Ok(handle) = HasWindowHandle::window_handle(window) else {
        return;
    };
    let RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
        return;
    };
    unsafe {
        let layer: id = msg_send![appkit.ns_view.as_ptr() as id, layer];
        if layer != nil {
            let flipped: BOOL = msg_send![layer, contentsAreFlipped];
            let gravity = if bottom_aligned != (flipped == YES) {
                kCAGravityBottom
            } else {
                kCAGravityTop
            };
            let _: () = msg_send![layer, setContentsGravity: gravity];
        }
    }
}

#[cfg(all(target_os = "macos", not(test)))]
pub(crate) fn macos_window_id(window: &gpui::Window) -> Option<i64> {
    use cocoa::base::nil;
    use objc::{msg_send, sel, sel_impl};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let handle = HasWindowHandle::window_handle(window).ok()?;
    let RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
        return None;
    };
    unsafe {
        let view = appkit.ns_view.as_ptr() as cocoa::base::id;
        let native_window: cocoa::base::id = msg_send![view, window];
        if native_window == nil {
            return None;
        }
        let number: isize = msg_send![native_window, windowNumber];
        (number >= 0).then_some(number as i64)
    }
}

#[cfg(any(target_os = "linux", test))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WindowCorner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

#[cfg(any(target_os = "linux", test))]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct EdgeGaps {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

#[cfg(any(target_os = "linux", test))]
pub(crate) fn window_corner_gaps(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    work_x: f32,
    work_y: f32,
    work_width: f32,
    work_height: f32,
) -> (WindowCorner, EdgeGaps) {
    let center_x = x + width / 2.0;
    let center_y = y + height / 2.0;
    let to_right = center_x >= work_x + work_width / 2.0;
    let to_bottom = center_y >= work_y + work_height / 2.0;
    let corner = match (to_bottom, to_right) {
        (false, false) => WindowCorner::TopLeft,
        (false, true) => WindowCorner::TopRight,
        (true, false) => WindowCorner::BottomLeft,
        (true, true) => WindowCorner::BottomRight,
    };
    (
        corner,
        EdgeGaps {
            top: (y - work_y).max(0.0),
            left: (x - work_x).max(0.0),
            right: (work_x + work_width - (x + width)).max(0.0),
            bottom: (work_y + work_height - (y + height)).max(0.0),
        },
    )
}

#[cfg(target_os = "linux")]
pub(crate) fn layer_anchor(corner: WindowCorner) -> gpui::layer_shell::Anchor {
    use gpui::layer_shell::Anchor;
    match corner {
        WindowCorner::TopLeft => Anchor::TOP | Anchor::LEFT,
        WindowCorner::TopRight => Anchor::TOP | Anchor::RIGHT,
        WindowCorner::BottomLeft => Anchor::BOTTOM | Anchor::LEFT,
        WindowCorner::BottomRight => Anchor::BOTTOM | Anchor::RIGHT,
    }
}

#[cfg(target_os = "linux")]
pub(crate) fn layer_margin(gaps: EdgeGaps) -> (Pixels, Pixels, Pixels, Pixels) {
    (px(gaps.top), px(gaps.right), px(gaps.bottom), px(gaps.left))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corner_gaps_follow_the_window_instead_of_a_fixed_edge() {
        let (corner, gaps) =
            window_corner_gaps(40.0, 700.0, 200.0, 120.0, 0.0, 0.0, 1920.0, 1080.0);
        assert_eq!(corner, WindowCorner::BottomLeft);
        assert_eq!(gaps.left, 40.0);
        assert_eq!(gaps.bottom, 1080.0 - 820.0);
        let (corner, gaps) =
            window_corner_gaps(1600.0, 8.0, 300.0, 400.0, 0.0, 0.0, 1920.0, 1080.0);
        assert_eq!(corner, WindowCorner::TopRight);
        assert_eq!(gaps.top, 8.0);
        assert_eq!(gaps.right, 20.0);
    }
}
