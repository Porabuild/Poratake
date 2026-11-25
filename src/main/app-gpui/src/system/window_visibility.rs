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

#[cfg(target_os = "macos")]
pub fn show_overlay(window: &Window, focus: bool) {
    if focus {
        window.activate_window();
        return;
    }
    invalidate_overlay_view(window);
    #[cfg(not(test))]
    {
        use objc::{msg_send, sel, sel_impl};

        let Some(ns_window) = appkit_window(window) else {
            return;
        };
        unsafe {
            let _: () = msg_send![ns_window, orderFrontRegardless];
        }
    }
}

#[cfg(target_os = "macos")]
fn invalidate_overlay_view(window: &Window) {
    #[cfg(not(test))]
    {
        use objc::{msg_send, sel, sel_impl};

        if let Some(view) = appkit_view(window) {
            unsafe {
                let _: () = msg_send![view, setNeedsDisplay: true];
            }
        }
    }
    #[cfg(test)]
    let _ = window;
}

#[cfg(all(target_os = "macos", not(test)))]
fn appkit_view(window: &Window) -> Option<cocoa::base::id> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let handle = HasWindowHandle::window_handle(window).ok()?;
    let RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
        return None;
    };
    Some(appkit.ns_view.as_ptr() as cocoa::base::id)
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

    let view = appkit_view(window)?;
    let ns_window: id = unsafe { msg_send![view, window] };
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

#[cfg(all(target_os = "linux", not(test)))]
fn platform_set_visible(window: &Window, visible: bool) {
    use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};

    #[link(name = "xcb")]
    unsafe extern "C" {
        fn xcb_unmap_window(c: *mut core::ffi::c_void, window: u32) -> u32;
        fn xcb_map_window(c: *mut core::ffi::c_void, window: u32) -> u32;
        fn xcb_flush(c: *mut core::ffi::c_void) -> core::ffi::c_int;
    }

    let Ok(win_handle) = HasWindowHandle::window_handle(window) else {
        return;
    };
    let Ok(display_handle) = HasDisplayHandle::display_handle(window) else {
        return;
    };
    let (RawWindowHandle::Xcb(xcb_win), RawDisplayHandle::Xcb(xcb_display)) =
        (win_handle.as_raw(), display_handle.as_raw())
    else {
        return;
    };
    let Some(connection) = xcb_display.connection else {
        return;
    };
    unsafe {
        if visible {
            let _ = xcb_map_window(connection.as_ptr(), xcb_win.window.get());
        } else {
            let _ = xcb_unmap_window(connection.as_ptr(), xcb_win.window.get());
        }
        let _ = xcb_flush(connection.as_ptr());
    }
}

#[cfg(any(test, not(any(windows, target_os = "macos", target_os = "linux"))))]
fn platform_set_visible(_window: &Window, _visible: bool) {}
