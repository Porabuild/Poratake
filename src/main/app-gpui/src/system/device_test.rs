//! The microphone and camera tests on the Devices settings page.
//!
//! `devices/index.ts` drives both from the settings window: the mic test calls
//! `media-devices startMicTest` and streams levels back on the
//! `media-devices:mic-level` event, and the camera test shows the same floating
//! preview window that appears while recording, through `camera-preview show`.
//! Neither had a client here, so both buttons were simply missing from the page.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::OnceLock;

use poratake_daemon_common::contract::{CameraPreviewRequest, MicrophoneTestRequest};

use crate::daemon::DaemonHandle;

/// The most recent level, as `f32` bits. An atomic rather than a lock because it
/// is written from the daemon's reader thread and read once per frame.
static LEVEL: AtomicU32 = AtomicU32::new(0);
static MIC_ACTIVE: AtomicBool = AtomicBool::new(false);
static CAMERA_ACTIVE: AtomicBool = AtomicBool::new(false);
static SUBSCRIBED: OnceLock<()> = OnceLock::new();

/// `(data as { level?: number })?.level ?? 0`, clamped -- the meter multiplies
/// this by its segment count, so a stray value out of range would overflow it.
pub fn decode_level(data: &serde_json::Value) -> f32 {
    data.get("level")
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(0.0)
        .clamp(0.0, 1.0) as f32
}

pub fn level() -> f32 {
    f32::from_bits(LEVEL.load(Ordering::Relaxed))
}

pub fn mic_active() -> bool {
    MIC_ACTIVE.load(Ordering::Relaxed)
}

pub fn camera_active() -> bool {
    CAMERA_ACTIVE.load(Ordering::Relaxed)
}

/// `forwardMicLevel` drops levels once the session is over, so the meter reads
/// zero rather than freezing on its last value.
fn subscribe(daemon: &DaemonHandle) {
    if SUBSCRIBED.get().is_some() {
        return;
    }
    let _ = SUBSCRIBED.set(());
    daemon.on_event(std::sync::Arc::new(|event, data| {
        if event != "media-devices:mic-level" {
            return;
        }
        if !MIC_ACTIVE.load(Ordering::Relaxed) {
            return;
        }
        LEVEL.store(decode_level(data).to_bits(), Ordering::Relaxed);
    }));
}

pub fn start_mic_test(daemon: &DaemonHandle, device_id: Option<&str>, device_name: Option<&str>) {
    if !crate::system::permissions::ensure_access(crate::system::permissions::Device::Microphone) {
        return;
    }
    subscribe(daemon);
    MIC_ACTIVE.store(true, Ordering::Relaxed);
    LEVEL.store(0f32.to_bits(), Ordering::Relaxed);
    let request = MicrophoneTestRequest {
        device_id: device_id.map(str::to_owned),
        device_name: device_name.map(str::to_owned),
    };
    if let Err(error) = daemon.media_devices().start_mic_test(&request) {
        eprintln!("[devices] startMicTest failed: {error}");
        MIC_ACTIVE.store(false, Ordering::Relaxed);
    }
}

pub fn stop_mic_test(daemon: &DaemonHandle) {
    if !MIC_ACTIVE.swap(false, Ordering::Relaxed) {
        return;
    }
    LEVEL.store(0f32.to_bits(), Ordering::Relaxed);
    if let Err(error) = daemon.media_devices().stop_mic_test() {
        eprintln!("[devices] stopMicTest failed: {error}");
    }
}

pub fn start_camera_test(
    daemon: &DaemonHandle,
    device_id: Option<&str>,
    device_name: Option<&str>,
    flipped: bool,
) {
    if !crate::system::permissions::ensure_access(crate::system::permissions::Device::Camera) {
        return;
    }
    CAMERA_ACTIVE.store(true, Ordering::Relaxed);
    let request = camera_preview_request(device_id, device_name, flipped);
    if let Err(error) = daemon.camera_preview().show(&request) {
        eprintln!("[devices] camera-preview show failed: {error}");
        CAMERA_ACTIVE.store(false, Ordering::Relaxed);
    }
}

fn camera_preview_request(
    device_id: Option<&str>,
    device_name: Option<&str>,
    flipped: bool,
) -> CameraPreviewRequest {
    CameraPreviewRequest {
        device_id: device_id.map(str::to_owned),
        device_name: device_name.map(str::to_owned),
        flipped: Some(flipped),
        ..CameraPreviewRequest::default()
    }
}

pub fn stop_camera_test(daemon: &DaemonHandle) {
    if !CAMERA_ACTIVE.swap(false, Ordering::Relaxed) {
        return;
    }
    if let Err(error) = daemon.camera_preview().hide() {
        eprintln!("[devices] camera-preview hide failed: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_missing_or_out_of_range_level_reads_as_something_the_meter_can_use() {
        assert_eq!(decode_level(&json!({})), 0.0);
        assert_eq!(decode_level(&json!({ "level": 0.5 })), 0.5);
        assert_eq!(decode_level(&json!({ "level": 4.0 })), 1.0);
        assert_eq!(decode_level(&json!({ "level": -1.0 })), 0.0);
        assert_eq!(decode_level(&json!({ "level": "loud" })), 0.0);
    }

    /// `LEVEL` round-trips through its bit pattern, which is the only reason the
    /// meter can read it without a lock.
    #[test]
    fn the_level_survives_the_atomic() {
        for value in [0.0f32, 0.125, 0.5, 1.0] {
            LEVEL.store(value.to_bits(), Ordering::Relaxed);
            assert_eq!(level(), value);
        }
        LEVEL.store(0f32.to_bits(), Ordering::Relaxed);
    }

    #[test]
    fn camera_test_forwards_the_selected_device() {
        assert_eq!(
            serde_json::to_value(camera_preview_request(
                Some("camera-id"),
                Some("External Camera"),
                true,
            ))
            .expect("camera preview request"),
            json!({
                "deviceId": "camera-id",
                "deviceName": "External Camera",
                "flipped": true,
            })
        );
    }
}
