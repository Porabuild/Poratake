//! 1:1 ports of the video editor style types (`src/types/cursor.ts`,
//! `camera.ts`, `keyboard.ts`, `subtitle.ts`, `audio.ts`,
//! `video-wallpaper.ts`, `first-frame.ts` and the export settings in
//! `video.ts`) so `state.json` round-trips between both shells.

use serde::{Deserialize, Serialize};

fn t() -> bool {
    true
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorStyle {
    #[serde(default = "t")]
    pub enabled: bool,
    #[serde(default = "cursor_size")]
    pub size: f64,
    #[serde(default = "black")]
    pub color: String,
    #[serde(default = "white")]
    pub border_color: String,
    #[serde(default = "two")]
    pub border_width: f64,
    #[serde(default = "half")]
    pub smoothing: f64,
    #[serde(default = "t")]
    pub show_click_highlight: bool,
    #[serde(default = "click_highlight_color")]
    pub click_highlight_color: String,
    #[serde(default = "thirty")]
    pub click_highlight_radius: f64,
    #[serde(default = "fifteen")]
    pub click_highlight_duration: f64,
    #[serde(default)]
    pub hide_on_idle: bool,
    #[serde(default = "two")]
    pub hide_on_idle_timeout: f64,
    #[serde(default)]
    pub show_trail: bool,
    #[serde(default = "ten")]
    pub trail_length: f64,
    #[serde(default = "point_eight")]
    pub trail_opacity_decay: f64,
    #[serde(default = "t")]
    pub motion_blur: bool,
    #[serde(default = "half")]
    pub motion_blur_strength: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_cursor_image: Option<String>,
}

fn cursor_size() -> f64 {
    100.0
}
fn black() -> String {
    "#000000".into()
}
fn white() -> String {
    "#ffffff".into()
}
fn two() -> f64 {
    2.0
}
fn half() -> f64 {
    0.5
}
fn click_highlight_color() -> String {
    "rgba(255, 200, 0, 0.5)".into()
}
fn thirty() -> f64 {
    30.0
}
fn fifteen() -> f64 {
    15.0
}
fn ten() -> f64 {
    10.0
}
fn point_eight() -> f64 {
    0.8
}

impl Default for CursorStyle {
    fn default() -> Self {
        Self {
            enabled: true,
            size: 100.0,
            color: black(),
            border_color: white(),
            border_width: 2.0,
            smoothing: 0.5,
            show_click_highlight: true,
            click_highlight_color: click_highlight_color(),
            click_highlight_radius: 30.0,
            click_highlight_duration: 15.0,
            hide_on_idle: false,
            hide_on_idle_timeout: 2.0,
            show_trail: false,
            trail_length: 10.0,
            trail_opacity_decay: 0.8,
            motion_blur: true,
            motion_blur_strength: 0.5,
            custom_cursor_image: None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraStyle {
    #[serde(default = "t")]
    pub visible: bool,
    #[serde(default = "camera_position")]
    pub position: String,
    #[serde(default = "camera_shape")]
    pub shape: String,
    #[serde(default = "medium")]
    pub size: String,
    #[serde(default = "fifty")]
    pub border_radius: f64,
    #[serde(default = "three")]
    pub padding: f64,
    #[serde(default = "hundred")]
    pub shadow: f64,
    #[serde(default = "t")]
    pub mirrored: bool,
}

fn camera_position() -> String {
    "bottom-right".into()
}
fn camera_shape() -> String {
    "square".into()
}
fn medium() -> String {
    "medium".into()
}
fn fifty() -> f64 {
    50.0
}
fn three() -> f64 {
    3.0
}
fn hundred() -> f64 {
    100.0
}

impl Default for CameraStyle {
    fn default() -> Self {
        Self {
            visible: true,
            position: camera_position(),
            shape: camera_shape(),
            size: medium(),
            border_radius: 50.0,
            padding: 3.0,
            shadow: 100.0,
            mirrored: true,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyboardStyle {
    #[serde(default)]
    pub visible: bool,
    #[serde(default = "one")]
    pub display_duration: f64,
    #[serde(default = "bottom_center")]
    pub position: String,
    #[serde(default = "medium")]
    pub font_size: String,
    #[serde(default = "point_seven_five")]
    pub opacity: f64,
}

fn one() -> f64 {
    1.0
}
fn bottom_center() -> String {
    "bottom-center".into()
}
fn point_seven_five() -> f64 {
    0.75
}

impl Default for KeyboardStyle {
    fn default() -> Self {
        Self {
            visible: false,
            display_duration: 1.0,
            position: bottom_center(),
            font_size: medium(),
            opacity: 0.75,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleStyle {
    #[serde(default = "t")]
    pub visible: bool,
    #[serde(default = "medium")]
    pub font_size: String,
    #[serde(default = "bottom")]
    pub position: String,
    #[serde(default = "dark")]
    pub background_color: String,
    #[serde(default = "point_nine")]
    pub opacity: f64,
}

fn bottom() -> String {
    "bottom".into()
}
fn dark() -> String {
    "dark".into()
}
fn point_nine() -> f64 {
    0.9
}

impl Default for SubtitleStyle {
    fn default() -> Self {
        Self {
            visible: true,
            font_size: medium(),
            position: bottom(),
            background_color: dark(),
            opacity: 0.9,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioStyle {
    #[serde(default = "t")]
    pub system_audio_enabled: bool,
    #[serde(default = "t")]
    pub mic_audio_enabled: bool,
    #[serde(default = "one")]
    pub system_audio_volume: f64,
    #[serde(default = "one")]
    pub mic_audio_volume: f64,
    #[serde(default)]
    pub keyboard_sound_enabled: bool,
    #[serde(default = "point_seven")]
    pub keyboard_sound_volume: f64,
    #[serde(default = "cherry_blue")]
    pub keyboard_sound_type: String,
}

fn point_seven() -> f64 {
    0.7
}
fn cherry_blue() -> String {
    "cherry-blue".into()
}

impl Default for AudioStyle {
    fn default() -> Self {
        Self {
            system_audio_enabled: true,
            mic_audio_enabled: true,
            system_audio_volume: 1.0,
            mic_audio_volume: 1.0,
            keyboard_sound_enabled: false,
            keyboard_sound_volume: 0.7,
            keyboard_sound_type: cherry_blue(),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoWallpaperSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub gradient: Option<serde_json::Value>,
    #[serde(default)]
    pub background_image: Option<String>,
    #[serde(default)]
    pub padding: f64,
    #[serde(default)]
    pub corners: f64,
    #[serde(default)]
    pub shadow: f64,
    #[serde(default)]
    pub aspect_ratio: Option<serde_json::Value>,
    #[serde(default)]
    pub device_frame: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FirstFrameSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub image_data: Option<String>,
    #[serde(default = "cover")]
    pub fit: String,
}

fn cover() -> String {
    "cover".into()
}

impl Default for FirstFrameSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            image_data: None,
            fit: cover(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportSettings {
    #[serde(default = "mp4")]
    pub format: String,
    #[serde(default = "original")]
    pub resolution: String,
    #[serde(default = "studio")]
    pub quality_preset: String,
    #[serde(default = "sixty")]
    pub frame_rate: String,
    #[serde(default = "t")]
    pub open_in_finder: bool,
}

fn mp4() -> String {
    "mp4".into()
}
fn original() -> String {
    "original".into()
}
fn studio() -> String {
    "studio".into()
}
fn sixty() -> String {
    "60".into()
}

impl Default for ExportSettings {
    fn default() -> Self {
        Self {
            format: mp4(),
            resolution: original(),
            quality_preset: studio(),
            frame_rate: sixty(),
            open_in_finder: true,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoomSettings {
    #[serde(default = "one_point_two")]
    pub transition_in_duration: f64,
    #[serde(default = "one_point_two")]
    pub transition_out_duration: f64,
    #[serde(default = "ease_in_out")]
    pub easing: String,
    #[serde(default = "point_three")]
    pub follow_smoothness: f64,
    #[serde(default = "point_one_two")]
    pub look_ahead: f64,
}

fn one_point_two() -> f64 {
    1.2
}
fn ease_in_out() -> String {
    "ease-in-out".into()
}
fn point_three() -> f64 {
    0.3
}
fn point_one_two() -> f64 {
    0.12
}

impl Default for ZoomSettings {
    fn default() -> Self {
        Self {
            transition_in_duration: 1.2,
            transition_out_duration: 1.2,
            easing: ease_in_out(),
            follow_smoothness: 0.3,
            look_ahead: 0.12,
        }
    }
}

#[allow(dead_code)]
pub const CAMERA_POSITIONS: [(&str, &str); 9] = [
    ("top-left", "Top Left"),
    ("top-center", "Top Center"),
    ("top-right", "Top Right"),
    ("middle-left", "Middle Left"),
    ("middle-center", "Middle Center"),
    ("middle-right", "Middle Right"),
    ("bottom-left", "Bottom Left"),
    ("bottom-center", "Bottom Center"),
    ("bottom-right", "Bottom Right"),
];

pub const CAMERA_SHAPES: [(&str, &str); 3] = [
    ("rectangle", "Rectangle"),
    ("square", "Square"),
    ("vertical", "Vertical"),
];

pub const SIZE_OPTIONS: [(&str, &str); 3] =
    [("small", "Small"), ("medium", "Medium"), ("large", "Large")];

pub const SUBTITLE_POSITIONS: [(&str, &str); 2] = [("bottom", "Bottom"), ("top", "Top")];

pub const SUBTITLE_BACKGROUNDS: [(&str, &str); 3] =
    [("dark", "Dark"), ("light", "Light"), ("none", "None")];

pub const KEYBOARD_SOUND_TYPES: [(&str, &str); 3] = [
    ("cherry-blue", "Cherry MX Blue"),
    ("cherry-brown", "Cherry MX Brown"),
    ("cherry-red", "Cherry MX Red"),
];

pub const EXPORT_FORMATS: [(&str, &str); 2] = [("mp4", "MP4"), ("gif", "GIF")];

pub const EXPORT_RESOLUTIONS: [(&str, &str); 5] = [
    ("original", "Original"),
    ("4k", "4K (3840x2160)"),
    ("1080p", "1080p (1920x1080)"),
    ("720p", "720p (1280x720)"),
    ("480p", "480p (854x480)"),
];

pub const EXPORT_QUALITY_PRESETS: [(&str, &str); 4] = [
    ("studio", "Studio"),
    ("social", "Social Media"),
    ("web", "Web"),
    ("web-low", "Web (Low)"),
];

pub const EXPORT_FRAME_RATES: [(&str, &str); 8] = [
    ("60", "60 FPS"),
    ("50", "50 FPS"),
    ("40", "40 FPS"),
    ("30", "30 FPS"),
    ("25", "25 FPS"),
    ("24", "24 FPS"),
    ("20", "20 FPS"),
    ("10", "10 FPS"),
];

pub const MP4_RESOLUTIONS: &[&str] = &["original", "4k", "1080p", "720p", "480p"];
pub const GIF_RESOLUTIONS: &[&str] = &["1080p", "720p", "480p"];
pub const MP4_FRAME_RATES: &[&str] = &["60", "50", "40", "30", "25", "24", "20", "10"];
pub const GIF_FRAME_RATES: &[&str] = &["50", "30", "25", "24", "20", "10"];

pub const CURSOR_SIZE_MIN: f64 = 50.0;
pub const CURSOR_SIZE_MAX: f64 = 250.0;
pub const CAMERA_PADDING_MAX: f64 = 10.0;
pub const CAMERA_RADIUS_MAX: f64 = 100.0;
pub const CAMERA_SHADOW_MAX: f64 = 100.0;
pub const ZOOM_LEVEL_MIN: f64 = 1.0;
pub const ZOOM_LEVEL_MAX: f64 = 3.0;
pub const ZOOM_SPEED_MIN: f64 = 0.2;
pub const ZOOM_SPEED_MAX: f64 = 2.0;
pub const MUSIC_SPEEDS: [(&str, &str); 8] = [
    ("0.5", "0.5x"),
    ("0.75", "0.75x"),
    ("1", "1x"),
    ("1.25", "1.25x"),
    ("1.5", "1.5x"),
    ("2", "2x"),
    ("3", "3x"),
    ("4", "4x"),
];
pub const CURSOR_COLORS: [(&str, &str); 9] = [
    ("#ffffff", "White"),
    ("#000000", "Black"),
    ("#facc15", "Yellow"),
    ("#ef4444", "Red"),
    ("#3b82f6", "Blue"),
    ("#22c55e", "Green"),
    ("#f97316", "Orange"),
    ("#a855f7", "Purple"),
    ("#ec4899", "Pink"),
];
pub const CURSOR_BORDERS: [(&str, &str); 4] = [
    ("#000000", "Black"),
    ("#ffffff", "White"),
    ("#6b7280", "Gray"),
    ("transparent", "None"),
];
pub const CAMERA_POSITION_GRID: [[&str; 3]; 3] = [
    ["top-left", "top-center", "top-right"],
    ["middle-left", "middle-center", "middle-right"],
    ["bottom-left", "bottom-center", "bottom-right"],
];

pub const FIRST_FRAME_FITS: [(&str, &str); 2] = [("cover", "Cover"), ("stretch", "Stretch")];

#[derive(Clone, Debug, PartialEq)]
pub struct DrawingToolSettings {
    pub active_tool: String,
    pub selected_color: String,
    pub stroke_width: f64,
    pub arrow_style: String,
    pub highlight_color: String,
    pub highlight_opacity: f64,
    pub number_style: String,
    pub number_size: String,
    pub number_start_value: f64,
    pub text_background: bool,
    pub text_font_size: f64,
    pub text_font_family: String,
    pub redact_style: String,
    pub redact_intensity: f64,
    pub shape_fill_mode: String,
}

impl Default for DrawingToolSettings {
    fn default() -> Self {
        Self {
            active_tool: "select".into(),
            selected_color: "#FF3B30".into(),
            stroke_width: 4.0,
            arrow_style: "standard".into(),
            highlight_color: "#FFFF00".into(),
            highlight_opacity: 0.4,
            number_style: "numeric".into(),
            number_size: "medium".into(),
            number_start_value: 1.0,
            text_background: true,
            text_font_size: 24.0,
            text_font_family: "sans".into(),
            redact_style: "pixelate".into(),
            redact_intensity: 5.0,
            shape_fill_mode: "outline".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_serialize_with_the_renderer_key_names() {
        let json = serde_json::to_value(CursorStyle::default()).expect("cursor style");
        assert_eq!(json["showClickHighlight"], true);
        assert_eq!(json["clickHighlightRadius"], 30.0);
        assert_eq!(json["motionBlurStrength"], 0.5);
        assert!(json.get("customCursorImage").is_none());

        let json = serde_json::to_value(AudioStyle::default()).expect("audio style");
        assert_eq!(json["keyboardSoundType"], "cherry-blue");
        assert_eq!(json["systemAudioVolume"], 1.0);

        let json = serde_json::to_value(ExportSettings::default()).expect("export settings");
        assert_eq!(json["qualityPreset"], "studio");
        assert_eq!(json["frameRate"], "60");
        assert_eq!(json["openInFinder"], true);
    }

    #[test]
    fn missing_fields_fall_back_to_the_renderer_defaults() {
        let parsed: CameraStyle = serde_json::from_str("{}").expect("camera style");
        assert_eq!(parsed, CameraStyle::default());
        let parsed: SubtitleStyle = serde_json::from_str(r#"{"visible":false}"#).expect("subtitle");
        assert!(!parsed.visible);
        assert_eq!(parsed.font_size, "medium");
    }

    #[test]
    fn sidebar_panels_match_electron() {
        use crate::ui::chrome;
        assert_eq!(chrome::VIDEO_SIDEBAR_WIDTH, 288.0);
        assert_eq!(chrome::VIDEO_TAB_RAIL_WIDTH, 40.0);
        assert_eq!(chrome::VIDEO_TAB_BUTTON_SIZE, 32.0);
        assert_eq!(chrome::VIDEO_PANEL_PAD, 16.0);
        assert_eq!(chrome::VIDEO_PANEL_GAP, 16.0);
        assert_eq!(chrome::SETTINGS_HEADER_TITLE, 14.0);
        assert_eq!(chrome::SETTINGS_HEADER_DESC, 12.0);
        assert_eq!(CURSOR_SIZE_MIN, 50.0);
        assert_eq!(CURSOR_SIZE_MAX, 250.0);
        assert_eq!(CAMERA_PADDING_MAX, 10.0);
        assert_eq!(CAMERA_RADIUS_MAX, 100.0);
        assert_eq!(CAMERA_SHADOW_MAX, 100.0);
        assert_eq!(ZOOM_LEVEL_MIN, 1.0);
        assert_eq!(ZOOM_LEVEL_MAX, 3.0);
        assert_eq!(ZOOM_SPEED_MIN, 0.2);
        assert_eq!(ZOOM_SPEED_MAX, 2.0);
        assert_eq!(CURSOR_COLORS.len(), 9);
        assert_eq!(EXPORT_QUALITY_PRESETS[1], ("social", "Social Media"));
        assert_eq!(EXPORT_RESOLUTIONS[1], ("4k", "4K (3840x2160)"));
        assert_eq!(EXPORT_FRAME_RATES[0], ("60", "60 FPS"));
        assert_eq!(GIF_RESOLUTIONS, &["1080p", "720p", "480p"]);
        assert_eq!(SIZE_OPTIONS.len(), 3);
        assert_eq!(SUBTITLE_BACKGROUNDS.len(), 3);
        assert_eq!(FIRST_FRAME_FITS.len(), 2);
        assert_eq!(MUSIC_SPEEDS.len(), 8);
        let drawing = DrawingToolSettings::default();
        assert_eq!(drawing.active_tool, "select");
        assert_eq!(drawing.selected_color, "#FF3B30");
        assert_eq!(drawing.stroke_width, 4.0);
        assert_eq!(drawing.highlight_opacity, 0.4);
        assert_eq!(drawing.text_font_size, 24.0);
        assert_eq!(crate::ui::colors::VIDEO_DRAWING_TOOLS.len(), 10);
    }
}
