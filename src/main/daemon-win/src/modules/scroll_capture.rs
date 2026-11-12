use crate::overlay::{
    WM_MOUSELEAVE, add_key_handler, apply_round_region, create_popup_window, create_ui_font,
    default_wndproc, ensure_window_class, monitors, point_from_lparam, rects_intersect,
    remove_key_handler, scale_for_dpi,
};
use crate::panel::{
    ACTIVE_BUTTON, BUTTON_TEXT, BUTTON_TEXT_ON_FILL, NEUTRAL_BUTTON, PANEL_ALPHA, PANEL_BACKGROUND,
    PANEL_BUTTON_RADIUS, PANEL_CORNER_RADIUS, PANEL_FONT_SIZE, PANEL_FONT_WEIGHT, PRIMARY_BUTTON,
    button_at, button_fill, button_rect, button_state, draw_label, draw_pill, paint_buffered,
    panel_height, panel_width,
};
use crate::protocol::{Request, params, respond_error, respond_success, send_event};
use crate::router::{Module, Reply, method_not_found};
use crate::ui::run_on_ui;
use poratake_daemon_common::contract::{
    SCROLL_CAPTURE_CANCELLED_EVENT, SCROLL_CAPTURE_DONE_EVENT, SCROLL_CAPTURE_FRAME_EVENT,
    SCROLL_CAPTURE_MODULE, SCROLL_CAPTURE_SCROLL_ENDED_EVENT, ScrollCaptureAutoScrollRequest,
    ScrollCaptureFinishRequest, ScrollCaptureFinishResult, ScrollCaptureMethod,
    ScrollCaptureStartRequest, ScrollSpeed,
};
use poratake_daemon_common::scroll::{
    CaptureOutcome, CapturedFrame, FRAME_OVERLAP_PERCENT, FrameAccumulator, PngWriteError,
    scroll_interval_millis, scroll_plan, write_png_cancellable,
};
use serde_json::json;
use std::cell::RefCell;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
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
const WHEEL_DELTA: i32 = -120;
const DISABLED_BUTTON_TEXT: COLORREF = COLORREF(0x00808080);

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

fn logical_height(physical_height: usize, scale_factor: f64) -> usize {
    (physical_height as f64 / scale_factor).ceil() as usize
}

fn dpi_for_scale_factor(scale_factor: f64) -> u32 {
    (96.0 * scale_factor).round().max(96.0) as u32
}

struct CaptureState {
    bounds: Option<CaptureBounds>,
    accumulator: FrameAccumulator,
    is_capturing: bool,
    is_finishing: bool,
    is_auto_scrolling: bool,
    done_requested: bool,
    finish_job: Option<Arc<FinishJob>>,
    auto_scroll_speed: ScrollSpeed,
    max_height: usize,
    scale_factor: f64,
    scroll_step_points: usize,
    scroll_steps_per_frame: usize,
    current_scroll_step: usize,
    capture_on_next_tick: bool,
}

impl Default for CaptureState {
    fn default() -> Self {
        Self {
            bounds: None,
            accumulator: FrameAccumulator::default(),
            is_capturing: false,
            is_finishing: false,
            is_auto_scrolling: false,
            done_requested: false,
            finish_job: None,
            auto_scroll_speed: ScrollSpeed::default(),
            max_height: 20_000,
            scale_factor: 1.0,
            scroll_step_points: 0,
            scroll_steps_per_frame: 0,
            current_scroll_step: 0,
            capture_on_next_tick: false,
        }
    }
}

#[derive(Clone, Copy)]
enum FinishJobState {
    Writing,
    CancelRequested,
    Completed(bool),
}

enum FinishCancelRequest {
    Accepted,
    AlreadyRequested,
    Completed(bool),
}

struct FinishJobStatus {
    state: FinishJobState,
    cancel_request_ids: Vec<String>,
}

struct FinishCompletion {
    cancelled: bool,
    cancel_request_ids: Vec<String>,
}

struct FinishJob {
    cancelled: AtomicBool,
    status: Mutex<FinishJobStatus>,
}

impl FinishJob {
    fn new() -> Self {
        Self {
            cancelled: AtomicBool::new(false),
            status: Mutex::new(FinishJobStatus {
                state: FinishJobState::Writing,
                cancel_request_ids: Vec::new(),
            }),
        }
    }

    #[cfg(test)]
    fn request_cancel(&self) -> FinishCancelRequest {
        self.request_cancel_inner(None, || {})
    }

    fn request_cancel_with_intent(&self, write_intent: impl FnOnce()) -> FinishCancelRequest {
        self.request_cancel_inner(None, write_intent)
    }

    fn request_cancel_rpc_with_intent(
        &self,
        request_id: String,
        write_intent: impl FnOnce(),
    ) -> FinishCancelRequest {
        self.request_cancel_inner(Some(request_id), write_intent)
    }

    fn request_cancel_inner(
        &self,
        request_id: Option<String>,
        write_intent: impl FnOnce(),
    ) -> FinishCancelRequest {
        let Ok(mut status) = self.status.lock() else {
            return FinishCancelRequest::Completed(false);
        };
        match status.state {
            FinishJobState::Writing => {
                status.state = FinishJobState::CancelRequested;
                if let Some(request_id) = request_id {
                    status.cancel_request_ids.push(request_id);
                }
                write_intent();
                self.cancelled.store(true, Ordering::Release);
                FinishCancelRequest::Accepted
            }
            FinishJobState::CancelRequested => {
                if let Some(request_id) = request_id {
                    status.cancel_request_ids.push(request_id);
                }
                FinishCancelRequest::AlreadyRequested
            }
            FinishJobState::Completed(cancelled) => FinishCancelRequest::Completed(cancelled),
        }
    }

    fn complete(&self, writer_cancelled: bool) -> FinishCompletion {
        let Ok(mut status) = self.status.lock() else {
            return FinishCompletion {
                cancelled: writer_cancelled || self.cancelled.load(Ordering::Acquire),
                cancel_request_ids: Vec::new(),
            };
        };
        let cancelled = writer_cancelled || matches!(status.state, FinishJobState::CancelRequested);
        status.state = FinishJobState::Completed(cancelled);
        FinishCompletion {
            cancelled,
            cancel_request_ids: std::mem::take(&mut status.cancel_request_ids),
        }
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    fn is_completed(&self) -> bool {
        self.status
            .lock()
            .is_ok_and(|status| matches!(status.state, FinishJobState::Completed(_)))
    }
}

struct FinishWorker {
    job: Arc<FinishJob>,
    supervisor: JoinHandle<()>,
}

type PngWriteResult = Result<(usize, usize), PngWriteError>;
type FinishWriterTask = Box<dyn FnOnce() -> PngWriteResult + Send + 'static>;
type FinishSupervisorTask = Box<dyn FnOnce() + Send + 'static>;
type FinishPngWriter =
    Box<dyn FnOnce(&Path, &[CapturedFrame], &AtomicBool) -> PngWriteResult + Send + 'static>;
type FinishWriterSpawner =
    Box<dyn FnOnce(FinishWriterTask) -> io::Result<JoinHandle<PngWriteResult>> + Send + 'static>;
type FinishSupervisorSpawner =
    Box<dyn FnOnce(FinishSupervisorTask) -> io::Result<JoinHandle<()>> + Send + 'static>;

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

fn spawn_finish_writer(
    state: Arc<Mutex<CaptureState>>,
    output_path: PathBuf,
    job: Arc<FinishJob>,
    writer: FinishPngWriter,
    spawn: FinishWriterSpawner,
) -> io::Result<JoinHandle<PngWriteResult>> {
    let task = Box::new(move || {
        let frames = match state.lock() {
            Ok(mut state) => state.accumulator.take_frames(),
            Err(_) => {
                return Err(PngWriteError::Failed(
                    "Scroll capture state poisoned".to_string(),
                ));
            }
        };
        writer(&output_path, &frames, &job.cancelled)
    });
    spawn(task)
}

fn joined_writer_result(result: thread::Result<PngWriteResult>) -> PngWriteResult {
    match result {
        Ok(result) => result,
        Err(_) => Err(PngWriteError::Failed(
            "Scroll capture worker panicked".to_string(),
        )),
    }
}

fn retryable_finish_completion(
    state: &Arc<Mutex<CaptureState>>,
    job: &FinishJob,
) -> FinishCompletion {
    let Ok(mut state) = state.lock() else {
        return job.complete(false);
    };
    let completion = job.complete(false);
    if completion.cancelled {
        state.reset();
    } else {
        state.is_finishing = false;
        state.done_requested = false;
        state.finish_job = None;
    }
    completion
}

fn respond_cancel_requests(request_ids: Vec<String>, cancelled: bool) {
    for request_id in request_ids {
        respond_success(&request_id, json!({ "cancelled": cancelled }));
    }
}

fn respond_finish_result(
    request_id: &str,
    output_path: PathBuf,
    frame_count: usize,
    completion: FinishCompletion,
    result: Result<PngWriteResult, String>,
) {
    if completion.cancelled {
        let result = ScrollCaptureFinishResult {
            success: false,
            output_path,
            width: 0,
            height: 0,
            frame_count,
        };
        match serde_json::to_value(result) {
            Ok(result) => respond_success(request_id, result),
            Err(error) => respond_error(request_id, "STITCH_ERROR", &error.to_string()),
        }
        respond_cancel_requests(completion.cancel_request_ids, true);
        return;
    }

    match result {
        Err(message) => respond_error(request_id, "STITCH_RETRYABLE", &message),
        Ok(Ok((width, height))) => {
            let result = ScrollCaptureFinishResult {
                success: true,
                output_path,
                width,
                height,
                frame_count,
            };
            match serde_json::to_value(result) {
                Ok(result) => respond_success(request_id, result),
                Err(error) => respond_error(request_id, "STITCH_ERROR", &error.to_string()),
            }
        }
        Ok(Err(PngWriteError::Cancelled)) => {
            let result = ScrollCaptureFinishResult {
                success: false,
                output_path,
                width: 0,
                height: 0,
                frame_count,
            };
            match serde_json::to_value(result) {
                Ok(result) => respond_success(request_id, result),
                Err(error) => respond_error(request_id, "STITCH_ERROR", &error.to_string()),
            }
        }
        Ok(Err(PngWriteError::Failed(message))) => {
            respond_error(request_id, "STITCH_ERROR", &message)
        }
    }
    respond_cancel_requests(completion.cancel_request_ids, false);
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
    static UI_STATE: RefCell<ScrollUiState> = const { RefCell::new(ScrollUiState {
        boundary: None,
        panel: None,
        font: None,
        dpi: 96,
        hovered: None,
        pressed: None,
        escape_token: None,
        enter_token: None,
        capture_state: None,
    }) };
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

fn panel_status() -> (bool, bool, bool) {
    UI_STATE
        .with(|ui| ui.borrow().capture_state.clone())
        .and_then(|state| {
            state.lock().ok().map(|state| {
                (
                    state.is_auto_scrolling,
                    state.done_requested || state.is_finishing,
                    state
                        .finish_job
                        .as_ref()
                        .is_some_and(|job| job.is_cancelled()),
                )
            })
        })
        .unwrap_or_default()
}

fn panel_button_enabled(index: usize, finishing: bool, cancel_requested: bool) -> bool {
    match PANEL_BUTTONS[index] {
        PanelButton::Cancel => !cancel_requested,
        PanelButton::Auto | PanelButton::Done => !finishing,
    }
}

fn panel_button_at(point: POINT) -> Option<usize> {
    let index = button_at(
        &PANEL_BUTTON_WIDTHS,
        point,
        UI_STATE.with(|ui| ui.borrow().dpi),
    );
    let (_, finishing, cancel_requested) = panel_status();
    index.filter(|index| panel_button_enabled(*index, finishing, cancel_requested))
}

fn panel_button_label(
    index: usize,
    auto_scrolling: bool,
    finishing: bool,
    cancel_requested: bool,
) -> &'static str {
    match PANEL_BUTTONS[index] {
        PanelButton::Auto if finishing => "Auto",
        PanelButton::Auto if auto_scrolling => "Stop",
        PanelButton::Auto => "Auto",
        PanelButton::Done if finishing => "Saving",
        PanelButton::Done => "Done",
        PanelButton::Cancel if cancel_requested => "Canceling",
        PanelButton::Cancel => "Cancel",
    }
}

fn panel_button_palette(
    index: usize,
    auto_scrolling: bool,
    finishing: bool,
    cancel_requested: bool,
) -> [COLORREF; 3] {
    if !panel_button_enabled(index, finishing, cancel_requested) {
        return [PANEL_BACKGROUND; 3];
    }
    match PANEL_BUTTONS[index] {
        PanelButton::Auto if auto_scrolling => ACTIVE_BUTTON,
        PanelButton::Done => PRIMARY_BUTTON,
        _ => NEUTRAL_BUTTON,
    }
}

fn panel_button_text(
    index: usize,
    auto_scrolling: bool,
    finishing: bool,
    cancel_requested: bool,
) -> COLORREF {
    if !panel_button_enabled(index, finishing, cancel_requested) {
        return DISABLED_BUTTON_TEXT;
    }
    match PANEL_BUTTONS[index] {
        PanelButton::Auto if auto_scrolling => BUTTON_TEXT_ON_FILL,
        PanelButton::Done => BUTTON_TEXT_ON_FILL,
        _ => BUTTON_TEXT,
    }
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
    let (auto_scrolling, finishing, cancel_requested) = panel_status();

    paint_buffered(window, font, |dc, _| {
        for index in 0..PANEL_BUTTONS.len() {
            let rect = button_rect(&PANEL_BUTTON_WIDTHS, index, dpi);
            let enabled = panel_button_enabled(index, finishing, cancel_requested);
            let state = button_state(
                enabled && hovered == Some(index),
                enabled && pressed == Some(index),
            );
            draw_pill(
                dc,
                rect,
                scale_for_dpi(PANEL_BUTTON_RADIUS, dpi),
                button_fill(
                    panel_button_palette(index, auto_scrolling, finishing, cancel_requested),
                    state,
                ),
            );
            draw_label(
                dc,
                panel_button_label(index, auto_scrolling, finishing, cancel_requested),
                rect,
                panel_button_text(index, auto_scrolling, finishing, cancel_requested),
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
    let hovered = panel_button_at(point);
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
    let pressed = panel_button_at(point);
    let pressed = UI_STATE.with(|ui| {
        let mut ui = ui.borrow_mut();
        ui.pressed = pressed;
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

    let (_, finishing, cancel_requested) = panel_status();
    if !panel_button_enabled(pressed, finishing, cancel_requested) {
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

    let (auto_scrolling, enabled) = state
        .lock()
        .ok()
        .map(|state| {
            (
                state.is_auto_scrolling,
                !state.is_finishing && !state.done_requested,
            )
        })
        .unwrap_or((false, false));

    if !enabled {
        return;
    }

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
            if !state.is_capturing || state.is_finishing || state.done_requested {
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
    send_event(SCROLL_CAPTURE_DONE_EVENT, None);
}

fn cancel_from_ui() {
    let state = UI_STATE.with(|ui| ui.borrow().capture_state.clone());
    let Some(state) = state else {
        return;
    };

    enum CancelAction {
        Finishing(FinishCancelRequest),
        Reset(bool),
        Ignore,
    }

    let action = state
        .lock()
        .ok()
        .map(|mut state| {
            if state.is_finishing {
                let Some(job) = state.finish_job.clone() else {
                    return CancelAction::Ignore;
                };
                return CancelAction::Finishing(job.request_cancel_with_intent(|| {
                    send_event(SCROLL_CAPTURE_CANCELLED_EVENT, None)
                }));
            }
            let was_capturing = state.is_capturing;
            state.reset();
            CancelAction::Reset(was_capturing)
        })
        .unwrap_or(CancelAction::Ignore);

    match action {
        CancelAction::Finishing(FinishCancelRequest::Accepted) => {
            let panel = UI_STATE.with(|ui| ui.borrow().panel);
            if let Some(panel) = panel {
                repaint_panel(panel);
            }
        }
        CancelAction::Finishing(_) | CancelAction::Ignore => (),
        CancelAction::Reset(was_capturing) => {
            teardown_ui();
            if was_capturing {
                send_event(SCROLL_CAPTURE_CANCELLED_EVENT, None);
            }
        }
    }
}

fn show_capture_ui(
    state: Arc<Mutex<CaptureState>>,
    bounds: CaptureBounds,
    dpi: u32,
    visible: bool,
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
            if visible {
                let _ = ShowWindow(boundary, SW_SHOWNOACTIVATE);
            }
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
            if visible {
                let _ = ShowWindow(panel, SW_SHOWNOACTIVATE);
            }
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

    if visible {
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
    }

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
        if state.is_finishing || state.done_requested {
            return Err((
                "ALREADY_FINISHING".to_string(),
                "Scroll capture is finishing".to_string(),
            ));
        }
        state
            .is_auto_scrolling
            .then_some(scroll_interval_millis(state.auto_scroll_speed) as u32)
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

        (bounds, plan.interval_millis as u32)
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
                        logical_height(state.accumulator.estimated_height(), state.scale_factor);
                    if estimated_height >= state.max_height {
                        stop = true;
                        event = Some(json!({
                            "reason": "max-height",
                            "frameCount": state.accumulator.frames().len(),
                            "estimatedHeight": estimated_height,
                        }));
                    } else {
                        send_event(
                            SCROLL_CAPTURE_FRAME_EVENT,
                            Some(json!({
                                "frameCount": state.accumulator.frames().len(),
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
                        "frameCount": state.accumulator.frames().len(),
                    }));
                }
                Ok(CaptureOutcome::NoOverlap) => {
                    stop = true;
                    event = Some(json!({
                        "reason": "no-overlap",
                        "frameCount": state.accumulator.frames().len(),
                    }));
                }
                Err(message) => {
                    stop = true;
                    event = Some(json!({
                        "reason": "capture-error",
                        "message": message,
                        "frameCount": state.accumulator.frames().len(),
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
                    "frameCount": state.accumulator.frames().len(),
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
        send_event(SCROLL_CAPTURE_SCROLL_ENDED_EVENT, event);
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
    let width = bounds.width as usize;
    let height = bounds.height as usize;
    let expected = height
        .saturating_sub(state.scroll_step_points)
        .clamp(1, height);
    Ok(state.accumulator.submit(pixels, width, height, expected))
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

        for pixel in pixels.as_chunks_mut::<4>().0 {
            pixel.swap(0, 2);
            pixel[3] = 255;
        }

        Ok(pixels)
    }
}

pub struct ScrollCaptureModule {
    state: Arc<Mutex<CaptureState>>,
    finish_worker: Option<FinishWorker>,
}

impl ScrollCaptureModule {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(CaptureState::default())),
            finish_worker: None,
        }
    }

    fn finish_worker_active(&mut self) -> bool {
        if self
            .finish_worker
            .as_ref()
            .is_some_and(|finish| !finish.job.is_completed())
        {
            return true;
        }
        if let Some(finish) = self.finish_worker.take() {
            let _ = finish.supervisor.join();
        }
        false
    }

    fn prepare_start(&mut self) -> Result<(), (String, String)> {
        if self.finish_worker_active() {
            return Err((
                "ALREADY_CAPTURING".to_string(),
                "Scroll capture is already active".to_string(),
            ));
        }
        let state = self.state.lock().map_err(|_| {
            (
                "INTERNAL_ERROR".to_string(),
                "Scroll capture state poisoned".to_string(),
            )
        })?;
        if state.is_capturing {
            return Err((
                "ALREADY_CAPTURING".to_string(),
                "Scroll capture is already active".to_string(),
            ));
        }
        Ok(())
    }

    fn start(&mut self, request: &Request) -> Reply {
        let request_params: ScrollCaptureStartRequest = match params(request) {
            Ok(request_params) => request_params,
            Err(error) => return Reply::Now(Err(error)),
        };
        if let Err(message) = request_params.validate() {
            return Reply::Now(Err(("INVALID_PARAMS".into(), message)));
        }

        if let Err(error) = self.prepare_start() {
            return Reply::Now(Err(error));
        }

        let bounds = CaptureBounds {
            x: request_params.capture.rect.x,
            y: request_params.capture.rect.y,
            width: request_params.capture.rect.width,
            height: request_params.capture.rect.height,
        };
        let capture_state = CaptureState {
            bounds: Some(bounds),
            is_capturing: true,
            auto_scroll_speed: request_params.auto_scroll_speed,
            max_height: request_params.normalized_max_height(),
            scale_factor: request_params.capture.scale_factor,
            scroll_step_points: ((request_params.capture.rect.height as usize
                * (100 - FRAME_OVERLAP_PERCENT))
                / 100)
                .max(1),
            ..CaptureState::default()
        };
        let state = Arc::new(Mutex::new(capture_state));
        self.state = state.clone();
        let request_id = request.id.clone();
        let dpi = dpi_for_scale_factor(request_params.capture.scale_factor);
        let native_controls = request_params.native_controls.unwrap_or(true);
        run_on_ui(
            move || match show_capture_ui(state.clone(), bounds, dpi, native_controls) {
                Ok(()) => respond_success(&request_id, json!({ "started": true })),
                Err(message) => {
                    if let Ok(mut state) = state.lock() {
                        state.reset();
                    }
                    teardown_ui();
                    respond_error(&request_id, "UI_ERROR", &message);
                }
            },
        );
        Reply::Deferred
    }

    fn start_auto_scroll(&self, request: &Request) -> Reply {
        let request_params: ScrollCaptureAutoScrollRequest = match params(request) {
            Ok(request_params) => request_params,
            Err(error) => return Reply::Now(Err(error)),
        };
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
            if state.is_finishing || state.done_requested {
                return Reply::Now(Err((
                    "ALREADY_FINISHING".to_string(),
                    "Scroll capture is finishing".to_string(),
                )));
            }
            if let Some(speed) = request_params.speed {
                state.auto_scroll_speed = speed;
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

    fn finish(&mut self, request: &Request) -> Reply {
        self.finish_with_spawners(
            request,
            Box::new(|task| {
                thread::Builder::new()
                    .name("scroll-capture-finish".to_string())
                    .spawn(task)
            }),
            Box::new(|task| {
                thread::Builder::new()
                    .name("scroll-capture-png".to_string())
                    .spawn(task)
            }),
            Box::new(write_png_cancellable),
        )
    }

    fn finish_with_spawners(
        &mut self,
        request: &Request,
        spawn_supervisor: FinishSupervisorSpawner,
        spawn_writer: FinishWriterSpawner,
        writer: FinishPngWriter,
    ) -> Reply {
        let request_params: ScrollCaptureFinishRequest = match params(request) {
            Ok(request_params) => request_params,
            Err(error) => return Reply::Now(Err(error)),
        };

        if self.finish_worker_active() {
            return Reply::Now(Err((
                "ALREADY_FINISHING".to_string(),
                "Scroll capture is already finishing".to_string(),
            )));
        }

        let job = Arc::new(FinishJob::new());
        let frame_count = {
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
            if state.is_finishing {
                return Reply::Now(Err((
                    "ALREADY_FINISHING".to_string(),
                    "Scroll capture is already finishing".to_string(),
                )));
            }

            state.stop_auto_scroll();
            state.is_finishing = true;
            state.done_requested = true;
            state.finish_job = Some(job.clone());
            state.accumulator.frames().len()
        };

        let state = self.state.clone();
        run_on_ui(move || stop_auto_scroll_ui(&state));
        let output_path = request_params.output_path;
        let supervisor_output_path = output_path.clone();
        let request_id = request.id.clone();
        let supervisor_state = self.state.clone();
        let supervisor_job = job.clone();
        let supervisor = Box::new(move || {
            let result = match spawn_finish_writer(
                supervisor_state.clone(),
                supervisor_output_path.clone(),
                supervisor_job.clone(),
                writer,
                spawn_writer,
            ) {
                Ok(worker) => Ok(joined_writer_result(worker.join())),
                Err(error) => Err(error.to_string()),
            };

            let completion = if result.is_err() {
                let completion = retryable_finish_completion(&supervisor_state, &supervisor_job);
                if completion.cancelled {
                    run_on_ui(teardown_ui);
                } else {
                    run_on_ui(|| {
                        let panel = UI_STATE.with(|ui| ui.borrow().panel);
                        if let Some(panel) = panel {
                            repaint_panel(panel);
                        }
                    });
                }
                completion
            } else {
                let writer_cancelled = matches!(&result, Ok(Err(PngWriteError::Cancelled)));
                let completion = supervisor_job.complete(writer_cancelled);
                if let Ok(mut state) = supervisor_state.lock() {
                    state.reset();
                }
                run_on_ui(teardown_ui);
                completion
            };

            respond_finish_result(
                &request_id,
                supervisor_output_path,
                frame_count,
                completion,
                result,
            );
        });

        match spawn_supervisor(supervisor) {
            Ok(supervisor) => self.finish_worker = Some(FinishWorker { job, supervisor }),
            Err(error) => {
                let completion = retryable_finish_completion(&self.state, &job);
                if completion.cancelled {
                    run_on_ui(teardown_ui);
                    let result = ScrollCaptureFinishResult {
                        success: false,
                        output_path,
                        width: 0,
                        height: 0,
                        frame_count,
                    };
                    let result = serde_json::to_value(result)
                        .map(Some)
                        .map_err(|error| ("STITCH_ERROR".to_string(), error.to_string()));
                    respond_cancel_requests(completion.cancel_request_ids, true);
                    return Reply::Now(result);
                }
                run_on_ui(|| {
                    let panel = UI_STATE.with(|ui| ui.borrow().panel);
                    if let Some(panel) = panel {
                        repaint_panel(panel);
                    }
                });
                return Reply::Now(Err(("STITCH_RETRYABLE".into(), error.to_string())));
            }
        }

        Reply::Deferred
    }

    fn cancel(&mut self, request: &Request) -> Reply {
        if let Some(finish) = self.finish_worker.as_ref() {
            let cancellation = finish
                .job
                .request_cancel_rpc_with_intent(request.id.clone(), || {});
            match cancellation {
                FinishCancelRequest::Accepted | FinishCancelRequest::AlreadyRequested => {
                    return Reply::Deferred;
                }
                FinishCancelRequest::Completed(cancelled) => {
                    self.finish_worker_active();
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
                        send_event(SCROLL_CAPTURE_CANCELLED_EVENT, None);
                    }
                    return Reply::Now(Ok(Some(json!({
                        "cancelled": cancelled || was_capturing
                    }))));
                }
            }
        }

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
            send_event(SCROLL_CAPTURE_CANCELLED_EVENT, None);
        }
        Reply::Now(Ok(Some(json!({ "cancelled": was_capturing }))))
    }
}

impl Module for ScrollCaptureModule {
    fn name(&self) -> &'static str {
        SCROLL_CAPTURE_MODULE
    }

    fn handle(&mut self, request: &Request) -> Reply {
        match ScrollCaptureMethod::parse(&request.method) {
            Some(ScrollCaptureMethod::Start) => self.start(request),
            Some(ScrollCaptureMethod::StartAutoScroll) => self.start_auto_scroll(request),
            Some(ScrollCaptureMethod::StopAutoScroll) => self.stop_auto_scroll(request),
            Some(ScrollCaptureMethod::Finish) => self.finish(request),
            Some(ScrollCaptureMethod::Cancel) => self.cancel(request),
            None => method_not_found(&request.method),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::router::{Module, Router};
    use std::collections::HashMap;

    fn finish_request() -> Request {
        Request {
            id: "finish-1".to_string(),
            module: SCROLL_CAPTURE_MODULE.to_string(),
            method: "finish".to_string(),
            params: Some(HashMap::from([(
                "outputPath".to_string(),
                json!("scroll-capture-test.png"),
            )])),
        }
    }

    fn module_with_frame() -> ScrollCaptureModule {
        let module = ScrollCaptureModule::new();
        let mut state = module.state.lock().unwrap();
        state.is_capturing = true;
        assert_eq!(
            state.accumulator.submit(vec![0; 2 * 2 * 4], 2, 2, 1),
            CaptureOutcome::Added
        );
        drop(state);
        module
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
        assert_eq!(panel_button_label(0, false, false, false), "Auto");
        assert_eq!(panel_button_label(0, true, false, false), "Stop");
        assert_eq!(
            panel_button_palette(0, false, false, false)[0].0,
            NEUTRAL_BUTTON[0].0
        );
        assert_eq!(
            panel_button_palette(0, true, false, false)[0].0,
            ACTIVE_BUTTON[0].0
        );
        assert_eq!(
            panel_button_palette(1, false, false, false)[0].0,
            PRIMARY_BUTTON[0].0
        );
    }

    #[test]
    fn finishing_panel_disables_done_and_auto_while_cancel_stays_available() {
        assert_eq!(panel_button_label(1, false, true, false), "Saving");
        assert_eq!(panel_button_label(2, false, true, true), "Canceling");
        assert!(!panel_button_enabled(0, true, false));
        assert!(!panel_button_enabled(1, true, false));
        assert!(panel_button_enabled(2, true, false));
        assert!(!panel_button_enabled(2, true, true));
        assert_eq!(
            panel_button_palette(1, false, true, false)[0].0,
            PANEL_BACKGROUND.0
        );
    }

    #[test]
    fn cancellation_wins_before_finish_completion() {
        use std::time::Duration;

        let job = Arc::new(FinishJob::new());
        let writer_job = job.clone();
        let (started, started_rx) = std::sync::mpsc::channel();
        let (release, release_rx) = std::sync::mpsc::channel();
        let (completed, completed_rx) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            started.send(()).unwrap();
            release_rx
                .recv_timeout(Duration::from_secs(5))
                .expect("test did not release the held worker");
            completed
                .send(writer_job.complete(false).cancelled)
                .unwrap();
        });

        started_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("held worker did not start");
        assert!(matches!(
            job.request_cancel(),
            FinishCancelRequest::Accepted
        ));
        release.send(()).unwrap();

        assert!(
            completed_rx
                .recv_timeout(Duration::from_secs(5))
                .expect("worker did not complete")
        );
        worker.join().unwrap();
        assert!(job.is_cancelled());
        assert!(matches!(
            job.request_cancel(),
            FinishCancelRequest::Completed(true)
        ));
    }

    #[test]
    fn cancel_intent_is_written_before_finish_can_respond() {
        use std::time::Duration;

        let job = Arc::new(FinishJob::new());
        let order = Arc::new(Mutex::new(Vec::new()));
        let cancel_job = job.clone();
        let cancel_order = order.clone();
        let (intent_written, intent_written_rx) = std::sync::mpsc::channel();
        let (release_intent, release_intent_rx) = std::sync::mpsc::channel();
        let (cancelled, cancelled_rx) = std::sync::mpsc::channel();
        let cancel = std::thread::spawn(move || {
            let result = cancel_job.request_cancel_with_intent(|| {
                cancel_order.lock().unwrap().push("intent");
                intent_written.send(()).unwrap();
                release_intent_rx
                    .recv_timeout(Duration::from_secs(5))
                    .expect("test did not release cancellation intent");
            });
            cancelled.send(result).unwrap();
        });

        intent_written_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("cancel intent was not written");
        assert!(!job.is_cancelled());
        let finish_job = job.clone();
        let finish_order = order.clone();
        let (finish_done, finish_done_rx) = std::sync::mpsc::channel();
        let finish = std::thread::spawn(move || {
            let cancelled = finish_job.complete(false).cancelled;
            finish_order.lock().unwrap().push("finish");
            finish_done.send(cancelled).unwrap();
        });
        assert!(finish_done_rx.try_recv().is_err());

        release_intent.send(()).unwrap();
        assert!(matches!(
            cancelled_rx
                .recv_timeout(Duration::from_secs(5))
                .expect("cancel intent did not finish"),
            FinishCancelRequest::Accepted
        ));
        assert!(
            finish_done_rx
                .recv_timeout(Duration::from_secs(5))
                .expect("finish did not complete")
        );
        cancel.join().unwrap();
        finish.join().unwrap();
        assert_eq!(*order.lock().unwrap(), ["intent", "finish"]);
    }

    #[test]
    fn cancel_rpc_defers_until_the_finishing_worker_exits() {
        use std::time::{Duration, Instant};

        let mut module = ScrollCaptureModule::new();
        let job = Arc::new(FinishJob::new());
        let worker_job = job.clone();
        let state = module.state.clone();
        if let Ok(mut state) = state.lock() {
            state.is_capturing = true;
            state.is_finishing = true;
            state.finish_job = Some(job.clone());
        }
        let (started, started_rx) = std::sync::mpsc::channel();
        let (release, release_rx) = std::sync::mpsc::channel();
        let (acknowledged, acknowledged_rx) = std::sync::mpsc::channel();
        let writer = std::thread::spawn(move || {
            started.send(()).unwrap();
            release_rx
                .recv_timeout(Duration::from_secs(5))
                .expect("test did not release the held writer");
        });
        let supervisor = std::thread::spawn(move || {
            writer.join().unwrap();
            let completion = worker_job.complete(false);
            if let Ok(mut state) = state.lock() {
                state.reset();
            }
            respond_cancel_requests(completion.cancel_request_ids, completion.cancelled);
            acknowledged.send(()).unwrap();
        });
        module.finish_worker = Some(FinishWorker {
            job: job.clone(),
            supervisor,
        });
        started_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("held writer did not start");

        struct SharedScrollModule(Arc<Mutex<ScrollCaptureModule>>);

        impl Module for SharedScrollModule {
            fn name(&self) -> &'static str {
                SCROLL_CAPTURE_MODULE
            }

            fn handle(&mut self, request: &Request) -> Reply {
                self.0.lock().unwrap().handle(request)
            }
        }

        struct ProbeModule(std::sync::mpsc::Sender<()>);

        impl Module for ProbeModule {
            fn name(&self) -> &'static str {
                "probe"
            }

            fn handle(&mut self, _request: &Request) -> Reply {
                self.0.send(()).unwrap();
                Reply::Now(Ok(None))
            }
        }

        let (probed, probed_rx) = std::sync::mpsc::channel();
        let module = Arc::new(Mutex::new(module));
        let mut router = Router::new();
        router.register(Box::new(SharedScrollModule(module.clone())));
        router.register(Box::new(ProbeModule(probed)));
        router.route(Request {
            id: "cancel-1".to_string(),
            module: SCROLL_CAPTURE_MODULE.to_string(),
            method: "cancel".to_string(),
            params: None,
        });
        assert!(job.is_cancelled());
        router.route(Request {
            id: "probe-1".to_string(),
            module: "probe".to_string(),
            method: "ping".to_string(),
            params: None,
        });
        probed_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("unrelated daemon request was blocked by Cancel");
        assert!(acknowledged_rx.try_recv().is_err());

        release.send(()).unwrap();
        acknowledged_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("Cancel was not acknowledged after worker exit");
        let deadline = Instant::now() + Duration::from_secs(5);
        while !module
            .lock()
            .unwrap()
            .finish_worker
            .as_ref()
            .unwrap()
            .supervisor
            .is_finished()
        {
            assert!(Instant::now() < deadline, "supervisor did not exit");
            std::thread::yield_now();
        }
        assert!(!module.lock().unwrap().finish_worker_active());
    }

    #[test]
    fn supervisor_spawn_failure_keeps_frames_for_retry() {
        let mut module = module_with_frame();
        let reply = module.finish_with_spawners(
            &finish_request(),
            Box::new(|_| Err(io::Error::other("injected supervisor failure"))),
            Box::new(|task| thread::Builder::new().spawn(task)),
            Box::new(write_png_cancellable),
        );

        assert!(matches!(
            reply,
            Reply::Now(Err((code, _))) if code == "STITCH_RETRYABLE"
        ));
        let state = module.state.lock().unwrap();
        assert!(state.is_capturing);
        assert!(!state.is_finishing);
        assert!(!state.done_requested);
        assert_eq!(state.accumulator.frames().len(), 1);
        drop(state);
        assert!(matches!(
            module.cancel(&Request {
                id: "cancel-after-spawn-failure".to_string(),
                module: SCROLL_CAPTURE_MODULE.to_string(),
                method: "cancel".to_string(),
                params: None,
            }),
            Reply::Now(Ok(Some(value))) if value == json!({"cancelled": true})
        ));
        assert!(module.state.lock().unwrap().accumulator.frames().is_empty());
    }

    #[test]
    fn writer_spawn_failure_keeps_frames_for_retry() {
        let mut module = module_with_frame();
        let reply = module.finish_with_spawners(
            &finish_request(),
            Box::new(|task| {
                task();
                thread::Builder::new().spawn(|| {})
            }),
            Box::new(|_| Err(io::Error::other("injected writer failure"))),
            Box::new(write_png_cancellable),
        );

        assert!(matches!(reply, Reply::Deferred));
        let state = module.state.lock().unwrap();
        assert!(state.is_capturing);
        assert!(!state.is_finishing);
        assert!(!state.done_requested);
        assert_eq!(state.accumulator.frames().len(), 1);
        drop(state);
        assert!(!module.finish_worker_active());
        assert!(matches!(
            module.cancel(&Request {
                id: "cancel-after-writer-failure".to_string(),
                module: SCROLL_CAPTURE_MODULE.to_string(),
                method: "cancel".to_string(),
                params: None,
            }),
            Reply::Now(Ok(Some(value))) if value == json!({"cancelled": true})
        ));
        assert!(module.state.lock().unwrap().accumulator.frames().is_empty());
    }

    #[test]
    fn native_cancel_racing_retryable_finish_clears_retained_frames() {
        use std::time::{Duration, Instant};

        let module = module_with_frame();
        let state = module.state.clone();
        let job = Arc::new(FinishJob::new());
        {
            let mut state = state.lock().unwrap();
            state.is_finishing = true;
            state.done_requested = true;
            state.finish_job = Some(job.clone());
        }
        let (completed, completed_rx) = std::sync::mpsc::channel();
        let retry_state = state.clone();
        let retry_job = job.clone();
        let status = job.status.lock().unwrap();
        let retry = thread::spawn(move || {
            let completion = retryable_finish_completion(&retry_state, &retry_job);
            completed.send(completion.cancelled).unwrap();
        });

        let deadline = Instant::now() + Duration::from_secs(5);
        let mut completion_holds_capture_state = false;
        while Instant::now() < deadline {
            match state.try_lock() {
                Err(std::sync::TryLockError::WouldBlock) => {
                    completion_holds_capture_state = true;
                    break;
                }
                Ok(state) => drop(state),
                Err(std::sync::TryLockError::Poisoned(_)) => break,
            }
            thread::yield_now();
        }
        drop(status);
        assert!(
            completion_holds_capture_state,
            "retryable completion did not lock capture state before job status"
        );

        assert!(
            !completed_rx
                .recv_timeout(Duration::from_secs(5))
                .expect("retryable completion did not finish")
        );
        retry.join().unwrap();

        UI_STATE.with(|ui| ui.borrow_mut().capture_state = Some(state.clone()));
        cancel_from_ui();

        let state = state.lock().unwrap();
        assert!(!state.is_capturing);
        assert!(!state.is_finishing);
        assert!(state.accumulator.frames().is_empty());
        UI_STATE.with(|ui| ui.borrow_mut().capture_state = None);
    }

    #[test]
    fn writer_panic_becomes_a_terminal_stitch_error() {
        let mut module = module_with_frame();
        let reply = module.finish_with_spawners(
            &finish_request(),
            Box::new(|task| {
                task();
                thread::Builder::new().spawn(|| {})
            }),
            Box::new(|task| thread::Builder::new().spawn(task)),
            Box::new(|_, _, _| panic!("injected writer panic")),
        );

        assert!(matches!(reply, Reply::Deferred));
        let state = module.state.lock().unwrap();
        assert!(!state.is_capturing);
        assert!(!state.is_finishing);
        assert!(state.accumulator.frames().is_empty());
        drop(state);
        assert!(module.finish_worker.as_ref().unwrap().job.is_completed());
        assert!(!module.finish_worker_active());
    }

    #[test]
    fn finish_completion_wins_a_later_cancel() {
        let job = FinishJob::new();

        assert!(!job.complete(false).cancelled);
        assert!(matches!(
            job.request_cancel(),
            FinishCancelRequest::Completed(false)
        ));
        assert!(!job.is_cancelled());
    }

    #[test]
    fn retry_waits_until_the_finishing_worker_exits() {
        use std::time::Duration;

        let mut module = ScrollCaptureModule::new();
        let job = Arc::new(FinishJob::new());
        let worker_job = job.clone();
        let state = module.state.clone();
        if let Ok(mut state) = state.lock() {
            state.is_capturing = true;
            state.is_finishing = true;
            state.finish_job = Some(job.clone());
        }
        let (started, started_rx) = std::sync::mpsc::channel();
        let (release, release_rx) = std::sync::mpsc::channel();
        let (completed, completed_rx) = std::sync::mpsc::channel();
        let (exit, exit_rx) = std::sync::mpsc::channel();
        let worker_state = state.clone();
        let worker = std::thread::spawn(move || {
            started.send(()).unwrap();
            release_rx
                .recv_timeout(Duration::from_secs(5))
                .expect("test did not release the finishing worker");
            worker_job.complete(true);
            completed.send(()).unwrap();
            exit_rx
                .recv_timeout(Duration::from_secs(5))
                .expect("test did not release the supervisor");
            if let Ok(mut state) = worker_state.lock() {
                state.reset();
            }
        });
        module.finish_worker = Some(FinishWorker {
            job,
            supervisor: worker,
        });

        started_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("finishing worker did not start");
        assert!(module.prepare_start().is_err());
        assert!(module.finish_worker_active());
        assert!(module.state.lock().unwrap().is_capturing);
        release.send(()).unwrap();
        completed_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("finishing worker did not complete");

        let (retry_started, retry_started_rx) = std::sync::mpsc::channel();
        let (retry_result, retry_result_rx) = std::sync::mpsc::channel();
        let retry = std::thread::spawn(move || {
            retry_started.send(()).unwrap();
            retry_result.send(module.prepare_start()).unwrap();
        });
        retry_started_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("retry request did not start");
        assert!(retry_result_rx.try_recv().is_err());
        exit.send(()).unwrap();

        assert_eq!(
            retry_result_rx
                .recv_timeout(Duration::from_secs(5))
                .expect("retry stayed blocked after the supervisor exited"),
            Ok(())
        );
        retry.join().unwrap();
        assert!(!state.lock().unwrap().is_capturing);
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
