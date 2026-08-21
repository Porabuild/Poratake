use crate::overlay::{
    WM_MOUSELEAVE, add_key_handler, apply_round_region, create_popup_window, create_ui_font,
    default_wndproc, ensure_window_class, monitors, point_from_lparam, rects_intersect,
    remove_key_handler, scale_for_dpi,
};
use crate::panel::{
    ACTIVE_BUTTON, BUTTON_TEXT, BUTTON_TEXT_ON_FILL, NEUTRAL_BUTTON, PANEL_ALPHA,
    PANEL_BUTTON_RADIUS, PANEL_CORNER_RADIUS, PANEL_FONT_SIZE, PANEL_FONT_WEIGHT, PRIMARY_BUTTON,
    button_at, button_fill, button_rect, button_state, draw_label, draw_pill, paint_buffered,
    panel_height, panel_width,
};
use crate::protocol::{Request, param_i32, param_str, respond_error, respond_success, send_event};
use crate::router::{Module, Reply, method_not_found};
use crate::ui::run_on_ui;
use serde_json::json;
use std::cell::RefCell;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::sync::{Arc, Mutex, OnceLock};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BeginPaint, BitBlt, CAPTUREBLT, CombineRgn,
    CreateCompatibleBitmap, CreateCompatibleDC, CreateRectRgn, CreateSolidBrush, DIB_RGB_COLORS,
    DeleteDC, DeleteObject, EndPaint, FillRect, FrameRect, GetDC, GetDIBits, HFONT, InvalidateRect,
    PAINTSTRUCT, RGN_DIFF, ReleaseDC, SRCCOPY, SelectObject, SetWindowRgn,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_WHEEL, MOUSEINPUT, SendInput, TME_LEAVE,
    TRACKMOUSEEVENT, TrackMouseEvent, VK_ESCAPE, VK_RETURN,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DestroyWindow, GetClientRect, GetCursorPos, GetWindowRect, HWND_TOPMOST, IDC_ARROW,
    IsWindowVisible, KillTimer, LWA_ALPHA, LWA_COLORKEY, LoadCursorW, SW_HIDE, SW_SHOWNOACTIVATE,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetCursorPos, SetLayeredWindowAttributes, SetTimer,
    SetWindowDisplayAffinity, SetWindowPos, ShowWindow, WDA_EXCLUDEFROMCAPTURE, WM_ERASEBKGND,
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_PAINT, WM_TIMER, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT,
};

const BOUNDARY_CLASS_NAME: &str = "PoratakeScrollCaptureBoundary";
const PANEL_CLASS_NAME: &str = "PoratakeScrollCapturePanel";
const PANEL_BUTTON_WIDTHS: [i32; 3] = [76, 76, 76];
const PANEL_GAP: i32 = 16;
const BOUNDARY_THICKNESS: i32 = 2;
const COLOR_KEY: COLORREF = COLORREF(0);
const BOUNDARY_COLOR: COLORREF = COLORREF(0x00FF7A00);
const AUTO_SCROLL_TIMER_ID: usize = 1;
const FRAME_OVERLAP_PERCENT: i32 = 30;
const MAX_DUPLICATE_FRAMES: usize = 3;
const WHEEL_DELTA: i32 = -120;
const WHEEL_LOGICAL_PIXELS: f64 = 48.0;
const MAX_OVERLAP_CHANNEL_DIFFERENCE: u64 = 18;

#[derive(Clone, Copy)]
struct CaptureBounds {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

impl CaptureBounds {
    fn rect(self) -> RECT {
        RECT {
            left: self.x,
            top: self.y,
            right: self.x + self.width,
            bottom: self.y + self.height,
        }
    }

    fn contains(self, point: POINT) -> bool {
        point.x >= self.x
            && point.x < self.x + self.width
            && point.y >= self.y
            && point.y < self.y + self.height
    }
}

#[derive(Clone, Copy)]
enum ScrollSpeed {
    Slow,
    Medium,
    Fast,
}

impl ScrollSpeed {
    fn parse(value: Option<&str>) -> Self {
        match value {
            Some("slow") => Self::Slow,
            Some("fast") => Self::Fast,
            _ => Self::Medium,
        }
    }

    fn interval(self) -> u32 {
        match self {
            Self::Slow => 40,
            Self::Medium => 30,
            Self::Fast => 20,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct ScrollPlan {
    target_logical_points: usize,
    wheel_detents: usize,
    interval: u32,
}

fn scroll_plan(logical_viewport_height: f64, speed: ScrollSpeed) -> ScrollPlan {
    let target_logical_points = (logical_viewport_height * (100 - FRAME_OVERLAP_PERCENT) as f64
        / 100.0)
        .round()
        .max(1.0) as usize;
    let wheel_detents =
        ((target_logical_points as f64 / WHEEL_LOGICAL_PIXELS).ceil() as usize).max(1);

    ScrollPlan {
        target_logical_points,
        wheel_detents,
        interval: speed.interval(),
    }
}

fn logical_height(physical_height: usize, scale_factor: f64) -> usize {
    (physical_height as f64 / scale_factor).ceil() as usize
}

fn dpi_for_scale_factor(scale_factor: f64) -> u32 {
    (96.0 * scale_factor).round().max(96.0) as u32
}

struct CapturedFrame {
    width: usize,
    height: usize,
    pixels: Vec<u8>,
    overlap: usize,
}

enum CaptureOutcome {
    Added,
    Repeated,
    Ended,
    NoOverlap,
}

struct CaptureState {
    bounds: Option<CaptureBounds>,
    frames: Vec<CapturedFrame>,
    is_capturing: bool,
    is_auto_scrolling: bool,
    done_requested: bool,
    auto_scroll_speed: ScrollSpeed,
    max_height: usize,
    scale_factor: f64,
    last_frame_hash: Option<u64>,
    duplicate_frame_count: usize,
    scroll_step_points: usize,
    scroll_steps_per_frame: usize,
    current_scroll_step: usize,
    capture_on_next_tick: bool,
    estimated_height: usize,
}

impl Default for CaptureState {
    fn default() -> Self {
        Self {
            bounds: None,
            frames: Vec::new(),
            is_capturing: false,
            is_auto_scrolling: false,
            done_requested: false,
            auto_scroll_speed: ScrollSpeed::Medium,
            max_height: 20_000,
            scale_factor: 1.0,
            last_frame_hash: None,
            duplicate_frame_count: 0,
            scroll_step_points: 0,
            scroll_steps_per_frame: 0,
            current_scroll_step: 0,
            capture_on_next_tick: false,
            estimated_height: 0,
        }
    }
}

impl CaptureState {
    fn stop_auto_scroll(&mut self) {
        self.is_auto_scrolling = false;
        self.current_scroll_step = 0;
        self.capture_on_next_tick = false;
    }

    fn reset(&mut self) {
        *self = Self::default();
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PanelButton {
    Auto,
    Done,
    Cancel,
}

const PANEL_BUTTONS: [PanelButton; 3] = [PanelButton::Auto, PanelButton::Done, PanelButton::Cancel];

struct ScrollUiState {
    boundary: Option<HWND>,
    panel: Option<HWND>,
    font: Option<HFONT>,
    dpi: u32,
    hovered: Option<usize>,
    pressed: Option<usize>,
    escape_token: Option<usize>,
    enter_token: Option<usize>,
    capture_state: Option<Arc<Mutex<CaptureState>>>,
}

thread_local! {
    static UI_STATE: RefCell<ScrollUiState> = RefCell::new(ScrollUiState {
        boundary: None,
        panel: None,
        font: None,
        dpi: 96,
        hovered: None,
        pressed: None,
        escape_token: None,
        enter_token: None,
        capture_state: None,
    });
}

unsafe extern "system" fn boundary_wndproc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_PAINT => {
            paint_boundary(window);
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        _ => default_wndproc(window, message, wparam, lparam),
    }
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
            track_panel_hover(window, point_from_lparam(lparam));
            LRESULT(0)
        }
        WM_MOUSELEAVE => {
            clear_panel_hover(window);
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            press_panel_button(window, point_from_lparam(lparam));
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            release_panel_button(window, point_from_lparam(lparam));
            LRESULT(0)
        }
        WM_TIMER => {
            if wparam.0 == AUTO_SCROLL_TIMER_ID {
                auto_scroll_tick();
            }
            LRESULT(0)
        }
        _ => default_wndproc(window, message, wparam, lparam),
    }
}

fn client_rect(window: HWND) -> RECT {
    let mut client = RECT::default();
    unsafe {
        let _ = GetClientRect(window, &mut client);
    }
    client
}

fn apply_ring_region(window: HWND, thickness: i32) {
    let client = client_rect(window);

    unsafe {
        let ring = CreateRectRgn(0, 0, client.right, client.bottom);
        let hole = CreateRectRgn(
            thickness,
            thickness,
            client.right - thickness,
            client.bottom - thickness,
        );
        CombineRgn(Some(ring), Some(ring), Some(hole), RGN_DIFF);
        let _ = DeleteObject(hole.into());
        let _ = SetWindowRgn(window, Some(ring), true);
    }
}

fn panel_button_label(index: usize, auto_scrolling: bool) -> &'static str {
    match PANEL_BUTTONS[index] {
        PanelButton::Auto if auto_scrolling => "Stop",
        PanelButton::Auto => "Auto",
        PanelButton::Done => "Done",
        PanelButton::Cancel => "Cancel",
    }
}

fn panel_button_palette(index: usize, auto_scrolling: bool) -> [COLORREF; 3] {
    match PANEL_BUTTONS[index] {
        PanelButton::Auto if auto_scrolling => ACTIVE_BUTTON,
        PanelButton::Done => PRIMARY_BUTTON,
        _ => NEUTRAL_BUTTON,
    }
}

fn panel_button_text(index: usize, auto_scrolling: bool) -> COLORREF {
    match PANEL_BUTTONS[index] {
        PanelButton::Auto if auto_scrolling => BUTTON_TEXT_ON_FILL,
        PanelButton::Done => BUTTON_TEXT_ON_FILL,
        _ => BUTTON_TEXT,
    }
}

fn is_auto_scrolling() -> bool {
    UI_STATE
        .with(|ui| ui.borrow().capture_state.clone())
        .and_then(|state| state.lock().ok().map(|state| state.is_auto_scrolling))
        .unwrap_or(false)
}

fn paint_boundary(window: HWND) {
    let client = client_rect(window);
    let thickness = UI_STATE.with(|ui| scale_for_dpi(BOUNDARY_THICKNESS, ui.borrow().dpi));

    unsafe {
        let mut paint_struct = PAINTSTRUCT::default();
        let dc = BeginPaint(window, &mut paint_struct);
        let transparent = CreateSolidBrush(COLOR_KEY);
        let accent = CreateSolidBrush(BOUNDARY_COLOR);
        FillRect(dc, &paint_struct.rcPaint, transparent);

        let mut edge = client;
        for _ in 0..thickness.max(1) {
            if edge.left >= edge.right || edge.top >= edge.bottom {
                break;
            }
            FrameRect(dc, &edge, accent);
            edge.left += 1;
            edge.top += 1;
            edge.right -= 1;
            edge.bottom -= 1;
        }

        let _ = DeleteObject(transparent.into());
        let _ = DeleteObject(accent.into());
        let _ = EndPaint(window, &paint_struct);
    }
}

fn paint_panel(window: HWND) {
    let (font, dpi, hovered, pressed) = UI_STATE.with(|ui| {
        let ui = ui.borrow();
        (ui.font, ui.dpi, ui.hovered, ui.pressed)
    });
    let auto_scrolling = is_auto_scrolling();

    paint_buffered(window, font, |dc, _| {
        for index in 0..PANEL_BUTTONS.len() {
            let rect = button_rect(&PANEL_BUTTON_WIDTHS, index, dpi);
            let state = button_state(hovered == Some(index), pressed == Some(index));
            draw_pill(
                dc,
                rect,
                scale_for_dpi(PANEL_BUTTON_RADIUS, dpi),
                button_fill(panel_button_palette(index, auto_scrolling), state),
            );
            draw_label(
                dc,
                panel_button_label(index, auto_scrolling),
                rect,
                panel_button_text(index, auto_scrolling),
            );
        }
    });
}

fn repaint_panel(window: HWND) {
    unsafe {
        let _ = InvalidateRect(Some(window), None, false);
    }
}

fn track_panel_hover(window: HWND, point: POINT) {
    let hovered = UI_STATE.with(|ui| button_at(&PANEL_BUTTON_WIDTHS, point, ui.borrow().dpi));
    let changed = UI_STATE.with(|ui| {
        let mut ui = ui.borrow_mut();
        if ui.hovered == hovered {
            return false;
        }
        ui.hovered = hovered;
        true
    });

    let mut tracking = TRACKMOUSEEVENT {
        cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
        dwFlags: TME_LEAVE,
        hwndTrack: window,
        dwHoverTime: 0,
    };
    unsafe {
        let _ = TrackMouseEvent(&mut tracking);
    }

    if changed {
        repaint_panel(window);
    }
}

fn clear_panel_hover(window: HWND) {
    let changed = UI_STATE.with(|ui| {
        let mut ui = ui.borrow_mut();
        ui.pressed = None;
        ui.hovered.take().is_some()
    });

    if changed {
        repaint_panel(window);
    }
}

fn press_panel_button(window: HWND, point: POINT) {
    let pressed = UI_STATE.with(|ui| {
        let mut ui = ui.borrow_mut();
        ui.pressed = button_at(&PANEL_BUTTON_WIDTHS, point, ui.dpi);
        ui.pressed
    });

    if pressed.is_some() {
        repaint_panel(window);
    }
}

fn release_panel_button(window: HWND, point: POINT) {
    let (pressed, released) = UI_STATE.with(|ui| {
        let mut ui = ui.borrow_mut();
        let released = button_at(&PANEL_BUTTON_WIDTHS, point, ui.dpi);
        (ui.pressed.take(), released)
    });

    let Some(pressed) = pressed else {
        return;
    };

    repaint_panel(window);

    if released != Some(pressed) {
        return;
    }

    match PANEL_BUTTONS[pressed] {
        PanelButton::Auto => toggle_auto_scroll_from_ui(),
        PanelButton::Done => done_from_ui(),
        PanelButton::Cancel => cancel_from_ui(),
    }
}

fn toggle_auto_scroll_from_ui() {
    let state = UI_STATE.with(|ui| ui.borrow().capture_state.clone());
    let Some(state) = state else {
        return;
    };

    let auto_scrolling = state
        .lock()
        .ok()
        .map(|state| state.is_auto_scrolling)
        .unwrap_or(false);

    if auto_scrolling {
        stop_auto_scroll_ui(&state);
        return;
    }

    let _ = start_auto_scroll_ui(&state);
}

fn done_from_ui() {
    let state = UI_STATE.with(|ui| ui.borrow().capture_state.clone());
    let Some(state) = state else {
        return;
    };

    let should_emit = state
        .lock()
        .ok()
        .map(|mut state| {
            if !state.is_capturing || state.done_requested {
                return false;
            }
            state.done_requested = true;
            true
        })
        .unwrap_or(false);
    if !should_emit {
        return;
    }

    stop_auto_scroll_ui(&state);
    send_event("scroll-capture:done", None);
}

fn cancel_from_ui() {
    let state = UI_STATE.with(|ui| ui.borrow().capture_state.clone());
    let Some(state) = state else {
        return;
    };

    let was_capturing = state
        .lock()
        .ok()
        .map(|mut state| {
            let was_capturing = state.is_capturing;
            state.reset();
            was_capturing
        })
        .unwrap_or(false);

    teardown_ui();
    if was_capturing {
        send_event("scroll-capture:cancelled", None);
    }
}

fn show_capture_ui(
    state: Arc<Mutex<CaptureState>>,
    bounds: CaptureBounds,
    dpi: u32,
) -> Result<(), String> {
    teardown_ui();
    ensure_window_class(BOUNDARY_CLASS_NAME, Some(boundary_wndproc), None);
    let arrow = unsafe { LoadCursorW(None, IDC_ARROW) }.ok();
    ensure_window_class(PANEL_CLASS_NAME, Some(panel_wndproc), arrow);

    UI_STATE.with(|ui| {
        ui.borrow_mut().dpi = dpi.max(96);
    });

    let thickness = scale_for_dpi(BOUNDARY_THICKNESS, dpi);
    let boundary_rect = RECT {
        left: bounds.x - thickness,
        top: bounds.y - thickness,
        right: bounds.x + bounds.width + thickness,
        bottom: bounds.y + bounds.height + thickness,
    };
    let boundary_style =
        WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED | WS_EX_TRANSPARENT;
    let Some(boundary) = create_popup_window(BOUNDARY_CLASS_NAME, boundary_style, &boundary_rect)
    else {
        return Err("Failed to create the scroll capture boundary".to_string());
    };

    apply_ring_region(boundary, thickness);

    let boundary_setup = unsafe {
        let layered = SetLayeredWindowAttributes(boundary, COLOR_KEY, 255, LWA_COLORKEY);
        if layered.is_ok() {
            let _ = SetWindowDisplayAffinity(boundary, WDA_EXCLUDEFROMCAPTURE);
            let _ = ShowWindow(boundary, SW_SHOWNOACTIVATE);
            SetWindowPos(
                boundary,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            )
        } else {
            layered
        }
    };
    if boundary_setup.is_err() {
        unsafe {
            let _ = DestroyWindow(boundary);
        }
        return Err("Failed to configure the scroll capture boundary".to_string());
    }

    let panel_rect = capture_panel_rect(bounds, dpi);
    let panel_style = WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED;
    let Some(panel) = create_popup_window(PANEL_CLASS_NAME, panel_style, &panel_rect) else {
        unsafe {
            let _ = DestroyWindow(boundary);
        }
        return Err("Failed to create the scroll capture controls".to_string());
    };

    apply_round_region(panel, scale_for_dpi(PANEL_CORNER_RADIUS, dpi));

    let panel_setup = unsafe {
        let layered = SetLayeredWindowAttributes(panel, COLOR_KEY, PANEL_ALPHA, LWA_ALPHA);
        if layered.is_ok() {
            let _ = SetWindowDisplayAffinity(panel, WDA_EXCLUDEFROMCAPTURE);
            let _ = ShowWindow(panel, SW_SHOWNOACTIVATE);
            SetWindowPos(
                panel,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            )
        } else {
            layered
        }
    };
    if panel_setup.is_err() {
        unsafe {
            let _ = DestroyWindow(panel);
            let _ = DestroyWindow(boundary);
        }
        return Err("Failed to configure the scroll capture controls".to_string());
    }

    UI_STATE.with(|ui| {
        let mut ui = ui.borrow_mut();
        ui.boundary = Some(boundary);
        ui.panel = Some(panel);
        ui.font = Some(create_ui_font(dpi, PANEL_FONT_SIZE, PANEL_FONT_WEIGHT));
        ui.capture_state = Some(state);
    });

    let escape_token = match add_key_handler(VK_ESCAPE.0 as u32, cancel_from_ui) {
        Ok(token) => token,
        Err(message) => {
            teardown_ui();
            return Err(message);
        }
    };
    UI_STATE.with(|ui| {
        ui.borrow_mut().escape_token = Some(escape_token);
    });
    let enter_token = match add_key_handler(VK_RETURN.0 as u32, done_from_ui) {
        Ok(token) => token,
        Err(message) => {
            teardown_ui();
            return Err(message);
        }
    };
    UI_STATE.with(|ui| {
        let mut ui = ui.borrow_mut();
        ui.enter_token = Some(enter_token);
    });

    Ok(())
}

fn capture_panel_rect(bounds: CaptureBounds, dpi: u32) -> RECT {
    let capture_rect = bounds.rect();
    let monitor = monitors()
        .into_iter()
        .find(|monitor| rects_intersect(&capture_rect, &monitor.rect))
        .map(|monitor| monitor.rect)
        .unwrap_or(capture_rect);

    let width = scale_for_dpi(panel_width(&PANEL_BUTTON_WIDTHS), dpi);
    let height = scale_for_dpi(panel_height(), dpi);
    let gap = scale_for_dpi(PANEL_GAP, dpi);
    let min_x = monitor.left;
    let max_x = (monitor.right - width).max(min_x);
    let x = (bounds.x + (bounds.width - width) / 2).clamp(min_x, max_x);
    let below = bounds.y + bounds.height + gap;
    let y = if below + height <= monitor.bottom {
        below
    } else {
        (bounds.y - height - gap).max(monitor.top)
    };

    RECT {
        left: x,
        top: y,
        right: x + width,
        bottom: y + height,
    }
}

fn teardown_ui() {
    let (boundary, panel, font, escape_token, enter_token) = UI_STATE.with(|ui| {
        let mut ui = ui.borrow_mut();
        ui.capture_state = None;
        ui.hovered = None;
        ui.pressed = None;
        (
            ui.boundary.take(),
            ui.panel.take(),
            ui.font.take(),
            ui.escape_token.take(),
            ui.enter_token.take(),
        )
    });

    if let Some(panel) = panel {
        unsafe {
            let _ = KillTimer(Some(panel), AUTO_SCROLL_TIMER_ID);
            let _ = DestroyWindow(panel);
        }
    }

    if let Some(boundary) = boundary {
        unsafe {
            let _ = DestroyWindow(boundary);
        }
    }

    if let Some(font) = font {
        unsafe {
            let _ = DeleteObject(font.into());
        }
    }

    if let Some(token) = escape_token {
        remove_key_handler(token);
    }
    if let Some(token) = enter_token {
        remove_key_handler(token);
    }
}

fn start_auto_scroll_ui(state: &Arc<Mutex<CaptureState>>) -> Result<(), (String, String)> {
    let running_interval = {
        let state = state.lock().map_err(|_| {
            (
                "INTERNAL_ERROR".to_string(),
                "Scroll capture state poisoned".to_string(),
            )
        })?;
        state
            .is_auto_scrolling
            .then_some(state.auto_scroll_speed.interval())
    };

    if let Some(interval) = running_interval {
        let panel = UI_STATE.with(|ui| ui.borrow().panel);
        let Some(panel) = panel else {
            stop_auto_scroll_ui(state);
            return Err((
                "UI_ERROR".to_string(),
                "Scroll capture controls are unavailable".to_string(),
            ));
        };
        unsafe {
            if SetTimer(Some(panel), AUTO_SCROLL_TIMER_ID, interval, None) == 0 {
                stop_auto_scroll_ui(state);
                return Err((
                    "UI_ERROR".to_string(),
                    "Failed to update the auto-scroll timer".to_string(),
                ));
            }
        }
        return Ok(());
    }

    let (bounds, interval) = {
        let mut state = state.lock().map_err(|_| {
            (
                "INTERNAL_ERROR".to_string(),
                "Scroll capture state poisoned".to_string(),
            )
        })?;

        if !state.is_capturing {
            return Err((
                "NOT_CAPTURING".to_string(),
                "Not in capture mode".to_string(),
            ));
        }

        let Some(bounds) = state.bounds else {
            return Err((
                "NOT_CAPTURING".to_string(),
                "Not in capture mode".to_string(),
            ));
        };

        state.is_auto_scrolling = true;
        state.current_scroll_step = 0;
        state.capture_on_next_tick = false;
        let plan = scroll_plan(
            bounds.height as f64 / state.scale_factor,
            state.auto_scroll_speed,
        );
        state.scroll_step_points =
            (plan.target_logical_points as f64 * state.scale_factor).round() as usize;
        state.scroll_steps_per_frame = plan.wheel_detents;

        match capture_current_frame(&mut state) {
            Ok(CaptureOutcome::Added | CaptureOutcome::Repeated) => {}
            Ok(CaptureOutcome::Ended) => {
                state.stop_auto_scroll();
                return Err((
                    "SCROLL_ENDED".to_string(),
                    "Scrollable content has ended".to_string(),
                ));
            }
            Ok(CaptureOutcome::NoOverlap) => {
                state.stop_auto_scroll();
                return Err((
                    "NO_OVERLAP".to_string(),
                    "The captured frame does not overlap the previous frame".to_string(),
                ));
            }
            Err(error) => {
                state.stop_auto_scroll();
                return Err(("CAPTURE_FAILED".to_string(), error));
            }
        }

        (bounds, plan.interval)
    };

    unsafe {
        let _ = SetCursorPos(bounds.x + bounds.width / 2, bounds.y + bounds.height / 2);
    }

    let panel = UI_STATE.with(|ui| ui.borrow().panel);
    let Some(panel) = panel else {
        if let Ok(mut state) = state.lock() {
            state.stop_auto_scroll();
        }
        return Err((
            "UI_ERROR".to_string(),
            "Scroll capture controls are unavailable".to_string(),
        ));
    };

    unsafe {
        if SetTimer(Some(panel), AUTO_SCROLL_TIMER_ID, interval, None) == 0 {
            if let Ok(mut state) = state.lock() {
                state.stop_auto_scroll();
            }
            return Err((
                "UI_ERROR".to_string(),
                "Failed to start the auto-scroll timer".to_string(),
            ));
        }
        let _ = InvalidateRect(Some(panel), None, false);
    }

    Ok(())
}

fn stop_auto_scroll_ui(state: &Arc<Mutex<CaptureState>>) {
    if let Ok(mut state) = state.lock() {
        state.stop_auto_scroll();
    }

    let panel = UI_STATE.with(|ui| ui.borrow().panel);
    if let Some(panel) = panel {
        unsafe {
            let _ = KillTimer(Some(panel), AUTO_SCROLL_TIMER_ID);
            let _ = InvalidateRect(Some(panel), None, false);
        }
    }
}

fn auto_scroll_tick() {
    let state = UI_STATE.with(|ui| ui.borrow().capture_state.clone());
    let Some(state) = state else {
        return;
    };

    let mut event = None;
    let mut stop = false;

    if let Ok(mut state) = state.lock() {
        if !state.is_capturing || !state.is_auto_scrolling {
            return;
        }

        let Some(bounds) = state.bounds else {
            return;
        };

        if state.capture_on_next_tick {
            state.capture_on_next_tick = false;
            match capture_current_frame(&mut state) {
                Ok(CaptureOutcome::Added) => {
                    let estimated_height =
                        logical_height(state.estimated_height, state.scale_factor);
                    if estimated_height >= state.max_height {
                        stop = true;
                        event = Some(json!({
                            "reason": "max-height",
                            "frameCount": state.frames.len(),
                            "estimatedHeight": estimated_height,
                        }));
                    } else {
                        send_event(
                            "scroll-capture:frame-captured",
                            Some(json!({
                                "frameCount": state.frames.len(),
                                "estimatedHeight": estimated_height,
                            })),
                        );
                    }
                }
                Ok(CaptureOutcome::Repeated) => {}
                Ok(CaptureOutcome::Ended) => {
                    stop = true;
                    event = Some(json!({
                        "reason": "duplicate",
                        "frameCount": state.frames.len(),
                    }));
                }
                Ok(CaptureOutcome::NoOverlap) => {
                    stop = true;
                    event = Some(json!({
                        "reason": "no-overlap",
                        "frameCount": state.frames.len(),
                    }));
                }
                Err(message) => {
                    stop = true;
                    event = Some(json!({
                        "reason": "capture-error",
                        "message": message,
                        "frameCount": state.frames.len(),
                    }));
                }
            }
        } else {
            let mut cursor = POINT::default();
            let cursor_inside =
                unsafe { GetCursorPos(&mut cursor).is_ok() } && bounds.contains(cursor);
            if !cursor_inside {
                return;
            }

            if !inject_scroll_wheel() {
                stop = true;
                event = Some(json!({
                    "reason": "input-error",
                    "frameCount": state.frames.len(),
                }));
            } else {
                state.current_scroll_step += 1;
                if state.current_scroll_step >= state.scroll_steps_per_frame {
                    state.current_scroll_step = 0;
                    state.capture_on_next_tick = true;
                }
            }
        }

        if stop {
            state.stop_auto_scroll();
        }
    }

    if stop {
        let panel = UI_STATE.with(|ui| ui.borrow().panel);
        if let Some(panel) = panel {
            unsafe {
                let _ = KillTimer(Some(panel), AUTO_SCROLL_TIMER_ID);
                let _ = InvalidateRect(Some(panel), None, false);
            }
        }
        send_event("scroll-capture:scroll-ended", event);
    }
}

fn inject_scroll_wheel() -> bool {
    let input = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                mouseData: WHEEL_DELTA as u32,
                dwFlags: MOUSEEVENTF_WHEEL,
                ..Default::default()
            },
        },
    };

    unsafe { SendInput(&[input], std::mem::size_of::<INPUT>() as i32) == 1 }
}

fn with_hidden_capture_windows<T, E>(
    visibility: [bool; 2],
    mut set_visible: impl FnMut(usize, bool),
    capture: impl FnOnce() -> Result<T, E>,
) -> Result<T, E> {
    for (index, visible) in visibility.into_iter().enumerate() {
        if visible {
            set_visible(index, false);
        }
    }

    let result = capture();

    for (index, visible) in visibility.into_iter().enumerate() {
        if visible {
            set_visible(index, true);
        }
    }

    result
}

fn windows_to_hide(visible: [bool; 2], covers_capture: [bool; 2]) -> [bool; 2] {
    [
        visible[0] && covers_capture[0],
        visible[1] && covers_capture[1],
    ]
}

fn window_rect(window: HWND) -> RECT {
    let mut rect = RECT::default();
    unsafe {
        let _ = GetWindowRect(window, &mut rect);
    }
    rect
}

fn contains_rect(outer: &RECT, inner: &RECT) -> bool {
    outer.left <= inner.left
        && outer.top <= inner.top
        && outer.right >= inner.right
        && outer.bottom >= inner.bottom
}

fn ring_covers_rect(ring: &RECT, thickness: i32, capture: &RECT) -> bool {
    let hole = RECT {
        left: ring.left + thickness,
        top: ring.top + thickness,
        right: ring.right - thickness,
        bottom: ring.bottom - thickness,
    };

    !contains_rect(&hole, capture)
}

fn capture_pixels_without_ui(bounds: CaptureBounds) -> Result<Vec<u8>, String> {
    let (windows, thickness) = UI_STATE.with(|ui| {
        let ui = ui.borrow();
        (
            [ui.boundary, ui.panel],
            scale_for_dpi(BOUNDARY_THICKNESS, ui.dpi),
        )
    });
    let capture_rect = bounds.rect();
    let visible = windows
        .map(|window| window.is_some_and(|window| unsafe { IsWindowVisible(window).as_bool() }));
    let covers_capture = [
        windows[0]
            .is_some_and(|window| ring_covers_rect(&window_rect(window), thickness, &capture_rect)),
        windows[1].is_some_and(|window| rects_intersect(&window_rect(window), &capture_rect)),
    ];

    with_hidden_capture_windows(
        windows_to_hide(visible, covers_capture),
        |index, visible| {
            let Some(window) = windows[index] else {
                return;
            };
            let command = if visible { SW_SHOWNOACTIVATE } else { SW_HIDE };
            unsafe {
                let _ = ShowWindow(window, command);
            }
        },
        || capture_pixels(bounds),
    )
}

fn capture_current_frame(state: &mut CaptureState) -> Result<CaptureOutcome, String> {
    let bounds = state
        .bounds
        .ok_or_else(|| "Capture bounds are unavailable".to_string())?;
    let pixels = capture_pixels_without_ui(bounds)?;
    let hash = frame_hash(&pixels, bounds.width as usize, bounds.height as usize);

    if state.last_frame_hash == Some(hash) {
        state.duplicate_frame_count += 1;
        if state.duplicate_frame_count >= MAX_DUPLICATE_FRAMES {
            return Ok(CaptureOutcome::Ended);
        }
        return Ok(CaptureOutcome::Repeated);
    }

    let width = bounds.width as usize;
    let height = bounds.height as usize;
    let overlap = if let Some(previous) = state.frames.last() {
        let expected = height
            .saturating_sub(state.scroll_step_points)
            .clamp(1, height);
        let Some(overlap) = find_overlap(previous, &pixels, width, height, expected) else {
            return Ok(CaptureOutcome::NoOverlap);
        };
        overlap
    } else {
        0
    };
    if !state.frames.is_empty() && overlap == height {
        state.duplicate_frame_count += 1;
        if state.duplicate_frame_count >= MAX_DUPLICATE_FRAMES {
            return Ok(CaptureOutcome::Ended);
        }
        return Ok(CaptureOutcome::Repeated);
    }
    let new_content_height = height.saturating_sub(overlap);

    state.duplicate_frame_count = 0;
    state.last_frame_hash = Some(hash);

    if state.frames.is_empty() {
        state.estimated_height = height;
    } else {
        state.estimated_height = state.estimated_height.saturating_add(new_content_height);
    }

    state.frames.push(CapturedFrame {
        width,
        height,
        pixels,
        overlap,
    });

    Ok(CaptureOutcome::Added)
}

fn capture_pixels(bounds: CaptureBounds) -> Result<Vec<u8>, String> {
    let width = bounds.width;
    let height = bounds.height;
    let pixel_count = (width as usize)
        .checked_mul(height as usize)
        .and_then(|count| count.checked_mul(4))
        .ok_or_else(|| "Capture dimensions are too large".to_string())?;

    unsafe {
        let screen_dc = GetDC(None);
        if screen_dc.is_invalid() {
            return Err("Failed to acquire the screen device context".to_string());
        }

        let memory_dc = CreateCompatibleDC(Some(screen_dc));
        if memory_dc.is_invalid() {
            ReleaseDC(None, screen_dc);
            return Err("Failed to create a capture device context".to_string());
        }

        let bitmap = CreateCompatibleBitmap(screen_dc, width, height);
        if bitmap.is_invalid() {
            let _ = DeleteDC(memory_dc);
            ReleaseDC(None, screen_dc);
            return Err("Failed to create a capture bitmap".to_string());
        }

        let previous = SelectObject(memory_dc, bitmap.into());
        let copied = BitBlt(
            memory_dc,
            0,
            0,
            width,
            height,
            Some(screen_dc),
            bounds.x,
            bounds.y,
            SRCCOPY | CAPTUREBLT,
        );
        SelectObject(memory_dc, previous);

        let mut bitmap_info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut pixels = vec![0_u8; pixel_count];
        let rows = if copied.is_ok() {
            GetDIBits(
                memory_dc,
                bitmap,
                0,
                height as u32,
                Some(pixels.as_mut_ptr().cast()),
                &mut bitmap_info,
                DIB_RGB_COLORS,
            )
        } else {
            0
        };

        let _ = DeleteObject(bitmap.into());
        let _ = DeleteDC(memory_dc);
        ReleaseDC(None, screen_dc);

        if rows != height {
            return Err("Failed to copy pixels from the screen".to_string());
        }

        for pixel in pixels.chunks_exact_mut(4) {
            pixel.swap(0, 2);
            pixel[3] = 255;
        }

        Ok(pixels)
    }
}

fn frame_hash(pixels: &[u8], width: usize, height: usize) -> u64 {
    let sample_width = 100_usize.min(width);
    let sample_height = 50_usize.min(height);
    let sample_top = height.saturating_sub((height / 4).max(sample_height));
    let sample_region_height = height - sample_top;
    let mut hash = 0xcbf29ce484222325_u64;

    for sample_y in 0..sample_height {
        let y = sample_top + sample_y * sample_region_height / sample_height;
        for sample_x in 0..sample_width {
            let x = sample_x * width / sample_width;
            let offset = (y * width + x) * 4;
            for channel in &pixels[offset..offset + 3] {
                hash ^= *channel as u64;
                hash = hash.wrapping_mul(0x100000001b3);
            }
        }
    }

    hash
}

fn find_overlap(
    previous: &CapturedFrame,
    current_pixels: &[u8],
    width: usize,
    height: usize,
    expected: usize,
) -> Option<usize> {
    let strip_height = 40_usize.min(height / 2);
    if strip_height == 0 || previous.width != width || previous.height != height {
        return None;
    }

    let min_overlap = strip_height;
    let max_overlap = height;
    let strip_width = width.min(800);
    let start_x = (width - strip_width) / 2;
    let mut best_overlap = expected.clamp(min_overlap, max_overlap);
    let mut best_score = u64::MAX;

    let mut overlap = min_overlap;
    while overlap <= max_overlap {
        let score = compare_overlap_strip(
            &previous.pixels,
            current_pixels,
            width,
            height - strip_height,
            overlap - strip_height,
            start_x,
            strip_width,
            strip_height,
        );
        if score < best_score
            || (score == best_score && overlap.abs_diff(expected) < best_overlap.abs_diff(expected))
        {
            best_score = score;
            best_overlap = overlap;
        }
        overlap = overlap.saturating_add(4);
        if overlap == usize::MAX {
            break;
        }
    }

    let fine_start = best_overlap.saturating_sub(3).max(min_overlap);
    let fine_end = (best_overlap + 3).min(max_overlap);
    for overlap in fine_start..=fine_end {
        let score = compare_overlap_strip(
            &previous.pixels,
            current_pixels,
            width,
            height - strip_height,
            overlap - strip_height,
            start_x,
            strip_width,
            strip_height,
        );
        if score < best_score
            || (score == best_score && overlap.abs_diff(expected) < best_overlap.abs_diff(expected))
        {
            best_score = score;
            best_overlap = overlap;
        }
    }

    let sampled_channels = strip_width.div_ceil(2) * strip_height.div_ceil(2) * 3;
    if best_score > sampled_channels as u64 * MAX_OVERLAP_CHANNEL_DIFFERENCE {
        return None;
    }

    Some(best_overlap)
}

fn compare_overlap_strip(
    previous: &[u8],
    current: &[u8],
    width: usize,
    previous_y: usize,
    current_y: usize,
    start_x: usize,
    strip_width: usize,
    strip_height: usize,
) -> u64 {
    let mut difference = 0_u64;
    for y in (0..strip_height).step_by(2) {
        for x in (0..strip_width).step_by(2) {
            let previous_offset = ((previous_y + y) * width + start_x + x) * 4;
            let current_offset = ((current_y + y) * width + start_x + x) * 4;
            for channel in 0..3 {
                difference += previous[previous_offset + channel]
                    .abs_diff(current[current_offset + channel])
                    as u64;
            }
        }
    }
    difference
}

fn stitch_frames(frames: &[CapturedFrame]) -> Result<(usize, usize), String> {
    let Some(first) = frames.first() else {
        return Err("No frames were captured".to_string());
    };

    if frames
        .iter()
        .any(|frame| frame.width != first.width || frame.height != first.height)
    {
        return Err("Captured frame dimensions do not match".to_string());
    }

    let total_height = frames
        .iter()
        .skip(1)
        .try_fold(first.height, |height, frame| {
            height.checked_add(frame.height.saturating_sub(frame.overlap))
        })
        .ok_or_else(|| "Stitched image is too large".to_string())?;
    first
        .width
        .checked_mul(total_height)
        .and_then(|count| count.checked_mul(4))
        .ok_or_else(|| "Stitched image is too large".to_string())?;

    Ok((first.width, total_height))
}

fn write_png(
    path: &str,
    width: usize,
    height: usize,
    frames: &[CapturedFrame],
) -> Result<(), String> {
    if width == 0 || height == 0 || frames.is_empty() {
        return Err("Invalid image dimensions".to_string());
    }
    let width_u32 = u32::try_from(width).map_err(|_| "Image width is too large".to_string())?;
    let height_u32 = u32::try_from(height).map_err(|_| "Image height is too large".to_string())?;
    let file = File::create(path).map_err(|error| error.to_string())?;
    let mut writer = BufWriter::new(file);
    writer
        .write_all(&[137, 80, 78, 71, 13, 10, 26, 10])
        .map_err(|error| error.to_string())?;

    let mut header = Vec::with_capacity(13);
    header.extend_from_slice(&width_u32.to_be_bytes());
    header.extend_from_slice(&height_u32.to_be_bytes());
    header.extend_from_slice(&[8, 6, 0, 0, 0]);
    write_png_chunk(&mut writer, b"IHDR", &header)?;
    write_png_pixels(&mut writer, width, height, frames)?;
    write_png_chunk(&mut writer, b"IEND", &[])?;
    writer.flush().map_err(|error| error.to_string())
}

fn write_png_chunk(
    writer: &mut impl Write,
    chunk_type: &[u8; 4],
    data: &[u8],
) -> Result<(), String> {
    let length = u32::try_from(data.len()).map_err(|_| "PNG chunk is too large".to_string())?;
    writer
        .write_all(&length.to_be_bytes())
        .and_then(|_| writer.write_all(chunk_type))
        .and_then(|_| writer.write_all(data))
        .map_err(|error| error.to_string())?;

    let mut crc = crc32_start();
    crc = crc32_update(crc, chunk_type);
    crc = crc32_update(crc, data);
    writer
        .write_all(&crc32_finish(crc).to_be_bytes())
        .map_err(|error| error.to_string())
}

fn write_png_pixels(
    writer: &mut impl Write,
    width: usize,
    height: usize,
    frames: &[CapturedFrame],
) -> Result<(), String> {
    let row_bytes = width
        .checked_mul(4)
        .ok_or_else(|| "PNG row is too large".to_string())?;
    let filtered_row_bytes = row_bytes + 1;
    let blocks_per_row = filtered_row_bytes.div_ceil(u16::MAX as usize);
    let raw_length = filtered_row_bytes
        .checked_mul(height)
        .ok_or_else(|| "PNG data is too large".to_string())?;
    let block_count = blocks_per_row
        .checked_mul(height)
        .ok_or_else(|| "PNG data is too large".to_string())?;
    let payload_length = raw_length
        .checked_add(block_count * 5)
        .and_then(|length| length.checked_add(6))
        .ok_or_else(|| "PNG data is too large".to_string())?;
    let payload_length =
        u32::try_from(payload_length).map_err(|_| "PNG data is too large".to_string())?;

    writer
        .write_all(&payload_length.to_be_bytes())
        .and_then(|_| writer.write_all(b"IDAT"))
        .map_err(|error| error.to_string())?;

    let mut crc = crc32_update(crc32_start(), b"IDAT");
    let zlib_header = [0x78, 0x01];
    write_crc_bytes(writer, &mut crc, &zlib_header)?;
    let mut adler_a = 1_u32;
    let mut adler_b = 0_u32;

    let rows = frames.iter().enumerate().flat_map(|(index, frame)| {
        let first_row = if index == 0 {
            0
        } else {
            frame.overlap.min(frame.height)
        };
        (first_row..frame.height)
            .map(move |row| &frame.pixels[row * row_bytes..(row + 1) * row_bytes])
    });

    for (row, pixels) in rows.enumerate() {
        let mut filtered = Vec::with_capacity(filtered_row_bytes);
        filtered.push(0);
        filtered.extend_from_slice(pixels);
        update_adler32(&mut adler_a, &mut adler_b, &filtered);

        let mut offset = 0;
        while offset < filtered.len() {
            let block_length = (filtered.len() - offset).min(u16::MAX as usize);
            let is_final = row + 1 == height && offset + block_length == filtered.len();
            let length = block_length as u16;
            let header = [
                u8::from(is_final),
                length as u8,
                (length >> 8) as u8,
                !length as u8,
                (!length >> 8) as u8,
            ];
            write_crc_bytes(writer, &mut crc, &header)?;
            write_crc_bytes(writer, &mut crc, &filtered[offset..offset + block_length])?;
            offset += block_length;
        }
    }

    let adler = (adler_b << 16) | adler_a;
    write_crc_bytes(writer, &mut crc, &adler.to_be_bytes())?;
    writer
        .write_all(&crc32_finish(crc).to_be_bytes())
        .map_err(|error| error.to_string())
}

fn write_crc_bytes(writer: &mut impl Write, crc: &mut u32, bytes: &[u8]) -> Result<(), String> {
    writer.write_all(bytes).map_err(|error| error.to_string())?;
    *crc = crc32_update(*crc, bytes);
    Ok(())
}

fn update_adler32(a: &mut u32, b: &mut u32, bytes: &[u8]) {
    for chunk in bytes.chunks(5_552) {
        for byte in chunk {
            *a += *byte as u32;
            *b += *a;
        }
        *a %= 65_521;
        *b %= 65_521;
    }
}

fn crc32_start() -> u32 {
    u32::MAX
}

fn crc32_update(mut crc: u32, bytes: &[u8]) -> u32 {
    static TABLE: OnceLock<[u32; 256]> = OnceLock::new();
    let table = TABLE.get_or_init(|| {
        let mut table = [0_u32; 256];
        for (index, entry) in table.iter_mut().enumerate() {
            let mut value = index as u32;
            for _ in 0..8 {
                let mask = 0_u32.wrapping_sub(value & 1);
                value = (value >> 1) ^ (0xedb88320 & mask);
            }
            *entry = value;
        }
        table
    });

    for byte in bytes {
        crc = table[((crc ^ *byte as u32) & 0xff) as usize] ^ (crc >> 8);
    }
    crc
}

fn crc32_finish(crc: u32) -> u32 {
    !crc
}

pub struct ScrollCaptureModule {
    state: Arc<Mutex<CaptureState>>,
}

impl ScrollCaptureModule {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(CaptureState::default())),
        }
    }

    fn start(&mut self, request: &Request) -> Reply {
        let Some(x) = param_i32(&request.params, "x") else {
            return invalid_start_params();
        };
        let Some(y) = param_i32(&request.params, "y") else {
            return invalid_start_params();
        };
        let Some(width) = param_i32(&request.params, "width") else {
            return invalid_start_params();
        };
        let Some(height) = param_i32(&request.params, "height") else {
            return invalid_start_params();
        };
        if width <= 0
            || height <= 0
            || x.checked_add(width).is_none()
            || y.checked_add(height).is_none()
        {
            return invalid_start_params();
        }

        let scale_factor = request
            .params
            .as_ref()
            .and_then(|params| params.get("scaleFactor"))
            .and_then(|value| value.as_f64())
            .unwrap_or(1.0);
        if !scale_factor.is_finite() || !(0.25..=8.0).contains(&scale_factor) {
            return Reply::Now(Err((
                "INVALID_PARAMS".to_string(),
                "scaleFactor must be between 0.25 and 8".to_string(),
            )));
        }

        let speed = ScrollSpeed::parse(param_str(&request.params, "autoScrollSpeed"));
        let logical_capture_height = logical_height(height as usize, scale_factor);
        let max_height = (param_i32(&request.params, "maxHeight")
            .unwrap_or(20_000)
            .max(1) as usize)
            .max(logical_capture_height);
        let bounds = CaptureBounds {
            x,
            y,
            width,
            height,
        };

        {
            let Ok(state) = self.state.lock() else {
                return Reply::Now(Err((
                    "INTERNAL_ERROR".to_string(),
                    "Scroll capture state poisoned".to_string(),
                )));
            };
            if state.is_capturing {
                return Reply::Now(Err((
                    "ALREADY_CAPTURING".to_string(),
                    "Scroll capture is already active".to_string(),
                )));
            }
        }

        let mut capture_state = CaptureState::default();
        capture_state.bounds = Some(bounds);
        capture_state.is_capturing = true;
        capture_state.auto_scroll_speed = speed;
        capture_state.max_height = max_height;
        capture_state.scale_factor = scale_factor;
        capture_state.scroll_step_points =
            ((height as usize * (100 - FRAME_OVERLAP_PERCENT as usize)) / 100).max(1);
        let state = Arc::new(Mutex::new(capture_state));
        self.state = state.clone();
        let request_id = request.id.clone();
        let dpi = dpi_for_scale_factor(scale_factor);
        run_on_ui(move || match show_capture_ui(state.clone(), bounds, dpi) {
            Ok(()) => respond_success(&request_id, json!({ "started": true })),
            Err(message) => {
                if let Ok(mut state) = state.lock() {
                    state.reset();
                }
                teardown_ui();
                respond_error(&request_id, "UI_ERROR", &message);
            }
        });
        Reply::Deferred
    }

    fn start_auto_scroll(&self, request: &Request) -> Reply {
        {
            let Ok(mut state) = self.state.lock() else {
                return Reply::Now(Err((
                    "INTERNAL_ERROR".to_string(),
                    "Scroll capture state poisoned".to_string(),
                )));
            };
            if !state.is_capturing {
                return Reply::Now(Err((
                    "NOT_CAPTURING".to_string(),
                    "Not in capture mode".to_string(),
                )));
            }
            if let Some(speed) = param_str(&request.params, "speed") {
                state.auto_scroll_speed = match speed {
                    "slow" => ScrollSpeed::Slow,
                    "medium" => ScrollSpeed::Medium,
                    "fast" => ScrollSpeed::Fast,
                    _ => state.auto_scroll_speed,
                };
            }
        }

        let state = self.state.clone();
        let request_id = request.id.clone();
        run_on_ui(move || match start_auto_scroll_ui(&state) {
            Ok(()) => respond_success(&request_id, json!({ "autoScrolling": true })),
            Err((code, message)) => respond_error(&request_id, &code, &message),
        });
        Reply::Deferred
    }

    fn stop_auto_scroll(&self, request: &Request) -> Reply {
        let state = self.state.clone();
        let request_id = request.id.clone();
        run_on_ui(move || {
            stop_auto_scroll_ui(&state);
            respond_success(&request_id, json!({ "autoScrolling": false }));
        });
        Reply::Deferred
    }

    fn finish(&self, request: &Request) -> Reply {
        let Some(output_path) = param_str(&request.params, "outputPath") else {
            return Reply::Now(Err((
                "INVALID_PARAMS".to_string(),
                "finish requires outputPath".to_string(),
            )));
        };

        let frames = {
            let Ok(mut state) = self.state.lock() else {
                return Reply::Now(Err((
                    "INTERNAL_ERROR".to_string(),
                    "Scroll capture state poisoned".to_string(),
                )));
            };
            if !state.is_capturing {
                return Reply::Now(Err((
                    "NOT_CAPTURING".to_string(),
                    "Not in capture mode".to_string(),
                )));
            }

            state.stop_auto_scroll();
            let frames = std::mem::take(&mut state.frames);
            state.reset();
            frames
        };

        run_on_ui(teardown_ui);
        let output_path = output_path.to_string();
        let request_id = request.id.clone();
        let frame_count = frames.len();
        std::thread::spawn(move || {
            let result = stitch_frames(&frames).and_then(|(width, height)| {
                write_png(&output_path, width, height, &frames)?;
                Ok((width, height))
            });

            match result {
                Ok((width, height)) => respond_success(
                    &request_id,
                    json!({
                        "success": true,
                        "outputPath": output_path,
                        "width": width,
                        "height": height,
                        "frameCount": frame_count,
                    }),
                ),
                Err(message) => respond_error(&request_id, "STITCH_ERROR", &message),
            }
        });

        Reply::Deferred
    }

    fn cancel(&self) -> Reply {
        let was_capturing = self
            .state
            .lock()
            .ok()
            .map(|mut state| {
                let was_capturing = state.is_capturing;
                state.reset();
                was_capturing
            })
            .unwrap_or(false);
        run_on_ui(teardown_ui);
        if was_capturing {
            send_event("scroll-capture:cancelled", None);
        }
        Reply::Now(Ok(Some(json!({ "cancelled": true }))))
    }

    fn status(&self) -> Reply {
        let Ok(state) = self.state.lock() else {
            return Reply::Now(Err((
                "INTERNAL_ERROR".to_string(),
                "Scroll capture state poisoned".to_string(),
            )));
        };
        let estimated_height = logical_height(state.estimated_height, state.scale_factor);
        Reply::Now(Ok(Some(json!({
            "isCapturing": state.is_capturing,
            "isAutoScrolling": state.is_auto_scrolling,
            "frameCount": state.frames.len(),
            "estimatedHeight": estimated_height,
        }))))
    }
}

impl Module for ScrollCaptureModule {
    fn name(&self) -> &'static str {
        "scroll-capture"
    }

    fn handle(&mut self, request: &Request) -> Reply {
        match request.method.as_str() {
            "start" => self.start(request),
            "startAutoScroll" => self.start_auto_scroll(request),
            "stopAutoScroll" => self.stop_auto_scroll(request),
            "finish" => self.finish(request),
            "cancel" => self.cancel(),
            "status" => self.status(),
            method => method_not_found(method),
        }
    }
}

fn invalid_start_params() -> Reply {
    Reply::Now(Err((
        "INVALID_PARAMS".to_string(),
        "start requires x, y, width, height".to_string(),
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(start_row: usize, width: usize, height: usize) -> CapturedFrame {
        let mut pixels = Vec::with_capacity(width * height * 4);
        for y in 0..height {
            for x in 0..width {
                let seed = ((start_row + y) as u32).wrapping_mul(0x45d9f3b)
                    ^ (x as u32).wrapping_mul(0x119de1f3);
                pixels.extend_from_slice(&[
                    seed as u8,
                    seed.rotate_left(9) as u8,
                    seed.rotate_left(17) as u8,
                    255,
                ]);
            }
        }
        CapturedFrame {
            width,
            height,
            pixels,
            overlap: 0,
        }
    }

    #[test]
    fn finds_overlap_between_translated_frames() {
        let width = 96;
        let height = 160;
        let overlap = 52;
        let previous = frame(0, width, height);
        let current = frame(height - overlap, width, height);

        assert_eq!(
            find_overlap(&previous, &current.pixels, width, height, 48),
            Some(overlap)
        );
    }

    #[test]
    fn recognizes_repeated_frame_content() {
        let width = 96;
        let height = 160;
        let previous = frame(0, width, height);
        let repeated = frame(0, width, height);

        assert_eq!(
            frame_hash(&previous.pixels, width, height),
            frame_hash(&repeated.pixels, width, height)
        );
        assert_eq!(
            find_overlap(&previous, &repeated.pixels, width, height, 48),
            Some(height)
        );
    }

    #[test]
    fn rejects_frames_without_confident_overlap() {
        let width = 96;
        let height = 160;
        let previous = frame(0, width, height);
        let unrelated = frame(10_000, width, height);

        assert_eq!(
            find_overlap(&previous, &unrelated.pixels, width, height, 48),
            None
        );
    }

    #[test]
    fn plans_equal_displacement_across_speeds() {
        let slow = scroll_plan(1_000.0, ScrollSpeed::Slow);
        let medium = scroll_plan(1_000.0, ScrollSpeed::Medium);
        let fast = scroll_plan(1_000.0, ScrollSpeed::Fast);

        assert_eq!(slow.target_logical_points, 700);
        assert_eq!(slow.wheel_detents, medium.wheel_detents);
        assert_eq!(medium.wheel_detents, fast.wheel_detents);
        assert!(slow.interval > medium.interval);
        assert!(medium.interval > fast.interval);
    }

    #[test]
    fn converts_physical_stitched_height_to_logical_max_height_units() {
        assert_eq!(logical_height(3_000, 2.0), 1_500);
        assert_eq!(logical_height(1_501, 1.5), 1_001);
    }

    #[test]
    fn derives_panel_dpi_from_the_display_scale_factor() {
        assert_eq!(dpi_for_scale_factor(1.5), 144);
        assert_eq!(dpi_for_scale_factor(0.5), 96);
    }

    #[test]
    fn auto_button_switches_to_a_stop_action_while_scrolling() {
        assert_eq!(panel_button_label(0, false), "Auto");
        assert_eq!(panel_button_label(0, true), "Stop");
        assert_eq!(panel_button_palette(0, false)[0].0, NEUTRAL_BUTTON[0].0);
        assert_eq!(panel_button_palette(0, true)[0].0, ACTIVE_BUTTON[0].0);
        assert_eq!(panel_button_palette(1, false)[0].0, PRIMARY_BUTTON[0].0);
    }

    #[test]
    fn boundary_ring_leaves_the_capture_area_untouched() {
        let capture = RECT {
            left: 100,
            top: 200,
            right: 500,
            bottom: 600,
        };
        let ring = RECT {
            left: capture.left - 2,
            top: capture.top - 2,
            right: capture.right + 2,
            bottom: capture.bottom + 2,
        };

        assert!(!ring_covers_rect(&ring, 2, &capture));
        assert!(ring_covers_rect(&ring, 3, &capture));
    }

    #[test]
    fn hides_only_windows_the_capture_would_include() {
        assert_eq!(
            windows_to_hide([true, true], [false, false]),
            [false, false]
        );
        assert_eq!(windows_to_hide([true, true], [true, false]), [true, false]);
        assert_eq!(windows_to_hide([false, true], [true, true]), [false, true]);
    }

    #[test]
    fn capture_window_visibility_is_restored_after_success() {
        let events = RefCell::new(Vec::new());

        let result = with_hidden_capture_windows(
            [true, false],
            |index, visible| events.borrow_mut().push((index, visible)),
            || {
                events.borrow_mut().push((usize::MAX, true));
                Ok::<_, ()>(42)
            },
        );

        assert_eq!(result, Ok(42));
        assert_eq!(
            events.into_inner(),
            vec![(0, false), (usize::MAX, true), (0, true)]
        );
    }

    #[test]
    fn capture_window_visibility_is_restored_after_failure() {
        let events = RefCell::new(Vec::new());

        let result = with_hidden_capture_windows(
            [true, true],
            |index, visible| events.borrow_mut().push((index, visible)),
            || {
                events.borrow_mut().push((usize::MAX, false));
                Err::<(), _>("capture failed")
            },
        );

        assert_eq!(result, Err("capture failed"));
        assert_eq!(
            events.into_inner(),
            vec![
                (0, false),
                (1, false),
                (usize::MAX, false),
                (0, true),
                (1, true)
            ]
        );
    }
}
