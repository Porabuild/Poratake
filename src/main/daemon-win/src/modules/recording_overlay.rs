use crate::overlay::{
    create_popup_window, default_wndproc, ensure_window_class, monitors, rect_height, rect_width,
};
use crate::protocol::{param_i32, respond_error, respond_success, Request};
use crate::router::{method_not_found, Module, Reply};
use crate::ui::run_on_ui;
use serde_json::json;
use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateSolidBrush, DeleteObject, EndPaint, FillRect, PAINTSTRUCT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DestroyWindow, SetLayeredWindowAttributes, SetWindowDisplayAffinity, SetWindowPos, ShowWindow,
    HWND_TOPMOST, LWA_ALPHA, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SW_SHOWNOACTIVATE,
    WDA_EXCLUDEFROMCAPTURE, WM_PAINT, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    WS_EX_TOPMOST, WS_EX_TRANSPARENT,
};

const CLASS_NAME: &str = "CaptyRecordingOverlay";
const DIM_ALPHA: u8 = 128;

enum OverlayWindowError {
    Create,
    ContentProtection,
}

thread_local! {
    static WINDOWS: RefCell<Vec<HWND>> = const { RefCell::new(Vec::new()) };
}

unsafe extern "system" fn wndproc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_PAINT {
        paint(window);
        return LRESULT(0);
    }

    default_wndproc(window, message, wparam, lparam)
}

fn paint(window: HWND) {
    unsafe {
        let mut paint = PAINTSTRUCT::default();
        let dc = BeginPaint(window, &mut paint);
        let brush = CreateSolidBrush(COLORREF(0));
        FillRect(dc, &paint.rcPaint, brush);
        let _ = DeleteObject(brush.into());
        let _ = EndPaint(window, &paint);
    }
}

fn intersection(left: &RECT, right: &RECT) -> Option<RECT> {
    let rect = RECT {
        left: left.left.max(right.left),
        top: left.top.max(right.top),
        right: left.right.min(right.right),
        bottom: left.bottom.min(right.bottom),
    };

    if rect_width(&rect) <= 0 || rect_height(&rect) <= 0 {
        return None;
    }

    Some(rect)
}

fn dim_rectangles(monitor: RECT, recording: RECT) -> Vec<RECT> {
    let Some(hole) = intersection(&monitor, &recording) else {
        return vec![monitor];
    };

    let candidates = [
        RECT {
            left: monitor.left,
            top: monitor.top,
            right: monitor.right,
            bottom: hole.top,
        },
        RECT {
            left: monitor.left,
            top: hole.bottom,
            right: monitor.right,
            bottom: monitor.bottom,
        },
        RECT {
            left: monitor.left,
            top: hole.top,
            right: hole.left,
            bottom: hole.bottom,
        },
        RECT {
            left: hole.right,
            top: hole.top,
            right: monitor.right,
            bottom: hole.bottom,
        },
    ];

    candidates
        .into_iter()
        .filter(|rect| rect_width(rect) > 0 && rect_height(rect) > 0)
        .collect()
}

fn create_dim_window(rect: &RECT) -> Result<HWND, OverlayWindowError> {
    let styles =
        WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED | WS_EX_TRANSPARENT;
    let window = create_popup_window(CLASS_NAME, styles, rect).ok_or(OverlayWindowError::Create)?;

    unsafe {
        let _ = SetLayeredWindowAttributes(window, COLORREF(0), DIM_ALPHA, LWA_ALPHA);
        if SetWindowDisplayAffinity(window, WDA_EXCLUDEFROMCAPTURE).is_err() {
            let _ = DestroyWindow(window);
            return Err(OverlayWindowError::ContentProtection);
        }
        let _ = ShowWindow(window, SW_SHOWNOACTIVATE);
        let _ = SetWindowPos(
            window,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }

    Ok(window)
}

fn teardown() {
    let windows = WINDOWS.with(|windows| std::mem::take(&mut *windows.borrow_mut()));
    for window in windows {
        unsafe {
            let _ = DestroyWindow(window);
        }
    }
}

fn show_overlay(recording: RECT) -> Result<bool, OverlayWindowError> {
    teardown();
    ensure_window_class(CLASS_NAME, Some(wndproc), None);

    let mut windows = Vec::new();
    for monitor in monitors() {
        for rect in dim_rectangles(monitor.rect, recording) {
            match create_dim_window(&rect) {
                Ok(window) => windows.push(window),
                Err(error) => {
                    for window in windows {
                        unsafe {
                            let _ = DestroyWindow(window);
                        }
                    }
                    return Err(error);
                }
            }
        }
    }

    let visible = !windows.is_empty();
    WINDOWS.with(|state| {
        *state.borrow_mut() = windows;
    });
    Ok(visible)
}

pub struct RecordingOverlayModule {
    visible: Arc<AtomicBool>,
}

impl RecordingOverlayModule {
    pub fn new() -> Self {
        Self {
            visible: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl Module for RecordingOverlayModule {
    fn name(&self) -> &'static str {
        "recording-overlay"
    }

    fn handle(&mut self, request: &Request) -> Reply {
        match request.method.as_str() {
            "show" => {
                let Some(x) = param_i32(&request.params, "x") else {
                    return Reply::Now(Err((
                        "INVALID_PARAMS".to_string(),
                        "show requires x, y, width, height".to_string(),
                    )));
                };
                let Some(y) = param_i32(&request.params, "y") else {
                    return Reply::Now(Err((
                        "INVALID_PARAMS".to_string(),
                        "show requires x, y, width, height".to_string(),
                    )));
                };
                let Some(width) = param_i32(&request.params, "width") else {
                    return Reply::Now(Err((
                        "INVALID_PARAMS".to_string(),
                        "show requires x, y, width, height".to_string(),
                    )));
                };
                let Some(height) = param_i32(&request.params, "height") else {
                    return Reply::Now(Err((
                        "INVALID_PARAMS".to_string(),
                        "show requires x, y, width, height".to_string(),
                    )));
                };

                if width <= 0 || height <= 0 {
                    return Reply::Now(Err((
                        "INVALID_PARAMS".to_string(),
                        "show requires positive width and height".to_string(),
                    )));
                }

                let request_id = request.id.clone();
                let visible = self.visible.clone();
                run_on_ui(move || {
                    let result = show_overlay(RECT {
                        left: x,
                        top: y,
                        right: x.saturating_add(width),
                        bottom: y.saturating_add(height),
                    });
                    match result {
                        Ok(shown) => {
                            visible.store(shown, Ordering::Release);
                            respond_success(&request_id, json!({ "visible": shown }));
                        }
                        Err(OverlayWindowError::Create) => {
                            visible.store(false, Ordering::Release);
                            respond_error(
                                &request_id,
                                "WINDOW_ERROR",
                                "Failed to create recording overlay window",
                            );
                        }
                        Err(OverlayWindowError::ContentProtection) => {
                            visible.store(false, Ordering::Release);
                            respond_error(
                                &request_id,
                                "CONTENT_PROTECTION_ERROR",
                                "Failed to protect recording overlay window",
                            );
                        }
                    }
                });
                Reply::Deferred
            }
            "hide" => {
                let request_id = request.id.clone();
                let visible = self.visible.clone();
                run_on_ui(move || {
                    teardown();
                    visible.store(false, Ordering::Release);
                    respond_success(&request_id, json!({ "visible": false }));
                });
                Reply::Deferred
            }
            "status" => Reply::Now(Ok(Some(json!({
                "visible": self.visible.load(Ordering::Acquire)
            })))),
            method => method_not_found(method),
        }
    }
}
