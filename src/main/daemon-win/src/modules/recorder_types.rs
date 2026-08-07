use crate::protocol::{param_bool, param_i32, param_str, Request};
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RecorderState {
    Idle,
    Recording,
    Paused,
}

impl RecorderState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Recording => "recording",
            Self::Paused => "paused",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CaptureRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl CaptureRect {
    pub fn right(self) -> Option<i32> {
        self.x.checked_add(self.width)
    }

    pub fn bottom(self) -> Option<i32> {
        self.y.checked_add(self.height)
    }
}

#[derive(Clone, Debug)]
pub struct RecordingConfig {
    pub capture_rect: Option<CaptureRect>,
    pub display_id: Option<i32>,
    pub frame_rate: u32,
    pub output_path: PathBuf,
    pub keyboard_enabled: bool,
    pub include_audio: bool,
    pub mic_enabled: bool,
    pub mic_device_id: Option<String>,
    pub mic_device_name: Option<String>,
    pub camera_enabled: bool,
    pub camera_device_id: Option<String>,
    pub camera_device_name: Option<String>,
}

impl RecordingConfig {
    pub fn from_request(request: &Request) -> Result<Self, RecorderError> {
        if request.params.is_none() {
            return Err(RecorderError::invalid_params("start requires params"));
        }

        let Some(output_path) = param_str(&request.params, "outputPath") else {
            return Err(RecorderError::invalid_params("outputPath is required"));
        };

        if output_path.trim().is_empty() {
            return Err(RecorderError::invalid_params("outputPath is required"));
        }

        let coordinates = [
            param_i32(&request.params, "x"),
            param_i32(&request.params, "y"),
            param_i32(&request.params, "width"),
            param_i32(&request.params, "height"),
        ];
        let provided_coordinates = coordinates.iter().filter(|value| value.is_some()).count();

        if provided_coordinates != 0 && provided_coordinates != coordinates.len() {
            return Err(RecorderError::invalid_params(
                "x, y, width, and height must be provided together",
            ));
        }

        let capture_rect = if provided_coordinates == coordinates.len() {
            let rect = CaptureRect {
                x: coordinates[0].unwrap_or_default(),
                y: coordinates[1].unwrap_or_default(),
                width: coordinates[2].unwrap_or_default(),
                height: coordinates[3].unwrap_or_default(),
            };

            if rect.width <= 0 || rect.height <= 0 {
                return Err(RecorderError::invalid_params(
                    "width and height must be positive",
                ));
            }

            if rect.right().is_none() || rect.bottom().is_none() {
                return Err(RecorderError::invalid_params(
                    "capture area is outside the supported coordinate range",
                ));
            }

            Some(rect)
        } else {
            None
        };

        let frame_rate = param_i32(&request.params, "frameRate").unwrap_or(60);
        if !(1..=240).contains(&frame_rate) {
            return Err(RecorderError::invalid_params(
                "frameRate must be between 1 and 240",
            ));
        }

        if param_str(&request.params, "iosDeviceId").is_some() {
            return Err(RecorderError::configuration(
                "iOS device recording is not supported on Windows",
            ));
        }

        let output_path = PathBuf::from(output_path);
        let parent = recording_project_dir(&output_path)?;

        if !parent.is_dir() {
            return Err(RecorderError::configuration(
                "outputPath parent directory does not exist",
            ));
        }

        Ok(Self {
            capture_rect,
            display_id: param_i32(&request.params, "displayId"),
            frame_rate: frame_rate as u32,
            output_path,
            keyboard_enabled: param_bool(&request.params, "keyboardEnabled").unwrap_or(false),
            include_audio: param_bool(&request.params, "includeAudio").unwrap_or(true),
            mic_enabled: param_bool(&request.params, "micEnabled").unwrap_or(false),
            mic_device_id: param_str(&request.params, "micDeviceId").map(str::to_owned),
            mic_device_name: param_str(&request.params, "micDeviceName").map(str::to_owned),
            camera_enabled: param_bool(&request.params, "cameraEnabled").unwrap_or(false),
            camera_device_id: param_str(&request.params, "cameraDeviceId").map(str::to_owned),
            camera_device_name: param_str(&request.params, "cameraDeviceName").map(str::to_owned),
        })
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct RecorderStatus {
    pub state: RecorderState,
    pub duration: f64,
}

impl RecorderStatus {
    pub fn idle() -> Self {
        Self {
            state: RecorderState::Idle,
            duration: 0.0,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingResult {
    pub output_path: PathBuf,
    pub cursor_path: PathBuf,
    pub keys_path: Option<PathBuf>,
    pub camera_path: Option<PathBuf>,
    pub system_audio_path: Option<PathBuf>,
    pub mic_audio_path: Option<PathBuf>,
    pub duration: f64,
}

#[derive(Clone, Debug)]
pub struct StagedAsset {
    pub temporary_path: PathBuf,
    pub output_path: PathBuf,
}

pub fn recording_project_dir(output_path: &Path) -> Result<&Path, RecorderError> {
    output_path
        .parent()
        .ok_or_else(|| RecorderError::configuration("outputPath must have a parent directory"))
}

#[derive(Clone, Debug)]
pub struct RecorderError {
    pub code: &'static str,
    pub message: String,
}

impl RecorderError {
    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self {
            code: "INVALID_PARAMS",
            message: message.into(),
        }
    }

    pub fn invalid_state(message: impl Into<String>) -> Self {
        Self {
            code: "INVALID_STATE",
            message: message.into(),
        }
    }

    pub fn configuration(message: impl Into<String>) -> Self {
        Self {
            code: "CONFIGURATION_ERROR",
            message: message.into(),
        }
    }

    pub fn capture(message: impl Into<String>) -> Self {
        Self {
            code: "CAPTURE_ERROR",
            message: message.into(),
        }
    }

    pub fn start(message: impl Into<String>) -> Self {
        Self {
            code: "START_FAILED",
            message: message.into(),
        }
    }

    pub fn stop(message: impl Into<String>) -> Self {
        Self {
            code: "STOP_FAILED",
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashMap;

    #[test]
    fn parses_camera_config_and_derives_project_directory() {
        let project_dir = std::env::temp_dir();
        let output_path = project_dir.join("recording.mov");
        let params = HashMap::from([
            ("outputPath".to_string(), json!(output_path)),
            ("cameraEnabled".to_string(), json!(true)),
            ("cameraDeviceId".to_string(), json!("camera-id")),
            ("cameraDeviceName".to_string(), json!("Camera Name")),
        ]);
        let request = Request {
            id: "request".to_string(),
            module: "screen-recorder".to_string(),
            method: "start".to_string(),
            params: Some(params),
        };
        let Ok(config) = RecordingConfig::from_request(&request) else {
            panic!("camera recording config should parse");
        };

        assert!(config.camera_enabled);
        assert_eq!(config.camera_device_id.as_deref(), Some("camera-id"));
        assert_eq!(config.camera_device_name.as_deref(), Some("Camera Name"));
        let Ok(derived_project_dir) = recording_project_dir(&config.output_path) else {
            panic!("recording project directory should derive from outputPath");
        };
        assert_eq!(derived_project_dir, project_dir.as_path());
    }

    #[test]
    fn serializes_exact_camera_result_path() {
        let camera_path = PathBuf::from(r"C:\recording\camera.mov");
        let result = RecordingResult {
            output_path: PathBuf::from(r"C:\recording\recording.mov"),
            cursor_path: PathBuf::from(r"C:\recording\cursor.json"),
            keys_path: None,
            camera_path: Some(camera_path.clone()),
            system_audio_path: None,
            mic_audio_path: None,
            duration: 2.5,
        };
        let Ok(value) = serde_json::to_value(result) else {
            panic!("recording result should serialize");
        };

        assert_eq!(value["cameraPath"], json!(camera_path));
    }
}
