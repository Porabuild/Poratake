use crate::overlay::{
    WM_MOUSELEAVE, add_key_handler, create_popup_window, default_wndproc, ensure_window_class,
    monitors, remove_key_handler, to_wide,
};
use crate::protocol::{Request, param_bool, param_i32, respond_error, respond_success, send_event};
use crate::router::{Module, Reply, method_not_found};
use crate::ui::run_on_ui;
use serde_json::json;
use std::cell::RefCell;
use std::sync::{Arc, Mutex};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateFontW, CreateRoundRectRgn, CreateSolidBrush, DT_CENTER, DT_SINGLELINE,
    DT_VCENTER, DeleteObject, DrawTextW, EndPaint, FONT_CHARSET, FONT_CLIP_PRECISION,
    FONT_OUTPUT_PRECISION, FONT_QUALITY, FW_NORMAL, FillRect, FrameRect, HFONT, InvalidateRect,
    PAINTSTRUCT, ScreenToClient, SelectObject, SetBkMode, SetTextColor, SetWindowRgn, TRANSPARENT,
};
use windows::Win32::System::Diagnostics::Debug::MessageBeep;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{
    GetDpiForWindow, LogicalToPhysicalPointForPerMonitorDPI, PhysicalToLogicalPointForPerMonitorDPI,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SetFocus, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent, VK_ESCAPE, VK_RETURN,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, BN_CLICKED, BS_DEFPUSHBUTTON, CreatePopupMenu, CreateWindowExW, DestroyMenu,
    DestroyWindow, ES_NUMBER, ES_RIGHT, GetClientRect, GetWindowRect, GetWindowTextW, HMENU,
    HTCAPTION, HTCLIENT, HWND_TOPMOST, LWA_ALPHA, MB_ICONWARNING, MF_CHECKED, MF_STRING, SW_SHOW,
    SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SetForegroundWindow, SetLayeredWindowAttributes,
    SetWindowPos, SetWindowTextW, ShowWindow, TPM_LEFTALIGN, TPM_NONOTIFY, TPM_RETURNCMD,
    TPM_TOPALIGN, TrackPopupMenu, WINDOW_EX_STYLE, WINDOW_STYLE, WM_ACTIVATE, WM_CLOSE, WM_COMMAND,
    WM_DPICHANGED, WM_ERASEBKGND, WM_LBUTTONDOWN, WM_MOUSEMOVE, WM_NCHITTEST, WM_PAINT, WM_SETFONT,
    WS_BORDER, WS_CHILD, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
    WS_TABSTOP, WS_VISIBLE,
};
use windows::core::PCWSTR;

const PANEL_CLASS_NAME: &str = "PoratakeAllInOnePanel";
const SIZE_EDITOR_CLASS_NAME: &str = "PoratakeAllInOneSizeEditor";
const PANEL_WIDTH: i32 = 288;
const PANEL_WIDTH_WITHOUT_RECORDING: i32 = 240;
const PANEL_HEIGHT: i32 = 48;
const SIZE_EDITOR_WIDTH: i32 = 220;
const SIZE_EDITOR_HEIGHT: i32 = 88;
const BUTTON_WIDTH: i32 = 48;
const CORNER_RADIUS: i32 = 8;
const WIDTH_EDIT_ID: usize = 1001;
const HEIGHT_EDIT_ID: usize = 1002;
const APPLY_BUTTON_ID: usize = 1003;
const MIN_SIZE: i32 = 20;
const MAX_SIZE: i32 = 100000;
const EM_SETSEL_MESSAGE: u32 = 0x00B1;
const WA_INACTIVE_VALUE: usize = 0;

#[derive(Clone, Copy, PartialEq, Eq)]
struct AspectRatio {
    name: &'static str,
    width: i32,
    height: i32,
}

const ASPECT_RATIOS: [AspectRatio; 8] = [
    AspectRatio {
        name: "Free",
        width: 0,
        height: 0,
    },
    AspectRatio {
        name: "16:9",
        width: 16,
        height: 9,
    },
    AspectRatio {
        name: "9:16",
        width: 9,
        height: 16,
    },
    AspectRatio {
        name: "4:3",
        width: 4,
        height: 3,
    },
    AspectRatio {
        name: "1:1",
        width: 1,
        height: 1,
    },
    AspectRatio {
        name: "21:9",
        width: 21,
        height: 9,
    },
    AspectRatio {
        name: "4:5",
        width: 4,
        height: 5,
    },
    AspectRatio {
        name: "3:2",
        width: 3,
        height: 2,
    },
];

type SharedVisibility = Arc<Mutex<bool>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PanelButton {
    Close,
    AspectRatio,
    Size,
    Screenshot,
    Record,
    Drag,
}

struct SizeEditorState {
    window: HWND,
    width_edit: HWND,
    height_edit: HWND,
}

struct AllInOneUiState {
    panel: Option<HWND>,
    size_editor: Option<SizeEditorState>,
    font: Option<HFONT>,
    escape_key_token: Option<usize>,
    return_key_token: Option<usize>,
    aspect_ratio: AspectRatio,
    selection_width: i32,
    selection_height: i32,
    recording_enabled: bool,
    hovered: Option<PanelButton>,
    shared_visibility: Option<SharedVisibility>,
}

thread_local! {
    static STATE: RefCell<AllInOneUiState> = const { RefCell::new(AllInOneUiState {
        panel: None,
        size_editor: None,
        font: None,
        escape_key_token: None,
        return_key_token: None,
        aspect_ratio: ASPECT_RATIOS[0],
        selection_width: 0,
        selection_height: 0,
        recording_enabled: true,
        hovered: None,
        shared_visibility: None,
    }) };
}

unsafe extern "system" fn panel_wndproc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_PAINT => {
            paint_panel(window);
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_MOUSEMOVE => {
            handle_panel_hover(window, point_from_lparam(lparam));
            LRESULT(0)
        }
        WM_MOUSELEAVE => {
            clear_panel_hover(window);
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            handle_panel_click(window, point_from_lparam(lparam));
            LRESULT(0)
        }
        WM_NCHITTEST => handle_panel_hit_test(window, lparam),
        WM_DPICHANGED => {
            apply_panel_dpi_rect(window, wparam, lparam);
            refresh_panel_resources(window);
            LRESULT(0)
        }
        WM_CLOSE => {
            emit_close();
            LRESULT(0)
        }
        _ => default_wndproc(window, message, wparam, lparam),
    }
}

unsafe extern "system" fn size_editor_wndproc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_PAINT => {
            paint_size_editor(window);
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_COMMAND => {
            let identifier = wparam.0 & 0xffff;
            let notification = (wparam.0 >> 16) as u32;
            if identifier == APPLY_BUTTON_ID && notification == BN_CLICKED {
                apply_size();
            }
            LRESULT(0)
        }
        WM_ACTIVATE => {
            handle_size_editor_activation(wparam, lparam);
            LRESULT(0)
        }
        WM_DPICHANGED => {
            apply_suggested_rect(window, lparam);
            layout_size_editor(window);
            LRESULT(0)
        }
        WM_CLOSE => {
            close_size_editor();
            LRESULT(0)
        }
        _ => default_wndproc(window, message, wparam, lparam),
    }
}

fn point_from_lparam(lparam: LPARAM) -> POINT {
    let value = lparam.0 as u32;
    POINT {
        x: (value as u16 as i16) as i32,
        y: ((value >> 16) as u16 as i16) as i32,
    }
}

fn scaled(window: HWND, value: i32) -> i32 {
    let dpi = unsafe { GetDpiForWindow(window) }.max(96) as i32;
    scaled_for_dpi(value, dpi)
}

fn scaled_for_dpi(value: i32, dpi: i32) -> i32 {
    (value * dpi + 48) / 96
}

fn logical_panel_width(recording_enabled: bool) -> i32 {
    if recording_enabled {
        return PANEL_WIDTH;
    }
    PANEL_WIDTH_WITHOUT_RECORDING
}

fn panel_column_count(recording_enabled: bool) -> i32 {
    if recording_enabled {
        return 6;
    }
    5
}

fn set_shared_visibility(shared: &SharedVisibility, visible: bool) {
    if let Ok(mut state) = shared.lock() {
        *state = visible;
    }
}

fn physical_to_logical_rect(window: HWND, rect: RECT) -> Option<RECT> {
    let mut top_left = POINT {
        x: rect.left,
        y: rect.top,
    };
    let mut bottom_right = POINT {
        x: rect.right,
        y: rect.bottom,
    };
    let top_ok = unsafe { PhysicalToLogicalPointForPerMonitorDPI(Some(window), &mut top_left) };
    let bottom_ok =
        unsafe { PhysicalToLogicalPointForPerMonitorDPI(Some(window), &mut bottom_right) };
    if !top_ok.as_bool() || !bottom_ok.as_bool() {
        return None;
    }
    Some(RECT {
        left: top_left.x,
        top: top_left.y,
        right: bottom_right.x,
        bottom: bottom_right.y,
    })
}

fn logical_to_physical_rect(window: HWND, rect: RECT) -> Option<RECT> {
    let mut top_left = POINT {
        x: rect.left,
        y: rect.top,
    };
    let mut bottom_right = POINT {
        x: rect.right,
        y: rect.bottom,
    };
    let top_ok = unsafe { LogicalToPhysicalPointForPerMonitorDPI(Some(window), &mut top_left) };
    let bottom_ok =
        unsafe { LogicalToPhysicalPointForPerMonitorDPI(Some(window), &mut bottom_right) };
    if !top_ok.as_bool() || !bottom_ok.as_bool() {
        return None;
    }
    Some(RECT {
        left: top_left.x,
        top: top_left.y,
        right: bottom_right.x,
        bottom: bottom_right.y,
    })
}

fn contains(rect: &RECT, point: POINT) -> bool {
    point.x >= rect.left && point.x < rect.right && point.y >= rect.top && point.y < rect.bottom
}

fn physical_panel_rect(x: i32, y: i32, recording_enabled: bool) -> RECT {
    let width = logical_panel_width(recording_enabled);
    let logical = RECT {
        left: x,
        top: y,
        right: x + width,
        bottom: y + PANEL_HEIGHT,
    };
    let center = POINT {
        x: x + width / 2,
        y: y + PANEL_HEIGHT / 2,
    };
    let all_monitors = monitors();
    let mut fallback = None;

    for monitor in &all_monitors {
        let probe_rect = RECT {
            left: monitor.rect.left,
            top: monitor.rect.top,
            right: monitor.rect.left + 1,
            bottom: monitor.rect.top + 1,
        };
        let probe_style = WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE;
        let Some(probe) = create_popup_window(PANEL_CLASS_NAME, probe_style, &probe_rect) else {
            continue;
        };
        let logical_monitor = physical_to_logical_rect(probe, monitor.rect);
        let converted = logical_to_physical_rect(probe, logical);
        unsafe {
            let _ = DestroyWindow(probe);
        }
        if fallback.is_none() && monitor.is_primary {
            fallback = converted;
        }
        if logical_monitor.is_some_and(|bounds| contains(&bounds, center)) {
            return converted.unwrap_or(logical);
        }
    }

    fallback.unwrap_or(logical)
}

fn apply_suggested_rect(window: HWND, lparam: LPARAM) {
    if lparam.0 == 0 {
        return;
    }
    let suggested = unsafe { *(lparam.0 as *const RECT) };
    unsafe {
        let _ = SetWindowPos(
            window,
            Some(HWND_TOPMOST),
            suggested.left,
            suggested.top,
            suggested.right - suggested.left,
            suggested.bottom - suggested.top,
            SWP_NOACTIVATE,
        );
    }
}

fn apply_panel_dpi_rect(window: HWND, wparam: WPARAM, lparam: LPARAM) {
    if lparam.0 == 0 {
        return;
    }
    let suggested = unsafe { *(lparam.0 as *const RECT) };
    let dpi = ((wparam.0 & 0xffff) as i32).max(96);
    let recording_enabled = STATE.with(|state| state.borrow().recording_enabled);
    unsafe {
        let _ = SetWindowPos(
            window,
            Some(HWND_TOPMOST),
            suggested.left,
            suggested.top,
            scaled_for_dpi(logical_panel_width(recording_enabled), dpi),
            scaled_for_dpi(PANEL_HEIGHT, dpi),
            SWP_NOACTIVATE,
        );
    }
}

fn panel_button_at(
    width: i32,
    height: i32,
    point: POINT,
    recording_enabled: bool,
) -> Option<PanelButton> {
    if width <= 0
        || height <= 0
        || point.x < 0
        || point.y < 0
        || point.x >= width
        || point.y >= height
    {
        return None;
    }
    let column_count = panel_column_count(recording_enabled);
    let column = (point.x * column_count / width).clamp(0, column_count - 1);
    panel_button_for_column(column, recording_enabled)
}

fn panel_button_for_column(column: i32, recording_enabled: bool) -> Option<PanelButton> {
    if column < 0 || column >= panel_column_count(recording_enabled) {
        return None;
    }
    match column {
        0 => Some(PanelButton::Close),
        1 => Some(PanelButton::AspectRatio),
        2 => Some(PanelButton::Size),
        3 => Some(PanelButton::Screenshot),
        4 if recording_enabled => Some(PanelButton::Record),
        4 | 5 => Some(PanelButton::Drag),
        _ => None,
    }
}

fn panel_button(window: HWND, point: POINT) -> Option<PanelButton> {
    let mut bounds = RECT::default();
    unsafe {
        let _ = GetClientRect(window, &mut bounds);
    }
    let recording_enabled = STATE.with(|state| state.borrow().recording_enabled);
    panel_button_at(
        bounds.right - bounds.left,
        bounds.bottom - bounds.top,
        point,
        recording_enabled,
    )
}

fn panel_button_columns(button: PanelButton, recording_enabled: bool) -> Option<(i32, i32)> {
    match button {
        PanelButton::Close => Some((0, 1)),
        PanelButton::AspectRatio => Some((1, 2)),
        PanelButton::Size => Some((2, 3)),
        PanelButton::Screenshot => Some((3, 4)),
        PanelButton::Record if recording_enabled => Some((4, 5)),
        PanelButton::Record => None,
        PanelButton::Drag if recording_enabled => Some((5, 6)),
        PanelButton::Drag => Some((4, 5)),
    }
}

fn panel_button_rect(window: HWND, button: PanelButton) -> Option<RECT> {
    let mut client = RECT::default();
    unsafe {
        let _ = GetClientRect(window, &mut client);
    }
    let recording_enabled = STATE.with(|state| state.borrow().recording_enabled);
    let column_count = panel_column_count(recording_enabled);
    let column_width = (client.right - client.left) / column_count;
    let (start, end) = panel_button_columns(button, recording_enabled)?;
    Some(RECT {
        left: column_width * start,
        top: client.top,
        right: if end == column_count {
            client.right
        } else {
            column_width * end
        },
        bottom: client.bottom,
    })
}

fn handle_panel_hover(window: HWND, point: POINT) {
    let hovered = panel_button(window, point);
    let changed = STATE.with(|state| {
        let mut state = state.borrow_mut();
        if state.hovered == hovered {
            return false;
        }
        state.hovered = hovered;
        true
    });
    if !changed {
        return;
    }
    let mut tracking = TRACKMOUSEEVENT {
        cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
        dwFlags: TME_LEAVE,
        hwndTrack: window,
        dwHoverTime: 0,
    };
    unsafe {
        let _ = TrackMouseEvent(&mut tracking);
        let _ = InvalidateRect(Some(window), None, false);
    }
}

fn clear_panel_hover(window: HWND) {
    let changed = STATE.with(|state| state.borrow_mut().hovered.take().is_some());
    if changed {
        unsafe {
            let _ = InvalidateRect(Some(window), None, false);
        }
    }
}

fn handle_panel_hit_test(window: HWND, lparam: LPARAM) -> LRESULT {
    let mut point = point_from_lparam(lparam);
    unsafe {
        let _ = ScreenToClient(window, &mut point);
    }
    if panel_button(window, point) == Some(PanelButton::Drag) {
        return LRESULT(HTCAPTION as isize);
    }
    LRESULT(HTCLIENT as isize)
}

fn draw_centered_text(dc: windows::Win32::Graphics::Gdi::HDC, text: &str, mut rect: RECT) {
    let wide = to_wide(text);
    let mut buffer = wide[..wide.len() - 1].to_vec();
    unsafe {
        DrawTextW(
            dc,
            &mut buffer,
            &mut rect,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        );
    }
}

fn draw_panel_text(
    dc: windows::Win32::Graphics::Gdi::HDC,
    window: HWND,
    button: PanelButton,
    text: &str,
) {
    let Some(rect) = panel_button_rect(window, button) else {
        return;
    };
    draw_centered_text(dc, text, rect);
}

fn paint_panel(window: HWND) {
    let (font, hovered, ratio, recording_enabled) = STATE.with(|state| {
        let state = state.borrow();
        (
            state.font,
            state.hovered,
            state.aspect_ratio,
            state.recording_enabled,
        )
    });
    unsafe {
        let mut paint_struct = PAINTSTRUCT::default();
        let dc = BeginPaint(window, &mut paint_struct);
        let background = CreateSolidBrush(COLORREF(0x00242424));
        FillRect(dc, &paint_struct.rcPaint, background);
        let _ = DeleteObject(background.into());

        if let Some(button) = hovered {
            if let Some(rect) = panel_button_rect(window, button) {
                let hover_brush = CreateSolidBrush(COLORREF(0x00383838));
                FillRect(dc, &rect, hover_brush);
                let _ = DeleteObject(hover_brush.into());
            }
        }

        SetBkMode(dc, TRANSPARENT);
        SetTextColor(dc, COLORREF(0x00EEEEEE));
        let previous_font = font.map(|font| SelectObject(dc, font.into()));
        draw_panel_text(dc, window, PanelButton::Close, "X");
        draw_panel_text(dc, window, PanelButton::AspectRatio, ratio.name);
        draw_panel_text(dc, window, PanelButton::Size, "Size");
        draw_panel_text(dc, window, PanelButton::Screenshot, "Shot");
        if recording_enabled {
            draw_panel_text(dc, window, PanelButton::Record, "Rec");
        }
        draw_panel_text(dc, window, PanelButton::Drag, "::");

        let mut client = RECT::default();
        let _ = GetClientRect(window, &mut client);
        let column_count = panel_column_count(recording_enabled);
        let column_width = (client.right - client.left) / column_count;
        let separator_brush = CreateSolidBrush(COLORREF(0x00444444));
        for column in 1..column_count {
            let separator = RECT {
                left: column_width * column,
                top: scaled(window, 8),
                right: column_width * column + 1,
                bottom: client.bottom - scaled(window, 8),
            };
            FillRect(dc, &separator, separator_brush);
        }
        let _ = DeleteObject(separator_brush.into());

        if let Some(previous) = previous_font {
            SelectObject(dc, previous);
        }
        let _ = EndPaint(window, &paint_struct);
    }
}

fn refresh_panel_resources(window: HWND) {
    let old_font = STATE.with(|state| state.borrow_mut().font.take());
    if let Some(font) = old_font {
        unsafe {
            let _ = DeleteObject(font.into());
        }
    }
    let font_name = to_wide("Segoe UI");
    let font = unsafe {
        CreateFontW(
            -scaled(window, 11),
            0,
            0,
            0,
            FW_NORMAL.0 as i32,
            0,
            0,
            0,
            FONT_CHARSET(0),
            FONT_OUTPUT_PRECISION(0),
            FONT_CLIP_PRECISION(0),
            FONT_QUALITY(0),
            0,
            PCWSTR(font_name.as_ptr()),
        )
    };
    STATE.with(|state| {
        state.borrow_mut().font = Some(font);
    });
    let radius = scaled(window, CORNER_RADIUS) * 2;
    let mut client = RECT::default();
    unsafe {
        let _ = GetClientRect(window, &mut client);
        let region = CreateRoundRectRgn(0, 0, client.right + 1, client.bottom + 1, radius, radius);
        let _ = SetWindowRgn(window, Some(region), true);
        let _ = InvalidateRect(Some(window), None, true);
    }
    sync_editor_fonts();
}

fn position_panel(window: HWND, x: i32, y: i32) {
    let recording_enabled = STATE.with(|state| state.borrow().recording_enabled);
    let rect = physical_panel_rect(x, y, recording_enabled);
    unsafe {
        let _ = SetWindowPos(
            window,
            Some(HWND_TOPMOST),
            rect.left,
            rect.top,
            rect.right - rect.left,
            rect.bottom - rect.top,
            SWP_NOACTIVATE,
        );
    }
    refresh_panel_resources(window);
}

fn handle_panel_click(window: HWND, point: POINT) {
    let Some(button) = panel_button(window, point) else {
        return;
    };
    match button {
        PanelButton::Close => {
            close_size_editor();
            emit_close();
        }
        PanelButton::AspectRatio => {
            close_size_editor();
            show_aspect_ratio_menu(window);
        }
        PanelButton::Size => toggle_size_editor(),
        PanelButton::Screenshot => {
            close_size_editor();
            send_event("all-in-one:screenshot", None);
        }
        PanelButton::Record => {
            close_size_editor();
            let enabled = STATE.with(|state| state.borrow().recording_enabled);
            if enabled {
                send_event("all-in-one:record", None);
            }
        }
        PanelButton::Drag => {}
    }
}

fn emit_close() {
    send_event("all-in-one:close", None);
}

fn show_aspect_ratio_menu(window: HWND) {
    let Ok(menu) = (unsafe { CreatePopupMenu() }) else {
        return;
    };
    let current = STATE.with(|state| state.borrow().aspect_ratio);
    for (index, ratio) in ASPECT_RATIOS.iter().enumerate() {
        let mut flags = MF_STRING;
        if *ratio == current {
            flags |= MF_CHECKED;
        }
        let label = to_wide(ratio.name);
        unsafe {
            let _ = AppendMenuW(menu, flags, index + 1, PCWSTR(label.as_ptr()));
        }
    }
    let mut panel_rect = RECT::default();
    unsafe {
        let _ = GetWindowRect(window, &mut panel_rect);
        let _ = SetForegroundWindow(window);
    }
    let command = unsafe {
        TrackPopupMenu(
            menu,
            TPM_LEFTALIGN | TPM_TOPALIGN | TPM_RETURNCMD | TPM_NONOTIFY,
            panel_rect.left + scaled(window, BUTTON_WIDTH),
            panel_rect.bottom,
            None,
            window,
            None,
        )
    }
    .0 as usize;
    unsafe {
        let _ = DestroyMenu(menu);
    }
    let Some(ratio) = command
        .checked_sub(1)
        .and_then(|index| ASPECT_RATIOS.get(index))
        .copied()
    else {
        return;
    };
    STATE.with(|state| {
        state.borrow_mut().aspect_ratio = ratio;
    });
    unsafe {
        let _ = InvalidateRect(Some(window), None, false);
    }
    send_event(
        "all-in-one:select-aspect-ratio",
        Some(json!({
            "width": ratio.width,
            "height": ratio.height,
            "name": ratio.name,
        })),
    );
}

fn create_child_control(
    parent: HWND,
    class_name: &str,
    text: &str,
    style: WINDOW_STYLE,
    identifier: usize,
) -> Option<HWND> {
    let class_name = to_wide(class_name);
    let text = to_wide(text);
    let instance = unsafe { GetModuleHandleW(None) }.unwrap_or_default();
    unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(class_name.as_ptr()),
            PCWSTR(text.as_ptr()),
            style,
            0,
            0,
            1,
            1,
            Some(parent),
            Some(HMENU(identifier as *mut core::ffi::c_void)),
            Some(instance.into()),
            None,
        )
        .ok()
    }
}

fn toggle_size_editor() {
    let is_open = STATE.with(|state| state.borrow().size_editor.is_some());
    if is_open {
        close_size_editor();
        return;
    }
    show_size_editor();
}

fn show_size_editor() {
    let (panel, width, height) = STATE.with(|state| {
        let state = state.borrow();
        (state.panel, state.selection_width, state.selection_height)
    });
    let Some(panel) = panel else {
        return;
    };
    ensure_window_class(SIZE_EDITOR_CLASS_NAME, Some(size_editor_wndproc), None);
    let mut panel_rect = RECT::default();
    unsafe {
        let _ = GetWindowRect(panel, &mut panel_rect);
    }
    let editor_width = scaled(panel, SIZE_EDITOR_WIDTH);
    let editor_height = scaled(panel, SIZE_EDITOR_HEIGHT);
    let left = panel_rect.left + (panel_rect.right - panel_rect.left - editor_width) / 2;
    let top = panel_rect.bottom + scaled(panel, 6);
    let rect = RECT {
        left,
        top,
        right: left + editor_width,
        bottom: top + editor_height,
    };
    let ex_style = WS_EX_TOPMOST | WS_EX_TOOLWINDOW;
    let Some(window) = create_popup_window(SIZE_EDITOR_CLASS_NAME, ex_style, &rect) else {
        return;
    };
    let edit_style = WS_CHILD
        | WS_VISIBLE
        | WS_BORDER
        | WS_TABSTOP
        | WINDOW_STYLE(ES_NUMBER as u32)
        | WINDOW_STYLE(ES_RIGHT as u32);
    let button_style = WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_DEFPUSHBUTTON as u32);
    let Some(width_edit) = create_child_control(
        window,
        "EDIT",
        &width.to_string(),
        edit_style,
        WIDTH_EDIT_ID,
    ) else {
        unsafe {
            let _ = DestroyWindow(window);
        }
        return;
    };
    let Some(height_edit) = create_child_control(
        window,
        "EDIT",
        &height.to_string(),
        edit_style,
        HEIGHT_EDIT_ID,
    ) else {
        unsafe {
            let _ = DestroyWindow(window);
        }
        return;
    };
    if create_child_control(window, "BUTTON", "Apply", button_style, APPLY_BUTTON_ID).is_none() {
        unsafe {
            let _ = DestroyWindow(window);
        }
        return;
    }
    let return_key_token = match add_key_handler(VK_RETURN.0 as u32, apply_size) {
        Ok(token) => token,
        Err(_) => {
            unsafe {
                let _ = DestroyWindow(window);
            }
            return;
        }
    };
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.size_editor = Some(SizeEditorState {
            window,
            width_edit,
            height_edit,
        });
        state.return_key_token = Some(return_key_token);
    });
    layout_size_editor(window);
    sync_editor_fonts();
    send_event("all-in-one:size-editor-opened", None);
    unsafe {
        let _ = ShowWindow(window, SW_SHOW);
        let _ = SetForegroundWindow(window);
        let _ = SetFocus(Some(width_edit));
        let _ = windows::Win32::UI::WindowsAndMessaging::SendMessageW(
            width_edit,
            EM_SETSEL_MESSAGE,
            Some(WPARAM(0)),
            Some(LPARAM(-1)),
        );
    }
}

fn control_by_id(window: HWND, identifier: i32) -> Option<HWND> {
    unsafe { windows::Win32::UI::WindowsAndMessaging::GetDlgItem(Some(window), identifier).ok() }
}

fn layout_size_editor(window: HWND) {
    let Some(width_edit) = control_by_id(window, WIDTH_EDIT_ID as i32) else {
        return;
    };
    let Some(height_edit) = control_by_id(window, HEIGHT_EDIT_ID as i32) else {
        return;
    };
    let Some(apply_button) = control_by_id(window, APPLY_BUTTON_ID as i32) else {
        return;
    };
    let positions = [
        (width_edit, 34, 16, 70, 24),
        (height_edit, 138, 16, 70, 24),
        (apply_button, 130, 48, 78, 28),
    ];
    for (control, x, y, width, height) in positions {
        unsafe {
            let _ = SetWindowPos(
                control,
                None,
                scaled(window, x),
                scaled(window, y),
                scaled(window, width),
                scaled(window, height),
                SWP_NOACTIVATE,
            );
        }
    }
    let radius = scaled(window, CORNER_RADIUS) * 2;
    let mut client = RECT::default();
    unsafe {
        let _ = GetClientRect(window, &mut client);
        let region = CreateRoundRectRgn(0, 0, client.right + 1, client.bottom + 1, radius, radius);
        let _ = SetWindowRgn(window, Some(region), true);
        let _ = InvalidateRect(Some(window), None, true);
    }
}

fn sync_editor_fonts() {
    let (font, controls) = STATE.with(|state| {
        let state = state.borrow();
        let controls = state.size_editor.as_ref().map(|editor| {
            [
                editor.width_edit,
                editor.height_edit,
                control_by_id(editor.window, APPLY_BUTTON_ID as i32).unwrap_or_default(),
            ]
        });
        (state.font, controls)
    });
    let (Some(font), Some(controls)) = (font, controls) else {
        return;
    };
    for control in controls {
        if control.is_invalid() {
            continue;
        }
        unsafe {
            let _ = windows::Win32::UI::WindowsAndMessaging::SendMessageW(
                control,
                WM_SETFONT,
                Some(WPARAM(font.0 as usize)),
                Some(LPARAM(1)),
            );
        }
    }
}

fn paint_size_editor(window: HWND) {
    let font = STATE.with(|state| state.borrow().font);
    unsafe {
        let mut paint_struct = PAINTSTRUCT::default();
        let dc = BeginPaint(window, &mut paint_struct);
        let background = CreateSolidBrush(COLORREF(0x00242424));
        FillRect(dc, &paint_struct.rcPaint, background);
        let _ = DeleteObject(background.into());
        let border = CreateSolidBrush(COLORREF(0x00444444));
        let mut client = RECT::default();
        let _ = GetClientRect(window, &mut client);
        FrameRect(dc, &client, border);
        let _ = DeleteObject(border.into());
        SetBkMode(dc, TRANSPARENT);
        SetTextColor(dc, COLORREF(0x00CCCCCC));
        let previous_font = font.map(|font| SelectObject(dc, font.into()));
        draw_centered_text(
            dc,
            "W",
            RECT {
                left: scaled(window, 12),
                top: scaled(window, 16),
                right: scaled(window, 30),
                bottom: scaled(window, 40),
            },
        );
        draw_centered_text(
            dc,
            "H",
            RECT {
                left: scaled(window, 116),
                top: scaled(window, 16),
                right: scaled(window, 134),
                bottom: scaled(window, 40),
            },
        );
        if let Some(previous) = previous_font {
            SelectObject(dc, previous);
        }
        let _ = EndPaint(window, &paint_struct);
    }
}

fn read_size(control: HWND) -> Option<i32> {
    let mut buffer = vec![0u16; 16];
    let length = unsafe { GetWindowTextW(control, &mut buffer) };
    if length <= 0 {
        return None;
    }
    String::from_utf16_lossy(&buffer[..length as usize])
        .parse::<i32>()
        .ok()
        .filter(|value| (MIN_SIZE..=MAX_SIZE).contains(value))
}

fn apply_size() {
    let controls = STATE.with(|state| {
        state
            .borrow()
            .size_editor
            .as_ref()
            .map(|editor| (editor.width_edit, editor.height_edit))
    });
    let Some((width_edit, height_edit)) = controls else {
        return;
    };
    let (Some(width), Some(height)) = (read_size(width_edit), read_size(height_edit)) else {
        unsafe {
            let _ = MessageBeep(MB_ICONWARNING);
        }
        return;
    };
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.selection_width = width;
        state.selection_height = height;
    });
    send_event(
        "all-in-one:update-size",
        Some(json!({ "width": width, "height": height })),
    );
    close_size_editor();
}

fn handle_size_editor_activation(wparam: WPARAM, lparam: LPARAM) {
    if wparam.0 & 0xffff != WA_INACTIVE_VALUE {
        return;
    }
    let panel = STATE.with(|state| state.borrow().panel);
    let activated = HWND(lparam.0 as *mut core::ffi::c_void);
    if panel == Some(activated) {
        return;
    }
    close_size_editor();
}

fn close_size_editor() {
    let (editor, return_key_token) = STATE.with(|state| {
        let mut state = state.borrow_mut();
        (state.size_editor.take(), state.return_key_token.take())
    });
    let Some(editor) = editor else {
        return;
    };
    if let Some(token) = return_key_token {
        remove_key_handler(token);
    }
    unsafe {
        let _ = DestroyWindow(editor.window);
    }
    send_event("all-in-one:size-editor-closed", None);
}

fn sync_editor_size() {
    let values = STATE.with(|state| {
        let state = state.borrow();
        state.size_editor.as_ref().map(|editor| {
            (
                editor.width_edit,
                editor.height_edit,
                state.selection_width,
                state.selection_height,
            )
        })
    });
    let Some((width_edit, height_edit, width, height)) = values else {
        return;
    };
    let width = to_wide(&width.to_string());
    let height = to_wide(&height.to_string());
    unsafe {
        let _ = SetWindowTextW(width_edit, PCWSTR(width.as_ptr()));
        let _ = SetWindowTextW(height_edit, PCWSTR(height.as_ptr()));
    }
}

fn handle_escape() {
    let editor_open = STATE.with(|state| state.borrow().size_editor.is_some());
    if editor_open {
        close_size_editor();
        return;
    }
    let panel_visible = STATE.with(|state| state.borrow().panel.is_some());
    if panel_visible {
        emit_close();
    }
}

fn update_selection_size(width: Option<i32>, height: Option<i32>) {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        if width.is_some_and(|value| value > 0) {
            state.selection_width = width.unwrap_or_default();
        }
        if height.is_some_and(|value| value > 0) {
            state.selection_height = height.unwrap_or_default();
        }
    });
    sync_editor_size();
}

fn show_panel(
    x: i32,
    y: i32,
    selection_width: Option<i32>,
    selection_height: Option<i32>,
    recording_enabled: bool,
    shared: SharedVisibility,
) -> bool {
    update_selection_size(selection_width, selection_height);
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.recording_enabled = recording_enabled;
        state.hovered = None;
        state.shared_visibility = Some(shared.clone());
    });
    let existing = STATE.with(|state| state.borrow().panel);
    if let Some(window) = existing {
        position_panel(window, x, y);
        unsafe {
            let _ = ShowWindow(window, SW_SHOWNOACTIVATE);
            let _ = SetForegroundWindow(window);
        }
        set_shared_visibility(&shared, true);
        return true;
    }
    ensure_window_class(PANEL_CLASS_NAME, Some(panel_wndproc), None);
    let rect = physical_panel_rect(x, y, recording_enabled);
    let ex_style = WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_LAYERED;
    let Some(window) = create_popup_window(PANEL_CLASS_NAME, ex_style, &rect) else {
        set_shared_visibility(&shared, false);
        STATE.with(|state| {
            state.borrow_mut().shared_visibility = None;
        });
        return false;
    };
    let layered = unsafe { SetLayeredWindowAttributes(window, COLORREF(0), 250, LWA_ALPHA) };
    if layered.is_err() {
        unsafe {
            let _ = DestroyWindow(window);
        }
        set_shared_visibility(&shared, false);
        STATE.with(|state| {
            state.borrow_mut().shared_visibility = None;
        });
        return false;
    }
    STATE.with(|state| {
        state.borrow_mut().panel = Some(window);
    });
    refresh_panel_resources(window);
    let escape_key_token = match add_key_handler(VK_ESCAPE.0 as u32, handle_escape) {
        Ok(token) => token,
        Err(_) => {
            hide_panel();
            return false;
        }
    };
    STATE.with(|state| {
        state.borrow_mut().escape_key_token = Some(escape_key_token);
    });
    unsafe {
        let _ = ShowWindow(window, SW_SHOWNOACTIVATE);
        let _ = SetForegroundWindow(window);
    }
    set_shared_visibility(&shared, true);
    true
}

fn hide_panel() {
    close_size_editor();
    let (window, font, escape_key_token, shared) = STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.hovered = None;
        (
            state.panel.take(),
            state.font.take(),
            state.escape_key_token.take(),
            state.shared_visibility.take(),
        )
    });
    if let Some(window) = window {
        unsafe {
            let _ = DestroyWindow(window);
        }
    }
    if let Some(font) = font {
        unsafe {
            let _ = DeleteObject(font.into());
        }
    }
    if let Some(token) = escape_key_token {
        remove_key_handler(token);
    }
    if let Some(shared) = shared {
        set_shared_visibility(&shared, false);
    }
}

fn update_panel(x: i32, y: i32, selection_width: Option<i32>, selection_height: Option<i32>) {
    update_selection_size(selection_width, selection_height);
    let panel = STATE.with(|state| state.borrow().panel);
    if let Some(panel) = panel {
        position_panel(panel, x, y);
    }
}

fn focus_panel() {
    let panel = STATE.with(|state| state.borrow().panel);
    if let Some(panel) = panel {
        unsafe {
            let _ = ShowWindow(panel, SW_SHOW);
            let _ = SetForegroundWindow(panel);
        }
    }
}

fn set_aspect_ratio(width: i32, height: i32) {
    let ratio = ASPECT_RATIOS
        .iter()
        .find(|ratio| ratio.width == width && ratio.height == height)
        .copied()
        .unwrap_or(ASPECT_RATIOS[0]);
    let panel = STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.aspect_ratio = ratio;
        state.panel
    });
    if let Some(panel) = panel {
        unsafe {
            let _ = InvalidateRect(Some(panel), None, false);
        }
    }
}

pub struct AllInOneModule {
    visible: SharedVisibility,
}

impl AllInOneModule {
    pub fn new() -> Self {
        AllInOneModule {
            visible: Arc::new(Mutex::new(false)),
        }
    }
}

impl Module for AllInOneModule {
    fn name(&self) -> &'static str {
        "all-in-one"
    }

    fn handle(&mut self, request: &Request) -> Reply {
        match request.method.as_str() {
            "show" => {
                let x = param_i32(&request.params, "x").unwrap_or(100);
                let y = param_i32(&request.params, "y").unwrap_or(100);
                let selection_width = param_i32(&request.params, "selectionWidth");
                let selection_height = param_i32(&request.params, "selectionHeight");
                let recording_enabled =
                    param_bool(&request.params, "recordingEnabled").unwrap_or(true);
                let shared = self.visible.clone();
                let request_id = request.id.clone();
                run_on_ui(move || {
                    let visible = show_panel(
                        x,
                        y,
                        selection_width,
                        selection_height,
                        recording_enabled,
                        shared,
                    );
                    if !visible {
                        respond_error(&request_id, "UI_ERROR", "Failed to show all-in-one control");
                        return;
                    }
                    respond_success(&request_id, json!({ "visible": true }));
                });
                Reply::Deferred
            }
            "hide" => {
                let request_id = request.id.clone();
                run_on_ui(move || {
                    hide_panel();
                    respond_success(&request_id, json!({ "visible": false }));
                });
                Reply::Deferred
            }
            "update" => {
                let (Some(x), Some(y)) = (
                    param_i32(&request.params, "x"),
                    param_i32(&request.params, "y"),
                ) else {
                    return Reply::Now(Err((
                        "INVALID_PARAMS".to_string(),
                        "update requires x, y".to_string(),
                    )));
                };
                let selection_width = param_i32(&request.params, "selectionWidth");
                let selection_height = param_i32(&request.params, "selectionHeight");
                let request_id = request.id.clone();
                run_on_ui(move || {
                    update_panel(x, y, selection_width, selection_height);
                    respond_success(&request_id, json!({ "updated": true }));
                });
                Reply::Deferred
            }
            "focus" => {
                let request_id = request.id.clone();
                run_on_ui(move || {
                    focus_panel();
                    respond_success(&request_id, json!({ "focused": true }));
                });
                Reply::Deferred
            }
            "status" => {
                let visible = self.visible.lock().map(|visible| *visible).unwrap_or(false);
                Reply::Now(Ok(Some(json!({ "visible": visible }))))
            }
            "setAspectRatio" => {
                let width = param_i32(&request.params, "width").unwrap_or(0);
                let height = param_i32(&request.params, "height").unwrap_or(0);
                let request_id = request.id.clone();
                run_on_ui(move || {
                    set_aspect_ratio(width, height);
                    respond_success(&request_id, json!({ "updated": true }));
                });
                Reply::Deferred
            }
            method => method_not_found(method),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enabled_geometry_maps_six_equal_columns() {
        let expected = [
            PanelButton::Close,
            PanelButton::AspectRatio,
            PanelButton::Size,
            PanelButton::Screenshot,
            PanelButton::Record,
            PanelButton::Drag,
        ];

        assert_eq!(logical_panel_width(true), 288);
        assert_eq!(panel_column_count(true), 6);
        for (column, button) in expected.into_iter().enumerate() {
            let point = POINT {
                x: column as i32 * BUTTON_WIDTH + BUTTON_WIDTH / 2,
                y: PANEL_HEIGHT / 2,
            };
            assert_eq!(
                panel_button_at(PANEL_WIDTH, PANEL_HEIGHT, point, true),
                Some(button)
            );
            assert_eq!(
                panel_button_columns(button, true),
                Some((column as i32, column as i32 + 1))
            );
        }
    }

    #[test]
    fn disabled_geometry_maps_five_equal_columns_without_recording() {
        let expected = [
            PanelButton::Close,
            PanelButton::AspectRatio,
            PanelButton::Size,
            PanelButton::Screenshot,
            PanelButton::Drag,
        ];

        assert_eq!(logical_panel_width(false), 240);
        assert_eq!(panel_column_count(false), 5);
        for (column, button) in expected.into_iter().enumerate() {
            let point = POINT {
                x: column as i32 * BUTTON_WIDTH + BUTTON_WIDTH / 2,
                y: PANEL_HEIGHT / 2,
            };
            assert_eq!(
                panel_button_at(PANEL_WIDTH_WITHOUT_RECORDING, PANEL_HEIGHT, point, false),
                Some(button)
            );
            assert_eq!(
                panel_button_columns(button, false),
                Some((column as i32, column as i32 + 1))
            );
        }
        assert_eq!(panel_button_columns(PanelButton::Record, false), None);
        assert_eq!(panel_button_for_column(5, false), None);
    }

    #[test]
    fn geometry_rejects_points_outside_the_panel() {
        let outside = [
            POINT { x: -1, y: 0 },
            POINT {
                x: PANEL_WIDTH_WITHOUT_RECORDING,
                y: 0,
            },
            POINT { x: 0, y: -1 },
            POINT {
                x: 0,
                y: PANEL_HEIGHT,
            },
        ];

        for point in outside {
            assert_eq!(
                panel_button_at(PANEL_WIDTH_WITHOUT_RECORDING, PANEL_HEIGHT, point, false),
                None
            );
        }
    }

    #[test]
    fn dpi_scaling_preserves_enabled_and_disabled_logical_widths() {
        assert_eq!(scaled_for_dpi(logical_panel_width(true), 144), 432);
        assert_eq!(scaled_for_dpi(logical_panel_width(false), 144), 360);
        assert_eq!(scaled_for_dpi(PANEL_HEIGHT, 144), 72);
    }
}
