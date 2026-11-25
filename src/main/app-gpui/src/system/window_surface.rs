use gpui::{Bounds, Pixels, Window};
use herogpui::gpui;

pub fn prepare_transparent_surface(window: &Window) {
    #[cfg(windows)]
    if let Some(hwnd) = crate::windows::window_hwnd(window) {
        crate::system::window_composition::configure_transparent_surface(hwnd);
    }
    #[cfg(not(windows))]
    let _ = window;
}

pub fn set_rounded_clip(window: &Window, radius: f32) {
    #[cfg(windows)]
    crate::system::window_composition::set_rounded_client_region(window, radius);
    #[cfg(not(windows))]
    let _ = (window, radius);
}

pub fn disable_transitions(window: &Window) {
    #[cfg(windows)]
    if let Some(hwnd) = crate::windows::window_hwnd(window) {
        crate::system::window_composition::disable_transitions(hwnd);
    }
    #[cfg(all(target_os = "macos", not(test)))]
    crate::system::window_visibility::disable_macos_transitions(window);
    #[cfg(not(any(windows, all(target_os = "macos", not(test)))))]
    let _ = window;
}

pub fn stage(window: &Window, bounds: Bounds<Pixels>, scale: Option<f32>, activate: bool) {
    #[cfg(windows)]
    if let Some(hwnd) = crate::windows::window_hwnd(window) {
        crate::system::window_composition::stage_window(
            hwnd,
            bounds,
            scale.unwrap_or_else(|| window.scale_factor()),
            activate,
        );
    }
    #[cfg(not(windows))]
    let _ = (window, bounds, scale, activate);
}

#[cfg(windows)]
pub fn reveal(window: &Window, activate: bool) {
    if let Some(hwnd) = crate::windows::window_hwnd(window) {
        crate::system::window_composition::reveal_window(hwnd, activate);
    }
}

#[cfg(windows)]
pub fn hide(window: &Window) {
    if let Some(hwnd) = crate::windows::window_hwnd(window) {
        crate::system::window_composition::hide_window(hwnd);
    }
}

#[cfg(windows)]
pub fn apply_bounds(window: &Window, bounds: Bounds<Pixels>, scale: Option<f32>) {
    if let Some(hwnd) = crate::windows::window_hwnd(window) {
        crate::system::window_composition::apply_window_bounds(
            hwnd,
            bounds,
            scale.unwrap_or_else(|| window.scale_factor()),
        );
    }
}

pub fn set_backdrop_mode(window: &Window, dark: bool, translucent: bool) -> bool {
    #[cfg(windows)]
    {
        let Some(hwnd) = crate::windows::window_hwnd(window) else {
            return false;
        };
        if translucent {
            return crate::system::window_composition::configure_acrylic_surface(hwnd, dark);
        }
        crate::system::window_composition::configure_opaque_surface(hwnd, dark);
        true
    }
    #[cfg(not(windows))]
    {
        let _ = (window, dark, translucent);
        false
    }
}
