use crate::desktop_frame::{
    DesktopFrame, clear_frozen, prewarm_capture, reclaim_idle_memory, store_frozen,
    stream_capture_monitors,
};
use crate::overlay::{
    add_key_handler, bitmap_info, create_popup_window, default_wndproc, disable_window_transitions,
    ensure_window_class, remove_key_handler, show_window_topmost,
};
use crate::protocol::{Request, param_bool, respond_error, respond_success};
use crate::router::{Module, Reply, method_not_found};
use crate::ui::run_on_ui;
use poratake_daemon_common::contract::{FREEZE_SCREEN_MODULE, FreezeScreenMethod};
use serde_json::json;
use std::cell::RefCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, DIB_RGB_COLORS, EndPaint, MONITOR_DEFAULTTONEAREST, MonitorFromPoint, PAINTSTRUCT,
    SetDIBitsToDevice,
};
use windows::Win32::UI::Input::KeyboardAndMouse::VK_SPACE;
use windows::Win32::UI::WindowsAndMessaging::{
    DestroyWindow, GetCursorPos, LWA_ALPHA, SetLayeredWindowAttributes, WM_PAINT, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT,
};

const CLASS_NAME: &str = "PoratakeFreezeOverlay";
const FREEZE_RECEIVE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

struct FrozenWindow {
    window: HWND,
    frame: Arc<DesktopFrame>,
}

struct FreezeUiState {
    windows: Vec<FrozenWindow>,
    key_token: Option<usize>,
    frozen_flag: Option<Arc<AtomicBool>>,
}

thread_local! {
    static STATE: RefCell<FreezeUiState> = const { RefCell::new(FreezeUiState {
        windows: Vec::new(),
        key_token: None,
        frozen_flag: None,
    }) };
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
    let frame = STATE.with(|state| {
        state
            .borrow()
            .windows
            .iter()
            .find(|item| item.window == window)
            .map(|item| Arc::clone(&item.frame))
    });

    unsafe {
        let mut paint_struct = PAINTSTRUCT::default();
        let dc = BeginPaint(window, &mut paint_struct);

        if let Some(frame) = frame {
            let info = bitmap_info(
                frame.width as i32,
                frame.height as i32,
                frame.pixels.len() as u32,
            );
            let _ = SetDIBitsToDevice(
                dc,
                0,
                0,
                frame.width,
                frame.height,
                0,
                0,
                0,
                frame.height,
                frame.pixels.as_ptr().cast(),
                &info,
                DIB_RGB_COLORS,
            );
        }

        let _ = EndPaint(window, &paint_struct);
    }
}

fn cursor_monitor_handle() -> Option<isize> {
    let mut point = POINT::default();
    unsafe { GetCursorPos(&mut point).ok()? };
    let monitor = unsafe { MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST) };
    Some(monitor.0 as isize)
}

fn is_frozen(captured_frames: usize) -> bool {
    captured_frames > 0
}

fn create_overlays() -> bool {
    teardown();

    ensure_window_class(CLASS_NAME, Some(wndproc), None);

    let started = std::time::Instant::now();
    let frames = stream_capture_monitors(cursor_monitor_handle());
    let deadline = started + FREEZE_RECEIVE_TIMEOUT;
    let mut stored = Vec::new();
    let mut captured_at = started.elapsed();

    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        let Ok(frame) = frames.recv_timeout(remaining) else {
            break;
        };

        let frame = Arc::new(frame);
        let ex_style =
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED | WS_EX_TRANSPARENT;

        let Some(window) = create_popup_window(CLASS_NAME, ex_style, &frame.bounds) else {
            continue;
        };

        let _ = disable_window_transitions(window);

        unsafe {
            let _ = SetLayeredWindowAttributes(window, COLORREF(0), 255, LWA_ALPHA);
        }

        STATE.with(|state| {
            state.borrow_mut().windows.push(FrozenWindow {
                window,
                frame: Arc::clone(&frame),
            });
        });

        show_window_topmost(window);

        stored.push(frame);
        captured_at = started.elapsed();
    }

    let frozen = is_frozen(stored.len());
    store_frozen(stored);

    if crate::desktop_frame::capture_timing_enabled() {
        let total = started.elapsed();
        eprintln!(
            "[freeze-timing] capture={:?} present={:?} total={:?}",
            captured_at,
            total - captured_at,
            total
        );
    }

    frozen
}

fn teardown() {
    let (windows, key_token) = STATE.with(|state| {
        let mut state = state.borrow_mut();
        (std::mem::take(&mut state.windows), state.key_token.take())
    });

    let had_windows = !windows.is_empty();
    for entry in windows {
        unsafe {
            let _ = DestroyWindow(entry.window);
        }
    }

    clear_frozen();

    if let Some(token) = key_token {
        remove_key_handler(token);
    }

    if had_windows {
        reclaim_idle_memory();
    }
}

fn start_space_key_watch(frozen: Arc<AtomicBool>) -> Result<(), String> {
    STATE.with(|state| {
        state.borrow_mut().frozen_flag = Some(frozen);
    });

    let token = add_key_handler(VK_SPACE.0 as u32, || {
        let flag = STATE.with(|state| state.borrow().frozen_flag.clone());
        teardown();
        if let Some(flag) = flag {
            flag.store(false, Ordering::SeqCst);
        }
    })?;

    STATE.with(|state| {
        state.borrow_mut().key_token = Some(token);
    });

    Ok(())
}

pub struct FreezeScreenModule {
    frozen: Arc<AtomicBool>,
}

impl FreezeScreenModule {
    pub fn new() -> Self {
        FreezeScreenModule {
            frozen: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl Module for FreezeScreenModule {
    fn name(&self) -> &'static str {
        FREEZE_SCREEN_MODULE
    }

    fn handle(&mut self, request: &Request) -> Reply {
        match FreezeScreenMethod::parse(&request.method) {
            Some(FreezeScreenMethod::Freeze) => {
                let watch_space_key = param_bool(&request.params, "watchSpaceKey").unwrap_or(false);
                let frozen = self.frozen.clone();
                let request_id = request.id.clone();

                run_on_ui(move || {
                    let is_frozen = create_overlays();
                    if watch_space_key
                        && is_frozen
                        && let Err(message) = start_space_key_watch(frozen.clone())
                    {
                        teardown();
                        frozen.store(false, Ordering::SeqCst);
                        respond_error(&request_id, "UI_ERROR", &message);
                        return;
                    }
                    frozen.store(is_frozen, Ordering::SeqCst);
                    respond_success(&request_id, json!({ "frozen": is_frozen }));
                });

                Reply::Deferred
            }
            Some(FreezeScreenMethod::Prewarm) => {
                prewarm_capture();
                Reply::Now(Ok(Some(json!({ "prewarmed": true }))))
            }
            Some(FreezeScreenMethod::Release) => {
                let frozen = self.frozen.clone();
                let request_id = request.id.clone();

                run_on_ui(move || {
                    teardown();
                    frozen.store(false, Ordering::SeqCst);
                    respond_success(&request_id, json!({ "frozen": false }));
                });

                Reply::Deferred
            }
            None => method_not_found(&request.method),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_capture_is_not_frozen() {
        assert!(!is_frozen(0));
    }

    #[test]
    fn captured_frame_is_frozen() {
        assert!(is_frozen(1));
    }
}
