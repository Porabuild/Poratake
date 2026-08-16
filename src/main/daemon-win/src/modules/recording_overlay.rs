use super::window_selector::window_bounds;
use crate::overlay::{
    create_popup_window, default_wndproc, disable_window_transitions, ensure_window_class,
    monitors, rect_height, rect_width, scale_for_dpi,
};
use crate::protocol::{param_i32, param_i64, param_str, respond_error, respond_success, Request};
use crate::router::{method_not_found, Module, Reply};
use crate::ui::run_on_ui;
use serde_json::json;
use std::cell::RefCell;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CombineRgn, CreateRoundRectRgn, CreateSolidBrush, DeleteObject, EndPaint, FillRect,
    SetWindowRgn, PAINTSTRUCT, RGN_DIFF,
};
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    DestroyWindow, GetWindowThreadProcessId, IsIconic, IsWindow, IsWindowVisible,
    SetLayeredWindowAttributes, SetWindowDisplayAffinity, SetWindowPos, ShowWindow, CHILDID_SELF,
    EVENT_OBJECT_LOCATIONCHANGE, EVENT_SYSTEM_FOREGROUND, HWND_TOPMOST, LWA_ALPHA, OBJID_WINDOW,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SW_HIDE, SW_SHOWNOACTIVATE, WDA_EXCLUDEFROMCAPTURE,
    WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS, WM_PAINT, WS_EX_LAYERED, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT,
};

const CLASS_NAME: &str = "PoratakeRecordingOverlay";
const HIGHLIGHT_CLASS_NAME: &str = "PoratakeRecordingHighlight";
const DIM_ALPHA: u8 = 128;
const HIGHLIGHT_THICKNESS: i32 = 1;
const HIGHLIGHT_GAP: i32 = 1;
const HIGHLIGHT_RADIUS: i32 = 8;
const HIGHLIGHT_FALLBACK: COLORREF = COLORREF(0x00EF_9288);

enum OverlayWindowError {
    Create,
    ContentProtection,
}

struct WindowHighlight {
    target: HWND,
    frame: HWND,
    hook: HWINEVENTHOOK,
    insert_after: HWND,
}

thread_local! {
    static WINDOWS: RefCell<Vec<HWND>> = const { RefCell::new(Vec::new()) };
    static HIGHLIGHT: RefCell<Option<WindowHighlight>> = const { RefCell::new(None) };
    static HIGHLIGHT_COLOR: RefCell<COLORREF> = const { RefCell::new(HIGHLIGHT_FALLBACK) };
}

/// The app hands over its live theme accent as `#rrggbb`; COLORREF orders the
/// channels the other way round.
fn parse_color(value: Option<&str>) -> COLORREF {
    let Some(hex) = value.map(|value| value.trim_start_matches('#')) else {
        return HIGHLIGHT_FALLBACK;
    };
    if hex.len() != 6 {
        return HIGHLIGHT_FALLBACK;
    }

    let channel = |range: std::ops::Range<usize>| u32::from_str_radix(&hex[range], 16).ok();
    match (channel(0..2), channel(2..4), channel(4..6)) {
        (Some(red), Some(green), Some(blue)) => COLORREF((blue << 16) | (green << 8) | red),
        _ => HIGHLIGHT_FALLBACK,
    }
}

unsafe extern "system" fn wndproc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_PAINT {
        paint(window, COLORREF(0));
        return LRESULT(0);
    }

    default_wndproc(window, message, wparam, lparam)
}

unsafe extern "system" fn highlight_wndproc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_PAINT {
        paint(window, HIGHLIGHT_COLOR.with(|color| *color.borrow()));
        return LRESULT(0);
    }

    default_wndproc(window, message, wparam, lparam)
}

fn paint(window: HWND, color: COLORREF) {
    unsafe {
        let mut paint = PAINTSTRUCT::default();
        let dc = BeginPaint(window, &mut paint);
        let brush = CreateSolidBrush(color);
        FillRect(dc, &paint.rcPaint, brush);
        let _ = DeleteObject(brush.into());
        let _ = EndPaint(window, &paint);
    }
}

/// The ring sits just outside the window with a hair of breathing room, so it
/// points at the window without touching or covering a pixel of it.
fn highlight_frame(bounds: RECT, inset: i32) -> RECT {
    RECT {
        left: bounds.left - inset,
        top: bounds.top - inset,
        right: bounds.right + inset,
        bottom: bounds.bottom + inset,
    }
}

struct HighlightMetrics {
    inset: i32,
    thickness: i32,
    radius: i32,
}

fn highlight_metrics(window: HWND) -> HighlightMetrics {
    let dpi = unsafe { GetDpiForWindow(window) };
    let scaled = |value: i32| scale_for_dpi(value, dpi).max(1);
    let thickness = scaled(HIGHLIGHT_THICKNESS);

    HighlightMetrics {
        inset: thickness + scaled(HIGHLIGHT_GAP),
        thickness,
        radius: scaled(HIGHLIGHT_RADIUS),
    }
}

fn apply_ring(window: HWND, width: i32, height: i32, metrics: &HighlightMetrics) {
    let diameter = metrics.radius * 2;
    let inner_diameter = ((metrics.radius - metrics.thickness).max(0)) * 2;

    unsafe {
        let outer = CreateRoundRectRgn(0, 0, width + 1, height + 1, diameter, diameter);
        let inner = CreateRoundRectRgn(
            metrics.thickness,
            metrics.thickness,
            width - metrics.thickness + 1,
            height - metrics.thickness + 1,
            inner_diameter,
            inner_diameter,
        );
        if outer.is_invalid() || inner.is_invalid() {
            let _ = DeleteObject(outer.into());
            let _ = DeleteObject(inner.into());
            return;
        }

        CombineRgn(Some(outer), Some(outer), Some(inner), RGN_DIFF);
        let _ = DeleteObject(inner.into());
        // SetWindowRgn takes ownership of the region on success.
        if SetWindowRgn(window, Some(outer), true) == 0 {
            let _ = DeleteObject(outer.into());
        }
    }
}

fn is_trackable(window: HWND) -> bool {
    unsafe {
        IsWindow(Some(window)).as_bool()
            && IsWindowVisible(window).as_bool()
            && !IsIconic(window).as_bool()
    }
}

fn place_frame(target: HWND, frame: HWND, insert_after: HWND) {
    let bounds = window_bounds(target).filter(|_| is_trackable(target));
    let Some(bounds) = bounds else {
        unsafe {
            let _ = ShowWindow(frame, SW_HIDE);
        }
        return;
    };

    let metrics = highlight_metrics(target);
    let rect = highlight_frame(bounds, metrics.inset);
    let width = rect_width(&rect).max(0);
    let height = rect_height(&rect).max(0);
    let insert_after = unsafe {
        if IsWindow(Some(insert_after)).as_bool() {
            insert_after
        } else {
            HWND_TOPMOST
        }
    };

    unsafe {
        let _ = SetWindowPos(
            frame,
            Some(insert_after),
            rect.left,
            rect.top,
            width,
            height,
            SWP_NOACTIVATE,
        );
        apply_ring(frame, width, height, &metrics);
        let _ = ShowWindow(frame, SW_SHOWNOACTIVATE);
    }
}

unsafe extern "system" fn highlight_event(
    _hook: HWINEVENTHOOK,
    _event: u32,
    window: HWND,
    id_object: i32,
    id_child: i32,
    _thread: u32,
    _time: u32,
) {
    if id_object != OBJID_WINDOW.0 || id_child != CHILDID_SELF as i32 {
        return;
    }

    HIGHLIGHT.with(|state| {
        let state = state.borrow();
        let Some(highlight) = state.as_ref() else {
            return;
        };
        if highlight.target != window {
            return;
        }
        place_frame(highlight.target, highlight.frame, highlight.insert_after);
    });
}

fn create_frame_window() -> Result<HWND, OverlayWindowError> {
    let styles =
        WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED | WS_EX_TRANSPARENT;
    let window = create_popup_window(HIGHLIGHT_CLASS_NAME, styles, &RECT::default())
        .ok_or(OverlayWindowError::Create)?;

    let _ = disable_window_transitions(window);
    unsafe {
        let _ = SetLayeredWindowAttributes(window, COLORREF(0), u8::MAX, LWA_ALPHA);
        if SetWindowDisplayAffinity(window, WDA_EXCLUDEFROMCAPTURE).is_err() {
            let _ = DestroyWindow(window);
            return Err(OverlayWindowError::ContentProtection);
        }
    }

    Ok(window)
}

fn show_window_highlight(
    target: HWND,
    color: COLORREF,
    insert_after: Option<HWND>,
) -> Result<bool, OverlayWindowError> {
    teardown();
    HIGHLIGHT_COLOR.with(|current| *current.borrow_mut() = color);
    if !is_trackable(target) {
        return Ok(false);
    }
    ensure_window_class(HIGHLIGHT_CLASS_NAME, Some(highlight_wndproc), None);

    let frame = create_frame_window()?;
    let insert_after = insert_after.unwrap_or(HWND_TOPMOST);

    let mut process = 0u32;
    let thread = unsafe { GetWindowThreadProcessId(target, Some(&mut process)) };
    // Out-of-context hooks are delivered on this thread's message loop, which is
    // the same UI thread that owns the frame window.
    let hook = unsafe {
        SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_OBJECT_LOCATIONCHANGE,
            None,
            Some(highlight_event),
            process,
            thread,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        )
    };

    place_frame(target, frame, insert_after);
    HIGHLIGHT.with(|state| {
        *state.borrow_mut() = Some(WindowHighlight {
            target,
            frame,
            hook,
            insert_after,
        });
    });
    Ok(true)
}

fn destroy_all(windows: Vec<HWND>) {
    for window in windows {
        unsafe {
            let _ = DestroyWindow(window);
        }
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

    let _ = disable_window_transitions(window);
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
    destroy_all(WINDOWS.with(|windows| std::mem::take(&mut *windows.borrow_mut())));

    let highlight = HIGHLIGHT.with(|state| state.borrow_mut().take());
    let Some(highlight) = highlight else {
        return;
    };
    unsafe {
        let _ = UnhookWinEvent(highlight.hook);
        let _ = DestroyWindow(highlight.frame);
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
            "showWindow" => {
                let Some(handle) = param_i64(&request.params, "windowId") else {
                    return Reply::Now(Err((
                        "INVALID_PARAMS".to_string(),
                        "showWindow requires windowId".to_string(),
                    )));
                };

                let color = parse_color(param_str(&request.params, "color"));
                let insert_after_handle = param_i64(&request.params, "belowWindowId");
                let request_id = request.id.clone();
                let visible = self.visible.clone();
                run_on_ui(move || {
                    let target = HWND(handle as isize as *mut c_void);
                    let insert_after = insert_after_handle
                        .map(|handle| HWND(handle as isize as *mut c_void));
                    match show_window_highlight(target, color, insert_after) {
                        Ok(shown) => {
                            visible.store(shown, Ordering::Release);
                            respond_success(&request_id, json!({ "visible": shown }));
                        }
                        Err(OverlayWindowError::Create) => {
                            visible.store(false, Ordering::Release);
                            respond_error(
                                &request_id,
                                "WINDOW_ERROR",
                                "Failed to create recording highlight window",
                            );
                        }
                        Err(OverlayWindowError::ContentProtection) => {
                            visible.store(false, Ordering::Release);
                            respond_error(
                                &request_id,
                                "CONTENT_PROTECTION_ERROR",
                                "Failed to protect recording highlight window",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_theme_accent_into_a_colorref() {
        assert_eq!(parse_color(Some("#8892ef")).0, 0x00EF_9288);
        assert_eq!(parse_color(Some("5F6CD9")).0, 0x00D9_6C5F);
    }

    #[test]
    fn falls_back_when_the_accent_is_unusable() {
        assert_eq!(parse_color(None), HIGHLIGHT_FALLBACK);
        assert_eq!(parse_color(Some("#fff")), HIGHLIGHT_FALLBACK);
        assert_eq!(parse_color(Some("#zzzzzz")), HIGHLIGHT_FALLBACK);
    }

    #[test]
    fn frames_the_window_without_touching_it() {
        let bounds = RECT {
            left: 100,
            top: 200,
            right: 500,
            bottom: 600,
        };
        let frame = highlight_frame(bounds, 2);

        assert_eq!(frame.left, bounds.left - 2);
        assert_eq!(frame.top, bounds.top - 2);
        assert_eq!(frame.right, bounds.right + 2);
        assert_eq!(frame.bottom, bounds.bottom + 2);
    }

    #[test]
    fn frame_grows_by_the_inset_on_every_side() {
        let bounds = RECT {
            left: 0,
            top: 0,
            right: 40,
            bottom: 30,
        };
        let frame = highlight_frame(bounds, 3);

        assert_eq!(rect_width(&frame), rect_width(&bounds) + 6);
        assert_eq!(rect_height(&frame), rect_height(&bounds) + 6);
    }
}
