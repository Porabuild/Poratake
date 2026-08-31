//! Port of `src/main/daemon/index.ts` — spawns the platform daemon
//! (`poratake-daemon`) and speaks the same newline-delimited JSON-RPC
//! protocol over stdin/stdout, with timeouts, event fan-out and restart
//! backoff.

use std::collections::HashMap;
use std::io::{BufRead as _, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::{Arc, Weak};
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, Result};
use parking_lot::Mutex;
#[cfg(target_os = "linux")]
use poratake_daemon_common::contract::ScreenshotLinuxMethod;
use poratake_daemon_common::contract::{
    CameraPreviewMethod, CameraPreviewRequest, ContentProtectionRequest, DesktopHelperMethod,
    DesktopWallpaperMethod, DesktopWallpaperResult, FreezeScreenMethod, IosDeviceList, MediaDevice,
    MediaDeviceKind, MediaDeviceListRequest, MediaDeviceLists, MediaDevicesMethod,
    MicrophoneTestRequest, OcrMethod, OcrRecognizeRequest, OcrRecognizeResult, PrintImageRequest,
    PrintMethod, QrCodeMethod, RecordingControlMethod, RecordingOverlayMethod,
    RecordingOverlayShowWindowRequest, RecordingOverlayVisibilityResult, ScreenRecorderMethod,
    ScreenRecorderMicrophoneRequest, ScreenRecorderStartRequest, ScreenRecorderToggleRequest,
    ScreenshotMethod, ScrollCaptureFinishRequest, ScrollCaptureFinishResult, ScrollCaptureMethod,
    ScrollCaptureStartRequest, TimerControlMethod, TimerShowRequest, WindowSelectorMethod,
    CAMERA_PREVIEW_MODULE, DESKTOP_HELPER_MODULE, DESKTOP_WALLPAPER_MODULE, FREEZE_SCREEN_MODULE,
    MEDIA_DEVICES_MODULE, OCR_MODULE, PRINT_MODULE, QRCODE_MODULE, RECORDING_CONTROL_MODULE,
    RECORDING_OVERLAY_MODULE, SCREENSHOT_MODULE, SCREEN_RECORDER_MODULE, SCROLL_CAPTURE_MODULE,
    TIMER_CONTROL_MODULE, WINDOW_SELECTOR_MODULE,
};
#[cfg(target_os = "macos")]
use poratake_daemon_common::geometry::CaptureRect;
#[cfg(target_os = "linux")]
use poratake_daemon_common::geometry::DisplayInfo;
use poratake_daemon_common::geometry::WindowInfo;
use poratake_daemon_common::geometry::{CaptureAreaRequest, CaptureWindowRequest};
use poratake_daemon_common::qrcode::{QrDetectRequest, QrDetectResult};
use serde_json::{json, Value};

const REQUEST_TIMEOUT_MS: u64 = 30_000;
const MAX_RESTART_ATTEMPTS: u32 = 5;
const RESTART_BACKOFF_BASE_MS: u64 = 1_000;

pub type EventHandler = Arc<dyn Fn(&str, &Value) + Send + Sync>;

struct Pending {
    sender: SyncSender<std::result::Result<Value, String>>,
}

#[derive(Clone)]
pub struct DaemonHandle {
    inner: Arc<DaemonInner>,
}

struct DaemonInner {
    lifecycle: Mutex<()>,
    child: Mutex<Option<Child>>,
    stdin: Mutex<Option<std::process::ChildStdin>>,
    pending: Mutex<HashMap<String, Pending>>,
    next_id: AtomicU64,
    shutting_down: AtomicBool,
    restart_attempts: AtomicU32,
    event_handlers: Mutex<HashMap<u64, EventHandler>>,
    next_event_handler: AtomicU64,
    binary_path: PathBuf,
}

pub struct EventSubscription {
    inner: Weak<DaemonInner>,
    id: u64,
}

impl Drop for EventSubscription {
    fn drop(&mut self) {
        if let Some(inner) = self.inner.upgrade() {
            inner.event_handlers.lock().remove(&self.id);
        }
    }
}

fn find_daemon_binary() -> PathBuf {
    if let Ok(path) = std::env::var("PORATAKE_DAEMON_PATH") {
        let path = PathBuf::from(path);
        if path.exists() {
            return path;
        }
    }

    const BINARY: &str = if cfg!(windows) {
        "poratake-daemon.exe"
    } else if cfg!(target_os = "linux") {
        "poratake-daemon-linux"
    } else {
        "poratake-daemon"
    };

    // Packaged layout: <root>/daemon/poratake-daemon(.exe) next to the app.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let sibling = dir.join(BINARY);
            if sibling.exists() {
                return sibling;
            }
            let packaged = dir.join("daemon").join(BINARY);
            if packaged.exists() {
                return packaged;
            }
            for ancestors in dir.ancestors().skip(1) {
                let candidate = ancestors
                    .join("src")
                    .join("main")
                    .join("daemon")
                    .join(BINARY);
                if candidate.exists() {
                    return candidate;
                }
            }
        }
    }

    let dev = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("app-gpui manifest dir has a parent (src/main)")
        .join("daemon")
        .join(BINARY);
    if dev.exists() {
        return dev;
    }

    PathBuf::from(format!("src/main/daemon/{BINARY}"))
}

impl DaemonHandle {
    pub fn new() -> Self {
        Self::with_binary(find_daemon_binary())
    }

    pub fn with_binary(binary_path: PathBuf) -> Self {
        Self {
            inner: Arc::new(DaemonInner {
                lifecycle: Mutex::new(()),
                child: Mutex::new(None),
                stdin: Mutex::new(None),
                pending: Mutex::new(HashMap::new()),
                next_id: AtomicU64::new(1),
                shutting_down: AtomicBool::new(false),
                restart_attempts: AtomicU32::new(0),
                event_handlers: Mutex::new(HashMap::new()),
                next_event_handler: AtomicU64::new(1),
                binary_path,
            }),
        }
    }

    pub fn on_event(&self, handler: EventHandler) {
        self.insert_event_handler(handler);
    }

    pub fn subscribe(&self, handler: EventHandler) -> EventSubscription {
        EventSubscription {
            inner: Arc::downgrade(&self.inner),
            id: self.insert_event_handler(handler),
        }
    }

    fn insert_event_handler(&self, handler: EventHandler) -> u64 {
        let id = self
            .inner
            .next_event_handler
            .fetch_add(1, Ordering::Relaxed);
        self.inner.event_handlers.lock().insert(id, handler);
        id
    }

    pub fn is_running(&self) -> bool {
        self.inner.child.lock().is_some()
    }

    fn ensure_running(&self) -> Result<()> {
        if !self.is_running() {
            self.start()?;
        }
        Ok(())
    }

    /// Starts the daemon and waits for its `system:ready` event.
    pub fn start(&self) -> Result<()> {
        let _lifecycle = self.inner.lifecycle.lock();
        if self.inner.child.lock().is_some() {
            return Ok(());
        }

        self.inner.shutting_down.store(false, Ordering::SeqCst);

        let mut command = Command::new(&self.inner.binary_path);
        #[cfg(target_os = "linux")]
        {
            command
                .arg("--session")
                .arg(crate::system::linux_session::current().id());
            // The shell resolves FFmpeg with bundle-aware fallbacks the
            // standalone daemon cannot see; hand the answer over so both
            // processes agree on one binary.
            let ffmpeg = crate::video::ffmpeg_path();
            if ffmpeg.is_file() {
                command.env("PORATAKE_FFMPEG_PATH", &ffmpeg);
            }
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|error| anyhow!("failed to spawn daemon: {error}"))?;

        let stdin = child.stdin.take().expect("daemon stdin");
        let stdout = child.stdout.take().expect("daemon stdout");

        *self.inner.child.lock() = Some(child);
        *self.inner.stdin.lock() = Some(stdin);

        let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel::<()>(8);
        let ready_subscription = self.subscribe(Arc::new(move |event, _| {
            if event == "system:ready" {
                ready_tx.send(()).ok();
            }
        }));

        self.spawn_reader(stdout);
        self.watch_child_by_pid(Self::pid_of_current(&self.inner));

        let ready = ready_rx.recv_timeout(Duration::from_secs(10));
        drop(ready_subscription);
        match ready {
            Ok(()) => {
                self.inner.restart_attempts.store(0, Ordering::SeqCst);
                Ok(())
            }
            Err(_) => {
                let mut child = self.inner.child.lock().take();
                *self.inner.stdin.lock() = None;
                if let Some(child) = child.as_mut() {
                    let _ = child.kill();
                    let _ = child.wait();
                }
                self.inner.reject_all_pending("Daemon ready timeout");
                Err(anyhow!("Daemon ready timeout"))
            }
        }
    }

    fn pid_of_current(inner: &Arc<DaemonInner>) -> Option<u32> {
        inner.child.lock().as_ref().map(|child| child.id())
    }

    fn spawn_reader(&self, stdout: std::process::ChildStdout) {
        let inner = self.inner.clone();
        thread::Builder::new()
            .name("daemon-reader".into())
            .spawn(move || {
                let reader = BufReader::new(stdout);
                for line in reader.lines() {
                    match line {
                        Ok(line) if !line.trim().is_empty() => {
                            inner.handle_line(&line);
                        }
                        Ok(_) => {}
                        Err(_) => break,
                    }
                }
            })
            .ok();
    }

    fn watch_child_by_pid(&self, pid: Option<u32>) {
        let Some(pid) = pid else {
            return;
        };
        let inner = self.inner.clone();
        thread::Builder::new()
            .name("daemon-watchdog".into())
            .spawn(move || {
                // Poll for exit; std has no async wait without blocking a
                // dedicated thread, which is exactly what this is.
                loop {
                    thread::sleep(Duration::from_millis(250));
                    let status = {
                        let mut guard = inner.child.lock();
                        match guard.as_mut() {
                            Some(child) if child.id() == pid => {
                                Some(matches!(child.try_wait(), Ok(Some(_))))
                            }
                            _ => None,
                        }
                    };
                    let Some(exited) = status else {
                        break;
                    };
                    if exited {
                        inner.handle_exit();
                        break;
                    }
                }
            })
            .ok();
    }

    pub fn stop(&self) {
        let _lifecycle = self.inner.lifecycle.lock();
        self.inner.shutting_down.store(true, Ordering::SeqCst);
        self.send_raw(&json!({
            "id": "quit",
            "module": "system",
            "method": "quit"
        }));
        thread::sleep(Duration::from_millis(500));

        let mut guard = self.inner.child.lock();
        if let Some(child) = guard.as_mut() {
            let _ = child.kill();
        }
        *guard = None;
        *self.inner.stdin.lock() = None;
        self.inner.reject_all_pending("Daemon stopped");
    }

    /// Calls `module.method`, returning the parsed `result`.
    fn call(&self, module: &str, method: &str, params: Option<Value>) -> Result<Value> {
        self.call_with_timeout(
            module,
            method,
            params,
            Duration::from_millis(REQUEST_TIMEOUT_MS),
        )
    }

    pub fn screenshot(&self) -> ScreenshotClient<'_> {
        ScreenshotClient { daemon: self }
    }

    pub fn freeze_screen(&self) -> FreezeScreenClient<'_> {
        FreezeScreenClient { daemon: self }
    }

    pub fn qrcode(&self) -> QrCodeClient<'_> {
        QrCodeClient { daemon: self }
    }

    pub fn ocr(&self) -> OcrClient<'_> {
        OcrClient { daemon: self }
    }

    pub fn print(&self) -> PrintClient<'_> {
        PrintClient { daemon: self }
    }

    pub fn camera_preview(&self) -> CameraPreviewClient<'_> {
        CameraPreviewClient { daemon: self }
    }

    pub fn desktop_helper(&self) -> DesktopHelperClient<'_> {
        DesktopHelperClient { daemon: self }
    }

    pub fn desktop_wallpaper(&self) -> DesktopWallpaperClient<'_> {
        DesktopWallpaperClient { daemon: self }
    }

    pub fn media_devices(&self) -> MediaDevicesClient<'_> {
        MediaDevicesClient { daemon: self }
    }

    pub fn recording_control(&self) -> RecordingControlClient<'_> {
        RecordingControlClient { daemon: self }
    }

    pub fn recording_overlay(&self) -> RecordingOverlayClient<'_> {
        RecordingOverlayClient { daemon: self }
    }

    pub fn screen_recorder(&self) -> ScreenRecorderClient<'_> {
        ScreenRecorderClient { daemon: self }
    }

    pub fn scroll_capture(&self) -> ScrollCaptureClient<'_> {
        ScrollCaptureClient { daemon: self }
    }

    pub fn timer_control(&self) -> TimerControlClient<'_> {
        TimerControlClient { daemon: self }
    }

    pub fn window_selector(&self) -> WindowSelectorClient<'_> {
        WindowSelectorClient { daemon: self }
    }

    fn call_with_timeout(
        &self,
        module: &str,
        method: &str,
        params: Option<Value>,
        timeout: Duration,
    ) -> Result<Value> {
        let (id, rx) = self.begin_call(module, method, params)?;

        match rx.recv_timeout(timeout) {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(message)) => Err(anyhow!("{message}")),
            Err(RecvTimeoutError::Timeout) => {
                self.inner.pending.lock().remove(&id);
                Err(anyhow!("Request timeout: {module}.{method}"))
            }
            Err(RecvTimeoutError::Disconnected) => {
                Err(anyhow!("Request dropped: {module}.{method}"))
            }
        }
    }

    #[cfg(target_os = "linux")]
    fn call_without_timeout(
        &self,
        module: &str,
        method: &str,
        params: Option<Value>,
    ) -> Result<Value> {
        let (_, rx) = self.begin_call(module, method, params)?;
        match rx.recv() {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(message)) => Err(anyhow!("{message}")),
            Err(_) => Err(anyhow!("Request dropped: {module}.{method}")),
        }
    }

    fn begin_call(
        &self,
        module: &str,
        method: &str,
        params: Option<Value>,
    ) -> Result<(String, Receiver<std::result::Result<Value, String>>)> {
        let id = format!("req-{}", self.inner.next_id.fetch_add(1, Ordering::SeqCst));
        let request = json!({
            "id": id,
            "module": module,
            "method": method,
            "params": params.unwrap_or(Value::Null),
        });

        let (tx, rx) = std::sync::mpsc::sync_channel::<std::result::Result<Value, String>>(1);
        self.inner
            .pending
            .lock()
            .insert(id.clone(), Pending { sender: tx });

        if let Err(error) = self.send_raw_value(&request) {
            self.inner.pending.lock().remove(&id);
            return Err(anyhow!("Daemon stdin write failed: {error}"));
        }

        Ok((id, rx))
    }

    fn send_raw_value(&self, request: &Value) -> Result<()> {
        let mut line = serde_json::to_string(request)?;
        line.push('\n');
        let mut guard = self.inner.stdin.lock();
        match guard.as_mut() {
            Some(stdin) => stdin.write_all(line.as_bytes()).map_err(Into::into),
            None => Err(anyhow!("Daemon stdin not writable")),
        }
    }

    fn send_raw(&self, request: &Value) -> bool {
        self.send_raw_value(request).is_ok()
    }
}

pub struct QrCodeClient<'a> {
    daemon: &'a DaemonHandle,
}

pub struct OcrClient<'a> {
    daemon: &'a DaemonHandle,
}

impl OcrClient<'_> {
    pub fn recognize(&self, image_path: &Path) -> Result<String> {
        self.daemon.ensure_running()?;
        let request = OcrRecognizeRequest {
            image_path: image_path.to_path_buf(),
        };
        let response = self.daemon.call(
            OCR_MODULE,
            OcrMethod::Recognize.id(),
            Some(serde_json::to_value(request)?),
        )?;
        let result: OcrRecognizeResult = serde_json::from_value(response)?;
        Ok(result.text)
    }
}

pub struct PrintClient<'a> {
    daemon: &'a DaemonHandle,
}

impl PrintClient<'_> {
    pub fn image(&self, image_base64: String) -> Result<()> {
        self.daemon.ensure_running()?;
        let params = serde_json::to_value(PrintImageRequest { image_base64 })?;
        self.daemon
            .call(PRINT_MODULE, PrintMethod::Image.id(), Some(params))
            .map(|_| ())
    }
}

pub struct CameraPreviewClient<'a> {
    daemon: &'a DaemonHandle,
}

impl CameraPreviewClient<'_> {
    pub fn show(&self, request: &CameraPreviewRequest) -> Result<()> {
        self.daemon.ensure_running()?;
        self.call(
            CameraPreviewMethod::Show,
            Some(serde_json::to_value(request)?),
        )
    }

    pub fn hide(&self) -> Result<()> {
        self.daemon.ensure_running()?;
        self.call(CameraPreviewMethod::Hide, None)
    }

    pub fn set_content_protection(&self, enabled: bool) -> Result<()> {
        self.daemon.ensure_running()?;
        self.call(
            CameraPreviewMethod::SetContentProtection,
            Some(serde_json::to_value(ContentProtectionRequest { enabled })?),
        )
    }

    fn call(&self, method: CameraPreviewMethod, params: Option<Value>) -> Result<()> {
        self.daemon
            .call(CAMERA_PREVIEW_MODULE, method.id(), params)
            .map(|_| ())
    }
}

pub struct DesktopHelperClient<'a> {
    daemon: &'a DaemonHandle,
}

impl DesktopHelperClient<'_> {
    pub fn hide(&self) -> Result<()> {
        self.call(DesktopHelperMethod::Hide)
    }

    pub fn show(&self) -> Result<()> {
        self.call(DesktopHelperMethod::Show)
    }

    fn call(&self, method: DesktopHelperMethod) -> Result<()> {
        self.daemon.ensure_running()?;
        self.daemon
            .call(DESKTOP_HELPER_MODULE, method.id(), None)
            .map(|_| ())
    }
}

pub struct DesktopWallpaperClient<'a> {
    daemon: &'a DaemonHandle,
}

impl DesktopWallpaperClient<'_> {
    pub fn get(&self) -> Result<DesktopWallpaperResult> {
        self.daemon.ensure_running()?;
        let response = self.daemon.call(
            DESKTOP_WALLPAPER_MODULE,
            DesktopWallpaperMethod::Get.id(),
            None,
        )?;
        serde_json::from_value(response).map_err(Into::into)
    }
}

pub struct MediaDevicesClient<'a> {
    daemon: &'a DaemonHandle,
}

impl MediaDevicesClient<'_> {
    pub fn list(&self, kinds: &[MediaDeviceKind]) -> Result<MediaDeviceLists> {
        self.daemon.ensure_running()?;
        let params = if kinds.is_empty() {
            None
        } else {
            Some(serde_json::to_value(MediaDeviceListRequest {
                kinds: kinds.to_vec(),
            })?)
        };
        let response =
            self.daemon
                .call(MEDIA_DEVICES_MODULE, MediaDevicesMethod::List.id(), params)?;
        serde_json::from_value(response).map_err(Into::into)
    }

    pub fn start_mic_test(&self, request: &MicrophoneTestRequest) -> Result<()> {
        self.daemon.ensure_running()?;
        self.daemon
            .call(
                MEDIA_DEVICES_MODULE,
                MediaDevicesMethod::StartMicTest.id(),
                Some(serde_json::to_value(request)?),
            )
            .map(|_| ())
    }

    pub fn stop_mic_test(&self) -> Result<()> {
        self.daemon.ensure_running()?;
        self.daemon
            .call(
                MEDIA_DEVICES_MODULE,
                MediaDevicesMethod::StopMicTest.id(),
                None,
            )
            .map(|_| ())
    }
}

pub struct RecordingControlClient<'a> {
    daemon: &'a DaemonHandle,
}

pub struct RecordingOverlayClient<'a> {
    daemon: &'a DaemonHandle,
}

impl RecordingOverlayClient<'_> {
    #[cfg(target_os = "macos")]
    pub fn show(&self, rect: CaptureRect) -> Result<bool> {
        let response = self.daemon.call(
            RECORDING_OVERLAY_MODULE,
            RecordingOverlayMethod::Show.id(),
            Some(serde_json::to_value(rect)?),
        )?;
        let result: RecordingOverlayVisibilityResult = serde_json::from_value(response)?;
        Ok(result.visible)
    }

    pub fn show_window(&self, request: RecordingOverlayShowWindowRequest) -> Result<bool> {
        let response = self.daemon.call(
            RECORDING_OVERLAY_MODULE,
            RecordingOverlayMethod::ShowWindow.id(),
            Some(serde_json::to_value(request)?),
        )?;
        let result: RecordingOverlayVisibilityResult = serde_json::from_value(response)?;
        Ok(result.visible)
    }

    pub fn hide(&self) -> Result<()> {
        if !self.daemon.is_running() {
            return Ok(());
        }
        self.daemon
            .call(
                RECORDING_OVERLAY_MODULE,
                RecordingOverlayMethod::Hide.id(),
                None,
            )
            .map(|_| ())
    }
}

pub struct ScreenRecorderClient<'a> {
    daemon: &'a DaemonHandle,
}

impl ScreenRecorderClient<'_> {
    pub fn start(&self, request: &ScreenRecorderStartRequest) -> Result<()> {
        request.validate().map_err(anyhow::Error::msg)?;
        self.daemon.ensure_running()?;
        self.daemon
            .call_with_timeout(
                SCREEN_RECORDER_MODULE,
                ScreenRecorderMethod::Start.id(),
                Some(serde_json::to_value(request)?),
                Duration::from_secs(60),
            )
            .map(|_| ())
    }

    pub fn pause(&self) -> Result<()> {
        self.call(ScreenRecorderMethod::Pause, None)
    }

    pub fn resume(&self) -> Result<()> {
        self.call(ScreenRecorderMethod::Resume, None)
    }

    pub fn stop(&self) -> Result<()> {
        self.daemon
            .call_with_timeout(
                SCREEN_RECORDER_MODULE,
                ScreenRecorderMethod::Stop.id(),
                None,
                Duration::from_secs(60),
            )
            .map(|_| ())
    }

    pub fn set_microphone(&self, request: ScreenRecorderMicrophoneRequest) -> Result<()> {
        self.call(
            ScreenRecorderMethod::SetMicrophone,
            Some(serde_json::to_value(request)?),
        )
    }

    pub fn set_system_audio(&self, enabled: bool) -> Result<()> {
        self.toggle(ScreenRecorderMethod::SetSystemAudio, enabled)
    }

    pub fn set_camera(&self, enabled: bool) -> Result<()> {
        self.toggle(ScreenRecorderMethod::SetCamera, enabled)
    }

    fn toggle(&self, method: ScreenRecorderMethod, enabled: bool) -> Result<()> {
        self.call(
            method,
            Some(serde_json::to_value(ScreenRecorderToggleRequest {
                enabled,
            })?),
        )
    }

    fn call(&self, method: ScreenRecorderMethod, params: Option<Value>) -> Result<()> {
        self.daemon
            .call(SCREEN_RECORDER_MODULE, method.id(), params)
            .map(|_| ())
    }
}

pub struct ScrollCaptureClient<'a> {
    daemon: &'a DaemonHandle,
}

impl ScrollCaptureClient<'_> {
    pub fn start(&self, request: &ScrollCaptureStartRequest) -> Result<()> {
        self.daemon.ensure_running()?;
        self.daemon
            .call(
                SCROLL_CAPTURE_MODULE,
                ScrollCaptureMethod::Start.id(),
                Some(serde_json::to_value(request)?),
            )
            .map(|_| ())
    }

    pub fn finish(
        &self,
        request: &ScrollCaptureFinishRequest,
    ) -> Result<ScrollCaptureFinishResult> {
        self.daemon.ensure_running()?;
        let response = self.daemon.call(
            SCROLL_CAPTURE_MODULE,
            ScrollCaptureMethod::Finish.id(),
            Some(serde_json::to_value(request)?),
        )?;
        serde_json::from_value(response).map_err(Into::into)
    }

    pub fn cancel(&self) -> Result<()> {
        if !self.daemon.is_running() {
            return Ok(());
        }
        self.daemon
            .call(
                SCROLL_CAPTURE_MODULE,
                ScrollCaptureMethod::Cancel.id(),
                None,
            )
            .map(|_| ())
    }
}

impl RecordingControlClient<'_> {
    pub fn list_ios_devices(&self) -> Result<Vec<MediaDevice>> {
        self.daemon.ensure_running()?;
        let response = self.daemon.call(
            RECORDING_CONTROL_MODULE,
            RecordingControlMethod::ListIosDevices.id(),
            None,
        )?;
        let response: IosDeviceList = serde_json::from_value(response)?;
        Ok(response.devices)
    }
}

pub struct FreezeScreenClient<'a> {
    daemon: &'a DaemonHandle,
}

impl FreezeScreenClient<'_> {
    pub fn freeze(&self) -> Result<()> {
        self.daemon.ensure_running()?;
        self.daemon
            .call(
                FREEZE_SCREEN_MODULE,
                FreezeScreenMethod::Freeze.id(),
                Some(json!({})),
            )
            .map(|_| ())
    }

    pub fn release(&self) -> Result<()> {
        if !self.daemon.is_running() {
            return Ok(());
        }
        self.daemon
            .call(
                FREEZE_SCREEN_MODULE,
                FreezeScreenMethod::Release.id(),
                Some(json!({})),
            )
            .map(|_| ())
    }

    pub fn prewarm(&self) -> Result<()> {
        if !self.daemon.is_running() {
            return Ok(());
        }
        self.daemon
            .call(
                FREEZE_SCREEN_MODULE,
                FreezeScreenMethod::Prewarm.id(),
                Some(json!({})),
            )
            .map(|_| ())
    }
}

pub struct TimerControlClient<'a> {
    daemon: &'a DaemonHandle,
}

impl TimerControlClient<'_> {
    pub fn show(&self, request: &TimerShowRequest) -> Result<()> {
        self.daemon.ensure_running()?;
        let params = serde_json::to_value(request)?;
        self.daemon
            .call(
                TIMER_CONTROL_MODULE,
                TimerControlMethod::Show.id(),
                Some(params),
            )
            .map(|_| ())
    }

    pub fn hide(&self) -> Result<()> {
        if !self.daemon.is_running() {
            return Ok(());
        }
        self.daemon
            .call(TIMER_CONTROL_MODULE, TimerControlMethod::Hide.id(), None)
            .map(|_| ())
    }
}

impl QrCodeClient<'_> {
    pub fn detect(&self, image_path: &Path) -> Result<String> {
        self.daemon.ensure_running()?;
        let params = serde_json::to_value(QrDetectRequest {
            image_path: image_path.to_path_buf(),
        })?;
        let response = self
            .daemon
            .call(QRCODE_MODULE, QrCodeMethod::Detect.id(), Some(params))?;
        let result: QrDetectResult = serde_json::from_value(response)?;
        Ok(result.payload)
    }
}

pub struct WindowSelectorClient<'a> {
    daemon: &'a DaemonHandle,
}

impl WindowSelectorClient<'_> {
    pub fn list(&self) -> Result<Vec<WindowInfo>> {
        self.daemon.ensure_running()?;
        let response = self.daemon.call(
            WINDOW_SELECTOR_MODULE,
            WindowSelectorMethod::List.id(),
            None,
        )?;
        serde_json::from_value(response.get("windows").cloned().unwrap_or_default())
            .map_err(Into::into)
    }
}

pub struct ScreenshotClient<'a> {
    daemon: &'a DaemonHandle,
}

impl ScreenshotClient<'_> {
    pub fn capture_area(&self, request: &CaptureAreaRequest) -> Result<()> {
        self.daemon.ensure_running()?;
        let params = serde_json::to_value(request)?;
        #[cfg(target_os = "linux")]
        let result = if crate::system::linux_session::current()
            == crate::system::linux_session::LinuxSession::Wayland
        {
            self.daemon.call_without_timeout(
                SCREENSHOT_MODULE,
                ScreenshotMethod::CaptureArea.id(),
                Some(params),
            )
        } else {
            self.daemon.call(
                SCREENSHOT_MODULE,
                ScreenshotMethod::CaptureArea.id(),
                Some(params),
            )
        };
        #[cfg(not(target_os = "linux"))]
        let result = self.daemon.call(
            SCREENSHOT_MODULE,
            ScreenshotMethod::CaptureArea.id(),
            Some(params),
        );
        result.map_err(|error| anyhow!("capture-area failed: {error}"))?;
        Ok(())
    }

    pub fn capture_window(&self, request: &CaptureWindowRequest) -> Result<()> {
        self.daemon.ensure_running()?;
        let params = serde_json::to_value(request)?;
        self.daemon
            .call(
                SCREENSHOT_MODULE,
                ScreenshotMethod::CaptureWindow.id(),
                Some(params),
            )
            .map_err(|error| anyhow!("capture-window failed: {error}"))?;
        Ok(())
    }

    #[cfg(target_os = "linux")]
    pub fn list_displays(&self) -> Result<Vec<DisplayInfo>> {
        self.daemon.ensure_running()?;
        let response = self.daemon.call(
            SCREENSHOT_MODULE,
            ScreenshotLinuxMethod::ListDisplays.id(),
            None,
        )?;
        serde_json::from_value(response.get("displays").cloned().unwrap_or_default())
            .map_err(Into::into)
    }
}

impl DaemonInner {
    fn dispatch_event(&self, event: &str, data: &Value) {
        let handlers: Vec<_> = self.event_handlers.lock().values().cloned().collect();
        for handler in handlers {
            handler(event, data);
        }
    }

    fn handle_line(self: &Arc<Self>, line: &str) {
        let message: Value = match serde_json::from_str(line) {
            Ok(value) => value,
            Err(_) => {
                eprintln!("[daemon] Failed to parse: {line}");
                return;
            }
        };

        if let Some(event) = message.get("event").and_then(Value::as_str) {
            let data = message.get("data").cloned().unwrap_or(Value::Null);
            self.dispatch_event(event, &data);
            return;
        }

        let id = match message.get("id").and_then(Value::as_str) {
            Some(id) => id.to_string(),
            None => return,
        };

        let pending = self.pending.lock().remove(&id);
        if let Some(pending) = pending {
            let outcome = if message.get("success").and_then(Value::as_bool) == Some(true) {
                Ok(message.get("result").cloned().unwrap_or(Value::Null))
            } else {
                let error = message.get("error");
                let message_text = error
                    .and_then(|error| error.get("message"))
                    .and_then(Value::as_str)
                    .unwrap_or("Unknown error")
                    .to_string();
                Err(message_text)
            };
            // Ignore send failures when the caller already timed out.
            match pending.sender.try_send(outcome) {
                Ok(()) | Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => {}
            }
        }
    }

    fn handle_exit(self: &Arc<Self>) {
        *self.child.lock() = None;
        *self.stdin.lock() = None;
        self.reject_all_pending("Daemon exited");
        self.dispatch_event(
            poratake_daemon_common::contract::SYSTEM_EXIT_EVENT,
            &Value::Null,
        );

        if self.shutting_down.load(Ordering::SeqCst) {
            return;
        }

        self.schedule_restart();
    }

    fn schedule_restart(self: &Arc<Self>) {
        let attempts = self.restart_attempts.fetch_add(1, Ordering::SeqCst);
        if attempts >= MAX_RESTART_ATTEMPTS {
            eprintln!("[daemon] Max restart attempts reached");
            return;
        }

        let delay = RESTART_BACKOFF_BASE_MS * 2u64.saturating_pow(attempts);
        eprintln!(
            "[daemon] Restarting in {delay}ms (attempt {})",
            attempts + 1
        );
        thread::sleep(Duration::from_millis(delay));

        if self.shutting_down.load(Ordering::SeqCst) || self.child.lock().is_some() {
            return;
        }

        let handle = DaemonHandle {
            inner: self.clone(),
        };
        if let Err(error) = handle.start() {
            eprintln!("[daemon] restart failed: {error}");
            self.schedule_restart();
        } else {
            self.restart_attempts.store(0, Ordering::SeqCst);
        }
    }

    fn reject_all_pending(&self, message: &str) {
        let mut pending = self.pending.lock();
        for (_, entry) in pending.drain() {
            let _ = entry.sender.try_send(Err(message.to_string()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scoped_event_subscriptions_are_removed_when_dropped() {
        let daemon = DaemonHandle::with_binary(PathBuf::from("unused"));
        let events = Arc::new(AtomicU32::new(0));
        let received = events.clone();
        let subscription = daemon.subscribe(Arc::new(move |event, _| {
            if event == "timer-control:cancel" {
                received.fetch_add(1, Ordering::Relaxed);
            }
        }));

        daemon
            .inner
            .handle_line(r#"{"event":"timer-control:cancel"}"#);
        assert_eq!(events.load(Ordering::Relaxed), 1);
        drop(subscription);
        daemon
            .inner
            .handle_line(r#"{"event":"timer-control:cancel"}"#);
        assert_eq!(events.load(Ordering::Relaxed), 1);
        assert!(daemon.inner.event_handlers.lock().is_empty());
    }

    #[test]
    fn daemon_exit_is_observable_to_scoped_waiters() {
        let daemon = DaemonHandle::with_binary(PathBuf::from("unused"));
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        let _subscription = daemon.subscribe(Arc::new(move |event, _| {
            if event == poratake_daemon_common::contract::SYSTEM_EXIT_EVENT {
                tx.send(()).ok();
            }
        }));

        daemon.inner.dispatch_event(
            poratake_daemon_common::contract::SYSTEM_EXIT_EVENT,
            &Value::Null,
        );
        assert!(rx.try_recv().is_ok());
    }

    #[test]
    fn recording_overlay_is_exposed_only_through_the_typed_client() {
        let _ = DaemonHandle::recording_overlay;
        let _ = RecordingOverlayClient::show_window;
        let _ = RecordingOverlayClient::hide;
    }
}
