use super::camera_devices::{enumerate_cameras, select_camera};
use crate::com::{MtaInterface, retain_process_mta};
use crate::overlay::{
    create_popup_window, default_wndproc, disable_window_transitions, ensure_window_class, monitors,
};
use crate::protocol::{Request, param_bool, param_i32, param_str, respond_error, respond_success};
use crate::router::{Module, Reply, method_not_found};
use crate::ui::run_on_ui;
use serde_json::json;
use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
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
    pixels: Vec<u8>,
    width: i32,
    height: i32,
}

struct CameraRuntime {
    lifecycle: Mutex<CameraLifecycle>,
    capture: Mutex<Option<CaptureSession>>,
    transition: Mutex<()>,
    position: Mutex<Option<(i32, i32)>>,
}

#[derive(Clone, Copy)]
struct CameraLifecycle {
    generation: u64,
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

impl CameraRuntime {
    fn new() -> Self {
        Self {
            lifecycle: Mutex::new(CameraLifecycle {
                generation: 0,
                running: false,
                visible: false,
            }),
            capture: Mutex::new(None),
            transition: Mutex::new(()),
            position: Mutex::new(None),
        }
    }

    fn advance_generation(&self) -> u64 {
        let Ok(mut lifecycle) = self.lifecycle.lock() else {
            return 0;
        };
        lifecycle.generation = lifecycle.generation.wrapping_add(1);
        lifecycle.running = false;
        lifecycle.visible = false;
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
}

struct CameraUiState {
    window: Option<HWND>,
    frame: Option<Arc<Mutex<Option<CameraFrame>>>>,
    runtime: Option<Arc<CameraRuntime>>,
    flipped: bool,
    content_protected: bool,
}

thread_local! {
    static STATE: RefCell<CameraUiState> = const { RefCell::new(CameraUiState {
        window: None,
        frame: None,
        runtime: None,
        flipped: false,
        content_protected: false,
    }) };
}

fn scale(value: i32, dpi: u32) -> i32 {
    ((value as i64 * dpi.max(96) as i64) / 96) as i32
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
    let (frame, flipped) = STATE.with(|state| {
        let state = state.borrow();
        (state.frame.clone(), state.flipped)
    });
    let frame = frame.and_then(|frame| frame.lock().ok().and_then(|frame| frame.clone()));

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
            let padding = scale(SHADOW_PADDING, dpi);
            let preview = scale(PREVIEW_SIZE, dpi);
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
    let padding = scale(SHADOW_PADDING, dpi);
    let preview = scale(PREVIEW_SIZE, dpi);
    let radius = scale(130, dpi);
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
        if let Some(runtime) = &state.borrow().runtime {
            if let Ok(mut position) = runtime.position.lock() {
                *position = Some((rect.left, rect.top));
            }
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
    frame: Arc<Mutex<Option<CameraFrame>>>,
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
            state.frame = Some(frame);
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
    let size = scale(TOTAL_SIZE, dpi);
    let position = match (x, y) {
        (Some(x), Some(y)) => (x, y),
        _ => default_position(size),
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
        state.frame = Some(frame);
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

fn teardown_window(runtime: &CameraRuntime, generation: u64) -> bool {
    if !runtime.update_lifecycle(generation, Some(false), Some(false)) {
        return false;
    }
    let window = STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.frame = None;
        state.runtime = None;
        state.window.take()
    });
    if let Some(window) = window {
        unsafe {
            let _ = DestroyWindow(window);
        }
    }
    if let Ok(mut position) = runtime.position.lock() {
        *position = None;
    }
    true
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
    let pixels = unsafe { std::slice::from_raw_parts(pointer, length as usize) }.to_vec();
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

fn start_capture(
    config: CameraConfig,
    runtime: Arc<CameraRuntime>,
    window: HWND,
    generation: u64,
    request_id: Option<String>,
) {
    if !runtime.update_lifecycle(generation, Some(false), Some(false)) {
        if let Some(request_id) = request_id {
            respond_error(
                &request_id,
                "CAMERA_CANCELLED",
                "Camera preview was replaced",
            );
        }
        return;
    }
    let frame = STATE.with(|state| state.borrow().frame.clone());
    let Some(frame) = frame else {
        if let Some(request_id) = request_id {
            respond_error(
                &request_id,
                "CAMERA_ERROR",
                "Camera frame state unavailable",
            );
        }
        return;
    };
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
                    if ready_stop.load(Ordering::Acquire)
                        || ready_runtime.generation() != generation
                    {
                        if let Some(request_id) = request_id {
                            respond_error(
                                &request_id,
                                "CAMERA_CANCELLED",
                                "Camera preview was replaced",
                            );
                        }
                        return;
                    }
                    if !ready_runtime.update_lifecycle(generation, Some(true), None) {
                        if let Some(request_id) = request_id {
                            respond_error(
                                &request_id,
                                "CAMERA_CANCELLED",
                                "Camera preview was replaced",
                            );
                        }
                        return;
                    }
                    let runtime = ready_runtime.clone();
                    run_on_ui(move || {
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
                        let window = HWND(window_value as *mut core::ffi::c_void);
                        if show_window(window, &runtime, generation) {
                            if let Some(request_id) = request_id {
                                respond_success(&request_id, json!({ "visible": true }));
                            }
                        } else if let Some(request_id) = request_id {
                            respond_error(
                                &request_id,
                                "CAMERA_CANCELLED",
                                "Camera preview was replaced",
                            );
                        }
                    });
                }
                Err(error) => {
                    let current =
                        ready_runtime.update_lifecycle(generation, Some(false), Some(false));
                    if let Some(request_id) = request_id {
                        respond_error(&request_id, "CAMERA_ERROR", &error);
                    }
                    if current {
                        let runtime = ready_runtime.clone();
                        run_on_ui(move || hide_window_only(&runtime, generation));
                    }
                }
            },
        );
        if finished_runtime.update_lifecycle(generation, Some(false), Some(false)) {
            let runtime = finished_runtime.clone();
            run_on_ui(move || hide_window_only(&runtime, generation));
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
    if stored && !runtime.is_generation(generation) {
        if let Some(session) = take_capture_generation(&runtime, generation) {
            stop_capture_session_async(runtime.clone(), session);
        }
    }
}

pub struct CameraPreviewModule {
    config: CameraConfig,
    runtime: Arc<CameraRuntime>,
}

impl CameraPreviewModule {
    pub fn new() -> Self {
        Self {
            config: CameraConfig::default(),
            runtime: Arc::new(CameraRuntime::new()),
        }
    }
}

impl Module for CameraPreviewModule {
    fn name(&self) -> &'static str {
        "camera-preview"
    }

    fn handle(&mut self, request: &Request) -> Reply {
        match request.method.as_str() {
            "show" => {
                let previous = self.config.clone();
                self.config.device_id = param_str(&request.params, "deviceId").map(str::to_string);
                self.config.device_name =
                    param_str(&request.params, "deviceName").map(str::to_string);
                self.config.resolution = param_str(&request.params, "resolution")
                    .unwrap_or("720p")
                    .to_string();
                self.config.flipped = param_bool(&request.params, "flipped").unwrap_or(false);
                let changed = previous.device_id != self.config.device_id
                    || previous.device_name != self.config.device_name
                    || previous.resolution != self.config.resolution;
                let x = param_i32(&request.params, "x");
                let y = param_i32(&request.params, "y");
                let request_id = request.id.clone();
                let config = self.config.clone();
                let runtime = self.runtime.clone();
                let lifecycle = runtime.lifecycle();
                let restart = changed || !lifecycle.running;
                let generation = if restart {
                    runtime.advance_generation()
                } else {
                    lifecycle.generation
                };
                run_on_ui(move || {
                    if !runtime.is_generation(generation) {
                        respond_error(
                            &request_id,
                            "CAMERA_CANCELLED",
                            "Camera preview was replaced",
                        );
                        return;
                    }
                    if !restart {
                        let frame = STATE
                            .with(|state| state.borrow().frame.clone())
                            .unwrap_or_else(|| Arc::new(Mutex::new(None)));
                        let content_protected =
                            STATE.with(|state| state.borrow().content_protected);
                        let window = match create_or_update_window(
                            runtime.clone(),
                            frame,
                            x,
                            y,
                            config.flipped,
                            content_protected,
                        ) {
                            Ok(window) => window,
                            Err(error) => {
                                respond_window_error(&request_id, error);
                                return;
                            }
                        };
                        if show_window(window, &runtime, generation) {
                            respond_success(&request_id, json!({ "visible": true }));
                        } else {
                            respond_error(
                                &request_id,
                                "CAMERA_CANCELLED",
                                "Camera preview was replaced",
                            );
                        }
                        return;
                    }
                    hide_window_only(&runtime, generation);
                    stop_capture_async(
                        runtime,
                        generation,
                        move |runtime, outcome| match outcome {
                            CaptureTransitionOutcome::Replaced => {
                                respond_error(
                                    &request_id,
                                    "CAMERA_CANCELLED",
                                    "Camera preview was replaced",
                                );
                            }
                            CaptureTransitionOutcome::Failed => {
                                respond_error(
                                    &request_id,
                                    "CAMERA_ERROR",
                                    "Camera transition state unavailable",
                                );
                            }
                            CaptureTransitionOutcome::Current => {
                                let frame = Arc::new(Mutex::new(None));
                                let content_protected =
                                    STATE.with(|state| state.borrow().content_protected);
                                let window = match create_or_update_window(
                                    runtime.clone(),
                                    frame,
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
                                start_capture(
                                    config,
                                    runtime,
                                    window,
                                    generation,
                                    Some(request_id),
                                );
                            }
                        },
                    );
                });
                Reply::Deferred
            }
            "hide" => {
                let generation = self.runtime.advance_generation();
                let request_id = request.id.clone();
                let runtime = self.runtime.clone();
                run_on_ui(move || {
                    if !runtime.is_generation(generation) {
                        respond_error(
                            &request_id,
                            "CAMERA_CANCELLED",
                            "Camera preview was replaced",
                        );
                        return;
                    }
                    hide_window_only(&runtime, generation);
                    stop_capture_async(
                        runtime,
                        generation,
                        move |runtime, outcome| match outcome {
                            CaptureTransitionOutcome::Replaced => respond_error(
                                &request_id,
                                "CAMERA_CANCELLED",
                                "Camera preview was replaced",
                            ),
                            CaptureTransitionOutcome::Failed => respond_error(
                                &request_id,
                                "CAMERA_ERROR",
                                "Camera transition state unavailable",
                            ),
                            CaptureTransitionOutcome::Current => {
                                if teardown_window(&runtime, generation) {
                                    respond_success(&request_id, json!({ "visible": false }));
                                } else {
                                    respond_error(
                                        &request_id,
                                        "CAMERA_CANCELLED",
                                        "Camera preview was replaced",
                                    );
                                }
                            }
                        },
                    );
                });
                Reply::Deferred
            }
            "update" => {
                let mut changed = false;
                if let Some(device_id) = request
                    .params
                    .as_ref()
                    .and_then(|params| params.get("deviceId"))
                {
                    let value = device_id.as_str().map(str::to_string);
                    changed |= value != self.config.device_id;
                    self.config.device_id = value;
                }
                if let Some(device_name) = request
                    .params
                    .as_ref()
                    .and_then(|params| params.get("deviceName"))
                {
                    let value = device_name.as_str().map(str::to_string);
                    changed |= value != self.config.device_name;
                    self.config.device_name = value;
                }
                if let Some(resolution) = param_str(&request.params, "resolution") {
                    changed |= resolution != self.config.resolution;
                    self.config.resolution = resolution.to_string();
                }
                if let Some(flipped) = param_bool(&request.params, "flipped") {
                    self.config.flipped = flipped;
                }
                let x = param_i32(&request.params, "x");
                let y = param_i32(&request.params, "y");
                let config = self.config.clone();
                let runtime = self.runtime.clone();
                let generation = if changed {
                    runtime.advance_generation()
                } else {
                    runtime.generation()
                };
                run_on_ui(move || {
                    if !runtime.is_generation(generation) {
                        return;
                    }
                    if STATE.with(|state| state.borrow().window.is_none()) {
                        return;
                    }
                    if changed {
                        hide_window_only(&runtime, generation);
                        stop_capture_async(runtime, generation, move |runtime, outcome| {
                            if !matches!(outcome, CaptureTransitionOutcome::Current) {
                                return;
                            }
                            if STATE.with(|state| state.borrow().window.is_none()) {
                                return;
                            }
                            let frame = Arc::new(Mutex::new(None));
                            let content_protected =
                                STATE.with(|state| state.borrow().content_protected);
                            let window = match create_or_update_window(
                                runtime.clone(),
                                frame,
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
                    let Some(frame) = STATE.with(|state| state.borrow().frame.clone()) else {
                        return;
                    };
                    let content_protected = STATE.with(|state| state.borrow().content_protected);
                    let Ok(_) = create_or_update_window(
                        runtime.clone(),
                        frame,
                        x,
                        y,
                        config.flipped,
                        content_protected,
                    ) else {
                        return;
                    };
                });
                Reply::Now(Ok(Some(json!({ "updated": true }))))
            }
            "setContentProtection" => {
                self.config.content_protected =
                    param_bool(&request.params, "enabled").unwrap_or(false);
                let enabled = self.config.content_protected;
                let request_id = request.id.clone();
                run_on_ui(move || {
                    if let Some(window) = STATE.with(|state| state.borrow().window) {
                        if !set_content_protection(window, enabled) {
                            respond_error(
                                &request_id,
                                "CONTENT_PROTECTION_ERROR",
                                "Failed to update camera preview content protection",
                            );
                            return;
                        }
                    }
                    STATE.with(|state| state.borrow_mut().content_protected = enabled);
                    respond_success(&request_id, json!({ "protected": enabled }));
                });
                Reply::Deferred
            }
            method => method_not_found(method),
        }
    }
}
