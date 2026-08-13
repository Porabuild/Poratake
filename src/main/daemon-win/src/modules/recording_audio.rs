use super::recorder_types::{RecorderError, RecordingConfig, StagedAsset};
use crate::com::retain_process_mta;
use std::collections::VecDeque;
use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;
use windows::core::PCWSTR;
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT};
use windows::Win32::Media::Audio::{
    eCapture, eMultimedia, eRender, IAudioCaptureClient, IAudioClient, IMMDevice,
    IMMDeviceEnumerator, MMDeviceEnumerator, AUDCLNT_BUFFERFLAGS_DATA_DISCONTINUITY,
    AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_BUFFERFLAGS_TIMESTAMP_ERROR, AUDCLNT_SHAREMODE_SHARED,
    AUDCLNT_STREAMFLAGS_EVENTCALLBACK, AUDCLNT_STREAMFLAGS_LOOPBACK, DEVICE_STATE_ACTIVE,
    WAVEFORMATEX, WAVEFORMATEXTENSIBLE,
};
use windows::Win32::Media::MediaFoundation::{
    IMFAttributes, IMFByteStream, IMFSinkWriter, MFAudioFormat_AAC, MFAudioFormat_Float,
    MFAudioFormat_PCM, MFCreateAttributes, MFCreateMediaType, MFCreateMemoryBuffer, MFCreateSample,
    MFCreateSinkWriterFromURL, MFMediaType_Audio, MFShutdown, MFStartup, MFSTARTUP_FULL,
    MF_MT_AAC_AUDIO_PROFILE_LEVEL_INDICATION, MF_MT_AAC_PAYLOAD_TYPE,
    MF_MT_AUDIO_AVG_BYTES_PER_SECOND, MF_MT_AUDIO_BITS_PER_SAMPLE, MF_MT_AUDIO_BLOCK_ALIGNMENT,
    MF_MT_AUDIO_NUM_CHANNELS, MF_MT_AUDIO_SAMPLES_PER_SECOND, MF_MT_MAJOR_TYPE, MF_MT_SUBTYPE,
    MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, MF_SINK_WRITER_DISABLE_THROTTLING, MF_VERSION,
};
use windows::Win32::System::Com::StructuredStorage::PropVariantToStringAlloc;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_ALL,
    COINIT_MULTITHREADED, STGM_READ,
};
use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject};

pub(super) const HNS_PER_SECOND: i64 = 10_000_000;
const AAC_SAMPLE_RATE: u32 = 48_000;
const AAC_CHANNELS: u32 = 2;
const AAC_BIT_RATE: u32 = 192_000;
const AAC_BITS_PER_SAMPLE: u32 = 16;
const AAC_BLOCK_ALIGN: u32 = AAC_CHANNELS * AAC_BITS_PER_SAMPLE / 8;
const CAPTURE_WAIT_MS: u32 = 100;
const SILENCE_CHUNK_FRAMES: u32 = 4_096;
const MAX_PENDING_AUDIO_BYTES: usize = 16 * 1024 * 1024;
const SYSTEM_AUDIO_FILE: &str = "system.m4a";
const MICROPHONE_AUDIO_FILE: &str = "mic.m4a";
const IDLE_COMMAND_WAIT: Duration = Duration::from_millis(CAPTURE_WAIT_MS as u64);
const RECONFIGURE_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Copy)]
pub(super) enum AudioKind {
    System,
    Microphone,
}

impl AudioKind {
    fn label(self) -> &'static str {
        match self {
            Self::System => "system audio",
            Self::Microphone => "microphone",
        }
    }
}

#[derive(Clone, Default, Debug, Eq, PartialEq)]
pub struct AudioDevice {
    pub id: Option<String>,
    pub name: Option<String>,
}

enum AudioCommand {
    Reconfigure(Option<AudioDevice>, Sender<Result<(), RecorderError>>),
    Stop(f64),
    Abort,
}

#[derive(Clone, Default)]
struct AudioClockState {
    origin: Option<i64>,
    pauses: Vec<(i64, i64)>,
    pause_start: Option<i64>,
}

impl AudioClockState {
    fn sync(&mut self, source_time: i64) {
        if self.origin.is_none() {
            self.origin = Some(source_time);
        }
    }

    fn pause(&mut self, source_time: i64) {
        if self.pause_start.is_none() {
            self.pause_start = Some(source_time);
        }
    }

    fn resume(&mut self, source_time: i64) {
        let Some(start) = self.pause_start.take() else {
            return;
        };
        self.pauses.push((start, source_time.max(start)));
    }

    fn visible_segments(
        &self,
        packet_start: i64,
        frame_count: u32,
        sample_rate: u32,
    ) -> Vec<AudioSegment> {
        let Some(origin) = self.origin else {
            return Vec::new();
        };
        let packet_duration = frames_to_hns(frame_count, sample_rate);
        let packet_end = packet_start.saturating_add(packet_duration);
        if packet_end <= origin {
            return Vec::new();
        }

        let mut pauses = self.pauses.clone();
        if let Some(start) = self.pause_start {
            pauses.push((start, i64::MAX));
        }
        pauses.sort_unstable_by_key(|pause| pause.0);

        let mut ranges = Vec::new();
        let mut cursor = packet_start.max(origin);
        for (pause_start, pause_end) in &pauses {
            if *pause_end <= cursor {
                continue;
            }
            if *pause_start >= packet_end {
                break;
            }
            if *pause_start > cursor {
                ranges.push((cursor, (*pause_start).min(packet_end)));
            }
            cursor = cursor.max(*pause_end);
            if cursor >= packet_end {
                break;
            }
        }
        if cursor < packet_end {
            ranges.push((cursor, packet_end));
        }

        ranges
            .into_iter()
            .filter_map(|(start, end)| {
                let first_frame =
                    hns_to_frames_ceil(start.saturating_sub(packet_start), sample_rate)
                        .min(frame_count);
                let end_frame = hns_to_frames_floor(end.saturating_sub(packet_start), sample_rate)
                    .min(frame_count);
                if end_frame <= first_frame {
                    return None;
                }
                let actual_source =
                    packet_start.saturating_add(frames_to_hns(first_frame, sample_rate));
                Some(AudioSegment {
                    first_frame,
                    frame_count: end_frame - first_frame,
                    timestamp: self.map_source(actual_source, origin),
                })
            })
            .collect()
    }

    fn map_source(&self, source_time: i64, origin: i64) -> i64 {
        let mut pause_duration = 0_i64;
        for (start, end) in &self.pauses {
            if *start >= source_time {
                continue;
            }
            pause_duration = pause_duration
                .saturating_add((*end).min(source_time).saturating_sub((*start).max(origin)));
        }
        source_time
            .saturating_sub(origin)
            .saturating_sub(pause_duration)
            .max(0)
    }
}

struct AudioSegment {
    first_frame: u32,
    frame_count: u32,
    timestamp: i64,
}

pub struct AudioCaptureSet {
    parent: PathBuf,
    clock: Arc<Mutex<AudioClockState>>,
    mic_muted: Arc<AtomicBool>,
    system: Option<AudioTrack>,
    microphone: Option<AudioTrack>,
}

impl AudioCaptureSet {
    pub fn start(config: &RecordingConfig, mic_muted: bool) -> Result<Self, RecorderError> {
        let parent = config
            .output_path
            .parent()
            .ok_or_else(|| {
                RecorderError::configuration("Recording output has no parent directory")
            })?
            .to_path_buf();
        let mut set = Self {
            parent,
            clock: Arc::new(Mutex::new(AudioClockState::default())),
            mic_muted: Arc::new(AtomicBool::new(mic_muted)),
            system: None,
            microphone: None,
        };

        if let Err(error) = set.open_initial_tracks(config) {
            set.abort_tracks();
            return Err(error);
        }

        Ok(set)
    }

    fn open_initial_tracks(&mut self, config: &RecordingConfig) -> Result<(), RecorderError> {
        if config.include_audio {
            self.set_system_audio(true)?;
        }
        if !config.mic_enabled {
            return Ok(());
        }
        self.set_microphone(Some(AudioDevice {
            id: config.mic_device_id.clone(),
            name: config.mic_device_name.clone(),
        }))
    }

    pub fn sync_with_first_frame(&self, source_time: i64) {
        if let Ok(mut clock) = self.clock.lock() {
            clock.sync(source_time);
        }
    }

    pub fn pause(&self, source_time: i64) {
        if let Ok(mut clock) = self.clock.lock() {
            clock.pause(source_time);
        }
    }

    pub fn resume(&self, source_time: i64) {
        if let Ok(mut clock) = self.clock.lock() {
            clock.resume(source_time);
        }
    }

    pub fn set_mic_muted(&self, muted: bool) {
        self.mic_muted.store(muted, Ordering::Release);
    }

    pub fn set_microphone(&mut self, device: Option<AudioDevice>) -> Result<(), RecorderError> {
        if let Some(track) = &self.microphone {
            return track.reconfigure(device);
        }

        let Some(device) = device else {
            return Ok(());
        };
        self.microphone = Some(self.open_track(
            AudioKind::Microphone,
            MICROPHONE_AUDIO_FILE,
            device,
            self.mic_muted.clone(),
        )?);
        Ok(())
    }

    pub fn set_system_audio(&mut self, enabled: bool) -> Result<(), RecorderError> {
        if let Some(track) = &self.system {
            return track.reconfigure(enabled.then(AudioDevice::default));
        }

        if !enabled {
            return Ok(());
        }
        self.system = Some(self.open_track(
            AudioKind::System,
            SYSTEM_AUDIO_FILE,
            AudioDevice::default(),
            Arc::new(AtomicBool::new(false)),
        )?);
        Ok(())
    }

    fn open_track(
        &self,
        kind: AudioKind,
        file_name: &str,
        device: AudioDevice,
        muted: Arc<AtomicBool>,
    ) -> Result<AudioTrack, RecorderError> {
        AudioTrack::start(
            kind,
            self.parent.join(file_name),
            device,
            self.clock.clone(),
            muted,
        )
    }

    pub fn try_error(&self) -> Option<RecorderError> {
        self.tracks().find_map(AudioTrack::try_error)
    }

    pub fn finish(mut self, duration: f64) -> Result<AudioAssetPaths, RecorderError> {
        if let Some(track) = &self.system {
            track.request_stop(duration);
        }
        if let Some(track) = &self.microphone {
            track.request_stop(duration);
        }
        let system = self.system.take().map(AudioTrack::wait_for_finish);
        let microphone = self.microphone.take().map(AudioTrack::wait_for_finish);
        let system_audio = transpose_track(system);
        let microphone_audio = transpose_track(microphone);
        match (system_audio, microphone_audio) {
            (Ok(system_audio), Ok(microphone_audio)) => Ok(AudioAssetPaths {
                system_audio,
                microphone_audio,
            }),
            (Err(error), Ok(Some(asset))) => {
                let _ = std::fs::remove_file(asset.temporary_path);
                Err(error)
            }
            (Ok(Some(asset)), Err(error)) => {
                let _ = std::fs::remove_file(asset.temporary_path);
                Err(error)
            }
            (Err(error), _) | (_, Err(error)) => Err(error),
        }
    }

    pub fn abort(mut self) {
        self.abort_tracks();
    }

    fn abort_tracks(&mut self) {
        if let Some(track) = self.system.take() {
            track.abort();
        }
        if let Some(track) = self.microphone.take() {
            track.abort();
        }
    }

    fn tracks(&self) -> impl Iterator<Item = &AudioTrack> {
        self.system.iter().chain(self.microphone.iter())
    }
}

pub struct AudioAssetPaths {
    pub system_audio: Option<StagedAsset>,
    pub microphone_audio: Option<StagedAsset>,
}

fn transpose_track(
    result: Option<Result<StagedAsset, RecorderError>>,
) -> Result<Option<StagedAsset>, RecorderError> {
    match result {
        Some(result) => result.map(Some),
        None => Ok(None),
    }
}

struct AudioTrack {
    commands: Sender<AudioCommand>,
    completion: Receiver<Result<StagedAsset, RecorderError>>,
    health: Receiver<RecorderError>,
    thread: Option<JoinHandle<()>>,
}

impl AudioTrack {
    fn start(
        kind: AudioKind,
        output_path: PathBuf,
        device: AudioDevice,
        clock: Arc<Mutex<AudioClockState>>,
        muted: Arc<AtomicBool>,
    ) -> Result<Self, RecorderError> {
        let (command_sender, command_receiver) = std::sync::mpsc::channel();
        let (ready_sender, ready_receiver) = std::sync::mpsc::channel();
        let (completion_sender, completion_receiver) = std::sync::mpsc::channel();
        let (health_sender, health_receiver) = std::sync::mpsc::channel();
        let thread = std::thread::spawn(move || {
            run_audio_worker(
                kind,
                output_path,
                device,
                command_receiver,
                ready_sender,
                completion_sender,
                health_sender,
                clock,
                muted,
            );
        });

        match ready_receiver.recv() {
            Ok(Ok(())) => Ok(Self {
                commands: command_sender,
                completion: completion_receiver,
                health: health_receiver,
                thread: Some(thread),
            }),
            Ok(Err(error)) => {
                let _ = thread.join();
                Err(error)
            }
            Err(_) => {
                let _ = thread.join();
                Err(RecorderError::start(format!(
                    "{} capture worker did not start",
                    kind.label()
                )))
            }
        }
    }

    fn reconfigure(&self, device: Option<AudioDevice>) -> Result<(), RecorderError> {
        let (response_sender, response_receiver) = std::sync::mpsc::channel();
        self.commands
            .send(AudioCommand::Reconfigure(device, response_sender))
            .map_err(|_| RecorderError::capture("Audio capture worker is unavailable"))?;
        response_receiver
            .recv_timeout(RECONFIGURE_TIMEOUT)
            .map_err(|_| RecorderError::capture("Audio device did not switch in time"))?
    }

    fn request_stop(&self, duration: f64) {
        let _ = self.commands.send(AudioCommand::Stop(duration));
    }

    fn try_error(&self) -> Option<RecorderError> {
        match self.health.try_recv() {
            Ok(error) => Some(error),
            Err(std::sync::mpsc::TryRecvError::Empty) => None,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => Some(RecorderError::capture(
                "Audio capture health channel disconnected",
            )),
        }
    }

    fn wait_for_finish(mut self) -> Result<StagedAsset, RecorderError> {
        let result = self.completion.recv().map_err(|_| {
            RecorderError::stop("Audio capture worker stopped without finalizing its asset")
        })?;
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        result
    }

    fn abort(mut self) {
        let _ = self.commands.send(AudioCommand::Abort);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for AudioTrack {
    fn drop(&mut self) {
        if self.thread.is_none() {
            return;
        }
        let _ = self.commands.send(AudioCommand::Abort);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn run_audio_worker(
    kind: AudioKind,
    output_path: PathBuf,
    device: AudioDevice,
    commands: Receiver<AudioCommand>,
    ready: Sender<Result<(), RecorderError>>,
    completion: Sender<Result<StagedAsset, RecorderError>>,
    health: Sender<RecorderError>,
    clock: Arc<Mutex<AudioClockState>>,
    muted: Arc<AtomicBool>,
) {
    let apartment = match ComApartment::initialize() {
        Ok(apartment) => apartment,
        Err(error) => {
            let failure = RecorderError::start(format!(
                "Failed to initialize {} capture: {error}",
                kind.label()
            ));
            let _ = ready.send(Err(failure.clone()));
            let _ = completion.send(Err(failure));
            return;
        }
    };

    let mut capture = match WasapiCapture::new(kind, device.id.as_deref(), device.name.as_deref()) {
        Ok(capture) => capture,
        Err(error) => {
            let _ = ready.send(Err(error.clone()));
            let _ = completion.send(Err(error));
            return;
        }
    };
    let encoder = match AudioEncoder::new(&output_path) {
        Ok(encoder) => encoder,
        Err(error) => {
            let _ = ready.send(Err(error.clone()));
            let _ = completion.send(Err(error));
            drop(capture);
            drop(apartment);
            return;
        }
    };

    if let Err(error) = capture.start() {
        let _ = ready.send(Err(error.clone()));
        let _ = completion.send(Err(error));
        encoder.abort();
        drop(capture);
        drop(apartment);
        return;
    }

    let _ = ready.send(Ok(()));
    let mut processor = AudioProcessor::new(encoder, capture.format, clock, muted);
    let mut capture = Some(capture);
    let mut fatal = false;
    let result = loop {
        match receive_audio_command(&commands, capture.is_some()) {
            Ok(Some(AudioCommand::Reconfigure(device, response))) => {
                let _ = response.send(reconfigure_capture(
                    kind,
                    device,
                    &mut capture,
                    &mut processor,
                ));
                continue;
            }
            Ok(Some(AudioCommand::Stop(duration))) => {
                capture = None;
                break processor.finish(duration);
            }
            Ok(Some(AudioCommand::Abort)) => {
                capture = None;
                processor.abort();
                break Err(RecorderError::stop("Audio recording was aborted"));
            }
            Ok(None) => {}
            Err(error) => {
                capture = None;
                processor.abort();
                fatal = true;
                break Err(error);
            }
        }

        let Some(active) = capture.as_ref() else {
            continue;
        };

        let wait = unsafe { WaitForSingleObject(active.event.handle, CAPTURE_WAIT_MS) };
        if wait == WAIT_TIMEOUT {
            continue;
        }
        if wait != WAIT_OBJECT_0 {
            capture = None;
            processor.abort();
            fatal = true;
            break Err(RecorderError::capture(format!(
                "{} capture event failed",
                kind.label()
            )));
        }
        if let Err(error) = active.drain(&mut processor) {
            capture = None;
            processor.abort();
            fatal = true;
            break Err(error);
        }
    };

    if fatal {
        if let Err(error) = &result {
            let _ = health.send(error.clone());
        }
    }
    let _ = completion.send(result);
    drop(capture);
    drop(apartment);
}

fn receive_audio_command(
    commands: &Receiver<AudioCommand>,
    is_capturing: bool,
) -> Result<Option<AudioCommand>, RecorderError> {
    if is_capturing {
        return match commands.try_recv() {
            Ok(command) => Ok(Some(command)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err(audio_controller_disconnected()),
        };
    }

    match commands.recv_timeout(IDLE_COMMAND_WAIT) {
        Ok(command) => Ok(Some(command)),
        Err(RecvTimeoutError::Timeout) => Ok(None),
        Err(RecvTimeoutError::Disconnected) => Err(audio_controller_disconnected()),
    }
}

fn audio_controller_disconnected() -> RecorderError {
    RecorderError::stop("Audio recording controller disconnected")
}

fn reconfigure_capture(
    kind: AudioKind,
    device: Option<AudioDevice>,
    capture: &mut Option<WasapiCapture>,
    processor: &mut AudioProcessor,
) -> Result<(), RecorderError> {
    *capture = None;
    let Some(device) = device else {
        return Ok(());
    };

    let mut active = WasapiCapture::new(kind, device.id.as_deref(), device.name.as_deref())?;
    active.start()?;
    processor.switch_format(active.format);
    *capture = Some(active);
    Ok(())
}

struct ComApartment;

impl ComApartment {
    fn initialize() -> Result<Self, windows::core::Error> {
        retain_process_mta()?;
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.ok()?;
        Ok(Self)
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct AudioFormat {
    pub(super) sample_rate: u32,
    pub(super) channels: u32,
    pub(super) bits_per_sample: u32,
    pub(super) valid_bits_per_sample: u32,
    pub(super) block_align: u32,
    pub(super) channel_mask: Option<u32>,
    pub(super) floating_point: bool,
}

struct WasapiCapture {
    client: IAudioClient,
    capture: IAudioCaptureClient,
    event: OwnedHandle,
    format: AudioFormat,
    started: bool,
}

impl WasapiCapture {
    fn new(
        kind: AudioKind,
        device_id: Option<&str>,
        device_name: Option<&str>,
    ) -> Result<Self, RecorderError> {
        let enumerator: IMMDeviceEnumerator =
            unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }.map_err(
                |error| {
                    RecorderError::configuration(format!(
                        "Failed to open Windows audio devices: {error}"
                    ))
                },
            )?;
        let device = select_device(&enumerator, kind, device_id, device_name)?;
        let client: IAudioClient =
            unsafe { device.Activate(CLSCTX_ALL, None) }.map_err(|error| {
                RecorderError::configuration(format!(
                    "Failed to open {} device: {error}",
                    kind.label()
                ))
            })?;
        let mix_format = unsafe { client.GetMixFormat() }.map_err(|error| {
            RecorderError::configuration(format!("Failed to read {} format: {error}", kind.label()))
        })?;
        if mix_format.is_null() {
            return Err(RecorderError::configuration(format!(
                "{} device did not provide a capture format",
                kind.label()
            )));
        }

        let format = unsafe { parse_audio_format(mix_format) };
        let flags = AUDCLNT_STREAMFLAGS_EVENTCALLBACK
            | if matches!(kind, AudioKind::System) {
                AUDCLNT_STREAMFLAGS_LOOPBACK
            } else {
                0
            };
        let initialize = unsafe {
            client.Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                flags,
                HNS_PER_SECOND,
                0,
                mix_format,
                None,
            )
        };
        unsafe {
            CoTaskMemFree(Some(mix_format as *const c_void));
        }
        let format = format?;
        initialize.map_err(|error| {
            RecorderError::configuration(format!(
                "Failed to initialize {} capture: {error}",
                kind.label()
            ))
        })?;

        let event = OwnedHandle::new(
            unsafe { CreateEventW(None, false, false, PCWSTR::null()) }.map_err(|error| {
                RecorderError::capture(format!(
                    "Failed to create {} capture event: {error}",
                    kind.label()
                ))
            })?,
        );
        unsafe { client.SetEventHandle(event.handle) }.map_err(|error| {
            RecorderError::capture(format!(
                "Failed to attach {} capture event: {error}",
                kind.label()
            ))
        })?;
        let capture: IAudioCaptureClient = unsafe { client.GetService() }.map_err(|error| {
            RecorderError::capture(format!(
                "Failed to open {} capture client: {error}",
                kind.label()
            ))
        })?;

        Ok(Self {
            client,
            capture,
            event,
            format,
            started: false,
        })
    }

    fn start(&mut self) -> Result<(), RecorderError> {
        unsafe { self.client.Start() }.map_err(|error| {
            RecorderError::start(format!("Failed to start Windows audio capture: {error}"))
        })?;
        self.started = true;
        Ok(())
    }

    fn stop(&mut self) {
        if !self.started {
            return;
        }
        unsafe {
            let _ = self.client.Stop();
        }
        self.started = false;
    }

    fn drain(&self, processor: &mut AudioProcessor) -> Result<(), RecorderError> {
        loop {
            let packet_size = unsafe { self.capture.GetNextPacketSize() }.map_err(|error| {
                RecorderError::capture(format!(
                    "Audio device became unavailable while recording: {error}"
                ))
            })?;
            if packet_size == 0 {
                return Ok(());
            }

            let mut data = std::ptr::null_mut();
            let mut frames = 0;
            let mut flags = 0;
            let mut device_position = 0;
            let mut qpc_position = 0;
            unsafe {
                self.capture.GetBuffer(
                    &mut data,
                    &mut frames,
                    &mut flags,
                    Some(&mut device_position),
                    Some(&mut qpc_position),
                )
            }
            .map_err(|error| {
                RecorderError::capture(format!(
                    "Audio device became unavailable while reading a packet: {error}"
                ))
            })?;

            let result = processor.process(AudioPacket {
                data,
                frames,
                flags,
                qpc_position: qpc_position.min(i64::MAX as u64) as i64,
            });
            let release = unsafe { self.capture.ReleaseBuffer(frames) }.map_err(|error| {
                RecorderError::capture(format!("Failed to release captured audio: {error}"))
            });
            result?;
            release?;
        }
    }
}

impl Drop for WasapiCapture {
    fn drop(&mut self) {
        self.stop();
    }
}

pub(super) struct OwnedHandle {
    pub(super) handle: HANDLE,
}

impl OwnedHandle {
    pub(super) fn new(handle: HANDLE) -> Self {
        Self { handle }
    }
}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}

pub(super) fn select_device(
    enumerator: &IMMDeviceEnumerator,
    kind: AudioKind,
    device_id: Option<&str>,
    device_name: Option<&str>,
) -> Result<IMMDevice, RecorderError> {
    if matches!(kind, AudioKind::System) {
        return unsafe { enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia) }.map_err(
            |error| {
                RecorderError::configuration(format!(
                    "No active system audio output is available: {error}"
                ))
            },
        );
    }

    for selector in microphone_selectors(device_id, device_name)
        .into_iter()
        .flatten()
    {
        match selector {
            MicrophoneSelector::Id(device_id) => {
                let wide = wide_string(device_id);
                if let Ok(device) = unsafe { enumerator.GetDevice(PCWSTR(wide.as_ptr())) } {
                    if unsafe { device.GetState() }.is_ok_and(|state| state == DEVICE_STATE_ACTIVE)
                    {
                        return Ok(device);
                    }
                }
            }
            MicrophoneSelector::Name(device_name) => {
                if let Some(device) = find_device_by_name(enumerator, device_name)? {
                    return Ok(device);
                }
            }
        }
    }

    unsafe { enumerator.GetDefaultAudioEndpoint(eCapture, eMultimedia) }.map_err(|error| {
        RecorderError::configuration(format!("No active microphone is available: {error}"))
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MicrophoneSelector<'a> {
    Id(&'a str),
    Name(&'a str),
}

fn microphone_selectors<'a>(
    device_id: Option<&'a str>,
    device_name: Option<&'a str>,
) -> [Option<MicrophoneSelector<'a>>; 2] {
    [
        device_id
            .filter(|value| !value.trim().is_empty())
            .map(MicrophoneSelector::Id),
        device_name
            .filter(|value| !value.trim().is_empty())
            .map(MicrophoneSelector::Name),
    ]
}

fn find_device_by_name(
    enumerator: &IMMDeviceEnumerator,
    requested_name: &str,
) -> Result<Option<IMMDevice>, RecorderError> {
    let collection = unsafe { enumerator.EnumAudioEndpoints(eCapture, DEVICE_STATE_ACTIVE) }
        .map_err(|error| {
            RecorderError::configuration(format!("Failed to list microphones: {error}"))
        })?;
    let count = unsafe { collection.GetCount() }.map_err(|error| {
        RecorderError::configuration(format!("Failed to count microphones: {error}"))
    })?;
    let requested = requested_name.to_lowercase();
    let mut partial = None;

    for index in 0..count {
        let device = unsafe { collection.Item(index) }.map_err(|error| {
            RecorderError::configuration(format!("Failed to inspect microphone: {error}"))
        })?;
        let Ok(Some(name)) = device_friendly_name(&device) else {
            continue;
        };
        let candidate = name.to_lowercase();
        if candidate == requested {
            return Ok(Some(device));
        }
        if partial.is_none() && (candidate.contains(&requested) || requested.contains(&candidate)) {
            partial = Some(device);
        }
    }
    Ok(partial)
}

fn device_friendly_name(device: &IMMDevice) -> Result<Option<String>, RecorderError> {
    let store = unsafe { device.OpenPropertyStore(STGM_READ) }.map_err(|error| {
        RecorderError::configuration(format!("Failed to open microphone properties: {error}"))
    })?;
    let value = unsafe { store.GetValue(&PKEY_Device_FriendlyName) }.map_err(|error| {
        RecorderError::configuration(format!("Failed to read microphone name: {error}"))
    })?;
    let text = unsafe { PropVariantToStringAlloc(&value) };
    let text = match text {
        Ok(text) => text,
        Err(_) => return Ok(None),
    };
    let result = if text.0.is_null() {
        None
    } else {
        unsafe { text.to_string() }.ok()
    };
    unsafe {
        CoTaskMemFree(Some(text.0 as *const c_void));
    }
    Ok(result)
}

pub(super) unsafe fn parse_audio_format(
    format: *const WAVEFORMATEX,
) -> Result<AudioFormat, RecorderError> {
    let wave = unsafe { std::ptr::read_unaligned(format) };
    if wave.nSamplesPerSec == 0 || wave.nChannels == 0 || wave.nBlockAlign == 0 {
        return Err(RecorderError::configuration(
            "Audio device reported an invalid capture format",
        ));
    }

    let mut floating_point = wave.wFormatTag == 3;
    let mut channel_mask = None;
    let mut valid_bits_per_sample = u32::from(wave.wBitsPerSample);
    if wave.wFormatTag == 0xfffe && usize::from(wave.cbSize) >= 22 {
        let extensible = format as *const WAVEFORMATEXTENSIBLE;
        let subtype =
            unsafe { std::ptr::read_unaligned(std::ptr::addr_of!((*extensible).SubFormat)) };
        let mask =
            unsafe { std::ptr::read_unaligned(std::ptr::addr_of!((*extensible).dwChannelMask)) };
        let samples =
            unsafe { std::ptr::read_unaligned(std::ptr::addr_of!((*extensible).Samples)) };
        floating_point = subtype == MFAudioFormat_Float;
        if subtype != MFAudioFormat_Float && subtype != MFAudioFormat_PCM {
            return Err(RecorderError::configuration(
                "Audio device uses an unsupported capture sample format",
            ));
        }
        channel_mask = Some(mask);
        let valid_bits = unsafe { samples.wValidBitsPerSample };
        if valid_bits > 0 {
            valid_bits_per_sample = u32::from(valid_bits);
        }
    } else if wave.wFormatTag != 1 && wave.wFormatTag != 3 {
        return Err(RecorderError::configuration(
            "Audio device uses an unsupported capture sample format",
        ));
    }
    if floating_point && wave.wBitsPerSample != 32 {
        return Err(RecorderError::configuration(
            "Audio device uses an unsupported floating-point format",
        ));
    }
    if !floating_point && !matches!(wave.wBitsPerSample, 8 | 16 | 24 | 32) {
        return Err(RecorderError::configuration(
            "Audio device uses an unsupported PCM sample width",
        ));
    }
    if valid_bits_per_sample == 0 || valid_bits_per_sample > u32::from(wave.wBitsPerSample) {
        return Err(RecorderError::configuration(
            "Audio device reported invalid sample precision",
        ));
    }
    if wave.nBlockAlign % wave.nChannels != 0 {
        return Err(RecorderError::configuration(
            "Audio device reported invalid channel alignment",
        ));
    }
    let bytes_per_channel = u32::from(wave.nBlockAlign) / u32::from(wave.nChannels);
    if bytes_per_channel < u32::from(wave.wBitsPerSample).div_ceil(8) {
        return Err(RecorderError::configuration(
            "Audio device reported invalid sample alignment",
        ));
    }

    Ok(AudioFormat {
        sample_rate: wave.nSamplesPerSec,
        channels: u32::from(wave.nChannels),
        bits_per_sample: u32::from(wave.wBitsPerSample),
        valid_bits_per_sample,
        block_align: u32::from(wave.nBlockAlign),
        channel_mask,
        floating_point,
    })
}

#[derive(Clone, Copy)]
struct AudioPacket {
    data: *mut u8,
    frames: u32,
    flags: u32,
    qpc_position: i64,
}

struct BufferedAudioPacket {
    data: Vec<u8>,
    frames: u32,
    flags: u32,
    qpc_position: i64,
}

struct AudioProcessor {
    encoder: Option<AudioEncoder>,
    format: AudioFormat,
    clock: Arc<Mutex<AudioClockState>>,
    muted: Arc<AtomicBool>,
    next_time: i64,
    last_source_end: Option<i64>,
    pending: VecDeque<BufferedAudioPacket>,
    pending_bytes: usize,
}

impl AudioProcessor {
    fn new(
        encoder: AudioEncoder,
        format: AudioFormat,
        clock: Arc<Mutex<AudioClockState>>,
        muted: Arc<AtomicBool>,
    ) -> Self {
        Self {
            encoder: Some(encoder),
            format,
            clock,
            muted,
            next_time: 0,
            last_source_end: None,
            pending: VecDeque::new(),
            pending_bytes: 0,
        }
    }

    fn switch_format(&mut self, format: AudioFormat) {
        self.format = format;
        self.last_source_end = None;
        self.pending.clear();
        self.pending_bytes = 0;
    }

    fn process(&mut self, packet: AudioPacket) -> Result<(), RecorderError> {
        if packet.frames == 0 {
            return Ok(());
        }
        let clock = self
            .clock
            .lock()
            .map_err(|_| RecorderError::capture("Audio timeline is unavailable"))?
            .clone();
        if clock.origin.is_none() {
            self.buffer_pending(packet)?;
            return Ok(());
        }
        self.flush_pending_with_clock(&clock)?;
        self.process_with_clock(packet, &clock)
    }

    fn process_with_clock(
        &mut self,
        packet: AudioPacket,
        clock: &AudioClockState,
    ) -> Result<(), RecorderError> {
        let timestamp_error = packet.flags & AUDCLNT_BUFFERFLAGS_TIMESTAMP_ERROR.0 as u32 != 0;
        let packet_start = if timestamp_error {
            self.last_source_end.unwrap_or(packet.qpc_position)
        } else {
            packet.qpc_position
        };
        self.last_source_end = Some(
            packet_start.saturating_add(frames_to_hns(packet.frames, self.format.sample_rate)),
        );
        let segments = clock.visible_segments(packet_start, packet.frames, self.format.sample_rate);
        let silent = packet.flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0
            || self.muted.load(Ordering::Acquire);
        let discontinuity = packet.flags & AUDCLNT_BUFFERFLAGS_DATA_DISCONTINUITY.0 as u32 != 0;

        for segment in segments {
            if discontinuity || segment.timestamp > self.next_time {
                self.fill_until(segment.timestamp)?;
            }
            self.write_segment(&packet, segment, silent)?;
        }
        Ok(())
    }

    fn buffer_pending(&mut self, packet: AudioPacket) -> Result<(), RecorderError> {
        let silent = packet.flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0;
        let length = usize::try_from(packet.frames)
            .unwrap_or(usize::MAX)
            .checked_mul(self.format.block_align as usize)
            .ok_or_else(|| RecorderError::capture("Captured audio packet is too large"))?;
        let data = if silent {
            Vec::new()
        } else {
            if packet.data.is_null() {
                return Err(RecorderError::capture("Captured audio packet is invalid"));
            }
            unsafe { std::slice::from_raw_parts(packet.data, length) }.to_vec()
        };
        self.pending_bytes = self.pending_bytes.saturating_add(data.len());
        self.pending.push_back(BufferedAudioPacket {
            data,
            frames: packet.frames,
            flags: packet.flags,
            qpc_position: packet.qpc_position,
        });
        while self.pending_bytes > MAX_PENDING_AUDIO_BYTES {
            let Some(oldest) = self.pending.pop_front() else {
                break;
            };
            self.pending_bytes = self.pending_bytes.saturating_sub(oldest.data.len());
        }
        Ok(())
    }

    fn flush_pending(&mut self) -> Result<(), RecorderError> {
        let clock = self
            .clock
            .lock()
            .map_err(|_| RecorderError::capture("Audio timeline is unavailable"))?
            .clone();
        self.flush_pending_with_clock(&clock)
    }

    fn flush_pending_with_clock(&mut self, clock: &AudioClockState) -> Result<(), RecorderError> {
        let pending = std::mem::take(&mut self.pending);
        self.pending_bytes = 0;
        for mut buffered in pending {
            let data = if buffered.data.is_empty() {
                std::ptr::null_mut()
            } else {
                buffered.data.as_mut_ptr()
            };
            self.process_with_clock(
                AudioPacket {
                    data,
                    frames: buffered.frames,
                    flags: buffered.flags,
                    qpc_position: buffered.qpc_position,
                },
                clock,
            )?;
        }
        Ok(())
    }

    fn write_segment(
        &mut self,
        packet: &AudioPacket,
        mut segment: AudioSegment,
        silent: bool,
    ) -> Result<(), RecorderError> {
        if segment.timestamp < self.next_time {
            let overlap = self.next_time - segment.timestamp;
            let trim =
                hns_to_frames_ceil(overlap, self.format.sample_rate).min(segment.frame_count);
            segment.first_frame += trim;
            segment.frame_count -= trim;
            segment.timestamp = segment
                .timestamp
                .saturating_add(frames_to_hns(trim, self.format.sample_rate));
        }
        if segment.frame_count == 0 {
            return Ok(());
        }

        let output_frames = resampled_frame_count(segment.frame_count, self.format.sample_rate);
        if output_frames == 0 {
            return Ok(());
        }
        let encoder = self
            .encoder
            .as_mut()
            .ok_or_else(|| RecorderError::capture("Audio encoder is unavailable"))?;
        if silent {
            encoder.write_silence(output_frames, segment.timestamp)?;
        } else {
            let byte_offset = usize::try_from(segment.first_frame)
                .unwrap_or(usize::MAX)
                .saturating_mul(self.format.block_align as usize);
            let byte_length = usize::try_from(segment.frame_count)
                .unwrap_or(usize::MAX)
                .saturating_mul(self.format.block_align as usize);
            if packet.data.is_null() || byte_length > u32::MAX as usize {
                return Err(RecorderError::capture("Captured audio packet is invalid"));
            }
            let data =
                unsafe { std::slice::from_raw_parts(packet.data.add(byte_offset), byte_length) };
            let pcm = convert_to_pcm16(data, segment.frame_count, self.format, output_frames)?;
            encoder.write(&pcm, output_frames, segment.timestamp)?;
        }
        self.next_time = segment
            .timestamp
            .saturating_add(frames_to_hns(output_frames, AAC_SAMPLE_RATE));
        Ok(())
    }

    fn fill_until(&mut self, target: i64) -> Result<(), RecorderError> {
        while self.next_time < target {
            let missing = target - self.next_time;
            let frames = hns_to_frames_floor(missing, AAC_SAMPLE_RATE).min(SILENCE_CHUNK_FRAMES);
            if frames == 0 {
                break;
            }
            self.encoder
                .as_mut()
                .ok_or_else(|| RecorderError::capture("Audio encoder is unavailable"))?
                .write_silence(frames, self.next_time)?;
            self.next_time = self
                .next_time
                .saturating_add(frames_to_hns(frames, AAC_SAMPLE_RATE));
        }
        Ok(())
    }

    fn finish(mut self, duration: f64) -> Result<StagedAsset, RecorderError> {
        self.flush_pending()?;
        let target = seconds_to_hns(duration);
        self.fill_until(target)?;
        let encoder = self
            .encoder
            .take()
            .ok_or_else(|| RecorderError::stop("Audio encoder is unavailable"))?;
        encoder.finish()
    }

    fn abort(mut self) {
        if let Some(encoder) = self.encoder.take() {
            encoder.abort();
        }
    }
}

struct AudioEncoder {
    writer: Option<IMFSinkWriter>,
    stream_index: u32,
    output_path: PathBuf,
    temporary_path: PathBuf,
    block_align: u32,
    sample_rate: u32,
    mf_started: bool,
    committed: bool,
}

impl AudioEncoder {
    fn new(output_path: &Path) -> Result<Self, RecorderError> {
        unsafe { MFStartup(MF_VERSION, MFSTARTUP_FULL) }.map_err(|error| {
            RecorderError::capture(format!("Failed to start Media Foundation audio: {error}"))
        })?;
        let temporary_path = match temporary_audio_path(output_path) {
            Ok(path) => path,
            Err(error) => {
                unsafe {
                    let _ = MFShutdown();
                }
                return Err(error);
            }
        };
        if temporary_path.exists() {
            if let Err(error) = std::fs::remove_file(&temporary_path) {
                unsafe {
                    let _ = MFShutdown();
                }
                return Err(RecorderError::capture(format!(
                    "Failed to replace temporary audio asset: {error}"
                )));
            }
        }
        let result = Self::create(output_path, temporary_path.clone());
        if result.is_err() {
            unsafe {
                let _ = MFShutdown();
            }
            let _ = std::fs::remove_file(temporary_path);
        }
        result
    }

    fn create(output_path: &Path, temporary_path: PathBuf) -> Result<Self, RecorderError> {
        let attributes = create_attributes(2)?;
        unsafe {
            attributes
                .SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 1)
                .map_err(audio_attribute_error)?;
            attributes
                .SetUINT32(&MF_SINK_WRITER_DISABLE_THROTTLING, 1)
                .map_err(audio_attribute_error)?;
        }
        let wide_path = wide_path(&temporary_path);
        let writer = unsafe {
            MFCreateSinkWriterFromURL(
                PCWSTR(wide_path.as_ptr()),
                None::<&IMFByteStream>,
                Some(&attributes),
            )
        }
        .map_err(|error| {
            RecorderError::capture(format!("Failed to create AAC sink writer: {error}"))
        })?;

        let output_type = unsafe { MFCreateMediaType() }.map_err(|error| {
            RecorderError::capture(format!("Failed to create AAC media type: {error}"))
        })?;
        unsafe {
            output_type
                .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)
                .map_err(audio_attribute_error)?;
            output_type
                .SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_AAC)
                .map_err(audio_attribute_error)?;
            output_type
                .SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, AAC_CHANNELS)
                .map_err(audio_attribute_error)?;
            output_type
                .SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, AAC_SAMPLE_RATE)
                .map_err(audio_attribute_error)?;
            output_type
                .SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, AAC_BIT_RATE / 8)
                .map_err(audio_attribute_error)?;
            output_type
                .SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, 1)
                .map_err(audio_attribute_error)?;
            output_type
                .SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, AAC_BITS_PER_SAMPLE)
                .map_err(audio_attribute_error)?;
            output_type
                .SetUINT32(&MF_MT_AAC_PAYLOAD_TYPE, 0)
                .map_err(audio_attribute_error)?;
            output_type
                .SetUINT32(&MF_MT_AAC_AUDIO_PROFILE_LEVEL_INDICATION, 0x29)
                .map_err(audio_attribute_error)?;
        }
        let stream_index = unsafe { writer.AddStream(&output_type) }.map_err(|error| {
            RecorderError::capture(format!("Failed to add AAC audio stream: {error}"))
        })?;

        let input_type = unsafe { MFCreateMediaType() }.map_err(|error| {
            RecorderError::capture(format!("Failed to create PCM media type: {error}"))
        })?;
        unsafe {
            input_type
                .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)
                .map_err(audio_attribute_error)?;
            input_type
                .SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_PCM)
                .map_err(audio_attribute_error)?;
            input_type
                .SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, AAC_CHANNELS)
                .map_err(audio_attribute_error)?;
            input_type
                .SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, AAC_SAMPLE_RATE)
                .map_err(audio_attribute_error)?;
            input_type
                .SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, AAC_BITS_PER_SAMPLE)
                .map_err(audio_attribute_error)?;
            input_type
                .SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, AAC_BLOCK_ALIGN)
                .map_err(audio_attribute_error)?;
            input_type
                .SetUINT32(
                    &MF_MT_AUDIO_AVG_BYTES_PER_SECOND,
                    AAC_SAMPLE_RATE * AAC_BLOCK_ALIGN,
                )
                .map_err(audio_attribute_error)?;
            writer
                .SetInputMediaType(stream_index, &input_type, None::<&IMFAttributes>)
                .map_err(|error| {
                    RecorderError::capture(format!(
                        "Failed to configure AAC input conversion: {error}"
                    ))
                })?;
            writer.BeginWriting().map_err(|error| {
                RecorderError::capture(format!("Failed to begin AAC encoding: {error}"))
            })?;
        }

        Ok(Self {
            writer: Some(writer),
            stream_index,
            output_path: output_path.to_path_buf(),
            temporary_path,
            block_align: AAC_BLOCK_ALIGN,
            sample_rate: AAC_SAMPLE_RATE,
            mf_started: true,
            committed: false,
        })
    }

    fn write_silence(&mut self, frames: u32, timestamp: i64) -> Result<(), RecorderError> {
        let length = frames
            .checked_mul(self.block_align)
            .ok_or_else(|| RecorderError::capture("Silent audio packet is too large"))?;
        let silence = vec![0_u8; length as usize];
        self.write(&silence, frames, timestamp)
    }

    fn write(&mut self, data: &[u8], frames: u32, timestamp: i64) -> Result<(), RecorderError> {
        let length = u32::try_from(data.len())
            .map_err(|_| RecorderError::capture("Captured audio packet is too large"))?;
        let buffer = unsafe { MFCreateMemoryBuffer(length) }.map_err(|error| {
            RecorderError::capture(format!("Failed to create AAC input buffer: {error}"))
        })?;
        let mut destination = std::ptr::null_mut();
        unsafe { buffer.Lock(&mut destination, None, None) }.map_err(|error| {
            RecorderError::capture(format!("Failed to lock AAC input buffer: {error}"))
        })?;
        if destination.is_null() {
            unsafe {
                let _ = buffer.Unlock();
            }
            return Err(RecorderError::capture("AAC input buffer is unavailable"));
        }
        unsafe {
            std::ptr::copy_nonoverlapping(data.as_ptr(), destination, data.len());
        }
        unsafe { buffer.Unlock() }.map_err(|error| {
            RecorderError::capture(format!("Failed to unlock AAC input buffer: {error}"))
        })?;
        unsafe { buffer.SetCurrentLength(length) }.map_err(|error| {
            RecorderError::capture(format!("Failed to size AAC input buffer: {error}"))
        })?;

        let sample = unsafe { MFCreateSample() }.map_err(|error| {
            RecorderError::capture(format!("Failed to create AAC input sample: {error}"))
        })?;
        unsafe {
            sample.AddBuffer(&buffer).map_err(|error| {
                RecorderError::capture(format!("Failed to attach AAC input buffer: {error}"))
            })?;
            sample.SetSampleTime(timestamp).map_err(|error| {
                RecorderError::capture(format!("Failed to set AAC sample time: {error}"))
            })?;
            sample
                .SetSampleDuration(frames_to_hns(frames, self.sample_rate))
                .map_err(|error| {
                    RecorderError::capture(format!("Failed to set AAC sample duration: {error}"))
                })?;
            self.writer
                .as_ref()
                .ok_or_else(|| RecorderError::capture("AAC sink writer is unavailable"))?
                .WriteSample(self.stream_index, &sample)
                .map_err(|error| {
                    RecorderError::capture(format!("Failed to encode AAC sample: {error}"))
                })?;
        }
        Ok(())
    }

    fn finish(mut self) -> Result<StagedAsset, RecorderError> {
        let writer = self
            .writer
            .take()
            .ok_or_else(|| RecorderError::stop("AAC sink writer is unavailable"))?;
        unsafe { writer.Finalize() }.map_err(|error| {
            RecorderError::stop(format!("Failed to finalize AAC recording: {error}"))
        })?;
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
        unsafe {
            let _ = MFShutdown();
        }
        self.mf_started = false;
    }
}

impl Drop for AudioEncoder {
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
    unsafe { MFCreateAttributes(&mut attributes, capacity) }.map_err(|error| {
        RecorderError::capture(format!("Failed to create audio writer attributes: {error}"))
    })?;
    attributes.ok_or_else(|| RecorderError::capture("Audio writer attributes were not created"))
}

fn audio_attribute_error(error: windows::core::Error) -> RecorderError {
    RecorderError::capture(format!("Failed to configure AAC media type: {error}"))
}

fn resampled_frame_count(source_frames: u32, source_rate: u32) -> u32 {
    let frames = u64::from(source_frames)
        .saturating_mul(u64::from(AAC_SAMPLE_RATE))
        .saturating_add(u64::from(source_rate) / 2)
        / u64::from(source_rate).max(1);
    frames.min(u64::from(u32::MAX)) as u32
}

fn convert_to_pcm16(
    data: &[u8],
    source_frames: u32,
    format: AudioFormat,
    output_frames: u32,
) -> Result<Vec<u8>, RecorderError> {
    let source_bytes = usize::try_from(source_frames)
        .unwrap_or(usize::MAX)
        .saturating_mul(format.block_align as usize);
    if data.len() < source_bytes {
        return Err(RecorderError::capture("Captured audio packet is truncated"));
    }
    let output_bytes = usize::try_from(output_frames)
        .unwrap_or(usize::MAX)
        .checked_mul(AAC_BLOCK_ALIGN as usize)
        .ok_or_else(|| RecorderError::capture("Converted audio packet is too large"))?;
    let mut output = Vec::with_capacity(output_bytes);
    if source_frames == 0 || output_frames == 0 {
        return Ok(output);
    }

    for output_frame in 0..output_frames {
        let source_position = if output_frames == 1 || source_frames == 1 {
            0.0
        } else {
            output_frame as f64 * (source_frames - 1) as f64 / (output_frames - 1) as f64
        };
        let lower = source_position.floor() as u32;
        let upper = (lower + 1).min(source_frames - 1);
        let fraction = (source_position - f64::from(lower)) as f32;
        let lower_sample = stereo_sample(data, lower, format)?;
        let upper_sample = stereo_sample(data, upper, format)?;
        let left = lower_sample.0 + (upper_sample.0 - lower_sample.0) * fraction;
        let right = lower_sample.1 + (upper_sample.1 - lower_sample.1) * fraction;
        output.extend_from_slice(&float_to_i16(left).to_le_bytes());
        output.extend_from_slice(&float_to_i16(right).to_le_bytes());
    }
    Ok(output)
}

fn stereo_sample(
    data: &[u8],
    frame: u32,
    format: AudioFormat,
) -> Result<(f32, f32), RecorderError> {
    if format.channels == 1 {
        let sample = channel_sample(data, frame, 0, format)?;
        return Ok((sample, sample));
    }

    let mut left = 0.0_f32;
    let mut right = 0.0_f32;
    for channel in 0..format.channels {
        let sample = channel_sample(data, frame, channel, format)?;
        let speaker = format
            .channel_mask
            .and_then(|mask| channel_speaker(mask, channel));
        match speaker {
            Some(0) => left += sample,
            Some(1) => right += sample,
            Some(2) => {
                left += sample * 0.707;
                right += sample * 0.707;
            }
            Some(3) => {
                left += sample * 0.25;
                right += sample * 0.25;
            }
            Some(4 | 9) => left += sample * 0.707,
            Some(5 | 10) => right += sample * 0.707,
            Some(8) => {
                left += sample * 0.5;
                right += sample * 0.5;
            }
            Some(_) => {
                left += sample * 0.35;
                right += sample * 0.35;
            }
            None if channel == 0 => left += sample,
            None if channel == 1 => right += sample,
            None => {
                left += sample * 0.25;
                right += sample * 0.25;
            }
        }
    }
    Ok((left.clamp(-1.0, 1.0), right.clamp(-1.0, 1.0)))
}

fn channel_speaker(mask: u32, channel: u32) -> Option<u32> {
    let mut index = 0;
    for bit in 0..32 {
        if mask & (1 << bit) == 0 {
            continue;
        }
        if index == channel {
            return Some(bit);
        }
        index += 1;
    }
    None
}

pub(super) fn channel_sample(
    data: &[u8],
    frame: u32,
    channel: u32,
    format: AudioFormat,
) -> Result<f32, RecorderError> {
    let channel_stride = format.block_align / format.channels;
    let offset = usize::try_from(frame)
        .unwrap_or(usize::MAX)
        .saturating_mul(format.block_align as usize)
        .saturating_add(
            usize::try_from(channel)
                .unwrap_or(usize::MAX)
                .saturating_mul(channel_stride as usize),
        );
    let width = format.bits_per_sample.div_ceil(8) as usize;
    let end = offset.saturating_add(width);
    let sample = data
        .get(offset..end)
        .ok_or_else(|| RecorderError::capture("Captured audio sample is truncated"))?;

    if format.floating_point {
        return Ok(
            f32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]).clamp(-1.0, 1.0),
        );
    }
    if format.bits_per_sample == 8 {
        return Ok((f32::from(sample[0]) - 128.0) / 128.0);
    }

    let mut value = 0_i64;
    for (index, byte) in sample.iter().enumerate() {
        value |= i64::from(*byte) << (index * 8);
    }
    let container_bits = format.bits_per_sample;
    let sign_bit = 1_i64 << (container_bits - 1);
    if value & sign_bit != 0 {
        value |= !0_i64 << container_bits;
    }
    if format.valid_bits_per_sample < container_bits {
        value >>= container_bits - format.valid_bits_per_sample;
    }
    let scale = (1_u64 << (format.valid_bits_per_sample - 1)) as f32;
    Ok((value as f32 / scale).clamp(-1.0, 1.0))
}

fn float_to_i16(sample: f32) -> i16 {
    (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16
}

fn temporary_audio_path(output_path: &Path) -> Result<PathBuf, RecorderError> {
    let name = output_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| RecorderError::configuration("Audio asset path has an invalid file name"))?;
    Ok(output_path.with_file_name(format!(".{name}.poratake-partial.m4a")))
}

fn wide_string(value: &str) -> Vec<u16> {
    std::ffi::OsStr::new(value)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

fn wide_path(path: &Path) -> Vec<u16> {
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

fn frames_to_hns(frames: u32, sample_rate: u32) -> i64 {
    i64::from(frames)
        .saturating_mul(HNS_PER_SECOND)
        .saturating_div(i64::from(sample_rate).max(1))
}

fn hns_to_frames_floor(duration: i64, sample_rate: u32) -> u32 {
    if duration <= 0 {
        return 0;
    }
    let frames = duration
        .saturating_mul(i64::from(sample_rate))
        .saturating_div(HNS_PER_SECOND);
    frames.clamp(0, i64::from(u32::MAX)) as u32
}

fn hns_to_frames_ceil(duration: i64, sample_rate: u32) -> u32 {
    if duration <= 0 {
        return 0;
    }
    let numerator = duration.saturating_mul(i64::from(sample_rate));
    let frames = numerator
        .saturating_add(HNS_PER_SECOND - 1)
        .saturating_div(HNS_PER_SECOND);
    frames.clamp(0, i64::from(u32::MAX)) as u32
}

fn seconds_to_hns(seconds: f64) -> i64 {
    if !seconds.is_finite() || seconds <= 0.0 {
        return 0;
    }
    (seconds * HNS_PER_SECOND as f64)
        .round()
        .clamp(0.0, i64::MAX as f64) as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn microphone_selection_prefers_endpoint_id_before_friendly_name() {
        assert_eq!(
            microphone_selectors(Some("endpoint-id"), Some("Microphone")),
            [
                Some(MicrophoneSelector::Id("endpoint-id")),
                Some(MicrophoneSelector::Name("Microphone")),
            ]
        );
    }

    #[test]
    fn microphone_selection_ignores_empty_values() {
        assert_eq!(microphone_selectors(Some(" "), Some("")), [None, None]);
    }
}
