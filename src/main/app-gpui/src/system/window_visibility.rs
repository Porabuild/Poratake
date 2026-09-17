//! Hides and re-shows an app window around a capture, mirroring the
//! editor hide/show in `screenshot:capture-for-editor`. GPUI exposes no
//! per-window visibility call, so this goes to the platform handle directly.

use gpui::Window;
use herogpui::gpui;

pub fn hide(window: &Window) {
    platform_set_visible(window, false);
}

pub fn show(window: &Window) {
    platform_set_visible(window, true);
}

#[cfg(all(windows, not(test)))]
fn platform_set_visible(window: &Window, visible: bool) {
    use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE, SW_SHOW};

    if let Some(hwnd) = crate::windows::window_hwnd(window) {
        unsafe {
            let _ = ShowWindow(hwnd, if visible { SW_SHOW } else { SW_HIDE });
        }
    }
}

#[cfg(all(target_os = "macos", not(test)))]
fn platform_set_visible(window: &Window, visible: bool) {
    use cocoa::base::nil;
    use objc::{msg_send, sel, sel_impl};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let handle = HasWindowHandle::window_handle(window).ok();
    let Some(handle) = handle else {
        return;
    };
    let RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
        return;
    };
    unsafe {
        let view = appkit.ns_view.as_ptr() as cocoa::base::id;
        let ns_window: cocoa::base::id = msg_send![view, window];
        if ns_window == nil {
            return;
        }
        if visible {
            let _: () = msg_send![ns_window, makeKeyAndOrderFront: nil];
        } else {
            let _: () = msg_send![ns_window, orderOut: nil];
        }
    }
}

#[cfg(any(test, all(not(windows), not(target_os = "macos"))))]
fn platform_set_visible(_window: &Window, _visible: bool) {}
