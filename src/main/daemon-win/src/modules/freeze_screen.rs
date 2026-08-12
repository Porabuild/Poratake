use crate::desktop_frame::{
    capture_monitors, clear_frozen, prewarm_capture, store_frozen, to_hbitmap,
};
use crate::overlay::{
    add_key_handler, create_popup_window, default_wndproc, disable_window_transitions,
    ensure_window_class, remove_key_handler,
};
use crate::protocol::{param_bool, respond_error, respond_success, Request};
use crate::router::{method_not_found, Module, Reply};
use crate::ui::run_on_ui;
use serde_json::json;
use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, CreateCompatibleDC, DeleteDC, DeleteObject, EndPaint, SelectObject,
    HBITMAP, PAINTSTRUCT, SRCCOPY,
};
use windows::Win32::UI::Input::KeyboardAndMouse::VK_SPACE;
use windows::Win32::UI::WindowsAndMessaging::{
    DestroyWindow, SetLayeredWindowAttributes, SetWindowPos, ShowWindow, HWND_TOPMOST, LWA_ALPHA,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SW_SHOWNOACTIVATE, WM_PAINT, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT,
};

const CLASS_NAME: &str = "CaptyFreezeOverlay";

struct FrozenWindow {
    window: HWND,
    bitmap: HBITMAP,
    width: i32,
    height: i32,
}

struct FreezeUiState {
    windows: Vec<FrozenWindow>,
    key_token: Option<usize>,
    frozen_flag: Option<Arc<AtomicBool>>,
}

thread_local! {
    static STATE: RefCell<FreezeUiState> = RefCell::new(FreezeUiState {
        windows: Vec::new(),
        key_token: None,
        frozen_flag: None,
    });
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
    let entry = STATE.with(|state| {
        state
            .borrow()
            .windows
            .iter()
            .find(|item| item.window == window)
            .map(|item| (item.bitmap, item.width, item.height))
    });

    unsafe {
        let mut paint_struct = PAINTSTRUCT::default();
        let dc = BeginPaint(window, &mut paint_struct);

        if let Some((bitmap, width, height)) = entry {
            let memory_dc = CreateCompatibleDC(Some(dc));
            let previous = SelectObject(memory_dc, bitmap.into());
            let _ = BitBlt(dc, 0, 0, width, height, Some(memory_dc), 0, 0, SRCCOPY);
            SelectObject(memory_dc, previous);
            let _ = DeleteDC(memory_dc);
        }

        let _ = EndPaint(window, &paint_struct);
    }
}

fn create_overlays() {
    teardown();

    ensure_window_class(CLASS_NAME, Some(wndproc), None);

    let frames = capture_monitors();

    for frame in &frames {
        let Some(bitmap) = to_hbitmap(frame) else {
            continue;
        };

        let ex_style =
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED | WS_EX_TRANSPARENT;

        let Some(window) = create_popup_window(CLASS_NAME, ex_style, &frame.bounds) else {
            unsafe {
                let _ = DeleteObject(bitmap.into());
            }
            continue;
        };

        let _ = disable_window_transitions(window);

        unsafe {
            let _ = SetLayeredWindowAttributes(window, COLORREF(0), 255, LWA_ALPHA);
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

        STATE.with(|state| {
            state.borrow_mut().windows.push(FrozenWindow {
                window,
                bitmap,
                width: frame.width as i32,
                height: frame.height as i32,
            });
        });
    }

    store_frozen(frames);
}

fn teardown() {
    clear_frozen();

    let (windows, key_token) = STATE.with(|state| {
        let mut state = state.borrow_mut();
        (std::mem::take(&mut state.windows), state.key_token.take())
    });

    for entry in windows {
        unsafe {
            let _ = DestroyWindow(entry.window);
            let _ = DeleteObject(entry.bitmap.into());
        }
    }

    if let Some(token) = key_token {
        remove_key_handler(token);
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
        "freeze-screen"
    }

    fn handle(&mut self, request: &Request) -> Reply {
        match request.method.as_str() {
            "freeze" => {
                let watch_space_key = param_bool(&request.params, "watchSpaceKey").unwrap_or(false);
                let frozen = self.frozen.clone();
                let request_id = request.id.clone();

                run_on_ui(move || {
                    create_overlays();
                    if watch_space_key {
                        if let Err(message) = start_space_key_watch(frozen.clone()) {
                            teardown();
                            frozen.store(false, Ordering::SeqCst);
                            respond_error(&request_id, "UI_ERROR", &message);
                            return;
                        }
                    }
                    frozen.store(true, Ordering::SeqCst);
                    respond_success(&request_id, json!({ "frozen": true }));
                });

                Reply::Deferred
            }
            "prewarm" => {
                prewarm_capture();
                Reply::Now(Ok(Some(json!({ "prewarmed": true }))))
            }
            "release" => {
                let frozen = self.frozen.clone();
                let request_id = request.id.clone();

                run_on_ui(move || {
                    teardown();
                    frozen.store(false, Ordering::SeqCst);
                    respond_success(&request_id, json!({ "frozen": false }));
                });

                Reply::Deferred
            }
            "status" => Reply::Now(Ok(Some(
                json!({ "frozen": self.frozen.load(Ordering::SeqCst) }),
            ))),
            method => method_not_found(method),
        }
    }
}
