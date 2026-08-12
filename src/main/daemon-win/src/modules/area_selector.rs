use crate::overlay::{
    add_key_handler, configure_overlay_window, create_popup_window, default_wndproc,
    disable_window_transitions, ensure_window_class, monitors, rect_height, rect_width,
    remove_key_handler, to_wide,
};
use crate::protocol::{
    param_bool, param_i32, param_str, respond_error, respond_success, send_event, Request,
};
use crate::router::{method_not_found, Module, Reply};
use crate::ui::run_on_ui;
use serde_json::{json, Value};
use std::cell::RefCell;
use std::ffi::c_void;
use std::sync::{Arc, Mutex};
use windows::core::{Error, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CombineRgn, CreateFontW, CreateRectRgn, CreateSolidBrush, DeleteObject, DrawTextW,
    EndPaint, FillRect, FrameRect, GetTextExtentPoint32W, InvalidateRect, SelectObject, SetBkMode,
    SetTextColor, SetWindowRgn, DT_CENTER, DT_SINGLELINE, DT_VCENTER, ERROR, FONT_CHARSET,
    FONT_CLIP_PRECISION, FONT_OUTPUT_PRECISION, FONT_QUALITY, FW_SEMIBOLD, HFONT, PAINTSTRUCT,
    RGN_DIFF, TRANSPARENT,
};
use windows::Win32::UI::HiDpi::{
    GetDpiForWindow, LogicalToPhysicalPointForPerMonitorDPI, PhysicalToLogicalPointForPerMonitorDPI,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture, VK_ESCAPE};
use windows::Win32::UI::WindowsAndMessaging::{
    DestroyWindow, GetClientRect, LoadCursorW, SetCursor, SetLayeredWindowAttributes, SetWindowPos,
    ShowWindow, HTTRANSPARENT, HWND_TOPMOST, IDC_CROSS, IDC_SIZEALL, IDC_SIZENESW, IDC_SIZENS,
    IDC_SIZENWSE, IDC_SIZEWE, LWA_ALPHA, LWA_COLORKEY, SWP_HIDEWINDOW, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW, SW_HIDE, SW_SHOWNOACTIVATE, WM_ERASEBKGND,
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_NCHITTEST, WM_PAINT, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT,
};

const INPUT_CLASS_NAME: &str = "CaptyAreaSelectorInput";
const VISUAL_CLASS_NAME: &str = "CaptyAreaSelectorVisual";
const PROMPT_TEXT: &str = "Please select an area to begin";
const MIN_SELECTION_SIZE: i32 = 10;
const MIN_RESIZE_SIZE: i32 = 20;

fn scaled_hole_rect(
    hole: (i32, i32, i32, i32, i32, i32, i32),
    client_width: i32,
    client_height: i32,
) -> Option<(i32, i32, i32, i32)> {
    let (x, y, width, height, window_width, window_height, inset) = hole;

    if width - inset * 2 <= 0 || height - inset * 2 <= 0 {
        return None;
    }

    let scale_x = client_width as f64 / window_width as f64;
    let scale_y = client_height as f64 / window_height as f64;

    Some((
        ((x + inset) as f64 * scale_x).round() as i32,
        ((y + inset) as f64 * scale_y).round() as i32,
        ((x + width - inset) as f64 * scale_x).round() as i32,
        ((y + height - inset) as f64 * scale_y).round() as i32,
    ))
}

fn set_electron_window_region(
    window: HWND,
    hole: Option<(i32, i32, i32, i32, i32, i32, i32)>,
) -> windows::core::Result<()> {
    unsafe {
        let Some(hole) = hole else {
            if SetWindowRgn(window, None, true) == 0 {
                return Err(Error::from_thread());
            }
            return Ok(());
        };

        let mut client = RECT::default();
        GetClientRect(window, &mut client)?;
        let client_width = rect_width(&client);
        let client_height = rect_height(&client);
        let (_, _, _, _, window_width, window_height, _) = hole;
        if client_width <= 0 || client_height <= 0 || window_width <= 0 || window_height <= 0 {
            return Err(Error::from_thread());
        }

        let Some((hole_left, hole_top, hole_right, hole_bottom)) =
            scaled_hole_rect(hole, client_width, client_height)
        else {
            if SetWindowRgn(window, None, true) == 0 {
                return Err(Error::from_thread());
            }
            return Ok(());
        };

        let region = CreateRectRgn(0, 0, client_width, client_height);
        if region.0.is_null() {
            return Err(Error::from_thread());
        }
        let hole_region = CreateRectRgn(hole_left, hole_top, hole_right, hole_bottom);
        if hole_region.0.is_null() {
            let error = Error::from_thread();
            let _ = DeleteObject(region.into());
            return Err(error);
        }

        let combined = CombineRgn(Some(region), Some(region), Some(hole_region), RGN_DIFF);
        let _ = DeleteObject(hole_region.into());
        if combined.0 == ERROR {
            let error = Error::from_thread();
            let _ = DeleteObject(region.into());
            return Err(error);
        }

        if SetWindowRgn(window, Some(region), true) == 0 {
            let error = Error::from_thread();
            let _ = DeleteObject(region.into());
            return Err(error);
        }
        Ok(())
    }
}
const KEY_COLOR: COLORREF = COLORREF(0x00FF00FF);
const INPUT_ALPHA: u8 = 1;
const DEFAULT_ALPHA: u8 = 112;
const SIMPLE_ALPHA: u8 = 192;

#[derive(Default)]
struct SharedState {
    active: bool,
    hidden: bool,
}

type SharedSelectorState = Arc<Mutex<SharedState>>;

#[derive(Clone, Copy, PartialEq, Eq)]
enum HandlePosition {
    TopLeft,
    Top,
    TopRight,
    Right,
    BottomRight,
    Bottom,
    BottomLeft,
    Left,
    None,
}

#[derive(Clone, Copy)]
enum Interaction {
    Idle,
    Creating { start: POINT },
    Moving { offset: POINT },
    Resizing { handle: HandlePosition },
}

struct OverlayEntry {
    window: HWND,
    visual_window: HWND,
    physical_bounds: RECT,
    screen_id: i32,
    selection: Option<RECT>,
}

struct SelectorUiState {
    entries: Vec<OverlayEntry>,
    active_window: Option<HWND>,
    interaction: Interaction,
    aspect_ratio: Option<f64>,
    show_prompt: bool,
    simple_style: bool,
    hidden: bool,
    key_token: Option<usize>,
    font: Option<HFONT>,
    shared: Option<SharedSelectorState>,
}

thread_local! {
    static STATE: RefCell<SelectorUiState> = RefCell::new(SelectorUiState {
        entries: Vec::new(),
        active_window: None,
        interaction: Interaction::Idle,
        aspect_ratio: None,
        show_prompt: true,
        simple_style: false,
        hidden: false,
        key_token: None,
        font: None,
        shared: None,
    });
}

struct StartParams {
    fullscreen: bool,
    display_id: Option<i64>,
    preset: Option<RECT>,
    show_prompt: bool,
    simple_style: bool,
}

unsafe extern "system" fn input_wndproc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_PAINT => {
            paint_input(window);
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_LBUTTONDOWN => {
            handle_mouse_down(window, point_from_lparam(lparam));
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            handle_mouse_move(window, point_from_lparam(lparam));
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            handle_mouse_up(window, point_from_lparam(lparam));
            LRESULT(0)
        }
        _ => default_wndproc(window, message, wparam, lparam),
    }
}

unsafe extern "system" fn visual_wndproc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_PAINT => {
            paint_visual(window);
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_NCHITTEST => LRESULT(HTTRANSPARENT as isize),
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

fn normalize_rect(first: POINT, second: POINT) -> RECT {
    RECT {
        left: first.x.min(second.x),
        top: first.y.min(second.y),
        right: first.x.max(second.x),
        bottom: first.y.max(second.y),
    }
}

fn contains(rect: &RECT, point: POINT) -> bool {
    point.x >= rect.left && point.x <= rect.right && point.y >= rect.top && point.y <= rect.bottom
}

fn clamp_point(window: HWND, point: POINT) -> POINT {
    let mut bounds = RECT::default();
    unsafe {
        let _ = GetClientRect(window, &mut bounds);
    }
    POINT {
        x: point.x.clamp(bounds.left, bounds.right),
        y: point.y.clamp(bounds.top, bounds.bottom),
    }
}

fn scaled(window: HWND, value: i32) -> i32 {
    let dpi = unsafe { GetDpiForWindow(window) }.max(96) as i32;
    (value * dpi + 48) / 96
}

fn hit_test_handle(window: HWND, rect: &RECT, point: POINT) -> HandlePosition {
    let corner = scaled(window, 16);
    let edge = scaled(window, 12);
    let center_x = (rect.left + rect.right) / 2;
    let center_y = (rect.top + rect.bottom) / 2;

    let tests = [
        (
            HandlePosition::TopLeft,
            RECT {
                left: rect.left - edge,
                top: rect.top - edge,
                right: rect.left + corner,
                bottom: rect.top + corner,
            },
        ),
        (
            HandlePosition::TopRight,
            RECT {
                left: rect.right - corner,
                top: rect.top - edge,
                right: rect.right + edge,
                bottom: rect.top + corner,
            },
        ),
        (
            HandlePosition::BottomRight,
            RECT {
                left: rect.right - corner,
                top: rect.bottom - corner,
                right: rect.right + edge,
                bottom: rect.bottom + edge,
            },
        ),
        (
            HandlePosition::BottomLeft,
            RECT {
                left: rect.left - edge,
                top: rect.bottom - corner,
                right: rect.left + corner,
                bottom: rect.bottom + edge,
            },
        ),
        (
            HandlePosition::Top,
            RECT {
                left: center_x - corner * 2,
                top: rect.top - edge,
                right: center_x + corner * 2,
                bottom: rect.top + edge,
            },
        ),
        (
            HandlePosition::Right,
            RECT {
                left: rect.right - edge,
                top: center_y - corner * 2,
                right: rect.right + edge,
                bottom: center_y + corner * 2,
            },
        ),
        (
            HandlePosition::Bottom,
            RECT {
                left: center_x - corner * 2,
                top: rect.bottom - edge,
                right: center_x + corner * 2,
                bottom: rect.bottom + edge,
            },
        ),
        (
            HandlePosition::Left,
            RECT {
                left: rect.left - edge,
                top: center_y - corner * 2,
                right: rect.left + edge,
                bottom: center_y + corner * 2,
            },
        ),
    ];

    tests
        .into_iter()
        .find(|(_, bounds)| contains(bounds, point))
        .map(|(handle, _)| handle)
        .unwrap_or(HandlePosition::None)
}

fn set_pointer_cursor(window: HWND, selection: Option<RECT>, point: POINT) {
    let cursor_id = if let Some(rect) = selection {
        match hit_test_handle(window, &rect, point) {
            HandlePosition::TopLeft | HandlePosition::BottomRight => IDC_SIZENWSE,
            HandlePosition::TopRight | HandlePosition::BottomLeft => IDC_SIZENESW,
            HandlePosition::Top | HandlePosition::Bottom => IDC_SIZENS,
            HandlePosition::Left | HandlePosition::Right => IDC_SIZEWE,
            HandlePosition::None if contains(&rect, point) => IDC_SIZEALL,
            HandlePosition::None => IDC_CROSS,
        }
    } else {
        IDC_CROSS
    };

    if let Ok(cursor) = unsafe { LoadCursorW(None, cursor_id) } {
        unsafe {
            SetCursor(Some(cursor));
        }
    }
}

fn handle_mouse_down(window: HWND, point: POINT) {
    let point = clamp_point(window, point);
    let interaction = STATE.with(|state| {
        let mut state = state.borrow_mut();
        if state.hidden {
            return Interaction::Idle;
        }

        let selection = state
            .entries
            .iter()
            .find(|entry| entry.window == window)
            .and_then(|entry| entry.selection);

        if let Some(rect) = selection {
            let handle = hit_test_handle(window, &rect, point);
            if handle != HandlePosition::None {
                state.active_window = Some(window);
                return Interaction::Resizing { handle };
            }

            if contains(&rect, point) {
                state.active_window = Some(window);
                return Interaction::Moving {
                    offset: POINT {
                        x: point.x - rect.left,
                        y: point.y - rect.top,
                    },
                };
            }
        }

        for entry in &mut state.entries {
            entry.selection = None;
        }
        state.active_window = Some(window);
        if let Some(entry) = state
            .entries
            .iter_mut()
            .find(|entry| entry.window == window)
        {
            entry.selection = Some(RECT {
                left: point.x,
                top: point.y,
                right: point.x,
                bottom: point.y,
            });
        }
        Interaction::Creating { start: point }
    });

    if matches!(interaction, Interaction::Idle) {
        return;
    }

    STATE.with(|state| {
        state.borrow_mut().interaction = interaction;
    });
    unsafe {
        let _ = SetCapture(window);
    }
    invalidate_visual(window);
}

fn handle_mouse_move(window: HWND, point: POINT) {
    let point = clamp_point(window, point);
    let changed = STATE.with(|state| {
        let mut state = state.borrow_mut();
        let interaction = state.interaction;
        let aspect_ratio = state.aspect_ratio;
        let Some(entry) = state
            .entries
            .iter_mut()
            .find(|entry| entry.window == window)
        else {
            return false;
        };

        match interaction {
            Interaction::Idle => return false,
            Interaction::Creating { start } => {
                let mut rect = normalize_rect(start, point);
                if let Some(ratio) = aspect_ratio {
                    rect = adjust_rect_to_ratio(rect, ratio, HandlePosition::None);
                    rect = fit_rect_in_client(window, rect);
                }
                entry.selection = Some(rect);
            }
            Interaction::Moving { offset } => {
                let Some(rect) = entry.selection else {
                    return false;
                };
                let width = rect_width(&rect);
                let height = rect_height(&rect);
                let mut client = RECT::default();
                unsafe {
                    let _ = GetClientRect(window, &mut client);
                }
                let left = if width >= rect_width(&client) {
                    client.left
                } else {
                    (point.x - offset.x).clamp(client.left, client.right - width)
                };
                let top = if height >= rect_height(&client) {
                    client.top
                } else {
                    (point.y - offset.y).clamp(client.top, client.bottom - height)
                };
                entry.selection = Some(RECT {
                    left,
                    top,
                    right: left + width,
                    bottom: top + height,
                });
            }
            Interaction::Resizing { handle } => {
                let Some(rect) = entry.selection else {
                    return false;
                };
                let resized = resize_rect(rect, point, handle, aspect_ratio);
                entry.selection = Some(fit_rect_in_client(window, resized));
            }
        }
        true
    });

    if !changed {
        let selection = STATE.with(|state| {
            state
                .borrow()
                .entries
                .iter()
                .find(|entry| entry.window == window)
                .and_then(|entry| entry.selection)
        });
        set_pointer_cursor(window, selection, point);
        return;
    }

    invalidate_visual(window);

    let should_emit = STATE.with(|state| {
        matches!(
            state.borrow().interaction,
            Interaction::Moving { .. } | Interaction::Resizing { .. }
        )
    });
    if should_emit {
        emit_selection("updated", window);
    }
}

fn handle_mouse_up(window: HWND, point: POINT) {
    let point = clamp_point(window, point);
    unsafe {
        let _ = ReleaseCapture();
    }

    let (interaction, selection) = STATE.with(|state| {
        let mut state = state.borrow_mut();
        let interaction = state.interaction;
        state.interaction = Interaction::Idle;
        let selection = state
            .entries
            .iter()
            .find(|entry| entry.window == window)
            .and_then(|entry| entry.selection);
        (interaction, selection)
    });

    let Some(selection) = selection else {
        return;
    };

    match interaction {
        Interaction::Creating { .. } => {
            if rect_width(&selection) <= MIN_SELECTION_SIZE
                || rect_height(&selection) <= MIN_SELECTION_SIZE
            {
                cancel_from_ui();
                return;
            }
            hide_other_screens(window);
            emit_selection("selected", window);
        }
        Interaction::Moving { .. } | Interaction::Resizing { .. } => {
            emit_selection("updated", window);
        }
        Interaction::Idle => {}
    }

    set_pointer_cursor(window, Some(selection), point);
}

fn resize_rect(
    rect: RECT,
    point: POINT,
    handle: HandlePosition,
    aspect_ratio: Option<f64>,
) -> RECT {
    let mut resized = rect;
    match handle {
        HandlePosition::TopLeft => {
            resized.left = point.x.min(rect.right - MIN_RESIZE_SIZE);
            resized.top = point.y.min(rect.bottom - MIN_RESIZE_SIZE);
        }
        HandlePosition::Top => {
            resized.top = point.y.min(rect.bottom - MIN_RESIZE_SIZE);
        }
        HandlePosition::TopRight => {
            resized.right = point.x.max(rect.left + MIN_RESIZE_SIZE);
            resized.top = point.y.min(rect.bottom - MIN_RESIZE_SIZE);
        }
        HandlePosition::Right => {
            resized.right = point.x.max(rect.left + MIN_RESIZE_SIZE);
        }
        HandlePosition::BottomRight => {
            resized.right = point.x.max(rect.left + MIN_RESIZE_SIZE);
            resized.bottom = point.y.max(rect.top + MIN_RESIZE_SIZE);
        }
        HandlePosition::Bottom => {
            resized.bottom = point.y.max(rect.top + MIN_RESIZE_SIZE);
        }
        HandlePosition::BottomLeft => {
            resized.left = point.x.min(rect.right - MIN_RESIZE_SIZE);
            resized.bottom = point.y.max(rect.top + MIN_RESIZE_SIZE);
        }
        HandlePosition::Left => {
            resized.left = point.x.min(rect.right - MIN_RESIZE_SIZE);
        }
        HandlePosition::None => return resized,
    }

    aspect_ratio
        .map(|ratio| adjust_rect_to_ratio(resized, ratio, handle))
        .unwrap_or(resized)
}

fn adjust_rect_to_ratio(mut rect: RECT, ratio: f64, handle: HandlePosition) -> RECT {
    if ratio <= 0.0 || rect_height(&rect) <= 0 || rect_width(&rect) <= 0 {
        return rect;
    }

    let width = rect_width(&rect);
    let height = rect_height(&rect);
    if width as f64 / height as f64 > ratio {
        let new_width = (height as f64 * ratio).round() as i32;
        match handle {
            HandlePosition::TopLeft | HandlePosition::Left | HandlePosition::BottomLeft => {
                rect.left = rect.right - new_width
            }
            HandlePosition::None => {
                let center = (rect.left + rect.right) / 2;
                rect.left = center - new_width / 2;
                rect.right = rect.left + new_width;
                return rect;
            }
            _ => rect.right = rect.left + new_width,
        }
        rect.right = rect.left + new_width;
        return rect;
    }

    let new_height = (width as f64 / ratio).round() as i32;
    match handle {
        HandlePosition::TopLeft | HandlePosition::Top | HandlePosition::TopRight => {
            rect.top = rect.bottom - new_height
        }
        HandlePosition::None => {
            let center = (rect.top + rect.bottom) / 2;
            rect.top = center - new_height / 2;
            rect.bottom = rect.top + new_height;
            return rect;
        }
        _ => rect.bottom = rect.top + new_height,
    }
    rect.bottom = rect.top + new_height;
    rect
}

fn fit_rect_in_client(window: HWND, mut rect: RECT) -> RECT {
    let mut client = RECT::default();
    unsafe {
        let _ = GetClientRect(window, &mut client);
    }

    let width = rect_width(&rect).max(1).min(rect_width(&client));
    let height = rect_height(&rect).max(1).min(rect_height(&client));
    rect.left = rect.left.clamp(client.left, client.right - width);
    rect.top = rect.top.clamp(client.top, client.bottom - height);
    rect.right = rect.left + width;
    rect.bottom = rect.top + height;
    rect
}

fn paint_input(window: HWND) {
    unsafe {
        let mut paint_struct = PAINTSTRUCT::default();
        let dc = BeginPaint(window, &mut paint_struct);
        let black = CreateSolidBrush(COLORREF(0));
        FillRect(dc, &paint_struct.rcPaint, black);
        let _ = DeleteObject(black.into());
        let _ = EndPaint(window, &paint_struct);
    }
}

fn paint_visual(visual_window: HWND) {
    let snapshot = STATE.with(|state| {
        let state = state.borrow();
        state
            .entries
            .iter()
            .find(|entry| entry.visual_window == visual_window)
            .map(|entry| {
                (
                    entry.window,
                    entry.selection,
                    state.show_prompt,
                    state.simple_style,
                    state.font,
                )
            })
    });
    let Some((window, selection, show_prompt, simple_style, font)) = snapshot else {
        return;
    };

    unsafe {
        let mut paint_struct = PAINTSTRUCT::default();
        let dc = BeginPaint(visual_window, &mut paint_struct);
        let mut client = RECT::default();
        let _ = GetClientRect(visual_window, &mut client);

        let background_color = if simple_style { KEY_COLOR } else { COLORREF(0) };
        let background = CreateSolidBrush(background_color);
        FillRect(dc, &client, background);
        let _ = DeleteObject(background.into());

        if let Some(rect) = selection {
            let selection_color = if simple_style {
                COLORREF(0x00606060)
            } else {
                KEY_COLOR
            };
            let selection_brush = CreateSolidBrush(selection_color);
            FillRect(dc, &rect, selection_brush);
            let _ = DeleteObject(selection_brush.into());
            draw_selection(dc, window, rect, font);
        } else if show_prompt {
            draw_prompt(dc, window, &client, font);
        }

        let _ = EndPaint(visual_window, &paint_struct);
    }
}

fn invalidate_visual(window: HWND) {
    let visual_window = STATE.with(|state| {
        state
            .borrow()
            .entries
            .iter()
            .find(|entry| entry.window == window)
            .map(|entry| entry.visual_window)
    });
    if let Some(visual_window) = visual_window {
        unsafe {
            let _ = InvalidateRect(Some(visual_window), None, false);
        }
    }
}

fn draw_selection(
    dc: windows::Win32::Graphics::Gdi::HDC,
    window: HWND,
    rect: RECT,
    font: Option<HFONT>,
) {
    unsafe {
        let white = CreateSolidBrush(COLORREF(0x00FFFFFF));
        let thickness = scaled(window, 3);
        for inset in 0..thickness {
            let frame = RECT {
                left: rect.left + inset,
                top: rect.top + inset,
                right: rect.right - inset,
                bottom: rect.bottom - inset,
            };
            FrameRect(dc, &frame, white);
        }

        let length = scaled(window, 20);
        let handle = scaled(window, 4);
        let center_x = (rect.left + rect.right) / 2;
        let center_y = (rect.top + rect.bottom) / 2;
        let handles = [
            RECT {
                left: rect.left,
                top: rect.top,
                right: rect.left + length,
                bottom: rect.top + handle,
            },
            RECT {
                left: rect.left,
                top: rect.top,
                right: rect.left + handle,
                bottom: rect.top + length,
            },
            RECT {
                left: rect.right - length,
                top: rect.top,
                right: rect.right,
                bottom: rect.top + handle,
            },
            RECT {
                left: rect.right - handle,
                top: rect.top,
                right: rect.right,
                bottom: rect.top + length,
            },
            RECT {
                left: rect.left,
                top: rect.bottom - handle,
                right: rect.left + length,
                bottom: rect.bottom,
            },
            RECT {
                left: rect.left,
                top: rect.bottom - length,
                right: rect.left + handle,
                bottom: rect.bottom,
            },
            RECT {
                left: rect.right - length,
                top: rect.bottom - handle,
                right: rect.right,
                bottom: rect.bottom,
            },
            RECT {
                left: rect.right - handle,
                top: rect.bottom - length,
                right: rect.right,
                bottom: rect.bottom,
            },
            RECT {
                left: center_x - length / 2,
                top: rect.top,
                right: center_x + length / 2,
                bottom: rect.top + handle,
            },
            RECT {
                left: center_x - length / 2,
                top: rect.bottom - handle,
                right: center_x + length / 2,
                bottom: rect.bottom,
            },
            RECT {
                left: rect.left,
                top: center_y - length / 2,
                right: rect.left + handle,
                bottom: center_y + length / 2,
            },
            RECT {
                left: rect.right - handle,
                top: center_y - length / 2,
                right: rect.right,
                bottom: center_y + length / 2,
            },
        ];
        for handle_rect in handles {
            FillRect(dc, &handle_rect, white);
        }
        let _ = DeleteObject(white.into());
    }

    let dimensions = logical_rect_for_selection(window, rect)
        .map(|logical| format!("{} x {}", rect_width(&logical), rect_height(&logical)))
        .unwrap_or_else(|| format!("{} x {}", rect_width(&rect), rect_height(&rect)));
    draw_text_box(dc, window, &dimensions, rect, false, font);
}

fn draw_prompt(
    dc: windows::Win32::Graphics::Gdi::HDC,
    window: HWND,
    bounds: &RECT,
    font: Option<HFONT>,
) {
    draw_text_box(dc, window, PROMPT_TEXT, *bounds, true, font);
}

fn draw_text_box(
    dc: windows::Win32::Graphics::Gdi::HDC,
    window: HWND,
    text: &str,
    anchor: RECT,
    centered: bool,
    font: Option<HFONT>,
) {
    let Some(font) = font else {
        return;
    };

    unsafe {
        let previous_font = SelectObject(dc, font.into());
        SetBkMode(dc, TRANSPARENT);
        let wide = to_wide(text);
        let mut size = SIZE::default();
        let _ = GetTextExtentPoint32W(dc, &wide[..wide.len() - 1], &mut size);
        let padding_x = scaled(window, 10);
        let padding_y = scaled(window, 5);
        let box_width = size.cx + padding_x * 2;
        let box_height = size.cy + padding_y * 2;
        let center_x = (anchor.left + anchor.right) / 2;
        let mut top = if centered {
            (anchor.top + anchor.bottom - box_height) / 2
        } else {
            anchor.top - box_height - scaled(window, 8)
        };
        if top < 0 {
            top = anchor.bottom + scaled(window, 8);
        }
        let mut text_rect = RECT {
            left: center_x - box_width / 2,
            top,
            right: center_x + box_width / 2,
            bottom: top + box_height,
        };
        let background = CreateSolidBrush(COLORREF(0x00141414));
        FillRect(dc, &text_rect, background);
        let _ = DeleteObject(background.into());
        SetTextColor(dc, COLORREF(0x00FFFFFF));
        let mut buffer = wide[..wide.len() - 1].to_vec();
        DrawTextW(
            dc,
            &mut buffer,
            &mut text_rect,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        );
        SelectObject(dc, previous_font);
    }
}

fn logical_rect_for_selection(window: HWND, local: RECT) -> Option<RECT> {
    let physical_bounds = STATE.with(|state| {
        state
            .borrow()
            .entries
            .iter()
            .find(|entry| entry.window == window)
            .map(|entry| entry.physical_bounds)
    })?;
    let global = RECT {
        left: physical_bounds.left + local.left,
        top: physical_bounds.top + local.top,
        right: physical_bounds.left + local.right,
        bottom: physical_bounds.top + local.bottom,
    };
    physical_to_logical_rect(window, global)
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

fn selection_event_data(window: HWND) -> Option<Value> {
    let (screen_id, selection) = STATE.with(|state| {
        state
            .borrow()
            .entries
            .iter()
            .find(|entry| entry.window == window)
            .and_then(|entry| {
                entry
                    .selection
                    .map(|selection| (entry.screen_id, selection))
            })
    })?;
    let logical = logical_rect_for_selection(window, selection)?;
    Some(json!({
        "x": logical.left,
        "y": logical.top,
        "width": rect_width(&logical),
        "height": rect_height(&logical),
        "screenId": screen_id,
    }))
}

fn emit_selection(status: &str, window: HWND) {
    let Some(data) = selection_event_data(window) else {
        return;
    };
    send_event(&format!("area-selector:{status}"), Some(data));
}

fn hide_other_screens(active_window: HWND) {
    let other_windows = STATE.with(|state| {
        state
            .borrow()
            .entries
            .iter()
            .filter(|entry| entry.window != active_window)
            .map(|entry| (entry.window, entry.visual_window))
            .collect::<Vec<_>>()
    });
    for (window, visual_window) in other_windows {
        unsafe {
            let _ = ShowWindow(window, SW_HIDE);
            let _ = ShowWindow(visual_window, SW_HIDE);
        }
    }
}

fn set_shared_state(shared: &SharedSelectorState, active: bool, hidden: bool) {
    if let Ok(mut state) = shared.lock() {
        state.active = active;
        state.hidden = hidden;
    }
}

fn start_selection(params: StartParams, shared: SharedSelectorState, request_id: String) {
    teardown();
    ensure_window_class(
        INPUT_CLASS_NAME,
        Some(input_wndproc),
        unsafe { LoadCursorW(None, IDC_CROSS) }.ok(),
    );
    ensure_window_class(VISUAL_CLASS_NAME, Some(visual_wndproc), None);

    let all_monitors = monitors();
    let explicit_target = params.display_id.and_then(|display_id| {
        all_monitors
            .iter()
            .position(|monitor| monitor.device_number as i64 == display_id)
    });
    let sole_target = if all_monitors.len() == 1 {
        Some(0)
    } else {
        None
    };
    let primary_target = all_monitors
        .iter()
        .position(|monitor| monitor.is_primary)
        .or(sole_target);
    let fullscreen_target = explicit_target.or(sole_target);
    let target_index = if params.display_id.is_some() {
        explicit_target.or(sole_target)
    } else {
        primary_target
    };
    let target_screen_id = target_index
        .and_then(|index| all_monitors.get(index))
        .map(|monitor| monitor.device_number);

    let font = unsafe {
        let font_name = to_wide("Segoe UI");
        CreateFontW(
            -16,
            0,
            0,
            0,
            FW_SEMIBOLD.0 as i32,
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
        let mut state = state.borrow_mut();
        state.aspect_ratio = None;
        state.show_prompt = params.show_prompt;
        state.simple_style = params.simple_style;
        state.hidden = false;
        state.font = Some(font);
        state.shared = Some(shared.clone());
    });

    for (index, monitor) in all_monitors.into_iter().enumerate() {
        if params.fullscreen && fullscreen_target.is_some() && fullscreen_target != Some(index) {
            continue;
        }
        create_overlay(monitor.rect, monitor.device_number, params.simple_style);
    }

    let token = match add_key_handler(VK_ESCAPE.0 as u32, cancel_from_ui) {
        Ok(token) => token,
        Err(message) => {
            teardown();
            respond_error(&request_id, "UI_ERROR", &message);
            return;
        }
    };
    STATE.with(|state| {
        state.borrow_mut().key_token = Some(token);
    });
    set_shared_state(&shared, true, false);

    let target_window = STATE.with(|state| {
        let state = state.borrow();
        let target_screen_id = target_screen_id?;
        state
            .entries
            .iter()
            .find(|entry| entry.screen_id == target_screen_id)
            .map(|entry| entry.window)
    });

    let target_window = params
        .preset
        .and_then(find_window_for_logical_rect)
        .or(target_window);
    respond_success(&request_id, json!({ "started": true }));

    if params.fullscreen {
        if let Some(window) = target_window {
            select_full_window(window);
        }
        return;
    }

    if let (Some(window), Some(preset)) = (target_window, params.preset) {
        apply_logical_selection(window, preset, true);
    }
}

fn create_overlay(rect: RECT, screen_id: i32, simple_style: bool) {
    let ex_style = WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_LAYERED;
    let Some(window) = create_popup_window(INPUT_CLASS_NAME, ex_style, &rect) else {
        return;
    };
    let visual_style =
        WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED | WS_EX_TRANSPARENT;
    let Some(visual_window) = create_popup_window(VISUAL_CLASS_NAME, visual_style, &rect) else {
        unsafe {
            let _ = DestroyWindow(window);
        }
        return;
    };

    let alpha = if simple_style {
        SIMPLE_ALPHA
    } else {
        DEFAULT_ALPHA
    };
    unsafe {
        let _ = SetLayeredWindowAttributes(window, COLORREF(0), INPUT_ALPHA, LWA_ALPHA);
        let _ =
            SetLayeredWindowAttributes(visual_window, KEY_COLOR, alpha, LWA_ALPHA | LWA_COLORKEY);
        let _ = ShowWindow(window, SW_SHOWNOACTIVATE);
        let _ = ShowWindow(visual_window, SW_SHOWNOACTIVATE);
        let _ = SetWindowPos(
            window,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
        let _ = SetWindowPos(
            visual_window,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }

    STATE.with(|state| {
        state.borrow_mut().entries.push(OverlayEntry {
            window,
            visual_window,
            physical_bounds: rect,
            screen_id,
            selection: None,
        });
    });
}

fn find_window_for_logical_rect(rect: RECT) -> Option<HWND> {
    let center = POINT {
        x: (rect.left + rect.right) / 2,
        y: (rect.top + rect.bottom) / 2,
    };
    STATE.with(|state| {
        state.borrow().entries.iter().find_map(|entry| {
            let logical = physical_to_logical_rect(entry.window, entry.physical_bounds)?;
            contains(&logical, center).then_some(entry.window)
        })
    })
}

fn select_full_window(window: HWND) {
    let mut client = RECT::default();
    unsafe {
        let _ = GetClientRect(window, &mut client);
    }
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.active_window = Some(window);
        if let Some(entry) = state
            .entries
            .iter_mut()
            .find(|entry| entry.window == window)
        {
            entry.selection = Some(client);
        }
    });
    hide_other_screens(window);
    invalidate_visual(window);
    emit_selection("selected", window);
}

fn apply_logical_selection(window: HWND, logical: RECT, emit_initial: bool) {
    let Some(physical) = logical_to_physical_rect(window, logical) else {
        return;
    };
    let physical_bounds = STATE.with(|state| {
        state
            .borrow()
            .entries
            .iter()
            .find(|entry| entry.window == window)
            .map(|entry| entry.physical_bounds)
    });
    let Some(bounds) = physical_bounds else {
        return;
    };
    let local = RECT {
        left: physical.left - bounds.left,
        top: physical.top - bounds.top,
        right: physical.right - bounds.left,
        bottom: physical.bottom - bounds.top,
    };
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.active_window = Some(window);
        if let Some(entry) = state
            .entries
            .iter_mut()
            .find(|entry| entry.window == window)
        {
            entry.selection = Some(local);
        }
    });
    invalidate_visual(window);
    if emit_initial {
        hide_other_screens(window);
        emit_selection("selected", window);
    }
}

fn confirm_selection(request_id: String) {
    let (is_active, window) = STATE.with(|state| {
        let state = state.borrow();
        (!state.entries.is_empty(), state.active_window)
    });
    if !is_active {
        respond_error(&request_id, "NOT_ACTIVE", "No active selection");
        return;
    }
    let Some(window) = window else {
        teardown();
        respond_error(&request_id, "NO_SELECTION", "No selection to confirm");
        return;
    };
    let Some(data) = selection_event_data(window) else {
        teardown();
        respond_error(&request_id, "NO_SELECTION", "No selection to confirm");
        return;
    };

    teardown();
    send_event("area-selector:confirmed", Some(data));
    respond_success(&request_id, json!({ "confirmed": true }));
}

fn cancel_from_ui() {
    teardown();
    send_event("area-selector:cancelled", None);
}

fn cancel_from_request(request_id: String) {
    cancel_from_ui();
    respond_success(&request_id, json!({ "cancelled": true }));
}

fn update_selection(request_id: String, logical: RECT) {
    let window = STATE.with(|state| state.borrow().active_window);
    if let Some(window) = window {
        apply_logical_selection(window, logical, false);
    }
    respond_success(&request_id, json!({ "updated": true }));
}

fn hide_selector(request_id: String) {
    let should_hide = STATE.with(|state| {
        let mut state = state.borrow_mut();
        if state.hidden {
            return false;
        }
        state.hidden = true;
        true
    });
    if should_hide {
        let windows = STATE.with(|state| {
            state
                .borrow()
                .entries
                .iter()
                .map(|entry| (entry.window, entry.visual_window))
                .collect::<Vec<_>>()
        });
        for (window, visual_window) in windows {
            unsafe {
                let _ = ShowWindow(window, SW_HIDE);
                let _ = ShowWindow(visual_window, SW_HIDE);
            }
        }
        if let Some(shared) = STATE.with(|state| state.borrow().shared.clone()) {
            set_shared_state(&shared, true, true);
        }
    }
    respond_success(&request_id, json!({ "hidden": true }));
}

fn show_selector(request_id: String) {
    let should_show = STATE.with(|state| {
        let mut state = state.borrow_mut();
        if !state.hidden {
            return false;
        }
        state.hidden = false;
        true
    });
    if should_show {
        let windows = STATE.with(|state| {
            state
                .borrow()
                .entries
                .iter()
                .map(|entry| (entry.window, entry.visual_window))
                .collect::<Vec<_>>()
        });
        for (window, visual_window) in windows {
            unsafe {
                let _ = ShowWindow(window, SW_SHOWNOACTIVATE);
                let _ = ShowWindow(visual_window, SW_SHOWNOACTIVATE);
            }
        }
        if let Some(shared) = STATE.with(|state| state.borrow().shared.clone()) {
            set_shared_state(&shared, true, false);
        }
    }
    respond_success(&request_id, json!({ "visible": true }));
}

fn set_aspect_ratio(request_id: String, ratio: Option<f64>) {
    let changed_window = STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.aspect_ratio = ratio;
        let window = state.active_window?;
        let entry = state
            .entries
            .iter_mut()
            .find(|entry| entry.window == window)?;
        let selection = entry.selection?;
        let Some(ratio) = ratio else {
            return Some(window);
        };
        entry.selection = Some(adjust_rect_to_ratio(selection, ratio, HandlePosition::None));
        Some(window)
    });
    if let Some(window) = changed_window {
        invalidate_visual(window);
        if ratio.is_some() {
            emit_selection("updated", window);
        }
    }
    respond_success(&request_id, json!({ "updated": true }));
}

fn teardown() {
    let (entries, key_token, font, shared) = STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.active_window = None;
        state.interaction = Interaction::Idle;
        state.aspect_ratio = None;
        state.hidden = false;
        (
            std::mem::take(&mut state.entries),
            state.key_token.take(),
            state.font.take(),
            state.shared.take(),
        )
    });

    unsafe {
        let _ = ReleaseCapture();
    }
    for entry in entries {
        unsafe {
            let _ = DestroyWindow(entry.visual_window);
            let _ = DestroyWindow(entry.window);
        }
    }
    if let Some(token) = key_token {
        remove_key_handler(token);
    }
    if let Some(font) = font {
        unsafe {
            let _ = DeleteObject(font.into());
        }
    }
    if let Some(shared) = shared {
        set_shared_state(&shared, false, false);
    }
}

pub struct AreaSelectorModule {
    shared: SharedSelectorState,
}

impl AreaSelectorModule {
    pub fn new() -> Self {
        AreaSelectorModule {
            shared: Arc::new(Mutex::new(SharedState::default())),
        }
    }
}

impl Module for AreaSelectorModule {
    fn name(&self) -> &'static str {
        "area-selector"
    }

    fn handle(&mut self, request: &Request) -> Reply {
        match request.method.as_str() {
            "start" => {
                let preset = match (
                    param_i32(&request.params, "presetX"),
                    param_i32(&request.params, "presetY"),
                    param_i32(&request.params, "presetWidth"),
                    param_i32(&request.params, "presetHeight"),
                ) {
                    (Some(x), Some(y), Some(width), Some(height)) => Some(RECT {
                        left: x,
                        top: y,
                        right: x + width,
                        bottom: y + height,
                    }),
                    _ => None,
                };
                let display_id = request
                    .params
                    .as_ref()
                    .and_then(|params| params.get("displayId"))
                    .and_then(Value::as_i64);
                let params = StartParams {
                    fullscreen: param_bool(&request.params, "fullscreen").unwrap_or(false),
                    display_id,
                    preset,
                    show_prompt: param_bool(&request.params, "showPrompt").unwrap_or(true),
                    simple_style: param_str(&request.params, "style") == Some("simple"),
                };
                set_shared_state(&self.shared, true, false);
                let shared = self.shared.clone();
                let request_id = request.id.clone();
                run_on_ui(move || start_selection(params, shared, request_id));
                Reply::Deferred
            }
            "confirm" => {
                let request_id = request.id.clone();
                run_on_ui(move || confirm_selection(request_id));
                Reply::Deferred
            }
            "cancel" => {
                let request_id = request.id.clone();
                run_on_ui(move || cancel_from_request(request_id));
                Reply::Deferred
            }
            "update" => {
                let (Some(x), Some(y), Some(width), Some(height)) = (
                    param_i32(&request.params, "x"),
                    param_i32(&request.params, "y"),
                    param_i32(&request.params, "width"),
                    param_i32(&request.params, "height"),
                ) else {
                    return Reply::Now(Err((
                        "INVALID_PARAMS".to_string(),
                        "update requires x, y, width, height".to_string(),
                    )));
                };
                let request_id = request.id.clone();
                run_on_ui(move || {
                    update_selection(
                        request_id,
                        RECT {
                            left: x,
                            top: y,
                            right: x + width,
                            bottom: y + height,
                        },
                    )
                });
                Reply::Deferred
            }
            "hide" => {
                let request_id = request.id.clone();
                run_on_ui(move || hide_selector(request_id));
                Reply::Deferred
            }
            "show" => {
                let request_id = request.id.clone();
                run_on_ui(move || show_selector(request_id));
                Reply::Deferred
            }
            "status" => {
                let active = self
                    .shared
                    .lock()
                    .map(|state| state.active && !state.hidden)
                    .unwrap_or(false);
                Reply::Now(Ok(Some(json!({ "active": active }))))
            }
            "setAspectRatio" => {
                let width = param_i32(&request.params, "width").unwrap_or(0);
                let height = param_i32(&request.params, "height").unwrap_or(0);
                let ratio = (width > 0 && height > 0).then_some(width as f64 / height as f64);
                let request_id = request.id.clone();
                run_on_ui(move || set_aspect_ratio(request_id, ratio));
                Reply::Deferred
            }
            "setWindowRegion" => {
                let Some(window_handle) = param_str(&request.params, "windowHandle")
                    .and_then(|value| value.parse::<usize>().ok())
                    .filter(|value| *value != 0)
                else {
                    return Reply::Now(Err((
                        "INVALID_PARAMS".to_string(),
                        "setWindowRegion requires a windowHandle".to_string(),
                    )));
                };
                let values = (
                    param_i32(&request.params, "x"),
                    param_i32(&request.params, "y"),
                    param_i32(&request.params, "width"),
                    param_i32(&request.params, "height"),
                    param_i32(&request.params, "windowWidth"),
                    param_i32(&request.params, "windowHeight"),
                    param_i32(&request.params, "inset"),
                );
                let hole = match values {
                    (None, None, None, None, None, None, None) => None,
                    (
                        Some(x),
                        Some(y),
                        Some(width),
                        Some(height),
                        Some(window_width),
                        Some(window_height),
                        Some(inset),
                    ) if width > 0
                        && height > 0
                        && window_width > 0
                        && window_height > 0
                        && inset >= 0 =>
                    {
                        Some((x, y, width, height, window_width, window_height, inset))
                    }
                    _ => {
                        return Reply::Now(Err((
                            "INVALID_PARAMS".to_string(),
                            "setWindowRegion requires complete region dimensions".to_string(),
                        )));
                    }
                };
                let window = HWND(window_handle as *mut c_void);
                match set_electron_window_region(window, hole) {
                    Ok(()) => Reply::Now(Ok(Some(json!({ "updated": true })))),
                    Err(error) => Reply::Now(Err((
                        "WINDOW_CONFIGURATION_FAILED".to_string(),
                        error.to_string(),
                    ))),
                }
            }
            method @ ("disableWindowTransitions"
            | "hideWindowWithoutTransitions"
            | "showWindowWithoutTransitions") => {
                let Some(window_handle) = param_str(&request.params, "windowHandle")
                    .and_then(|value| value.parse::<usize>().ok())
                    .filter(|value| *value != 0)
                else {
                    return Reply::Now(Err((
                        "INVALID_PARAMS".to_string(),
                        "window transition method requires a windowHandle".to_string(),
                    )));
                };
                let no_activate = param_bool(&request.params, "noActivate").unwrap_or(true);
                let window = HWND(window_handle as *mut c_void);
                let result = if no_activate {
                    configure_overlay_window(window)
                } else {
                    disable_window_transitions(window)
                };
                let result = result.and_then(|()| match method {
                    "showWindowWithoutTransitions" => unsafe {
                        SetWindowPos(
                            window,
                            Some(HWND_TOPMOST),
                            0,
                            0,
                            0,
                            0,
                            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
                        )
                    },
                    "hideWindowWithoutTransitions" => unsafe {
                        set_electron_window_region(window, None)?;
                        SetWindowPos(
                            window,
                            None,
                            0,
                            0,
                            0,
                            0,
                            SWP_NOMOVE
                                | SWP_NOSIZE
                                | SWP_NOACTIVATE
                                | SWP_NOZORDER
                                | SWP_HIDEWINDOW,
                        )
                    },
                    _ => Ok(()),
                });
                match result {
                    Ok(()) => Reply::Now(Ok(Some(json!({ "disabled": true })))),
                    Err(error) => Reply::Now(Err((
                        "WINDOW_CONFIGURATION_FAILED".to_string(),
                        error.to_string(),
                    ))),
                }
            }
            method => method_not_found(method),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::scaled_hole_rect;

    #[test]
    fn maps_the_inset_hole_when_client_matches_the_window() {
        assert_eq!(
            scaled_hole_rect((100, 50, 400, 300, 1920, 1080, 16), 1920, 1080),
            Some((116, 66, 484, 334))
        );
    }

    #[test]
    fn scales_the_hole_to_the_physical_client_area() {
        assert_eq!(
            scaled_hole_rect((100, 50, 400, 300, 1920, 1080, 16), 3840, 2160),
            Some((232, 132, 968, 668))
        );
    }

    #[test]
    fn rounds_fractional_scaling_to_the_nearest_pixel() {
        assert_eq!(
            scaled_hole_rect((100, 100, 400, 400, 1000, 1000, 0), 1250, 1250),
            Some((125, 125, 625, 625))
        );
    }

    #[test]
    fn reports_no_hole_when_the_inset_swallows_the_selection() {
        assert_eq!(
            scaled_hole_rect((0, 0, 32, 300, 1920, 1080, 16), 1920, 1080),
            None
        );
        assert_eq!(
            scaled_hole_rect((0, 0, 400, 20, 1920, 1080, 16), 1920, 1080),
            None
        );
    }
}
