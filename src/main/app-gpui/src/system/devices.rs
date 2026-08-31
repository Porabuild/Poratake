//! Port of `src/main/devices/index.ts` — the daemon's microphone and camera
//! lists, used by the Devices settings page and the recording control bar.

pub use poratake_daemon_common::contract::{
    MediaDevice, MediaDeviceKind as DeviceKind, MediaDeviceLists,
};

use crate::daemon::DaemonHandle;

/// The Swift daemon asks for TCC authorization per kind, so a caller that only
/// needs microphones must not trigger a camera prompt.
pub fn list(daemon: &DaemonHandle, kinds: &[DeviceKind]) -> MediaDeviceLists {
    match daemon.media_devices().list(kinds) {
        Ok(response) => response,
        Err(error) => {
            eprintln!("[devices] list failed: {error}");
            MediaDeviceLists::default()
        }
    }
}

pub fn list_ios(daemon: &DaemonHandle) -> Vec<MediaDevice> {
    match daemon.recording_control().list_ios_devices() {
        Ok(response) => response,
        Err(error) => {
            eprintln!("[devices] iOS device list failed: {error}");
            Vec::new()
        }
    }
}

pub fn options(devices: &[MediaDevice]) -> Vec<(String, String)> {
    devices
        .iter()
        .map(|device| {
            let label = if device.label.trim().is_empty() {
                device.id.clone()
            } else {
                device.label.clone()
            };
            (device.id.clone(), label)
        })
        .collect()
}

pub fn options_with_selection(
    devices: &[MediaDevice],
    selected: Option<&str>,
) -> Vec<(String, String)> {
    let mut result = vec![(String::new(), "System Default".to_string())];
    result.extend(options(devices));
    if let Some(selected_id) = selected {
        if !selected_id.is_empty() && !devices.iter().any(|device| device.id == selected_id) {
            result.push((
                selected_id.to_string(),
                format!("Unavailable ({selected_id})"),
            ));
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_the_daemon_payload() {
        let payload = json!({
            "microphones": [{ "id": "mic-1", "label": "Built-in" }],
            "cameras": [],
            "defaultMicrophoneId": "mic-1",
            "defaultCameraId": null
        });
        let parsed: MediaDeviceLists = serde_json::from_value(payload).expect("parse");
        assert_eq!(parsed.microphones.len(), 1);
        assert_eq!(parsed.default_microphone_id.as_deref(), Some("mic-1"));
        assert!(parsed.default_camera_id.is_none());
    }

    #[test]
    fn unlabelled_devices_fall_back_to_their_id() {
        let devices = vec![
            MediaDevice {
                id: "a".into(),
                label: "Mic A".into(),
            },
            MediaDevice {
                id: "b".into(),
                label: "  ".into(),
            },
        ];
        let options = options(&devices);
        assert_eq!(options[0].1, "Mic A");
        assert_eq!(options[1].1, "b");
    }

    #[test]
    fn device_options_keep_default_and_disconnected_selection() {
        let options = options_with_selection(&[], Some("missing"));
        assert_eq!(options[0], (String::new(), "System Default".into()));
        assert_eq!(
            options[1],
            ("missing".into(), "Unavailable (missing)".into())
        );
    }
}
