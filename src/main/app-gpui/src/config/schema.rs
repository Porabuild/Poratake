//! Remaining `SettingsConfig` sections: appearance-adjacent preferences,
//! editor defaults, cloud, recording, wallpaper and history.

use serde::{Deserialize, Serialize};

use crate::config::shortcuts::{GeneralConfig, ScreenshotConfig, ShortcutsConfig};

fn default_true() -> bool {
    true
}

// ---------------------------------------------------------------------------
// Editor preferences (types/settings.ts `EditorPreferences`)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EditorPreferences {
    #[serde(default = "ep_last_tool")]
    pub last_tool: String,
    #[serde(default = "ep_color")]
    pub color: String,
    #[serde(default = "ep_stroke_width")]
    pub stroke_width: f64,
    #[serde(default = "ep_arrow_style")]
    pub arrow_style: String,
    #[serde(default = "ep_highlight_color")]
    pub highlight_color: String,
    #[serde(default = "ep_highlight_opacity")]
    pub highlight_opacity: f64,
    #[serde(default = "ep_number_style")]
    pub number_style: String,
    #[serde(default = "ep_number_size")]
    pub number_size: String,
    #[serde(default = "ep_number_start_value")]
    pub number_start_value: f64,
    #[serde(default = "default_true")]
    pub text_background: bool,
    #[serde(default = "ep_text_font_size")]
    pub text_font_size: f64,
    #[serde(default = "ep_text_font_family")]
    pub text_font_family: String,
    #[serde(default = "ep_redact_style")]
    pub redact_style: String,
    #[serde(default = "ep_redact_intensity")]
    pub redact_intensity: f64,
    #[serde(default = "ep_shape_fill_mode")]
    pub shape_fill_mode: String,
}

impl Default for EditorPreferences {
    fn default() -> Self {
        Self {
            last_tool: ep_last_tool(),
            color: ep_color(),
            stroke_width: 3.0,
            arrow_style: ep_arrow_style(),
            highlight_color: ep_highlight_color(),
            highlight_opacity: 0.4,
            number_style: ep_number_style(),
            number_size: ep_number_size(),
            number_start_value: 1.0,
            text_background: true,
            text_font_size: 20.0,
            text_font_family: ep_text_font_family(),
            redact_style: ep_redact_style(),
            redact_intensity: 5.0,
            shape_fill_mode: ep_shape_fill_mode(),
        }
    }
}

fn ep_last_tool() -> String {
    "select".into()
}
fn ep_color() -> String {
    "#FF3B30".into()
}
fn ep_stroke_width() -> f64 {
    3.0
}
fn ep_highlight_opacity() -> f64 {
    0.4
}
fn ep_arrow_style() -> String {
    "standard".into()
}
fn ep_highlight_color() -> String {
    "#FFFF00".into()
}
fn ep_number_style() -> String {
    "numeric".into()
}
fn ep_number_size() -> String {
    "medium".into()
}
fn ep_number_start_value() -> f64 {
    1.0
}
fn ep_text_font_size() -> f64 {
    20.0
}
fn ep_redact_intensity() -> f64 {
    5.0
}
fn ep_text_font_family() -> String {
    "sans".into()
}
fn ep_redact_style() -> String {
    "pixelate".into()
}
fn ep_shape_fill_mode() -> String {
    "outline".into()
}

// ---------------------------------------------------------------------------
// Storage / save locations / preview
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StorageConfig {
    #[serde(default)]
    pub screenshots_path: String,
    #[serde(default)]
    pub recordings_path: String,
    #[serde(default = "st_naming_pattern")]
    pub naming_pattern: String,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            screenshots_path: String::new(),
            recordings_path: String::new(),
            naming_pattern: st_naming_pattern(),
        }
    }
}

/// `DEFAULT_STORAGE_CONFIG.namingPattern`, which the settings row's reset
/// button writes back.
pub fn default_naming_pattern() -> String {
    st_naming_pattern()
}

fn st_naming_pattern() -> String {
    "%type %Y-%m-%d at %H.%M.%S".to_string()
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub struct SaveLocationsConfig {
    #[serde(default)]
    pub screenshot: String,
    #[serde(default)]
    pub video: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PreviewConfig {
    #[serde(default)]
    pub display_id: Option<i64>,
    #[serde(default = "pv_corner")]
    pub corner: String,
    #[serde(default = "default_true")]
    pub auto_dismiss: bool,
    #[serde(default = "pv_auto_dismiss_seconds")]
    pub auto_dismiss_seconds: f64,
}

impl Default for PreviewConfig {
    fn default() -> Self {
        Self {
            display_id: None,
            corner: pv_corner(),
            auto_dismiss: true,
            auto_dismiss_seconds: 10.0,
        }
    }
}

fn pv_corner() -> String {
    "bottom-right".to_string()
}

fn pv_auto_dismiss_seconds() -> f64 {
    10.0
}

// ---------------------------------------------------------------------------
// History (types/history.ts)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "h_max_items")]
    pub max_items: f64,
    #[serde(default = "h_filter")]
    pub filter: String,
    #[serde(default = "h_sort_order")]
    pub sort_order: String,
    #[serde(default = "h_layout")]
    pub layout: String,
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_items: 50.0,
            filter: h_filter(),
            sort_order: h_sort_order(),
            layout: h_layout(),
        }
    }
}

fn h_max_items() -> f64 {
    50.0
}
fn h_filter() -> String {
    "all".into()
}
fn h_sort_order() -> String {
    "newest".into()
}
fn h_layout() -> String {
    "grid".into()
}

// ---------------------------------------------------------------------------
// Onboarding
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub struct OnboardingConfig {
    #[serde(default)]
    pub completed: bool,
    #[serde(default)]
    pub skipped: bool,
}

// ---------------------------------------------------------------------------
// Cloud (types/settings.ts) — secrets stay opaque strings; values written by
// the Electron shell may carry the safe-storage prefix and must pass through.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub struct S3ProviderConfig {
    #[serde(default)]
    pub endpoint: String,
    #[serde(default)]
    pub region: String,
    #[serde(default)]
    pub bucket: String,
    #[serde(default)]
    pub access_key_id: String,
    #[serde(default)]
    pub secret_access_key: String,
    #[serde(default)]
    pub path_prefix: String,
    #[serde(default)]
    pub custom_domain: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub struct RestHeader {
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub value: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RestProviderConfig {
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub headers: Vec<RestHeader>,
    #[serde(default = "cl_file_field_name")]
    pub file_field_name: String,
    #[serde(default)]
    pub response_is_plain_text: bool,
    #[serde(default)]
    pub response_url_path: String,
}

impl Default for RestProviderConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            headers: Vec::new(),
            file_field_name: cl_file_field_name(),
            response_is_plain_text: false,
            response_url_path: String::new(),
        }
    }
}

fn cl_file_field_name() -> String {
    "file".to_string()
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CloudConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "cl_active_provider")]
    pub active_provider: String,
    #[serde(default)]
    pub s3: S3ProviderConfig,
    #[serde(default)]
    pub rest: RestProviderConfig,
}

impl Default for CloudConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            active_provider: cl_active_provider(),
            s3: S3ProviderConfig::default(),
            rest: RestProviderConfig::default(),
        }
    }
}

fn cl_active_provider() -> String {
    "s3".to_string()
}

// ---------------------------------------------------------------------------
// Recording (types/settings.ts)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CameraPosition {
    #[serde(default)]
    pub x: f64,
    #[serde(default)]
    pub y: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CameraSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub selected_device_id: Option<String>,
    #[serde(default)]
    pub selected_device_name: Option<String>,
    #[serde(default = "cam_shape")]
    pub shape: String,
    #[serde(default = "cam_size")]
    pub size: String,
    #[serde(default)]
    pub position: Option<CameraPosition>,
    #[serde(default = "cam_resolution")]
    pub resolution: String,
    #[serde(default)]
    pub flipped: bool,
}

impl Default for CameraSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            selected_device_id: None,
            selected_device_name: None,
            shape: cam_shape(),
            size: cam_size(),
            position: None,
            resolution: cam_resolution(),
            flipped: false,
        }
    }
}

fn cam_shape() -> String {
    "rounded".into()
}
fn cam_size() -> String {
    "large".into()
}
fn cam_resolution() -> String {
    "720p".into()
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct IosDeviceSettings {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RecordingSettings {
    #[serde(default)]
    pub auto_zoom: bool,
    #[serde(default = "default_true")]
    pub show_preview: bool,
    #[serde(default = "rec_start_delay")]
    pub start_delay: f64,
    #[serde(default = "rec_frame_rate")]
    pub frame_rate: u32,
    #[serde(default = "default_true")]
    pub system_audio: bool,
    #[serde(default)]
    pub mic_enabled: bool,
    #[serde(default)]
    pub selected_mic_id: Option<String>,
    #[serde(default)]
    pub selected_mic_name: Option<String>,
    #[serde(default)]
    pub camera: CameraSettings,
    #[serde(default)]
    pub ios_device: Option<IosDeviceSettings>,
}

impl Default for RecordingSettings {
    fn default() -> Self {
        Self {
            auto_zoom: false,
            show_preview: true,
            start_delay: 3.0,
            frame_rate: 60,
            system_audio: true,
            mic_enabled: false,
            selected_mic_id: None,
            selected_mic_name: None,
            camera: CameraSettings::default(),
            ios_device: None,
        }
    }
}

fn rec_start_delay() -> f64 {
    3.0
}

fn rec_frame_rate() -> u32 {
    60
}

// ---------------------------------------------------------------------------
// All-in-one + scroll capture
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct AllInOneLastArea {
    #[serde(default)]
    pub x: f64,
    #[serde(default)]
    pub y: f64,
    #[serde(default)]
    pub width: f64,
    #[serde(default)]
    pub height: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AllInOneRecordingChoices {
    #[serde(default = "default_true")]
    pub system_audio: bool,
    #[serde(default)]
    pub mic_enabled: bool,
    #[serde(default)]
    pub camera_enabled: bool,
}

impl Default for AllInOneRecordingChoices {
    fn default() -> Self {
        Self {
            system_audio: true,
            mic_enabled: false,
            camera_enabled: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AllInOneTargets {
    #[serde(default = "ao_target")]
    pub screenshot: String,
    #[serde(default = "ao_target")]
    pub record: String,
}

impl Default for AllInOneTargets {
    fn default() -> Self {
        Self {
            screenshot: ao_target(),
            record: ao_target(),
        }
    }
}

fn ao_target() -> String {
    "area".into()
}

fn ao_last_mode() -> String {
    "screenshot".into()
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AllInOneConfig {
    #[serde(default = "default_true")]
    pub remember_choices: bool,
    #[serde(default = "ao_last_mode")]
    pub last_mode: String,
    #[serde(default)]
    pub last_targets: AllInOneTargets,
    #[serde(default)]
    pub last_area: Option<AllInOneLastArea>,
    #[serde(default)]
    pub recording: AllInOneRecordingChoices,
}

impl Default for AllInOneConfig {
    fn default() -> Self {
        Self {
            remember_choices: true,
            last_mode: ao_last_mode(),
            last_targets: AllInOneTargets::default(),
            last_area: None,
            recording: AllInOneRecordingChoices::default(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScrollCaptureConfig {
    #[serde(default = "sc_speed")]
    pub auto_scroll_speed: String,
    #[serde(default = "sc_max_height")]
    pub max_height: f64,
}

impl Default for ScrollCaptureConfig {
    fn default() -> Self {
        Self {
            auto_scroll_speed: sc_speed(),
            max_height: 20000.0,
        }
    }
}

fn sc_speed() -> String {
    "medium".into()
}

fn sc_max_height() -> f64 {
    20000.0
}

// ---------------------------------------------------------------------------
// Wallpaper (types/settings.ts)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GradientOption {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub colors: Vec<String>,
    #[serde(default)]
    pub angle: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GradientBackgroundData {
    #[serde(flatten)]
    pub gradient: GradientOption,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ImageBackgroundData {
    #[serde(default)]
    pub image_url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum CustomBackgroundData {
    Gradient { data: GradientBackgroundData },
    Image { data: ImageBackgroundData },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CustomBackground {
    #[serde(default)]
    pub id: String,
    #[serde(flatten)]
    pub data: CustomBackgroundData,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WallpaperPreset {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub gradient: Option<GradientOption>,
    #[serde(default)]
    pub background_image: Option<String>,
    #[serde(default)]
    pub background_blur: Option<f64>,
    #[serde(default)]
    pub noise: Option<f64>,
    #[serde(default)]
    pub padding: f64,
    #[serde(default)]
    pub corners: f64,
    #[serde(default)]
    pub shadow: f64,
    #[serde(default)]
    pub spacing: Option<f64>,
    #[serde(default)]
    pub window_frame: Option<WindowFrameSettings>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WindowFrameSettings {
    #[serde(default = "wf_style")]
    pub style: String,
}

impl Default for WindowFrameSettings {
    fn default() -> Self {
        Self { style: wf_style() }
    }
}

fn wf_style() -> String {
    "none".into()
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CustomGradient {
    #[serde(flatten)]
    pub gradient: GradientOption,
    #[serde(default)]
    pub name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub struct WallpaperConfig {
    #[serde(default)]
    pub custom_backgrounds: Vec<CustomBackground>,
    #[serde(default)]
    pub presets: Vec<WallpaperPreset>,
    #[serde(default)]
    pub custom_gradients: Option<Vec<CustomGradient>>,
    #[serde(default)]
    pub default_preset_id: Option<String>,
}

// ---------------------------------------------------------------------------
// Root config (settings.ts `SettingsConfig`)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
#[derive(Default)]
pub struct SettingsConfig {
    pub appearance: Appearance,
    pub general: GeneralConfig,
    pub screenshot: ScreenshotConfig,
    pub shortcuts: ShortcutsConfig,
    pub editor: EditorPreferences,
    pub wallpaper: WallpaperConfig,
    pub history: HistoryConfig,
    pub onboarding: OnboardingConfig,
    pub cloud: CloudConfig,
    pub recording: RecordingSettings,
    pub storage: StorageConfig,
    pub save_locations: SaveLocationsConfig,
    pub preview: PreviewConfig,
    pub all_in_one: AllInOneConfig,
    pub scroll_capture: ScrollCaptureConfig,
}

/// Appearance (types/settings.ts `AppearanceConfig`).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Appearance {
    #[serde(default = "ap_mode")]
    pub mode: String,
    #[serde(default = "ap_theme")]
    pub theme: String,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            mode: ap_mode(),
            theme: ap_theme(),
        }
    }
}

fn ap_mode() -> String {
    "dark".to_string()
}

fn ap_theme() -> String {
    "default".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The GPUI shell reads and writes the same `config.json` as the Electron
    /// shell, so every serialized key has to match `src/types/settings.ts`
    /// exactly — including the ones camelCase cannot derive.
    #[test]
    fn serializes_the_keys_the_electron_shell_writes() {
        let json = serde_json::to_value(SettingsConfig::default()).expect("settings");

        let shortcuts = &json["shortcuts"];
        for key in [
            "screenshot",
            "captureText",
            "scanQRCode",
            "timerCapture",
            "scrollCapture",
            "recording",
            "history",
            "allInOne",
            "openInEditor",
            "clipboardInEditor",
            "editor",
            "editorActions",
            "videoEditorSidebar",
        ] {
            assert!(shortcuts.get(key).is_some(), "missing shortcuts.{key}");
        }

        let sidebar = &shortcuts["videoEditorSidebar"];
        for key in [
            "cursor",
            "zoom",
            "drawing",
            "camera",
            "audio",
            "wallpaper",
            "keyboard",
            "subtitle",
            "first-frame",
            "export",
        ] {
            assert!(
                sidebar.get(key).is_some(),
                "missing shortcuts.videoEditorSidebar.{key}"
            );
        }

        for key in [
            "general",
            "shortcuts",
            "editor",
            "wallpaper",
            "history",
            "onboarding",
            "cloud",
            "recording",
            "storage",
            "saveLocations",
            "preview",
            "allInOne",
        ] {
            assert!(json.get(key).is_some(), "missing {key}");
        }
        assert_eq!(json["recording"]["frameRate"], 60);
    }

    #[test]
    fn round_trips_through_the_shared_config_file() {
        let original = SettingsConfig::default();
        let encoded = serde_json::to_string(&original).expect("encode");
        let decoded: SettingsConfig = serde_json::from_str(&encoded).expect("decode");
        assert_eq!(decoded, original);
    }
}
