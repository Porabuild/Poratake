use super::recorder_types::{RecorderError, StagedAsset};
use serde::Serialize;
use std::collections::VecDeque;
use std::fs::OpenOptions;
use std::io::{BufWriter, Write};
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use windows::core::{implement, PCWSTR};
use windows::Win32::Media::MediaFoundation::{
    IMFActivate, IMFAttributes, IMFByteStream, IMFMediaEvent, IMFMediaSource, IMFMediaType,
    IMFSample, IMFSinkWriter, IMFSourceReader, IMFSourceReaderCallback,
    IMFSourceReaderCallback_Impl, MFCreateAttributes, MFCreateMediaType, MFCreateMemoryBuffer,
    MFCreateSample, MFCreateSinkWriterFromURL, MFCreateSourceReaderFromMediaSource,
    MFEnumDeviceSources, MFMediaType_Video, MFShutdown, MFStartup, MFVideoFormat_H264,
    MFVideoFormat_RGB32, MFVideoInterlace_Progressive, MFSTARTUP_FULL,
    MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME, MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE,
    MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID,
    MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK, MF_MT_ALL_SAMPLES_INDEPENDENT,
    MF_MT_AVG_BITRATE, MF_MT_FIXED_SIZE_SAMPLES, MF_MT_FRAME_RATE, MF_MT_FRAME_SIZE,
    MF_MT_INTERLACE_MODE, MF_MT_MAJOR_TYPE, MF_MT_MPEG2_PROFILE, MF_MT_PIXEL_ASPECT_RATIO,
    MF_MT_SAMPLE_SIZE, MF_MT_SUBTYPE, MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS,
    MF_SINK_WRITER_DISABLE_THROTTLING, MF_SOURCE_READERF_CURRENTMEDIATYPECHANGED,
    MF_SOURCE_READERF_ENDOFSTREAM, MF_SOURCE_READER_ASYNC_CALLBACK,
    MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, MF_SOURCE_READER_FIRST_VIDEO_STREAM, MF_VERSION,
};
use windows::Win32::System::Com::{
    CoInitializeEx, CoTaskMemFree, CoUninitialize, COINIT_MULTITHREADED,
};

const CAMERA_VIDEO_NAME: &str = "camera.mov";
const CAMERA_METADATA_NAME: &str = "camera.json";
const TARGET_WIDTH: u32 = 1280;
const TARGET_HEIGHT: u32 = 720;
const MAX_PENDING_FRAMES: usize = 8;
const START_TIMEOUT: Duration = Duration::from_secs(30);
const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);
const FLUSH_TIMEOUT: Duration = Duration::from_secs(5);
const HNS_PER_SECOND: f64 = 10_000_000.0;

#[derive(Clone, Debug)]
pub struct CameraRecordingConfig {
    pub project_dir: PathBuf,
    pub device_id: Option<String>,
    pub device_name: Option<String>,
    pub frame_rate: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct CameraSyncClock {
    pub monotonic_time: Instant,
    pub wall_time: SystemTime,
}

#[derive(Clone, Debug)]
pub struct CameraRecordingResult {
    pub video_path: PathBuf,
    pub metadata_path: PathBuf,
    pub duration: f64,
    pub staged_assets: Vec<StagedAsset>,
}

struct CameraSession {
    events: Sender<WorkerEvent>,
    health: Receiver<RecorderError>,
    thread: JoinHandle<()>,
}

pub struct RecordingCamera {
    session: Option<CameraSession>,
}

impl RecordingCamera {
    pub fn new() -> Self {
        Self { session: None }
    }

    pub fn start(&mut self, config: CameraRecordingConfig) -> Result<(), RecorderError> {
        if self.session.is_some() {
            return Err(RecorderError::invalid_state(
                "Cannot start camera: camera is already recording",
            ));
        }

        validate_config(&config)?;
        let (events, worker_events) = std::sync::mpsc::channel();
        let (ready_sender, ready_receiver) = std::sync::mpsc::channel();
        let (health_sender, health_receiver) = std::sync::mpsc::channel();
        let callback_events = events.clone();
        let thread = std::thread::spawn(move || {
            run_worker(
                config,
                worker_events,
                callback_events,
                ready_sender,
                health_sender,
            )
        });
        let ready = ready_receiver.recv();

        match ready {
            Ok(Ok(())) => {
                self.session = Some(CameraSession {
                    events,
                    health: health_receiver,
                    thread,
                });
                Ok(())
            }
            Ok(Err(error)) => {
                let _ = thread.join();
                Err(error)
            }
            Err(_) => {
                let _ = thread.join();
                Err(RecorderError::start(
                    "Camera worker stopped before the first frame",
                ))
            }
        }
    }

    pub fn sync_with_screen_start(&self, clock: CameraSyncClock) -> Result<(), RecorderError> {
        let session = self
            .session
            .as_ref()
            .ok_or_else(|| RecorderError::invalid_state("Camera is not recording"))?;
        let (response_sender, response_receiver) = std::sync::mpsc::channel();
        session
            .events
            .send(WorkerEvent::Command(CameraCommand::Sync(
                clock,
                response_sender,
            )))
            .map_err(|_| RecorderError::capture("Camera worker is unavailable"))?;
        response_receiver
            .recv_timeout(COMMAND_TIMEOUT)
            .map_err(|_| RecorderError::capture("Camera did not synchronize in time"))?
    }

    pub fn pause(&self) -> Result<(), RecorderError> {
        let session = self
            .session
            .as_ref()
            .ok_or_else(|| RecorderError::invalid_state("Camera is not recording"))?;
        let (response_sender, response_receiver) = std::sync::mpsc::channel();
        session
            .events
            .send(WorkerEvent::Command(CameraCommand::Pause(response_sender)))
            .map_err(|_| RecorderError::capture("Camera worker is unavailable"))?;
        response_receiver
            .recv_timeout(COMMAND_TIMEOUT)
            .map_err(|_| RecorderError::capture("Camera did not pause in time"))?
    }

    pub fn resume(&self) -> Result<(), RecorderError> {
        let session = self
            .session
            .as_ref()
            .ok_or_else(|| RecorderError::invalid_state("Camera is not recording"))?;
        let (response_sender, response_receiver) = std::sync::mpsc::channel();
        session
            .events
            .send(WorkerEvent::Command(CameraCommand::Resume(response_sender)))
            .map_err(|_| RecorderError::capture("Camera worker is unavailable"))?;
        response_receiver
            .recv_timeout(COMMAND_TIMEOUT)
            .map_err(|_| RecorderError::capture("Camera did not resume in time"))?
    }

    pub fn try_error(&self) -> Option<RecorderError> {
        let session = self.session.as_ref()?;
        match session.health.try_recv() {
            Ok(error) => Some(error),
            Err(std::sync::mpsc::TryRecvError::Empty) => None,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                Some(RecorderError::capture("Camera health channel disconnected"))
            }
        }
    }

    pub fn stop(&mut self) -> Result<CameraRecordingResult, RecorderError> {
        let session = self
            .session
            .take()
            .ok_or_else(|| RecorderError::invalid_state("Camera is not recording"))?;
        let (response_sender, response_receiver) = std::sync::mpsc::channel();
        if session
            .events
            .send(WorkerEvent::Command(CameraCommand::Stop(response_sender)))
            .is_err()
        {
            let _ = session.thread.join();
            return Err(RecorderError::stop("Camera worker is unavailable"));
        }
        let result = response_receiver
            .recv()
            .map_err(|_| RecorderError::stop("Camera worker stopped before finalizing"));
        let joined = session.thread.join();
        if joined.is_err() {
            return Err(RecorderError::stop("Camera worker terminated unexpectedly"));
        }
        result?
    }

    pub fn abort(&mut self) {
        let Some(session) = self.session.take() else {
            return;
        };
        let (done_sender, done_receiver) = std::sync::mpsc::channel();
        if session
            .events
            .send(WorkerEvent::Command(CameraCommand::Abort(done_sender)))
            .is_ok()
        {
            let _ = done_receiver.recv();
        }
        let _ = session.thread.join();
    }
}

impl Default for RecordingCamera {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for RecordingCamera {
    fn drop(&mut self) {
        self.abort();
    }
}

enum CameraCommand {
    Sync(CameraSyncClock, Sender<Result<(), RecorderError>>),
    Pause(Sender<Result<(), RecorderError>>),
    Resume(Sender<Result<(), RecorderError>>),
    Stop(Sender<Result<CameraRecordingResult, RecorderError>>),
    Abort(Sender<()>),
}

enum WorkerEvent {
    Command(CameraCommand),
    Frame(CameraFrame),
    CaptureError(String),
    EndOfStream,
    Flushed,
}

struct CameraFrame {
    source_time: i64,
    arrived_at: Instant,
    bytes: Vec<u8>,
}

#[derive(Clone, Copy)]
struct CameraCapturePoint {
    source_time: i64,
    arrived_at: Instant,
}

#[implement(IMFSourceReaderCallback)]
struct SourceReaderCallback {
    events: Sender<WorkerEvent>,
}

#[allow(non_snake_case)]
impl IMFSourceReaderCallback_Impl for SourceReaderCallback_Impl {
    fn OnReadSample(
        &self,
        status: windows::core::HRESULT,
        _stream_index: u32,
        stream_flags: u32,
        timestamp: i64,
        sample: windows::core::Ref<'_, IMFSample>,
    ) -> windows::core::Result<()> {
        if status.is_err() {
            let error = windows::core::Error::from(status);
            let _ = self.events.send(WorkerEvent::CaptureError(format!(
                "Camera capture failed: {error}"
            )));
            return Ok(());
        }
        if stream_flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
            let _ = self.events.send(WorkerEvent::EndOfStream);
            return Ok(());
        }
        if stream_flags & MF_SOURCE_READERF_CURRENTMEDIATYPECHANGED.0 as u32 != 0 {
            let _ = self.events.send(WorkerEvent::CaptureError(
                "Camera format changed during recording".to_string(),
            ));
            return Ok(());
        }
        let Some(sample) = sample.as_ref() else {
            let _ = self.events.send(WorkerEvent::Frame(CameraFrame {
                source_time: timestamp,
                arrived_at: Instant::now(),
                bytes: Vec::new(),
            }));
            return Ok(());
        };
        let arrived_at = Instant::now();
        match copy_sample(sample, timestamp, arrived_at) {
            Ok(frame) => {
                let _ = self.events.send(WorkerEvent::Frame(frame));
            }
            Err(error) => {
                let _ = self.events.send(WorkerEvent::CaptureError(error));
            }
        }
        Ok(())
    }

    fn OnFlush(&self, _stream_index: u32) -> windows::core::Result<()> {
        let _ = self.events.send(WorkerEvent::Flushed);
        Ok(())
    }

    fn OnEvent(
        &self,
        _stream_index: u32,
        _event: windows::core::Ref<'_, IMFMediaEvent>,
    ) -> windows::core::Result<()> {
        Ok(())
    }
}

fn copy_sample(
    sample: &IMFSample,
    source_time: i64,
    arrived_at: Instant,
) -> Result<CameraFrame, String> {
    let buffer =
        unsafe { sample.ConvertToContiguousBuffer() }.map_err(|error| error.to_string())?;
    let mut pointer = std::ptr::null_mut();
    let mut length = 0;
    unsafe { buffer.Lock(&mut pointer, None, Some(&mut length)) }
        .map_err(|error| error.to_string())?;
    if pointer.is_null() || length == 0 {
        let _ = unsafe { buffer.Unlock() };
        return Ok(CameraFrame {
            source_time,
            arrived_at,
            bytes: Vec::new(),
        });
    }
    let bytes = unsafe { std::slice::from_raw_parts(pointer, length as usize) }.to_vec();
    unsafe { buffer.Unlock() }.map_err(|error| error.to_string())?;
    Ok(CameraFrame {
        source_time,
        arrived_at,
        bytes,
    })
}

fn run_worker(
    config: CameraRecordingConfig,
    events: Receiver<WorkerEvent>,
    callback_events: Sender<WorkerEvent>,
    ready: Sender<Result<(), RecorderError>>,
    health: Sender<RecorderError>,
) {
    let apartment = match CameraApartment::initialize() {
        Ok(apartment) => apartment,
        Err(error) => {
            let _ = ready.send(Err(error));
            return;
        }
    };
    let mut runtime = match CameraRuntime::prepare(config, callback_events) {
        Ok(runtime) => runtime,
        Err(error) => {
            let _ = ready.send(Err(error));
            drop(apartment);
            return;
        }
    };
    if let Err(error) = runtime.request_frame() {
        runtime.abort(&events);
        let _ = ready.send(Err(error));
        drop(apartment);
        return;
    }

    let mut ready = Some(ready);
    let mut terminal_error = None;
    let start_deadline = Instant::now() + START_TIMEOUT;
    loop {
        let wait = if ready.is_some() {
            start_deadline.saturating_duration_since(Instant::now())
        } else {
            START_TIMEOUT
        };
        let event = match events.recv_timeout(wait) {
            Ok(event) => event,
            Err(RecvTimeoutError::Timeout) if ready.is_some() => {
                if let Some(ready) = ready.take() {
                    let _ = ready.send(Err(RecorderError::start(
                        "Timed out waiting for the first camera frame",
                    )));
                }
                runtime.abort(&events);
                break;
            }
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => {
                if let Some(ready) = ready.take() {
                    let _ = ready.send(Err(RecorderError::start(
                        "Camera worker event channel disconnected",
                    )));
                } else {
                    set_terminal_error(
                        &mut terminal_error,
                        &health,
                        RecorderError::capture("Camera worker event channel disconnected"),
                    );
                }
                runtime.abort(&events);
                break;
            }
        };

        match event {
            WorkerEvent::Command(command) => {
                if handle_command(command, &mut runtime, &events, &mut terminal_error) {
                    break;
                }
            }
            WorkerEvent::Frame(frame) => {
                if terminal_error.is_some() {
                    continue;
                }
                if !frame.bytes.is_empty() {
                    if let Err(error) = runtime.handle_frame(frame) {
                        if let Some(ready) = ready.take() {
                            let _ = ready.send(Err(error));
                            runtime.abort(&events);
                            break;
                        }
                        set_terminal_error(&mut terminal_error, &health, error);
                        continue;
                    }
                    if let Some(ready) = ready.take() {
                        let _ = ready.send(Ok(()));
                    }
                }
                if let Err(error) = runtime.request_frame() {
                    if let Some(ready) = ready.take() {
                        let _ = ready.send(Err(error));
                        runtime.abort(&events);
                        break;
                    }
                    set_terminal_error(&mut terminal_error, &health, error);
                }
            }
            WorkerEvent::CaptureError(message) => {
                let error = RecorderError::capture(message);
                if let Some(ready) = ready.take() {
                    let _ = ready.send(Err(error));
                    runtime.abort(&events);
                    break;
                }
                set_terminal_error(&mut terminal_error, &health, error);
            }
            WorkerEvent::EndOfStream => {
                let error = RecorderError::capture("Camera stream ended during recording");
                if let Some(ready) = ready.take() {
                    let _ = ready.send(Err(error));
                    runtime.abort(&events);
                    break;
                }
                set_terminal_error(&mut terminal_error, &health, error);
            }
            WorkerEvent::Flushed => {}
        }
    }
    drop(apartment);
}

fn set_terminal_error(
    terminal_error: &mut Option<RecorderError>,
    health: &Sender<RecorderError>,
    error: RecorderError,
) {
    if terminal_error.is_some() {
        return;
    }
    let _ = health.send(error.clone());
    *terminal_error = Some(error);
}

fn handle_command(
    command: CameraCommand,
    runtime: &mut CameraRuntime,
    events: &Receiver<WorkerEvent>,
    terminal_error: &mut Option<RecorderError>,
) -> bool {
    match command {
        CameraCommand::Sync(clock, response) => {
            let result = terminal_error
                .clone()
                .map_or_else(|| runtime.sync(clock), Err);
            if let Err(error) = &result {
                if error.code != "INVALID_STATE" {
                    *terminal_error = Some(error.clone());
                }
            }
            let _ = response.send(result);
            false
        }
        CameraCommand::Pause(response) => {
            let result = terminal_error.clone().map_or_else(|| runtime.pause(), Err);
            let _ = response.send(result);
            false
        }
        CameraCommand::Resume(response) => {
            let result = terminal_error.clone().map_or_else(|| runtime.resume(), Err);
            let _ = response.send(result);
            false
        }
        CameraCommand::Stop(response) => {
            let result = if let Some(error) = terminal_error.as_ref() {
                runtime.abort(events);
                Err(error.clone())
            } else {
                runtime.finish(events)
            };
            let _ = response.send(result);
            true
        }
        CameraCommand::Abort(response) => {
            runtime.abort(events);
            let _ = response.send(());
            true
        }
    }
}

struct CameraApartment {
    com_initialized: bool,
    mf_started: bool,
}

impl CameraApartment {
    fn initialize() -> Result<Self, RecorderError> {
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }
            .ok()
            .map_err(|error| {
                RecorderError::start(format!(
                    "Failed to initialize camera COM apartment: {error}"
                ))
            })?;
        if let Err(error) = unsafe { MFStartup(MF_VERSION, MFSTARTUP_FULL) } {
            unsafe { CoUninitialize() };
            return Err(RecorderError::start(format!(
                "Failed to start Media Foundation for camera capture: {error}"
            )));
        }
        Ok(Self {
            com_initialized: true,
            mf_started: true,
        })
    }
}

impl Drop for CameraApartment {
    fn drop(&mut self) {
        if self.mf_started {
            unsafe {
                let _ = MFShutdown();
            }
            self.mf_started = false;
        }
        if self.com_initialized {
            unsafe { CoUninitialize() };
            self.com_initialized = false;
        }
    }
}

struct MediaSourceGuard {
    source: Option<IMFMediaSource>,
}

impl MediaSourceGuard {
    fn new(source: IMFMediaSource) -> Self {
        Self {
            source: Some(source),
        }
    }

    fn get(&self) -> Result<&IMFMediaSource, RecorderError> {
        self.source
            .as_ref()
            .ok_or_else(|| RecorderError::capture("Camera media source is unavailable"))
    }

    fn take(&mut self) -> Result<IMFMediaSource, RecorderError> {
        self.source
            .take()
            .ok_or_else(|| RecorderError::capture("Camera media source is unavailable"))
    }
}

impl Drop for MediaSourceGuard {
    fn drop(&mut self) {
        if let Some(source) = self.source.take() {
            let _ = unsafe { source.Shutdown() };
        }
    }
}

struct CameraRuntime {
    source: Option<IMFMediaSource>,
    reader: Option<IMFSourceReader>,
    callback: Option<IMFSourceReaderCallback>,
    writer: Option<IMFSinkWriter>,
    stream_index: u32,
    device_id: String,
    device_name: String,
    width: u32,
    height: u32,
    metadata_frame_rate: u32,
    frame_duration: i64,
    video_path: PathBuf,
    metadata_path: PathBuf,
    temporary_video_path: PathBuf,
    temporary_metadata_path: PathBuf,
    pending_frames: VecDeque<CameraFrame>,
    sync_clock: Option<CameraSyncClock>,
    source_origin: Option<i64>,
    paused: bool,
    pause_started: Option<i64>,
    total_pause: i64,
    last_capture: Option<CameraCapturePoint>,
    last_written: Option<i64>,
    first_written: Option<i64>,
    capture_stopped: bool,
    committed: bool,
}

impl CameraRuntime {
    fn prepare(
        config: CameraRecordingConfig,
        events: Sender<WorkerEvent>,
    ) -> Result<Self, RecorderError> {
        let selected = select_camera(config.device_id.as_deref(), config.device_name.as_deref())?;
        let source: IMFMediaSource =
            unsafe { selected.activation.ActivateObject() }.map_err(|error| {
                RecorderError::configuration(format!("Failed to open camera: {error}"))
            })?;
        let mut source = MediaSourceGuard::new(source);
        let callback: IMFSourceReaderCallback = SourceReaderCallback { events }.into();
        let attributes = create_attributes(3)?;
        unsafe {
            attributes
                .SetUnknown(&MF_SOURCE_READER_ASYNC_CALLBACK, &callback)
                .map_err(mf_error)?;
            attributes
                .SetUINT32(&MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, 1)
                .map_err(mf_error)?;
            attributes
                .SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 1)
                .map_err(mf_error)?;
        }
        let reader = unsafe { MFCreateSourceReaderFromMediaSource(source.get()?, &attributes) }
            .map_err(|error| {
                RecorderError::configuration(format!("Failed to create camera reader: {error}"))
            })?;
        let requested_format = select_format(&reader, config.frame_rate)?;
        let requested_input_type = create_video_type(
            MFVideoFormat_RGB32,
            requested_format.width,
            requested_format.height,
            requested_format.frame_rate_numerator,
            requested_format.frame_rate_denominator,
            None,
        )?;
        unsafe {
            requested_input_type
                .SetUINT32(&MF_MT_FIXED_SIZE_SAMPLES, 1)
                .map_err(mf_error)?;
            requested_input_type
                .SetUINT32(&MF_MT_ALL_SAMPLES_INDEPENDENT, 1)
                .map_err(mf_error)?;
            requested_input_type
                .SetUINT32(
                    &MF_MT_SAMPLE_SIZE,
                    requested_format
                        .width
                        .saturating_mul(requested_format.height)
                        .saturating_mul(4),
                )
                .map_err(mf_error)?;
            reader
                .SetCurrentMediaType(
                    MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32,
                    None,
                    &requested_input_type,
                )
                .map_err(|error| {
                    RecorderError::configuration(format!(
                        "Camera cannot provide RGB video at the selected format: {error}"
                    ))
                })?;
        }
        let input_type =
            unsafe { reader.GetCurrentMediaType(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32) }
                .map_err(|error| {
                    RecorderError::configuration(format!(
                        "Failed to read the active camera format: {error}"
                    ))
                })?;
        let format = media_type_format(&input_type)?;
        unsafe {
            input_type
                .SetUINT32(&MF_MT_FIXED_SIZE_SAMPLES, 1)
                .map_err(mf_error)?;
            input_type
                .SetUINT32(&MF_MT_ALL_SAMPLES_INDEPENDENT, 1)
                .map_err(mf_error)?;
            input_type
                .SetUINT32(
                    &MF_MT_SAMPLE_SIZE,
                    format.width.saturating_mul(format.height).saturating_mul(4),
                )
                .map_err(mf_error)?;
        }

        let video_path = config.project_dir.join(CAMERA_VIDEO_NAME);
        let metadata_path = config.project_dir.join(CAMERA_METADATA_NAME);
        let temporary_video_path = config.project_dir.join(".camera.capty-partial.mp4");
        let temporary_metadata_path = config.project_dir.join(".camera.capty-partial.json");
        remove_if_exists(&temporary_video_path)?;
        remove_if_exists(&temporary_metadata_path)?;
        let mut partial_video = PartialAssetGuard::new(temporary_video_path.clone());
        let writer_attributes = create_attributes(2)?;
        unsafe {
            writer_attributes
                .SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 1)
                .map_err(mf_error)?;
            writer_attributes
                .SetUINT32(&MF_SINK_WRITER_DISABLE_THROTTLING, 1)
                .map_err(mf_error)?;
        }
        let wide_path = wide_path(&temporary_video_path);
        let writer = unsafe {
            MFCreateSinkWriterFromURL(
                PCWSTR(wide_path.as_ptr()),
                None::<&IMFByteStream>,
                Some(&writer_attributes),
            )
        }
        .map_err(|error| {
            RecorderError::capture(format!("Failed to create camera H.264 writer: {error}"))
        })?;
        let output_type = create_video_type(
            MFVideoFormat_H264,
            format.width,
            format.height,
            format.frame_rate_numerator,
            format.frame_rate_denominator,
            Some(camera_bitrate(format.width, format.height)),
        )?;
        unsafe {
            output_type
                .SetUINT32(&MF_MT_MPEG2_PROFILE, 100)
                .map_err(mf_error)?;
        }
        let stream_index = unsafe { writer.AddStream(&output_type) }.map_err(|error| {
            RecorderError::capture(format!("Failed to add camera H.264 stream: {error}"))
        })?;
        unsafe {
            writer
                .SetInputMediaType(stream_index, &input_type, None::<&IMFAttributes>)
                .map_err(|error| {
                    RecorderError::capture(format!("Failed to configure camera encoder: {error}"))
                })?;
            writer.BeginWriting().map_err(|error| {
                RecorderError::capture(format!("Failed to start camera encoder: {error}"))
            })?;
        }

        let frame_duration = (10_000_000_i64
            .saturating_mul(i64::from(format.frame_rate_denominator))
            / i64::from(format.frame_rate_numerator))
        .max(1);
        partial_video.keep();
        Ok(Self {
            source: Some(source.take()?),
            reader: Some(reader),
            callback: Some(callback),
            writer: Some(writer),
            stream_index,
            device_id: selected.id,
            device_name: selected.name,
            width: format.width,
            height: format.height,
            metadata_frame_rate: config.frame_rate,
            frame_duration,
            video_path,
            metadata_path,
            temporary_video_path,
            temporary_metadata_path,
            pending_frames: VecDeque::with_capacity(MAX_PENDING_FRAMES),
            sync_clock: None,
            source_origin: None,
            paused: false,
            pause_started: None,
            total_pause: 0,
            last_capture: None,
            last_written: None,
            first_written: None,
            capture_stopped: false,
            committed: false,
        })
    }

    fn request_frame(&self) -> Result<(), RecorderError> {
        let reader = self
            .reader
            .as_ref()
            .ok_or_else(|| RecorderError::capture("Camera reader is unavailable"))?;
        unsafe {
            reader.ReadSample(
                MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32,
                0,
                None,
                None,
                None,
                None,
            )
        }
        .map_err(|error| RecorderError::capture(format!("Failed to request camera frame: {error}")))
    }

    fn handle_frame(&mut self, frame: CameraFrame) -> Result<(), RecorderError> {
        self.last_capture = Some(CameraCapturePoint {
            source_time: frame.source_time,
            arrived_at: frame.arrived_at,
        });
        if self.paused {
            return Ok(());
        }
        if self.sync_clock.is_none() {
            if self.pending_frames.len() == MAX_PENDING_FRAMES {
                self.pending_frames.pop_front();
            }
            self.pending_frames.push_back(frame);
            return Ok(());
        }
        self.write_frame(frame)
    }

    fn sync(&mut self, clock: CameraSyncClock) -> Result<(), RecorderError> {
        if self.sync_clock.is_some() {
            return Err(RecorderError::invalid_state(
                "Camera is already synchronized",
            ));
        }
        let capture = self
            .last_capture
            .ok_or_else(|| RecorderError::capture("Camera has not produced a frame"))?;
        self.source_origin = Some(bridge_source_time(capture, clock.monotonic_time));
        self.sync_clock = Some(clock);
        while let Some(frame) = self.pending_frames.pop_front() {
            self.write_frame(frame)?;
        }
        Ok(())
    }

    fn pause(&mut self) -> Result<(), RecorderError> {
        if self.sync_clock.is_none() {
            return Err(RecorderError::invalid_state(
                "Cannot pause camera before screen synchronization",
            ));
        }
        if self.paused {
            return Err(RecorderError::invalid_state("Camera is already paused"));
        }
        self.paused = true;
        if self.pause_started.is_none() {
            self.pause_started = self
                .last_capture
                .zip(self.source_origin)
                .map(|(capture, origin)| capture.source_time.max(origin));
        }
        Ok(())
    }

    fn resume(&mut self) -> Result<(), RecorderError> {
        if !self.paused {
            return Err(RecorderError::invalid_state("Camera is not paused"));
        }
        self.paused = false;
        Ok(())
    }

    fn write_frame(&mut self, frame: CameraFrame) -> Result<(), RecorderError> {
        let origin = self
            .source_origin
            .ok_or_else(|| RecorderError::capture("Camera is not synchronized"))?;
        if frame.source_time < origin {
            return Ok(());
        }
        if let Some(pause_started) = self.pause_started.take() {
            self.total_pause = self
                .total_pause
                .saturating_add(frame.source_time.saturating_sub(pause_started));
        }
        let timestamp = adjusted_camera_timestamp(frame.source_time, origin, self.total_pause);
        if let Some(last_written) = self.last_written {
            if timestamp <= last_written {
                return Ok(());
            }
        }
        let minimum_size = self.width.saturating_mul(self.height).saturating_mul(4) as usize;
        if frame.bytes.len() < minimum_size {
            return Err(RecorderError::capture(
                "Camera returned an incomplete RGB frame",
            ));
        }
        let buffer =
            unsafe { MFCreateMemoryBuffer(frame.bytes.len() as u32) }.map_err(|error| {
                RecorderError::capture(format!("Failed to allocate camera frame: {error}"))
            })?;
        let mut pointer = std::ptr::null_mut();
        let mut capacity = 0;
        unsafe { buffer.Lock(&mut pointer, Some(&mut capacity), None) }.map_err(|error| {
            RecorderError::capture(format!("Failed to lock camera frame: {error}"))
        })?;
        if pointer.is_null() || frame.bytes.len() > capacity as usize {
            let _ = unsafe { buffer.Unlock() };
            return Err(RecorderError::capture(
                "Camera frame buffer has an invalid size",
            ));
        }
        unsafe {
            std::ptr::copy_nonoverlapping(frame.bytes.as_ptr(), pointer, frame.bytes.len());
        }
        unsafe { buffer.Unlock() }.map_err(|error| {
            RecorderError::capture(format!("Failed to unlock camera frame: {error}"))
        })?;
        unsafe { buffer.SetCurrentLength(frame.bytes.len() as u32) }.map_err(|error| {
            RecorderError::capture(format!("Failed to set camera frame length: {error}"))
        })?;
        let sample = unsafe { MFCreateSample() }.map_err(|error| {
            RecorderError::capture(format!("Failed to create camera sample: {error}"))
        })?;
        unsafe {
            sample.AddBuffer(&buffer).map_err(|error| {
                RecorderError::capture(format!("Failed to attach camera frame: {error}"))
            })?;
            sample.SetSampleTime(timestamp).map_err(|error| {
                RecorderError::capture(format!("Failed to set camera timestamp: {error}"))
            })?;
            sample
                .SetSampleDuration(self.frame_duration)
                .map_err(|error| {
                    RecorderError::capture(format!("Failed to set camera duration: {error}"))
                })?;
            self.writer
                .as_ref()
                .ok_or_else(|| RecorderError::capture("Camera H.264 writer is unavailable"))?
                .WriteSample(self.stream_index, &sample)
                .map_err(|error| {
                    RecorderError::capture(format!("Failed to encode camera frame: {error}"))
                })?;
        }
        self.first_written.get_or_insert(timestamp);
        self.last_written = Some(timestamp);
        Ok(())
    }

    fn finish(
        &mut self,
        events: &Receiver<WorkerEvent>,
    ) -> Result<CameraRecordingResult, RecorderError> {
        if self.sync_clock.is_none() {
            self.abort(events);
            return Err(RecorderError::stop(
                "Camera was not synchronized to the first screen frame",
            ));
        }
        if self.last_written.is_none() {
            self.abort(events);
            return Err(RecorderError::stop(
                "Camera did not record a synchronized frame",
            ));
        }
        self.stop_capture(events);
        let writer = self
            .writer
            .take()
            .ok_or_else(|| RecorderError::stop("Camera H.264 writer is unavailable"))?;
        unsafe { writer.Finalize() }.map_err(|error| {
            RecorderError::stop(format!("Failed to finalize camera recording: {error}"))
        })?;
        drop(writer);
        let duration =
            camera_duration_hns(self.last_written.unwrap_or_default(), self.frame_duration) as f64
                / HNS_PER_SECOND;
        self.write_metadata(duration)?;
        self.committed = true;
        Ok(CameraRecordingResult {
            video_path: self.video_path.clone(),
            metadata_path: self.metadata_path.clone(),
            duration,
            staged_assets: vec![
                StagedAsset {
                    temporary_path: self.temporary_video_path.clone(),
                    output_path: self.video_path.clone(),
                },
                StagedAsset {
                    temporary_path: self.temporary_metadata_path.clone(),
                    output_path: self.metadata_path.clone(),
                },
            ],
        })
    }

    fn write_metadata(&self, duration: f64) -> Result<(), RecorderError> {
        let clock = self
            .sync_clock
            .ok_or_else(|| RecorderError::stop("Camera synchronization clock is unavailable"))?;
        let offset = self.first_written.unwrap_or_default() as f64 / 10_000.0;
        let data = CameraDataFile {
            video_file: CAMERA_VIDEO_NAME,
            meta: CameraMetadata {
                device_id: &self.device_id,
                device_name: &self.device_name,
                width: self.width,
                height: self.height,
                duration: round(duration, 3),
                start_time: format_system_time(clock.wall_time),
                frame_rate: self.metadata_frame_rate,
                sync_offset_ms: round(offset, 1),
                synced: true,
            },
        };
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&self.temporary_metadata_path)
            .map_err(|error| {
                RecorderError::stop(format!("Failed to create camera metadata: {error}"))
            })?;
        let mut writer = BufWriter::new(file);
        serde_json::to_writer_pretty(&mut writer, &data).map_err(|error| {
            RecorderError::stop(format!("Failed to encode camera metadata: {error}"))
        })?;
        writer.write_all(b"\n").map_err(|error| {
            RecorderError::stop(format!("Failed to write camera metadata: {error}"))
        })?;
        writer.flush().map_err(|error| {
            RecorderError::stop(format!("Failed to flush camera metadata: {error}"))
        })?;
        drop(writer);
        Ok(())
    }

    fn stop_capture(&mut self, events: &Receiver<WorkerEvent>) {
        if self.capture_stopped {
            return;
        }
        self.capture_stopped = true;
        if let Some(reader) = self.reader.as_ref() {
            if unsafe { reader.Flush(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32) }.is_ok() {
                let deadline = std::time::Instant::now() + FLUSH_TIMEOUT;
                while std::time::Instant::now() < deadline {
                    match events
                        .recv_timeout(deadline.saturating_duration_since(std::time::Instant::now()))
                    {
                        Ok(WorkerEvent::Flushed) => break,
                        Ok(_) => continue,
                        Err(_) => break,
                    }
                }
            }
        }
        if let Some(source) = self.source.take() {
            let _ = unsafe { source.Shutdown() };
        }
        self.reader.take();
        self.callback.take();
    }

    fn abort(&mut self, events: &Receiver<WorkerEvent>) {
        self.stop_capture(events);
        self.writer.take();
        let _ = std::fs::remove_file(&self.temporary_video_path);
        let _ = std::fs::remove_file(&self.temporary_metadata_path);
    }
}

impl Drop for CameraRuntime {
    fn drop(&mut self) {
        self.writer.take();
        if !self.committed {
            let _ = std::fs::remove_file(&self.temporary_video_path);
            let _ = std::fs::remove_file(&self.temporary_metadata_path);
        }
    }
}

struct PartialAssetGuard {
    path: PathBuf,
    retained: bool,
}

impl PartialAssetGuard {
    fn new(path: PathBuf) -> Self {
        Self {
            path,
            retained: false,
        }
    }

    fn keep(&mut self) {
        self.retained = true;
    }
}

impl Drop for PartialAssetGuard {
    fn drop(&mut self) {
        if !self.retained {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

struct CameraDevice {
    activation: IMFActivate,
    id: String,
    name: String,
}

fn select_camera(
    requested_id: Option<&str>,
    requested_name: Option<&str>,
) -> Result<CameraDevice, RecorderError> {
    let mut devices = enumerate_cameras()?;
    if devices.is_empty() {
        return Err(RecorderError::configuration("No camera device found"));
    }
    let requested_id = requested_id
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let requested_name = requested_name
        .map(str::trim)
        .filter(|value| !value.is_empty());

    if let Some(requested_id) = requested_id {
        if let Some(index) = devices
            .iter()
            .position(|device| device.id.eq_ignore_ascii_case(requested_id))
        {
            return Ok(devices.swap_remove(index));
        }
    }

    if let Some(requested_name) = requested_name {
        let exact: Vec<usize> = devices
            .iter()
            .enumerate()
            .filter_map(|(index, device)| {
                device
                    .name
                    .eq_ignore_ascii_case(requested_name)
                    .then_some(index)
            })
            .collect();
        if exact.len() == 1 {
            return Ok(devices.swap_remove(exact[0]));
        }
        if exact.len() > 1 {
            return Err(RecorderError::configuration(
                "Multiple cameras have the selected name; select a device ID",
            ));
        }

        let needle = requested_name.to_lowercase();
        let partial: Vec<usize> = devices
            .iter()
            .enumerate()
            .filter_map(|(index, device)| {
                let name = device.name.to_lowercase();
                (name.contains(&needle) || needle.contains(&name)).then_some(index)
            })
            .collect();
        if partial.len() == 1 {
            return Ok(devices.swap_remove(partial[0]));
        }
        if partial.len() > 1 {
            return Err(RecorderError::configuration(
                "The selected camera name matches multiple devices",
            ));
        }
    }

    if requested_id.is_some() || requested_name.is_some() {
        return Err(RecorderError::configuration(
            "The selected camera is no longer available",
        ));
    }
    Ok(devices.swap_remove(0))
}

fn enumerate_cameras() -> Result<Vec<CameraDevice>, RecorderError> {
    let attributes = create_attributes(1)?;
    unsafe {
        attributes
            .SetGUID(
                &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE,
                &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID,
            )
            .map_err(mf_error)?;
    }
    let mut raw = std::ptr::null_mut();
    let mut count = 0;
    unsafe { MFEnumDeviceSources(&attributes, &mut raw, &mut count) }.map_err(|error| {
        RecorderError::configuration(format!("Failed to enumerate cameras: {error}"))
    })?;
    if count == 0 {
        if !raw.is_null() {
            unsafe {
                CoTaskMemFree(Some(raw.cast()));
            }
        }
        return Ok(Vec::new());
    }
    if raw.is_null() {
        return Err(RecorderError::configuration(
            "Camera enumeration returned an invalid device list",
        ));
    }
    let entries = unsafe { std::slice::from_raw_parts_mut(raw, count as usize) };
    let mut devices = Vec::with_capacity(count as usize);
    for entry in entries {
        let Some(activation) = (unsafe { std::ptr::read(entry) }) else {
            continue;
        };
        let name = attribute_string(&activation, &MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME)
            .unwrap_or_else(|| "Camera".to_string());
        let id = attribute_string(
            &activation,
            &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK,
        )
        .unwrap_or_default();
        devices.push(CameraDevice {
            activation,
            id,
            name,
        });
    }
    unsafe {
        CoTaskMemFree(Some(raw.cast()));
    }
    Ok(devices)
}

fn attribute_string(activate: &IMFActivate, key: &windows::core::GUID) -> Option<String> {
    let length = unsafe { activate.GetStringLength(key) }.ok()?;
    let mut buffer = vec![0; length as usize + 1];
    unsafe { activate.GetString(key, &mut buffer, None) }.ok()?;
    Some(String::from_utf16_lossy(&buffer[..length as usize]))
}

#[derive(Clone, Copy)]
struct CameraFormat {
    width: u32,
    height: u32,
    frame_rate_numerator: u32,
    frame_rate_denominator: u32,
}

fn select_format(
    reader: &IMFSourceReader,
    requested_frame_rate: u32,
) -> Result<CameraFormat, RecorderError> {
    let mut formats = Vec::new();
    let mut index = 0;
    loop {
        let media_type = match unsafe {
            reader.GetNativeMediaType(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32, index)
        } {
            Ok(media_type) => media_type,
            Err(_) => break,
        };
        index += 1;
        let Ok(format) = media_type_format(&media_type) else {
            continue;
        };
        formats.push(format);
    }
    formats
        .into_iter()
        .min_by_key(|format| format_score(*format, requested_frame_rate))
        .ok_or_else(|| RecorderError::configuration("Camera has no supported video format"))
}

fn media_type_format(media_type: &IMFMediaType) -> Result<CameraFormat, RecorderError> {
    let size = unsafe { media_type.GetUINT64(&MF_MT_FRAME_SIZE) }.map_err(|error| {
        RecorderError::configuration(format!("Camera format has no frame size: {error}"))
    })?;
    let rate = unsafe { media_type.GetUINT64(&MF_MT_FRAME_RATE) }.map_err(|error| {
        RecorderError::configuration(format!("Camera format has no frame rate: {error}"))
    })?;
    let format = CameraFormat {
        width: (size >> 32) as u32,
        height: size as u32,
        frame_rate_numerator: (rate >> 32) as u32,
        frame_rate_denominator: rate as u32,
    };
    if format.width == 0
        || format.height == 0
        || format.width % 2 != 0
        || format.height % 2 != 0
        || format.frame_rate_numerator == 0
        || format.frame_rate_denominator == 0
    {
        return Err(RecorderError::configuration(
            "Camera format has invalid dimensions or frame rate",
        ));
    }
    Ok(format)
}

fn format_score(format: CameraFormat, requested_frame_rate: u32) -> u64 {
    let width_delta = format.width.abs_diff(TARGET_WIDTH) as u64;
    let height_delta = format.height.abs_diff(TARGET_HEIGHT) as u64;
    let frame_rate = divide_ratio(format.frame_rate_numerator, format.frame_rate_denominator);
    let rate_delta = frame_rate.abs_diff(requested_frame_rate) as u64;
    width_delta
        .saturating_add(height_delta)
        .saturating_mul(1_000)
        .saturating_add(rate_delta)
}

fn divide_ratio(numerator: u32, denominator: u32) -> u32 {
    ((u64::from(numerator) + u64::from(denominator) / 2) / u64::from(denominator))
        .clamp(1, u64::from(u32::MAX)) as u32
}

fn validate_config(config: &CameraRecordingConfig) -> Result<(), RecorderError> {
    if !config.project_dir.is_dir() {
        return Err(RecorderError::configuration(
            "Camera project directory does not exist",
        ));
    }
    if !(1..=240).contains(&config.frame_rate) {
        return Err(RecorderError::configuration(
            "Camera frame rate must be between 1 and 240",
        ));
    }
    Ok(())
}

fn create_attributes(capacity: u32) -> Result<IMFAttributes, RecorderError> {
    let mut attributes = None;
    unsafe { MFCreateAttributes(&mut attributes, capacity) }.map_err(|error| {
        RecorderError::capture(format!(
            "Failed to create Media Foundation attributes: {error}"
        ))
    })?;
    attributes.ok_or_else(|| RecorderError::capture("Media Foundation attributes are unavailable"))
}

fn create_video_type(
    subtype: windows::core::GUID,
    width: u32,
    height: u32,
    frame_rate_numerator: u32,
    frame_rate_denominator: u32,
    bitrate: Option<u32>,
) -> Result<IMFMediaType, RecorderError> {
    let media_type = unsafe { MFCreateMediaType() }.map_err(|error| {
        RecorderError::capture(format!("Failed to create camera video type: {error}"))
    })?;
    unsafe {
        media_type
            .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)
            .map_err(mf_error)?;
        media_type
            .SetGUID(&MF_MT_SUBTYPE, &subtype)
            .map_err(mf_error)?;
        media_type
            .SetUINT64(&MF_MT_FRAME_SIZE, pack_ratio(width, height))
            .map_err(mf_error)?;
        media_type
            .SetUINT64(
                &MF_MT_FRAME_RATE,
                pack_ratio(frame_rate_numerator, frame_rate_denominator),
            )
            .map_err(mf_error)?;
        media_type
            .SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, pack_ratio(1, 1))
            .map_err(mf_error)?;
        media_type
            .SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)
            .map_err(mf_error)?;
        if let Some(bitrate) = bitrate {
            media_type
                .SetUINT32(&MF_MT_AVG_BITRATE, bitrate)
                .map_err(mf_error)?;
        }
    }
    Ok(media_type)
}

fn mf_error(error: windows::core::Error) -> RecorderError {
    RecorderError::capture(format!(
        "Failed to configure camera Media Foundation state: {error}"
    ))
}

fn camera_bitrate(width: u32, height: u32) -> u32 {
    u64::from(width)
        .saturating_mul(u64::from(height))
        .saturating_mul(8)
        .clamp(4_000_000, 40_000_000) as u32
}

fn pack_ratio(numerator: u32, denominator: u32) -> u64 {
    (u64::from(numerator) << 32) | u64::from(denominator)
}

fn duration_hns(duration: Duration) -> i64 {
    let seconds = duration
        .as_secs()
        .saturating_mul(10_000_000)
        .saturating_add(u64::from(duration.subsec_nanos()) / 100);
    seconds.min(i64::MAX as u64) as i64
}

fn bridge_source_time(capture: CameraCapturePoint, screen_time: Instant) -> i64 {
    if screen_time >= capture.arrived_at {
        return capture
            .source_time
            .saturating_add(duration_hns(screen_time.duration_since(capture.arrived_at)));
    }
    capture
        .source_time
        .saturating_sub(duration_hns(capture.arrived_at.duration_since(screen_time)))
}

fn adjusted_camera_timestamp(source_time: i64, origin: i64, total_pause: i64) -> i64 {
    source_time
        .saturating_sub(origin)
        .saturating_sub(total_pause)
        .max(0)
}

fn camera_duration_hns(last_written: i64, frame_duration: i64) -> i64 {
    last_written.saturating_add(frame_duration).max(0)
}

fn wide_path(path: &Path) -> Vec<u16> {
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

fn remove_if_exists(path: &Path) -> Result<(), RecorderError> {
    if !path.exists() {
        return Ok(());
    }
    std::fs::remove_file(path).map_err(|error| {
        RecorderError::capture(format!("Failed to replace camera temporary file: {error}"))
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CameraDataFile<'a> {
    video_file: &'a str,
    meta: CameraMetadata<'a>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CameraMetadata<'a> {
    device_id: &'a str,
    device_name: &'a str,
    width: u32,
    height: u32,
    duration: f64,
    start_time: String,
    frame_rate: u32,
    sync_offset_ms: f64,
    synced: bool,
}

fn round(value: f64, decimals: u32) -> f64 {
    let factor = 10_f64.powi(decimals as i32);
    (value * factor).round() / factor
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

    #[test]
    fn camera_timeline_uses_source_timestamps_despite_callback_jitter() {
        let base = Instant::now();
        let capture = CameraCapturePoint {
            source_time: 20_000_000,
            arrived_at: base + Duration::from_millis(700),
        };
        let origin = bridge_source_time(capture, base + Duration::from_millis(800));

        assert_eq!(origin, 21_000_000);
        assert_eq!(adjusted_camera_timestamp(22_000_000, origin, 0), 1_000_000);
        assert_eq!(adjusted_camera_timestamp(23_000_000, origin, 0), 2_000_000);
    }

    #[test]
    fn camera_timeline_removes_source_timestamp_pause_gap() {
        assert_eq!(
            adjusted_camera_timestamp(80_000_000, 20_000_000, 30_000_000),
            30_000_000
        );
    }

    #[test]
    fn camera_duration_includes_the_final_sample() {
        let frame_duration = 333_333;

        assert_eq!(camera_duration_hns(0, frame_duration), frame_duration);
        assert_eq!(
            camera_duration_hns(frame_duration * 4, frame_duration),
            frame_duration * 5
        );
    }
}
