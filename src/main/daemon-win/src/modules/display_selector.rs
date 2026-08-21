use crate::overlay::{
    WM_MOUSELEAVE, add_key_handler, create_popup_window, default_wndproc, ensure_window_class,
    monitors, rect_height, rect_width, remove_key_handler, show_window_topmost,
};
use crate::protocol::{Request, respond_error, respond_success};
use crate::router::{Module, Reply, method_not_found};
use crate::ui::run_on_ui;
use serde_json::json;
use std::cell::RefCell;
use std::sync::{Arc, Mutex};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateSolidBrush, DeleteObject, EndPaint, FillRect, PAINTSTRUCT,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent, VK_ESCAPE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DestroyWindow, LWA_ALPHA, SetLayeredWindowAttributes, WM_LBUTTONDOWN, WM_MOUSEMOVE, WM_PAINT,
    WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
};

const CLASS_NAME: &str = "PoratakeDisplaySelector";
const IDLE_ALPHA: u8 = 128;
const HOVER_ALPHA: u8 = 1;

type PendingRequest = Arc<Mutex<Option<String>>>;

struct DisplayEntry {
    window: HWND,
    display_number: i32,
    screen_id: i32,
    rect: RECT,
    hovered: bool,
}

struct SelectorUiState {
    entries: Vec<DisplayEntry>,
    key_token: Option<usize>,
    pending: Option<PendingRequest>,
}

thread_local! {
    static STATE: RefCell<SelectorUiState> = RefCell::new(SelectorUiState {
        entries: Vec::new(),
        key_token: None,
        pending: None,
    });
}

fn take_pending_id() -> Option<String> {
    let pending = STATE.with(|state| state.borrow().pending.clone());
    let id = pending?.lock().ok()?.take();
    id
}

unsafe extern "system" fn wndproc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_PAINT => {
            paint(window);
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            handle_hover(window, true);
            LRESULT(0)
        }
        WM_MOUSELEAVE => {
            handle_hover(window, false);
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            handle_selection(window);
            LRESULT(0)
        }
        _ => default_wndproc(window, message, wparam, lparam),
    }
}

fn paint(window: HWND) {
    unsafe {
        let mut paint_struct = PAINTSTRUCT::default();
        let dc = BeginPaint(window, &mut paint_struct);
        let black = CreateSolidBrush(COLORREF(0));
        FillRect(dc, &paint_struct.rcPaint, black);
        let _ = DeleteObject(black.into());
        let _ = EndPaint(window, &paint_struct);
    }
}

fn handle_hover(window: HWND, hovered: bool) {
    let changed = STATE.with(|state| {
        let mut state = state.borrow_mut();
        let Some(entry) = state
            .entries
            .iter_mut()
            .find(|entry| entry.window == window)
        else {
            return false;
        };

        if entry.hovered == hovered {
            return false;
        }

        entry.hovered = hovered;
        true
    });

    if !changed {
        return;
    }

    let alpha = if hovered { HOVER_ALPHA } else { IDLE_ALPHA };
    unsafe {
        let _ = SetLayeredWindowAttributes(window, COLORREF(0), alpha, LWA_ALPHA);
    }

    if hovered {
        let mut track = TRACKMOUSEEVENT {
            cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
            dwFlags: TME_LEAVE,
            hwndTrack: window,
            dwHoverTime: 0,
        };
        unsafe {
            let _ = TrackMouseEvent(&mut track);
        }
    }
}

fn handle_selection(window: HWND) {
    let selected = STATE.with(|state| {
        state
            .borrow()
            .entries
            .iter()
            .find(|entry| entry.window == window)
            .map(|entry| (entry.display_number, entry.screen_id, entry.rect))
    });

    let Some((display_number, screen_id, rect)) = selected else {
        return;
    };

    if let Some(request_id) = take_pending_id() {
        respond_success(
            &request_id,
            json!({
                "status": "selected",
                "displayNumber": display_number,
                "screenId": screen_id,
                "bounds": {
                    "x": rect.left,
                    "y": rect.top,
                    "width": rect_width(&rect),
                    "height": rect_height(&rect),
                },
            }),
        );
    }

    teardown();
}

fn handle_cancellation() {
    if let Some(request_id) = take_pending_id() {
        respond_success(&request_id, json!({ "status": "cancelled" }));
    }
    teardown();
}

fn start_selection(pending: PendingRequest) {
    ensure_window_class(CLASS_NAME, Some(wndproc), None);

    STATE.with(|state| {
        state.borrow_mut().pending = Some(pending);
    });

    let all = monitors();
    let mut display_number = 1;

    for monitor in all.iter().filter(|item| item.is_primary) {
        create_display_overlay(monitor.rect, 1, monitor.device_number);
    }

    for monitor in all.iter().filter(|item| !item.is_primary) {
        display_number += 1;
        create_display_overlay(monitor.rect, display_number, monitor.device_number);
    }

    let token = match add_key_handler(VK_ESCAPE.0 as u32, handle_cancellation) {
        Ok(token) => token,
        Err(message) => {
            if let Some(request_id) = take_pending_id() {
                respond_error(&request_id, "UI_ERROR", &message);
            }
            teardown();
            return;
        }
    };
    STATE.with(|state| {
        state.borrow_mut().key_token = Some(token);
    });
}

fn create_display_overlay(rect: RECT, display_number: i32, screen_id: i32) {
    let ex_style = WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED;
    let Some(window) = create_popup_window(CLASS_NAME, ex_style, &rect) else {
        return;
    };

    unsafe {
        let _ = SetLayeredWindowAttributes(window, COLORREF(0), IDLE_ALPHA, LWA_ALPHA);
    }
    show_window_topmost(window);

    STATE.with(|state| {
        state.borrow_mut().entries.push(DisplayEntry {
            window,
            display_number,
            screen_id,
            rect,
            hovered: false,
        });
    });
}

fn teardown() {
    let (entries, key_token) = STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.pending = None;
        (std::mem::take(&mut state.entries), state.key_token.take())
    });

    for entry in entries {
        unsafe {
            let _ = DestroyWindow(entry.window);
        }
    }

    if let Some(token) = key_token {
        remove_key_handler(token);
    }
}

pub struct DisplaySelectorModule {
    pending: PendingRequest,
}

impl DisplaySelectorModule {
    pub fn new() -> Self {
        DisplaySelectorModule {
            pending: Arc::new(Mutex::new(None)),
        }
    }
}

impl Module for DisplaySelectorModule {
    fn name(&self) -> &'static str {
        "display-selector"
    }

    fn handle(&mut self, request: &Request) -> Reply {
        match request.method.as_str() {
            "select" => {
                {
                    let Ok(mut pending) = self.pending.lock() else {
                        respond_error(&request.id, "INTERNAL_ERROR", "Selector state poisoned");
                        return Reply::Deferred;
                    };

                    if pending.is_some() {
                        return Reply::Now(Err((
                            "ALREADY_ACTIVE".to_string(),
                            "Display selector is already active".to_string(),
                        )));
                    }

                    *pending = Some(request.id.clone());
                }

                let pending = self.pending.clone();
                run_on_ui(move || start_selection(pending));

                Reply::Deferred
            }
            "cancel" => {
                if let Ok(mut pending) = self.pending.lock() {
                    pending.take();
                }
                run_on_ui(teardown);
                Reply::Now(Ok(Some(json!({ "cancelled": true }))))
            }
            method => method_not_found(method),
        }
    }
}
