use crate::overlay::{
    add_key_handler, create_popup_window, default_wndproc, ensure_window_class, rect_height,
    rect_width, remove_key_handler, WM_MOUSELEAVE,
};
use crate::protocol::{respond_error, respond_success, Request};
use crate::router::{method_not_found, Module, Reply};
use crate::ui::run_on_ui;
use serde_json::json;
use std::cell::RefCell;
use std::sync::{Arc, Mutex};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DwmGetWindowAttribute, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS,
};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CombineRgn, CreateRectRgn, CreateSolidBrush, DeleteObject, EndPaint, FillRect,
    SetWindowRgn, PAINTSTRUCT, RGN_DIFF,
};
use windows::Win32::System::Threading::{
    GetCurrentProcessId, OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT, VK_ESCAPE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DestroyWindow, EnumWindows, GetClassNameW, GetSystemMetrics, GetWindowLongW, GetWindowTextW,
    GetWindowThreadProcessId, IsIconic, IsWindowVisible, LoadCursorW, SetLayeredWindowAttributes,
    SetWindowPos, ShowWindow, GWL_EXSTYLE, HWND_TOPMOST, IDC_ARROW, LWA_ALPHA, SM_CXVIRTUALSCREEN,
    SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOSIZE, SW_SHOWNOACTIVATE, WM_LBUTTONDOWN, WM_MOUSEMOVE, WM_PAINT, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT,
};

const OVERLAY_CLASS: &str = "CaptyWindowSelector";
const DIM_CLASS: &str = "CaptyWindowSelectorDim";
const IDLE_ALPHA: u8 = 1;
const DIM_ALPHA: u8 = 160;
const MIN_WINDOW_SIZE: i32 = 50;

const EXCLUDED_CLASSES: [&str; 4] = [
    "Progman",
    "WorkerW",
    "Shell_TrayWnd",
    "Shell_SecondaryTrayWnd",
];

type PendingRequest = Arc<Mutex<Option<String>>>;

#[derive(Clone)]
struct TargetWindow {
    window_id: isize,
    title: String,
    owner_name: String,
    owner_pid: u32,
    rect: RECT,
}

struct OverlayEntry {
    window: HWND,
    target: TargetWindow,
    hovered: bool,
}

struct SelectorUiState {
    overlays: Vec<OverlayEntry>,
    dim_window: Option<HWND>,
    key_token: Option<usize>,
    pending: Option<PendingRequest>,
}

thread_local! {
    static STATE: RefCell<SelectorUiState> = RefCell::new(SelectorUiState {
        overlays: Vec::new(),
        dim_window: None,
        key_token: None,
        pending: None,
    });
}

fn take_pending_id() -> Option<String> {
    let pending = STATE.with(|state| state.borrow().pending.clone());
    pending?.lock().ok()?.take()
}

fn window_process_name(pid: u32) -> String {
    unsafe {
        let Ok(process) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return String::from("Unknown");
        };

        let mut buffer = [0u16; 1024];
        let mut length = buffer.len() as u32;
        let result = QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_FORMAT(0),
            windows::core::PWSTR(buffer.as_mut_ptr()),
            &mut length,
        );
        let _ = windows::Win32::Foundation::CloseHandle(process);

        if result.is_err() {
            return String::from("Unknown");
        }

        let full_path = String::from_utf16_lossy(&buffer[..length as usize]);
        std::path::Path::new(&full_path)
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .unwrap_or_else(|| String::from("Unknown"))
    }
}

fn window_class_name(window: HWND) -> String {
    let mut buffer = [0u16; 256];
    let length = unsafe { GetClassNameW(window, &mut buffer) };
    if length <= 0 {
        return String::new();
    }
    String::from_utf16_lossy(&buffer[..length as usize])
}

fn window_title(window: HWND) -> String {
    let mut buffer = [0u16; 512];
    let length = unsafe { GetWindowTextW(window, &mut buffer) };
    if length <= 0 {
        return String::new();
    }
    String::from_utf16_lossy(&buffer[..length as usize])
}

fn is_cloaked(window: HWND) -> bool {
    let mut cloaked: u32 = 0;
    let result = unsafe {
        DwmGetWindowAttribute(
            window,
            DWMWA_CLOAKED,
            &mut cloaked as *mut u32 as *mut _,
            std::mem::size_of::<u32>() as u32,
        )
    };
    result.is_ok() && cloaked != 0
}

fn window_bounds(window: HWND) -> Option<RECT> {
    let mut rect = RECT::default();
    let result = unsafe {
        DwmGetWindowAttribute(
            window,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut rect as *mut RECT as *mut _,
            std::mem::size_of::<RECT>() as u32,
        )
    };
    if result.is_err() {
        return None;
    }
    Some(rect)
}

fn collect_target_windows() -> Vec<TargetWindow> {
    let mut targets: Vec<TargetWindow> = Vec::new();

    unsafe extern "system" fn enum_proc(window: HWND, lparam: LPARAM) -> windows::core::BOOL {
        let targets = unsafe { &mut *(lparam.0 as *mut Vec<TargetWindow>) };
        let keep_enumerating = windows::core::BOOL(1);

        if !unsafe { IsWindowVisible(window) }.as_bool() {
            return keep_enumerating;
        }
        if unsafe { IsIconic(window) }.as_bool() {
            return keep_enumerating;
        }

        let ex_style = unsafe { GetWindowLongW(window, GWL_EXSTYLE) } as u32;
        if ex_style & WS_EX_TOOLWINDOW.0 != 0 {
            return keep_enumerating;
        }

        if is_cloaked(window) {
            return keep_enumerating;
        }

        let class_name = window_class_name(window);
        if EXCLUDED_CLASSES.contains(&class_name.as_str()) {
            return keep_enumerating;
        }

        let Some(rect) = window_bounds(window) else {
            return keep_enumerating;
        };
        if rect_width(&rect) < MIN_WINDOW_SIZE || rect_height(&rect) < MIN_WINDOW_SIZE {
            return keep_enumerating;
        }

        let mut pid: u32 = 0;
        unsafe {
            GetWindowThreadProcessId(window, Some(&mut pid));
        }
        if pid == 0 || pid == unsafe { GetCurrentProcessId() } {
            return keep_enumerating;
        }

        let owner_name = window_process_name(pid);
        let raw_title = window_title(window);
        let title = if raw_title.is_empty() {
            owner_name.clone()
        } else {
            raw_title
        };

        targets.push(TargetWindow {
            window_id: window.0 as isize,
            title,
            owner_name,
            owner_pid: pid,
            rect,
        });

        keep_enumerating
    }

    unsafe {
        let _ = EnumWindows(Some(enum_proc), LPARAM(&mut targets as *mut _ as isize));
    }

    targets
}

unsafe extern "system" fn dim_wndproc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_PAINT {
        paint_solid(window, COLORREF(0));
        return LRESULT(0);
    }
    default_wndproc(window, message, wparam, lparam)
}

unsafe extern "system" fn overlay_wndproc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_PAINT => {
            paint_solid(window, COLORREF(0));
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

fn paint_solid(window: HWND, color: COLORREF) {
    unsafe {
        let mut paint_struct = PAINTSTRUCT::default();
        let dc = BeginPaint(window, &mut paint_struct);
        let brush = CreateSolidBrush(color);
        FillRect(dc, &paint_struct.rcPaint, brush);
        let _ = DeleteObject(brush.into());
        let _ = EndPaint(window, &paint_struct);
    }
}

fn handle_hover(window: HWND, hovered: bool) {
    let (changed, target_rect) = STATE.with(|state| {
        let mut state = state.borrow_mut();
        let Some(index) = state
            .overlays
            .iter()
            .position(|entry| entry.window == window)
        else {
            return (false, None);
        };

        if state.overlays[index].hovered == hovered {
            return (false, None);
        }

        state.overlays[index].hovered = hovered;
        let target_rect = if hovered {
            Some(state.overlays[index].target.rect)
        } else {
            state
                .overlays
                .iter()
                .find(|entry| entry.hovered)
                .map(|entry| entry.target.rect)
        };
        (true, target_rect)
    });

    if !changed {
        return;
    }

    update_dim_cutout(target_rect);

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

fn update_dim_cutout(target: Option<RECT>) {
    let dim_window = STATE.with(|state| state.borrow().dim_window);
    let Some(dim_window) = dim_window else {
        return;
    };

    let Some(target) = target else {
        unsafe {
            let _ = SetWindowRgn(dim_window, None, true);
        }
        return;
    };

    let virtual_x = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
    let virtual_y = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };
    let virtual_width = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) };
    let virtual_height = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) };

    unsafe {
        let dim_region = CreateRectRgn(0, 0, virtual_width, virtual_height);
        let target_region = CreateRectRgn(
            target.left - virtual_x,
            target.top - virtual_y,
            target.right - virtual_x,
            target.bottom - virtual_y,
        );
        CombineRgn(
            Some(dim_region),
            Some(dim_region),
            Some(target_region),
            RGN_DIFF,
        );
        let _ = DeleteObject(target_region.into());
        let _ = SetWindowRgn(dim_window, Some(dim_region), true);
    }
}

fn handle_selection(window: HWND) {
    let target = STATE.with(|state| {
        state
            .borrow()
            .overlays
            .iter()
            .find(|entry| entry.window == window)
            .map(|entry| entry.target.clone())
    });

    let Some(target) = target else {
        return;
    };

    if let Some(request_id) = take_pending_id() {
        respond_success(
            &request_id,
            json!({
                "status": "selected",
                "windowId": target.window_id as i64,
                "windowTitle": target.title,
                "ownerName": target.owner_name,
                "ownerPid": target.owner_pid,
                "bounds": {
                    "x": target.rect.left,
                    "y": target.rect.top,
                    "width": rect_width(&target.rect),
                    "height": rect_height(&target.rect),
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

fn create_dim_overlay() {
    ensure_window_class(DIM_CLASS, Some(dim_wndproc), None);

    let rect = RECT {
        left: unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) },
        top: unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) },
        right: unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) }
            + unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) },
        bottom: unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) }
            + unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) },
    };

    let ex_style =
        WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED | WS_EX_TRANSPARENT;

    let Some(window) = create_popup_window(DIM_CLASS, ex_style, &rect) else {
        return;
    };

    unsafe {
        let _ = SetLayeredWindowAttributes(window, COLORREF(0), DIM_ALPHA, LWA_ALPHA);
        let _ = ShowWindow(window, SW_SHOWNOACTIVATE);
    }

    STATE.with(|state| {
        state.borrow_mut().dim_window = Some(window);
    });
}

fn create_window_overlays(targets: Vec<TargetWindow>) {
    let default_cursor = unsafe { LoadCursorW(None, IDC_ARROW) }.ok();
    ensure_window_class(OVERLAY_CLASS, Some(overlay_wndproc), default_cursor);

    let ex_style = WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED;

    for target in targets.into_iter().rev() {
        let Some(window) = create_popup_window(OVERLAY_CLASS, ex_style, &target.rect) else {
            continue;
        };

        unsafe {
            let _ = SetLayeredWindowAttributes(window, COLORREF(0), IDLE_ALPHA, LWA_ALPHA);
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
            state.borrow_mut().overlays.push(OverlayEntry {
                window,
                target,
                hovered: false,
            });
        });
    }
}

fn start_selection(pending: PendingRequest) {
    let targets = collect_target_windows();

    if targets.is_empty() {
        let request_id = pending.lock().ok().and_then(|mut value| value.take());
        if let Some(request_id) = request_id {
            respond_error(&request_id, "NO_WINDOWS", "No visible windows found");
        }
        return;
    }

    STATE.with(|state| {
        state.borrow_mut().pending = Some(pending);
    });

    create_dim_overlay();
    create_window_overlays(targets);

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

fn teardown() {
    let (overlays, dim_window, key_token) = STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.pending = None;
        (
            std::mem::take(&mut state.overlays),
            state.dim_window.take(),
            state.key_token.take(),
        )
    });

    for entry in overlays {
        unsafe {
            let _ = DestroyWindow(entry.window);
        }
    }

    if let Some(window) = dim_window {
        unsafe {
            let _ = DestroyWindow(window);
        }
    }

    if let Some(token) = key_token {
        remove_key_handler(token);
    }
}

pub struct WindowSelectorModule {
    pending: PendingRequest,
}

impl WindowSelectorModule {
    pub fn new() -> Self {
        WindowSelectorModule {
            pending: Arc::new(Mutex::new(None)),
        }
    }
}

impl Module for WindowSelectorModule {
    fn name(&self) -> &'static str {
        "window-selector"
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
                            "Window selector is already active".to_string(),
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
