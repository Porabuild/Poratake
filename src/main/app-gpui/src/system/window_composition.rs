//! Per-window composition tweaks gpui does not expose.
//!
//! gpui makes a window transparent by applying the Windows accent policy
//! `ACCENT_ENABLE_TRANSPARENTGRADIENT` through `SetWindowCompositionAttribute`
//! (`gpui-0.2.2/src/platform/windows/window.rs`). That gradient also paints a
//! 1px highlight along the *top edge* of the window, which reads as a stray
//! border on any surface meant to sit flush with its own window — the tray menu
//! being the case that prompted this.
//!
//! Clearing the accent does not cost the window its transparency. gpui creates
//! these windows with `WS_EX_NOREDIRECTIONBITMAP` and composites a swapchain
//! that carries an alpha channel, so an unpainted pixel is transparent because
//! nothing is drawn there, not because the gradient lets the desktop through.

#![cfg(windows)]

#[cfg(not(test))]
use std::sync::mpsc::SyncSender;
use std::sync::mpsc::{sync_channel, Receiver};
#[cfg(not(test))]
use std::sync::LazyLock;
use std::time::Duration;

#[cfg(not(test))]
use gpui::Point;
use gpui::{Bounds, Pixels};
use smallvec::SmallVec;
use windows::core::BOOL;
use windows::Win32::Foundation::{HWND, POINT, RECT};

#[cfg(not(test))]
static ANIMATION_FRAME_SENDER: LazyLock<SyncSender<usize>> = LazyLock::new(|| {
    let (sender, receiver) = sync_channel(16);
    let _ = std::thread::Builder::new()
        .name("AnimationFrameDriver".to_owned())
        .spawn(move || {
            while let Ok(first) = receiver.recv() {
                let mut windows = SmallVec::<[usize; 4]>::from_slice(&[first]);
                drain_animation_windows(&receiver, &mut windows);
                let started_at = std::time::Instant::now();
                let wait_succeeded = unsafe { windows::Win32::Graphics::Dwm::DwmFlush() }.is_ok();
                if needs_vsync_fallback(wait_succeeded, started_at.elapsed()) {
                    std::thread::sleep(Duration::from_millis(8));
                }
                drain_animation_windows(&receiver, &mut windows);
                for window in windows {
                    let window = HWND(window as *mut core::ffi::c_void);
                    unsafe {
                        let _ = windows::Win32::Graphics::Gdi::RedrawWindow(
                            Some(window),
                            None,
                            None,
                            windows::Win32::Graphics::Gdi::RDW_INVALIDATE
                                | windows::Win32::Graphics::Gdi::RDW_UPDATENOW,
                        );
                    }
                }
            }
        });
    sender
});

#[cfg(not(test))]
pub fn request_animation_frame(window: HWND) {
    let _ = ANIMATION_FRAME_SENDER.try_send(window.0 as usize);
}

fn needs_vsync_fallback(wait_succeeded: bool, elapsed: Duration) -> bool {
    !wait_succeeded || elapsed < Duration::from_millis(1)
}

fn drain_animation_windows(receiver: &Receiver<usize>, windows: &mut SmallVec<[usize; 4]>) {
    while let Ok(window) = receiver.try_recv() {
        if !windows.contains(&window) {
            windows.push(window);
        }
    }
}

/// `WCA_ACCENT_POLICY`.
const ACCENT_POLICY_ATTRIBUTE: i32 = 19;
/// `ACCENT_DISABLED`.
const ACCENT_DISABLED: i32 = 0;

#[repr(C)]
struct AccentPolicy {
    accent_state: i32,
    accent_flags: i32,
    accent_color: u32,
    accent_color_secondary: u32,
}

#[repr(C)]
struct WindowCompositionAttributeData {
    attribute: i32,
    data: *mut core::ffi::c_void,
    size_of_data: i32,
}

type SetWindowCompositionAttributeFn =
    unsafe extern "system" fn(HWND, *mut WindowCompositionAttributeData) -> BOOL;

/// Resolved by name rather than linked directly. The API is undocumented, so a
/// hard import would fail the whole binary's load if it ever went away, where
/// this simply leaves gpui's gradient in place.
const FUNCTION_NAME: &[u8] = b"SetWindowCompositionAttribute\0";

fn resolve() -> Option<SetWindowCompositionAttributeFn> {
    use windows::core::{w, PCSTR};
    use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};

    let module = unsafe { GetModuleHandleW(w!("user32.dll")) }.ok()?;
    let address = unsafe { GetProcAddress(module, PCSTR(FUNCTION_NAME.as_ptr())) }?;
    let entry_point = address as *const core::ffi::c_void;
    // A Win32 entry point and a function pointer to it are the same width on
    // every platform this shell builds for.
    Some(unsafe { std::mem::transmute_copy(&entry_point) })
}

/// Removes the window's accent policy, and with it the top-edge highlight that
/// gpui's transparent gradient brings. `false` means the API was not available
/// and the accent is still whatever gpui set.
pub fn disable_accent(window: HWND) -> bool {
    let Some(set_window_composition_attribute) = resolve() else {
        return false;
    };

    let mut accent = AccentPolicy {
        accent_state: ACCENT_DISABLED,
        accent_flags: 0,
        accent_color: 0,
        accent_color_secondary: 0,
    };
    let mut data = WindowCompositionAttributeData {
        attribute: ACCENT_POLICY_ATTRIBUTE,
        data: &mut accent as *mut AccentPolicy as *mut core::ffi::c_void,
        size_of_data: std::mem::size_of::<AccentPolicy>() as i32,
    };

    unsafe { set_window_composition_attribute(window, &mut data) }.as_bool()
}

pub fn configure_transparent_surface(window: HWND) {
    use windows::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMNCRP_DISABLED, DWMWA_BORDER_COLOR, DWMWA_COLOR_NONE,
        DWMWA_NCRENDERING_POLICY, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND,
    };

    disable_transitions(window);
    let non_client_disabled = DWMNCRP_DISABLED;
    let border_color = DWMWA_COLOR_NONE;
    let corner_preference = DWMWCP_DONOTROUND;
    unsafe {
        let _ = DwmSetWindowAttribute(
            window,
            DWMWA_NCRENDERING_POLICY,
            &non_client_disabled as *const _ as *const core::ffi::c_void,
            std::mem::size_of_val(&non_client_disabled) as u32,
        );
        let _ = DwmSetWindowAttribute(
            window,
            DWMWA_BORDER_COLOR,
            &border_color as *const _ as *const core::ffi::c_void,
            std::mem::size_of_val(&border_color) as u32,
        );
        let _ = DwmSetWindowAttribute(
            window,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &corner_preference as *const _ as *const core::ffi::c_void,
            std::mem::size_of_val(&corner_preference) as u32,
        );
    }
    disable_accent(window);
}

pub fn configure_acrylic_surface(window: HWND, dark: bool) -> bool {
    use windows::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMSBT_TRANSIENTWINDOW, DWMWA_SYSTEMBACKDROP_TYPE,
        DWMWA_USE_IMMERSIVE_DARK_MODE,
    };

    let dark = BOOL::from(dark);
    let applied = unsafe {
        let _ = DwmSetWindowAttribute(
            window,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &dark as *const _ as *const core::ffi::c_void,
            std::mem::size_of_val(&dark) as u32,
        );
        DwmSetWindowAttribute(
            window,
            DWMWA_SYSTEMBACKDROP_TYPE,
            &DWMSBT_TRANSIENTWINDOW as *const _ as *const core::ffi::c_void,
            std::mem::size_of_val(&DWMSBT_TRANSIENTWINDOW) as u32,
        )
        .is_ok()
    };
    if applied {
        disable_accent(window);
    }
    applied
}

pub fn disable_transitions(window: HWND) {
    use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_TRANSITIONS_FORCEDISABLED};

    let disabled = BOOL(1);
    unsafe {
        let _ = DwmSetWindowAttribute(
            window,
            DWMWA_TRANSITIONS_FORCEDISABLED,
            &disabled as *const _ as *const core::ffi::c_void,
            std::mem::size_of_val(&disabled) as u32,
        );
    }
}

pub fn set_rounded_client_region(window: &gpui::Window, radius: f32) {
    use windows::Win32::Graphics::Gdi::{
        ClientToScreen, CreateRoundRectRgn, DeleteObject, SetWindowRgn,
    };
    use windows::Win32::UI::WindowsAndMessaging::{GetClientRect, GetWindowRect};

    let Some(hwnd) = crate::windows::window_hwnd(window) else {
        return;
    };
    unsafe {
        let mut client = windows::Win32::Foundation::RECT::default();
        let mut client_origin = windows::Win32::Foundation::POINT::default();
        let mut window_rect = windows::Win32::Foundation::RECT::default();
        if GetClientRect(hwnd, &mut client).is_err()
            || !ClientToScreen(hwnd, &mut client_origin).as_bool()
            || GetWindowRect(hwnd, &mut window_rect).is_err()
        {
            return;
        }
        let left = client_origin.x - window_rect.left;
        let top = client_origin.y - window_rect.top;
        let radius = (radius * window.scale_factor()).round() as i32;
        let region = CreateRoundRectRgn(
            left,
            top,
            left + client.right - client.left + 1,
            top + client.bottom - client.top + 1,
            radius * 2,
            radius * 2,
        );
        if !region.is_invalid() && SetWindowRgn(hwnd, Some(region), true) == 0 {
            let _ = DeleteObject(region.into());
        }
    }
}

pub fn set_stacked_rounded_client_region(
    window: &gpui::Window,
    offset: f32,
    item_height: f32,
    gap: f32,
    count: usize,
    radius: f32,
) {
    use windows::Win32::Graphics::Gdi::{
        ClientToScreen, CombineRgn, CreateRectRgn, CreateRoundRectRgn, DeleteObject, SetWindowRgn,
        RGN_OR,
    };
    use windows::Win32::UI::WindowsAndMessaging::{GetClientRect, GetWindowRect};

    let Some(hwnd) = crate::windows::window_hwnd(window) else {
        return;
    };
    unsafe {
        let mut client = windows::Win32::Foundation::RECT::default();
        let mut client_origin = windows::Win32::Foundation::POINT::default();
        let mut window_rect = windows::Win32::Foundation::RECT::default();
        if GetClientRect(hwnd, &mut client).is_err()
            || !ClientToScreen(hwnd, &mut client_origin).as_bool()
            || GetWindowRect(hwnd, &mut window_rect).is_err()
        {
            return;
        }
        let left = client_origin.x - window_rect.left;
        let scale = window.scale_factor();
        let top = client_origin.y - window_rect.top + (offset * scale).round() as i32;
        let radius = (radius * scale).round() as i32;
        let width = client.right - client.left;
        let stack = CreateRectRgn(0, 0, 0, 0);
        if stack.is_invalid() {
            return;
        }
        for index in 0..count {
            let item_rect = stacked_item_rect(left, top, width, item_height, gap, index, scale);
            let item = CreateRoundRectRgn(
                item_rect.left,
                item_rect.top,
                item_rect.right,
                item_rect.bottom,
                radius * 2,
                radius * 2,
            );
            if item.is_invalid() {
                continue;
            }
            let _ = CombineRgn(Some(stack), Some(stack), Some(item), RGN_OR);
            let _ = DeleteObject(item.into());
        }
        if SetWindowRgn(hwnd, Some(stack), true) == 0 {
            let _ = DeleteObject(stack.into());
        }
    }
}

fn stacked_item_rect(
    left: i32,
    top: i32,
    width: i32,
    item_height: f32,
    gap: f32,
    index: usize,
    scale: f32,
) -> RECT {
    let item_height = (item_height * scale).round() as i32;
    let gap = (gap * scale).round() as i32;
    let top = top + index as i32 * (item_height + gap);
    RECT {
        left,
        top,
        right: left + width,
        bottom: top + item_height,
    }
}

fn outer_window_bounds(
    bounds: Bounds<Pixels>,
    scale: f32,
    window_rect: RECT,
    client_origin: POINT,
    client_rect: RECT,
) -> (i32, i32, i32, i32) {
    let left = client_origin.x - window_rect.left;
    let top = client_origin.y - window_rect.top;
    let right = window_rect.right - client_origin.x - client_rect.right;
    let bottom = window_rect.bottom - client_origin.y - client_rect.bottom;
    let x = (f32::from(bounds.origin.x) * scale).round() as i32;
    let y = (f32::from(bounds.origin.y) * scale).round() as i32;
    let width = (f32::from(bounds.size.width) * scale).round() as i32;
    let height = (f32::from(bounds.size.height) * scale).round() as i32;
    (
        x - left,
        y - top,
        width + left + right,
        height + top + bottom,
    )
}

pub fn apply_window_bounds(window: HWND, bounds: Bounds<Pixels>, scale: f32) {
    use windows::Win32::Graphics::Gdi::ClientToScreen;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetClientRect, GetWindowRect, SetWindowPos, SWP_NOACTIVATE, SWP_NOZORDER,
    };

    let mut client_rect = RECT::default();
    let mut window_rect = RECT::default();
    let mut client_origin = POINT::default();
    unsafe {
        if GetClientRect(window, &mut client_rect).is_err()
            || GetWindowRect(window, &mut window_rect).is_err()
            || !ClientToScreen(window, &mut client_origin).as_bool()
        {
            return;
        }
        let (x, y, width, height) =
            outer_window_bounds(bounds, scale, window_rect, client_origin, client_rect);
        let _ = SetWindowPos(
            window,
            None,
            x,
            y,
            width,
            height,
            SWP_NOACTIVATE | SWP_NOZORDER,
        );
    }
}

#[cfg(not(test))]
pub fn apply_window_origin(window: HWND, origin: Point<Pixels>, scale: f32) {
    use windows::Win32::Graphics::Gdi::ClientToScreen;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetClientRect, GetWindowRect, SetWindowPos, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER,
    };

    let mut client_rect = RECT::default();
    let mut window_rect = RECT::default();
    let mut client_origin = POINT::default();
    unsafe {
        if GetClientRect(window, &mut client_rect).is_err()
            || GetWindowRect(window, &mut window_rect).is_err()
            || !ClientToScreen(window, &mut client_origin).as_bool()
        {
            return;
        }
        let bounds = Bounds {
            origin,
            size: gpui::Size::default(),
        };
        let (x, y, _, _) =
            outer_window_bounds(bounds, scale, window_rect, client_origin, client_rect);
        let _ = SetWindowPos(
            window,
            None,
            x,
            y,
            0,
            0,
            SWP_NOACTIVATE | SWP_NOSIZE | SWP_NOZORDER,
        );
    }
}

pub fn stage_window(window: HWND, bounds: Bounds<Pixels>, scale: f32, activate: bool) {
    use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_CLOAK};
    use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_SHOWNOACTIVATE};

    apply_window_bounds(window, bounds, scale);
    let cloaked = BOOL(1);
    unsafe {
        let _ = DwmSetWindowAttribute(
            window,
            DWMWA_CLOAK,
            &cloaked as *const _ as *const core::ffi::c_void,
            std::mem::size_of_val(&cloaked) as u32,
        );
        if !activate {
            let _ = ShowWindow(window, SW_SHOWNOACTIVATE);
        }
    }
}

pub fn reveal_window(window: HWND, activate: bool, duration_ms: u32) {
    use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_CLOAK};
    use windows::Win32::UI::WindowsAndMessaging::{
        AnimateWindow, ShowWindow, AW_ACTIVATE, AW_BLEND, SW_HIDE, SW_SHOW, SW_SHOWNOACTIVATE,
    };

    let cloaked = BOOL(0);
    unsafe {
        if duration_ms > 0 {
            let _ = ShowWindow(window, SW_HIDE);
        }
        let _ = DwmSetWindowAttribute(
            window,
            DWMWA_CLOAK,
            &cloaked as *const _ as *const core::ffi::c_void,
            std::mem::size_of_val(&cloaked) as u32,
        );
        if duration_ms == 0 {
            let _ = ShowWindow(window, if activate { SW_SHOW } else { SW_SHOWNOACTIVATE });
            return;
        }
        let flags = if activate {
            AW_BLEND | AW_ACTIVATE
        } else {
            AW_BLEND
        };
        if AnimateWindow(window, duration_ms, flags).is_err() {
            let _ = ShowWindow(window, if activate { SW_SHOW } else { SW_SHOWNOACTIVATE });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Getting either layout wrong is memory corruption in a call that takes no
    /// type information at all, so the sizes are asserted rather than trusted.
    #[test]
    fn the_layout_windows_expects_is_the_layout_we_send() {
        assert_eq!(std::mem::size_of::<AccentPolicy>(), 16);
        assert_eq!(std::mem::size_of::<WindowCompositionAttributeData>(), 24);
        assert_eq!(std::mem::align_of::<AccentPolicy>(), 4);
    }

    #[test]
    fn the_import_name_is_nul_terminated() {
        assert_eq!(FUNCTION_NAME.last(), Some(&0u8));
        assert_eq!(
            std::str::from_utf8(&FUNCTION_NAME[..FUNCTION_NAME.len() - 1]).unwrap(),
            "SetWindowCompositionAttribute"
        );
    }

    #[test]
    fn early_or_failed_compositor_waits_use_the_fallback_pacing() {
        assert!(needs_vsync_fallback(false, Duration::from_millis(8)));
        assert!(needs_vsync_fallback(true, Duration::from_micros(999)));
        assert!(!needs_vsync_fallback(true, Duration::from_millis(1)));
    }

    #[test]
    fn animation_windows_join_the_current_frame_once() {
        let (sender, receiver) = sync_channel(4);
        sender.send(11).unwrap();
        sender.send(12).unwrap();
        sender.send(11).unwrap();
        let mut windows = SmallVec::from_slice(&[10]);
        drain_animation_windows(&receiver, &mut windows);
        assert_eq!(windows.as_slice(), &[10, 11, 12]);
    }

    #[test]
    fn requested_client_bounds_include_the_native_frame() {
        let bounds = Bounds {
            origin: gpui::point(gpui::px(100.0), gpui::px(50.0)),
            size: gpui::size(gpui::px(200.0), gpui::px(140.0)),
        };
        assert_eq!(
            outer_window_bounds(
                bounds,
                1.5,
                RECT {
                    left: 0,
                    top: 0,
                    right: 302,
                    bottom: 212,
                },
                POINT { x: 1, y: 1 },
                RECT {
                    left: 0,
                    top: 0,
                    right: 300,
                    bottom: 210,
                },
            ),
            (149, 74, 302, 212)
        );
    }

    #[test]
    fn stacked_regions_scale_each_item_and_leave_device_pixel_gaps() {
        for scale in [1.0_f32, 1.25, 1.5] {
            let width = (208.0 * scale).round() as i32;
            let item_height = (148.0 * scale).round() as i32;
            let gap = (4.0 * scale).round() as i32;
            let rects = (0..4)
                .map(|index| stacked_item_rect(8, 31, width, 148.0, 4.0, index, scale))
                .collect::<Vec<_>>();
            assert_eq!(rects.len(), 4);
            for rect in &rects {
                assert_eq!(rect.right - rect.left, width);
                assert_eq!(rect.bottom - rect.top, item_height);
            }
            for pair in rects.windows(2) {
                assert_eq!(pair[1].top - pair[0].bottom, gap);
            }
        }
    }
}
