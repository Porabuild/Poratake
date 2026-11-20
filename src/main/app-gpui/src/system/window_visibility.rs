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
    use windows::Win32::Graphics::Dwm::DwmFlush;
    use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE, SW_SHOW};

    if let Some(hwnd) = crate::windows::window_hwnd(window) {
        crate::system::window_composition::disable_transitions(hwnd);
        unsafe {
            let _ = ShowWindow(hwnd, if visible { SW_SHOW } else { SW_HIDE });
            if !visible {
                let _ = DwmFlush();
            }
        }
    }
}

#[cfg(all(target_os = "macos", not(test)))]
fn appkit_window(window: &Window) -> Option<cocoa::base::id> {
    use cocoa::base::{id, nil};
    use objc::{msg_send, sel, sel_impl};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let handle = HasWindowHandle::window_handle(window).ok()?;
    let RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
        return None;
    };
    let ns_window: id = unsafe { msg_send![appkit.ns_view.as_ptr() as id, window] };
    (ns_window != nil).then_some(ns_window)
}

#[cfg(all(target_os = "macos", not(test)))]
pub fn disable_macos_transitions(window: &Window) {
    use objc::{msg_send, sel, sel_impl};

    const NS_WINDOW_ANIMATION_BEHAVIOR_NONE: isize = 2;
    if let Some(ns_window) = appkit_window(window) {
        let _: () = unsafe {
            msg_send![ns_window, setAnimationBehavior: NS_WINDOW_ANIMATION_BEHAVIOR_NONE]
        };
    }
}

#[cfg(all(target_os = "macos", not(test)))]
fn platform_set_visible(window: &Window, visible: bool) {
    use cocoa::base::nil;
    use objc::{msg_send, sel, sel_impl};

    let Some(ns_window) = appkit_window(window) else {
        return;
    };
    unsafe {
        if visible {
            let _: () = msg_send![ns_window, makeKeyAndOrderFront: nil];
        } else {
            let _: () = msg_send![ns_window, orderOut: nil];
        }
    }
}

#[cfg(any(test, all(not(windows), not(target_os = "macos"))))]
fn platform_set_visible(_window: &Window, _visible: bool) {}
