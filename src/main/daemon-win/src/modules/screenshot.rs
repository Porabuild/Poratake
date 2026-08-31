use crate::desktop_frame::{
    DesktopFrame, apply_alpha_mask, capture_rect, capture_window, frozen_rect, write_image,
};
use crate::protocol::{Request, params, respond_error, respond_success};
use crate::router::{Module, Reply, method_not_found};
use poratake_daemon_common::contract::{SCREENSHOT_MODULE, ScreenshotMethod};
use poratake_daemon_common::geometry::{CaptureAreaRequest, CaptureRect, CaptureWindowRequest};
use serde_json::json;
use std::ffi::c_void;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Dwm::DwmFlush;

pub struct ScreenshotModule;

impl Module for ScreenshotModule {
    fn name(&self) -> &'static str {
        SCREENSHOT_MODULE
    }

    fn handle(&mut self, request: &Request) -> Reply {
        match ScreenshotMethod::parse(&request.method) {
            Some(ScreenshotMethod::CaptureArea) => capture_area(request),
            Some(ScreenshotMethod::CaptureWindow) => capture_target_window(request),
            None => method_not_found(&request.method),
        }
    }
}

fn capture_area(request: &Request) -> Reply {
    let request_id = request.id.clone();
    let wire: CaptureAreaRequest = match params(request) {
        Ok(wire) => wire,
        Err(error) => return Reply::Now(Err(error)),
    };
    if wire.path.as_os_str().is_empty() {
        return missing_path();
    }
    let Some(bounds) = area_bounds(wire.capture.rect) else {
        return Reply::Now(Err((
            "INVALID_PARAMS".to_string(),
            "A capture area with a positive width and height is required".to_string(),
        )));
    };

    let cached = wire.cached;
    let window_id = wire.window_id;
    let path = wire.path.to_string_lossy().into_owned();

    crate::trace::trace("capture-area entered cached={cached}");

    spawn_capture(request_id, path, move || {
        crate::trace::trace("capture() entered");
        let frame = if cached { frozen_rect(bounds) } else { None };
        crate::trace::trace("frame resolved");
        let Some(mut frame) = frame else {
            crate::trace::trace("calling DwmFlush");
            unsafe { DwmFlush() }.map_err(|error| error.to_string())?;
            crate::trace::trace("calling capture_rect");
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
    let request_id = request.id.clone();
    let wire: CaptureWindowRequest = match params(request) {
        Ok(wire) => wire,
        Err(error) => return Reply::Now(Err(error)),
    };
    if wire.path.as_os_str().is_empty() {
        return missing_path();
    }

    let path = wire.path.to_string_lossy().into_owned();
    let window_id = wire.window_id;

    spawn_capture(request_id, path, move || {
        capture_window(HWND(window_id as isize as *mut c_void))
    })
}

fn spawn_capture(
    request_id: String,
    path: String,
    capture: impl FnOnce() -> Result<DesktopFrame, String> + Send + 'static,
) -> Reply {
    crate::trace::trace("spawning worker");
    let spawned = std::thread::Builder::new().spawn(move || {
        crate::trace::trace("worker thread started");
        let frame = match capture() {
            Ok(frame) => frame,
            Err(message) => {
                crate::trace::trace(&format!("capture failed: {message}"));
                respond_error(&request_id, "CAPTURE_FAILED", &message);
                return;
            }
        };
        crate::trace::trace("writing image");

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
        Ok(_) => {
            crate::trace::trace("worker spawned ok");
            Reply::Deferred
        }
        Err(error) => {
            crate::trace::trace("worker spawn failed: {error}");
            Reply::Now(Err((
                "CAPTURE_FAILED".to_string(),
                format!("Failed to start the capture: {error}"),
            )))
        }
    }
}

fn area_bounds(rect: CaptureRect) -> Option<RECT> {
    if rect.width <= 0 || rect.height <= 0 {
        return None;
    }

    Some(RECT {
        left: rect.x,
        top: rect.y,
        right: rect.x + rect.width,
        bottom: rect.y + rect.height,
    })
}

fn missing_path() -> Reply {
    Reply::Now(Err((
        "INVALID_PARAMS".to_string(),
        "A destination path is required".to_string(),
    )))
}
