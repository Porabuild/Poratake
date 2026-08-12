use super::recorder_types::{
    recording_project_dir, CaptureRect, RecorderError, RecorderState, RecorderStatus,
    RecordingConfig, RecordingResult, StagedAsset,
};
use super::recording_audio::{AudioCaptureSet, AudioDevice};
use super::recording_camera::{CameraRecordingConfig, CameraSyncClock, RecordingCamera};
use super::recording_input::{InputTracker, TrackerBounds, TrackerSource};
use crate::com::retain_process_mta;
use crate::display_color::hdr_white_scale;
use crate::tone_map::{source_view, target_view, FitStage, ToneMapStage};
use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};
use windows::core::{factory, implement, IInspectable, Interface, BOOL, PCWSTR};
use windows::Foundation::TypedEventHandler;
use windows::Graphics::Capture::{
    Direct3D11CaptureFrame, Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession,
};
use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Graphics::SizeInt32;
use windows::Win32::Foundation::{HMODULE, HWND, LPARAM, RECT};
use windows::Win32::Graphics::Direct3D::{
    D3D_DRIVER_TYPE, D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP, D3D_FEATURE_LEVEL,
    D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL_11_1,
};
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11RenderTargetView, ID3D11Resource,
    ID3D11ShaderResourceView, ID3D11Texture2D, D3D11_BIND_RENDER_TARGET,
    D3D11_BIND_SHADER_RESOURCE, D3D11_BOX, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
    D3D11_CREATE_DEVICE_VIDEO_SUPPORT, D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC,
    D3D11_USAGE_DEFAULT,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
use windows::Win32::Graphics::Dxgi::{IDXGIAdapter, IDXGIDevice};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, MonitorFromWindow, HDC, HMONITOR, MONITORINFO,
    MONITORINFOEXW, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::Media::MediaFoundation::{
    IMF2DBuffer, IMFAsyncCallback, IMFAsyncCallback_Impl, IMFAsyncResult, IMFAttributes,
    IMFByteStream, IMFDXGIDeviceManager, IMFSample, IMFSinkWriter, MFCreateAttributes,
    MFCreateDXGIDeviceManager, MFCreateDXGISurfaceBuffer, MFCreateMediaType,
    MFCreateSinkWriterFromURL, MFCreateTrackedSample, MFMediaType_Video, MFShutdown, MFStartup,
    MFVideoFormat_ARGB32, MFVideoFormat_H264, MFVideoInterlace_Progressive, MFSTARTUP_FULL,
    MF_MT_ALL_SAMPLES_INDEPENDENT, MF_MT_AVG_BITRATE, MF_MT_FIXED_SIZE_SAMPLES, MF_MT_FRAME_RATE,
    MF_MT_FRAME_SIZE, MF_MT_INTERLACE_MODE, MF_MT_MAJOR_TYPE, MF_MT_MPEG2_PROFILE,
    MF_MT_PIXEL_ASPECT_RATIO, MF_MT_SAMPLE_SIZE, MF_MT_SUBTYPE,
    MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, MF_SINK_WRITER_D3D_MANAGER, MF_VERSION,
};
use windows::Win32::System::WinRT::Direct3D11::{
    CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess,
};
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;
use windows::Win32::System::WinRT::{RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED};
use windows::Win32::UI::WindowsAndMessaging::{IsWindow, MONITORINFOF_PRIMARY};

const FIRST_FRAME_TIMEOUT: Duration = Duration::from_secs(30);
const COMMAND_TIMEOUT: Duration = Duration::from_secs(5);
const DEVICE_COMMAND_TIMEOUT: Duration = Duration::from_secs(15);
const FRAME_WAIT: Duration = Duration::from_millis(20);
const ENCODER_BACKPRESSURE_TIMEOUT: Duration = Duration::from_secs(30);
const ENCODER_TEXTURE_COUNT: usize = 4;
const HNS_PER_SECOND: f64 = 10_000_000.0;

#[derive(Clone, Copy, Eq, PartialEq)]
enum ControllerPhase {
    Idle,
    Starting,
    Running,
    Stopping,
}

struct ControllerState {
    phase: ControllerPhase,
    status: RecorderStatus,
    command_sender: Option<Sender<WorkerCommand>>,
    mic_muted: bool,
    generation: u64,
}

enum WorkerCommand {
    Pause(Sender<Result<RecorderStatus, RecorderError>>),
    Resume(Sender<Result<RecorderStatus, RecorderError>>),
    SetMicMuted(bool),
    SetMicrophone(Option<AudioDevice>, Sender<Result<(), RecorderError>>),
    SetSystemAudio(bool, Sender<Result<(), RecorderError>>),
    SetCamera(bool, Sender<Result<(), RecorderError>>),
    Stop(Sender<Result<RecordingResult, RecorderError>>),
}

pub struct CaptureController {
    state: Arc<Mutex<ControllerState>>,
}

pub struct CaptureStart {
    pub started: Receiver<Result<RecorderStatus, RecorderError>>,
    pub failure: CaptureFailure,
}

pub struct CaptureFailure {
    receiver: Receiver<RecorderError>,
    state: Arc<Mutex<ControllerState>>,
    generation: u64,
}

impl CaptureFailure {
    pub fn recv(self) -> Option<RecorderError> {
        let error = self.receiver.recv().ok()?;
        let state = self.state.lock().ok()?;
        if state.generation != self.generation {
            return None;
        }
        Some(error)
    }
}

impl CaptureController {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(ControllerState {
                phase: ControllerPhase::Idle,
                status: RecorderStatus::idle(),
                command_sender: None,
                mic_muted: false,
                generation: 0,
            })),
        }
    }

    pub fn start(&self, config: RecordingConfig) -> Result<CaptureStart, RecorderError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| RecorderError::start("Recorder state is unavailable"))?;

        if state.phase != ControllerPhase::Idle {
            return Err(RecorderError::invalid_state(format!(
                "Cannot start: recorder is {}",
                state.status.state.as_str()
            )));
        }

        let (command_sender, command_receiver) = std::sync::mpsc::channel();
        let (start_sender, start_receiver) = std::sync::mpsc::channel();
        let (failure_sender, failure_receiver) = std::sync::mpsc::channel();
        state.generation = state.generation.wrapping_add(1);
        let generation = state.generation;
        state.phase = ControllerPhase::Starting;
        state.status = RecorderStatus::idle();
        state.command_sender = Some(command_sender);
        let mic_muted = state.mic_muted;
        drop(state);

        let shared = self.state.clone();
        let panic_shared = shared.clone();
        let panic_start_sender = start_sender.clone();
        let panic_failure_sender = failure_sender.clone();
        let started = Arc::new(AtomicBool::new(false));
        let panic_started = started.clone();
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                run_worker(
                    config,
                    mic_muted,
                    shared,
                    command_receiver,
                    start_sender,
                    failure_sender,
                    generation,
                    started,
                );
            }));
            if result.is_err() {
                recover_worker_panic(
                    &panic_shared,
                    generation,
                    panic_started.load(Ordering::Acquire),
                    &panic_start_sender,
                    &panic_failure_sender,
                );
            }
        });
        Ok(CaptureStart {
            started: start_receiver,
            failure: CaptureFailure {
                receiver: failure_receiver,
                state: self.state.clone(),
                generation,
            },
        })
    }

    pub fn pause(&self) -> Result<RecorderStatus, RecorderError> {
        let sender = self.command_sender_for(RecorderState::Recording, "pause")?;
        let (result_sender, result_receiver) = std::sync::mpsc::channel();
        sender
            .send(WorkerCommand::Pause(result_sender))
            .map_err(|_| RecorderError::capture("Recorder worker is unavailable"))?;
        result_receiver
            .recv_timeout(COMMAND_TIMEOUT)
            .map_err(|_| RecorderError::capture("Recorder did not pause in time"))?
    }

    pub fn resume(&self) -> Result<RecorderStatus, RecorderError> {
        let sender = self.command_sender_for(RecorderState::Paused, "resume")?;
        let (result_sender, result_receiver) = std::sync::mpsc::channel();
        sender
            .send(WorkerCommand::Resume(result_sender))
            .map_err(|_| RecorderError::capture("Recorder worker is unavailable"))?;
        result_receiver
            .recv_timeout(COMMAND_TIMEOUT)
            .map_err(|_| RecorderError::capture("Recorder did not resume in time"))?
    }

    pub fn stop(&self) -> Result<Receiver<Result<RecordingResult, RecorderError>>, RecorderError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| RecorderError::stop("Recorder state is unavailable"))?;

        if state.phase != ControllerPhase::Running
            || !matches!(
                state.status.state,
                RecorderState::Recording | RecorderState::Paused
            )
        {
            return Err(RecorderError::invalid_state(format!(
                "Cannot stop: recorder is {}",
                state.status.state.as_str()
            )));
        }

        let sender = state
            .command_sender
            .clone()
            .ok_or_else(|| RecorderError::capture("Recorder worker is unavailable"))?;
        let (result_sender, result_receiver) = std::sync::mpsc::channel();
        state.phase = ControllerPhase::Stopping;
        state.status.state = RecorderState::Idle;
        if sender.send(WorkerCommand::Stop(result_sender)).is_err() {
            state.phase = ControllerPhase::Idle;
            state.status = RecorderStatus::idle();
            state.command_sender = None;
            return Err(RecorderError::capture("Recorder worker is unavailable"));
        }
        Ok(result_receiver)
    }

    pub fn status(&self) -> RecorderStatus {
        self.state
            .lock()
            .map(|state| state.status.clone())
            .unwrap_or_else(|_| RecorderStatus::idle())
    }

    pub fn set_mic_muted(&self, muted: bool) {
        if let Ok(mut state) = self.state.lock() {
            state.mic_muted = muted;
            if matches!(
                state.phase,
                ControllerPhase::Starting | ControllerPhase::Running
            ) {
                if let Some(sender) = &state.command_sender {
                    let _ = sender.send(WorkerCommand::SetMicMuted(muted));
                }
            }
        }
    }

    pub fn set_microphone(&self, device: Option<AudioDevice>) -> Result<(), RecorderError> {
        self.run_device_command("change the microphone", |response| {
            WorkerCommand::SetMicrophone(device, response)
        })
    }

    pub fn set_system_audio(&self, enabled: bool) -> Result<(), RecorderError> {
        self.run_device_command("change system audio", |response| {
            WorkerCommand::SetSystemAudio(enabled, response)
        })
    }

    pub fn set_camera(&self, enabled: bool) -> Result<(), RecorderError> {
        self.run_device_command("change the camera", |response| {
            WorkerCommand::SetCamera(enabled, response)
        })
    }

    fn run_device_command<F>(&self, operation: &str, build: F) -> Result<(), RecorderError>
    where
        F: FnOnce(Sender<Result<(), RecorderError>>) -> WorkerCommand,
    {
        let sender = self.active_command_sender(operation)?;
        let (result_sender, result_receiver) = std::sync::mpsc::channel();
        sender
            .send(build(result_sender))
            .map_err(|_| RecorderError::capture("Recorder worker is unavailable"))?;
        result_receiver
            .recv_timeout(DEVICE_COMMAND_TIMEOUT)
            .map_err(|_| RecorderError::capture(format!("Recorder did not {operation} in time")))?
    }

    fn active_command_sender(
        &self,
        operation: &str,
    ) -> Result<Sender<WorkerCommand>, RecorderError> {
        let state = self
            .state
            .lock()
            .map_err(|_| RecorderError::capture("Recorder state is unavailable"))?;

        if state.phase != ControllerPhase::Running
            || !matches!(
                state.status.state,
                RecorderState::Recording | RecorderState::Paused
            )
        {
            return Err(RecorderError::invalid_state(format!(
                "Cannot {operation}: recorder is {}",
                state.status.state.as_str()
            )));
        }

        state
            .command_sender
            .clone()
            .ok_or_else(|| RecorderError::capture("Recorder worker is unavailable"))
    }

    fn command_sender_for(
        &self,
        required_state: RecorderState,
        operation: &str,
    ) -> Result<Sender<WorkerCommand>, RecorderError> {
        let state = self
            .state
            .lock()
            .map_err(|_| RecorderError::capture("Recorder state is unavailable"))?;

        if state.phase != ControllerPhase::Running || state.status.state != required_state {
            return Err(RecorderError::invalid_state(format!(
                "Cannot {operation}: recorder is {}",
                state.status.state.as_str()
            )));
        }

        state
            .command_sender
            .clone()
            .ok_or_else(|| RecorderError::capture("Recorder worker is unavailable"))
    }
}

fn run_worker(
    config: RecordingConfig,
    mic_muted: bool,
    shared: Arc<Mutex<ControllerState>>,
    commands: Receiver<WorkerCommand>,
    start_sender: Sender<Result<RecorderStatus, RecorderError>>,
    failure_sender: Sender<RecorderError>,
    generation: u64,
    started: Arc<AtomicBool>,
) {
    let apartment = match RecordingApartment::initialize() {
        Ok(apartment) => apartment,
        Err(error) => {
            let failure = RecorderError::start(format!(
                "Failed to initialize the Windows recording apartment: {error}"
            ));
            set_idle(&shared, generation);
            let _ = start_sender.send(Err(failure));
            return;
        }
    };

    let mut runtime = match RecordingRuntime::prepare(&config, mic_muted) {
        Ok(runtime) => runtime,
        Err(error) => {
            set_idle(&shared, generation);
            let _ = start_sender.send(Err(error));
            return;
        }
    };

    if let Err(error) = runtime.start() {
        runtime.abort();
        set_idle(&shared, generation);
        let _ = start_sender.send(Err(error));
        drop(runtime);
        drop(apartment);
        return;
    }

    let start_deadline = Instant::now() + FIRST_FRAME_TIMEOUT;
    let mut start_sender = Some(start_sender);
    let mut failure_sender = Some(failure_sender);
    let mut paused = false;

    loop {
        match commands.try_recv() {
            Ok(command) => {
                if handle_command(command, &mut runtime, &shared, &mut paused, generation) {
                    break;
                }
                continue;
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                fail_worker(
                    RecorderError::capture("Recorder controller disconnected"),
                    &mut runtime,
                    &shared,
                    &mut start_sender,
                    &mut failure_sender,
                    generation,
                );
                break;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
        }

        if let Some(error) = runtime.try_error() {
            fail_worker(
                error,
                &mut runtime,
                &shared,
                &mut start_sender,
                &mut failure_sender,
                generation,
            );
            break;
        }

        if start_sender.is_some() && Instant::now() >= start_deadline {
            let error = RecorderError::capture("Timed out waiting for the first captured frame");
            fail_worker(
                error,
                &mut runtime,
                &shared,
                &mut start_sender,
                &mut failure_sender,
                generation,
            );
            break;
        }

        match runtime.frames.recv_timeout(FRAME_WAIT) {
            Ok(FrameMessage::Frame(frame)) => {
                let source_time = match frame_time(&frame) {
                    Ok(source_time) => source_time,
                    Err(error) => {
                        fail_worker(
                            error,
                            &mut runtime,
                            &shared,
                            &mut start_sender,
                            &mut failure_sender,
                            generation,
                        );
                        break;
                    }
                };
                if paused {
                    runtime.timeline.observe(source_time);
                    continue;
                }

                match runtime.write_frame(&frame, source_time) {
                    Ok(Some(duration)) => {
                        if start_sender.is_some() {
                            if let Err(error) = runtime.sync_with_first_frame(source_time) {
                                fail_worker(
                                    error,
                                    &mut runtime,
                                    &shared,
                                    &mut start_sender,
                                    &mut failure_sender,
                                    generation,
                                );
                                break;
                            }
                        }
                        update_duration(&shared, duration, generation);
                        if let Some(sender) = start_sender.take() {
                            let status = RecorderStatus {
                                state: RecorderState::Recording,
                                duration,
                            };
                            set_running(&shared, status.clone(), generation);
                            if sender.send(Ok(status)).is_ok() {
                                started.store(true, Ordering::Release);
                            }
                        }
                    }
                    Ok(None) => {}
                    Err(error) => {
                        fail_worker(
                            error,
                            &mut runtime,
                            &shared,
                            &mut start_sender,
                            &mut failure_sender,
                            generation,
                        );
                        break;
                    }
                }
            }
            Ok(FrameMessage::Closed(records_window)) => {
                // A closed window must not lose the recording: stop feeding the
                // encoder, tell the app, and stay alive so its stop finalizes
                // everything captured so far.
                if records_window && start_sender.is_none() {
                    if let Some(sender) = failure_sender.take() {
                        let _ = sender.send(RecorderError::target_closed(
                            "The recorded window was closed",
                        ));
                    }
                    continue;
                }

                let message = match records_window {
                    true => "The window to record was closed",
                    false => "The captured display was disconnected",
                };
                fail_worker(
                    RecorderError::capture(message),
                    &mut runtime,
                    &shared,
                    &mut start_sender,
                    &mut failure_sender,
                    generation,
                );
                break;
            }
            Ok(FrameMessage::Error(error)) => {
                fail_worker(
                    error,
                    &mut runtime,
                    &shared,
                    &mut start_sender,
                    &mut failure_sender,
                    generation,
                );
                break;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                fail_worker(
                    RecorderError::capture("Windows capture frame source stopped"),
                    &mut runtime,
                    &shared,
                    &mut start_sender,
                    &mut failure_sender,
                    generation,
                );
                break;
            }
        }
    }

    drop(runtime);
    drop(apartment);
}

fn recover_worker_panic(
    shared: &Arc<Mutex<ControllerState>>,
    generation: u64,
    started: bool,
    start_sender: &Sender<Result<RecorderStatus, RecorderError>>,
    failure_sender: &Sender<RecorderError>,
) {
    let (mut state, was_poisoned) = match shared.lock() {
        Ok(state) => (state, false),
        Err(poisoned) => (poisoned.into_inner(), true),
    };
    if state.generation != generation || state.phase == ControllerPhase::Idle {
        drop(state);
        if was_poisoned {
            shared.clear_poison();
        }
        return;
    }
    state.phase = ControllerPhase::Idle;
    state.status = RecorderStatus::idle();
    state.command_sender = None;
    drop(state);
    if was_poisoned {
        shared.clear_poison();
    }
    if started {
        let _ = failure_sender.send(RecorderError::capture(
            "Recorder worker terminated unexpectedly",
        ));
    } else {
        let _ = start_sender.send(Err(RecorderError::start(
            "Recorder worker terminated unexpectedly",
        )));
    }
}

fn report_worker_failure(
    error: RecorderError,
    start_sender: &mut Option<Sender<Result<RecorderStatus, RecorderError>>>,
    failure_sender: &mut Option<Sender<RecorderError>>,
) {
    if let Some(sender) = start_sender.take() {
        let _ = sender.send(Err(error));
        return;
    }
    if let Some(sender) = failure_sender.take() {
        let _ = sender.send(error);
    }
}

fn fail_worker(
    error: RecorderError,
    runtime: &mut RecordingRuntime,
    shared: &Arc<Mutex<ControllerState>>,
    start_sender: &mut Option<Sender<Result<RecorderStatus, RecorderError>>>,
    failure_sender: &mut Option<Sender<RecorderError>>,
    generation: u64,
) {
    if start_sender.is_some() {
        runtime.abort();
        set_idle(shared, generation);
        report_worker_failure(error, start_sender, failure_sender);
        return;
    }
    runtime.abort();
    set_idle(shared, generation);
    report_worker_failure(error, start_sender, failure_sender);
}

struct RecordingApartment;

impl RecordingApartment {
    fn initialize() -> Result<Self, windows::core::Error> {
        retain_process_mta()?;
        unsafe { RoInitialize(RO_INIT_MULTITHREADED) }?;
        Ok(Self)
    }
}

impl Drop for RecordingApartment {
    fn drop(&mut self) {
        unsafe {
            RoUninitialize();
        }
    }
}

fn handle_command(
    command: WorkerCommand,
    runtime: &mut RecordingRuntime,
    shared: &Arc<Mutex<ControllerState>>,
    paused: &mut bool,
    generation: u64,
) -> bool {
    match command {
        WorkerCommand::Pause(response) => {
            if *paused {
                let _ = response.send(Err(RecorderError::invalid_state(
                    "Cannot pause: recorder is paused",
                )));
                return false;
            }

            if let Some(camera) = runtime.camera.as_ref() {
                if let Err(error) = camera.pause() {
                    let _ = response.send(Err(error));
                    return false;
                }
            }
            *paused = true;
            runtime.timeline.pause();
            if let Some(audio) = runtime.audio.as_ref() {
                audio.pause(runtime.timeline.source_time());
            }
            if let Some(input) = runtime.input.as_ref() {
                input.pause();
            }
            let status = RecorderStatus {
                state: RecorderState::Paused,
                duration: runtime.timeline.duration(),
            };
            set_running(shared, status.clone(), generation);
            let _ = response.send(Ok(status));
            false
        }
        WorkerCommand::Resume(response) => {
            if !*paused {
                let _ = response.send(Err(RecorderError::invalid_state(
                    "Cannot resume: recorder is recording",
                )));
                return false;
            }

            if let Some(camera) = runtime.camera.as_ref() {
                if let Err(error) = camera.resume() {
                    let _ = response.send(Err(error));
                    return false;
                }
            }
            *paused = false;
            let status = RecorderStatus {
                state: RecorderState::Recording,
                duration: runtime.timeline.duration(),
            };
            set_running(shared, status.clone(), generation);
            let _ = response.send(Ok(status));
            false
        }
        WorkerCommand::SetMicMuted(muted) => {
            if let Some(audio) = runtime.audio.as_ref() {
                audio.set_mic_muted(muted);
            }
            false
        }
        WorkerCommand::SetMicrophone(device, response) => {
            let _ = response.send(runtime.set_microphone(device));
            false
        }
        WorkerCommand::SetSystemAudio(enabled, response) => {
            let _ = response.send(runtime.set_system_audio(enabled));
            false
        }
        WorkerCommand::SetCamera(enabled, response) => {
            let _ = response.send(runtime.set_camera(enabled));
            false
        }
        WorkerCommand::Stop(response) => {
            let result = runtime.finish();
            set_idle(shared, generation);
            let _ = response.send(result);
            true
        }
    }
}

fn set_running(shared: &Arc<Mutex<ControllerState>>, status: RecorderStatus, generation: u64) {
    if let Ok(mut state) = shared.lock() {
        if state.generation != generation {
            return;
        }
        state.phase = ControllerPhase::Running;
        state.status = status;
    }
}

fn update_duration(shared: &Arc<Mutex<ControllerState>>, duration: f64, generation: u64) {
    if let Ok(mut state) = shared.lock() {
        if state.generation != generation {
            return;
        }
        state.status.duration = duration;
    }
}

fn set_idle(shared: &Arc<Mutex<ControllerState>>, generation: u64) {
    if let Ok(mut state) = shared.lock() {
        if state.generation != generation {
            return;
        }
        state.phase = ControllerPhase::Idle;
        state.status = RecorderStatus::idle();
        state.command_sender = None;
    }
}

enum FrameMessage {
    Frame(Direct3D11CaptureFrame),
    Closed(bool),
    Error(RecorderError),
}

struct RecordingRuntime {
    capture: Option<WindowsCapture>,
    encoder: Option<MediaFoundationEncoder>,
    input: Option<InputTracker>,
    audio: Option<AudioCaptureSet>,
    camera: Option<RecordingCamera>,
    frames: Receiver<FrameMessage>,
    timeline: VideoTimeline,
    follows_target: bool,
    output_path: PathBuf,
}

impl RecordingRuntime {
    fn prepare(config: &RecordingConfig, mic_muted: bool) -> Result<Self, RecorderError> {
        let project_dir = recording_project_dir(&config.output_path)?.to_path_buf();
        let target = CaptureTarget::resolve(config)?;
        let tracker_source = target.tracker_source();
        let (device, context, winrt_device) = create_d3d_device()?;
        let (capture, frames, layout) = WindowsCapture::prepare(winrt_device, target)?;
        let follows_target = layout.fitted;
        let encoder = match MediaFoundationEncoder::new(
            &config.output_path,
            layout.width,
            layout.height,
            config.frame_rate,
            device,
            context,
            layout.crop,
            layout.white_scale,
            layout.fitted,
        ) {
            Ok(encoder) => encoder,
            Err(error) => {
                capture.close();
                return Err(error);
            }
        };

        let input = match InputTracker::start(
            TrackerBounds::new(tracker_source, layout.tracker_bounds),
            config.keyboard_enabled,
        ) {
            Ok(input) => input,
            Err(error) => {
                capture.close();
                encoder.abort();
                return Err(error);
            }
        };
        let audio = match AudioCaptureSet::start(config, mic_muted) {
            Ok(audio) => audio,
            Err(error) => {
                capture.close();
                encoder.abort();
                input.abort();
                return Err(error);
            }
        };
        let camera = if config.camera_enabled {
            let mut camera = RecordingCamera::new();
            if let Err(error) = camera.start(CameraRecordingConfig {
                project_dir,
                device_id: config.camera_device_id.clone(),
                device_name: config.camera_device_name.clone(),
                frame_rate: config.frame_rate,
            }) {
                capture.close();
                encoder.abort();
                input.abort();
                audio.abort();
                return Err(error);
            }
            Some(camera)
        } else {
            None
        };

        Ok(Self {
            capture: Some(capture),
            encoder: Some(encoder),
            input: Some(input),
            audio: Some(audio),
            camera,
            frames,
            timeline: VideoTimeline::new(config.frame_rate),
            follows_target,
            output_path: config.output_path.clone(),
        })
    }

    fn start(&self) -> Result<(), RecorderError> {
        self.capture
            .as_ref()
            .ok_or_else(|| RecorderError::capture("Capture session is unavailable"))?
            .start()
    }

    fn write_frame(
        &mut self,
        frame: &Direct3D11CaptureFrame,
        source_time: i64,
    ) -> Result<Option<f64>, RecorderError> {
        if self.timeline.is_resuming() {
            if let Some(audio) = self.audio.as_ref() {
                audio.resume(source_time);
            }
            if let Some(input) = self.input.as_ref() {
                input.resume();
            }
        }
        let content = frame.ContentSize().map_err(|error| {
            RecorderError::capture(format!("Failed to read captured frame size: {error}"))
        })?;
        let Some(timestamp) = self.timeline.timestamp_for(source_time) else {
            return self.follow_content_size(content).map(|_| None);
        };

        let encoder = self
            .encoder
            .as_mut()
            .ok_or_else(|| RecorderError::capture("Video encoder is unavailable"))?;
        encoder.write(frame, content, timestamp, self.timeline.frame_duration())?;
        self.timeline.commit(timestamp);
        self.follow_content_size(content)?;
        Ok(Some(self.timeline.duration()))
    }

    fn follow_content_size(&mut self, content: SizeInt32) -> Result<(), RecorderError> {
        if !self.follows_target {
            return Ok(());
        }
        let Some(capture) = self.capture.as_mut() else {
            return Ok(());
        };
        if !capture.resize_pool(content)? {
            return Ok(());
        }

        // The recreated pool allocates new textures, so views cached against
        // the old ones would keep rendering pixels nobody writes to any more.
        if let Some(encoder) = self.encoder.as_mut() {
            encoder.forget_source_views();
        }
        Ok(())
    }

    fn set_microphone(&mut self, device: Option<AudioDevice>) -> Result<(), RecorderError> {
        self.audio_mut()?.set_microphone(device)
    }

    fn set_system_audio(&mut self, enabled: bool) -> Result<(), RecorderError> {
        self.audio_mut()?.set_system_audio(enabled)
    }

    fn set_camera(&mut self, enabled: bool) -> Result<(), RecorderError> {
        self.camera
            .as_ref()
            .ok_or_else(|| {
                RecorderError::invalid_state("This recording was not started with a camera")
            })?
            .set_enabled(enabled)
    }

    fn audio_mut(&mut self) -> Result<&mut AudioCaptureSet, RecorderError> {
        self.audio
            .as_mut()
            .ok_or_else(|| RecorderError::capture("Audio capture is unavailable"))
    }

    fn try_error(&self) -> Option<RecorderError> {
        if let Some(error) = self.input.as_ref().and_then(InputTracker::try_error) {
            return Some(error);
        }
        if let Some(error) = self.audio.as_ref().and_then(AudioCaptureSet::try_error) {
            return Some(error);
        }
        self.camera.as_ref().and_then(RecordingCamera::try_error)
    }

    fn sync_with_first_frame(&self, source_time: i64) -> Result<(), RecorderError> {
        let before = Instant::now();
        let wall_time = SystemTime::now();
        let after = Instant::now();
        if let Some(camera) = self.camera.as_ref() {
            camera.sync_with_screen_start(CameraSyncClock {
                monotonic_time: before + after.duration_since(before) / 2,
                wall_time,
            })?;
        }
        if let Some(input) = self.input.as_ref() {
            input.sync_with_first_frame();
        }
        if let Some(audio) = self.audio.as_ref() {
            audio.sync_with_first_frame(source_time);
        }
        Ok(())
    }

    fn finish(&mut self) -> Result<RecordingResult, RecorderError> {
        if let Some(capture) = self.capture.take() {
            capture.close();
        }
        let duration = self.timeline.duration();
        let camera = self
            .camera
            .take()
            .map(|mut camera| std::thread::spawn(move || camera.stop()));
        let audio_result = self
            .audio
            .take()
            .ok_or_else(|| RecorderError::stop("Audio capture lifecycle is unavailable"))
            .and_then(|audio| audio.finish(duration));
        let camera_result = camera
            .map(|thread| {
                thread
                    .join()
                    .map_err(|_| RecorderError::stop("Camera finalizer terminated unexpectedly"))?
                    .map(Some)
            })
            .unwrap_or(Ok(None));
        let input_result = self
            .input
            .take()
            .ok_or_else(|| RecorderError::stop("Input tracker is unavailable"))
            .and_then(|input| input.finish(&self.output_path, duration));
        let encoder_result = self
            .encoder
            .take()
            .ok_or_else(|| RecorderError::stop("Video encoder is unavailable"))
            .and_then(MediaFoundationEncoder::finish);
        let mut staged_assets = Vec::new();
        let mut failure = None;

        let (cursor_path, keys_path) = match input_result {
            Ok(input) => {
                let cursor_path = input.cursor.output_path.clone();
                let keys_path = input.keys.as_ref().map(|asset| asset.output_path.clone());
                staged_assets.push(input.cursor);
                staged_assets.extend(input.keys);
                (cursor_path, keys_path)
            }
            Err(error) => {
                failure = Some(error);
                (PathBuf::new(), None)
            }
        };
        let (system_audio_path, mic_audio_path) = match audio_result {
            Ok(audio) => {
                let system_audio_path = audio
                    .system_audio
                    .as_ref()
                    .map(|asset| asset.output_path.clone());
                let mic_audio_path = audio
                    .microphone_audio
                    .as_ref()
                    .map(|asset| asset.output_path.clone());
                staged_assets.extend(audio.system_audio);
                staged_assets.extend(audio.microphone_audio);
                (system_audio_path, mic_audio_path)
            }
            Err(error) => {
                failure.get_or_insert(error);
                (None, None)
            }
        };
        let camera_path = match camera_result {
            Ok(Some(camera)) => {
                let camera_path = Some(camera.video_path);
                staged_assets.extend(camera.staged_assets);
                camera_path
            }
            Ok(None) => None,
            Err(error) => {
                failure.get_or_insert(error);
                None
            }
        };
        match encoder_result {
            Ok(video) => staged_assets.push(video),
            Err(error) => {
                failure.get_or_insert(error);
            }
        }
        if let Some(error) = failure {
            cleanup_staged_assets(&staged_assets);
            return Err(error);
        }
        publish_staged_assets(&staged_assets)?;
        Ok(RecordingResult {
            output_path: self.output_path.clone(),
            cursor_path,
            keys_path,
            camera_path,
            system_audio_path,
            mic_audio_path,
            duration,
        })
    }

    fn abort(&mut self) {
        if let Some(capture) = self.capture.take() {
            capture.close();
        }
        if let Some(input) = self.input.take() {
            input.abort();
        }
        if let Some(audio) = self.audio.take() {
            audio.abort();
        }
        if let Some(mut camera) = self.camera.take() {
            camera.abort();
        }
        if let Some(encoder) = self.encoder.take() {
            encoder.abort();
        }
    }
}

struct PublishedAsset {
    output_path: PathBuf,
    backup_path: Option<PathBuf>,
}

fn publish_staged_assets(assets: &[StagedAsset]) -> Result<(), RecorderError> {
    let mut backup_paths = Vec::with_capacity(assets.len());
    for asset in assets {
        if !asset.temporary_path.is_file() {
            cleanup_staged_assets(assets);
            return Err(RecorderError::stop(format!(
                "Staged recording asset is missing: {}",
                asset.temporary_path.display()
            )));
        }
        if asset.output_path.exists() {
            let backup = match transaction_backup_path(&asset.output_path) {
                Ok(backup) => backup,
                Err(error) => {
                    cleanup_staged_assets(assets);
                    return Err(error);
                }
            };
            if backup.exists() {
                cleanup_staged_assets(assets);
                return Err(RecorderError::stop(format!(
                    "Recording asset backup already exists: {}",
                    backup.display()
                )));
            }
            backup_paths.push(Some(backup));
        } else {
            backup_paths.push(None);
        }
    }

    let mut published = Vec::new();
    for (asset, backup_path) in assets.iter().zip(backup_paths) {
        if let Some(backup) = &backup_path {
            if let Err(error) = std::fs::rename(&asset.output_path, &backup) {
                rollback_published_assets(&published);
                cleanup_staged_assets(assets);
                return Err(RecorderError::stop(format!(
                    "Failed to stage existing recording asset: {error}"
                )));
            }
        }

        if let Err(error) = std::fs::rename(&asset.temporary_path, &asset.output_path) {
            if let Some(backup) = &backup_path {
                let _ = std::fs::rename(backup, &asset.output_path);
            }
            rollback_published_assets(&published);
            cleanup_staged_assets(assets);
            return Err(RecorderError::stop(format!(
                "Failed to publish recording project: {error}"
            )));
        }
        published.push(PublishedAsset {
            output_path: asset.output_path.clone(),
            backup_path,
        });
    }

    for asset in published {
        if let Some(backup) = asset.backup_path {
            let _ = std::fs::remove_file(backup);
        }
    }
    Ok(())
}

fn rollback_published_assets(published: &[PublishedAsset]) {
    for index in rollback_order(published.len()) {
        let asset = &published[index];
        let _ = std::fs::remove_file(&asset.output_path);
        if let Some(backup) = &asset.backup_path {
            let _ = std::fs::rename(backup, &asset.output_path);
        }
    }
}

fn rollback_order(count: usize) -> impl Iterator<Item = usize> {
    (0..count).rev()
}

fn cleanup_staged_assets(assets: &[StagedAsset]) {
    for asset in assets {
        let _ = std::fs::remove_file(&asset.temporary_path);
    }
}

fn transaction_backup_path(output_path: &Path) -> Result<PathBuf, RecorderError> {
    let name = output_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| RecorderError::stop("Recording asset has an invalid file name"))?;
    Ok(output_path.with_file_name(format!(".{name}.capty-backup")))
}

struct VideoTimeline {
    frame_duration: i64,
    first_source: Option<i64>,
    last_source: i64,
    pause_start: Option<i64>,
    total_pause: i64,
    last_written: Option<i64>,
}

impl VideoTimeline {
    fn new(frame_rate: u32) -> Self {
        Self {
            frame_duration: (10_000_000_i64 / i64::from(frame_rate)).max(1),
            first_source: None,
            last_source: 0,
            pause_start: None,
            total_pause: 0,
            last_written: None,
        }
    }

    fn observe(&mut self, source_time: i64) {
        self.last_source = source_time;
    }

    fn pause(&mut self) {
        self.pause_start = Some(self.last_source);
    }

    fn timestamp_for(&mut self, source_time: i64) -> Option<i64> {
        self.last_source = source_time;
        let first = *self.first_source.get_or_insert(source_time);

        if let Some(pause_start) = self.pause_start.take() {
            self.total_pause = self
                .total_pause
                .saturating_add(source_time.saturating_sub(pause_start));
        }

        let timestamp = source_time
            .saturating_sub(first)
            .saturating_sub(self.total_pause)
            .max(0);
        if let Some(last) = self.last_written {
            if timestamp <= last || timestamp.saturating_sub(last) < self.frame_duration {
                return None;
            }
        }
        Some(timestamp)
    }

    fn commit(&mut self, timestamp: i64) {
        self.last_written = Some(timestamp);
    }

    fn frame_duration(&self) -> i64 {
        self.frame_duration
    }

    fn source_time(&self) -> i64 {
        self.last_source
    }

    fn is_resuming(&self) -> bool {
        self.pause_start.is_some()
    }

    fn duration(&self) -> f64 {
        self.last_written
            .map(|timestamp| timestamp.saturating_add(self.frame_duration))
            .unwrap_or(0) as f64
            / HNS_PER_SECOND
    }
}

fn frame_time(frame: &Direct3D11CaptureFrame) -> Result<i64, RecorderError> {
    system_relative_time(frame.SystemRelativeTime())
}

fn system_relative_time(
    result: windows::core::Result<windows::Foundation::TimeSpan>,
) -> Result<i64, RecorderError> {
    result.map(|time| time.Duration).map_err(|error| {
        RecorderError::capture(format!("Failed to read captured frame timestamp: {error}"))
    })
}

#[derive(Clone)]
struct MonitorTarget {
    handle: isize,
    rect: RECT,
    device: String,
    device_number: i32,
    primary: bool,
}

struct CaptureTarget {
    monitor: MonitorTarget,
    requested_area: Option<CaptureRect>,
    window: Option<isize>,
}

impl CaptureTarget {
    fn resolve(config: &RecordingConfig) -> Result<Self, RecorderError> {
        let monitors = enumerate_monitors()?;
        if monitors.is_empty() {
            return Err(RecorderError::configuration("No display was found"));
        }

        if let Some(handle) = config.window_id {
            return Self::for_window(handle, &monitors);
        }

        if let Some(area) = config.capture_rect {
            let Some(monitor) = monitors
                .iter()
                .find(|monitor| monitor_contains(monitor.rect, area))
                .cloned()
            else {
                return Err(RecorderError::configuration(
                    "The recording area must be contained by one display",
                ));
            };
            return Ok(Self {
                monitor,
                requested_area: Some(area),
                window: None,
            });
        }

        if let Some(display_id) = config.display_id {
            let Some(monitor) = monitors
                .iter()
                .find(|monitor| monitor.device_number == display_id)
                .cloned()
            else {
                return Err(RecorderError::configuration(format!(
                    "Display {display_id} was not found"
                )));
            };
            return Ok(Self {
                monitor,
                requested_area: None,
                window: None,
            });
        }

        let monitor = monitors
            .iter()
            .find(|monitor| monitor.primary)
            .cloned()
            .unwrap_or_else(|| monitors[0].clone());
        Ok(Self {
            monitor,
            requested_area: None,
            window: None,
        })
    }

    fn for_window(handle: isize, monitors: &[MonitorTarget]) -> Result<Self, RecorderError> {
        let window = HWND(handle as *mut c_void);
        if !unsafe { IsWindow(Some(window)) }.as_bool() {
            return Err(RecorderError::configuration(
                "The window to record is no longer open",
            ));
        }

        let display = unsafe { MonitorFromWindow(window, MONITOR_DEFAULTTONEAREST) };
        let monitor = monitors
            .iter()
            .find(|monitor| monitor.handle == display.0 as isize)
            .or_else(|| monitors.iter().find(|monitor| monitor.primary))
            .cloned()
            .unwrap_or_else(|| monitors[0].clone());

        Ok(Self {
            monitor,
            requested_area: None,
            window: Some(handle),
        })
    }

    fn tracker_source(&self) -> TrackerSource {
        match self.window {
            Some(handle) => TrackerSource::Window(handle),
            None => TrackerSource::Screen,
        }
    }

    fn layout(&self, content: SizeInt32) -> Result<CaptureLayout, RecorderError> {
        if content.Width < 2 || content.Height < 2 {
            return Err(RecorderError::capture(
                "Captured display has invalid dimensions",
            ));
        }

        if self.window.is_some() {
            let width = content.Width & !1;
            let height = content.Height & !1;
            return Ok(CaptureLayout {
                width: width as u32,
                height: height as u32,
                crop: D3D11_BOX {
                    left: 0,
                    top: 0,
                    front: 0,
                    right: width as u32,
                    bottom: height as u32,
                    back: 1,
                },
                tracker_bounds: CaptureRect {
                    x: 0,
                    y: 0,
                    width,
                    height,
                },
                white_scale: hdr_white_scale(&self.monitor.device),
                fitted: true,
            });
        }

        let monitor_width = self.monitor.rect.right - self.monitor.rect.left;
        let monitor_height = self.monitor.rect.bottom - self.monitor.rect.top;
        if monitor_width <= 0 || monitor_height <= 0 {
            return Err(RecorderError::configuration(
                "Selected display has invalid bounds",
            ));
        }

        let (left, top, requested_width, requested_height, tracker_bounds) = if let Some(area) =
            self.requested_area
        {
            let scale_x = content.Width as f64 / monitor_width as f64;
            let scale_y = content.Height as f64 / monitor_height as f64;
            let left = ((area.x - self.monitor.rect.left) as f64 * scale_x).round() as i32;
            let top = ((area.y - self.monitor.rect.top) as f64 * scale_y).round() as i32;
            let right = ((area.right().unwrap_or(area.x) - self.monitor.rect.left) as f64 * scale_x)
                .round() as i32;
            let bottom = ((area.bottom().unwrap_or(area.y) - self.monitor.rect.top) as f64
                * scale_y)
                .round() as i32;
            (left, top, right - left, bottom - top, area)
        } else {
            (
                0,
                0,
                content.Width,
                content.Height,
                CaptureRect {
                    x: self.monitor.rect.left,
                    y: self.monitor.rect.top,
                    width: monitor_width,
                    height: monitor_height,
                },
            )
        };

        let width = requested_width & !1;
        let height = requested_height & !1;
        if left < 0
            || top < 0
            || width < 2
            || height < 2
            || left + width > content.Width
            || top + height > content.Height
        {
            return Err(RecorderError::configuration(
                "The recording area is outside the captured display surface",
            ));
        }

        Ok(CaptureLayout {
            width: width as u32,
            height: height as u32,
            crop: D3D11_BOX {
                left: left as u32,
                top: top as u32,
                front: 0,
                right: (left + width) as u32,
                bottom: (top + height) as u32,
                back: 1,
            },
            tracker_bounds,
            white_scale: hdr_white_scale(&self.monitor.device),
            fitted: false,
        })
    }
}

struct CaptureLayout {
    width: u32,
    height: u32,
    crop: D3D11_BOX,
    tracker_bounds: CaptureRect,
    white_scale: Option<f32>,
    fitted: bool,
}

fn capture_pixel_format(white_scale: Option<f32>) -> DirectXPixelFormat {
    match white_scale {
        Some(_) => DirectXPixelFormat::R16G16B16A16Float,
        None => DirectXPixelFormat::B8G8R8A8UIntNormalized,
    }
}

fn monitor_contains(rect: RECT, area: CaptureRect) -> bool {
    area.x >= rect.left
        && area.y >= rect.top
        && area.right().is_some_and(|right| right <= rect.right)
        && area.bottom().is_some_and(|bottom| bottom <= rect.bottom)
}

fn enumerate_monitors() -> Result<Vec<MonitorTarget>, RecorderError> {
    let mut monitors = Vec::new();
    unsafe extern "system" fn callback(
        monitor: HMONITOR,
        _dc: HDC,
        _rect: *mut RECT,
        data: LPARAM,
    ) -> BOOL {
        let monitors = unsafe { &mut *(data.0 as *mut Vec<MonitorTarget>) };
        let mut info = MONITORINFOEXW {
            monitorInfo: MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFOEXW>() as u32,
                ..Default::default()
            },
            ..Default::default()
        };

        if unsafe { GetMonitorInfoW(monitor, &mut info.monitorInfo) }.as_bool() {
            let name = String::from_utf16_lossy(&info.szDevice)
                .trim_end_matches('\0')
                .to_string();
            let device_number = name
                .chars()
                .filter(char::is_ascii_digit)
                .collect::<String>()
                .parse()
                .unwrap_or_default();
            monitors.push(MonitorTarget {
                handle: monitor.0 as isize,
                rect: info.monitorInfo.rcMonitor,
                device: name,
                device_number,
                primary: (info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY) != 0,
            });
        }
        BOOL(1)
    }

    let enumerated = unsafe {
        EnumDisplayMonitors(
            None,
            None,
            Some(callback),
            LPARAM(&mut monitors as *mut _ as isize),
        )
    };
    if !enumerated.as_bool() {
        return Err(RecorderError::configuration("Failed to list displays"));
    }
    Ok(monitors)
}

struct WindowsCapture {
    pool: Direct3D11CaptureFramePool,
    session: GraphicsCaptureSession,
    item: GraphicsCaptureItem,
    device: IDirect3DDevice,
    format: DirectXPixelFormat,
    pool_size: SizeInt32,
    frame_token: i64,
    closed_token: i64,
}

impl WindowsCapture {
    fn prepare(
        device: IDirect3DDevice,
        target: CaptureTarget,
    ) -> Result<(Self, Receiver<FrameMessage>, CaptureLayout), RecorderError> {
        if !GraphicsCaptureSession::IsSupported().unwrap_or(false) {
            return Err(RecorderError::configuration(
                "Windows Graphics Capture is not supported on this system",
            ));
        }

        let interop =
            factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>().map_err(|error| {
                RecorderError::capture(format!("Failed to open capture factory: {error}"))
            })?;
        let item: GraphicsCaptureItem = match target.window {
            Some(handle) => unsafe { interop.CreateForWindow(HWND(handle as *mut c_void)) }
                .map_err(|error| {
                    RecorderError::capture(format!("Failed to open window capture: {error}"))
                })?,
            None => {
                let monitor = HMONITOR(target.monitor.handle as *mut c_void);
                unsafe { interop.CreateForMonitor(monitor) }.map_err(|error| {
                    RecorderError::capture(format!("Failed to open display capture: {error}"))
                })?
            }
        };
        let content_size = item.Size().map_err(|error| {
            RecorderError::capture(format!("Failed to read capture size: {error}"))
        })?;
        let layout = target.layout(content_size)?;
        let format = capture_pixel_format(layout.white_scale);
        let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(&device, format, 4, content_size)
            .map_err(|error| {
                RecorderError::capture(format!("Failed to create capture frame pool: {error}"))
            })?;
        let session = pool.CreateCaptureSession(&item).map_err(|error| {
            RecorderError::capture(format!("Failed to create capture session: {error}"))
        })?;
        session.SetIsCursorCaptureEnabled(false).map_err(|error| {
            RecorderError::configuration(format!(
                "This Windows version cannot disable the captured cursor: {error}"
            ))
        })?;
        let _ = session.SetIsBorderRequired(false);

        let (sender, receiver) = std::sync::mpsc::channel();
        let frame_sender = sender.clone();
        let frame_handler =
            TypedEventHandler::<Direct3D11CaptureFramePool, IInspectable>::new(move |source, _| {
                let Some(source) = source.as_ref() else {
                    return Ok(());
                };
                match source.TryGetNextFrame() {
                    Ok(frame) => {
                        let _ = frame_sender.send(FrameMessage::Frame(frame));
                    }
                    Err(error) => {
                        let _ = frame_sender.send(FrameMessage::Error(RecorderError::capture(
                            format!("Failed to receive captured frame: {error}"),
                        )));
                    }
                }
                Ok(())
            });
        let frame_token = pool.FrameArrived(&frame_handler).map_err(|error| {
            RecorderError::capture(format!("Failed to subscribe to frames: {error}"))
        })?;

        let records_window = target.window.is_some();
        let closed_handler =
            TypedEventHandler::<GraphicsCaptureItem, IInspectable>::new(move |_, _| {
                let _ = sender.send(FrameMessage::Closed(records_window));
                Ok(())
            });
        let closed_token = item.Closed(&closed_handler).map_err(|error| {
            RecorderError::capture(format!("Failed to monitor the capture target: {error}"))
        })?;

        Ok((
            Self {
                pool,
                session,
                item,
                device,
                format,
                pool_size: content_size,
                frame_token,
                closed_token,
            },
            receiver,
            layout,
        ))
    }

    fn start(&self) -> Result<(), RecorderError> {
        self.session.StartCapture().map_err(|error| {
            RecorderError::capture(format!("Failed to start display capture: {error}"))
        })
    }

    /// Windows Graphics Capture keeps handing back frames sized to the pool it
    /// was created with, so a window that has been resized must be re-pooled
    /// before its new pixels can arrive.
    fn resize_pool(&mut self, content: SizeInt32) -> Result<bool, RecorderError> {
        if content.Width < 1
            || content.Height < 1
            || (content.Width == self.pool_size.Width && content.Height == self.pool_size.Height)
        {
            return Ok(false);
        }

        self.pool
            .Recreate(&self.device, self.format, 4, content)
            .map_err(|error| {
                RecorderError::capture(format!("Failed to resize the capture frame pool: {error}"))
            })?;
        self.pool_size = content;
        Ok(true)
    }

    fn close(self) {
        let _ = self.pool.RemoveFrameArrived(self.frame_token);
        let _ = self.item.RemoveClosed(self.closed_token);
        let _ = self.session.Close();
        let _ = self.pool.Close();
    }
}

fn create_d3d_device() -> Result<(ID3D11Device, ID3D11DeviceContext, IDirect3DDevice), RecorderError>
{
    let hardware = create_d3d_device_for_driver(D3D_DRIVER_TYPE_HARDWARE);
    let (device, context) = match hardware {
        Ok(result) => result,
        Err(_) => create_d3d_device_for_driver(D3D_DRIVER_TYPE_WARP)?,
    };
    let dxgi_device: IDXGIDevice = device.cast().map_err(|error| {
        RecorderError::capture(format!(
            "Failed to access the DXGI recording device: {error}"
        ))
    })?;
    let inspectable =
        unsafe { CreateDirect3D11DeviceFromDXGIDevice(&dxgi_device) }.map_err(|error| {
            RecorderError::capture(format!("Failed to create WinRT graphics device: {error}"))
        })?;
    let winrt_device: IDirect3DDevice = inspectable.cast().map_err(|error| {
        RecorderError::capture(format!("Failed to access WinRT graphics device: {error}"))
    })?;
    Ok((device, context, winrt_device))
}

fn create_d3d_device_for_driver(
    driver: D3D_DRIVER_TYPE,
) -> Result<(ID3D11Device, ID3D11DeviceContext), RecorderError> {
    match create_d3d_device_with_levels(driver, &[D3D_FEATURE_LEVEL_11_1, D3D_FEATURE_LEVEL_11_0]) {
        Ok(device) => Ok(device),
        Err(_) => create_d3d_device_with_levels(driver, &[D3D_FEATURE_LEVEL_11_0]),
    }
}

fn create_d3d_device_with_levels(
    driver: D3D_DRIVER_TYPE,
    levels: &[D3D_FEATURE_LEVEL],
) -> Result<(ID3D11Device, ID3D11DeviceContext), RecorderError> {
    let mut device = None;
    let mut context = None;
    unsafe {
        D3D11CreateDevice(
            None::<&IDXGIAdapter>,
            driver,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT | D3D11_CREATE_DEVICE_VIDEO_SUPPORT,
            Some(levels),
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut context),
        )
        .map_err(|error| {
            RecorderError::capture(format!("Failed to create D3D11 device: {error}"))
        })?;
    }
    let device = device.ok_or_else(|| RecorderError::capture("D3D11 device was not created"))?;
    let context = context.ok_or_else(|| RecorderError::capture("D3D11 context was not created"))?;
    Ok((device, context))
}

struct MediaFoundationEncoder {
    writer: Option<IMFSinkWriter>,
    stream_index: u32,
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    tone_map: Option<ToneMapStage>,
    fit: Option<FitStage>,
    frame_size: (u32, u32),
    target_views: Vec<ID3D11RenderTargetView>,
    source_views: Vec<(isize, ID3D11ShaderResourceView)>,
    textures: Vec<ID3D11Texture2D>,
    texture_slots: TextureSlotPool,
    texture_releases: Receiver<usize>,
    release_callbacks: Vec<IMFAsyncCallback>,
    crop: D3D11_BOX,
    temporary_path: PathBuf,
    output_path: PathBuf,
    mf_started: bool,
    committed: bool,
}

struct TextureSlotPool {
    available: std::collections::VecDeque<usize>,
    in_flight: Vec<bool>,
}

impl TextureSlotPool {
    fn new(count: usize) -> Self {
        Self {
            available: (0..count).collect(),
            in_flight: vec![false; count],
        }
    }

    fn acquire(&mut self) -> Option<usize> {
        let slot = self.available.pop_front()?;
        self.in_flight[slot] = true;
        Some(slot)
    }

    fn release(&mut self, slot: usize) -> bool {
        let Some(in_flight) = self.in_flight.get_mut(slot) else {
            return false;
        };
        if !*in_flight {
            return false;
        }
        *in_flight = false;
        self.available.push_back(slot);
        true
    }
}

#[implement(IMFAsyncCallback)]
struct TextureReleaseCallback {
    slot: usize,
    releases: Sender<usize>,
}

#[allow(non_snake_case)]
impl IMFAsyncCallback_Impl for TextureReleaseCallback_Impl {
    fn GetParameters(&self, _flags: *mut u32, _queue: *mut u32) -> windows::core::Result<()> {
        Err(windows::core::Error::from_hresult(
            windows::Win32::Foundation::E_NOTIMPL,
        ))
    }

    fn Invoke(&self, _result: windows::core::Ref<'_, IMFAsyncResult>) -> windows::core::Result<()> {
        let _ = self.releases.send(self.slot);
        Ok(())
    }
}

impl MediaFoundationEncoder {
    fn new(
        output_path: &Path,
        width: u32,
        height: u32,
        frame_rate: u32,
        device: ID3D11Device,
        context: ID3D11DeviceContext,
        crop: D3D11_BOX,
        white_scale: Option<f32>,
        fitted: bool,
    ) -> Result<Self, RecorderError> {
        unsafe {
            MFStartup(MF_VERSION, MFSTARTUP_FULL).map_err(|error| {
                RecorderError::capture(format!("Failed to start Media Foundation: {error}"))
            })?;
        }

        let temporary_path = match temporary_video_path(output_path) {
            Ok(path) => path,
            Err(error) => {
                unsafe {
                    let _ = MFShutdown();
                }
                return Err(error);
            }
        };
        if temporary_path.exists() {
            std::fs::remove_file(&temporary_path).map_err(|error| {
                RecorderError::capture(format!("Failed to replace temporary video: {error}"))
            })?;
        }

        let result = Self::create(
            output_path,
            temporary_path.clone(),
            width,
            height,
            frame_rate,
            device,
            context,
            crop,
            white_scale,
            fitted,
        );
        if result.is_err() {
            unsafe {
                let _ = MFShutdown();
            }
            let _ = std::fs::remove_file(temporary_path);
        }
        result
    }

    fn create(
        output_path: &Path,
        temporary_path: PathBuf,
        width: u32,
        height: u32,
        frame_rate: u32,
        device: ID3D11Device,
        context: ID3D11DeviceContext,
        crop: D3D11_BOX,
        white_scale: Option<f32>,
        fitted: bool,
    ) -> Result<Self, RecorderError> {
        let mut manager = None;
        let mut reset_token = 0;
        unsafe {
            MFCreateDXGIDeviceManager(&mut reset_token, &mut manager).map_err(|error| {
                RecorderError::capture(format!(
                    "Failed to create Media Foundation device manager: {error}"
                ))
            })?;
        }
        let manager: IMFDXGIDeviceManager = manager.ok_or_else(|| {
            RecorderError::capture("Media Foundation device manager was not created")
        })?;
        unsafe {
            manager.ResetDevice(&device, reset_token).map_err(|error| {
                RecorderError::capture(format!("Failed to attach D3D11 recording device: {error}"))
            })?;
        }

        let attributes = create_attributes(2)?;
        unsafe {
            attributes
                .SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 1)
                .map_err(mf_attribute_error)?;
            attributes
                .SetUnknown(&MF_SINK_WRITER_D3D_MANAGER, &manager)
                .map_err(mf_attribute_error)?;
        }

        let wide_path: Vec<u16> = temporary_path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let writer = unsafe {
            MFCreateSinkWriterFromURL(
                PCWSTR(wide_path.as_ptr()),
                None::<&IMFByteStream>,
                Some(&attributes),
            )
        }
        .map_err(|error| {
            RecorderError::capture(format!("Failed to create H.264 sink writer: {error}"))
        })?;

        let output_type = create_video_type(
            MFVideoFormat_H264,
            width,
            height,
            frame_rate,
            Some(video_bitrate(width, height, frame_rate)),
        )?;
        unsafe {
            output_type
                .SetUINT32(&MF_MT_MPEG2_PROFILE, 100)
                .map_err(mf_attribute_error)?;
        }
        let stream_index = unsafe { writer.AddStream(&output_type) }.map_err(|error| {
            RecorderError::capture(format!("Failed to add H.264 stream: {error}"))
        })?;

        let input_type = create_video_type(MFVideoFormat_ARGB32, width, height, frame_rate, None)?;
        unsafe {
            input_type
                .SetUINT32(&MF_MT_FIXED_SIZE_SAMPLES, 1)
                .map_err(mf_attribute_error)?;
            input_type
                .SetUINT32(&MF_MT_ALL_SAMPLES_INDEPENDENT, 1)
                .map_err(mf_attribute_error)?;
            input_type
                .SetUINT32(
                    &MF_MT_SAMPLE_SIZE,
                    width.saturating_mul(height).saturating_mul(4),
                )
                .map_err(mf_attribute_error)?;
            writer
                .SetInputMediaType(stream_index, &input_type, None::<&IMFAttributes>)
                .map_err(|error| {
                    RecorderError::capture(format!("Failed to configure BGRA video input: {error}"))
                })?;
            writer.BeginWriting().map_err(|error| {
                RecorderError::capture(format!("Failed to begin H.264 encoding: {error}"))
            })?;
        }

        let descriptor = D3D11_TEXTURE2D_DESC {
            Width: width,
            Height: height,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: (D3D11_BIND_RENDER_TARGET | D3D11_BIND_SHADER_RESOURCE).0 as u32,
            CPUAccessFlags: 0,
            MiscFlags: 0,
        };
        let mut textures = Vec::with_capacity(ENCODER_TEXTURE_COUNT);
        for _ in 0..ENCODER_TEXTURE_COUNT {
            let mut texture = None;
            unsafe {
                device
                    .CreateTexture2D(&descriptor, None, Some(&mut texture))
                    .map_err(|error| {
                        RecorderError::capture(format!("Failed to create encoder texture: {error}"))
                    })?;
            }
            textures.push(
                texture.ok_or_else(|| RecorderError::capture("Encoder texture was not created"))?,
            );
        }
        let (release_sender, texture_releases) = std::sync::mpsc::channel();
        let release_callbacks = (0..ENCODER_TEXTURE_COUNT)
            .map(|slot| {
                let callback: IMFAsyncCallback = TextureReleaseCallback {
                    slot,
                    releases: release_sender.clone(),
                }
                .into();
                callback
            })
            .collect();

        let fit = match fitted {
            true => Some(FitStage::new(&device, white_scale).map_err(RecorderError::capture)?),
            false => None,
        };
        let tone_map = match (fitted, white_scale) {
            (false, Some(scale)) => {
                Some(ToneMapStage::new(&device, scale).map_err(RecorderError::capture)?)
            }
            _ => None,
        };
        let mut target_views = Vec::new();
        if tone_map.is_some() || fit.is_some() {
            for texture in &textures {
                target_views.push(target_view(&device, texture).map_err(RecorderError::capture)?);
            }
        }

        Ok(Self {
            writer: Some(writer),
            stream_index,
            device,
            context,
            tone_map,
            fit,
            frame_size: (width, height),
            target_views,
            source_views: Vec::new(),
            textures,
            texture_slots: TextureSlotPool::new(ENCODER_TEXTURE_COUNT),
            texture_releases,
            release_callbacks,
            crop,
            temporary_path,
            output_path: output_path.to_path_buf(),
            mf_started: true,
            committed: false,
        })
    }

    fn forget_source_views(&mut self) {
        self.source_views.clear();
    }

    fn cached_source_view(
        &mut self,
        source: &ID3D11Texture2D,
    ) -> Result<ID3D11ShaderResourceView, RecorderError> {
        let key = source.as_raw() as isize;
        if let Some((_, view)) = self.source_views.iter().find(|(cached, _)| *cached == key) {
            return Ok(view.clone());
        }

        let view = source_view(&self.device, source).map_err(RecorderError::capture)?;
        self.source_views.push((key, view.clone()));
        Ok(view)
    }

    fn shaded_target_view(&self, slot: usize) -> Result<ID3D11RenderTargetView, RecorderError> {
        self.target_views
            .get(slot)
            .cloned()
            .ok_or_else(|| RecorderError::capture("Encoder texture view is missing"))
    }

    fn tone_map_frame(
        &mut self,
        source: &ID3D11Texture2D,
        slot: usize,
    ) -> Result<(), RecorderError> {
        let view = self.cached_source_view(source)?;
        let target = self.shaded_target_view(slot)?;
        let stage = self
            .tone_map
            .as_ref()
            .ok_or_else(|| RecorderError::capture("Tone mapping is not enabled"))?;

        stage
            .run(
                &self.context,
                &view,
                &target,
                (self.crop.left, self.crop.top),
                (
                    self.crop.right - self.crop.left,
                    self.crop.bottom - self.crop.top,
                ),
            )
            .map_err(RecorderError::capture)
    }

    fn fit_frame(
        &mut self,
        source: &ID3D11Texture2D,
        content: SizeInt32,
        slot: usize,
    ) -> Result<(), RecorderError> {
        let mut descriptor = D3D11_TEXTURE2D_DESC::default();
        unsafe { source.GetDesc(&mut descriptor) };
        let visible = (
            (content.Width.max(0) as u32).min(descriptor.Width),
            (content.Height.max(0) as u32).min(descriptor.Height),
        );

        let view = self.cached_source_view(source)?;
        let target = self.shaded_target_view(slot)?;
        let stage = self
            .fit
            .as_ref()
            .ok_or_else(|| RecorderError::capture("Window scaling is not enabled"))?;

        stage
            .run(
                &self.context,
                &view,
                &target,
                visible,
                (descriptor.Width, descriptor.Height),
                self.frame_size,
            )
            .map_err(RecorderError::capture)
    }

    fn write(
        &mut self,
        frame: &Direct3D11CaptureFrame,
        content: SizeInt32,
        timestamp: i64,
        duration: i64,
    ) -> Result<(), RecorderError> {
        if self.fit.is_none()
            && (content.Width < self.crop.right as i32 || content.Height < self.crop.bottom as i32)
        {
            return Err(RecorderError::capture(
                "Captured display size changed during recording",
            ));
        }

        let slot = self.acquire_texture()?;
        let texture = self.textures[slot].clone();
        let surface = frame.Surface().map_err(|error| {
            RecorderError::capture(format!("Failed to access captured surface: {error}"))
        })?;
        let access: IDirect3DDxgiInterfaceAccess = surface.cast().map_err(|error| {
            RecorderError::capture(format!("Failed to access captured DXGI surface: {error}"))
        })?;
        let source: ID3D11Texture2D = unsafe { access.GetInterface() }.map_err(|error| {
            RecorderError::capture(format!("Failed to access captured D3D11 texture: {error}"))
        })?;
        if self.fit.is_some() {
            self.fit_frame(&source, content, slot)?;
        } else if self.tone_map.is_some() {
            self.tone_map_frame(&source, slot)?;
        } else {
            let source_resource: ID3D11Resource = source.cast().map_err(|error| {
                RecorderError::capture(format!(
                    "Failed to access captured texture resource: {error}"
                ))
            })?;
            let target_resource: ID3D11Resource = texture.cast().map_err(|error| {
                RecorderError::capture(format!(
                    "Failed to access encoder texture resource: {error}"
                ))
            })?;
            unsafe {
                self.context.CopySubresourceRegion(
                    &target_resource,
                    0,
                    0,
                    0,
                    0,
                    &source_resource,
                    0,
                    Some(&self.crop),
                );
            }
        }

        let buffer =
            unsafe { MFCreateDXGISurfaceBuffer(&ID3D11Texture2D::IID, &texture, 0, false) }
                .map_err(|error| {
                    RecorderError::capture(format!("Failed to wrap encoder texture: {error}"))
                })?;
        let plane: IMF2DBuffer = buffer.cast().map_err(|error| {
            RecorderError::capture(format!("Failed to access encoder texture planes: {error}"))
        })?;
        let length = unsafe { plane.GetContiguousLength() }.map_err(|error| {
            RecorderError::capture(format!("Failed to measure the encoder texture: {error}"))
        })?;
        unsafe {
            buffer.SetCurrentLength(length).map_err(|error| {
                RecorderError::capture(format!("Failed to size the encoder buffer: {error}"))
            })?;
        }
        let tracked = unsafe { MFCreateTrackedSample() }.map_err(|error| {
            RecorderError::capture(format!("Failed to create tracked video sample: {error}"))
        })?;
        unsafe {
            tracked
                .SetAllocator(
                    &self.release_callbacks[slot],
                    None::<&windows::core::IUnknown>,
                )
                .map_err(|error| {
                    RecorderError::capture(format!(
                        "Failed to track encoder texture ownership: {error}"
                    ))
                })?;
        }
        let sample: IMFSample = tracked.cast().map_err(|error| {
            RecorderError::capture(format!("Failed to access tracked video sample: {error}"))
        })?;
        unsafe {
            sample.AddBuffer(&buffer).map_err(|error| {
                RecorderError::capture(format!("Failed to attach video buffer: {error}"))
            })?;
            sample.SetSampleTime(timestamp).map_err(|error| {
                RecorderError::capture(format!("Failed to set video timestamp: {error}"))
            })?;
            sample.SetSampleDuration(duration).map_err(|error| {
                RecorderError::capture(format!("Failed to set frame duration: {error}"))
            })?;
            self.writer
                .as_ref()
                .ok_or_else(|| RecorderError::capture("H.264 sink writer is unavailable"))?
                .WriteSample(self.stream_index, &sample)
                .map_err(|error| {
                    RecorderError::capture(format!("Failed to encode video frame: {error}"))
                })?;
        }
        Ok(())
    }

    fn acquire_texture(&mut self) -> Result<usize, RecorderError> {
        loop {
            while let Ok(slot) = self.texture_releases.try_recv() {
                self.texture_slots.release(slot);
            }
            if let Some(slot) = self.texture_slots.acquire() {
                return Ok(slot);
            }
            let slot = self
                .texture_releases
                .recv_timeout(ENCODER_BACKPRESSURE_TIMEOUT)
                .map_err(|_| {
                    RecorderError::capture(
                        "Timed out waiting for Media Foundation to release an encoder texture",
                    )
                })?;
            self.texture_slots.release(slot);
        }
    }

    fn finish(mut self) -> Result<StagedAsset, RecorderError> {
        let writer = self
            .writer
            .take()
            .ok_or_else(|| RecorderError::stop("H.264 sink writer is unavailable"))?;
        unsafe {
            writer.Finalize().map_err(|error| {
                RecorderError::stop(format!("Failed to finalize H.264 recording: {error}"))
            })?;
        }
        drop(writer);

        self.committed = true;
        self.shutdown_media_foundation();
        Ok(StagedAsset {
            temporary_path: self.temporary_path.clone(),
            output_path: self.output_path.clone(),
        })
    }

    fn abort(mut self) {
        self.writer.take();
        self.shutdown_media_foundation();
        let _ = std::fs::remove_file(&self.temporary_path);
    }

    fn shutdown_media_foundation(&mut self) {
        if !self.mf_started {
            return;
        }
        self.release_callbacks.clear();
        self.textures.clear();
        unsafe {
            let _ = MFShutdown();
        }
        self.mf_started = false;
    }
}

impl Drop for MediaFoundationEncoder {
    fn drop(&mut self) {
        self.writer.take();
        self.shutdown_media_foundation();
        if !self.committed {
            let _ = std::fs::remove_file(&self.temporary_path);
        }
    }
}

fn create_attributes(capacity: u32) -> Result<IMFAttributes, RecorderError> {
    let mut attributes = None;
    unsafe {
        MFCreateAttributes(&mut attributes, capacity).map_err(|error| {
            RecorderError::capture(format!(
                "Failed to create Media Foundation attributes: {error}"
            ))
        })?;
    }
    attributes.ok_or_else(|| RecorderError::capture("Media Foundation attributes were not created"))
}

fn create_video_type(
    subtype: windows::core::GUID,
    width: u32,
    height: u32,
    frame_rate: u32,
    bitrate: Option<u32>,
) -> Result<windows::Win32::Media::MediaFoundation::IMFMediaType, RecorderError> {
    let media_type = unsafe { MFCreateMediaType() }.map_err(|error| {
        RecorderError::capture(format!("Failed to create video media type: {error}"))
    })?;
    unsafe {
        media_type
            .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)
            .map_err(mf_attribute_error)?;
        media_type
            .SetGUID(&MF_MT_SUBTYPE, &subtype)
            .map_err(mf_attribute_error)?;
        media_type
            .SetUINT64(&MF_MT_FRAME_SIZE, pack_ratio(width, height))
            .map_err(mf_attribute_error)?;
        media_type
            .SetUINT64(&MF_MT_FRAME_RATE, pack_ratio(frame_rate, 1))
            .map_err(mf_attribute_error)?;
        media_type
            .SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, pack_ratio(1, 1))
            .map_err(mf_attribute_error)?;
        media_type
            .SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)
            .map_err(mf_attribute_error)?;
        if let Some(bitrate) = bitrate {
            media_type
                .SetUINT32(&MF_MT_AVG_BITRATE, bitrate)
                .map_err(mf_attribute_error)?;
        }
    }
    Ok(media_type)
}

fn mf_attribute_error(error: windows::core::Error) -> RecorderError {
    RecorderError::capture(format!(
        "Failed to configure Media Foundation media type: {error}"
    ))
}

fn pack_ratio(numerator: u32, denominator: u32) -> u64 {
    (u64::from(numerator) << 32) | u64::from(denominator)
}

fn video_bitrate(width: u32, height: u32, frame_rate: u32) -> u32 {
    let raw = u64::from(width)
        .saturating_mul(u64::from(height))
        .saturating_mul(u64::from(frame_rate))
        .saturating_mul(12);
    raw.clamp(50_000_000, 200_000_000) as u32
}

fn temporary_video_path(output_path: &Path) -> Result<PathBuf, RecorderError> {
    let name = output_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| RecorderError::configuration("outputPath has an invalid file name"))?;
    Ok(output_path.with_file_name(format!(".{name}.capty-partial.mp4")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captures_float_pixels_only_on_hdr_displays() {
        assert_eq!(
            capture_pixel_format(Some(2.5)),
            DirectXPixelFormat::R16G16B16A16Float
        );
        assert_eq!(
            capture_pixel_format(None),
            DirectXPixelFormat::B8G8R8A8UIntNormalized
        );
    }

    #[test]
    fn video_timeline_starts_at_zero_on_first_frame() {
        let mut timeline = VideoTimeline::new(10);

        assert_eq!(timeline.timestamp_for(50_000_000), Some(0));
        timeline.commit(0);
        assert_eq!(timeline.duration(), 0.1);
    }

    #[test]
    fn video_timeline_excludes_paused_source_time() {
        let mut timeline = VideoTimeline::new(10);
        let first = timeline.timestamp_for(10_000_000);
        assert_eq!(first, Some(0));
        timeline.commit(0);
        let second = timeline.timestamp_for(12_000_000);
        assert_eq!(second, Some(2_000_000));
        timeline.commit(2_000_000);

        timeline.pause();
        timeline.observe(42_000_000);
        assert!(timeline.is_resuming());
        assert_eq!(timeline.timestamp_for(52_000_000), None);
        assert!(!timeline.is_resuming());
        assert_eq!(timeline.timestamp_for(53_000_000), Some(3_000_000));
    }

    #[test]
    fn rejected_system_timestamp_becomes_capture_error() {
        let result = system_relative_time(Err(windows::core::Error::from_hresult(
            windows::Win32::Foundation::E_FAIL,
        )));
        let Err(error) = result else {
            panic!("failed frame timestamp should be rejected");
        };

        assert_eq!(error.code, "CAPTURE_ERROR");
        assert!(error.message.contains("frame timestamp"));
    }

    #[test]
    fn texture_pool_never_reuses_an_in_flight_slot() {
        let mut pool = TextureSlotPool::new(2);
        assert_eq!(pool.acquire(), Some(0));
        assert_eq!(pool.acquire(), Some(1));
        assert_eq!(pool.acquire(), None);
        assert!(pool.release(0));
        assert!(!pool.release(0));
        assert_eq!(pool.acquire(), Some(0));
    }

    #[test]
    fn transaction_rolls_back_published_assets_in_reverse_order() {
        assert_eq!(rollback_order(4).collect::<Vec<_>>(), vec![3, 2, 1, 0]);
    }

    #[test]
    fn post_start_failure_is_latched_once() {
        let (sender, receiver) = std::sync::mpsc::channel();
        let mut start_sender = None;
        let mut failure_sender = Some(sender);

        report_worker_failure(
            RecorderError::capture("first"),
            &mut start_sender,
            &mut failure_sender,
        );
        report_worker_failure(
            RecorderError::capture("second"),
            &mut start_sender,
            &mut failure_sender,
        );

        let failures = receiver.try_iter().collect::<Vec<_>>();
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].message, "first");
    }

    #[test]
    fn pre_start_worker_panic_reports_once_and_restores_idle() {
        let shared = Arc::new(Mutex::new(ControllerState {
            phase: ControllerPhase::Starting,
            status: RecorderStatus::idle(),
            command_sender: None,
            mic_muted: false,
            generation: 7,
        }));
        let poisoned = shared.clone();
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _state = poisoned.lock().unwrap_or_else(|error| error.into_inner());
            panic!("worker panic while holding controller state");
        }));
        assert!(shared.is_poisoned());
        let (start_sender, start_receiver) = std::sync::mpsc::channel();
        let (failure_sender, failure_receiver) = std::sync::mpsc::channel();

        recover_worker_panic(&shared, 7, false, &start_sender, &failure_sender);
        recover_worker_panic(&shared, 7, false, &start_sender, &failure_sender);

        let reports = start_receiver.try_iter().collect::<Vec<_>>();
        assert_eq!(reports.len(), 1);
        let Err(error) = &reports[0] else {
            panic!("pre-start panic should report a start failure");
        };
        assert_eq!(error.code, "START_FAILED");
        assert!(failure_receiver.try_recv().is_err());
        assert!(!shared.is_poisoned());
        let state = shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        assert!(state.phase == ControllerPhase::Idle);
        assert_eq!(state.status.state, RecorderState::Idle);
    }

    #[test]
    fn post_start_worker_panic_reports_once_and_restores_idle() {
        let shared = Arc::new(Mutex::new(ControllerState {
            phase: ControllerPhase::Running,
            status: RecorderStatus {
                state: RecorderState::Recording,
                duration: 1.0,
            },
            command_sender: None,
            mic_muted: false,
            generation: 8,
        }));
        let (start_sender, start_receiver) = std::sync::mpsc::channel();
        let (failure_sender, failure_receiver) = std::sync::mpsc::channel();

        recover_worker_panic(&shared, 8, true, &start_sender, &failure_sender);
        recover_worker_panic(&shared, 8, true, &start_sender, &failure_sender);

        let reports = failure_receiver.try_iter().collect::<Vec<_>>();
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].code, "CAPTURE_ERROR");
        assert!(start_receiver.try_recv().is_err());
        let state = shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        assert!(state.phase == ControllerPhase::Idle);
        assert_eq!(state.status.state, RecorderState::Idle);
    }
}

#[cfg(test)]
mod capture_surface_probe {
    use super::*;
    use std::time::Instant;

    #[test]
    fn captured_frames_can_be_bound_as_shader_resources() {
        if retain_process_mta().is_err() {
            eprintln!("skipping: could not join the process MTA");
            return;
        }
        if !GraphicsCaptureSession::IsSupported().unwrap_or(false) {
            eprintln!("skipping: Windows Graphics Capture is unavailable");
            return;
        }
        let Ok((device, _context, winrt_device)) = create_d3d_device() else {
            eprintln!("skipping: no D3D11 device available");
            return;
        };
        let Ok(monitors) = enumerate_monitors() else {
            eprintln!("skipping: no displays enumerated");
            return;
        };
        let Some(monitor) = monitors.into_iter().find(|monitor| monitor.primary) else {
            eprintln!("skipping: no primary display");
            return;
        };

        let Ok(interop) = factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>() else {
            eprintln!("skipping: no capture interop");
            return;
        };
        let handle = HMONITOR(monitor.handle as *mut c_void);
        let Ok(item) = (unsafe { interop.CreateForMonitor::<GraphicsCaptureItem>(handle) }) else {
            eprintln!("skipping: could not open the primary display for capture");
            return;
        };
        let Ok(size) = item.Size() else {
            eprintln!("skipping: could not read the capture size");
            return;
        };

        for format in [
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            DirectXPixelFormat::R16G16B16A16Float,
        ] {
            let Ok(pool) =
                Direct3D11CaptureFramePool::CreateFreeThreaded(&winrt_device, format, 2, size)
            else {
                eprintln!("skipping {format:?}: frame pool was refused");
                continue;
            };
            let Ok(session) = pool.CreateCaptureSession(&item) else {
                eprintln!("skipping {format:?}: capture session was refused");
                continue;
            };
            let _ = session.SetIsCursorCaptureEnabled(false);
            let _ = session.SetIsBorderRequired(false);
            if session.StartCapture().is_err() {
                eprintln!("skipping {format:?}: capture did not start");
                continue;
            }

            let started = Instant::now();
            let mut checked = false;
            while started.elapsed() < Duration::from_secs(10) {
                let Ok(frame) = pool.TryGetNextFrame() else {
                    continue;
                };
                let surface = frame.Surface().expect("captured surface");
                let access: IDirect3DDxgiInterfaceAccess =
                    surface.cast().expect("captured dxgi surface");
                let texture: ID3D11Texture2D =
                    unsafe { access.GetInterface() }.expect("captured texture");

                let mut descriptor = D3D11_TEXTURE2D_DESC::default();
                unsafe { texture.GetDesc(&mut descriptor) };
                let bind = descriptor.BindFlags;

                assert!(
                    bind & D3D11_BIND_SHADER_RESOURCE.0 as u32 != 0,
                    "{format:?} capture texture lacks BIND_SHADER_RESOURCE (BindFlags {bind:#x})"
                );

                let mut view = None;
                unsafe {
                    device
                        .CreateShaderResourceView(&texture, None, Some(&mut view))
                        .unwrap_or_else(|error| {
                            panic!("{format:?} capture texture rejected an SRV: {error}")
                        });
                }
                assert!(
                    view.is_some(),
                    "{format:?} produced no shader resource view"
                );
                checked = true;
                break;
            }

            let _ = session.Close();
            let _ = pool.Close();
            if !checked {
                eprintln!("skipping {format:?}: no frame arrived within 10s");
            }
        }
    }
}
