use gpui::Window;
use herogpui::gpui;

pub fn enable(window: &Window) {
    #[cfg(all(target_os = "macos", not(test)))]
    {
        use objc::{msg_send, sel, sel_impl};
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};

        let Some(handle) = HasWindowHandle::window_handle(window).ok() else {
            return;
        };
        let RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
            return;
        };
        unsafe {
            let view = appkit.ns_view.as_ptr() as cocoa::base::id;
            let ns_window: cocoa::base::id = msg_send![view, window];
            if ns_window == cocoa::base::nil {
                return;
            }
            let _: () = msg_send![ns_window, setIgnoresMouseEvents: true];
        }
    }
    #[cfg(all(windows, not(test)))]
    {
        use windows::Win32::UI::WindowsAndMessaging::{
            GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE, WS_EX_LAYERED, WS_EX_TRANSPARENT,
        };

        let Some(hwnd) = crate::windows::window_hwnd(window) else {
            return;
        };
        unsafe {
            let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            let flags = (WS_EX_LAYERED.0 | WS_EX_TRANSPARENT.0) as isize;
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style | flags);
        }
    }
    #[cfg(any(test, all(not(windows), not(target_os = "macos"))))]
    let _ = window;
}
