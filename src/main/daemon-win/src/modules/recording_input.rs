use super::recorder_types::{CaptureRect, RecorderError, StagedAsset, fit_rect};
use super::window_selector::window_bounds;
use serde::Serialize;
use std::ffi::c_void;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::thread::JoinHandle;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use windows::Win32::Foundation::{
    CloseHandle, HANDLE, HINSTANCE, HWND, LPARAM, LRESULT, POINT, WAIT_FAILED, WAIT_OBJECT_0,
    WPARAM,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::{CreateEventW, SetEvent};
use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
use windows::Win32::UI::WindowsAndMessaging::{
    CURSOR_SHOWING, CURSORINFO, CallNextHookEx, GetCursorInfo, GetCursorPos, IDC_CROSS, IDC_HAND,
    IDC_IBEAM, IDC_SIZENS, IDC_SIZEWE, KBDLLHOOKSTRUCT, LoadCursorW, MSG, MSLLHOOKSTRUCT,
    MsgWaitForMultipleObjects, PM_NOREMOVE, PM_REMOVE, PeekMessageW, QS_ALLINPUT,
    SetWindowsHookExW, UnhookWindowsHookEx, WH_KEYBOARD_LL, WH_MOUSE_LL, WM_KEYDOWN, WM_KEYUP,
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEHWHEEL, WM_MOUSEMOVE,
    WM_MOUSEWHEEL, WM_QUIT, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
};
use windows::core::PCWSTR;

const MOVEMENT_THRESHOLD: f64 = 0.001;
const WHEEL_DELTA: f64 = 120.0;
const MODIFIER_KEYS: [u32; 8] = [0x5B, 0x5C, 0xA0, 0xA1, 0xA2, 0xA3, 0xA4, 0xA5];

static ACTIVE_TRACKER: OnceLock<Mutex<Option<Weak<TrackerShared>>>> = OnceLock::new();

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CursorFile {
    recording_area: RecordingArea,
    events: Vec<CursorEvent>,
    meta: EventMeta,
}

#[derive(Serialize)]
struct RecordingArea {
    width: i32,
    height: i32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CursorEvent {
    timestamp: f64,
    x: f64,
    y: f64,
    #[serde(rename = "type")]
    event_type: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    button: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scroll_delta: Option<ScrollDelta>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cursor: Option<&'static str>,
}

#[derive(Serialize)]
struct ScrollDelta {
    x: f64,
    y: f64,
}

#[derive(Serialize)]
struct KeyboardFile {
    events: Vec<KeyboardEvent>,
    meta: EventMeta,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct KeyboardEvent {
    timestamp: f64,
    key: String,
    key_code: u32,
    modifiers: Vec<&'static str>,
    #[serde(rename = "type")]
    event_type: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EventMeta {
    start_time: String,
    duration: f64,
    sample_rate: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    platform: Option<&'static str>,
}

struct RawCursorEvent {
    wall_time: Instant,
    x: f64,
    y: f64,
    event_type: &'static str,
    button: Option<&'static str>,
    scroll_delta: Option<ScrollDelta>,
    cursor: Option<&'static str>,
}

struct RawKeyboardEvent {
    wall_time: Instant,
    key: String,
    key_code: u32,
    modifiers: Vec<&'static str>,
    event_type: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrackerSource {
    Screen,
    Window(isize),
}

/// Maps screen cursor positions into the recorded video frame. A window
/// recording keeps the frame it started with while the window itself moves and
/// resizes, so its cursor positions are read against the live window and then
/// placed inside the same letterbox the video frames go through.
#[derive(Clone, Copy, Debug)]
pub struct TrackerBounds {
    source: TrackerSource,
    frame: CaptureRect,
}

impl TrackerBounds {
    pub fn new(source: TrackerSource, frame: CaptureRect) -> Self {
        Self { source, frame }
    }

    fn area(&self) -> Option<CaptureRect> {
        let TrackerSource::Window(handle) = self.source else {
            return Some(self.frame);
        };

        let rect = window_bounds(HWND(handle as *mut c_void))?;
        Some(CaptureRect {
            x: rect.left,
            y: rect.top,
            width: rect.right - rect.left,
            height: rect.bottom - rect.top,
        })
    }

    fn normalize(&self, point: POINT) -> Option<(f64, f64)> {
        let area = self.area()?;
        let relative = relative_position(point, area)?;

        if self.source == TrackerSource::Screen {
            return Some(relative);
        }

        Some(place_in_frame(relative, area, self.frame))
    }
}

fn relative_position(point: POINT, area: CaptureRect) -> Option<(f64, f64)> {
    if area.width <= 0 || area.height <= 0 {
        return None;
    }

    Some((
        ((point.x - area.x) as f64 / area.width as f64).clamp(0.0, 1.0),
        ((point.y - area.y) as f64 / area.height as f64).clamp(0.0, 1.0),
    ))
}

fn place_in_frame(relative: (f64, f64), area: CaptureRect, frame: CaptureRect) -> (f64, f64) {
    if frame.width <= 0 || frame.height <= 0 {
        return relative;
    }

    let fit = fit_rect(
        (area.width as u32, area.height as u32),
        (frame.width as u32, frame.height as u32),
    );
    if fit.width <= 0.0 || fit.height <= 0.0 {
        return relative;
    }

    (
        (fit.x + relative.0 * fit.width) / frame.width as f64,
        (fit.y + relative.1 * fit.height) / frame.height as f64,
    )
}

struct TrackerInner {
    bounds: TrackerBounds,
    keyboard_enabled: bool,
    synced_at: Option<Instant>,
    synced_system_time: Option<SystemTime>,
    paused: bool,
    pause_start: Option<Instant>,
    pauses: Vec<(Instant, Instant)>,
    cursor_events: Vec<RawCursorEvent>,
    keyboard_events: Vec<RawKeyboardEvent>,
    last_x: f64,
    last_y: f64,
    last_cursor: Option<&'static str>,
    modifiers: ModifierState,
}

#[derive(Clone)]
struct ModifierState {
    down: [bool; 256],
}

impl ModifierState {
    fn from_system() -> Self {
        let mut state = Self::empty();
        for virtual_key in MODIFIER_KEYS {
            state.update(virtual_key, key_is_down(virtual_key));
        }
        state
    }

    fn empty() -> Self {
        Self { down: [false; 256] }
    }

    fn update(&mut self, virtual_key: u32, down: bool) {
        if is_modifier_key(virtual_key) {
            self.down[virtual_key as usize] = down;
        }
    }

    fn serialized(&self) -> Vec<&'static str> {
        let mut modifiers = Vec::new();
        if self.any_down(&[0x5B, 0x5C]) {
            modifiers.push("meta");
        }
        if self.any_down(&[0x11, 0xA2, 0xA3]) {
            modifiers.push("control");
        }
        if self.any_down(&[0x12, 0xA4, 0xA5]) {
            modifiers.push("alt");
        }
        if self.any_down(&[0x10, 0xA0, 0xA1]) {
            modifiers.push("shift");
        }
        modifiers
    }

    fn any_down(&self, virtual_keys: &[u32]) -> bool {
        virtual_keys.iter().any(|key| self.down[*key as usize])
    }
}

struct TrackerShared {
    inner: Mutex<TrackerInner>,
}

impl TrackerShared {
    fn new(bounds: TrackerBounds, keyboard_enabled: bool) -> Self {
        Self {
            inner: Mutex::new(TrackerInner {
                bounds,
                keyboard_enabled,
                synced_at: None,
                synced_system_time: None,
                paused: false,
                pause_start: None,
                pauses: Vec::new(),
                cursor_events: Vec::new(),
                keyboard_events: Vec::new(),
                last_x: -1.0,
                last_y: -1.0,
                last_cursor: None,
                modifiers: ModifierState::from_system(),
            }),
        }
    }

    fn sync(&self, wall_time: Instant, system_time: SystemTime) {
        let Ok(mut inner) = self.inner.lock() else {
            return;
        };

        if inner.synced_at.is_some() {
            return;
        }

        inner.synced_at = Some(wall_time);
        inner.synced_system_time = Some(system_time);
    }

    fn pause(&self) {
        let Ok(mut inner) = self.inner.lock() else {
            return;
        };

        if inner.paused {
            return;
        }

        inner.paused = true;
        inner.pause_start = Some(Instant::now());
    }

    fn resume(&self) {
        let Ok(mut inner) = self.inner.lock() else {
            return;
        };

        if !inner.paused {
            return;
        }

        if let Some(start) = inner.pause_start.take() {
            inner.pauses.push((start, Instant::now()));
        }
        inner.paused = false;
    }

    fn record_mouse(
        &self,
        point: POINT,
        event_type: &'static str,
        button: Option<&'static str>,
        scroll_delta: Option<ScrollDelta>,
    ) {
        let cursor = current_cursor_type();
        let Ok(mut inner) = self.inner.lock() else {
            return;
        };

        if inner.paused {
            return;
        }

        let Some((x, y)) = inner.bounds.normalize(point) else {
            return;
        };

        if event_type == "move"
            && (x - inner.last_x).abs() < MOVEMENT_THRESHOLD
            && (y - inner.last_y).abs() < MOVEMENT_THRESHOLD
        {
            return;
        }

        let changed_cursor = if inner.last_cursor != Some(cursor) {
            inner.last_cursor = Some(cursor);
            Some(cursor)
        } else {
            None
        };

        inner.last_x = x;
        inner.last_y = y;
        inner.cursor_events.push(RawCursorEvent {
            wall_time: Instant::now(),
            x,
            y,
            event_type,
            button,
            scroll_delta,
            cursor: changed_cursor,
        });
    }

    fn record_keyboard(&self, virtual_key: u32, event_type: &'static str) {
        let Ok(mut inner) = self.inner.lock() else {
            return;
        };

        inner.modifiers.update(virtual_key, event_type == "down");
        if is_modifier_key(virtual_key) {
            return;
        }

        if inner.paused || !inner.keyboard_enabled {
            return;
        }

        let modifiers = inner.modifiers.serialized();
        inner.keyboard_events.push(RawKeyboardEvent {
            wall_time: Instant::now(),
            key: key_name(virtual_key),
            key_code: virtual_key,
            modifiers,
            event_type,
        });
    }

    fn files(&self, duration: f64) -> Result<(CursorFile, Option<KeyboardFile>), RecorderError> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| RecorderError::capture("Input tracker state is unavailable"))?;

        if inner.paused {
            if let Some(start) = inner.pause_start.take() {
                inner.pauses.push((start, Instant::now()));
            }
            inner.paused = false;
        }

        let Some(origin) = inner.synced_at else {
            return Err(RecorderError::capture(
                "Input tracker was not synchronized to the first video frame",
            ));
        };
        let start_time = format_system_time(inner.synced_system_time.unwrap_or(SystemTime::now()));
        let cursor_count = inner.cursor_events.len();
        let keyboard_count = inner.keyboard_events.len();

        let cursor_events = inner
            .cursor_events
            .iter()
            .map(|event| CursorEvent {
                timestamp: event_timestamp(event.wall_time, origin, &inner.pauses),
                x: event.x,
                y: event.y,
                event_type: event.event_type,
                button: event.button,
                scroll_delta: event.scroll_delta.as_ref().map(|delta| ScrollDelta {
                    x: delta.x,
                    y: delta.y,
                }),
                cursor: event.cursor,
            })
            .collect();

        let cursor_file = CursorFile {
            recording_area: RecordingArea {
                width: inner.bounds.frame.width,
                height: inner.bounds.frame.height,
            },
            events: cursor_events,
            meta: EventMeta {
                start_time: start_time.clone(),
                duration,
                sample_rate: sample_rate(cursor_count, duration),
                platform: None,
            },
        };

        let keyboard_file = if inner.keyboard_enabled {
            Some(KeyboardFile {
                events: inner
                    .keyboard_events
                    .iter()
                    .map(|event| KeyboardEvent {
                        timestamp: event_timestamp(event.wall_time, origin, &inner.pauses),
                        key: event.key.clone(),
                        key_code: event.key_code,
                        modifiers: event.modifiers.clone(),
                        event_type: event.event_type,
                    })
                    .collect(),
                meta: EventMeta {
                    start_time,
                    duration,
                    sample_rate: sample_rate(keyboard_count, duration),
                    platform: Some("windows"),
                },
            })
        } else {
            None
        };

        Ok((cursor_file, keyboard_file))
    }
}

pub struct InputTracker {
    shared: Arc<TrackerShared>,
    stop_event: Option<HANDLE>,
    terminal: Receiver<Result<(), String>>,
    thread: Option<JoinHandle<Result<(), String>>>,
}

impl InputTracker {
    pub fn start(bounds: TrackerBounds, keyboard_enabled: bool) -> Result<Self, RecorderError> {
        let shared = Arc::new(TrackerShared::new(bounds, keyboard_enabled));
        let active = ACTIVE_TRACKER.get_or_init(|| Mutex::new(None));
        let mut current = active
            .lock()
            .map_err(|_| RecorderError::capture("Input tracker registry is unavailable"))?;

        if current.as_ref().and_then(Weak::upgrade).is_some() {
            return Err(RecorderError::invalid_state(
                "Another input tracker is already active",
            ));
        }

        let stop_event =
            unsafe { CreateEventW(None, true, false, PCWSTR::null()) }.map_err(|error| {
                RecorderError::capture(format!("Failed to create input hook stop event: {error}"))
            })?;
        *current = Some(Arc::downgrade(&shared));
        drop(current);
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let (terminal_tx, terminal_rx) = std::sync::mpsc::channel();
        let stop_event_bits = stop_event.0 as usize;
        let thread = std::thread::spawn(move || {
            let stop_event = HANDLE(stop_event_bits as *mut _);
            let result = std::panic::catch_unwind(|| {
                run_hook_thread(keyboard_enabled, stop_event, ready_tx)
            })
            .unwrap_or_else(|_| Err("Input hook thread terminated unexpectedly".to_string()));
            let _ = terminal_tx.send(result.clone());
            result
        });
        let ready = match ready_rx.recv() {
            Ok(ready) => ready,
            Err(_) => {
                clear_active_tracker();
                let _ = thread.join();
                unsafe {
                    let _ = CloseHandle(stop_event);
                }
                return Err(RecorderError::capture("Input hook thread did not start"));
            }
        };

        match ready {
            Ok(()) => {}
            Err(message) => {
                clear_active_tracker();
                let _ = thread.join();
                unsafe {
                    let _ = CloseHandle(stop_event);
                }
                return Err(RecorderError::capture(message));
            }
        }

        let tracker = Self {
            shared,
            stop_event: Some(stop_event),
            terminal: terminal_rx,
            thread: Some(thread),
        };
        tracker.record_initial_cursor();
        Ok(tracker)
    }

    pub fn sync_with_first_frame(&self) {
        self.shared.sync(Instant::now(), SystemTime::now());
    }

    pub fn pause(&self) {
        self.shared.pause();
    }

    pub fn resume(&self) {
        self.shared.resume();
    }

    pub fn try_error(&self) -> Option<RecorderError> {
        match self.terminal.try_recv() {
            Ok(Ok(())) => Some(RecorderError::capture(
                "Input hook thread stopped unexpectedly",
            )),
            Ok(Err(message)) => Some(RecorderError::capture(message)),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(RecorderError::capture(
                "Input hook health channel disconnected",
            )),
        }
    }

    pub fn finish(
        mut self,
        output_path: &Path,
        duration: f64,
    ) -> Result<InputAssetPaths, RecorderError> {
        self.stop_hooks()?;
        let (cursor, keyboard) = self.shared.files(duration)?;
        let parent = output_path
            .parent()
            .ok_or_else(|| RecorderError::capture("Recording output has no parent directory"))?;
        let cursor_path = parent.join("cursor.json");
        let cursor_temporary_path = parent.join(".cursor.json.poratake-staged");
        write_json_atomically(&cursor_temporary_path, &cursor)?;

        let keys = if let Some(keyboard) = keyboard {
            let path = parent.join("keys.json");
            let temporary_path = parent.join(".keys.json.poratake-staged");
            if let Err(error) = write_json_atomically(&temporary_path, &keyboard) {
                let _ = std::fs::remove_file(&cursor_temporary_path);
                return Err(error);
            }
            Some(StagedAsset {
                temporary_path,
                output_path: path,
            })
        } else {
            None
        };

        Ok(InputAssetPaths {
            cursor: StagedAsset {
                temporary_path: cursor_temporary_path,
                output_path: cursor_path,
            },
            keys,
        })
    }

    pub fn abort(mut self) {
        let _ = self.stop_hooks();
    }

    fn record_initial_cursor(&self) {
        let mut point = POINT::default();
        if unsafe { GetCursorPos(&mut point) }.is_ok() {
            self.shared.record_mouse(point, "move", None, None);
        }
    }

    fn stop_hooks(&mut self) -> Result<(), RecorderError> {
        if self.thread.is_none() {
            return Ok(());
        }

        let (stop_event, thread) = signal_and_take_hook_resources(
            &mut self.stop_event,
            &mut self.thread,
            |stop_event| unsafe { SetEvent(stop_event) },
        )?;
        let joined = thread.join();
        let closed = unsafe { CloseHandle(stop_event) };
        clear_active_tracker();
        let result = joined
            .map_err(|_| RecorderError::capture("Input hook thread terminated unexpectedly"))?;
        closed.map_err(|error| {
            RecorderError::capture(format!("Failed to close input hook stop event: {error}"))
        })?;
        result.map_err(RecorderError::capture)
    }
}

fn signal_and_take_hook_resources<T>(
    stop_event: &mut Option<HANDLE>,
    thread: &mut Option<T>,
    signal: impl FnOnce(HANDLE) -> windows::core::Result<()>,
) -> Result<(HANDLE, T), RecorderError> {
    let event = stop_event
        .as_ref()
        .copied()
        .ok_or_else(|| RecorderError::capture("Input hook stop event is unavailable"))?;
    if thread.is_none() {
        return Err(RecorderError::capture("Input hook thread is unavailable"));
    }
    signal(event).map_err(|error| {
        RecorderError::capture(format!("Failed to signal input hook shutdown: {error}"))
    })?;
    let event = stop_event
        .take()
        .ok_or_else(|| RecorderError::capture("Input hook stop event is unavailable"))?;
    let thread = thread
        .take()
        .ok_or_else(|| RecorderError::capture("Input hook thread is unavailable"))?;
    Ok((event, thread))
}

pub struct InputAssetPaths {
    pub cursor: StagedAsset,
    pub keys: Option<StagedAsset>,
}

impl Drop for InputTracker {
    fn drop(&mut self) {
        let _ = self.stop_hooks();
    }
}

fn run_hook_thread(
    keyboard_enabled: bool,
    stop_event: HANDLE,
    ready: std::sync::mpsc::Sender<Result<(), String>>,
) -> Result<(), String> {
    let mut message = MSG::default();
    unsafe {
        let _ = PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE);
    }

    let module = match unsafe { GetModuleHandleW(None) } {
        Ok(module) => HINSTANCE(module.0),
        Err(error) => {
            let _ = ready.send(Err(format!("Failed to resolve input hook module: {error}")));
            return Ok(());
        }
    };

    let mouse_hook =
        match unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook_proc), Some(module), 0) } {
            Ok(hook) => hook,
            Err(error) => {
                let _ = ready.send(Err(format!("Failed to install mouse hook: {error}")));
                return Ok(());
            }
        };

    let keyboard_hook = if keyboard_enabled {
        match unsafe {
            SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook_proc), Some(module), 0)
        } {
            Ok(hook) => Some(hook),
            Err(error) => {
                unsafe {
                    let _ = UnhookWindowsHookEx(mouse_hook);
                }
                let _ = ready.send(Err(format!("Failed to install keyboard hook: {error}")));
                return Ok(());
            }
        }
    } else {
        None
    };

    if ready.send(Ok(())).is_err() {
        unsafe {
            let _ = UnhookWindowsHookEx(mouse_hook);
            if let Some(hook) = keyboard_hook {
                let _ = UnhookWindowsHookEx(hook);
            }
        }
        return Err("Input tracker stopped before hook startup completed".to_string());
    }

    let loop_result = hook_message_loop(stop_event);
    let mouse_result = unsafe { UnhookWindowsHookEx(mouse_hook) }
        .map_err(|error| format!("Failed to remove mouse hook: {error}"));
    let keyboard_result = keyboard_hook
        .map(|hook| {
            unsafe { UnhookWindowsHookEx(hook) }
                .map_err(|error| format!("Failed to remove keyboard hook: {error}"))
        })
        .unwrap_or(Ok(()));
    loop_result?;
    mouse_result?;
    keyboard_result
}

fn hook_message_loop(stop_event: HANDLE) -> Result<(), String> {
    let handles = [stop_event];
    let mut message = MSG::default();
    loop {
        let wait =
            unsafe { MsgWaitForMultipleObjects(Some(&handles), false, u32::MAX, QS_ALLINPUT) };
        if wait == WAIT_OBJECT_0 {
            return Ok(());
        }
        if wait == WAIT_FAILED {
            return Err(format!(
                "Input hook message wait failed: {}",
                windows::core::Error::from_thread()
            ));
        }
        if wait.0 != WAIT_OBJECT_0.0 + handles.len() as u32 {
            return Err(format!("Input hook message wait returned {}", wait.0));
        }

        while unsafe { PeekMessageW(&mut message, None, 0, 0, PM_REMOVE) }.as_bool() {
            if message.message == WM_QUIT {
                return Err("Input hook thread received an unexpected quit message".to_string());
            }
        }
    }
}

unsafe extern "system" fn mouse_hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let info = unsafe { &*(lparam.0 as *const MSLLHOOKSTRUCT) };
        let message = wparam.0 as u32;
        if let Some(shared) = active_tracker() {
            match message {
                WM_MOUSEMOVE => shared.record_mouse(info.pt, "move", None, None),
                WM_LBUTTONDOWN => shared.record_mouse(info.pt, "down", Some("left"), None),
                WM_LBUTTONUP => shared.record_mouse(info.pt, "up", Some("left"), None),
                WM_RBUTTONDOWN => shared.record_mouse(info.pt, "down", Some("right"), None),
                WM_RBUTTONUP => shared.record_mouse(info.pt, "up", Some("right"), None),
                WM_MBUTTONDOWN => shared.record_mouse(info.pt, "down", Some("middle"), None),
                WM_MBUTTONUP => shared.record_mouse(info.pt, "up", Some("middle"), None),
                WM_MOUSEWHEEL => shared.record_mouse(
                    info.pt,
                    "scroll",
                    None,
                    Some(ScrollDelta {
                        x: 0.0,
                        y: wheel_delta(info.mouseData),
                    }),
                ),
                WM_MOUSEHWHEEL => shared.record_mouse(
                    info.pt,
                    "scroll",
                    None,
                    Some(ScrollDelta {
                        x: wheel_delta(info.mouseData),
                        y: 0.0,
                    }),
                ),
                _ => {}
            }
        }
    }

    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

unsafe extern "system" fn keyboard_hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let info = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
        let event_type = match wparam.0 as u32 {
            WM_KEYDOWN | WM_SYSKEYDOWN => Some("down"),
            WM_KEYUP | WM_SYSKEYUP => Some("up"),
            _ => None,
        };

        if let Some(event_type) = event_type {
            if let Some(shared) = active_tracker() {
                shared.record_keyboard(info.vkCode, event_type);
            }
        }
    }

    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

fn active_tracker() -> Option<Arc<TrackerShared>> {
    ACTIVE_TRACKER
        .get()?
        .lock()
        .ok()?
        .as_ref()
        .and_then(Weak::upgrade)
}

fn clear_active_tracker() {
    if let Some(active) = ACTIVE_TRACKER.get() {
        if let Ok(mut current) = active.lock() {
            *current = None;
        }
    }
}

fn wheel_delta(mouse_data: u32) -> f64 {
    ((mouse_data >> 16) as u16 as i16) as f64 / WHEEL_DELTA
}

fn current_cursor_type() -> &'static str {
    let mut info = CURSORINFO {
        cbSize: std::mem::size_of::<CURSORINFO>() as u32,
        ..Default::default()
    };

    if unsafe { GetCursorInfo(&mut info) }.is_err() || info.flags != CURSOR_SHOWING {
        return "arrow";
    }

    let handle = info.hCursor.0 as usize;
    let standard = standard_cursor_handles();
    if handle == standard.hand {
        return "pointingHand";
    }
    if handle == standard.ibeam {
        return "iBeam";
    }
    if handle == standard.cross {
        return "crosshair";
    }
    if handle == standard.size_we {
        return "resizeLeftRight";
    }
    if handle == standard.size_ns {
        return "resizeUpDown";
    }
    "arrow"
}

struct StandardCursorHandles {
    hand: usize,
    ibeam: usize,
    cross: usize,
    size_we: usize,
    size_ns: usize,
}

fn standard_cursor_handles() -> &'static StandardCursorHandles {
    static HANDLES: OnceLock<StandardCursorHandles> = OnceLock::new();
    HANDLES.get_or_init(|| StandardCursorHandles {
        hand: unsafe { LoadCursorW(None, IDC_HAND) }.unwrap_or_default().0 as usize,
        ibeam: unsafe { LoadCursorW(None, IDC_IBEAM) }
            .unwrap_or_default()
            .0 as usize,
        cross: unsafe { LoadCursorW(None, IDC_CROSS) }
            .unwrap_or_default()
            .0 as usize,
        size_we: unsafe { LoadCursorW(None, IDC_SIZEWE) }
            .unwrap_or_default()
            .0 as usize,
        size_ns: unsafe { LoadCursorW(None, IDC_SIZENS) }
            .unwrap_or_default()
            .0 as usize,
    })
}

fn is_modifier_key(virtual_key: u32) -> bool {
    matches!(
        virtual_key,
        0x10 | 0x11 | 0x12 | 0x5B | 0x5C | 0xA0 | 0xA1 | 0xA2 | 0xA3 | 0xA4 | 0xA5
    )
}

fn key_is_down(virtual_key: u32) -> bool {
    (unsafe { GetAsyncKeyState(virtual_key as i32) }) < 0
}

fn key_name(virtual_key: u32) -> String {
    if (0x30..=0x39).contains(&virtual_key) || (0x41..=0x5A).contains(&virtual_key) {
        return char::from_u32(virtual_key).unwrap_or('?').to_string();
    }

    if (0x70..=0x87).contains(&virtual_key) {
        return format!("F{}", virtual_key - 0x6F);
    }

    if (0x60..=0x69).contains(&virtual_key) {
        return format!("Keypad{}", virtual_key - 0x60);
    }

    match virtual_key {
        0x08 => "Delete",
        0x09 => "Tab",
        0x0D => "Return",
        0x1B => "Escape",
        0x20 => "Space",
        0x21 => "PageUp",
        0x22 => "PageDown",
        0x23 => "End",
        0x24 => "Home",
        0x25 => "LeftArrow",
        0x26 => "UpArrow",
        0x27 => "RightArrow",
        0x28 => "DownArrow",
        0x2D => "Insert",
        0x2E => "ForwardDelete",
        0x6A => "KeypadMultiply",
        0x6B => "KeypadPlus",
        0x6D => "KeypadMinus",
        0x6E => "KeypadDecimal",
        0x6F => "KeypadDivide",
        0xBA => ";",
        0xBB => "=",
        0xBC => ",",
        0xBD => "-",
        0xBE => ".",
        0xBF => "/",
        0xC0 => "`",
        0xDB => "[",
        0xDC => "\\",
        0xDD => "]",
        0xDE => "'",
        _ => return format!("Key{virtual_key}"),
    }
    .to_string()
}

fn event_timestamp(wall_time: Instant, origin: Instant, pauses: &[(Instant, Instant)]) -> f64 {
    if wall_time <= origin {
        return 0.0;
    }

    let mut elapsed = wall_time.duration_since(origin);
    for (start, end) in pauses {
        if *start >= wall_time || *end <= origin {
            continue;
        }

        let overlap_start = (*start).max(origin);
        let overlap_end = (*end).min(wall_time);
        elapsed = elapsed.saturating_sub(overlap_end.duration_since(overlap_start));
    }
    elapsed.as_secs_f64()
}

fn sample_rate(count: usize, duration: f64) -> u64 {
    if count == 0 || duration <= 0.0 {
        return 0;
    }
    (count as f64 / duration).floor() as u64
}

fn write_json_atomically(path: &Path, value: &impl Serialize) -> Result<(), RecorderError> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| RecorderError::capture("Invalid tracker asset path"))?;
    let temporary = path.with_file_name(format!(".{file_name}.tmp"));
    let file = File::create(&temporary).map_err(|error| {
        RecorderError::capture(format!("Failed to create tracker asset: {error}"))
    })?;
    let mut writer = BufWriter::new(file);
    serde_json::to_writer_pretty(&mut writer, value).map_err(|error| {
        RecorderError::capture(format!("Failed to encode tracker asset: {error}"))
    })?;
    writer.flush().map_err(|error| {
        RecorderError::capture(format!("Failed to flush tracker asset: {error}"))
    })?;
    drop(writer);

    if path.exists() {
        std::fs::remove_file(path).map_err(|error| {
            RecorderError::capture(format!("Failed to replace tracker asset: {error}"))
        })?;
    }
    std::fs::rename(&temporary, path).map_err(|error| {
        RecorderError::capture(format!("Failed to publish tracker asset: {error}"))
    })
}

fn format_system_time(time: SystemTime) -> String {
    let elapsed = time.duration_since(UNIX_EPOCH).unwrap_or_default();
    let total_seconds = elapsed.as_secs() as i64;
    let days = total_seconds.div_euclid(86_400);
    let seconds = total_seconds.rem_euclid(86_400);
    let (year, month, day) = civil_date(days);
    let hour = seconds / 3_600;
    let minute = (seconds % 3_600) / 60;
    let second = seconds % 60;
    format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{:03}Z",
        elapsed.subsec_millis()
    )
}

fn civil_date(days_since_epoch: i64) -> (i64, i64, i64) {
    let shifted = days_since_epoch + 719_468;
    let era = if shifted >= 0 {
        shifted
    } else {
        shifted - 146_096
    } / 146_097;
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn rect(x: i32, y: i32, width: i32, height: i32) -> CaptureRect {
        CaptureRect {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn a_moved_window_keeps_the_cursor_at_the_same_spot_in_the_frame() {
        let frame = rect(0, 0, 800, 600);
        let start = rect(100, 100, 800, 600);
        let moved = rect(500, 300, 800, 600);

        let before = place_in_frame(
            relative_position(POINT { x: 500, y: 400 }, start).expect("relative"),
            start,
            frame,
        );
        let after = place_in_frame(
            relative_position(POINT { x: 900, y: 600 }, moved).expect("relative"),
            moved,
            frame,
        );

        assert_eq!(before, after);
    }

    #[test]
    fn a_resized_window_places_the_cursor_inside_the_letterbox() {
        let frame = rect(0, 0, 800, 600);
        let narrow = rect(0, 0, 400, 600);

        let centre = place_in_frame((0.5, 0.5), narrow, frame);
        let left_edge = place_in_frame((0.0, 0.0), narrow, frame);

        assert_eq!(centre, (0.5, 0.5));
        assert_eq!(left_edge, (0.25, 0.0));
    }

    #[test]
    fn cursor_positions_stay_inside_the_recorded_area() {
        let area = rect(100, 100, 400, 400);

        assert_eq!(
            relative_position(POINT { x: -50, y: 5_000 }, area),
            Some((0.0, 1.0))
        );
        assert_eq!(
            relative_position(POINT { x: 0, y: 0 }, rect(0, 0, 0, 0)),
            None
        );
    }

    #[test]
    fn modifier_state_serializes_control_alt_k_chord_in_contract_order() {
        let mut modifiers = ModifierState::empty();
        modifiers.update(0xA2, true);
        modifiers.update(0xA5, true);

        assert_eq!(modifiers.serialized(), vec!["control", "alt"]);
        assert_eq!(key_name(0x4B), "K");
    }

    #[test]
    fn modifier_state_tracks_left_and_right_release_independently() {
        let mut modifiers = ModifierState::empty();
        modifiers.update(0xA2, true);
        modifiers.update(0xA3, true);
        modifiers.update(0xA2, false);
        assert_eq!(modifiers.serialized(), vec!["control"]);

        modifiers.update(0xA3, false);
        assert!(modifiers.serialized().is_empty());
    }

    #[test]
    fn key_mapping_and_wheel_delta_match_windows_contract() {
        assert_eq!(key_name(0x70), "F1");
        assert_eq!(key_name(0x6B), "KeypadPlus");
        assert_eq!(wheel_delta((120_u32) << 16), 1.0);
        assert_eq!(wheel_delta(((-120_i16) as u16 as u32) << 16), -1.0);
    }

    #[test]
    fn timestamp_excludes_only_pause_overlap_before_event() {
        let origin = Instant::now();
        let pause_start = origin + Duration::from_secs(2);
        let pause_end = origin + Duration::from_secs(5);
        let event = origin + Duration::from_secs(8);

        assert_eq!(
            event_timestamp(event, origin, &[(pause_start, pause_end)]),
            5.0
        );
        assert_eq!(
            event_timestamp(origin, origin, &[(pause_start, pause_end)]),
            0.0
        );
    }

    #[test]
    fn failed_stop_signal_retains_resources_for_retry() {
        let mut stop_event = Some(HANDLE(1_usize as *mut std::ffi::c_void));
        let mut thread = Some(42);

        let first = signal_and_take_hook_resources(&mut stop_event, &mut thread, |_| {
            Err(windows::core::Error::from_hresult(
                windows::Win32::Foundation::E_FAIL,
            ))
        });
        assert!(first.is_err());
        assert!(stop_event.is_some());
        assert_eq!(thread, Some(42));

        let second = signal_and_take_hook_resources(&mut stop_event, &mut thread, |_| Ok(()));
        let Ok((event, thread_token)) = second else {
            panic!("retained hook resources should remain available for retry");
        };
        assert_eq!(event.0 as usize, 1);
        assert_eq!(thread_token, 42);
        assert!(stop_event.is_none());
        assert!(thread.is_none());
    }
}
