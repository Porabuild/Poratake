use std::collections::HashMap;

use global_hotkey::hotkey::HotKey;
use global_hotkey::GlobalHotKeyManager;

use crate::config::schema::SettingsConfig;
use crate::system::accelerator;
use crate::system::tray::Intent;

pub fn bindings(config: &SettingsConfig) -> Vec<(Intent, String)> {
    let shortcuts = &config.shortcuts;
    vec![
        (Intent::CaptureArea, shortcuts.screenshot.area.clone()),
        (Intent::CaptureWindow, shortcuts.screenshot.window.clone()),
        (Intent::CaptureScreen, shortcuts.screenshot.screen.clone()),
        (Intent::AllInOne, shortcuts.all_in_one.clone()),
        (Intent::CaptureText, shortcuts.capture_text.clone()),
        (Intent::ScanQrCode, shortcuts.scan_qrcode.clone()),
        (Intent::TimerCapture, shortcuts.timer_capture.clone()),
        (Intent::ScrollCapture, shortcuts.scroll_capture.clone()),
        (Intent::History, shortcuts.history.clone()),
        (Intent::OpenInEditor, shortcuts.open_in_editor.clone()),
        (
            Intent::OpenClipboardInEditor,
            shortcuts.clipboard_in_editor.clone(),
        ),
        (Intent::RecordArea, shortcuts.recording.area.clone()),
        (Intent::RecordWindow, shortcuts.recording.window.clone()),
        (Intent::RecordScreen, shortcuts.recording.screen.clone()),
    ]
    .into_iter()
    .filter(|(_, value)| !value.trim().is_empty())
    .collect()
}

pub struct HotkeyRegistry {
    manager: Option<GlobalHotKeyManager>,
    registered: Vec<HotKey>,
    intents: HashMap<u32, Intent>,
}

impl HotkeyRegistry {
    pub fn new() -> Self {
        let manager = match GlobalHotKeyManager::new() {
            Ok(manager) => Some(manager),
            Err(error) => {
                eprintln!("[hotkey] manager unavailable: {error}");
                None
            }
        };
        Self {
            manager,
            registered: Vec::new(),
            intents: HashMap::new(),
        }
    }

    pub fn intent_for(&self, id: u32) -> Option<Intent> {
        self.intents.get(&id).copied()
    }

    pub fn apply(&mut self, bindings: &[(Intent, String)]) {
        let Some(manager) = &self.manager else {
            return;
        };

        if !self.registered.is_empty() {
            if let Err(error) = manager.unregister_all(&self.registered) {
                eprintln!("[hotkey] unregister failed: {error}");
            }
            self.registered.clear();
        }
        self.intents.clear();

        for (intent, value) in bindings {
            let Some(parsed) = accelerator::parse(value) else {
                eprintln!("[hotkey] unsupported accelerator {value:?} for {intent:?}");
                continue;
            };
            let hotkey = parsed.hotkey();
            if self.intents.contains_key(&hotkey.id()) {
                continue;
            }
            match manager.register(hotkey) {
                Ok(()) => {
                    self.intents.insert(hotkey.id(), *intent);
                    self.registered.push(hotkey);
                }
                Err(error) => eprintln!("[hotkey] register {value:?} failed: {error}"),
            }
        }
    }
}

impl Default for HotkeyRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bindings_skip_unset_shortcuts() {
        let config = SettingsConfig::default();
        let bindings = bindings(&config);
        assert!(bindings.iter().all(|(_, value)| !value.is_empty()));
        assert!(bindings
            .iter()
            .any(|(intent, _)| *intent == Intent::CaptureArea));
        assert!(bindings
            .iter()
            .all(|(intent, _)| *intent != Intent::CaptureText));
    }
}
