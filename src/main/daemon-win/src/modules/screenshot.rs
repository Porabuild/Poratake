use crate::desktop_frame::{
    DesktopFrame, FrozenComposition, capture_rect, capture_window, frozen_composition, write_image,
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
            "Capture area must have positive dimensions and non-overflowing bounds".to_string(),
        )));
    };

    let cached = wire.cached;
    let window_id = wire.window_id;
    let path = wire.path.to_string_lossy().into_owned();

    crate::trace::trace("capture-area entered cached={cached}");

    spawn_capture(request_id, path, move || {
        crate::trace::trace("capture() entered");
        let mut capture = capture_cached_or_live(
            cached,
            || frozen_composition(bounds).map(|composition| composition.map(CapturedImage::Frozen)),
            || {
                crate::trace::trace("calling DwmFlush");
                unsafe { DwmFlush() }.map_err(|error| error.to_string())?;
                crate::trace::trace("calling capture_rect");
                capture_rect(bounds).map(CapturedImage::Frame)
            },
        )?;
        crate::trace::trace("frame resolved");
        if let (CapturedImage::Frozen(composition), Some(window_id)) = (&mut capture, window_id) {
            let window = HWND(window_id as isize as *mut c_void);
            if let Ok(mask) = capture_window(window) {
                composition.set_alpha_mask(mask);
            }
        }
        Ok(capture)
    })
}

enum CapturedImage {
    Frame(DesktopFrame),
    Frozen(FrozenComposition),
}

impl CapturedImage {
    fn dimensions(&self) -> (u32, u32) {
        match self {
            Self::Frame(frame) => (frame.width, frame.height),
            Self::Frozen(composition) => composition.dimensions(),
        }
    }

    fn write_image(&self, path: &str) -> Result<(), String> {
        match self {
            Self::Frame(frame) => write_image(frame, path),
            Self::Frozen(composition) => composition.write_image(path),
        }
    }
}

fn capture_cached_or_live<T>(
    cached: bool,
    frozen: impl FnOnce() -> Result<Option<T>, String>,
    live: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    if cached && let Some(frame) = frozen()? {
        return Ok(frame);
    }
    live()
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
        capture_window(HWND(window_id as isize as *mut c_void)).map(CapturedImage::Frame)
    })
}

fn spawn_capture(
    request_id: String,
    path: String,
    capture: impl FnOnce() -> Result<CapturedImage, String> + Send + 'static,
) -> Reply {
    crate::trace::trace("spawning worker");
    let spawned = std::thread::Builder::new().spawn(move || {
        crate::trace::trace("worker thread started");
        let capture = match capture() {
            Ok(capture) => capture,
            Err(message) => {
                crate::trace::trace(&format!("capture failed: {message}"));
                respond_error(&request_id, "CAPTURE_FAILED", &message);
                return;
            }
        };
        crate::trace::trace("writing image");

        if let Err(message) = capture.write_image(&path) {
            respond_error(&request_id, "CAPTURE_FAILED", &message);
            return;
        }
        let (width, height) = capture.dimensions();

        respond_success(
            &request_id,
            json!({
                "path": path,
                "width": width,
                "height": height,
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
    let right = rect.x.checked_add(rect.width)?;
    let bottom = rect.y.checked_add(rect.height)?;

    Some(RECT {
        left: rect.x,
        top: rect.y,
        right,
        bottom,
    })
}

fn missing_path() -> Reply {
    Reply::Now(Err((
        "INVALID_PARAMS".to_string(),
        "A destination path is required".to_string(),
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn frame() -> DesktopFrame {
        DesktopFrame {
            bounds: RECT {
                left: 0,
                top: 0,
                right: 1,
                bottom: 1,
            },
            width: 1,
            height: 1,
            pixels: vec![0, 0, 0, 255],
        }
    }

    #[test]
    fn area_bounds_rejects_coordinate_overflow() {
        assert!(
            area_bounds(CaptureRect {
                x: i32::MAX - 10,
                y: 0,
                width: 11,
                height: 1,
            })
            .is_none()
        );
        assert!(
            area_bounds(CaptureRect {
                x: 0,
                y: i32::MAX - 10,
                width: 1,
                height: 11,
            })
            .is_none()
        );
    }

    #[test]
    fn area_bounds_preserves_valid_exclusive_edges() {
        let bounds = area_bounds(CaptureRect {
            x: -20,
            y: 30,
            width: 40,
            height: 50,
        })
        .expect("valid capture area");

        assert_eq!(bounds.left, -20);
        assert_eq!(bounds.top, 30);
        assert_eq!(bounds.right, 20);
        assert_eq!(bounds.bottom, 80);
    }

    #[test]
    fn capture_area_reports_coordinate_overflow_as_invalid_params() {
        for (x, y, width, height) in [(i32::MAX - 10, 0, 11, 1), (0, i32::MAX - 10, 1, 11)] {
            let request = Request {
                id: "overflowing-capture-area".into(),
                module: SCREENSHOT_MODULE.into(),
                method: ScreenshotMethod::CaptureArea.id().into(),
                params: Some(HashMap::from([
                    ("x".into(), serde_json::json!(x)),
                    ("y".into(), serde_json::json!(y)),
                    ("width".into(), serde_json::json!(width)),
                    ("height".into(), serde_json::json!(height)),
                    ("path".into(), serde_json::json!("capture.png")),
                ])),
            };

            let Reply::Now(Err((code, message))) = ScreenshotModule.handle(&request) else {
                panic!("overflowing bounds must be rejected before capture starts");
            };

            assert_eq!(code, "INVALID_PARAMS");
            assert_eq!(
                message,
                "Capture area must have positive dimensions and non-overflowing bounds"
            );
        }
    }

    #[test]
    fn failed_frozen_composition_does_not_fall_back_to_live_capture() {
        let mut live_called = false;
        let result = capture_cached_or_live(
            true,
            || Err("frozen allocation failed".into()),
            || {
                live_called = true;
                Ok(frame())
            },
        );

        match result {
            Ok(_) => panic!("frozen failure must be returned"),
            Err(error) => assert_eq!(error, "frozen allocation failed"),
        }
        assert!(!live_called);
    }

    #[test]
    fn missing_frozen_cache_uses_live_capture() {
        let captured =
            capture_cached_or_live(true, || Ok(None), || Ok(frame())).expect("live fallback");

        assert_eq!(captured.pixels, [0, 0, 0, 255]);
    }

    #[test]
    fn disabled_freeze_uses_live_capture_without_checking_cache() {
        let captured = capture_cached_or_live(
            false,
            || panic!("disabled freeze must not inspect the cache"),
            || Ok(frame()),
        )
        .expect("live capture");

        assert_eq!(captured.pixels, [0, 0, 0, 255]);
    }

    #[test]
    fn frozen_composition_uses_cached_pixels() {
        let captured = capture_cached_or_live(
            true,
            || Ok(Some(frame())),
            || panic!("cached pixels must not fall back to live capture"),
        )
        .expect("frozen capture");

        assert_eq!(captured.pixels, [0, 0, 0, 255]);
    }
}
