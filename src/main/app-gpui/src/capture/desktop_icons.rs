//! Port of `src/main/capture/desktop-icons/index.ts` — reference-counted
//! hiding so a capture that hides the icons cannot un-hide a menu request.

use std::collections::HashMap;
use std::sync::Arc;

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

pub struct CaptureHide {
    daemon: DaemonHandle,
}

impl Drop for CaptureHide {
    fn drop(&mut self) {
        show(&self.daemon, HideSource::Capture);
    }
}

pub fn hide_for_capture(daemon: &DaemonHandle, config: &ConfigStore) -> Option<Arc<CaptureHide>> {
    if !should_hide_for_capture(config) {
        return None;
    }
    hide(daemon, HideSource::Capture);
    Some(Arc::new(CaptureHide {
        daemon: daemon.clone(),
    }))
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
                let Some(remaining) = state.reasons.get(&source).copied() else {
                    return false;
                };
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

    #[cfg(windows)]
    fn active_overlay(cx: &gpui::TestAppContext) -> gpui::AnyWindowHandle {
        cx.read(|cx| {
            cx.windows()
                .into_iter()
                .find(|window| {
                    window
                        .downcast::<crate::capture::overlay::AreaOverlay>()
                        .is_some()
                })
                .unwrap()
        })
    }

    #[cfg(windows)]
    fn cancel_overlay(cx: &mut gpui::TestAppContext) {
        cx.simulate_keystrokes(active_overlay(cx), "escape");
        cx.run_until_parked();
        assert!(!cx.update(crate::capture::overlay::parity_is_active));
    }

    #[cfg(windows)]
    fn capture_count() -> Option<u32> {
        with_state(|state| state.reasons.get(&HideSource::Capture).copied())
    }

    #[cfg(windows)]
    #[test]
    fn starting_an_unrelated_capture_keeps_the_active_capture_hide() {
        if std::env::var_os("PORATAKE_TEST_CAPTURE_HIDE_CHILD").is_none() {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "capture::desktop_icons::tests::starting_an_unrelated_capture_keeps_the_active_capture_hide",
                    "--nocapture",
                ])
                .env("PORATAKE_TEST_CAPTURE_HIDE_CHILD", "1")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert_eq!(
                stderr.matches("[desktop-icons] show failed:").count(),
                2,
                "{stderr}"
            );
            return;
        }
        let mut cx = gpui::TestAppContext::single();
        let dir = tempfile::tempdir().unwrap();
        let config =
            std::sync::Arc::new(ConfigStore::load_at(dir.path().join("config.json")).unwrap());
        config.update(|settings| settings.screenshot.freeze_screen = false);
        let running = Arc::new(CaptureHide {
            daemon: DaemonHandle::with_binary(std::path::PathBuf::new()),
        });
        with_state(|state| {
            state.reasons.insert(HideSource::Capture, 1);
            state.hidden = true;
        });
        cx.update(|cx| {
            crate::state::set_test_state(cx, config.clone());
            crate::capture::overlay::init_bindings(cx);
            crate::capture::start_area_selection(
                crate::capture::intent::CaptureIntent::Screenshot,
                cx,
            );
        });
        cx.run_until_parked();
        assert!(cx.update(crate::capture::overlay::parity_is_active));
        cancel_overlay(&mut cx);
        assert_eq!(capture_count(), Some(1));
        for (intent, replace) in [
            (crate::capture::intent::CaptureIntent::ScrollCapture, true),
            (crate::capture::intent::CaptureIntent::Timer, false),
        ] {
            cx.update(|cx| crate::capture::start_area_selection(intent, cx));
            cx.run_until_parked();
            assert_eq!(capture_count(), Some(2));
            if replace {
                cx.update(|cx| {
                    crate::capture::start_area_selection(
                        crate::capture::intent::CaptureIntent::Screenshot,
                        cx,
                    );
                });
                cx.run_until_parked();
                assert_eq!(capture_count(), Some(1));
            }
            cancel_overlay(&mut cx);
            assert_eq!(capture_count(), Some(1));
        }
        let notifications = crate::system::notification::capture_for_test();
        for intent in [
            crate::capture::intent::CaptureIntent::ScrollCapture,
            crate::capture::intent::CaptureIntent::Timer,
        ] {
            cx.update(|cx| crate::capture::start_area_selection(intent, cx));
            cx.run_until_parked();
            assert_eq!(capture_count(), Some(2));
            let mut visual = gpui::VisualTestContext::from_window(active_overlay(&cx), &cx);
            visual.simulate_mouse_down(
                gpui::point(gpui::px(100.), gpui::px(100.)),
                gpui::MouseButton::Left,
                gpui::Modifiers::none(),
            );
            visual.simulate_mouse_move(
                gpui::point(gpui::px(300.), gpui::px(250.)),
                gpui::MouseButton::Left,
                gpui::Modifiers::none(),
            );
            visual.simulate_mouse_up(
                gpui::point(gpui::px(300.), gpui::px(250.)),
                gpui::MouseButton::Left,
                gpui::Modifiers::none(),
            );
            cx.run_until_parked();
            assert!(!cx.update(crate::capture::overlay::parity_is_active));
            assert_eq!(capture_count(), Some(1));
        }
        assert_eq!(
            *notifications.borrow(),
            ["Scroll Capture Failed", "Timer Capture Failed"]
        );
        config.update(|settings| settings.screenshot.hide_desktop_icons = false);
        cx.update(|cx| {
            crate::capture::start_area_selection(
                crate::capture::intent::CaptureIntent::ScrollCapture,
                cx,
            );
        });
        cx.run_until_parked();
        cancel_overlay(&mut cx);
        let remaining = capture_count();
        drop(running);
        let final_count = capture_count();
        let daemon = DaemonHandle::with_binary(std::path::PathBuf::new());
        let stale = Arc::new(CaptureHide {
            daemon: daemon.clone(),
        });
        with_state(|state| {
            state.reasons.insert(HideSource::Capture, 1);
            state.hidden = true;
        });
        assert!(!show(&daemon, HideSource::System));
        drop(stale);
        assert_eq!(capture_count(), None);
        *STATE.lock() = None;
        assert_eq!(remaining, Some(1));
        assert_eq!(final_count, None);
    }

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
