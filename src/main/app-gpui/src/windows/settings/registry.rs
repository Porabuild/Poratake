//! 1:1 port of `renderer/components/settings/settings-registry.ts` and the
//! per-category item tables it composes. Keeping settings as data rather than
//! hand-written rows is what makes search, section grouping and feature
//! gating work the same way in both shells.

use crate::config::schema::SettingsConfig;
use crate::system::capabilities::{is_supported, Feature};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Category {
    General,
    Appearance,
    Screenshot,
    Recording,
    Devices,
    Storage,
    Shortcuts,
    Cloud,
    About,
}

impl Category {
    const ALL: [Category; 9] = [
        Self::General,
        Self::Appearance,
        Self::Screenshot,
        Self::Recording,
        Self::Devices,
        Self::Storage,
        Self::Shortcuts,
        Self::Cloud,
        Self::About,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Self::General => "general",
            Self::Appearance => "appearance",
            Self::Screenshot => "screenshot",
            Self::Recording => "recording",
            Self::Devices => "devices",
            Self::Storage => "storage",
            Self::Shortcuts => "shortcuts",
            Self::Cloud => "cloud",
            Self::About => "about",
        }
    }

    /// The inverse of [`Category::id`]. `settings-window.tsx` reads the tab from
    /// `window.location.hash`, so a category is addressable by name there; this
    /// is what makes it addressable here.
    pub fn from_id(id: &str) -> Option<Self> {
        Self::supported()
            .into_iter()
            .chain(std::iter::once(Self::About))
            .find(|category| category.id() == id)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Appearance => "Appearance",
            Self::Screenshot => "Screenshot",
            Self::Recording => "Recording",
            Self::Devices => "Devices",
            Self::Storage => "Storage",
            Self::Shortcuts => "Shortcuts",
            Self::Cloud => "Cloud",
            Self::About => "About",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Self::General => "settings",
            Self::Appearance => "palette",
            Self::Screenshot => "camera",
            Self::Recording => "video",
            Self::Devices => "webcam",
            Self::Storage => "hard-drive",
            Self::Shortcuts => "keyboard",
            Self::Cloud => "cloud",
            Self::About => "info",
        }
    }

    pub fn searchable(self) -> bool {
        self != Self::About
    }

    fn feature(self) -> Option<Feature> {
        match self {
            Self::Recording | Self::Devices => Some(Feature::Recording),
            _ => None,
        }
    }

    pub fn supported() -> Vec<Category> {
        Self::ALL
            .into_iter()
            .filter(|category| category.feature().is_none_or(is_supported))
            .collect()
    }
}

pub type Options = &'static [(&'static str, &'static str)];

/// Select options are usually a fixed table, but the theme picker is derived
/// from `APP_THEME_PRESETS` so the two lists cannot drift apart.
#[derive(Clone, Copy)]
pub enum OptionSource {
    Static(Options),
    Themes,
}

impl OptionSource {
    pub fn resolve(self) -> Vec<(&'static str, &'static str)> {
        match self {
            Self::Static(options) => options.to_vec(),
            Self::Themes => crate::theme::presets::theme_options(),
        }
    }
}

pub enum Control {
    Switch {
        get: fn(&SettingsConfig) -> bool,
        set: fn(&mut SettingsConfig, bool),
        disabled: Option<fn(&SettingsConfig) -> bool>,
    },
    Select {
        options: OptionSource,
        get: fn(&SettingsConfig) -> String,
        set: fn(&mut SettingsConfig, &str),
    },
    Slider {
        min: f64,
        max: f64,
        step: f64,
        get: fn(&SettingsConfig) -> f64,
        set: fn(&mut SettingsConfig, f64),
    },
    Input {
        placeholder: &'static str,
        hint: Option<&'static str>,
        secret: bool,
        get: fn(&SettingsConfig) -> String,
        set: fn(&mut SettingsConfig, &str),
    },
    Shortcut {
        get: fn(&SettingsConfig) -> String,
        set: fn(&mut SettingsConfig, &str),
        /// `singleKey` in `registry/shortcuts.ts`: the editor tool and video
        /// editor sidebar bindings are bare keys, everything else is a global
        /// accelerator and needs at least one modifier.
        single_key: bool,
    },
    PathPicker {
        kind: PathKind,
    },
    NamingPattern,
    CloudTestConnection,
    RestHeaders,
    MicrophoneDevice,
    CameraDevice,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PathKind {
    Screenshots,
    Recordings,
}

pub struct Item {
    pub id: &'static str,
    pub category: Category,
    pub section: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    pub keywords: &'static str,
    pub feature: Option<Feature>,
    pub visible_when: Option<fn(&SettingsConfig) -> bool>,
    pub control: Control,
}

impl Item {
    pub fn is_visible(&self, config: &SettingsConfig) -> bool {
        self.feature.is_none_or(is_supported)
            && self.visible_when.is_none_or(|predicate| predicate(config))
    }

    pub fn matches(&self, query: &str) -> bool {
        let haystack = format!(
            "{} {} {} {}",
            self.label, self.description, self.section, self.keywords
        )
        .to_lowercase();
        query
            .split_whitespace()
            .all(|term| haystack.contains(&term.to_lowercase()))
    }
}

const PREVIEW_CORNERS: Options = &[
    ("bottom-right", "Bottom right"),
    ("bottom-left", "Bottom left"),
    ("top-right", "Top right"),
    ("top-left", "Top left"),
];

const DISMISS_SECONDS: Options = &[
    ("3", "3 seconds"),
    ("5", "5 seconds"),
    ("10", "10 seconds"),
    ("15", "15 seconds"),
    ("30", "30 seconds"),
    ("60", "1 minute"),
];

const APPEARANCE_MODES: Options = &[("system", "System"), ("light", "Light"), ("dark", "Dark")];

const SCREENSHOT_FORMATS: Options = &[("png", "PNG"), ("jpeg", "JPEG")];

const ATTACH_EDGES: Options = &[
    ("right", "Right (side-by-side)"),
    ("left", "Left"),
    ("bottom", "Bottom (stacked)"),
    ("top", "Top"),
];

const RECORDING_FRAME_RATES: Options = &[
    ("30", "30 FPS"),
    ("60", "60 FPS"),
    ("90", "90 FPS"),
    ("120", "120 FPS"),
    ("144", "144 FPS"),
    ("165", "165 FPS"),
    ("240", "240 FPS"),
];

const CLOUD_PROVIDERS: Options = &[
    ("rest", "Self-hosted cloud"),
    ("s3", "S3-compatible storage"),
];

fn is_s3(config: &SettingsConfig) -> bool {
    config.cloud.active_provider == "s3"
}

fn is_rest(config: &SettingsConfig) -> bool {
    config.cloud.active_provider == "rest"
}

macro_rules! switch {
    ($id:literal, $category:expr, $section:literal, $label:literal, $description:literal,
     $keywords:literal, $get:expr, $set:expr $(, feature = $feature:expr)?
     $(, visible = $visible:expr)? $(, disabled = $disabled:expr)?) => {
        Item {
            id: $id,
            category: $category,
            section: $section,
            label: $label,
            description: $description,
            keywords: $keywords,
            feature: { #[allow(unused_variables)] let value: Option<Feature> = None; $(let value = Some($feature);)? value },
            visible_when: { #[allow(unused_variables)] let value: Option<fn(&SettingsConfig) -> bool> = None; $(let value = Some($visible as fn(&SettingsConfig) -> bool);)? value },
            control: Control::Switch {
                get: $get as fn(&SettingsConfig) -> bool,
                set: $set as fn(&mut SettingsConfig, bool),
                disabled: { #[allow(unused_variables)] let value: Option<fn(&SettingsConfig) -> bool> = None; $(let value = Some($disabled as fn(&SettingsConfig) -> bool);)? value },
            },
        }
    };
}

macro_rules! select {
    ($id:literal, $category:expr, $section:literal, $label:literal, $description:literal,
     $keywords:literal, $options:expr, $get:expr, $set:expr
     $(, visible = $visible:expr)? $(, feature = $feature:expr)?) => {
        Item {
            id: $id,
            category: $category,
            section: $section,
            label: $label,
            description: $description,
            keywords: $keywords,
            feature: { #[allow(unused_variables)] let value: Option<Feature> = None; $(let value = Some($feature);)? value },
            visible_when: { #[allow(unused_variables)] let value: Option<fn(&SettingsConfig) -> bool> = None; $(let value = Some($visible as fn(&SettingsConfig) -> bool);)? value },
            control: Control::Select {
                options: $options,
                get: $get as fn(&SettingsConfig) -> String,
                set: $set as fn(&mut SettingsConfig, &str),
            },
        }
    };
}

macro_rules! input {
    ($id:literal, $category:expr, $section:literal, $label:literal, $description:literal,
     $keywords:literal, $placeholder:literal, $get:expr, $set:expr
     $(, hint = $hint:literal)? $(, secret = $secret:literal)? $(, visible = $visible:expr)?) => {
        Item {
            id: $id,
            category: $category,
            section: $section,
            label: $label,
            description: $description,
            keywords: $keywords,
            feature: None,
            visible_when: { #[allow(unused_variables)] let value: Option<fn(&SettingsConfig) -> bool> = None; $(let value = Some($visible as fn(&SettingsConfig) -> bool);)? value },
            control: Control::Input {
                placeholder: $placeholder,
                hint: { #[allow(unused_variables)] let value: Option<&'static str> = None; $(let value = Some($hint);)? value },
                secret: { #[allow(unused_variables)] let value = false; $(let value = $secret;)? value },
                get: $get as fn(&SettingsConfig) -> String,
                set: $set as fn(&mut SettingsConfig, &str),
            },
        }
    };
}

pub fn items() -> Vec<Item> {
    let mut items = general_items();
    items.extend(appearance_items());
    items.extend(screenshot_items());
    items.extend(recording_items());
    items.extend(devices_items());
    items.extend(storage_items());
    items.extend(crate::windows::settings::shortcut_items::items());
    items.extend(cloud_items());
    items
        .into_iter()
        .filter(|item| item.feature.is_none_or(is_supported))
        .filter(|item| item.category.feature().is_none_or(is_supported))
        .collect()
}

fn general_items() -> Vec<Item> {
    vec![
        switch!(
            "general.startOnLogin",
            Category::General,
            "Application",
            "Start on login",
            "Launch Poratake automatically when you log in",
            "startup launch boot login auto start",
            |config| config.general.start_on_login,
            |config, value| {
                config.general.start_on_login = value;
                // `updateConfig` in the Electron shell calls
                // `setLoginItemSettings` the moment the toggle flips; this
                // setter is that moment here.
                crate::system::startup::set_open_at_login(value);
            }
        ),
        switch!(
            "general.playSound",
            Category::General,
            "Application",
            "Play sound",
            "Play a sound effect when taking screenshots",
            "audio sound effect noise capture sound",
            |config| config.general.play_sound_on_screenshot,
            |config, value| config.general.play_sound_on_screenshot = value,
            feature = Feature::CaptureSound
        ),
        switch!(
            "general.showDeletionNotifications",
            Category::General,
            "Application",
            "Show deletion notifications",
            "Show a notification when a screenshot or video is permanently deleted",
            "notification delete deletion remove screenshot video recording",
            |config| config.general.show_deletion_notifications,
            |config, value| config.general.show_deletion_notifications = value
        ),
        select!(
            "general.previewCorner",
            Category::General,
            "Preview",
            "Preview corner",
            "Corner of the screen where capture previews appear and stack up",
            "preview corner position placement thumbnail bottom top left right",
            OptionSource::Static(PREVIEW_CORNERS),
            |config| config.preview.corner.clone(),
            |config, value| config.preview.corner = value.to_string()
        ),
        switch!(
            "allInOne.rememberChoices",
            Category::General,
            "All-in-One",
            "Remember All-in-One choices",
            "Restore the last capture mode, target, and recording input toggles",
            "all in one remember capture mode area camera microphone system audio",
            |config| config.all_in_one.remember_choices,
            |config, value| config.all_in_one.remember_choices = value,
            feature = Feature::AllInOne
        ),
        switch!(
            "general.previewAutoDismiss",
            Category::General,
            "Preview",
            "Dismiss previews automatically",
            "Hide capture previews after a delay, unless you are hovering or an action is running",
            "preview auto dismiss hide close timeout duration disappear",
            |config| config.preview.auto_dismiss,
            |config, value| config.preview.auto_dismiss = value
        ),
        select!(
            "general.previewAutoDismissSeconds",
            Category::General,
            "Preview",
            "Dismiss after",
            "How long a capture preview stays on screen",
            "preview auto dismiss delay seconds timeout duration disappear",
            OptionSource::Static(DISMISS_SECONDS),
            |config| format!("{}", config.preview.auto_dismiss_seconds as i64),
            |config, value| { config.preview.auto_dismiss_seconds = value.parse().unwrap_or(10.0) },
            visible = |config: &SettingsConfig| config.preview.auto_dismiss
        ),
        switch!(
            "general.historyEnabled",
            Category::General,
            "History",
            "Enable history",
            "Keep a history of your screenshots for quick access",
            "history recent past log",
            |config| config.history.enabled,
            |config, value| config.history.enabled = value
        ),
        Item {
            id: "general.historyMaxItems",
            category: Category::General,
            section: "History",
            label: "Maximum items",
            description: "Number of screenshots to keep in history",
            keywords: "history limit max count items",
            feature: None,
            visible_when: Some(|config| config.history.enabled),
            control: Control::Slider {
                min: 10.0,
                max: 200.0,
                step: 10.0,
                get: |config| config.history.max_items,
                set: |config, value| config.history.max_items = value.round(),
            },
        },
    ]
}

fn appearance_items() -> Vec<Item> {
    vec![
        select!(
            "appearance.mode",
            Category::Appearance,
            "Theme",
            "Appearance",
            "Choose a light, dark, or system-matched appearance",
            "appearance light dark system mode",
            OptionSource::Static(APPEARANCE_MODES),
            |config| config.appearance.mode.clone(),
            |config, value| config.appearance.mode = value.to_string()
        ),
        select!(
            "appearance.theme",
            Category::Appearance,
            "Theme",
            "Color theme",
            "Use the same paired color themes as Poracode",
            "theme color poracode palette",
            OptionSource::Themes,
            |config| config.appearance.theme.clone(),
            |config, value| config.appearance.theme = value.to_string()
        ),
    ]
}

fn screenshot_items() -> Vec<Item> {
    vec![
        switch!(
            "screenshot.autoCopyToClipboard", Category::Screenshot, "Capture Mode",
            "Copy to clipboard automatically",
            "Copy every screenshot to the clipboard as soon as it is captured",
            "clipboard copy automatic always paste",
            |config| config.screenshot.auto_copy_to_clipboard,
            |config, value| config.screenshot.auto_copy_to_clipboard = value
        ),
        switch!(
            "screenshot.captureToClipboard", Category::Screenshot, "Capture Mode",
            "Clipboard only",
            "Keep screenshots in the clipboard without opening the editor or preview",
            "clipboard copy capture direct only",
            |config| config.screenshot.capture_to_clipboard,
            |config, value| {
                config.screenshot.capture_to_clipboard = value;
                if value {
                    config.screenshot.show_preview = false;
                }
            }
        ),
        switch!(
            "screenshot.showPreview", Category::Screenshot, "Capture Mode",
            "Show preview",
            "Show a preview thumbnail after capturing instead of opening the editor",
            "preview thumbnail capture",
            |config| config.screenshot.show_preview,
            |config, value| {
                config.screenshot.show_preview = value;
                if value {
                    config.screenshot.capture_to_clipboard = false;
                }
            }
        ),
        switch!(
            "screenshot.hideDesktopIcons", Category::Screenshot, "Capture Mode",
            "Hide desktop icons",
            "Temporarily hide desktop icons when capturing screenshots",
            "desktop icons hide clean accessibility",
            |config| config.screenshot.hide_desktop_icons,
            |config, value| config.screenshot.hide_desktop_icons = value,
            feature = Feature::DesktopIcons
        ),
        switch!(
            "screenshot.freezeScreen", Category::Screenshot, "Capture Mode",
            "Freeze screen",
            "Capture areas and windows exactly as they appear in a static desktop snapshot",
            "freeze static snapshot still window overlap",
            |config| config.screenshot.freeze_screen,
            |config, value| config.screenshot.freeze_screen = value,
            feature = Feature::FreezeScreen
        ),
        switch!(
            "screenshot.closeOnCopy", Category::Screenshot, "Window Behavior",
            "Close on copy", "Automatically close the window after copying the screenshot",
            "close copy auto close window",
            |config| config.screenshot.close_on_copy,
            |config, value| config.screenshot.close_on_copy = value
        ),
        switch!(
            "screenshot.closeOnSave", Category::Screenshot, "Window Behavior",
            "Close on save", "Automatically close the window after saving the screenshot",
            "close save auto close window",
            |config| config.screenshot.close_on_save,
            |config, value| config.screenshot.close_on_save = value
        ),
        select!(
            "screenshot.format", Category::Screenshot, "Output",
            "File format", "Choose the format for saved screenshots",
            "format png jpeg jpg file type image",
            OptionSource::Static(SCREENSHOT_FORMATS),
            |config| config.screenshot.format.clone(),
            |config, value| config.screenshot.format = value.to_string()
        ),
        select!(
            "screenshot.multiImageAttachEdge", Category::Screenshot, "Open With",
            "Multi-image layout",
            "When opening multiple images at once, attach the extras to this edge of the first image",
            "open with multiple images attach side by side layout edge",
            OptionSource::Static(ATTACH_EDGES),
            |config| config.screenshot.multi_image_attach_edge.clone(),
            |config, value| config.screenshot.multi_image_attach_edge = value.to_string()
        ),
    ]
}

fn recording_items() -> Vec<Item> {
    vec![
        select!(
            "recording.frameRate",
            Category::Recording,
            "Quality",
            "Frame rate",
            "Record up to the selected display refresh rate",
            "fps frame rate refresh smooth recording",
            OptionSource::Static(RECORDING_FRAME_RATES),
            |config| config.recording.frame_rate.to_string(),
            |config, value| {
                if let Ok(value) = value.parse() {
                    config.recording.frame_rate = value;
                }
            }
        ),
        switch!(
            "recording.showPreview",
            Category::Recording,
            "Behavior",
            "Show recording preview",
            "Show a preview window after recording",
            "preview recording video record",
            |config| config.recording.show_preview,
            |config, value| config.recording.show_preview = value
        ),
        Item {
            id: "recording.startDelay",
            category: Category::Recording,
            section: "Behavior",
            label: "Start delay",
            description: "Countdown shown before recording starts (seconds)",
            keywords: "delay countdown timer start recording",
            feature: None,
            visible_when: None,
            control: Control::Slider {
                min: 0.0,
                max: 10.0,
                step: 1.0,
                get: |config| config.recording.start_delay,
                set: |config, value| config.recording.start_delay = value.round(),
            },
        },
        switch!(
            "recording.autoZoom",
            Category::Recording,
            "Behavior",
            "Auto zoom after recording",
            "Automatically generate zoom segments based on cursor clicks for new recordings",
            "zoom auto recording cursor clicks",
            |config| config.recording.auto_zoom,
            |config, value| config.recording.auto_zoom = value
        ),
    ]
}

fn devices_items() -> Vec<Item> {
    vec![
        Item {
            id: "devices.microphone",
            category: Category::Devices,
            section: "Microphone",
            label: "Microphone",
            description: "Choose which microphone is used for recordings",
            keywords: "microphone mic audio input device test voice",
            feature: Some(Feature::Recording),
            visible_when: None,
            control: Control::MicrophoneDevice,
        },
        Item {
            id: "devices.camera",
            category: Category::Devices,
            section: "Camera",
            label: "Camera",
            description: "Choose which camera is used for recordings",
            keywords: "camera webcam video device test preview mirror flip",
            feature: Some(Feature::Recording),
            visible_when: None,
            control: Control::CameraDevice,
        },
    ]
}

fn storage_items() -> Vec<Item> {
    vec![
        Item {
            id: "storage.namingPattern",
            category: Category::Storage,
            section: "File Naming",
            label: "Naming pattern",
            description: "Customize how files are named using tokens",
            keywords: "name pattern filename naming tokens",
            feature: None,
            visible_when: None,
            control: Control::NamingPattern,
        },
        Item {
            id: "storage.screenshotsPath",
            category: Category::Storage,
            section: "Save Locations",
            label: "Screenshots location",
            description: "Choose where to save screenshot files",
            keywords: "path folder directory save location screenshots",
            feature: None,
            visible_when: None,
            control: Control::PathPicker {
                kind: PathKind::Screenshots,
            },
        },
        Item {
            id: "storage.recordingsPath",
            category: Category::Storage,
            section: "Save Locations",
            label: "Recordings location",
            description: "Choose where to save recording files",
            keywords: "path folder directory save location recordings video",
            feature: None,
            visible_when: None,
            control: Control::PathPicker {
                kind: PathKind::Recordings,
            },
        },
    ]
}

fn cloud_upload_disabled(config: &SettingsConfig) -> bool {
    if config.cloud.active_provider == "rest" {
        let rest = &config.cloud.rest;
        if rest.url.is_empty() {
            return true;
        }
        if rest.response_is_plain_text {
            return false;
        }
        return rest.response_url_path.is_empty();
    }
    let s3 = &config.cloud.s3;
    s3.endpoint.is_empty()
        || s3.bucket.is_empty()
        || s3.access_key_id.is_empty()
        || s3.secret_access_key.is_empty()
}

fn cloud_items() -> Vec<Item> {
    vec![
        select!(
            "cloud.activeProvider",
            Category::Cloud,
            "Cloud Upload",
            "Upload provider",
            "Where uploaded screenshots and videos are sent",
            "cloud upload provider s3 rest api",
            OptionSource::Static(CLOUD_PROVIDERS),
            |config| config.cloud.active_provider.clone(),
            |config, value| config.cloud.active_provider = value.to_string()
        ),
        switch!(
            "cloud.enabled",
            Category::Cloud,
            "Cloud Upload",
            "Enable cloud upload",
            "Upload screenshots to your configured provider",
            "cloud upload s3 rest enable",
            |config| config.cloud.enabled,
            |config, value| config.cloud.enabled = value,
            disabled = cloud_upload_disabled
        ),
        Item {
            id: "cloud.testConnection",
            category: Category::Cloud,
            section: "Cloud Upload",
            label: "Test connection",
            description: "Test the connection to your configured provider",
            keywords: "cloud test connection verify",
            feature: None,
            visible_when: None,
            control: Control::CloudTestConnection,
        },
        input!(
            "cloud.s3.endpoint",
            Category::Cloud,
            "S3 Configuration",
            "Endpoint",
            "The S3 API endpoint",
            "cloud s3 endpoint url api",
            "s3.amazonaws.com or account.r2.cloudflarestorage.com",
            |config| config.cloud.s3.endpoint.clone(),
            |config, value| config.cloud.s3.endpoint = value.to_string(),
            hint = "For AWS, use s3.amazonaws.com or s3.region.amazonaws.com",
            visible = is_s3
        ),
        input!(
            "cloud.s3.region",
            Category::Cloud,
            "S3 Configuration",
            "Region",
            "AWS region for S3 storage",
            "cloud s3 region aws",
            "us-east-1 or auto",
            |config| config.cloud.s3.region.clone(),
            |config, value| config.cloud.s3.region = value.to_string(),
            hint = "Use \"auto\" for Cloudflare R2",
            visible = is_s3
        ),
        input!(
            "cloud.s3.bucket",
            Category::Cloud,
            "S3 Configuration",
            "Bucket name",
            "S3 bucket name for uploads",
            "cloud s3 bucket storage",
            "my-screenshots",
            |config| config.cloud.s3.bucket.clone(),
            |config, value| config.cloud.s3.bucket = value.to_string(),
            visible = is_s3
        ),
        input!(
            "cloud.s3.accessKeyId",
            Category::Cloud,
            "S3 Credentials",
            "Access Key ID",
            "S3 access key for authentication",
            "cloud s3 access key credentials auth",
            "AKIAIOSFODNN7EXAMPLE",
            |config| config.cloud.s3.access_key_id.clone(),
            |config, value| config.cloud.s3.access_key_id = value.to_string(),
            visible = is_s3
        ),
        input!(
            "cloud.s3.secretAccessKey",
            Category::Cloud,
            "S3 Credentials",
            "Secret Access Key",
            "S3 secret key for authentication",
            "cloud s3 secret credentials auth password",
            "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
            |config| config.cloud.s3.secret_access_key.clone(),
            |config, value| config.cloud.s3.secret_access_key = value.to_string(),
            secret = true,
            visible = is_s3
        ),
        input!(
            "cloud.s3.pathPrefix",
            Category::Cloud,
            "S3 Options",
            "Path prefix",
            "Optional folder prefix for uploaded files",
            "cloud s3 prefix path folder",
            "screenshots/",
            |config| config.cloud.s3.path_prefix.clone(),
            |config, value| config.cloud.s3.path_prefix = value.to_string(),
            visible = is_s3
        ),
        input!(
            "cloud.s3.customDomain",
            Category::Cloud,
            "S3 Options",
            "Custom domain",
            "Custom domain for public URLs",
            "cloud s3 domain cdn url custom",
            "https://cdn.example.com",
            |config| config.cloud.s3.custom_domain.clone(),
            |config, value| config.cloud.s3.custom_domain = value.to_string(),
            hint = "Leave empty to use the default S3 URL",
            visible = is_s3
        ),
        input!(
            "cloud.rest.url",
            Category::Cloud,
            "REST API Configuration",
            "Upload URL",
            "Endpoint that accepts the upload POST request",
            "cloud rest api url endpoint",
            "https://api.example.com/upload",
            |config| config.cloud.rest.url.clone(),
            |config, value| config.cloud.rest.url = value.to_string(),
            visible = is_rest
        ),
        input!(
            "cloud.rest.fileFieldName",
            Category::Cloud,
            "REST API Configuration",
            "File field name",
            "Multipart form field name for the uploaded file",
            "cloud rest api field name multipart",
            "file",
            |config| config.cloud.rest.file_field_name.clone(),
            |config, value| config.cloud.rest.file_field_name = value.to_string(),
            hint = "Defaults to \"file\" if left empty",
            visible = is_rest
        ),
        Item {
            id: "cloud.rest.headers",
            category: Category::Cloud,
            section: "REST API Configuration",
            label: "Request headers",
            description: "Custom headers sent with the upload request",
            keywords: "cloud rest api headers authorization auth",
            feature: None,
            visible_when: Some(is_rest),
            control: Control::RestHeaders,
        },
        switch!(
            "cloud.rest.responseIsPlainText",
            Category::Cloud,
            "REST API Response",
            "Response body is the URL",
            "Use the raw response body as the public URL",
            "cloud rest response plain text url",
            |config| config.cloud.rest.response_is_plain_text,
            |config, value| config.cloud.rest.response_is_plain_text = value,
            visible = is_rest
        ),
        input!(
            "cloud.rest.responseUrlPath",
            Category::Cloud,
            "REST API Response",
            "Response URL path",
            "JSON path to the URL in the response body",
            "cloud rest response json path url",
            "data.url",
            |config| config.cloud.rest.response_url_path.clone(),
            |config, value| config.cloud.rest.response_url_path = value.to_string(),
            hint = "Dot notation. Supports array indexes, e.g. \"files[0].url\"",
            visible = |config: &SettingsConfig| {
                is_rest(config) && !config.cloud.rest.response_is_plain_text
            }
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The switch writes the Run entry through `set_open_at_login` the moment
    /// it flips, in the same breath as `updateConfig` calls
    /// `setLoginItemSettings`. The module is split off before searching, so
    /// this test's own literals can never satisfy the assertion.
    #[test]
    fn the_start_on_login_switch_still_calls_set_open_at_login() {
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("src/windows/settings/registry.rs"),
        )
        .expect("read registry.rs");
        let production = source.split("#[cfg(test)]").next().expect("test module");
        assert!(
            production.contains("crate::system::startup::set_open_at_login(value)"),
            "the general.startOnLogin setter no longer registers the app at login"
        );
    }

    #[test]
    fn every_item_belongs_to_a_supported_category() {
        let categories: Vec<&str> = Category::supported()
            .into_iter()
            .map(Category::id)
            .collect();
        for item in items() {
            assert!(
                categories.contains(&item.category.id()),
                "{} is in an unsupported category",
                item.id
            );
        }
    }

    #[test]
    fn item_ids_are_unique() {
        let mut ids: Vec<&str> = items().iter().map(|item| item.id).collect();
        let total = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), total);
    }

    #[test]
    fn search_matches_labels_descriptions_and_keywords() {
        let all = items();
        let clipboard = all
            .iter()
            .find(|item| item.id == "screenshot.captureToClipboard");
        let clipboard = clipboard.expect("clipboard item");
        assert!(clipboard.matches("clipboard"));
        assert!(clipboard.matches("Clipboard Only"));
        assert!(clipboard.matches("capture direct"));
        assert!(!clipboard.matches("microphone"));
    }

    #[test]
    fn cloud_fields_follow_the_selected_provider() {
        let mut config = SettingsConfig::default();
        config.cloud.active_provider = "s3".into();
        let all = items();
        let endpoint = all
            .iter()
            .find(|item| item.id == "cloud.s3.endpoint")
            .unwrap();
        let rest_url = all.iter().find(|item| item.id == "cloud.rest.url").unwrap();
        assert!(endpoint.is_visible(&config));
        assert!(!rest_url.is_visible(&config));

        config.cloud.active_provider = "rest".into();
        assert!(!endpoint.is_visible(&config));
        assert!(rest_url.is_visible(&config));
    }

    #[test]
    fn cloud_upload_stays_disabled_until_a_provider_is_configured() {
        let mut config = SettingsConfig::default();
        config.cloud.active_provider = "rest".into();
        assert!(cloud_upload_disabled(&config));
        config.cloud.rest.url = "https://example.com/upload".into();
        assert!(cloud_upload_disabled(&config));
        config.cloud.rest.response_is_plain_text = true;
        assert!(!cloud_upload_disabled(&config));
    }
}
