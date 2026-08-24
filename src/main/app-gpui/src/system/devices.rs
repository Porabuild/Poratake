//! Port of `src/main/devices/index.ts` — the daemon's microphone and camera
//! lists, used by the Devices settings page and the recording control bar.

use serde::Deserialize;
use serde_json::json;

use crate::daemon::DaemonHandle;

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct MediaDevice {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub label: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MediaDeviceLists {
    #[serde(default)]
    pub microphones: Vec<MediaDevice>,
    #[serde(default)]
    pub cameras: Vec<MediaDevice>,
    #[serde(default)]
    pub default_microphone_id: Option<String>,
    #[serde(default)]
    pub default_camera_id: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DeviceKind {
    Microphone,
    Camera,
}

impl DeviceKind {
    fn id(self) -> &'static str {
        match self {
            Self::Microphone => "microphone",
            Self::Camera => "camera",
        }
    }
}

/// The Swift daemon asks for TCC authorization per kind, so a caller that only
/// needs microphones must not trigger a camera prompt.
pub fn list(daemon: &DaemonHandle, kinds: &[DeviceKind]) -> MediaDeviceLists {
    if !daemon.is_running() && daemon.start().is_err() {
        return MediaDeviceLists::default();
    }
    let params = (!kinds.is_empty())
        .then(|| json!({ "kinds": kinds.iter().map(|kind| kind.id()).collect::<Vec<_>>() }));
    match daemon.call("media-devices", "list", params) {
        Ok(response) => serde_json::from_value(response).unwrap_or_default(),
        Err(error) => {
            eprintln!("[devices] list failed: {error}");
            MediaDeviceLists::default()
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
