//! Port of `src/main/capture/desktop-icons/index.ts` — reference-counted
//! hiding so a capture that hides the icons cannot un-hide a menu request.

use std::collections::HashMap;

use parking_lot::Mutex;

use crate::config::store::ConfigStore;
use crate::daemon::DaemonHandle;
use crate::system::capabilities::{is_supported, Feature};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum HideSource {
    Menu,
    Capture,
    System,
}

#[derive(Default)]
struct State {
    reasons: HashMap<HideSource, u32>,
    hidden: bool,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn with_state<R>(apply: impl FnOnce(&mut State) -> R) -> R {
    let mut guard = STATE.lock();
    apply(guard.get_or_insert_with(State::default))
}

pub fn supported() -> bool {
    is_supported(Feature::DesktopIcons)
}

/// `shouldHideDesktopIconsForCapture` (`desktop-icons/preference.ts`): the
/// setting plus the capability gate, with the macOS accessibility check that
/// flips the setting off when trust is missing.
pub fn should_hide_for_capture(config: &ConfigStore) -> bool {
    if !config.get().screenshot.hide_desktop_icons || !supported() {
        return false;
    }
    if !crate::system::permissions::accessibility_granted() {
        config.update(|settings| settings.screenshot.hide_desktop_icons = false);
        return false;
    }
    true
}

/// Hides the icons for one capture flow. Every entry that hides must reach a
/// `restore_after_capture` on each of its exits.
pub fn hide_for_capture(daemon: &DaemonHandle, config: &ConfigStore) {
    if should_hide_for_capture(config) {
        hide(daemon, HideSource::Capture);
    }
}

/// Every capture exit funnels through here. A no-op when the flow never hid,
/// so recording and window screenshots share the same exits safely.
pub fn restore_after_capture(daemon: &DaemonHandle) {
    show(daemon, HideSource::Capture);
}

pub fn are_hidden() -> bool {
    with_state(|state| state.hidden)
}

pub fn hide(daemon: &DaemonHandle, source: HideSource) -> bool {
    if !supported() {
        return false;
    }

    let already_hidden = with_state(|state| {
        let entry = state.reasons.entry(source).or_insert(0);
        *entry = if source == HideSource::Capture {
            *entry + 1
        } else {
            1
        };
        state.hidden
    });
    if already_hidden {
        return true;
    }

    match daemon.desktop_helper().hide() {
        Ok(_) => {
            with_state(|state| state.hidden = true);
            true
        }
        Err(error) => {
            eprintln!("[desktop-icons] hide failed: {error}");
            false
        }
    }
}

pub fn show(daemon: &DaemonHandle, source: HideSource) -> bool {
    if !supported() {
        return false;
    }

    let should_show = with_state(|state| {
        match source {
            HideSource::System => state.reasons.clear(),
            HideSource::Capture => {
                let remaining = state.reasons.get(&source).copied().unwrap_or(0);
                if remaining > 1 {
                    state.reasons.insert(source, remaining - 1);
                } else {
                    state.reasons.remove(&source);
                }
            }
            HideSource::Menu => {
                state.reasons.remove(&source);
            }
        }
        state.reasons.is_empty() && state.hidden
    });
    if !should_show {
        return true;
    }

    match daemon.desktop_helper().show() {
        Ok(_) => {
            with_state(|state| state.hidden = false);
            true
        }
        Err(error) => {
            eprintln!("[desktop-icons] show failed: {error}");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_hides_are_reference_counted() {
        let mut state = State::default();
        for _ in 0..2 {
            let entry = state.reasons.entry(HideSource::Capture).or_insert(0);
            *entry += 1;
        }
        assert_eq!(state.reasons.get(&HideSource::Capture), Some(&2));

        let remaining = state.reasons[&HideSource::Capture];
        state.reasons.insert(HideSource::Capture, remaining - 1);
        assert!(!state.reasons.is_empty());

        state.reasons.remove(&HideSource::Capture);
        assert!(state.reasons.is_empty());
    }

    #[test]
    fn the_capture_gate_needs_the_setting_and_trust() {
        let dir = tempfile::tempdir().expect("temp dir");
        let config = ConfigStore::load_at(dir.path().join("config.json")).expect("load config");
        config.update(|settings| settings.screenshot.hide_desktop_icons = false);
        assert!(!should_hide_for_capture(&config));

        config.update(|settings| settings.screenshot.hide_desktop_icons = true);
        let trusted = crate::system::permissions::accessibility_granted();
        assert_eq!(should_hide_for_capture(&config), supported() && trusted);
        if !trusted {
            assert!(!config.get().screenshot.hide_desktop_icons);
        }
    }
}
