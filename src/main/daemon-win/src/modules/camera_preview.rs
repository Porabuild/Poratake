use super::camera_devices::{enumerate_cameras, select_camera};
use crate::com::{MtaInterface, retain_process_mta};
use crate::overlay::{
    create_popup_window, default_wndproc, disable_window_transitions, ensure_window_class,
    monitors, scale_for_dpi,
};
use crate::protocol::{Request, params, respond_error, respond_success};
use crate::router::{Module, Reply, method_not_found};
use crate::ui::run_on_ui;
use poratake_daemon_common::contract::{
    CAMERA_PREVIEW_MODULE, CameraPreviewMethod, CameraPreviewRequest, CameraPreviewUpdateRequest,
    ContentProtectionRequest, UpdateField,
};
use serde_json::json;
use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::JoinHandle;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BeginPaint, BitBlt, CreateCompatibleBitmap,
    CreateCompatibleDC, CreateRoundRectRgn, CreateSolidBrush, DIB_RGB_COLORS, DeleteDC,
    DeleteObject, EndPaint, FillRect, HALFTONE, InvalidateRect, PAINTSTRUCT, SRCCOPY, SelectObject,
    SetStretchBltMode, SetWindowRgn, StretchDIBits,
};
use windows::Win32::Media::MediaFoundation::{
    IMFMediaSource, IMFSample, IMFSourceReader, MF_MT_FRAME_SIZE, MF_MT_MAJOR_TYPE, MF_MT_SUBTYPE,
    MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING,
    MF_SOURCE_READER_FIRST_VIDEO_STREAM, MF_SOURCE_READERF_ENDOFSTREAM, MF_VERSION,
    MFCreateAttributes, MFCreateMediaType, MFCreateSourceReaderFromMediaSource, MFMediaType_Video,
    MFSTARTUP_FULL, MFShutdown, MFStartup, MFVideoFormat_RGB32,
};
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    DestroyWindow, GetClientRect, GetWindowRect, HTCAPTION, HWND_TOPMOST, LWA_ALPHA, PostMessageW,
    SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOMOVE, SetLayeredWindowAttributes,
    SetWindowDisplayAffinity, SetWindowPos, ShowWindow, WDA_EXCLUDEFROMCAPTURE, WDA_NONE, WM_APP,
    WM_DPICHANGED, WM_EXITSIZEMOVE, WM_NCHITTEST, WM_PAINT, WS_EX_LAYERED, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
};

const CLASS_NAME: &str = "PoratakeCameraPreview";
const PREVIEW_SIZE: i32 = 230;
const SHADOW_PADDING: i32 = 20;
const TOTAL_SIZE: i32 = PREVIEW_SIZE + SHADOW_PADDING * 2;
const WM_CAMERA_FRAME: u32 = WM_APP + 32;

#[derive(Clone, PartialEq)]
struct CameraConfig {
    device_id: Option<String>,
    device_name: Option<String>,
    resolution: String,
    flipped: bool,
    content_protected: bool,
}

impl Default for CameraConfig {
    fn default() -> Self {
        Self {
            device_id: None,
            device_name: None,
            resolution: "720p".to_string(),
            flipped: false,
            content_protected: false,
        }
    }
}

#[derive(Clone)]
struct CameraFrame {
    pixels: Arc<[u8]>,
    width: i32,
    height: i32,
}

pub(crate) struct CameraRuntime {
    lifecycle: Mutex<CameraLifecycle>,
    capture: Mutex<Option<CaptureSession>>,
    transition: Mutex<()>,
    position: Mutex<Option<(i32, i32)>>,
    frame: Arc<Mutex<Option<CameraFrame>>>,
    config: Mutex<CameraConfig>,
    ownership: Mutex<Option<RecordingOwnership>>,
    next_token: AtomicU64,
}

#[derive(Clone, Copy)]
struct PreviewHandoff {
    generation: u64,
    wanted: bool,
    synced: bool,
}

struct RecordingOwnership {
    token: u64,
    handoff: PreviewHandoff,
}

pub(crate) struct RecordingPreview {
    runtime: Arc<CameraRuntime>,
    token: u64,
    post_pending: Arc<AtomicBool>,
}

static SHARED_RUNTIME: OnceLock<Arc<CameraRuntime>> = OnceLock::new();

pub(crate) fn shared_runtime() -> Arc<CameraRuntime> {
    SHARED_RUNTIME
        .get_or_init(|| Arc::new(CameraRuntime::new()))
        .clone()
}

#[derive(Clone, Copy)]
struct CameraLifecycle {
    generation: u64,
    starting: bool,
    running: bool,
    visible: bool,
}

struct CaptureInterrupt {
    reader: MtaInterface<IMFSourceReader>,
    source: MtaInterface<IMFMediaSource>,
}

struct CaptureSession {
    generation: u64,
    stop: Arc<AtomicBool>,
    interrupt: Arc<Mutex<Option<CaptureInterrupt>>>,
    thread: JoinHandle<()>,
}

#[derive(Clone, Copy)]
enum CaptureTransitionOutcome {
    Current,
    Replaced,
    Failed,
}

enum CameraWindowError {
    Create,
    ContentProtection,
}

impl CameraWindowError {
    fn code(&self) -> &'static str {
        match self {
            Self::Create => "WINDOW_ERROR",
            Self::ContentProtection => "CONTENT_PROTECTION_ERROR",
        }
    }

    fn message(&self) -> &'static str {
        match self {
            Self::Create => "Failed to create camera preview window",
            Self::ContentProtection => "Failed to update camera preview content protection",
        }
    }
}

fn respond_window_error(request_id: &str, error: CameraWindowError) {
    respond_error(request_id, error.code(), error.message());
}

fn respond_camera_cancelled(request_id: Option<&str>) {
    if let Some(request_id) = request_id {
        respond_error(
            request_id,
            "CAMERA_CANCELLED",
            "Camera preview was replaced",
        );
    }
}

impl CameraRuntime {
    fn new() -> Self {
        Self {
            lifecycle: Mutex::new(CameraLifecycle {
                generation: 0,
                starting: false,
                running: false,
                visible: false,
            }),
            capture: Mutex::new(None),
            transition: Mutex::new(()),
            position: Mutex::new(None),
            frame: Arc::new(Mutex::new(None)),
            config: Mutex::new(CameraConfig::default()),
            ownership: Mutex::new(None),
            next_token: AtomicU64::new(1),
        }
    }

    fn advance_generation(&self) -> u64 {
        let Ok(mut ownership) = self.ownership.lock() else {
            return 0;
        };
        let Ok(mut lifecycle) = self.lifecycle.lock() else {
            return 0;
        };
        lifecycle.generation = lifecycle.generation.wrapping_add(1);
        lifecycle.starting = false;
        lifecycle.running = false;
        lifecycle.visible = false;
        if let Some(ownership) = ownership.as_mut() {
            ownership.handoff.generation = lifecycle.generation;
            ownership.handoff.wanted = false;
        }
        lifecycle.generation
    }

    fn generation(&self) -> u64 {
        self.lifecycle
            .lock()
            .map(|lifecycle| lifecycle.generation)
            .unwrap_or(0)
    }

    fn is_generation(&self, generation: u64) -> bool {
        self.lifecycle
            .lock()
            .map(|lifecycle| lifecycle.generation == generation)
            .unwrap_or(false)
    }

    fn update_lifecycle(
        &self,
        generation: u64,
        running: Option<bool>,
        visible: Option<bool>,
    ) -> bool {
        let Ok(mut lifecycle) = self.lifecycle.lock() else {
            return false;
        };
        if lifecycle.generation != generation {
            return false;
        }
        if let Some(running) = running {
            lifecycle.running = running;
            lifecycle.starting = false;
        }
        if let Some(visible) = visible {
            lifecycle.visible = visible;
        }
        true
    }

    fn lifecycle(&self) -> CameraLifecycle {
        self.lifecycle
            .lock()
            .map(|lifecycle| *lifecycle)
            .unwrap_or(CameraLifecycle {
                generation: 0,
                starting: false,
                running: false,
                visible: false,
            })
    }

    fn mark_visible(&self, generation: u64) -> bool {
        let Ok(mut lifecycle) = self.lifecycle.lock() else {
            return false;
        };
        if lifecycle.generation != generation || !lifecycle.running {
            return false;
        }
        lifecycle.visible = true;
        true
    }

    fn mark_starting(&self, generation: u64) -> bool {
        let Ok(mut lifecycle) = self.lifecycle.lock() else {
            return false;
        };
        if lifecycle.generation != generation {
            return false;
        }
        lifecycle.starting = true;
        lifecycle.running = false;
        lifecycle.visible = false;
        true
    }

    fn frame_slot(&self) -> Arc<Mutex<Option<CameraFrame>>> {
        self.frame.clone()
    }

    fn frame(&self) -> Option<CameraFrame> {
        self.frame.lock().ok().and_then(|frame| frame.clone())
    }

    fn set_frame(&self, frame: Option<CameraFrame>) {
        if let Ok(mut current) = self.frame.lock() {
            *current = frame;
        }
    }

    fn remember_config(&self, config: &CameraConfig) {
        if let Ok(mut stored) = self.config.lock() {
            *stored = config.clone();
        }
    }

    fn config_snapshot(&self) -> CameraConfig {
        self.config
            .lock()
            .map(|config| config.clone())
            .unwrap_or_default()
    }

    fn has_capture(&self) -> bool {
        self.capture
            .lock()
            .map(|capture| capture.is_some())
            .unwrap_or(false)
    }

    fn recording_owned(&self) -> bool {
        self.ownership
            .lock()
            .map(|ownership| ownership.is_some())
            .unwrap_or(false)
    }

    fn stop_standalone_lifecycle(&self, generation: u64) -> bool {
        let Ok(ownership) = self.ownership.lock() else {
            return false;
        };
        if ownership.is_some() {
            return false;
        }
        let Ok(mut lifecycle) = self.lifecycle.lock() else {
            return false;
        };
        if lifecycle.generation != generation {
            return false;
        }
        lifecycle.starting = false;
        lifecycle.running = false;
        lifecycle.visible = false;
        true
    }

    fn recording_generation(&self) -> Option<u64> {
        self.ownership.lock().ok().and_then(|ownership| {
            ownership
                .as_ref()
                .map(|ownership| ownership.handoff.generation)
        })
    }

    fn recording_wanted(&self) -> Option<bool> {
        self.ownership
            .lock()
            .ok()
            .and_then(|ownership| ownership.as_ref().map(|ownership| ownership.handoff.wanted))
    }

    fn recording_requested(&self, generation: u64) -> bool {
        self.ownership
            .lock()
            .ok()
            .and_then(|ownership| {
                ownership.as_ref().map(|ownership| {
                    ownership.handoff.generation == generation && ownership.handoff.wanted
                })
            })
            .unwrap_or(false)
    }

    fn request_recording_preview(&self, generation: u64) -> bool {
        let Ok(mut ownership) = self.ownership.lock() else {
            return false;
        };
        let Some(ownership) = ownership.as_mut() else {
            return false;
        };
        if ownership.handoff.generation != generation {
            return false;
        }
        ownership.handoff.wanted = true;
        true
    }

    fn set_recording_wanted(&self, wanted: bool) {
        if let Ok(mut ownership) = self.ownership.lock()
            && let Some(ownership) = ownership.as_mut()
        {
            ownership.handoff.wanted = wanted;
        }
    }

    #[cfg(test)]
    fn handoff(&self) -> Option<PreviewHandoff> {
        self.ownership
            .lock()
            .ok()
            .and_then(|ownership| ownership.as_ref().map(|ownership| ownership.handoff))
    }

    fn release_ownership(&self, token: u64) -> Option<(PreviewHandoff, CameraLifecycle)> {
        let mut ownership = self.ownership.lock().ok()?;
        let current = ownership.as_ref()?;
        if current.token != token {
            return None;
        }
        let handoff = ownership.take()?.handoff;
        let mut lifecycle = self.lifecycle.lock().ok()?;
        if lifecycle.generation == handoff.generation {
            lifecycle.running = false;
        }
        Some((handoff, *lifecycle))
    }
}

impl PreviewHandoff {
    fn begin(lifecycle: CameraLifecycle) -> Self {
        Self {
            generation: lifecycle.generation,
            wanted: lifecycle.visible,
            synced: false,
        }
    }

    fn adopt_pending(&mut self, pending_generation: Option<u64>, lifecycle: CameraLifecycle) {
        if pending_generation == Some(self.generation)
            && (lifecycle.starting || lifecycle.running || lifecycle.visible)
        {
            self.wanted = true;
        }
    }

    fn should_restore(self, lifecycle: CameraLifecycle, capture_active: bool) -> bool {
        !self.synced && self.wanted && !capture_active && lifecycle.generation == self.generation
    }
}

impl RecordingPreview {
    pub(crate) fn acquire() -> Result<Self, String> {
        Self::acquire_runtime(shared_runtime())
    }

    fn acquire_runtime(runtime: Arc<CameraRuntime>) -> Result<Self, String> {
        let transition = runtime
            .transition
            .lock()
            .map_err(|_| "Camera preview state is unavailable".to_string())?;
        let session = {
            let ownership = runtime
                .ownership
                .lock()
                .map_err(|_| "Camera preview state is unavailable".to_string())?;
            if ownership.is_some() {
                return Err("Camera preview is already used by a recording".to_string());
            }
            take_capture(&runtime)
        };
        let pending_generation = session.as_ref().map(|session| session.generation);
        let token = {
            let mut ownership = runtime
                .ownership
                .lock()
                .map_err(|_| "Camera preview state is unavailable".to_string())?;
            let mut lifecycle = runtime
                .lifecycle
                .lock()
                .map_err(|_| "Camera preview state is unavailable".to_string())?;
            let token = runtime.next_token.fetch_add(1, Ordering::AcqRel);
            let mut handoff = PreviewHandoff::begin(*lifecycle);
            handoff.adopt_pending(pending_generation, *lifecycle);
            *ownership = Some(RecordingOwnership { token, handoff });
            lifecycle.starting = false;
            lifecycle.running = true;
            token
        };
        if let Some(session) = session {
            stop_capture_session(session);
        }
        drop(transition);
        Ok(Self {
            runtime,
            token,
            post_pending: Arc::new(AtomicBool::new(false)),
        })
    }

    pub(crate) fn publish(&self, pixels: Arc<[u8]>, width: i32, height: i32) {
        if pixels.is_empty() {
            return;
        }
        {
            let Ok(ownership) = self.runtime.ownership.lock() else {
                return;
            };
            let Some(ownership) = ownership.as_ref() else {
                return;
            };
            if ownership.token != self.token {
                return;
            }
            let generation = ownership.handoff.generation;
            let Ok(lifecycle) = self.runtime.lifecycle.lock() else {
                return;
            };
            if lifecycle.generation != generation || !lifecycle.visible {
                return;
            }
            self.runtime.set_frame(Some(CameraFrame {
                pixels,
                width,
                height,
            }));
        }
        if self
            .post_pending
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return;
        }
        let runtime = self.runtime.clone();
        let token = self.token;
        let pending = self.post_pending.clone();
        run_on_ui(move || {
            pending.store(false, Ordering::Release);
            post_recording_frame(&runtime, token);
        });
    }

    pub(crate) fn mark_started(&self) {
        if let Ok(mut ownership) = self.runtime.ownership.lock()
            && let Some(ownership) = ownership.as_mut()
            && ownership.token == self.token
        {
            ownership.handoff.synced = true;
        }
    }
}

fn post_recording_frame(runtime: &Arc<CameraRuntime>, token: u64) {
    let Ok(ownership) = runtime.ownership.lock() else {
        return;
    };
    let Some(ownership) = ownership.as_ref() else {
        return;
    };
    if ownership.token != token {
        return;
    }
    let generation = ownership.handoff.generation;
    let Ok(lifecycle) = runtime.lifecycle.lock() else {
        return;
    };
    if lifecycle.generation != generation || !lifecycle.visible {
        return;
    }
    let window = STATE.with(|state| {
        let state = state.borrow();
        match &state.runtime {
            Some(panel) if Arc::ptr_eq(panel, runtime) => state.window,
            _ => None,
        }
    });
    if let Some(window) = window {
        unsafe {
            let _ = PostMessageW(Some(window), WM_CAMERA_FRAME, WPARAM(0), LPARAM(0));
        }
    }
}

impl Drop for RecordingPreview {
    fn drop(&mut self) {
        let Ok(_transition) = self.runtime.transition.lock() else {
            return;
        };
        let Some((handoff, lifecycle)) = self.runtime.release_ownership(self.token) else {
            return;
        };
        let capture_active = self.runtime.has_capture();
        if !handoff.should_restore(lifecycle, capture_active) {
            return;
        }
        let runtime = self.runtime.clone();
        run_on_ui(move || restore_standalone_preview(&runtime, handoff.generation));
    }
}

struct CameraUiState {
    window: Option<HWND>,
    runtime: Option<Arc<CameraRuntime>>,
    flipped: bool,
    content_protected: bool,
}

thread_local! {
    static STATE: RefCell<CameraUiState> = const { RefCell::new(CameraUiState {
        window: None,
        runtime: None,
        flipped: false,
        content_protected: false,
    }) };
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
        WM_CAMERA_FRAME => {
            unsafe {
                let _ = InvalidateRect(Some(window), None, false);
            }
            LRESULT(0)
        }
        WM_NCHITTEST => LRESULT(HTCAPTION as isize),
        WM_EXITSIZEMOVE => {
            emit_position(window);
            LRESULT(0)
        }
        WM_DPICHANGED => {
            let suggested = unsafe { &*(lparam.0 as *const RECT) };
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
            update_region(window);
            emit_position(window);
            LRESULT(0)
        }
        _ => default_wndproc(window, message, wparam, lparam),
    }
}

fn paint(window: HWND) {
    let (runtime, flipped) = STATE.with(|state| {
        let state = state.borrow();
        (state.runtime.clone(), state.flipped)
    });
    let frame = runtime.and_then(|runtime| runtime.frame());

    unsafe {
        let mut paint = PAINTSTRUCT::default();
        let dc = BeginPaint(window, &mut paint);
        let mut bounds = RECT::default();
        let _ = GetClientRect(window, &mut bounds);
        let buffer_dc = CreateCompatibleDC(Some(dc));
        let buffer = CreateCompatibleBitmap(dc, bounds.right, bounds.bottom);
        let previous_bitmap = SelectObject(buffer_dc, buffer.into());
        let black = CreateSolidBrush(COLORREF(0));
        FillRect(buffer_dc, &bounds, black);
        let _ = DeleteObject(black.into());

        if let Some(frame) = frame {
            let dpi = GetDpiForWindow(window).max(96);
            let padding = scale_for_dpi(SHADOW_PADDING, dpi);
            let preview = scale_for_dpi(PREVIEW_SIZE, dpi);
            let crop = frame.width.min(frame.height);
            let source_x = (frame.width - crop) / 2;
            let source_y = (frame.height - crop) / 2;
            let (destination_x, destination_width) = if flipped {
                (padding + preview, -preview)
            } else {
                (padding, preview)
            };
            let info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: frame.width,
                    biHeight: -frame.height,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    biSizeImage: frame.pixels.len() as u32,
                    ..Default::default()
                },
                ..Default::default()
            };
            let _ = SetStretchBltMode(buffer_dc, HALFTONE);
            let _ = StretchDIBits(
                buffer_dc,
                destination_x,
                padding,
                destination_width,
                preview,
                source_x,
                source_y,
                crop,
                crop,
                Some(frame.pixels.as_ptr().cast()),
                &info,
                DIB_RGB_COLORS,
                SRCCOPY,
            );
        }
        let _ = BitBlt(
            dc,
            0,
            0,
            bounds.right,
            bounds.bottom,
            Some(buffer_dc),
            0,
            0,
            SRCCOPY,
        );
        SelectObject(buffer_dc, previous_bitmap);
        let _ = DeleteObject(buffer.into());
        let _ = DeleteDC(buffer_dc);
        let _ = EndPaint(window, &paint);
    }
}

fn update_region(window: HWND) {
    let dpi = unsafe { GetDpiForWindow(window) }.max(96);
    let padding = scale_for_dpi(SHADOW_PADDING, dpi);
    let preview = scale_for_dpi(PREVIEW_SIZE, dpi);
    let radius = scale_for_dpi(130, dpi);
    unsafe {
        let region = CreateRoundRectRgn(
            padding,
            padding,
            padding + preview + 1,
            padding + preview + 1,
            radius,
            radius,
        );
        let _ = SetWindowRgn(window, Some(region), true);
    }
}

fn emit_position(window: HWND) {
    let mut rect = RECT::default();
    if unsafe { GetWindowRect(window, &mut rect) }.is_err() {
        return;
    }
    STATE.with(|state| {
        if let Some(runtime) = &state.borrow().runtime
            && let Ok(mut position) = runtime.position.lock()
        {
            *position = Some((rect.left, rect.top));
        }
    });
    crate::protocol::send_event(
        "camera-preview:position-changed",
        Some(json!({ "x": rect.left, "y": rect.top })),
    );
}

fn set_content_protection(window: HWND, enabled: bool) -> bool {
    let affinity = if enabled {
        WDA_EXCLUDEFROMCAPTURE
    } else {
        WDA_NONE
    };
    unsafe { SetWindowDisplayAffinity(window, affinity).is_ok() }
}

fn default_position(size: i32) -> (i32, i32) {
    let monitor = monitors()
        .into_iter()
        .find(|monitor| monitor.is_primary)
        .or_else(|| monitors().into_iter().next());
    let Some(monitor) = monitor else {
        return (100, 100);
    };
    (
        monitor.rect.right - size - 32,
        monitor.rect.bottom - size - 32,
    )
}

fn create_or_update_window(
    runtime: Arc<CameraRuntime>,
    x: Option<i32>,
    y: Option<i32>,
    flipped: bool,
    content_protected: bool,
) -> Result<HWND, CameraWindowError> {
    if let Some(window) = STATE.with(|state| state.borrow().window) {
        if !set_content_protection(window, content_protected) {
            return Err(CameraWindowError::ContentProtection);
        }
        if let (Some(x), Some(y)) = (x, y) {
            unsafe {
                let _ = SetWindowPos(
                    window,
                    Some(HWND_TOPMOST),
                    x,
                    y,
                    0,
                    0,
                    SWP_NOACTIVATE | windows::Win32::UI::WindowsAndMessaging::SWP_NOSIZE,
                );
            }
            if let Ok(mut position) = runtime.position.lock() {
                *position = Some((x, y));
            }
        }
        STATE.with(|state| {
            let mut state = state.borrow_mut();
            state.runtime = Some(runtime);
            state.flipped = flipped;
            state.content_protected = content_protected;
        });
        return Ok(window);
    }

    ensure_window_class(CLASS_NAME, Some(wndproc), None);
    let rect = RECT {
        left: x.unwrap_or(100),
        top: y.unwrap_or(100),
        right: x.unwrap_or(100) + TOTAL_SIZE,
        bottom: y.unwrap_or(100) + TOTAL_SIZE,
    };
    let styles = WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED;
    let window = create_popup_window(CLASS_NAME, styles, &rect).ok_or(CameraWindowError::Create)?;
    let _ = disable_window_transitions(window);
    let dpi = unsafe { GetDpiForWindow(window) }.max(96);
    let size = scale_for_dpi(TOTAL_SIZE, dpi);
    let position = match (x, y) {
        (Some(x), Some(y)) => (x, y),
        _ => runtime
            .position
            .lock()
            .ok()
            .and_then(|position| *position)
            .unwrap_or_else(|| default_position(size)),
    };
    unsafe {
        let _ = SetWindowPos(
            window,
            Some(HWND_TOPMOST),
            position.0,
            position.1,
            size,
            size,
            SWP_NOACTIVATE,
        );
        let _ = SetLayeredWindowAttributes(window, COLORREF(0), 255, LWA_ALPHA);
    }
    if !set_content_protection(window, content_protected) {
        unsafe {
            let _ = DestroyWindow(window);
        }
        return Err(CameraWindowError::ContentProtection);
    }
    update_region(window);
    if let Ok(mut stored) = runtime.position.lock() {
        *stored = Some(position);
    }
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.runtime = Some(runtime);
        state.flipped = flipped;
        state.content_protected = content_protected;
        state.window = Some(window);
    });
    Ok(window)
}

fn show_window(window: HWND, runtime: &CameraRuntime, generation: u64) -> bool {
    if !runtime.mark_visible(generation) {
        return false;
    }
    unsafe {
        let _ = ShowWindow(window, SW_SHOWNOACTIVATE);
        let _ = SetWindowPos(
            window,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOACTIVATE | SWP_NOMOVE | windows::Win32::UI::WindowsAndMessaging::SWP_NOSIZE,
        );
    }
    true
}

fn hide_window_only(runtime: &CameraRuntime, generation: u64) {
    if !runtime.update_lifecycle(generation, None, Some(false)) {
        return;
    }
    if let Some(window) = STATE.with(|state| state.borrow().window) {
        unsafe {
            let _ = windows::Win32::UI::WindowsAndMessaging::ShowWindow(
                window,
                windows::Win32::UI::WindowsAndMessaging::SW_HIDE,
            );
        }
    }
}

fn hide_standalone_window(runtime: &CameraRuntime, generation: u64) {
    if runtime.recording_wanted() == Some(true) {
        return;
    }
    hide_window_only(runtime, generation);
}

fn destroy_preview_panel(runtime: &CameraRuntime) {
    let window = STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.runtime = None;
        state.window.take()
    });
    if let Some(window) = window {
        unsafe {
            let _ = DestroyWindow(window);
        }
    }
    runtime.set_frame(None);
}

fn hide_recording_panel(runtime: &CameraRuntime, generation: u64) -> bool {
    if !runtime.update_lifecycle(generation, None, Some(false)) {
        return false;
    }
    runtime.set_recording_wanted(false);
    destroy_preview_panel(runtime);
    true
}

fn teardown_window(runtime: &CameraRuntime, generation: u64) -> bool {
    if runtime.recording_owned() {
        if runtime.lifecycle().visible {
            return false;
        }
        return hide_recording_panel(runtime, generation);
    }
    if !runtime.update_lifecycle(generation, Some(false), Some(false)) {
        return false;
    }
    destroy_preview_panel(runtime);
    if let Ok(mut position) = runtime.position.lock() {
        *position = None;
    }
    true
}

fn restore_standalone_preview(runtime: &Arc<CameraRuntime>, generation: u64) {
    if runtime.recording_owned() || !runtime.is_generation(generation) || runtime.has_capture() {
        return;
    }
    let config = runtime.config_snapshot();
    let content_protected = STATE.with(|state| state.borrow().content_protected);
    let window = match create_or_update_window(
        runtime.clone(),
        None,
        None,
        config.flipped,
        content_protected,
    ) {
        Ok(window) => window,
        Err(_) => return,
    };
    start_capture(config, runtime.clone(), window, generation, None);
}

fn requested_size(resolution: &str) -> (u32, u32) {
    match resolution {
        "1080p" => (1920, 1080),
        "480p" => (640, 480),
        "4k" => (3840, 2160),
        _ => (1280, 720),
    }
}

fn sample_frame(sample: IMFSample, width: i32, height: i32) -> Result<CameraFrame, String> {
    let buffer =
        unsafe { sample.ConvertToContiguousBuffer() }.map_err(|error| error.to_string())?;
    let mut pointer = std::ptr::null_mut();
    let mut length = 0;
    unsafe { buffer.Lock(&mut pointer, None, Some(&mut length)) }
        .map_err(|error| error.to_string())?;
    if pointer.is_null() || length == 0 {
        let _ = unsafe { buffer.Unlock() };
        return Err("Camera returned an empty frame".to_string());
    }
    let pixels: Arc<[u8]> =
        Arc::from(unsafe { std::slice::from_raw_parts(pointer, length as usize) });
    let _ = unsafe { buffer.Unlock() };
    Ok(CameraFrame {
        pixels,
        width,
        height,
    })
}

fn capture_loop(
    config: CameraConfig,
    stop: Arc<AtomicBool>,
    interrupt: Arc<Mutex<Option<CaptureInterrupt>>>,
    frame: Arc<Mutex<Option<CameraFrame>>>,
    window_value: isize,
    ready: impl FnOnce(Result<(), String>),
) {
    if let Err(error) = retain_process_mta() {
        ready(Err(error.to_string()));
        return;
    }
    let initialized = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok();
    let started = unsafe { MFStartup(MF_VERSION, MFSTARTUP_FULL) };
    if let Err(error) = started {
        ready(Err(error.to_string()));
        if initialized {
            unsafe { CoUninitialize() };
        }
        return;
    }

    let result = (|| -> Result<(IMFMediaSource, IMFSourceReader, i32, i32), String> {
        let selected = select_camera(
            enumerate_cameras()?,
            config.device_id.as_deref(),
            config.device_name.as_deref(),
        )
        .map_err(|error| error.to_string())?;
        let activate = selected.activation;
        let source: IMFMediaSource =
            unsafe { activate.ActivateObject() }.map_err(|error| error.to_string())?;
        let mut attributes = None;
        unsafe { MFCreateAttributes(&mut attributes, 2) }.map_err(|error| error.to_string())?;
        let attributes =
            attributes.ok_or_else(|| "Source reader attributes unavailable".to_string())?;
        unsafe { attributes.SetUINT32(&MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, 1) }
            .map_err(|error| error.to_string())?;
        unsafe { attributes.SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 1) }
            .map_err(|error| error.to_string())?;
        let reader = unsafe { MFCreateSourceReaderFromMediaSource(&source, &attributes) }
            .map_err(|error| error.to_string())?;
        let media_type = unsafe { MFCreateMediaType() }.map_err(|error| error.to_string())?;
        let (width, height) = requested_size(&config.resolution);
        unsafe { media_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video) }
            .map_err(|error| error.to_string())?;
        unsafe { media_type.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32) }
            .map_err(|error| error.to_string())?;
        unsafe { media_type.SetUINT64(&MF_MT_FRAME_SIZE, ((width as u64) << 32) | height as u64) }
            .map_err(|error| error.to_string())?;
        unsafe {
            reader.SetCurrentMediaType(
                MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32,
                None,
                &media_type,
            )
        }
        .map_err(|error| error.to_string())?;
        let active_type =
            unsafe { reader.GetCurrentMediaType(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32) }
                .map_err(|error| error.to_string())?;
        let packed = unsafe { active_type.GetUINT64(&MF_MT_FRAME_SIZE) }
            .map_err(|error| error.to_string())?;
        Ok((source, reader, (packed >> 32) as i32, packed as u32 as i32))
    })();

    let (source, reader, width, height) = match result {
        Ok(value) => value,
        Err(error) => {
            ready(Err(error));
            let _ = unsafe { MFShutdown() };
            if initialized {
                unsafe { CoUninitialize() };
            }
            return;
        }
    };

    let Ok(mut active) = interrupt.lock() else {
        ready(Err("Camera interrupt state unavailable".to_string()));
        let _ = unsafe { source.Shutdown() };
        drop(reader);
        drop(source);
        let _ = unsafe { MFShutdown() };
        if initialized {
            unsafe { CoUninitialize() };
        }
        return;
    };
    *active = Some(CaptureInterrupt {
        reader: MtaInterface::new(reader.clone()),
        source: MtaInterface::new(source.clone()),
    });
    drop(active);
    if stop.load(Ordering::Acquire) {
        ready(Err("Camera preview was cancelled".to_string()));
        if let Ok(mut active) = interrupt.lock() {
            active.take();
        }
        let _ = unsafe { source.Shutdown() };
        drop(reader);
        drop(source);
        let _ = unsafe { MFShutdown() };
        if initialized {
            unsafe { CoUninitialize() };
        }
        return;
    }

    ready(Ok(()));
    let window = HWND(window_value as *mut core::ffi::c_void);
    while !stop.load(Ordering::Acquire) {
        let mut flags = 0u32;
        let mut sample = None;
        let read = unsafe {
            reader.ReadSample(
                MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32,
                0,
                None,
                Some(&mut flags),
                None,
                Some(&mut sample),
            )
        };
        if read.is_err() || flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
            break;
        }
        let Some(sample) = sample else {
            continue;
        };
        let Ok(next_frame) = sample_frame(sample, width, height) else {
            continue;
        };
        if let Ok(mut current) = frame.lock() {
            *current = Some(next_frame);
        }
        if unsafe { PostMessageW(Some(window), WM_CAMERA_FRAME, WPARAM(0), LPARAM(0)) }.is_err() {
            break;
        }
    }

    if let Ok(mut active) = interrupt.lock() {
        active.take();
    }
    let _ = unsafe { source.Shutdown() };
    drop(reader);
    drop(source);
    let _ = unsafe { MFShutdown() };
    if initialized {
        unsafe { CoUninitialize() };
    }
}

fn take_capture(runtime: &CameraRuntime) -> Option<CaptureSession> {
    runtime
        .capture
        .lock()
        .ok()
        .and_then(|mut capture| capture.take())
}

fn take_capture_generation(runtime: &CameraRuntime, generation: u64) -> Option<CaptureSession> {
    let Ok(mut capture) = runtime.capture.lock() else {
        return None;
    };
    if capture
        .as_ref()
        .map(|session| session.generation != generation)
        .unwrap_or(true)
    {
        return None;
    }
    capture.take()
}

fn stop_capture_session(session: CaptureSession) {
    session.stop.store(true, Ordering::Release);
    let interrupt = session
        .interrupt
        .lock()
        .ok()
        .and_then(|mut interrupt| interrupt.take());
    if let Some(interrupt) = interrupt {
        let interrupter = std::thread::spawn(move || {
            let retained = retain_process_mta().is_ok();
            let initialized =
                retained && unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok();
            if initialized {
                unsafe {
                    interrupt.reader.with(|reader| {
                        let _ = reader.Flush(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32);
                    });
                    interrupt.source.with(|source| {
                        let _ = source.Shutdown();
                    });
                }
                drop(interrupt);
                unsafe { CoUninitialize() };
            }
        });
        let _ = interrupter.join();
    }
    let _ = session.thread.join();
}

fn stop_capture_session_async(runtime: Arc<CameraRuntime>, session: CaptureSession) {
    std::thread::spawn(move || {
        let Ok(_transition) = runtime.transition.lock() else {
            stop_capture_session(session);
            return;
        };
        stop_capture_session(session);
    });
}

fn stop_capture_async(
    runtime: Arc<CameraRuntime>,
    generation: u64,
    completion: impl FnOnce(Arc<CameraRuntime>, CaptureTransitionOutcome) + Send + 'static,
) {
    std::thread::spawn(move || {
        let outcome = match runtime.transition.lock() {
            Ok(_transition) => {
                if !runtime.is_generation(generation) {
                    CaptureTransitionOutcome::Replaced
                } else {
                    if let Some(session) = take_capture(&runtime) {
                        stop_capture_session(session);
                    }
                    if runtime.is_generation(generation) {
                        CaptureTransitionOutcome::Current
                    } else {
                        CaptureTransitionOutcome::Replaced
                    }
                }
            }
            Err(_) => CaptureTransitionOutcome::Failed,
        };
        let ui_runtime = runtime.clone();
        run_on_ui(move || {
            let outcome = match outcome {
                CaptureTransitionOutcome::Current if !ui_runtime.is_generation(generation) => {
                    CaptureTransitionOutcome::Replaced
                }
                outcome => outcome,
            };
            completion(ui_runtime, outcome);
        });
    });
}

fn show_adopted_preview(runtime: &CameraRuntime, generation: u64, window: HWND) -> bool {
    if !runtime.recording_requested(generation) {
        return false;
    }
    if !runtime.update_lifecycle(generation, Some(true), None) {
        return false;
    }
    show_window(window, runtime, generation)
}

fn respond_adopted_preview(
    runtime: &CameraRuntime,
    generation: u64,
    window: HWND,
    request_id: &Option<String>,
) {
    if show_adopted_preview(runtime, generation, window) {
        if let Some(request_id) = request_id {
            respond_success(request_id, json!({ "visible": true }));
        }
        return;
    }
    respond_camera_cancelled(request_id.as_deref());
}

fn complete_adopted_preview(
    runtime: Arc<CameraRuntime>,
    generation: u64,
    window_value: isize,
    request_id: Option<String>,
) {
    run_on_ui(move || {
        let window = HWND(window_value as *mut core::ffi::c_void);
        respond_adopted_preview(&runtime, generation, window, &request_id);
    });
}

fn start_capture(
    config: CameraConfig,
    runtime: Arc<CameraRuntime>,
    window: HWND,
    generation: u64,
    request_id: Option<String>,
) {
    let Ok(_transition) = runtime.transition.lock() else {
        if let Some(request_id) = request_id {
            respond_error(
                &request_id,
                "CAMERA_ERROR",
                "Camera transition state unavailable",
            );
        }
        return;
    };
    if runtime.recording_owned() {
        if runtime.request_recording_preview(generation) {
            if !runtime.lifecycle().visible {
                runtime.set_frame(None);
            }
            respond_adopted_preview(&runtime, generation, window, &request_id);
            return;
        }
        respond_camera_cancelled(request_id.as_deref());
        return;
    }
    let preserve_frame = runtime.lifecycle().visible;
    if !runtime.update_lifecycle(generation, Some(false), Some(false)) {
        respond_camera_cancelled(request_id.as_deref());
        return;
    }
    if !preserve_frame {
        runtime.set_frame(None);
    }
    let frame = runtime.frame_slot();
    if !runtime.mark_starting(generation) {
        respond_camera_cancelled(request_id.as_deref());
        return;
    }
    let stop = Arc::new(AtomicBool::new(false));
    let interrupt = Arc::new(Mutex::new(None));
    let window_value = window.0 as isize;
    let thread_stop = stop.clone();
    let thread_interrupt = interrupt.clone();
    let thread_runtime = runtime.clone();
    let thread = std::thread::spawn(move || {
        let ready_runtime = thread_runtime.clone();
        let finished_runtime = thread_runtime.clone();
        let ready_stop = thread_stop.clone();
        capture_loop(
            config,
            thread_stop,
            thread_interrupt,
            frame,
            window_value,
            move |result| match result {
                Ok(()) => {
                    if ready_runtime.recording_owned() {
                        complete_adopted_preview(
                            ready_runtime.clone(),
                            generation,
                            window_value,
                            request_id,
                        );
                        return;
                    }
                    if ready_stop.load(Ordering::Acquire)
                        || ready_runtime.generation() != generation
                    {
                        if ready_runtime.recording_owned() {
                            complete_adopted_preview(
                                ready_runtime.clone(),
                                generation,
                                window_value,
                                request_id,
                            );
                            return;
                        }
                        respond_camera_cancelled(request_id.as_deref());
                        return;
                    }
                    if !ready_runtime.update_lifecycle(generation, Some(true), None) {
                        respond_camera_cancelled(request_id.as_deref());
                        return;
                    }
                    let runtime = ready_runtime.clone();
                    run_on_ui(move || {
                        let window = HWND(window_value as *mut core::ffi::c_void);
                        if runtime.recording_owned() {
                            respond_adopted_preview(&runtime, generation, window, &request_id);
                            return;
                        }
                        let lifecycle = runtime.lifecycle();
                        if lifecycle.generation != generation || !lifecycle.running {
                            if let Some(request_id) = request_id {
                                respond_error(
                                    &request_id,
                                    "CAMERA_ERROR",
                                    "Camera stopped before preview became visible",
                                );
                            }
                            return;
                        }
                        if show_window(window, &runtime, generation) {
                            if let Some(request_id) = request_id.as_deref() {
                                respond_success(request_id, json!({ "visible": true }));
                            }
                        } else {
                            respond_camera_cancelled(request_id.as_deref());
                        }
                    });
                }
                Err(error) => {
                    if ready_runtime.recording_owned() {
                        complete_adopted_preview(
                            ready_runtime.clone(),
                            generation,
                            window_value,
                            request_id,
                        );
                        return;
                    }
                    let current = ready_runtime.stop_standalone_lifecycle(generation);
                    if let Some(request_id) = request_id {
                        respond_error(&request_id, "CAMERA_ERROR", &error);
                    }
                    if current {
                        let runtime = ready_runtime.clone();
                        run_on_ui(move || hide_standalone_window(&runtime, generation));
                    }
                }
            },
        );
        if finished_runtime.stop_standalone_lifecycle(generation) {
            let runtime = finished_runtime.clone();
            run_on_ui(move || hide_standalone_window(&runtime, generation));
        }
    });
    let session = CaptureSession {
        generation,
        stop,
        interrupt,
        thread,
    };
    let stored = match runtime.capture.lock() {
        Ok(mut capture) => {
            let replaced = capture.replace(session);
            drop(capture);
            if let Some(replaced) = replaced {
                stop_capture_session_async(runtime.clone(), replaced);
            }
            true
        }
        Err(_) => {
            session.stop.store(true, Ordering::Release);
            stop_capture_session_async(runtime.clone(), session);
            false
        }
    };
    if stored
        && !runtime.is_generation(generation)
        && let Some(session) = take_capture_generation(&runtime, generation)
    {
        stop_capture_session_async(runtime.clone(), session);
    }
}

fn show_preview_request(
    runtime: Arc<CameraRuntime>,
    config: CameraConfig,
    changed: bool,
    x: Option<i32>,
    y: Option<i32>,
    request_id: String,
) {
    if runtime.recording_owned() {
        show_recording_panel(runtime, config, x, y, request_id);
        return;
    }
    let lifecycle = runtime.lifecycle();
    let restart = changed || !lifecycle.running;
    let generation = if restart {
        runtime.advance_generation()
    } else {
        lifecycle.generation
    };
    if !runtime.is_generation(generation) {
        respond_camera_cancelled(Some(&request_id));
        return;
    }
    if !restart {
        let content_protected = STATE.with(|state| state.borrow().content_protected);
        let window =
            match create_or_update_window(runtime.clone(), x, y, config.flipped, content_protected)
            {
                Ok(window) => window,
                Err(error) => {
                    respond_window_error(&request_id, error);
                    return;
                }
            };
        if show_window(window, &runtime, generation) {
            respond_success(&request_id, json!({ "visible": true }));
        } else {
            respond_camera_cancelled(Some(&request_id));
        }
        return;
    }
    hide_standalone_window(&runtime, generation);
    stop_capture_async(runtime, generation, move |runtime, outcome| match outcome {
        CaptureTransitionOutcome::Replaced => respond_camera_cancelled(Some(&request_id)),
        CaptureTransitionOutcome::Failed => respond_error(
            &request_id,
            "CAMERA_ERROR",
            "Camera transition state unavailable",
        ),
        CaptureTransitionOutcome::Current => {
            let content_protected = STATE.with(|state| state.borrow().content_protected);
            let window = match create_or_update_window(
                runtime.clone(),
                x,
                y,
                config.flipped,
                content_protected,
            ) {
                Ok(window) => window,
                Err(error) => {
                    let _ = teardown_window(&runtime, generation);
                    respond_window_error(&request_id, error);
                    return;
                }
            };
            start_capture(config, runtime, window, generation, Some(request_id));
        }
    });
}

fn show_recording_panel(
    runtime: Arc<CameraRuntime>,
    config: CameraConfig,
    x: Option<i32>,
    y: Option<i32>,
    request_id: String,
) {
    let Some(generation) = runtime.recording_generation() else {
        show_preview_request(runtime, config, false, x, y, request_id);
        return;
    };
    runtime.set_recording_wanted(true);
    if !runtime.update_lifecycle(generation, Some(true), None) {
        respond_camera_cancelled(Some(&request_id));
        return;
    }
    let content_protected = STATE.with(|state| state.borrow().content_protected);
    let window =
        match create_or_update_window(runtime.clone(), x, y, config.flipped, content_protected) {
            Ok(window) => window,
            Err(error) => {
                respond_window_error(&request_id, error);
                return;
            }
        };
    if show_window(window, &runtime, generation) {
        respond_success(&request_id, json!({ "visible": true }));
    } else {
        respond_camera_cancelled(Some(&request_id));
    }
}

fn hide_preview_request(runtime: Arc<CameraRuntime>, request_id: String) {
    let generation = runtime.advance_generation();
    if runtime.recording_owned() {
        if hide_recording_panel(&runtime, generation) {
            respond_success(&request_id, json!({ "visible": false }));
        } else {
            respond_camera_cancelled(Some(&request_id));
        }
        return;
    }
    if !runtime.is_generation(generation) {
        respond_camera_cancelled(Some(&request_id));
        return;
    }
    hide_window_only(&runtime, generation);
    stop_capture_async(runtime, generation, move |runtime, outcome| match outcome {
        CaptureTransitionOutcome::Replaced => respond_camera_cancelled(Some(&request_id)),
        CaptureTransitionOutcome::Failed => respond_error(
            &request_id,
            "CAMERA_ERROR",
            "Camera transition state unavailable",
        ),
        CaptureTransitionOutcome::Current => {
            if teardown_window(&runtime, generation) {
                respond_success(&request_id, json!({ "visible": false }));
            } else {
                respond_camera_cancelled(Some(&request_id));
            }
        }
    });
}

fn update_preview_request(
    runtime: Arc<CameraRuntime>,
    config: CameraConfig,
    changed: bool,
    x: Option<i32>,
    y: Option<i32>,
) {
    if runtime.recording_owned() {
        if STATE.with(|state| state.borrow().window.is_none()) {
            return;
        }
        let content_protected = STATE.with(|state| state.borrow().content_protected);
        let _ = create_or_update_window(runtime.clone(), x, y, config.flipped, content_protected);
        return;
    }
    let lifecycle = runtime.lifecycle();
    let generation = if changed {
        runtime.advance_generation()
    } else {
        lifecycle.generation
    };
    if !runtime.is_generation(generation) {
        return;
    }
    if STATE.with(|state| state.borrow().window.is_none()) {
        return;
    }
    if changed {
        hide_standalone_window(&runtime, generation);
        stop_capture_async(runtime, generation, move |runtime, outcome| {
            if !matches!(outcome, CaptureTransitionOutcome::Current) {
                return;
            }
            if STATE.with(|state| state.borrow().window.is_none()) {
                return;
            }
            let content_protected = STATE.with(|state| state.borrow().content_protected);
            let window = match create_or_update_window(
                runtime.clone(),
                x,
                y,
                config.flipped,
                content_protected,
            ) {
                Ok(window) => window,
                Err(_) => {
                    let _ = teardown_window(&runtime, generation);
                    return;
                }
            };
            start_capture(config, runtime, window, generation, None);
        });
        return;
    }
    let content_protected = STATE.with(|state| state.borrow().content_protected);
    let _ = create_or_update_window(runtime.clone(), x, y, config.flipped, content_protected);
}

pub struct CameraPreviewModule {
    config: CameraConfig,
    runtime: Arc<CameraRuntime>,
}

impl CameraPreviewModule {
    pub fn new() -> Self {
        Self {
            config: CameraConfig::default(),
            runtime: shared_runtime(),
        }
    }
}

impl Module for CameraPreviewModule {
    fn name(&self) -> &'static str {
        CAMERA_PREVIEW_MODULE
    }

    fn handle(&mut self, request: &Request) -> Reply {
        match CameraPreviewMethod::parse(&request.method) {
            Some(CameraPreviewMethod::Show) => {
                let request_params: CameraPreviewRequest = match params(request) {
                    Ok(params) => params,
                    Err(error) => return Reply::Now(Err(error)),
                };
                let previous = self.config.clone();
                self.config.device_id = request_params.device_id;
                self.config.device_name = request_params.device_name;
                self.config.resolution = request_params.resolution.unwrap_or_else(|| "720p".into());
                self.config.flipped = request_params.flipped.unwrap_or(false);
                let changed = previous.device_id != self.config.device_id
                    || previous.device_name != self.config.device_name
                    || previous.resolution != self.config.resolution;
                let config = self.config.clone();
                self.runtime.remember_config(&config);
                let x = request_params.x;
                let y = request_params.y;
                let request_id = request.id.clone();
                let runtime = self.runtime.clone();
                run_on_ui(move || {
                    show_preview_request(runtime, config, changed, x, y, request_id);
                });
                Reply::Deferred
            }
            Some(CameraPreviewMethod::Hide) => {
                let request_id = request.id.clone();
                let runtime = self.runtime.clone();
                run_on_ui(move || hide_preview_request(runtime, request_id));
                Reply::Deferred
            }
            Some(CameraPreviewMethod::Update) => {
                let request_params: CameraPreviewUpdateRequest = match params(request) {
                    Ok(params) => params,
                    Err(error) => return Reply::Now(Err(error)),
                };
                let mut changed = false;
                if let UpdateField::Value(value) = request_params.device_id {
                    changed |= value != self.config.device_id;
                    self.config.device_id = value;
                }
                if let UpdateField::Value(value) = request_params.device_name {
                    changed |= value != self.config.device_name;
                    self.config.device_name = value;
                }
                if let Some(resolution) = request_params.resolution {
                    changed |= resolution != self.config.resolution;
                    self.config.resolution = resolution;
                }
                if let Some(flipped) = request_params.flipped {
                    self.config.flipped = flipped;
                }
                let x = request_params.x;
                let y = request_params.y;
                let config = self.config.clone();
                self.runtime.remember_config(&config);
                let runtime = self.runtime.clone();
                run_on_ui(move || {
                    update_preview_request(runtime, config, changed, x, y);
                });
                Reply::Now(Ok(Some(json!({ "updated": true }))))
            }
            Some(CameraPreviewMethod::SetContentProtection) => {
                let content_protection: ContentProtectionRequest = match params(request) {
                    Ok(params) => params,
                    Err(error) => return Reply::Now(Err(error)),
                };
                self.config.content_protected = content_protection.enabled;
                let enabled = self.config.content_protected;
                let request_id = request.id.clone();
                run_on_ui(move || {
                    if let Some(window) = STATE.with(|state| state.borrow().window)
                        && !set_content_protection(window, enabled)
                    {
                        respond_error(
                            &request_id,
                            "CONTENT_PROTECTION_ERROR",
                            "Failed to update camera preview content protection",
                        );
                        return;
                    }
                    STATE.with(|state| state.borrow_mut().content_protected = enabled);
                    respond_success(&request_id, json!({ "protected": enabled }));
                });
                Reply::Deferred
            }
            None => method_not_found(&request.method),
        }
    }
}

#[cfg(test)]
mod handoff_tests {
    use super::*;

    fn live_runtime() -> (Arc<CameraRuntime>, u64) {
        let runtime = Arc::new(CameraRuntime::new());
        let generation = runtime.advance_generation();
        runtime.update_lifecycle(generation, Some(true), Some(true));
        runtime.set_frame(Some(CameraFrame {
            pixels: Arc::from(vec![9u8; 16]),
            width: 2,
            height: 2,
        }));
        (runtime, generation)
    }

    #[test]
    fn recording_handoff_keeps_preview_and_retains_frame() {
        let (runtime, generation) = live_runtime();
        let preview = RecordingPreview::acquire_runtime(runtime.clone()).expect("handoff");

        let handoff = runtime.handoff().expect("ownership");
        assert_eq!(handoff.generation, generation);
        assert!(handoff.should_restore(runtime.lifecycle(), false));
        assert!(runtime.recording_owned());
        let lifecycle = runtime.lifecycle();
        assert!(lifecycle.running);
        assert!(lifecycle.visible);

        let retained = runtime.frame().expect("retained frame");
        assert_eq!((retained.width, retained.height), (2, 2));
        assert_eq!(retained.pixels.len(), 16);

        preview.mark_started();
        assert!(
            !runtime
                .handoff()
                .expect("ownership")
                .should_restore(runtime.lifecycle(), false)
        );
        drop(preview);

        assert!(!runtime.recording_owned());
        let lifecycle = runtime.lifecycle();
        assert!(!lifecycle.running);
        assert!(lifecycle.visible);
        assert!(runtime.frame().is_some());
    }

    #[test]
    fn recording_handoff_restore_uses_wanted_not_visible() {
        let (runtime, generation) = live_runtime();
        let preview = RecordingPreview::acquire_runtime(runtime.clone()).expect("handoff");

        let handoff = runtime.handoff().expect("ownership");
        assert!(handoff.should_restore(runtime.lifecycle(), false));

        runtime.set_recording_wanted(false);
        let hidden = runtime.handoff().expect("ownership");
        assert!(!hidden.should_restore(runtime.lifecycle(), false));

        runtime.set_recording_wanted(true);
        runtime.update_lifecycle(generation, None, Some(false));
        let pending = runtime.handoff().expect("ownership");
        assert!(pending.should_restore(runtime.lifecycle(), false));

        runtime.update_lifecycle(generation, None, Some(true));
        assert!(runtime.frame().is_some());
        assert!(
            !PreviewHandoff {
                generation: generation.wrapping_add(1),
                wanted: true,
                synced: false,
            }
            .should_restore(runtime.lifecycle(), false)
        );

        preview.mark_started();
        assert!(
            !runtime
                .handoff()
                .expect("ownership")
                .should_restore(runtime.lifecycle(), false)
        );
        drop(preview);
        assert!(!runtime.recording_owned());
    }

    fn pending_session(generation: u64) -> CaptureSession {
        CaptureSession {
            generation,
            stop: Arc::new(AtomicBool::new(false)),
            interrupt: Arc::new(Mutex::new(None)),
            thread: std::thread::spawn(|| {}),
        }
    }

    #[test]
    fn recording_handoff_adopts_only_a_matching_pending_generation() {
        let runtime = Arc::new(CameraRuntime::new());
        let generation = runtime.advance_generation();
        assert!(runtime.mark_starting(generation));
        *runtime.capture.lock().expect("capture") = Some(pending_session(generation));
        let preview = RecordingPreview::acquire_runtime(runtime.clone()).expect("handoff");
        let handoff = runtime.handoff().expect("ownership");
        assert_eq!(handoff.generation, generation);
        assert!(handoff.wanted);
        assert!(!runtime.lifecycle().visible);
        assert!(handoff.should_restore(runtime.lifecycle(), false));
        preview.mark_started();
        drop(preview);

        let runtime = Arc::new(CameraRuntime::new());
        let generation = runtime.advance_generation();
        assert!(runtime.mark_starting(generation));
        *runtime.capture.lock().expect("capture") =
            Some(pending_session(generation.wrapping_add(1)));
        let preview = RecordingPreview::acquire_runtime(runtime.clone()).expect("handoff");
        assert!(!runtime.handoff().expect("ownership").wanted);
        preview.mark_started();
        drop(preview);
    }

    #[test]
    fn stopped_standalone_session_is_not_adopted() {
        let runtime = Arc::new(CameraRuntime::new());
        let generation = runtime.advance_generation();
        *runtime.capture.lock().expect("capture") = Some(pending_session(generation));
        let preview = RecordingPreview::acquire_runtime(runtime.clone()).expect("handoff");
        let handoff = runtime.handoff().expect("ownership");
        assert_eq!(handoff.generation, generation);
        assert!(!handoff.wanted);
        assert!(!handoff.should_restore(runtime.lifecycle(), false));
        preview.mark_started();
        drop(preview);
        assert!(!runtime.recording_owned());
    }

    #[test]
    fn recording_hide_releases_cached_frame_and_keeps_position() {
        let (runtime, visible_generation) = live_runtime();
        let preview = RecordingPreview::acquire_runtime(runtime.clone()).expect("handoff");
        if let Ok(mut position) = runtime.position.lock() {
            *position = Some((11, 22));
        }

        let hidden_generation = runtime.advance_generation();
        assert!(hide_recording_panel(&runtime, hidden_generation));
        assert!(!runtime.mark_visible(visible_generation));
        assert!(runtime.frame().is_none());
        assert!(!runtime.lifecycle().visible);
        assert!(runtime.recording_owned());

        preview.publish(Arc::from(vec![1u8; 16]), 2, 2);
        assert!(runtime.frame().is_none());

        let position = *runtime.position.lock().expect("position");
        assert_eq!(position, Some((11, 22)));

        preview.mark_started();
        drop(preview);
    }

    #[test]
    fn generation_advance_clears_old_restore_intent() {
        let (runtime, generation) = live_runtime();
        let preview = RecordingPreview::acquire_runtime(runtime.clone()).expect("handoff");
        assert!(runtime.handoff().expect("ownership").wanted);

        let advanced = runtime.advance_generation();
        let handoff = runtime.handoff().expect("ownership");
        assert_eq!(handoff.generation, advanced);
        assert_ne!(advanced, generation);
        assert!(!handoff.wanted);
        assert!(!handoff.should_restore(runtime.lifecycle(), false));
        assert!(
            PreviewHandoff {
                generation: advanced,
                wanted: true,
                synced: false,
            }
            .should_restore(runtime.lifecycle(), false)
        );
        assert!(!runtime.mark_visible(generation));

        preview.mark_started();
        drop(preview);
        assert!(!runtime.recording_owned());
    }

    #[test]
    fn handoff_rejects_a_stale_standalone_stop_and_hide() {
        let (runtime, generation) = live_runtime();
        let preview = RecordingPreview::acquire_runtime(runtime.clone()).expect("handoff");

        assert!(!runtime.stop_standalone_lifecycle(generation));
        hide_standalone_window(&runtime, generation);
        let lifecycle = runtime.lifecycle();
        assert!(lifecycle.running);
        assert!(lifecycle.visible);

        preview.mark_started();
        drop(preview);
        assert!(!runtime.lifecycle().running);
        assert!(runtime.lifecycle().visible);
    }

    #[test]
    fn recording_handoff_rejects_a_second_owner() {
        let (runtime, _) = live_runtime();
        let preview = RecordingPreview::acquire_runtime(runtime.clone()).expect("handoff");
        assert!(RecordingPreview::acquire_runtime(runtime.clone()).is_err());
        preview.mark_started();
        drop(preview);
        assert!(!runtime.recording_owned());
    }
}
