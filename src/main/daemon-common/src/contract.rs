include!("generated_methods.rs");

pub const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
pub const SCROLL_CAPTURE_CANCELLED_EVENT: &str = "scroll-capture:cancelled";
pub const SCROLL_CAPTURE_DONE_EVENT: &str = "scroll-capture:done";
pub const SCROLL_CAPTURE_FRAME_EVENT: &str = "scroll-capture:frame-captured";
pub const SCROLL_CAPTURE_SCROLL_ENDED_EVENT: &str = "scroll-capture:scroll-ended";
pub const SYSTEM_EXIT_EVENT: &str = "system:exit";
pub const TIMER_CONTROL_CANCEL_EVENT: &str = "timer-control:cancel";
pub const TIMER_CONTROL_COMPLETED_EVENT: &str = "timer-control:completed";
pub const TIMER_CONTROL_HEIGHT: i32 = 52;
pub const TIMER_CONTROL_TOP_MARGIN: i32 = 20;
pub const TIMER_CONTROL_WIDTH: i32 = 140;

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TimerShowRequest {
    pub x: i32,
    pub y: i32,
    pub duration: i32,
    pub color: String,
    pub foreground_color: String,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PrintImageRequest {
    pub image_base64: String,
}

#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
pub struct MediaDevice {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub label: String,
}

#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
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

#[derive(Clone, Copy, Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MediaDeviceKind {
    Microphone,
    Camera,
}

#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
pub struct MediaDeviceListRequest {
    #[serde(default)]
    pub kinds: Vec<MediaDeviceKind>,
}

#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MicrophoneTestRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_name: Option<String>,
}

#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CameraPreviewRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flipped: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y: Option<i32>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum UpdateField<T> {
    #[default]
    Missing,
    Value(Option<T>),
}

impl<T> UpdateField<T> {
    pub fn is_missing(&self) -> bool {
        matches!(self, Self::Missing)
    }
}

impl<'de, T: serde::Deserialize<'de>> serde::Deserialize<'de> for UpdateField<T> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        <Option<T> as serde::Deserialize>::deserialize(deserializer).map(Self::Value)
    }
}

impl<T: serde::Serialize> serde::Serialize for UpdateField<T> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Missing => serializer.serialize_unit(),
            Self::Value(value) => serde::Serialize::serialize(value, serializer),
        }
    }
}

#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CameraPreviewUpdateRequest {
    #[serde(default, skip_serializing_if = "UpdateField::is_missing")]
    pub device_id: UpdateField<String>,
    #[serde(default, skip_serializing_if = "UpdateField::is_missing")]
    pub device_name: UpdateField<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flipped: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y: Option<i32>,
}

#[derive(Clone, Copy, Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
pub struct ContentProtectionRequest {
    pub enabled: bool,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
#[serde(tag = "type", content = "value", rename_all = "lowercase")]
pub enum DesktopWallpaperResult {
    Path(String),
    Data(String),
}

#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
pub struct IosDeviceList {
    #[serde(default)]
    pub devices: Vec<MediaDevice>,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OcrRecognizeRequest {
    pub image_path: std::path::PathBuf,
}

#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
pub struct OcrRecognizeResult {
    #[serde(default)]
    pub text: String,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RecordingOverlayShowWindowRequest {
    pub window_id: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub below_window_id: Option<i64>,
}

#[derive(Clone, Copy, Debug, Default, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
pub struct RecordingOverlayVisibilityResult {
    #[serde(default)]
    pub visible: bool,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ScreenRecorderStartRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_id: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_id: Option<i64>,
    #[serde(default = "default_true")]
    pub include_audio: bool,
    #[serde(default)]
    pub mic_enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mic_device_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mic_device_name: Option<String>,
    #[serde(default)]
    pub camera_enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera_device_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera_device_name: Option<String>,
    #[serde(default)]
    pub keyboard_enabled: bool,
    #[serde(default = "default_frame_rate")]
    pub frame_rate: u32,
    pub output_path: std::path::PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ios_device_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ios_device_name: Option<String>,
}

impl ScreenRecorderStartRequest {
    pub fn validate(&self) -> Result<(), &'static str> {
        let coordinates = [self.x, self.y, self.width, self.height];
        let provided = coordinates.iter().filter(|value| value.is_some()).count();
        if provided != 0 && provided != coordinates.len() {
            return Err("x, y, width, and height must be provided together");
        }
        if matches!(self.width, Some(width) if width <= 0)
            || matches!(self.height, Some(height) if height <= 0)
        {
            return Err("width and height must be positive");
        }
        if !(1..=240).contains(&self.frame_rate) {
            return Err("frameRate must be between 1 and 240");
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ScreenRecorderMicrophoneRequest {
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_name: Option<String>,
}

#[derive(Clone, Copy, Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
pub struct ScreenRecorderToggleRequest {
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

fn default_frame_rate() -> u32 {
    60
}

#[derive(Clone, Copy, Debug, Default, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ScrollSpeed {
    Slow,
    Fast,
    #[default]
    #[serde(other)]
    Medium,
}

impl ScrollSpeed {
    pub fn parse(value: &str) -> Self {
        match value {
            "slow" => Self::Slow,
            "fast" => Self::Fast,
            _ => Self::Medium,
        }
    }
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScrollCaptureStartRequest {
    #[serde(flatten)]
    pub capture: crate::geometry::DisplayCaptureContext,
    #[serde(default)]
    pub auto_scroll_speed: ScrollSpeed,
    #[serde(default = "default_scroll_max_height")]
    pub max_height: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_controls: Option<bool>,
}

fn default_scroll_max_height() -> i32 {
    20_000
}

impl ScrollCaptureStartRequest {
    pub fn validate(&self) -> Result<(), String> {
        self.capture.validate()
    }

    pub fn normalized_max_height(&self) -> usize {
        let logical_height =
            (self.capture.rect.height as f64 / self.capture.scale_factor).ceil() as usize;
        (self.max_height.max(1) as usize).max(logical_height)
    }
}

#[derive(Clone, Copy, Debug, Default, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
pub struct ScrollCaptureAutoScrollRequest {
    #[serde(default)]
    pub speed: Option<ScrollSpeed>,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ScrollCaptureFinishRequest {
    pub output_path: std::path::PathBuf,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ScrollCaptureFinishResult {
    pub success: bool,
    pub output_path: std::path::PathBuf,
    pub width: usize,
    pub height: usize,
    pub frame_count: usize,
}

impl TimerShowRequest {
    pub fn normalized_duration(&self) -> i32 {
        self.duration.max(1)
    }

    pub fn normalized_theme_colors(&self) -> Result<(String, String), String> {
        fn normalize(value: &str) -> Option<String> {
            let hex = value.trim_start_matches('#');
            (hex.len() == 6 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
                .then(|| format!("#{hex}"))
        }

        match (normalize(&self.color), normalize(&self.foreground_color)) {
            (Some(color), Some(foreground)) => Ok((color, foreground)),
            _ => Err("show requires theme colors".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn shared_methods(module: &str) -> Vec<String> {
        let contract: Value =
            serde_json::from_str(include_str!("../../../types/daemon-contract.json"))
                .expect("daemon contract");
        contract["modules"][module]["shared"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect()
    }

    #[test]
    fn screenshot_methods_round_trip() {
        for method in ScreenshotMethod::ALL {
            assert_eq!(ScreenshotMethod::parse(method.id()), Some(method));
        }

        let contract: Value =
            serde_json::from_str(include_str!("../../../types/daemon-contract.json"))
                .expect("daemon contract");
        let screenshot = &contract["modules"]["screenshot"];
        let shared: Vec<&str> = screenshot["shared"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        assert_eq!(
            ScreenshotMethod::ALL.map(ScreenshotMethod::id).as_slice(),
            shared.as_slice()
        );

        let linux: Vec<&str> = screenshot["linux"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        assert_eq!(
            ScreenshotLinuxMethod::ALL
                .map(ScreenshotLinuxMethod::id)
                .as_slice(),
            linux.as_slice()
        );
    }

    #[test]
    fn freeze_screen_methods_match_the_neutral_contract() {
        let contract: Value =
            serde_json::from_str(include_str!("../../../types/daemon-contract.json"))
                .expect("daemon contract");
        let expected: Vec<&str> = contract["modules"][FREEZE_SCREEN_MODULE]["shared"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();

        assert_eq!(
            FreezeScreenMethod::ALL
                .map(FreezeScreenMethod::id)
                .as_slice(),
            expected.as_slice()
        );
    }

    #[test]
    fn window_selector_methods_match_the_neutral_contract() {
        let contract: Value =
            serde_json::from_str(include_str!("../../../types/daemon-contract.json"))
                .expect("daemon contract");
        let expected: Vec<&str> = contract["modules"]["window-selector"]["shared"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();

        assert_eq!(
            WindowSelectorMethod::parse("list"),
            Some(WindowSelectorMethod::List)
        );
        assert_eq!(
            WindowSelectorMethod::ALL
                .map(WindowSelectorMethod::id)
                .as_slice(),
            expected.as_slice()
        );
    }

    #[test]
    fn qrcode_methods_match_the_neutral_contract() {
        let contract: Value =
            serde_json::from_str(include_str!("../../../types/daemon-contract.json"))
                .expect("daemon contract");
        let expected: Vec<&str> = contract["modules"][QRCODE_MODULE]["shared"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();

        assert_eq!(QrCodeMethod::parse("detect"), Some(QrCodeMethod::Detect));
        assert_eq!(
            QrCodeMethod::ALL.map(QrCodeMethod::id).as_slice(),
            expected.as_slice()
        );
    }

    #[test]
    fn print_methods_match_the_neutral_contract() {
        let contract: Value =
            serde_json::from_str(include_str!("../../../types/daemon-contract.json"))
                .expect("daemon contract");
        let expected: Vec<&str> = contract["modules"][PRINT_MODULE]["shared"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();

        assert_eq!(
            PrintMethod::ALL.map(PrintMethod::id).as_slice(),
            expected.as_slice()
        );
        assert_eq!(
            serde_json::to_value(PrintImageRequest {
                image_base64: "image-data".into(),
            })
            .expect("serialize print request"),
            serde_json::json!({ "imageBase64": "image-data" })
        );
    }

    #[test]
    fn desktop_and_media_methods_match_the_neutral_contract() {
        assert_eq!(
            CameraPreviewMethod::ALL
                .map(CameraPreviewMethod::id)
                .as_slice(),
            shared_methods(CAMERA_PREVIEW_MODULE)
        );
        assert_eq!(
            DesktopHelperMethod::ALL
                .map(DesktopHelperMethod::id)
                .as_slice(),
            shared_methods(DESKTOP_HELPER_MODULE)
        );
        assert_eq!(
            DesktopWallpaperMethod::ALL
                .map(DesktopWallpaperMethod::id)
                .as_slice(),
            shared_methods(DESKTOP_WALLPAPER_MODULE)
        );
        assert_eq!(
            MediaDevicesMethod::ALL
                .map(MediaDevicesMethod::id)
                .as_slice(),
            shared_methods(MEDIA_DEVICES_MODULE)
        );
        assert_eq!(
            RecordingControlMethod::ALL
                .map(RecordingControlMethod::id)
                .as_slice(),
            shared_methods(RECORDING_CONTROL_MODULE)
        );
    }

    #[test]
    fn desktop_and_media_records_use_the_native_wire_shape() {
        let camera = serde_json::to_value(CameraPreviewRequest {
            device_id: Some("camera-id".into()),
            device_name: Some("Camera".into()),
            flipped: Some(true),
            ..CameraPreviewRequest::default()
        })
        .expect("camera request");
        assert_eq!(
            camera,
            serde_json::json!({
                "deviceId": "camera-id",
                "deviceName": "Camera",
                "flipped": true
            })
        );
        assert!(camera.get("selectedDeviceId").is_none());
        assert_eq!(
            serde_json::to_value(ContentProtectionRequest { enabled: true })
                .expect("content protection request"),
            serde_json::json!({ "enabled": true })
        );

        assert_eq!(
            serde_json::to_value(MediaDeviceListRequest {
                kinds: vec![MediaDeviceKind::Microphone, MediaDeviceKind::Camera],
            })
            .expect("device list request"),
            serde_json::json!({ "kinds": ["microphone", "camera"] })
        );
        assert_eq!(
            serde_json::to_value(DesktopWallpaperResult::Path("wallpaper.png".into()))
                .expect("wallpaper result"),
            serde_json::json!({ "type": "path", "value": "wallpaper.png" })
        );
    }

    #[test]
    fn camera_update_distinguishes_missing_from_null_devices() {
        let missing: CameraPreviewUpdateRequest =
            serde_json::from_value(serde_json::json!({ "flipped": true }))
                .expect("missing device update");
        let cleared: CameraPreviewUpdateRequest = serde_json::from_value(serde_json::json!({
            "deviceId": null,
            "deviceName": null
        }))
        .expect("cleared device update");

        assert_eq!(missing.device_id, UpdateField::Missing);
        assert_eq!(cleared.device_id, UpdateField::Value(None));
        assert_eq!(cleared.device_name, UpdateField::Value(None));
        assert_eq!(
            serde_json::to_value(cleared).expect("serialize cleared update"),
            serde_json::json!({ "deviceId": null, "deviceName": null })
        );
    }

    #[test]
    fn recorder_and_analysis_methods_match_the_neutral_contract() {
        assert_eq!(
            OcrMethod::ALL.map(OcrMethod::id).as_slice(),
            shared_methods(OCR_MODULE)
        );
        assert_eq!(
            RecordingOverlayMethod::ALL
                .map(RecordingOverlayMethod::id)
                .as_slice(),
            shared_methods(RECORDING_OVERLAY_MODULE)
        );
        assert_eq!(
            ScreenRecorderMethod::ALL
                .map(ScreenRecorderMethod::id)
                .as_slice(),
            shared_methods(SCREEN_RECORDER_MODULE)
        );
    }

    #[test]
    fn recorder_and_analysis_records_use_the_native_wire_shape() {
        assert_eq!(
            serde_json::to_value(OcrRecognizeRequest {
                image_path: "capture.png".into(),
            })
            .expect("ocr request"),
            serde_json::json!({ "imagePath": "capture.png" })
        );
        assert_eq!(
            serde_json::to_value(RecordingOverlayShowWindowRequest {
                window_id: 42,
                color: Some("#8892ef".into()),
                below_window_id: Some(84),
            })
            .expect("recording overlay request"),
            serde_json::json!({
                "windowId": 42,
                "color": "#8892ef",
                "belowWindowId": 84,
            })
        );
        let start = ScreenRecorderStartRequest {
            x: Some(10),
            y: Some(20),
            width: Some(800),
            height: Some(600),
            display_id: Some(73),
            window_id: None,
            include_audio: true,
            mic_enabled: false,
            mic_device_id: None,
            mic_device_name: None,
            camera_enabled: true,
            camera_device_id: Some("camera-id".into()),
            camera_device_name: None,
            keyboard_enabled: false,
            frame_rate: 60,
            output_path: "recording.mp4".into(),
            ios_device_id: None,
            ios_device_name: None,
        };
        assert_eq!(
            serde_json::to_value(start).expect("screen recorder request"),
            serde_json::json!({
                "x": 10,
                "y": 20,
                "width": 800,
                "height": 600,
                "displayId": 73,
                "includeAudio": true,
                "micEnabled": false,
                "cameraEnabled": true,
                "cameraDeviceId": "camera-id",
                "keyboardEnabled": false,
                "frameRate": 60,
                "outputPath": "recording.mp4",
            })
        );
        let defaults: ScreenRecorderStartRequest =
            serde_json::from_value(serde_json::json!({ "outputPath": "recording.mp4" }))
                .expect("screen recorder defaults");
        assert!(defaults.include_audio);
        assert_eq!(defaults.frame_rate, 60);
        assert_eq!(defaults.x, None);
        assert_eq!(defaults.validate(), Ok(()));

        let mut partial = defaults.clone();
        partial.x = Some(10);
        assert_eq!(
            partial.validate(),
            Err("x, y, width, and height must be provided together")
        );

        let mut invalid_size = defaults;
        invalid_size.x = Some(10);
        invalid_size.y = Some(20);
        invalid_size.width = Some(0);
        invalid_size.height = Some(600);
        assert_eq!(
            invalid_size.validate(),
            Err("width and height must be positive")
        );

        let mut invalid_frame_rate = invalid_size;
        invalid_frame_rate.width = Some(800);
        invalid_frame_rate.frame_rate = 0;
        assert_eq!(
            invalid_frame_rate.validate(),
            Err("frameRate must be between 1 and 240")
        );

        for malformed in [
            serde_json::json!({ "outputPath": "recording.mp4", "x": "10" }),
            serde_json::json!({ "outputPath": "recording.mp4", "frameRate": 60.5 }),
            serde_json::json!({ "outputPath": "recording.mp4", "includeAudio": "false" }),
        ] {
            assert!(serde_json::from_value::<ScreenRecorderStartRequest>(malformed).is_err());
        }
        assert!(
            serde_json::from_value::<ScreenRecorderMicrophoneRequest>(serde_json::json!({
                "enabled": true,
                "deviceId": 42
            }))
            .is_err()
        );
        invalid_frame_rate.frame_rate = 241;
        assert_eq!(
            invalid_frame_rate.validate(),
            Err("frameRate must be between 1 and 240")
        );
    }

    #[test]
    fn scroll_capture_matches_the_neutral_contract() {
        assert_eq!(
            ScrollCaptureMethod::ALL
                .map(ScrollCaptureMethod::id)
                .as_slice(),
            shared_methods(SCROLL_CAPTURE_MODULE)
        );
        let request = ScrollCaptureStartRequest {
            capture: crate::geometry::DisplayCaptureContext::new(
                crate::geometry::CaptureRect {
                    x: 10,
                    y: 20,
                    width: 300,
                    height: 400,
                },
                2.0,
                crate::geometry::DisplayOrigin { x: 1_920, y: 0 },
                Some(42),
            ),
            auto_scroll_speed: ScrollSpeed::Fast,
            max_height: 20_000,
            native_controls: Some(true),
        };
        assert_eq!(request.normalized_max_height(), 20_000);
        assert_eq!(
            serde_json::to_value(request).expect("scroll start request"),
            serde_json::json!({
                "x": 10,
                "y": 20,
                "width": 300,
                "height": 400,
                "scaleFactor": 2.0,
                "autoScrollSpeed": "fast",
                "maxHeight": 20_000,
                "displayOriginX": 1_920,
                "displayOriginY": 0,
                "displayId": 42,
                "nativeControls": true
            })
        );
        let defaults: ScrollCaptureStartRequest = serde_json::from_value(serde_json::json!({
            "x": 10,
            "y": 20,
            "width": 300,
            "height": 400
        }))
        .expect("scroll defaults");
        assert_eq!(defaults.capture.scale_factor, 1.0);
        assert_eq!(defaults.auto_scroll_speed, ScrollSpeed::Medium);
        assert_eq!(defaults.max_height, 20_000);
        assert_eq!(defaults.native_controls, None);
    }

    #[test]
    fn timer_control_matches_the_neutral_contract() {
        let contract: Value =
            serde_json::from_str(include_str!("../../../types/daemon-contract.json"))
                .expect("daemon contract");
        let expected: Vec<&str> = contract["modules"][TIMER_CONTROL_MODULE]["shared"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();

        assert_eq!(
            TimerControlMethod::ALL
                .map(TimerControlMethod::id)
                .as_slice(),
            expected.as_slice()
        );

        let request = TimerShowRequest {
            x: 10,
            y: 20,
            duration: 5,
            color: "#112233".into(),
            foreground_color: "#ffffff".into(),
        };
        assert_eq!(
            serde_json::to_value(request).expect("serialize timer request"),
            serde_json::json!({
                "x": 10,
                "y": 20,
                "duration": 5,
                "color": "#112233",
                "foregroundColor": "#ffffff"
            })
        );
        assert_eq!(
            contract["geometry"]["timerControl"],
            serde_json::json!({ "width": 140, "height": 52, "topMargin": 20 })
        );
        let negative_duration = serde_json::from_value::<TimerShowRequest>(serde_json::json!({
            "x": 10,
            "y": 20,
            "duration": -5,
            "color": "#112233",
            "foregroundColor": "#ffffff"
        }))
        .expect("negative timer duration");
        assert_eq!(negative_duration.duration, -5);
        assert_eq!(negative_duration.normalized_duration(), 1);
    }

    #[test]
    fn timer_colors_share_the_native_hex_contract() {
        let request = |color: &str, foreground_color: &str| TimerShowRequest {
            x: 0,
            y: 0,
            duration: 5,
            color: color.into(),
            foreground_color: foreground_color.into(),
        };

        assert_eq!(
            request("8892ef", "#FFFFFF").normalized_theme_colors(),
            Ok(("#8892ef".into(), "#FFFFFF".into()))
        );
        for color in ["red", "rgb(1,2,3)", "#12345", "##1234567"] {
            assert!(request(color, "#ffffff").normalized_theme_colors().is_err());
        }
    }
}
