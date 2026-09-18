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

pub fn system_default_label(devices: &[MediaDevice], default_id: Option<&str>) -> String {
    let resolved = default_id
        .filter(|id| !id.is_empty())
        .and_then(|id| devices.iter().find(|device| device.id == id));
    match resolved {
        Some(device) if !device.label.trim().is_empty() => {
            format!("System Default ({})", device.label)
        }
        _ => "System Default".to_string(),
    }
}

pub fn options_with_selection(
    devices: &[MediaDevice],
    selected: Option<&str>,
    selected_name: Option<&str>,
    default_id: Option<&str>,
) -> Vec<(String, String)> {
    let mut result = vec![(String::new(), system_default_label(devices, default_id))];
    result.extend(options(devices));
    let Some(selected_id) = selected.filter(|id| !id.is_empty()) else {
        return result;
    };
    if devices.iter().any(|device| device.id == selected_id) {
        return result;
    }
    let name = selected_name
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or("Unknown device");
    result.push((selected_id.to_string(), format!("{name} (unavailable)")));
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
        let options = options_with_selection(&[], Some("missing"), Some("Studio Mic"), None);
        assert_eq!(options[0], (String::new(), "System Default".into()));
        assert_eq!(
            options[1],
            ("missing".into(), "Studio Mic (unavailable)".into())
        );
    }

    #[test]
    fn a_disconnected_selection_without_a_name_reads_as_unknown() {
        let options = options_with_selection(&[], Some("missing"), None, None);
        assert_eq!(
            options[1],
            ("missing".into(), "Unknown device (unavailable)".into())
        );
    }

    #[test]
    fn the_first_option_names_the_resolved_system_default() {
        let devices = vec![MediaDevice {
            id: "mic-1".into(),
            label: "MacBook Pro Microphone".into(),
        }];
        let options = options_with_selection(&devices, None, None, Some("mic-1"));
        assert_eq!(options[0].1, "System Default (MacBook Pro Microphone)");
        assert_eq!(options.len(), 2);

        let unresolved = options_with_selection(&devices, None, None, Some("mic-9"));
        assert_eq!(unresolved[0].1, "System Default");
    }
}
