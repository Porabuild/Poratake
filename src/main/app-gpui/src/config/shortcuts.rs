//! 1:1 port of `src/types/settings.ts` (and the type files it folds in) as
//! serde structures that read and write the exact same `config.json` the
//! Electron shell uses, so both shells stay interchangeable.

use serde::{Deserialize, Serialize};

fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GeneralConfig {
    #[serde(default = "default_true")]
    pub start_on_login: bool,
    #[serde(default)]
    pub play_sound_on_screenshot: bool,
    #[serde(default)]
    pub hide_menu_bar_icon: bool,
    #[serde(default = "default_true")]
    pub show_deletion_notifications: bool,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            start_on_login: true,
            play_sound_on_screenshot: false,
            hide_menu_bar_icon: false,
            show_deletion_notifications: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScreenshotConfig {
    #[serde(default)]
    pub close_on_copy: bool,
    #[serde(default)]
    pub close_on_save: bool,
    #[serde(default)]
    pub capture_to_clipboard: bool,
    #[serde(default = "default_true")]
    pub auto_copy_to_clipboard: bool,
    #[serde(default = "default_true")]
    pub show_preview: bool,
    #[serde(default = "default_true")]
    pub hide_desktop_icons: bool,
    #[serde(default = "default_true")]
    pub freeze_screen: bool,
    #[serde(default = "default_format")]
    pub format: String,
    #[serde(default = "default_attach_edge")]
    pub multi_image_attach_edge: String,
}

impl Default for ScreenshotConfig {
    fn default() -> Self {
        Self {
            close_on_copy: false,
            close_on_save: false,
            capture_to_clipboard: false,
            auto_copy_to_clipboard: true,
            show_preview: true,
            hide_desktop_icons: true,
            freeze_screen: true,
            format: default_format(),
            multi_image_attach_edge: default_attach_edge(),
        }
    }
}

fn default_format() -> String {
    "png".to_string()
}

fn default_attach_edge() -> String {
    "right".to_string()
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EditorShortcuts {
    #[serde(default = "sc_pen")]
    pub pen: String,
    #[serde(default = "sc_highlight")]
    pub highlight: String,
    #[serde(default = "sc_rectangle")]
    pub rectangle: String,
    #[serde(default = "sc_circle")]
    pub circle: String,
    #[serde(default = "sc_line")]
    pub line: String,
    #[serde(default = "sc_arrow")]
    pub arrow: String,
    #[serde(default = "sc_text")]
    pub text: String,
    #[serde(default = "sc_number")]
    pub number: String,
    #[serde(default = "sc_redact")]
    pub redact: String,
    #[serde(default = "sc_select")]
    pub select: String,
    #[serde(default = "sc_crop")]
    pub crop: String,
    #[serde(default = "sc_wallpaper")]
    pub wallpaper: String,
}

impl Default for EditorShortcuts {
    fn default() -> Self {
        Self {
            pen: sc_pen(),
            highlight: sc_highlight(),
            rectangle: sc_rectangle(),
            circle: sc_circle(),
            line: sc_line(),
            arrow: sc_arrow(),
            text: sc_text(),
            number: sc_number(),
            redact: sc_redact(),
            select: sc_select(),
            crop: sc_crop(),
            wallpaper: sc_wallpaper(),
        }
    }
}

fn sc_pen() -> String {
    "p".into()
}
fn sc_highlight() -> String {
    "h".into()
}
fn sc_rectangle() -> String {
    "r".into()
}
fn sc_circle() -> String {
    "o".into()
}
fn sc_line() -> String {
    "l".into()
}
fn sc_arrow() -> String {
    "a".into()
}
fn sc_text() -> String {
    "t".into()
}
fn sc_number() -> String {
    "n".into()
}
fn sc_redact() -> String {
    "x".into()
}
fn sc_select() -> String {
    "v".into()
}
fn sc_crop() -> String {
    "c".into()
}
fn sc_wallpaper() -> String {
    "w".into()
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VideoEditorSidebarShortcuts {
    #[serde(default = "ve_cursor")]
    pub cursor: String,
    #[serde(default = "ve_zoom")]
    pub zoom: String,
    #[serde(default = "ve_drawing")]
    pub drawing: String,
    #[serde(default = "ve_camera")]
    pub camera: String,
    #[serde(default = "ve_audio")]
    pub audio: String,
    #[serde(default = "sc_wallpaper")]
    pub wallpaper: String,
    #[serde(default = "ve_keyboard")]
    pub keyboard: String,
    #[serde(default = "ve_subtitle")]
    pub subtitle: String,
    #[serde(default = "ve_first_frame", rename = "first-frame")]
    pub first_frame: String,
    #[serde(default = "ve_export")]
    pub export: String,
}

impl Default for VideoEditorSidebarShortcuts {
    fn default() -> Self {
        Self {
            cursor: ve_cursor(),
            zoom: ve_zoom(),
            drawing: ve_drawing(),
            camera: ve_camera(),
            audio: ve_audio(),
            wallpaper: sc_wallpaper(),
            keyboard: ve_keyboard(),
            subtitle: ve_subtitle(),
            first_frame: ve_first_frame(),
            export: ve_export(),
        }
    }
}

fn ve_cursor() -> String {
    "q".into()
}
fn ve_zoom() -> String {
    "z".into()
}
fn ve_drawing() -> String {
    "d".into()
}
fn ve_camera() -> String {
    "m".into()
}
fn ve_audio() -> String {
    "a".into()
}
fn ve_keyboard() -> String {
    "k".into()
}
fn ve_subtitle() -> String {
    "s".into()
}
fn ve_first_frame() -> String {
    "f".into()
}
fn ve_export() -> String {
    "e".into()
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub struct RecordingShortcuts {
    #[serde(default)]
    pub area: String,
    #[serde(default)]
    pub screen: String,
    #[serde(default)]
    pub window: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutsConfig {
    #[serde(default)]
    pub screenshot: ScreenshotShortcuts,
    #[serde(default)]
    pub capture_text: String,
    #[serde(default, rename = "scanQRCode")]
    pub scan_qrcode: String,
    #[serde(default)]
    pub timer_capture: String,
    #[serde(default)]
    pub scroll_capture: String,
    #[serde(default)]
    pub recording: RecordingShortcuts,
    #[serde(default)]
    pub history: String,
    #[serde(default)]
    pub all_in_one: String,
    #[serde(default)]
    pub open_in_editor: String,
    #[serde(default)]
    pub clipboard_in_editor: String,
    #[serde(default)]
    pub editor: EditorShortcuts,
    #[serde(default)]
    pub editor_actions: EditorActionShortcuts,
    #[serde(default)]
    pub video_editor_sidebar: VideoEditorSidebarShortcuts,
}

impl Default for ShortcutsConfig {
    fn default() -> Self {
        let modifiers = default_global_shortcut_modifiers();
        Self {
            screenshot: ScreenshotShortcuts {
                area: format!("{modifiers}+4"),
                window: default_screenshot_window_shortcut(),
                screen: format!("{modifiers}+3"),
            },
            capture_text: String::new(),
            scan_qrcode: String::new(),
            timer_capture: String::new(),
            scroll_capture: String::new(),
            recording: RecordingShortcuts::default(),
            history: String::new(),
            all_in_one: default_all_in_one_shortcut(),
            open_in_editor: String::new(),
            clipboard_in_editor: String::new(),
            editor: EditorShortcuts::default(),
            editor_actions: EditorActionShortcuts::default(),
            video_editor_sidebar: VideoEditorSidebarShortcuts::default(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub struct ScreenshotShortcuts {
    #[serde(default)]
    pub area: String,
    #[serde(default)]
    pub window: String,
    #[serde(default)]
    pub screen: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EditorActionShortcuts {
    #[serde(default = "default_upload_to_cloud_shortcut")]
    pub upload_to_cloud: String,
}

impl Default for EditorActionShortcuts {
    fn default() -> Self {
        Self {
            upload_to_cloud: default_upload_to_cloud_shortcut(),
        }
    }
}

pub fn default_global_shortcut_modifiers() -> &'static str {
    if cfg!(windows) {
        "Alt+Shift"
    } else {
        "CommandOrControl+Shift"
    }
}

pub fn default_all_in_one_shortcut() -> String {
    format!("{}+S", default_global_shortcut_modifiers())
}

pub fn default_screenshot_window_shortcut() -> String {
    if cfg!(windows) {
        format!("{}+2", default_global_shortcut_modifiers())
    } else {
        format!("{}+5", default_global_shortcut_modifiers())
    }
}

pub const DEFAULT_UPLOAD_TO_CLOUD_SHORTCUT: &str = "CommandOrControl+Shift+U";

pub fn default_upload_to_cloud_shortcut() -> String {
    DEFAULT_UPLOAD_TO_CLOUD_SHORTCUT.to_string()
}
