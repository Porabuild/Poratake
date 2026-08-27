use crate::config::shortcuts::ShortcutsConfig;
use crate::system::accelerator;
use crate::system::capabilities::{is_supported, Feature};
use crate::system::tray::intent::Intent;
use crate::ui::menu::{MenuEntry, MenuItem};

#[derive(Clone, Debug, Default, PartialEq)]
#[allow(dead_code)]
pub enum UpdateStatus {
    #[default]
    Idle,
    Available(String),
    Downloading(u8),
    Ready(String),
}

#[derive(Clone, Debug)]
pub struct TrayMenuState {
    pub shortcuts: ShortcutsConfig,
    pub desktop_icons_hidden: bool,
    pub update: UpdateStatus,
    pub is_recording: bool,
    pub dark_mode: bool,
}

impl TrayMenuState {
    pub fn from_config(config: &crate::config::schema::SettingsConfig) -> Self {
        let appearance_mode = crate::theme::presets::ThemeMode::parse(&config.appearance.mode);
        Self {
            shortcuts: config.shortcuts.clone(),
            desktop_icons_hidden: crate::capture::desktop_icons::are_hidden(),
            update: UpdateStatus::Idle,
            is_recording: crate::video::recorder::is_recording(),
            dark_mode: matches!(
                crate::theme::presets::resolve_theme_mode(appearance_mode),
                crate::theme::presets::ThemeMode::Dark
            ),
        }
    }
}

enum Spec {
    Item {
        intent: Intent,
        label: String,
        icon: Option<&'static str>,
        accelerator: Option<String>,
        enabled: bool,
    },
    Separator,
}

fn item(intent: Intent, label: &str, icon: &'static str) -> Spec {
    Spec::Item {
        intent,
        label: label.to_string(),
        icon: Some(icon),
        accelerator: None,
        enabled: true,
    }
}

impl Spec {
    fn accelerator(mut self, value: &str) -> Self {
        if let Spec::Item {
            accelerator: slot, ..
        } = &mut self
        {
            if !value.is_empty() {
                *slot = Some(value.to_string());
            }
        }
        self
    }

    fn enabled(mut self, value: bool) -> Self {
        if let Spec::Item { enabled, .. } = &mut self {
            *enabled = value;
        }
        self
    }
}

fn push_gated(specs: &mut Vec<Spec>, feature: Feature, spec: Spec) {
    if is_supported(feature) {
        specs.push(spec);
    }
}

fn prune(specs: Vec<Spec>) -> Vec<Spec> {
    let mut result: Vec<Spec> = Vec::with_capacity(specs.len());
    for spec in specs {
        if matches!(spec, Spec::Separator) && matches!(result.last(), None | Some(Spec::Separator))
        {
            continue;
        }
        result.push(spec);
    }
    while matches!(result.last(), Some(Spec::Separator)) {
        result.pop();
    }
    result
}

fn specs(state: &TrayMenuState) -> Vec<Spec> {
    let shortcuts = &state.shortcuts;
    let screenshot = &shortcuts.screenshot;
    let recording = &shortcuts.recording;
    let mut specs: Vec<Spec> = Vec::new();

    match &state.update {
        UpdateStatus::Ready(version) => {
            specs.push(item(
                Intent::OpenAbout,
                &format!("Update Ready (v{version})"),
                "rotate-ccw",
            ));
            specs.push(Spec::Separator);
        }
        UpdateStatus::Downloading(progress) => {
            specs.push(
                item(
                    Intent::OpenAbout,
                    &format!("Downloading Update ({progress}%)..."),
                    "rotate-ccw",
                )
                .enabled(false),
            );
            specs.push(Spec::Separator);
        }
        UpdateStatus::Available(version) => {
            specs.push(
                item(
                    Intent::OpenAbout,
                    &format!("Update Available (v{version})"),
                    "rotate-ccw",
                )
                .enabled(false),
            );
            specs.push(Spec::Separator);
        }
        UpdateStatus::Idle => {}
    }

    push_gated(
        &mut specs,
        Feature::AllInOne,
        item(Intent::AllInOne, "All-in-one", "box").accelerator(&shortcuts.all_in_one),
    );
    specs.push(Spec::Separator);

    specs.push(
        item(Intent::CaptureScreen, "Capture Screen", "monitor").accelerator(&screenshot.screen),
    );
    specs.push(item(Intent::CaptureArea, "Capture Area", "scan").accelerator(&screenshot.area));
    push_gated(
        &mut specs,
        Feature::ScreenshotWindow,
        item(Intent::CaptureWindow, "Capture Window", "app-window").accelerator(&screenshot.window),
    );
    push_gated(
        &mut specs,
        Feature::ScrollCapture,
        item(Intent::ScrollCapture, "Scroll Capture", "scroll")
            .accelerator(&shortcuts.scroll_capture),
    );
    push_gated(
        &mut specs,
        Feature::Ocr,
        item(Intent::CaptureText, "Capture Text (OCR)", "text-cursor")
            .accelerator(&shortcuts.capture_text),
    );
    push_gated(
        &mut specs,
        Feature::QrCode,
        item(Intent::ScanQrCode, "Scan QR Code", "qr-code").accelerator(&shortcuts.scan_qrcode),
    );
    push_gated(
        &mut specs,
        Feature::TimerCapture,
        item(Intent::TimerCapture, "Timer Capture", "timer-reset")
            .accelerator(&shortcuts.timer_capture),
    );
    specs.push(
        item(Intent::OpenInEditor, "Open in Editor", "pencil")
            .accelerator(&shortcuts.open_in_editor),
    );
    specs.push(
        item(
            Intent::OpenClipboardInEditor,
            "Open Clipboard in Editor",
            "pencil",
        )
        .accelerator(&shortcuts.clipboard_in_editor),
    );
    specs.push(item(Intent::Pin, "Pin", "pin"));
    specs.push(Spec::Separator);

    push_gated(
        &mut specs,
        Feature::Recording,
        item(Intent::RecordScreen, "Record Screen", "monitor")
            .accelerator(&recording.screen)
            .enabled(!state.is_recording),
    );
    push_gated(
        &mut specs,
        Feature::Recording,
        item(Intent::RecordArea, "Record Area", "scan")
            .accelerator(&recording.area)
            .enabled(!state.is_recording),
    );
    push_gated(
        &mut specs,
        Feature::Recording,
        item(Intent::RecordWindow, "Record Window", "app-window")
            .accelerator(&recording.window)
            .enabled(!state.is_recording),
    );
    push_gated(
        &mut specs,
        Feature::VideoEditor,
        item(Intent::OpenInVideoEditor, "Open in Video Editor", "film"),
    );
    specs.push(Spec::Separator);

    specs.push(item(Intent::History, "History", "history").accelerator(&shortcuts.history));
    push_gated(
        &mut specs,
        Feature::DesktopIcons,
        item(
            Intent::ToggleDesktopIcons,
            if state.desktop_icons_hidden {
                "Show Desktop Icons"
            } else {
                "Hide Desktop Icons"
            },
            "monitor-dot",
        ),
    );
    specs.push(Spec::Separator);

    specs.push(item(Intent::OpenSettings, "Settings...", "settings"));
    specs.push(item(
        Intent::HideTrayIcon,
        if cfg!(target_os = "macos") {
            "Hide Menu Bar Icon"
        } else {
            "Hide Tray Icon"
        },
        "eye-off",
    ));
    specs.push(item(Intent::OpenIssues, "Poratake Issues", "aperture"));
    specs.push(item(Intent::Quit, "Quit", "power"));

    prune(specs)
}

pub fn entries(
    state: &TrayMenuState,
    tray_rect: Option<crate::system::native::TrayRect>,
) -> Vec<MenuEntry> {
    specs(state)
        .into_iter()
        .map(|spec| match spec {
            Spec::Separator => MenuEntry::Separator,
            Spec::Item {
                intent,
                label,
                icon,
                accelerator,
                enabled,
            } => {
                let mut item = MenuItem::new(label).disabled(!enabled);
                if let Some(icon) = icon {
                    item = item.icon(icon);
                }
                if let Some(accelerator) = accelerator {
                    item = item.shortcut(accelerator::display(&accelerator));
                }
                item = item.on_select(move |window, cx| {
                    window.remove_window();
                    crate::intents::dispatch(intent, tray_rect, cx);
                });
                MenuEntry::Item(item)
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> TrayMenuState {
        TrayMenuState {
            shortcuts: ShortcutsConfig::default(),
            desktop_icons_hidden: false,
            update: UpdateStatus::Idle,
            is_recording: crate::video::recorder::is_recording(),
            dark_mode: true,
        }
    }

    fn labels(specs: &[Spec]) -> Vec<String> {
        specs
            .iter()
            .map(|spec| match spec {
                Spec::Separator => "---".to_string(),
                Spec::Item { label, .. } => label.clone(),
            })
            .collect()
    }

    #[test]
    fn never_starts_or_ends_with_a_separator() {
        let built = specs(&state());
        assert!(matches!(built.first(), Some(Spec::Item { .. })));
        assert!(matches!(built.last(), Some(Spec::Item { .. })));
    }

    #[test]
    fn never_emits_adjacent_separators() {
        let built = specs(&state());
        for pair in built.windows(2) {
            assert!(
                !(matches!(pair[0], Spec::Separator) && matches!(pair[1], Spec::Separator)),
                "adjacent separators in {:?}",
                labels(&built)
            );
        }
    }

    #[test]
    fn desktop_icons_label_follows_state() {
        let mut current = state();
        assert!(labels(&specs(&current)).contains(&"Hide Desktop Icons".to_string()));
        current.desktop_icons_hidden = true;
        assert!(labels(&specs(&current)).contains(&"Show Desktop Icons".to_string()));
    }

    #[test]
    fn recording_items_disable_while_recording() {
        let mut current = state();
        current.is_recording = true;
        let built = specs(&current);
        let record_screen = built.iter().find(
            |spec| matches!(spec, Spec::Item { intent, .. } if *intent == Intent::RecordScreen),
        );
        match record_screen {
            Some(Spec::Item { enabled, .. }) => assert!(!enabled),
            _ => panic!("record screen item missing"),
        }
    }

    #[test]
    fn update_status_prepends_a_row() {
        let mut current = state();
        current.update = UpdateStatus::Ready("1.2.3".into());
        let built = labels(&specs(&current));
        assert_eq!(
            built.first().map(String::as_str),
            Some("Update Ready (v1.2.3)")
        );
    }
}
