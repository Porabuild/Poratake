//! 1:1 ports of the video editor style types (`src/types/cursor.ts`,
//! `camera.ts`, `keyboard.ts`, `subtitle.ts`, `audio.ts`,
//! `video-wallpaper.ts`, `first-frame.ts` and the export settings in
//! `video.ts`) so `state.json` round-trips between both shells.

use serde::{Deserialize, Serialize};

macro_rules! default_value {
    ($name:ident, $ty:ty, $value:expr) => {
        fn $name() -> $ty {
            $value
        }
    };
}

default_value!(t, bool, true);

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

default_value!(cursor_size, f64, 100.0);
default_value!(black, String, "#000000".into());
default_value!(white, String, "#ffffff".into());
default_value!(two, f64, 2.0);
default_value!(half, f64, 0.5);
default_value!(
    click_highlight_color,
    String,
    "rgba(255, 200, 0, 0.5)".into()
);
default_value!(thirty, f64, 30.0);
default_value!(fifteen, f64, 15.0);
default_value!(ten, f64, 10.0);
default_value!(point_eight, f64, 0.8);

impl Default for CursorStyle {
    fn default() -> Self {
        Self {
            enabled: true,
            size: cursor_size(),
            color: black(),
            border_color: white(),
            border_width: two(),
            smoothing: half(),
            show_click_highlight: true,
            click_highlight_color: click_highlight_color(),
            click_highlight_radius: thirty(),
            click_highlight_duration: fifteen(),
            hide_on_idle: false,
            hide_on_idle_timeout: two(),
            show_trail: false,
            trail_length: ten(),
            trail_opacity_decay: point_eight(),
            motion_blur: true,
            motion_blur_strength: half(),
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

default_value!(camera_position, String, "bottom-right".into());
default_value!(camera_shape, String, "square".into());
default_value!(medium, String, "medium".into());
default_value!(fifty, f64, 50.0);
default_value!(three, f64, 3.0);
default_value!(hundred, f64, 100.0);

impl Default for CameraStyle {
    fn default() -> Self {
        Self {
            visible: true,
            position: camera_position(),
            shape: camera_shape(),
            size: medium(),
            border_radius: fifty(),
            padding: three(),
            shadow: hundred(),
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

default_value!(one, f64, 1.0);
default_value!(bottom_center, String, "bottom-center".into());
default_value!(point_seven_five, f64, 0.75);

impl Default for KeyboardStyle {
    fn default() -> Self {
        Self {
            visible: false,
            display_duration: one(),
            position: bottom_center(),
            font_size: medium(),
            opacity: point_seven_five(),
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

default_value!(bottom, String, "bottom".into());
default_value!(dark, String, "dark".into());
default_value!(point_nine, f64, 0.9);

impl Default for SubtitleStyle {
    fn default() -> Self {
        Self {
            visible: true,
            font_size: medium(),
            position: bottom(),
            background_color: dark(),
            opacity: point_nine(),
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

default_value!(point_seven, f64, 0.7);
default_value!(cherry_blue, String, "cherry-blue".into());

impl Default for AudioStyle {
    fn default() -> Self {
        Self {
            system_audio_enabled: true,
            mic_audio_enabled: true,
            system_audio_volume: one(),
            mic_audio_volume: one(),
            keyboard_sound_enabled: false,
            keyboard_sound_volume: point_seven(),
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

default_value!(cover, String, "cover".into());

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
    #[serde(default = "default_export_resolution")]
    pub resolution: String,
    #[serde(default = "studio")]
    pub quality_preset: String,
    #[serde(default = "default_export_frame_rate")]
    pub frame_rate: String,
    #[serde(default = "t")]
    pub open_in_finder: bool,
}

default_value!(mp4, String, "mp4".into());
default_value!(studio, String, "studio".into());

fn default_export_resolution() -> String {
    MP4_DEFAULTS.resolution.to_string()
}

fn default_export_frame_rate() -> String {
    MP4_DEFAULTS.frame_rate.to_string()
}

impl Default for ExportSettings {
    fn default() -> Self {
        let defaults = MP4_DEFAULTS;
        Self {
            format: mp4(),
            resolution: defaults.resolution.to_string(),
            quality_preset: defaults.quality_preset.to_string(),
            frame_rate: defaults.frame_rate.to_string(),
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

default_value!(one_point_two, f64, 1.2);
default_value!(ease_in_out, String, "ease-in-out".into());
default_value!(point_three, f64, 0.3);
default_value!(point_one_two, f64, 0.12);

impl Default for ZoomSettings {
    fn default() -> Self {
        Self {
            transition_in_duration: one_point_two(),
            transition_out_duration: one_point_two(),
            easing: ease_in_out(),
            follow_smoothness: point_three(),
            look_ahead: point_one_two(),
        }
    }
}

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

pub const EXPORT_FRAME_RATES: [(&str, &str); 7] = [
    ("60", "60 FPS"),
    ("50", "50 FPS"),
    ("30", "30 FPS"),
    ("25", "25 FPS"),
    ("24", "24 FPS"),
    ("20", "20 FPS"),
    ("10", "10 FPS"),
];

pub struct FormatDefaults {
    pub resolution: &'static str,
    pub quality_preset: &'static str,
    pub frame_rate: &'static str,
}

pub const MP4_DEFAULTS: FormatDefaults = FormatDefaults {
    resolution: "4k",
    quality_preset: "studio",
    frame_rate: "30",
};

pub const GIF_DEFAULTS: FormatDefaults = FormatDefaults {
    resolution: "720p",
    quality_preset: "web",
    frame_rate: "20",
};

pub fn format_defaults(format: &str) -> &'static FormatDefaults {
    match format {
        "gif" => &GIF_DEFAULTS,
        _ => &MP4_DEFAULTS,
    }
}

pub fn format_resolutions(format: &str) -> &'static [&'static str] {
    match format {
        "gif" => GIF_RESOLUTIONS,
        _ => MP4_RESOLUTIONS,
    }
}

pub fn format_frame_rates(format: &str) -> &'static [&'static str] {
    match format {
        "gif" => GIF_FRAME_RATES,
        _ => MP4_FRAME_RATES,
    }
}

pub fn apply_format_defaults(settings: &mut ExportSettings) {
    let defaults = format_defaults(&settings.format);
    settings.resolution = defaults.resolution.to_string();
    settings.quality_preset = defaults.quality_preset.to_string();
    settings.frame_rate = defaults.frame_rate.to_string();
}

pub fn normalize_export_settings(settings: &mut ExportSettings) {
    if !EXPORT_FORMATS
        .iter()
        .any(|(value, _)| *value == settings.format)
    {
        settings.format = mp4();
    }
    let defaults = format_defaults(&settings.format);
    if !format_resolutions(&settings.format).contains(&settings.resolution.as_str()) {
        settings.resolution = defaults.resolution.to_string();
    }
    if !EXPORT_QUALITY_PRESETS
        .iter()
        .any(|(value, _)| *value == settings.quality_preset)
    {
        settings.quality_preset = defaults.quality_preset.to_string();
    }
    if !format_frame_rates(&settings.format).contains(&settings.frame_rate.as_str()) {
        settings.frame_rate = defaults.frame_rate.to_string();
    }
}

pub const VIDEO_SLIDER_STEPS: [(&str, f64); 18] = [
    ("cursor-size", 5.0),
    ("cursor-smoothing", 0.1),
    ("cursor-blur-strength", 0.05),
    ("cursor-idle-timeout", 0.5),
    ("zoom-level", 0.25),
    ("zoom-speed", 0.1),
    ("zoom-follow-smoothness", 0.02),
    ("zoom-look-ahead", 0.02),
    ("music-volume", 0.01),
    ("audio-keyboard-volume", 0.01),
    ("drawing-thickness", 1.0),
    ("drawing-redact-intensity", 1.0),
    ("camera-padding", 1.0),
    ("camera-corners", 1.0),
    ("camera-shadow", 1.0),
    ("wallpaper-padding", 1.0),
    ("wallpaper-corners", 1.0),
    ("wallpaper-shadow", 1.0),
];

pub fn slider_step(key: &str) -> Option<f64> {
    VIDEO_SLIDER_STEPS
        .iter()
        .find(|(name, _)| *name == key)
        .map(|(_, step)| *step)
}

pub const MP4_RESOLUTIONS: &[&str] = &["original", "4k", "1080p", "720p", "480p"];
pub const GIF_RESOLUTIONS: &[&str] = &["1080p", "720p", "480p"];
pub const MP4_FRAME_RATES: &[&str] = &["60", "50", "30", "25", "24", "20", "10"];
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
        assert_eq!(json["frameRate"], "30");
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
        assert!(
            !EXPORT_FRAME_RATES.iter().any(|(value, _)| *value == "40"),
            "ALL_FRAMERATE_OPTIONS has no 40 FPS row"
        );
        assert!(!MP4_FRAME_RATES.contains(&"40"));
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

    #[test]
    fn export_defaults_match_the_electron_format_config() {
        assert_eq!(MP4_DEFAULTS.resolution, "4k");
        assert_eq!(MP4_DEFAULTS.quality_preset, "studio");
        assert_eq!(MP4_DEFAULTS.frame_rate, "30");
        assert_eq!(GIF_DEFAULTS.resolution, "720p");
        assert_eq!(GIF_DEFAULTS.quality_preset, "web");
        assert_eq!(GIF_DEFAULTS.frame_rate, "20");
        let defaults = ExportSettings::default();
        assert_eq!(defaults.resolution, "4k");
        assert_eq!(defaults.quality_preset, "studio");
        assert_eq!(defaults.frame_rate, "30");
    }

    #[test]
    fn switching_to_gif_renormalizes_every_dependent_field() {
        let mut settings = ExportSettings {
            format: "gif".to_string(),
            ..ExportSettings::default()
        };
        apply_format_defaults(&mut settings);
        assert_eq!(settings.resolution, "720p");
        assert_eq!(settings.quality_preset, "web");
        assert_eq!(settings.frame_rate, "20");
        normalize_export_settings(&mut settings);
        assert_eq!(settings.frame_rate, "20");
    }

    #[test]
    fn a_gif_never_keeps_an_mp4_only_value() {
        let mut settings = ExportSettings {
            format: "gif".to_string(),
            resolution: "original".to_string(),
            quality_preset: "studio".to_string(),
            frame_rate: "60".to_string(),
            open_in_finder: true,
        };
        normalize_export_settings(&mut settings);
        assert_eq!(settings.resolution, "720p");
        assert_eq!(settings.frame_rate, "20");
        assert_eq!(settings.quality_preset, "studio");
        assert!(GIF_FRAME_RATES.contains(&settings.frame_rate.as_str()));
    }

    #[test]
    fn an_unknown_format_falls_back_to_mp4() {
        let mut settings = ExportSettings {
            format: "webm".to_string(),
            resolution: "nope".to_string(),
            quality_preset: "nope".to_string(),
            frame_rate: "999".to_string(),
            open_in_finder: false,
        };
        normalize_export_settings(&mut settings);
        assert_eq!(settings.format, "mp4");
        assert_eq!(settings.resolution, "4k");
        assert_eq!(settings.quality_preset, "studio");
        assert_eq!(settings.frame_rate, "30");
    }

    #[test]
    fn every_slider_step_matches_its_electron_counterpart() {
        for (key, expected) in [
            ("cursor-size", 5.0),
            ("cursor-smoothing", 0.1),
            ("cursor-blur-strength", 0.05),
            ("cursor-idle-timeout", 0.5),
            ("zoom-level", 0.25),
            ("zoom-speed", 0.1),
            ("zoom-follow-smoothness", 0.02),
            ("zoom-look-ahead", 0.02),
            ("music-volume", 0.01),
            ("audio-keyboard-volume", 0.01),
            ("drawing-thickness", 1.0),
            ("camera-padding", 1.0),
            ("wallpaper-padding", 1.0),
        ] {
            assert_eq!(slider_step(key), Some(expected), "{key}");
        }
    }

    #[test]
    fn slider_steps_are_unique_and_positive() {
        for (index, (key, step)) in VIDEO_SLIDER_STEPS.iter().enumerate() {
            assert!(*step > 0.0, "{key} has no step");
            assert!(
                !VIDEO_SLIDER_STEPS[..index]
                    .iter()
                    .any(|(other, _)| other == key),
                "{key} is listed twice"
            );
        }
        assert_eq!(slider_step("not-a-slider"), None);
    }
}
