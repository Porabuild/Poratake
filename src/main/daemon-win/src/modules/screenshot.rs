use crate::desktop_frame::{
    DesktopFrame, apply_alpha_mask, capture_rect, capture_window, frozen_rect, write_image,
};
use crate::protocol::{
    Request, param_bool, param_i32, param_i64, param_str, respond_error, respond_success,
};
use crate::router::{Module, Reply, method_not_found};
use serde_json::json;
use std::ffi::c_void;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Dwm::DwmFlush;

pub struct ScreenshotModule;

impl Module for ScreenshotModule {
    fn name(&self) -> &'static str {
        "screenshot"
    }

    fn handle(&mut self, request: &Request) -> Reply {
        match request.method.as_str() {
            "capture-area" => capture_area(request),
            "capture-window" => capture_target_window(request),
            method => method_not_found(method),
        }
    }
}

fn capture_area(request: &Request) -> Reply {
    let Some(path) = param_str(&request.params, "path").map(str::to_string) else {
        return missing_path();
    };
    let Some(bounds) = area_bounds(request) else {
        return Reply::Now(Err((
            "INVALID_PARAMS".to_string(),
            "A capture area with a positive width and height is required".to_string(),
        )));
    };

    let cached = param_bool(&request.params, "cached").unwrap_or(false);
    let window_id = param_i64(&request.params, "windowId");
    let request_id = request.id.clone();

    spawn_capture(request_id, path, move || {
        let frame = if cached { frozen_rect(bounds) } else { None };
        let Some(mut frame) = frame else {
            unsafe { DwmFlush() }.map_err(|error| error.to_string())?;
            return capture_rect(bounds);
        };
        let Some(window_id) = window_id else {
            return Ok(frame);
        };
        let window = HWND(window_id as isize as *mut c_void);
        let Ok(mask) = capture_window(window) else {
            return Ok(frame);
        };

        apply_alpha_mask(&mut frame, &mask);
        Ok(frame)
    })
}

fn capture_target_window(request: &Request) -> Reply {
    let Some(path) = param_str(&request.params, "path").map(str::to_string) else {
        return missing_path();
    };
    let Some(window_id) = param_i64(&request.params, "windowId") else {
        return Reply::Now(Err((
            "INVALID_PARAMS".to_string(),
            "A window id is required".to_string(),
        )));
    };

    let request_id = request.id.clone();

    spawn_capture(request_id, path, move || {
        capture_window(HWND(window_id as isize as *mut c_void))
    })
}

fn spawn_capture(
    request_id: String,
    path: String,
    capture: impl FnOnce() -> Result<DesktopFrame, String> + Send + 'static,
) -> Reply {
    let spawned = std::thread::Builder::new().spawn(move || {
        let frame = match capture() {
            Ok(frame) => frame,
            Err(message) => {
                respond_error(&request_id, "CAPTURE_FAILED", &message);
                return;
            }
        };

        if let Err(message) = write_image(&frame, &path) {
            respond_error(&request_id, "CAPTURE_FAILED", &message);
            return;
        }

        respond_success(
            &request_id,
            json!({
                "path": path,
                "width": frame.width,
                "height": frame.height,
            }),
        );
    });

    match spawned {
        Ok(_) => Reply::Deferred,
        Err(error) => Reply::Now(Err((
            "CAPTURE_FAILED".to_string(),
            format!("Failed to start the capture: {error}"),
        ))),
    }
}

fn area_bounds(request: &Request) -> Option<RECT> {
    let x = param_i32(&request.params, "x")?;
    let y = param_i32(&request.params, "y")?;
    let width = param_i32(&request.params, "width")?;
    let height = param_i32(&request.params, "height")?;

    if width <= 0 || height <= 0 {
        return None;
    }

    Some(RECT {
        left: x,
        top: y,
        right: x + width,
        bottom: y + height,
    })
}

fn missing_path() -> Reply {
    Reply::Now(Err((
        "INVALID_PARAMS".to_string(),
        "A destination path is required".to_string(),
    )))
}
