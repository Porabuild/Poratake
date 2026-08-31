//! Port of `renderer/components/settings/registry/shortcuts.ts`.

use crate::config::schema::SettingsConfig;
use crate::system::capabilities::Feature;
use crate::windows::settings::registry::{Category, Control, Item};

struct Spec {
    id: &'static str,
    section: &'static str,
    label: &'static str,
    description: &'static str,
    keywords: &'static str,
    feature: Option<Feature>,
    get: fn(&SettingsConfig) -> String,
    set: fn(&mut SettingsConfig, &str),
}

const SCREENSHOT: &str = "Screenshot Shortcuts";
const RECORDING: &str = "Recording Shortcuts";
const OTHER: &str = "Other Shortcuts";
const EDITOR_TOOLS: &str = "Editor Tool Shortcuts";
const EDITOR_ACTIONS: &str = "Editor Action Shortcuts";
const VIDEO_EDITOR: &str = "Video Editor Shortcuts";

fn specs() -> Vec<Spec> {
    vec![
        Spec {
            id: "shortcuts.screenshot.area",
            section: SCREENSHOT,
            label: "Capture Area",
            description: "Keyboard shortcut for area capture",
            keywords: "shortcut hotkey area capture screenshot",
            feature: None,
            get: |config| config.shortcuts.screenshot.area.clone(),
            set: |config, value| config.shortcuts.screenshot.area = value.to_string(),
        },
        Spec {
            id: "shortcuts.screenshot.window",
            section: SCREENSHOT,
            label: "Capture Window",
            description: "Keyboard shortcut for window capture",
            keywords: "shortcut hotkey window capture screenshot",
            feature: Some(Feature::ScreenshotWindow),
            get: |config| config.shortcuts.screenshot.window.clone(),
            set: |config, value| config.shortcuts.screenshot.window = value.to_string(),
        },
        Spec {
            id: "shortcuts.screenshot.screen",
            section: SCREENSHOT,
            label: "Capture Full Screen",
            description: "Keyboard shortcut for full screen capture",
            keywords: "shortcut hotkey screen full capture",
            feature: None,
            get: |config| config.shortcuts.screenshot.screen.clone(),
            set: |config, value| config.shortcuts.screenshot.screen = value.to_string(),
        },
        Spec {
            id: "shortcuts.timerCapture",
            section: SCREENSHOT,
            label: "Timer Capture",
            description: "Keyboard shortcut for timer capture",
            keywords: "shortcut hotkey timer delay capture",
            feature: Some(Feature::TimerCapture),
            get: |config| config.shortcuts.timer_capture.clone(),
            set: |config, value| config.shortcuts.timer_capture = value.to_string(),
        },
        Spec {
            id: "shortcuts.recording.area",
            section: RECORDING,
            label: "Record Area",
            description: "Keyboard shortcut for area recording",
            keywords: "shortcut hotkey area record video",
            feature: Some(Feature::Recording),
            get: |config| config.shortcuts.recording.area.clone(),
            set: |config, value| config.shortcuts.recording.area = value.to_string(),
        },
        Spec {
            id: "shortcuts.recording.window",
            section: RECORDING,
            label: "Record Window",
            description: "Keyboard shortcut for window recording",
            keywords: "shortcut hotkey window record video",
            feature: Some(Feature::Recording),
            get: |config| config.shortcuts.recording.window.clone(),
            set: |config, value| config.shortcuts.recording.window = value.to_string(),
        },
        Spec {
            id: "shortcuts.recording.screen",
            section: RECORDING,
            label: "Record Screen",
            description: "Keyboard shortcut for screen recording",
            keywords: "shortcut hotkey screen record video",
            feature: Some(Feature::Recording),
            get: |config| config.shortcuts.recording.screen.clone(),
            set: |config, value| config.shortcuts.recording.screen = value.to_string(),
        },
        Spec {
            id: "shortcuts.captureText",
            section: OTHER,
            label: "Capture Text (OCR)",
            description: "Keyboard shortcut for text capture",
            keywords: "shortcut hotkey text ocr recognize",
            feature: Some(Feature::Ocr),
            get: |config| config.shortcuts.capture_text.clone(),
            set: |config, value| config.shortcuts.capture_text = value.to_string(),
        },
        Spec {
            id: "shortcuts.scanQRCode",
            section: OTHER,
            label: "Scan QR Code",
            description: "Keyboard shortcut for QR code scanning",
            keywords: "shortcut hotkey qr code scan barcode",
            feature: Some(Feature::QrCode),
            get: |config| config.shortcuts.scan_qrcode.clone(),
            set: |config, value| config.shortcuts.scan_qrcode = value.to_string(),
        },
        Spec {
            id: "shortcuts.scrollCapture",
            section: OTHER,
            label: "Scroll Capture",
            description: "Keyboard shortcut for scroll capture",
            keywords: "shortcut hotkey scroll capture long page",
            feature: Some(Feature::ScrollCapture),
            get: |config| config.shortcuts.scroll_capture.clone(),
            set: |config, value| config.shortcuts.scroll_capture = value.to_string(),
        },
        Spec {
            id: "shortcuts.history",
            section: OTHER,
            label: "Open History",
            description: "Keyboard shortcut for opening history",
            keywords: "shortcut hotkey history recent",
            feature: None,
            get: |config| config.shortcuts.history.clone(),
            set: |config, value| config.shortcuts.history = value.to_string(),
        },
        Spec {
            id: "shortcuts.allInOne",
            section: OTHER,
            label: "All-in-one",
            description: "Keyboard shortcut for all-in-one mode",
            keywords: "shortcut hotkey all in one capture",
            feature: Some(Feature::AllInOne),
            get: |config| config.shortcuts.all_in_one.clone(),
            set: |config, value| config.shortcuts.all_in_one = value.to_string(),
        },
        Spec {
            id: "shortcuts.openInEditor",
            section: OTHER,
            label: "Open in Editor",
            description: "Keyboard shortcut for opening in editor",
            keywords: "shortcut hotkey open editor file",
            feature: None,
            get: |config| config.shortcuts.open_in_editor.clone(),
            set: |config, value| config.shortcuts.open_in_editor = value.to_string(),
        },
        Spec {
            id: "shortcuts.clipboardInEditor",
            section: OTHER,
            label: "Open Clipboard in Editor",
            description: "Keyboard shortcut for opening clipboard in editor",
            keywords: "shortcut hotkey clipboard editor paste",
            feature: None,
            get: |config| config.shortcuts.clipboard_in_editor.clone(),
            set: |config, value| config.shortcuts.clipboard_in_editor = value.to_string(),
        },
        Spec {
            id: "shortcuts.editorActions.uploadToCloud",
            section: EDITOR_ACTIONS,
            label: "Upload to Cloud",
            description: "Keyboard shortcut for uploading the screenshot to cloud",
            keywords: "shortcut hotkey cloud upload share",
            feature: None,
            get: |config| config.shortcuts.editor_actions.upload_to_cloud.clone(),
            set: |config, value| {
                config.shortcuts.editor_actions.upload_to_cloud = value.to_string()
            },
        },
    ]
}

macro_rules! tool_spec {
    ($id:literal, $label:literal, $description:literal, $keywords:literal, $field:ident) => {
        Spec {
            id: $id,
            section: EDITOR_TOOLS,
            label: $label,
            description: $description,
            keywords: $keywords,
            feature: None,
            get: |config| config.shortcuts.editor.$field.clone(),
            set: |config, value| config.shortcuts.editor.$field = value.to_string(),
        }
    };
}

macro_rules! panel_spec {
    ($id:literal, $label:literal, $description:literal, $keywords:literal, $field:ident) => {
        Spec {
            id: $id,
            section: VIDEO_EDITOR,
            label: $label,
            description: $description,
            keywords: $keywords,
            feature: Some(Feature::VideoEditor),
            get: |config| config.shortcuts.video_editor_sidebar.$field.clone(),
            set: |config, value| config.shortcuts.video_editor_sidebar.$field = value.to_string(),
        }
    };
}

fn tool_specs() -> Vec<Spec> {
    vec![
        tool_spec!(
            "shortcuts.editor.pen",
            "Pen Tool",
            "Keyboard shortcut for pen tool",
            "shortcut editor pen draw",
            pen
        ),
        tool_spec!(
            "shortcuts.editor.highlight",
            "Highlight Tool",
            "Keyboard shortcut for highlight tool",
            "shortcut editor highlight marker",
            highlight
        ),
        tool_spec!(
            "shortcuts.editor.rectangle",
            "Rectangle Tool",
            "Keyboard shortcut for rectangle tool",
            "shortcut editor rectangle box square",
            rectangle
        ),
        tool_spec!(
            "shortcuts.editor.circle",
            "Circle Tool",
            "Keyboard shortcut for circle tool",
            "shortcut editor circle ellipse oval",
            circle
        ),
        tool_spec!(
            "shortcuts.editor.line",
            "Line Tool",
            "Keyboard shortcut for line tool",
            "shortcut editor line",
            line
        ),
        tool_spec!(
            "shortcuts.editor.arrow",
            "Arrow Tool",
            "Keyboard shortcut for arrow tool",
            "shortcut editor arrow pointer",
            arrow
        ),
        tool_spec!(
            "shortcuts.editor.text",
            "Text Tool",
            "Keyboard shortcut for text tool",
            "shortcut editor text type label",
            text
        ),
        tool_spec!(
            "shortcuts.editor.number",
            "Number Tool",
            "Keyboard shortcut for number tool",
            "shortcut editor number badge step",
            number
        ),
        tool_spec!(
            "shortcuts.editor.redact",
            "Redact Tool",
            "Keyboard shortcut for redact tool",
            "shortcut editor redact blur pixelate",
            redact
        ),
        tool_spec!(
            "shortcuts.editor.select",
            "Select Tool",
            "Keyboard shortcut for select tool",
            "shortcut editor select move",
            select
        ),
        tool_spec!(
            "shortcuts.editor.crop",
            "Crop Tool",
            "Keyboard shortcut for crop tool",
            "shortcut editor crop trim",
            crop
        ),
        tool_spec!(
            "shortcuts.editor.wallpaper",
            "Wallpaper Tool",
            "Keyboard shortcut for wallpaper tool",
            "shortcut editor wallpaper background",
            wallpaper
        ),
    ]
}

fn panel_specs() -> Vec<Spec> {
    vec![
        panel_spec!(
            "shortcuts.videoEditorSidebar.cursor",
            "Cursor Panel",
            "Keyboard shortcut for cursor panel",
            "shortcut video editor cursor panel",
            cursor
        ),
        panel_spec!(
            "shortcuts.videoEditorSidebar.zoom",
            "Zoom Panel",
            "Keyboard shortcut for zoom panel",
            "shortcut video editor zoom panel",
            zoom
        ),
        panel_spec!(
            "shortcuts.videoEditorSidebar.drawing",
            "Drawing Panel",
            "Keyboard shortcut for drawing panel",
            "shortcut video editor drawing panel",
            drawing
        ),
        panel_spec!(
            "shortcuts.videoEditorSidebar.camera",
            "Camera Panel",
            "Keyboard shortcut for camera panel",
            "shortcut video editor camera panel",
            camera
        ),
        panel_spec!(
            "shortcuts.videoEditorSidebar.audio",
            "Audio Panel",
            "Keyboard shortcut for audio panel",
            "shortcut video editor audio panel",
            audio
        ),
        panel_spec!(
            "shortcuts.videoEditorSidebar.wallpaper",
            "Wallpaper Panel",
            "Keyboard shortcut for wallpaper panel",
            "shortcut video editor wallpaper panel",
            wallpaper
        ),
        panel_spec!(
            "shortcuts.videoEditorSidebar.keyboard",
            "Keyboard Panel",
            "Keyboard shortcut for keyboard panel",
            "shortcut video editor keyboard panel",
            keyboard
        ),
        panel_spec!(
            "shortcuts.videoEditorSidebar.subtitle",
            "Subtitle Panel",
            "Keyboard shortcut for subtitle panel",
            "shortcut video editor subtitle caption panel",
            subtitle
        ),
        panel_spec!(
            "shortcuts.videoEditorSidebar.first-frame",
            "First Frame Panel",
            "Keyboard shortcut for first frame panel",
            "shortcut video editor first frame thumbnail",
            first_frame
        ),
        panel_spec!(
            "shortcuts.videoEditorSidebar.export",
            "Export Panel",
            "Keyboard shortcut for export panel",
            "shortcut video editor export render panel",
            export
        ),
    ]
}

fn global_shortcuts_visible(_: &SettingsConfig) -> bool {
    crate::system::capabilities::global_shortcuts_supported()
}

pub fn items() -> Vec<Item> {
    specs()
        .into_iter()
        .chain(tool_specs())
        .chain(panel_specs())
        .map(|spec| {
            let global = matches!(spec.section, SCREENSHOT | RECORDING | OTHER);
            Item {
                id: spec.id,
                category: Category::Shortcuts,
                section: spec.section,
                label: spec.label,
                description: spec.description,
                keywords: spec.keywords,
                feature: spec.feature,
                visible_when: global.then_some(global_shortcuts_visible),
                control: Control::Shortcut {
                    get: spec.get,
                    set: spec.set,
                    single_key: matches!(spec.section, EDITOR_TOOLS | VIDEO_EDITOR),
                },
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shortcut_round_trips_through_the_config() {
        let mut config = SettingsConfig::default();
        for item in items() {
            let Control::Shortcut { get, set, .. } = item.control else {
                panic!("{} is not a shortcut", item.id);
            };
            set(&mut config, "Alt+Shift+9");
            assert_eq!(
                get(&config),
                "Alt+Shift+9",
                "{} did not round trip",
                item.id
            );
        }
    }

    /// `registry/shortcuts.ts` marks exactly the editor tool and video editor
    /// sidebar bindings `singleKey: true` — 22 of the 36 shortcut items.
    #[test]
    fn only_the_bare_key_bindings_are_single_key() {
        let single: Vec<&str> = items()
            .iter()
            .filter(|item| {
                matches!(
                    item.control,
                    Control::Shortcut {
                        single_key: true,
                        ..
                    }
                )
            })
            .map(|item| item.id)
            .collect();
        assert_eq!(single.len(), 22, "single-key shortcut count");
        assert!(single.contains(&"shortcuts.editor.pen"));
        assert!(single.contains(&"shortcuts.videoEditorSidebar.export"));
        assert!(!single.contains(&"shortcuts.screenshot.area"));
        assert!(!single.contains(&"shortcuts.editorActions.uploadToCloud"));
    }

    #[test]
    fn exposes_every_shortcut_the_renderer_registry_does() {
        let ids: Vec<&str> = items().iter().map(|item| item.id).collect();
        for id in [
            "shortcuts.screenshot.area",
            "shortcuts.screenshot.window",
            "shortcuts.screenshot.screen",
            "shortcuts.timerCapture",
            "shortcuts.scanQRCode",
            "shortcuts.captureText",
            "shortcuts.allInOne",
            "shortcuts.history",
            "shortcuts.openInEditor",
            "shortcuts.clipboardInEditor",
            "shortcuts.recording.area",
            "shortcuts.recording.window",
            "shortcuts.recording.screen",
            "shortcuts.editor.pen",
            "shortcuts.editor.highlight",
            "shortcuts.editor.rectangle",
            "shortcuts.editor.circle",
            "shortcuts.editor.line",
            "shortcuts.editor.arrow",
            "shortcuts.editor.text",
            "shortcuts.editor.number",
            "shortcuts.editor.redact",
            "shortcuts.editor.select",
            "shortcuts.editor.crop",
            "shortcuts.editor.wallpaper",
            "shortcuts.editorActions.uploadToCloud",
            "shortcuts.videoEditorSidebar.cursor",
            "shortcuts.videoEditorSidebar.zoom",
            "shortcuts.videoEditorSidebar.drawing",
            "shortcuts.videoEditorSidebar.camera",
            "shortcuts.videoEditorSidebar.audio",
            "shortcuts.videoEditorSidebar.wallpaper",
            "shortcuts.videoEditorSidebar.keyboard",
            "shortcuts.videoEditorSidebar.subtitle",
            "shortcuts.videoEditorSidebar.first-frame",
            "shortcuts.videoEditorSidebar.export",
        ] {
            assert!(ids.contains(&id), "missing {id}");
        }
    }

    #[test]
    fn covers_every_section_the_renderer_shows() {
        let sections: Vec<&str> = items().iter().map(|item| item.section).collect();
        for section in [
            SCREENSHOT,
            RECORDING,
            OTHER,
            EDITOR_TOOLS,
            EDITOR_ACTIONS,
            VIDEO_EDITOR,
        ] {
            assert!(sections.contains(&section), "missing {section}");
        }
    }
}
