use crate::overlay::{
    configure_overlay_window, disable_window_transitions, rect_height, rect_width,
};
use crate::protocol::{Request, param_bool, param_i32, param_str, respond_success};
use crate::router::{Module, Reply, method_not_found};
use crate::ui::run_on_ui;
use poratake_daemon_common::contract::{AREA_SELECTOR_MODULE, AreaSelectorMethod};
use serde_json::json;
use std::ffi::c_void;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Gdi::{
    CombineRgn, CreateRectRgn, DeleteObject, ERROR, RGN_DIFF, SetWindowRgn,
};
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, GetClientRect, GetForegroundWindow, GetWindowThreadProcessId, HWND_TOPMOST,
    IsIconic, IsWindow, SW_RESTORE, SWP_HIDEWINDOW, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SWP_NOZORDER, SWP_SHOWWINDOW, SetForegroundWindow, SetWindowPos, ShowWindow,
};
use windows::core::Error;

fn scaled_hole_rect(
    hole: (i32, i32, i32, i32, i32, i32, i32),
    client_width: i32,
    client_height: i32,
) -> Option<(i32, i32, i32, i32)> {
    let (x, y, width, height, window_width, window_height, inset) = hole;

    if width - inset * 2 <= 0 || height - inset * 2 <= 0 {
        return None;
    }

    let scale_x = client_width as f64 / window_width as f64;
    let scale_y = client_height as f64 / window_height as f64;

    Some((
        ((x + inset) as f64 * scale_x).round() as i32,
        ((y + inset) as f64 * scale_y).round() as i32,
        ((x + width - inset) as f64 * scale_x).round() as i32,
        ((y + height - inset) as f64 * scale_y).round() as i32,
    ))
}

fn set_electron_window_region(
    window: HWND,
    hole: Option<(i32, i32, i32, i32, i32, i32, i32)>,
) -> windows::core::Result<()> {
    unsafe {
        let Some(hole) = hole else {
            if SetWindowRgn(window, None, true) == 0 {
                return Err(Error::from_thread());
            }
            return Ok(());
        };

        let mut client = RECT::default();
        GetClientRect(window, &mut client)?;
        let client_width = rect_width(&client);
        let client_height = rect_height(&client);
        let (_, _, _, _, window_width, window_height, _) = hole;
        if client_width <= 0 || client_height <= 0 || window_width <= 0 || window_height <= 0 {
            return Err(Error::from_thread());
        }

        let Some((hole_left, hole_top, hole_right, hole_bottom)) =
            scaled_hole_rect(hole, client_width, client_height)
        else {
            if SetWindowRgn(window, None, true) == 0 {
                return Err(Error::from_thread());
            }
            return Ok(());
        };

        let region = CreateRectRgn(0, 0, client_width, client_height);
        if region.0.is_null() {
            return Err(Error::from_thread());
        }
        let hole_region = CreateRectRgn(hole_left, hole_top, hole_right, hole_bottom);
        if hole_region.0.is_null() {
            let error = Error::from_thread();
            let _ = DeleteObject(region.into());
            return Err(error);
        }

        let combined = CombineRgn(Some(region), Some(region), Some(hole_region), RGN_DIFF);
        let _ = DeleteObject(hole_region.into());
        if combined.0 == ERROR {
            let error = Error::from_thread();
            let _ = DeleteObject(region.into());
            return Err(error);
        }

        if SetWindowRgn(window, Some(region), true) == 0 {
            let error = Error::from_thread();
            let _ = DeleteObject(region.into());
            return Err(error);
        }
        Ok(())
    }
}

pub struct AreaSelectorModule;

impl AreaSelectorModule {
    pub fn new() -> Self {
        Self
    }
}

impl Module for AreaSelectorModule {
    fn name(&self) -> &'static str {
        AREA_SELECTOR_MODULE
    }

    fn handle(&mut self, request: &Request) -> Reply {
        match AreaSelectorMethod::parse(&request.method) {
            Some(AreaSelectorMethod::GetForegroundWindow) => {
                let window = unsafe { GetForegroundWindow() };
                let handle = if window.0.is_null() {
                    json!(null)
                } else {
                    json!(window.0 as usize)
                };
                Reply::Now(Ok(Some(json!({ "windowHandle": handle }))))
            }
            Some(AreaSelectorMethod::SetForegroundWindow) => {
                let Some(window_handle) = param_str(&request.params, "windowHandle")
                    .and_then(|value| value.parse::<usize>().ok())
                    .filter(|value| *value != 0)
                else {
                    return Reply::Now(Err((
                        "INVALID_PARAMS".to_string(),
                        "setForegroundWindow requires a windowHandle".to_string(),
                    )));
                };
                let request_id = request.id.clone();
                run_on_ui(move || {
                    let window = HWND(window_handle as *mut c_void);

                    unsafe {
                        if !IsWindow(Some(window)).as_bool() {
                            respond_success(&request_id, json!({ "restored": false }));
                            return;
                        }

                        if IsIconic(window).as_bool() {
                            let _ = ShowWindow(window, SW_RESTORE);
                        }

                        let foreground = GetForegroundWindow();
                        let foreground_thread = GetWindowThreadProcessId(foreground, None);
                        let target_thread = GetWindowThreadProcessId(window, None);
                        let current_thread = GetCurrentThreadId();

                        let attached_foreground = foreground_thread != 0
                            && foreground_thread != current_thread
                            && AttachThreadInput(current_thread, foreground_thread, true).as_bool();
                        let attached_target = target_thread != 0
                            && target_thread != current_thread
                            && AttachThreadInput(current_thread, target_thread, true).as_bool();

                        let restored = SetForegroundWindow(window).as_bool();
                        if restored {
                            let _ = BringWindowToTop(window);
                        }

                        if attached_foreground {
                            let _ = AttachThreadInput(current_thread, foreground_thread, false);
                        }
                        if attached_target {
                            let _ = AttachThreadInput(current_thread, target_thread, false);
                        }

                        respond_success(&request_id, json!({ "restored": restored }));
                    }
                });
                Reply::Deferred
            }
            Some(AreaSelectorMethod::SetWindowRegion) => {
                let Some(window_handle) = param_str(&request.params, "windowHandle")
                    .and_then(|value| value.parse::<usize>().ok())
                    .filter(|value| *value != 0)
                else {
                    return Reply::Now(Err((
                        "INVALID_PARAMS".to_string(),
                        "setWindowRegion requires a windowHandle".to_string(),
                    )));
                };
                let values = (
                    param_i32(&request.params, "x"),
                    param_i32(&request.params, "y"),
                    param_i32(&request.params, "width"),
                    param_i32(&request.params, "height"),
                    param_i32(&request.params, "windowWidth"),
                    param_i32(&request.params, "windowHeight"),
                    param_i32(&request.params, "inset"),
                );
                let hole = match values {
                    (None, None, None, None, None, None, None) => None,
                    (
                        Some(x),
                        Some(y),
                        Some(width),
                        Some(height),
                        Some(window_width),
                        Some(window_height),
                        Some(inset),
                    ) if width > 0
                        && height > 0
                        && window_width > 0
                        && window_height > 0
                        && inset >= 0 =>
                    {
                        Some((x, y, width, height, window_width, window_height, inset))
                    }
                    _ => {
                        return Reply::Now(Err((
                            "INVALID_PARAMS".to_string(),
                            "setWindowRegion requires complete region dimensions".to_string(),
                        )));
                    }
                };
                let window = HWND(window_handle as *mut c_void);
                match set_electron_window_region(window, hole) {
                    Ok(()) => Reply::Now(Ok(Some(json!({ "updated": true })))),
                    Err(error) => Reply::Now(Err((
                        "WINDOW_CONFIGURATION_FAILED".to_string(),
                        error.to_string(),
                    ))),
                }
            }
            Some(
                method @ (AreaSelectorMethod::DisableWindowTransitions
                | AreaSelectorMethod::HideWindowWithoutTransitions
                | AreaSelectorMethod::ShowWindowWithoutTransitions),
            ) => {
                let Some(window_handle) = param_str(&request.params, "windowHandle")
                    .and_then(|value| value.parse::<usize>().ok())
                    .filter(|value| *value != 0)
                else {
                    return Reply::Now(Err((
                        "INVALID_PARAMS".to_string(),
                        "window transition method requires a windowHandle".to_string(),
                    )));
                };
                let no_activate = param_bool(&request.params, "noActivate").unwrap_or(true);
                let window = HWND(window_handle as *mut c_void);
                let result = if no_activate {
                    configure_overlay_window(window)
                } else {
                    disable_window_transitions(window)
                };
                let result = result.and_then(|()| match method {
                    AreaSelectorMethod::ShowWindowWithoutTransitions => unsafe {
                        SetWindowPos(
                            window,
                            Some(HWND_TOPMOST),
                            0,
                            0,
                            0,
                            0,
                            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
                        )
                    },
                    AreaSelectorMethod::HideWindowWithoutTransitions => unsafe {
                        set_electron_window_region(window, None)?;
                        SetWindowPos(
                            window,
                            None,
                            0,
                            0,
                            0,
                            0,
                            SWP_NOMOVE
                                | SWP_NOSIZE
                                | SWP_NOACTIVATE
                                | SWP_NOZORDER
                                | SWP_HIDEWINDOW,
                        )
                    },
                    _ => Ok(()),
                });
                match result {
                    Ok(()) => Reply::Now(Ok(Some(json!({ "disabled": true })))),
                    Err(error) => Reply::Now(Err((
                        "WINDOW_CONFIGURATION_FAILED".to_string(),
                        error.to_string(),
                    ))),
                }
            }
            None => method_not_found(&request.method),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::scaled_hole_rect;

    #[test]
    fn maps_the_inset_hole_when_client_matches_the_window() {
        assert_eq!(
            scaled_hole_rect((100, 50, 400, 300, 1920, 1080, 16), 1920, 1080),
            Some((116, 66, 484, 334))
        );
    }

    #[test]
    fn scales_the_hole_to_the_physical_client_area() {
        assert_eq!(
            scaled_hole_rect((100, 50, 400, 300, 1920, 1080, 16), 3840, 2160),
            Some((232, 132, 968, 668))
        );
    }

    #[test]
    fn rounds_fractional_scaling_to_the_nearest_pixel() {
        assert_eq!(
            scaled_hole_rect((100, 100, 400, 400, 1000, 1000, 0), 1250, 1250),
            Some((125, 125, 625, 625))
        );
    }

    #[test]
    fn reports_no_hole_when_the_inset_swallows_the_selection() {
        assert_eq!(
            scaled_hole_rect((0, 0, 32, 300, 1920, 1080, 16), 1920, 1080),
            None
        );
        assert_eq!(
            scaled_hole_rect((0, 0, 400, 20, 1920, 1080, 16), 1920, 1080),
            None
        );
    }
}
