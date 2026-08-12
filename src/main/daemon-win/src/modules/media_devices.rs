use super::recording_audio::{
    channel_sample, parse_audio_format, select_device, AudioFormat, AudioKind, OwnedHandle,
    HNS_PER_SECOND,
};
use crate::com::retain_process_mta;
use crate::protocol::{param_str, send_event, Request};
use crate::router::{method_not_found, Module, Reply};
use serde_json::json;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use windows::core::{Result, PCWSTR, PWSTR};
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Foundation::{WAIT_OBJECT_0, WAIT_TIMEOUT};
use windows::Win32::Media::Audio::{
    eCapture, eMultimedia, IAudioCaptureClient, IAudioClient, IMMDevice, IMMDeviceEnumerator,
    MMDeviceEnumerator, AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_SHAREMODE_SHARED,
    AUDCLNT_STREAMFLAGS_EVENTCALLBACK, DEVICE_STATE_ACTIVE,
};
use windows::Win32::Media::MediaFoundation::{
    IMFActivate, MFCreateAttributes, MFEnumDeviceSources, MFShutdown, MFStartup, MFSTARTUP_FULL,
    MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME, MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE,
    MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID,
    MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK, MF_VERSION,
};
use windows::Win32::System::Com::StructuredStorage::{PropVariantClear, PropVariantToStringAlloc};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_ALL,
    COINIT_MULTITHREADED, STGM_READ,
};
use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject};

const MIC_LEVEL_EVENT: &str = "media-devices:mic-level";
const MIC_LEVEL_INTERVAL: Duration = Duration::from_millis(50);
const MIC_LEVEL_WAIT_MS: u32 = 100;
const MIC_LEVEL_FLOOR_DB: f32 = 60.0;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MediaDevice {
    pub id: String,
    pub label: String,
}

struct ComApartment;

impl ComApartment {
    fn initialize() -> Result<Self> {
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

struct MediaFoundation;

impl MediaFoundation {
    fn initialize() -> Result<Self> {
        unsafe { MFStartup(MF_VERSION, MFSTARTUP_FULL) }?;
        Ok(Self)
    }
}

impl Drop for MediaFoundation {
    fn drop(&mut self) {
        unsafe {
            let _ = MFShutdown();
        }
    }
}

pub fn enumerate_microphones() -> Result<Vec<MediaDevice>> {
    let _apartment = ComApartment::initialize()?;
    let enumerator: IMMDeviceEnumerator =
        unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }?;
    let collection = unsafe { enumerator.EnumAudioEndpoints(eCapture, DEVICE_STATE_ACTIVE) }?;
    let count = unsafe { collection.GetCount() }?;
    let mut devices = Vec::with_capacity(count as usize);

    for index in 0..count {
        let Ok(device) = (unsafe { collection.Item(index) }) else {
            continue;
        };
        let Ok(id) = microphone_id(&device) else {
            continue;
        };
        let Ok(label) = microphone_label(&device) else {
            continue;
        };
        if id.is_empty() || label.trim().is_empty() {
            continue;
        }
        devices.push(MediaDevice { id, label });
    }

    Ok(devices)
}

pub fn enumerate_cameras() -> Result<Vec<MediaDevice>> {
    let _apartment = ComApartment::initialize()?;
    let _media_foundation = MediaFoundation::initialize()?;
    let mut attributes = None;
    unsafe { MFCreateAttributes(&mut attributes, 1) }?;
    let Some(attributes) = attributes else {
        return Ok(Vec::new());
    };
    unsafe {
        attributes.SetGUID(
            &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE,
            &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID,
        )
    }?;

    let mut raw = std::ptr::null_mut();
    let mut count = 0;
    unsafe { MFEnumDeviceSources(&attributes, &mut raw, &mut count) }?;
    let activations = take_activations(raw, count);
    let mut devices = Vec::with_capacity(activations.len());

    for activation in &activations {
        let Ok(id) = camera_attribute(
            activation,
            &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK,
        ) else {
            continue;
        };
        let Ok(label) = camera_attribute(activation, &MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME) else {
            continue;
        };
        if id.is_empty() || label.trim().is_empty() {
            continue;
        }
        devices.push(MediaDevice { id, label });
    }

    Ok(devices)
}

fn microphone_id(device: &IMMDevice) -> Result<String> {
    let value = unsafe { device.GetId() }?;
    allocated_string(value, None)
}

fn microphone_label(device: &IMMDevice) -> Result<String> {
    let store = unsafe { device.OpenPropertyStore(STGM_READ) }?;
    let mut value = unsafe { store.GetValue(&PKEY_Device_FriendlyName) }?;
    let text = unsafe { PropVariantToStringAlloc(&value) };
    unsafe {
        let _ = PropVariantClear(&mut value);
    }
    allocated_string(text?, None)
}

fn camera_attribute(activation: &IMFActivate, key: &windows::core::GUID) -> Result<String> {
    let mut value = PWSTR::null();
    let mut length = 0;
    unsafe { activation.GetAllocatedString(key, &mut value, &mut length) }?;
    allocated_string(value, Some(length))
}

fn allocated_string(value: PWSTR, length: Option<u32>) -> Result<String> {
    if value.is_null() {
        return Ok(String::new());
    }
    let result = match length {
        Some(length) => unsafe {
            String::from_utf16_lossy(std::slice::from_raw_parts(value.0, length as usize))
        },
        None => unsafe { value.to_string().unwrap_or_default() },
    };
    unsafe {
        CoTaskMemFree(Some(value.0.cast::<c_void>()));
    }
    Ok(result)
}

fn take_activations(raw: *mut Option<IMFActivate>, count: u32) -> Vec<IMFActivate> {
    if raw.is_null() {
        return Vec::new();
    }
    let activations = unsafe {
        std::slice::from_raw_parts_mut(raw, count as usize)
            .iter_mut()
            .filter_map(Option::take)
            .collect()
    };
    unsafe {
        CoTaskMemFree(Some(raw.cast::<c_void>()));
    }
    activations
}

pub struct MediaDevicesModule {
    mic_test: Option<MicTest>,
}

struct MicTest {
    stop: Arc<AtomicBool>,
    thread: JoinHandle<()>,
}

impl MediaDevicesModule {
    pub fn new() -> Self {
        Self { mic_test: None }
    }

    fn stop_mic_test(&mut self) {
        let Some(test) = self.mic_test.take() else {
            return;
        };
        test.stop.store(true, Ordering::Release);
        let _ = test.thread.join();
    }

    fn start_mic_test(
        &mut self,
        device_id: Option<String>,
        device_name: Option<String>,
    ) -> std::result::Result<(), String> {
        self.stop_mic_test();

        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let (ready_sender, ready_receiver) = std::sync::mpsc::channel();
        let thread = std::thread::spawn(move || {
            run_mic_level_worker(device_id, device_name, worker_stop, ready_sender);
        });

        match ready_receiver.recv() {
            Ok(Ok(())) => {
                self.mic_test = Some(MicTest { stop, thread });
                Ok(())
            }
            Ok(Err(message)) => {
                let _ = thread.join();
                Err(message)
            }
            Err(_) => {
                let _ = thread.join();
                Err("Microphone level worker did not start".to_string())
            }
        }
    }
}

impl Module for MediaDevicesModule {
    fn name(&self) -> &'static str {
        "media-devices"
    }

    fn handle(&mut self, request: &Request) -> Reply {
        match request.method.as_str() {
            "list" => {
                let microphones = enumerate_microphones().unwrap_or_default();
                let cameras = enumerate_cameras().unwrap_or_default();
                Reply::Now(Ok(Some(json!({
                    "microphones": devices_json(&microphones),
                    "cameras": devices_json(&cameras),
                    "defaultMicrophoneId": default_microphone_id(),
                    "defaultCameraId": cameras.first().map(|device| device.id.clone()),
                }))))
            }
            "startMicTest" => {
                let device_id = param_str(&request.params, "deviceId").map(str::to_owned);
                let device_name = param_str(&request.params, "deviceName").map(str::to_owned);
                match self.start_mic_test(device_id, device_name) {
                    Ok(()) => Reply::Now(Ok(Some(json!({ "running": true })))),
                    Err(message) => Reply::Now(Err(("MIC_TEST_ERROR".to_string(), message))),
                }
            }
            "stopMicTest" => {
                self.stop_mic_test();
                Reply::Now(Ok(Some(json!({ "running": false }))))
            }
            method => method_not_found(method),
        }
    }
}

fn devices_json(devices: &[MediaDevice]) -> Vec<serde_json::Value> {
    devices
        .iter()
        .map(|device| json!({ "id": device.id, "label": device.label }))
        .collect()
}

fn default_microphone_id() -> Option<String> {
    let _apartment = ComApartment::initialize().ok()?;
    let enumerator: IMMDeviceEnumerator =
        unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }.ok()?;
    let device = unsafe { enumerator.GetDefaultAudioEndpoint(eCapture, eMultimedia) }.ok()?;
    microphone_id(&device).ok().filter(|id| !id.is_empty())
}

fn run_mic_level_worker(
    device_id: Option<String>,
    device_name: Option<String>,
    stop: Arc<AtomicBool>,
    ready: Sender<std::result::Result<(), String>>,
) {
    let _apartment = match ComApartment::initialize() {
        Ok(apartment) => apartment,
        Err(error) => {
            let _ = ready.send(Err(format!(
                "Failed to initialize microphone monitoring: {error}"
            )));
            return;
        }
    };

    let capture = match MicLevelCapture::open(device_id.as_deref(), device_name.as_deref()) {
        Ok(capture) => capture,
        Err(message) => {
            let _ = ready.send(Err(message));
            return;
        }
    };

    let _ = ready.send(Ok(()));
    capture.run(&stop);
    drop(capture);
    send_event(MIC_LEVEL_EVENT, Some(json!({ "level": 0.0 })));
}

struct MicLevelCapture {
    client: IAudioClient,
    capture: IAudioCaptureClient,
    event: OwnedHandle,
    format: AudioFormat,
}

impl MicLevelCapture {
    fn open(
        device_id: Option<&str>,
        device_name: Option<&str>,
    ) -> std::result::Result<Self, String> {
        let enumerator: IMMDeviceEnumerator =
            unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }
                .map_err(|error| format!("Failed to open Windows audio devices: {error}"))?;
        let device = select_device(&enumerator, AudioKind::Microphone, device_id, device_name)
            .map_err(|error| error.message)?;
        let client: IAudioClient = unsafe { device.Activate(CLSCTX_ALL, None) }
            .map_err(|error| format!("Failed to open microphone: {error}"))?;
        let mix_format = unsafe { client.GetMixFormat() }
            .map_err(|error| format!("Failed to read microphone format: {error}"))?;
        if mix_format.is_null() {
            return Err("Microphone did not provide a capture format".to_string());
        }

        let format = unsafe { parse_audio_format(mix_format) };
        let initialize = unsafe {
            client.Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
                HNS_PER_SECOND,
                0,
                mix_format,
                None,
            )
        };
        unsafe {
            CoTaskMemFree(Some(mix_format as *const c_void));
        }
        let format = format.map_err(|error| error.message)?;
        initialize
            .map_err(|error| format!("Failed to initialize microphone monitoring: {error}"))?;

        let event = OwnedHandle::new(
            unsafe { CreateEventW(None, false, false, PCWSTR::null()) }
                .map_err(|error| format!("Failed to create microphone event: {error}"))?,
        );
        unsafe { client.SetEventHandle(event.handle) }
            .map_err(|error| format!("Failed to attach microphone event: {error}"))?;
        let capture: IAudioCaptureClient = unsafe { client.GetService() }
            .map_err(|error| format!("Failed to open microphone capture client: {error}"))?;
        unsafe { client.Start() }
            .map_err(|error| format!("Failed to start microphone monitoring: {error}"))?;

        Ok(Self {
            client,
            capture,
            event,
            format,
        })
    }

    fn run(&self, stop: &AtomicBool) {
        let mut window = LevelWindow::new();

        while !stop.load(Ordering::Acquire) {
            let wait = unsafe { WaitForSingleObject(self.event.handle, MIC_LEVEL_WAIT_MS) };
            if wait != WAIT_OBJECT_0 && wait != WAIT_TIMEOUT {
                return;
            }
            if wait == WAIT_OBJECT_0 && self.drain(&mut window).is_err() {
                return;
            }
            window.emit_if_due();
        }
    }

    fn drain(&self, window: &mut LevelWindow) -> std::result::Result<(), ()> {
        loop {
            let packet_size = unsafe { self.capture.GetNextPacketSize() }.map_err(|_| ())?;
            if packet_size == 0 {
                return Ok(());
            }

            let mut data = std::ptr::null_mut();
            let mut frames = 0;
            let mut flags = 0;
            unsafe {
                self.capture
                    .GetBuffer(&mut data, &mut frames, &mut flags, None, None)
            }
            .map_err(|_| ())?;

            let accumulate = self.accumulate(window, data, frames, flags);
            let release = unsafe { self.capture.ReleaseBuffer(frames) }.map_err(|_| ());
            accumulate?;
            release?;
        }
    }

    fn accumulate(
        &self,
        window: &mut LevelWindow,
        data: *mut u8,
        frames: u32,
        flags: u32,
    ) -> std::result::Result<(), ()> {
        if frames == 0 {
            return Ok(());
        }

        let samples = u64::from(frames).saturating_mul(u64::from(self.format.channels));
        let silent = flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0;
        if silent || data.is_null() {
            window.add_silence(samples);
            return Ok(());
        }

        let length = (frames as usize).saturating_mul(self.format.block_align as usize);
        let bytes = unsafe { std::slice::from_raw_parts(data, length) };
        for frame in 0..frames {
            for channel in 0..self.format.channels {
                let sample = channel_sample(bytes, frame, channel, self.format).map_err(|_| ())?;
                window.add_sample(sample);
            }
        }
        Ok(())
    }
}

impl Drop for MicLevelCapture {
    fn drop(&mut self) {
        unsafe {
            let _ = self.client.Stop();
        }
    }
}

struct LevelWindow {
    sum: f64,
    count: u64,
    last_emit: Instant,
}

impl LevelWindow {
    fn new() -> Self {
        Self {
            sum: 0.0,
            count: 0,
            last_emit: Instant::now(),
        }
    }

    fn add_sample(&mut self, sample: f32) {
        self.sum += f64::from(sample) * f64::from(sample);
        self.count = self.count.saturating_add(1);
    }

    fn add_silence(&mut self, samples: u64) {
        self.count = self.count.saturating_add(samples);
    }

    fn emit_if_due(&mut self) {
        if self.count == 0 || self.last_emit.elapsed() < MIC_LEVEL_INTERVAL {
            return;
        }

        let rms = (self.sum / self.count as f64).sqrt() as f32;
        let db = 20.0 * rms.max(0.0001).log10();
        let level = ((db + MIC_LEVEL_FLOOR_DB) / MIC_LEVEL_FLOOR_DB).clamp(0.0, 1.0);
        send_event(MIC_LEVEL_EVENT, Some(json!({ "level": level })));

        self.sum = 0.0;
        self.count = 0;
        self.last_emit = Instant::now();
    }
}
