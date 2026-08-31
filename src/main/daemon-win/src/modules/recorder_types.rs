use crate::protocol::{Request, params};
use poratake_daemon_common::contract::ScreenRecorderStartRequest;
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FitRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Places `source` inside `target` at the largest scale that keeps its aspect
/// ratio, centring the letterbox. A recorded window keeps the video size it
/// started with, so every later window size is fitted into that frame.
pub fn fit_rect(source: (u32, u32), target: (u32, u32)) -> FitRect {
    let empty = FitRect {
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
    };
    if source.0 == 0 || source.1 == 0 || target.0 == 0 || target.1 == 0 {
        return empty;
    }

    let scale = f64::min(
        f64::from(target.0) / f64::from(source.0),
        f64::from(target.1) / f64::from(source.1),
    );
    let width = f64::from(source.0) * scale;
    let height = f64::from(source.1) * scale;

    FitRect {
        x: (f64::from(target.0) - width) / 2.0,
        y: (f64::from(target.1) - height) / 2.0,
        width,
        height,
    }
}

#[derive(Clone, Debug)]
pub struct RecordingConfig {
    pub capture_rect: Option<CaptureRect>,
    pub display_id: Option<i32>,
    pub window_id: Option<isize>,
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
        let Some(request_params) = request.params.as_ref() else {
            return Err(RecorderError::invalid_params("start requires params"));
        };
        if !request_params.contains_key("outputPath") {
            return Err(RecorderError::invalid_params("outputPath is required"));
        }
        let wire: ScreenRecorderStartRequest =
            params(request).map_err(|(_, message)| RecorderError::invalid_params(message))?;
        if wire.output_path.as_os_str().is_empty() {
            return Err(RecorderError::invalid_params("outputPath is required"));
        }
        wire.validate().map_err(RecorderError::invalid_params)?;

        let coordinates = [wire.x, wire.y, wire.width, wire.height];
        let provided_coordinates = coordinates.iter().filter(|value| value.is_some()).count();

        let capture_rect = if provided_coordinates == coordinates.len() {
            let rect = CaptureRect {
                x: coordinates[0].unwrap_or_default(),
                y: coordinates[1].unwrap_or_default(),
                width: coordinates[2].unwrap_or_default(),
                height: coordinates[3].unwrap_or_default(),
            };

            if rect.right().is_none() || rect.bottom().is_none() {
                return Err(RecorderError::invalid_params(
                    "capture area is outside the supported coordinate range",
                ));
            }

            Some(rect)
        } else {
            None
        };

        if wire.ios_device_id.is_some() {
            return Err(RecorderError::configuration(
                "iOS device recording is not supported on Windows",
            ));
        }

        let output_path = wire.output_path;
        let parent = recording_project_dir(&output_path)?;

        if !parent.is_dir() {
            return Err(RecorderError::configuration(
                "outputPath parent directory does not exist",
            ));
        }

        let window_id = wire.window_id.map(|handle| handle as isize);
        if window_id == Some(0) {
            return Err(RecorderError::invalid_params("windowId is not a window"));
        }

        let display_id = if capture_rect.is_none() && window_id.is_none() {
            wire.display_id
                .map(i32::try_from)
                .transpose()
                .map_err(|_| {
                    RecorderError::invalid_params("displayId is outside the supported range")
                })?
        } else {
            None
        };

        Ok(Self {
            capture_rect,
            display_id,
            window_id,
            frame_rate: wire.frame_rate,
            output_path,
            keyboard_enabled: wire.keyboard_enabled,
            include_audio: wire.include_audio,
            mic_enabled: wire.mic_enabled,
            mic_device_id: wire.mic_device_id,
            mic_device_name: wire.mic_device_name,
            camera_enabled: wire.camera_enabled,
            camera_device_id: wire.camera_device_id,
            camera_device_name: wire.camera_device_name,
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

    pub fn target_closed(message: impl Into<String>) -> Self {
        Self {
            code: "TARGET_CLOSED",
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

    fn start_request(params: HashMap<String, serde_json::Value>) -> Request {
        Request {
            id: "request".to_string(),
            module: "screen-recorder".to_string(),
            method: "start".to_string(),
            params: Some(params),
        }
    }

    #[test]
    fn parses_a_window_handle_that_does_not_fit_in_32_bits() {
        let handle: i64 = 0x0000_0002_0000_1234;
        let params = HashMap::from([
            (
                "outputPath".to_string(),
                json!(std::env::temp_dir().join("recording.mov")),
            ),
            ("windowId".to_string(), json!(handle)),
        ]);

        let Ok(config) = RecordingConfig::from_request(&start_request(params)) else {
            panic!("window recording config should parse");
        };

        assert_eq!(config.window_id, Some(handle as isize));
    }

    #[test]
    fn rejects_a_null_window_handle() {
        let params = HashMap::from([
            (
                "outputPath".to_string(),
                json!(std::env::temp_dir().join("recording.mov")),
            ),
            ("windowId".to_string(), json!(0)),
        ]);

        let Err(error) = RecordingConfig::from_request(&start_request(params)) else {
            panic!("a null window handle should be rejected");
        };

        assert_eq!(error.code, "INVALID_PARAMS");
    }

    #[test]
    fn ignores_an_unrepresentable_display_id_for_an_area_capture() {
        let params = HashMap::from([
            (
                "outputPath".to_string(),
                json!(std::env::temp_dir().join("recording.mov")),
            ),
            ("x".to_string(), json!(0)),
            ("y".to_string(), json!(0)),
            ("width".to_string(), json!(1920)),
            ("height".to_string(), json!(1080)),
            ("displayId".to_string(), json!(u32::MAX)),
        ]);

        let Ok(config) = RecordingConfig::from_request(&start_request(params)) else {
            panic!("area recording config should parse");
        };

        assert_eq!(config.display_id, None);
        assert!(config.capture_rect.is_some());
    }

    #[test]
    fn rejects_frame_rates_outside_the_shared_range() {
        for frame_rate in [0, 241] {
            let params = HashMap::from([
                (
                    "outputPath".to_string(),
                    json!(std::env::temp_dir().join("recording.mov")),
                ),
                ("frameRate".to_string(), json!(frame_rate)),
            ]);

            let Err(error) = RecordingConfig::from_request(&start_request(params)) else {
                panic!("invalid frame rate should be rejected");
            };
            assert_eq!(error.code, "INVALID_PARAMS");
        }
    }

    #[test]
    fn fits_a_wider_window_against_the_frame_width() {
        let fit = fit_rect((800, 200), (400, 400));

        assert_eq!(
            fit,
            FitRect {
                x: 0.0,
                y: 150.0,
                width: 400.0,
                height: 100.0,
            }
        );
    }

    #[test]
    fn fits_a_taller_window_against_the_frame_height() {
        let fit = fit_rect((200, 800), (400, 400));

        assert_eq!(
            fit,
            FitRect {
                x: 150.0,
                y: 0.0,
                width: 100.0,
                height: 400.0,
            }
        );
    }

    #[test]
    fn fills_the_frame_when_the_window_keeps_its_size() {
        let fit = fit_rect((1920, 1080), (1920, 1080));

        assert_eq!(
            fit,
            FitRect {
                x: 0.0,
                y: 0.0,
                width: 1920.0,
                height: 1080.0,
            }
        );
    }

    #[test]
    fn scales_a_shrunken_window_up_to_the_frame() {
        let fit = fit_rect((960, 540), (1920, 1080));

        assert_eq!(fit.width, 1920.0);
        assert_eq!(fit.height, 1080.0);
    }

    #[test]
    fn refuses_to_fit_an_empty_window() {
        assert_eq!(fit_rect((0, 0), (1920, 1080)).width, 0.0);
        assert_eq!(fit_rect((1920, 1080), (0, 0)).width, 0.0);
    }

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
