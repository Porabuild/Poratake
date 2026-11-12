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
    uncertain: bool,
    shutting_down: bool,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);
static TRANSITION: Mutex<()> = Mutex::new(());
static PENDING: Mutex<Vec<smol::channel::Receiver<()>>> = Mutex::new(Vec::new());

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
        let transition = track_transition();
        show(&self.daemon, HideSource::Capture);
        drop(transition);
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

pub(crate) fn track_transition() -> smol::channel::Sender<()> {
    let (sender, receiver) = smol::channel::bounded(1);
    let mut pending = PENDING.lock();
    pending.retain(|receiver| !receiver.is_closed());
    pending.push(receiver);
    sender
}

pub(crate) fn pending_transitions() -> Vec<smol::channel::Receiver<()>> {
    let mut pending = PENDING.lock();
    pending.retain(|receiver| !receiver.is_closed());
    pending.clone()
}

pub(crate) fn release_for_capture(hide: Option<Arc<CaptureHide>>, cx: &mut gpui::App) {
    if let Some(hide) = hide {
        let transition = track_transition();
        cx.background_executor()
            .spawn(async move {
                drop(hide);
                drop(transition);
            })
            .detach();
    }
}

pub fn are_hidden() -> bool {
    with_state(|state| state.hidden)
}

pub(crate) fn is_shutting_down() -> bool {
    with_state(|state| state.shutting_down)
}

pub(crate) fn needs_restoration() -> bool {
    with_state(|state| state.reasons.is_empty() && (state.hidden || state.uncertain))
}

pub(crate) fn restore_if_unowned(daemon: &DaemonHandle) -> bool {
    if !supported() {
        return true;
    }
    let _transition = TRANSITION.lock();
    if is_shutting_down() {
        return false;
    }
    if !needs_restoration() {
        return true;
    }
    show_locked(daemon, HideSource::Menu)
}

pub(crate) fn begin_shutdown() -> bool {
    with_state(|state| {
        if state.shutting_down {
            return false;
        }
        state.shutting_down = true;
        true
    })
}

pub(crate) fn toggle_menu(daemon: &DaemonHandle) -> bool {
    if !supported() {
        return false;
    }
    let _transition = TRANSITION.lock();
    let (hidden, shutting_down) = with_state(|state| (state.hidden, state.shutting_down));
    if shutting_down {
        return false;
    }
    if hidden {
        show_locked(daemon, HideSource::Menu)
    } else {
        hide_locked(daemon, HideSource::Menu)
    }
}

pub fn hide(daemon: &DaemonHandle, source: HideSource) -> bool {
    if !supported() {
        return false;
    }
    let _transition = TRANSITION.lock();
    hide_locked(daemon, source)
}

fn hide_locked(daemon: &DaemonHandle, source: HideSource) -> bool {
    let Some(already_hidden) = with_state(|state| {
        if state.shutting_down {
            return None;
        }
        let entry = state.reasons.entry(source).or_insert(0);
        *entry = if source == HideSource::Capture {
            *entry + 1
        } else {
            1
        };
        Some(state.hidden && !state.uncertain)
    }) else {
        return false;
    };
    if already_hidden {
        return true;
    }

    match daemon.desktop_helper().hide() {
        Ok(_) => {
            with_state(|state| {
                state.hidden = true;
                state.uncertain = false;
            });
            true
        }
        Err(error) => {
            with_state(|state| state.uncertain = true);
            eprintln!("[desktop-icons] hide failed: {error}");
            false
        }
    }
}

pub fn show(daemon: &DaemonHandle, source: HideSource) -> bool {
    if !supported() {
        return false;
    }
    let _transition = TRANSITION.lock();
    show_locked(daemon, source)
}

fn show_locked(daemon: &DaemonHandle, source: HideSource) -> bool {
    let should_show = with_state(|state| {
        match source {
            HideSource::System => {
                state.shutting_down = true;
                state.reasons.clear();
            }
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
        state.reasons.is_empty() && (state.hidden || state.uncertain)
    });
    if !should_show {
        return true;
    }

    match daemon.desktop_helper().show() {
        Ok(_) => {
            with_state(|state| {
                state.hidden = false;
                state.uncertain = false;
            });
            true
        }
        Err(error) => {
            with_state(|state| state.uncertain = true);
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
        assert_eq!(capture_count(), Some(2));
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
                assert_eq!(capture_count(), Some(2));
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
        assert!(!hide(&daemon, HideSource::Capture));
        assert_eq!(capture_count(), None);
        *STATE.lock() = None;
        assert_eq!(remaining, Some(1));
        assert_eq!(final_count, None);
    }

    #[cfg(windows)]
    #[test]
    fn a_pending_icon_transition_does_not_block_or_restore_a_replaced_selector() {
        const CHILD: &str = "PORATAKE_TEST_PENDING_ICON_TRANSITION_CHILD";
        if std::env::var_os(CHILD).is_none() {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "capture::desktop_icons::tests::a_pending_icon_transition_does_not_block_or_restore_a_replaced_selector",
                    "--nocapture",
                ])
                .env(CHILD, "1")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }
        let mut cx = gpui::TestAppContext::single();
        let dir = tempfile::tempdir().unwrap();
        let config = Arc::new(ConfigStore::load_at(dir.path().join("config.json")).unwrap());
        config.update(|settings| {
            settings.screenshot.freeze_screen = true;
            settings.screenshot.hide_desktop_icons = true;
        });
        let service = cx.update(|cx| {
            crate::state::set_test_state(cx, config);
            crate::capture::overlay::init_bindings(cx);
            crate::state::state(cx)
        });
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let gate = std::thread::spawn(move || {
            let _guard = TRANSITION.lock();
            started_tx.send(()).unwrap();
            release_rx
                .recv_timeout(std::time::Duration::from_secs(2))
                .is_ok()
        });
        started_rx.recv().unwrap();
        let generation = service.freeze.state.lock().next_generation;
        cx.update(|cx| {
            crate::capture::start_area_selection(
                crate::capture::intent::CaptureIntent::ScrollCapture,
                cx,
            );
            crate::capture::start_area_selection(
                crate::capture::intent::CaptureIntent::Screenshot,
                cx,
            );
            assert!(!crate::capture::overlay::parity_is_active(cx));
        });
        assert_eq!(service.freeze.state.lock().next_generation, generation);
        let _ = release_tx.send(());
        let responsive = gate.join().unwrap();
        cx.run_until_parked();
        assert!(
            responsive,
            "the UI waited for an in-flight desktop-icon transition"
        );
        assert_eq!(
            cx.update(crate::capture::overlay::active_intents_for_test),
            [crate::capture::intent::CaptureIntent::Screenshot]
        );
        cancel_overlay(&mut cx);
        cx.run_until_parked();
        assert_eq!(capture_count(), None);

        with_state(|state| {
            state.reasons.insert(HideSource::Menu, 1);
            state.hidden = true;
        });
        cx.update(|cx| {
            crate::capture::start_area_selection(
                crate::capture::intent::CaptureIntent::ScrollCapture,
                cx,
            );
        });
        cx.run_until_parked();
        assert_eq!(capture_count(), Some(1));
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let gate = std::thread::spawn(move || {
            let _guard = TRANSITION.lock();
            started_tx.send(()).unwrap();
            release_rx
                .recv_timeout(std::time::Duration::from_secs(2))
                .is_ok()
        });
        started_rx.recv().unwrap();
        let generation = service.freeze.state.lock().next_generation;
        cx.update(|cx| {
            crate::capture::start_area_selection(
                crate::capture::intent::CaptureIntent::Recording,
                cx,
            );
            assert!(!crate::capture::overlay::parity_is_active(cx));
        });
        assert_eq!(service.freeze.state.lock().next_generation, generation);
        let _ = release_tx.send(());
        let responsive = gate.join().unwrap();
        cx.run_until_parked();
        assert!(
            responsive,
            "replacing an open selector waited for an icon transition"
        );
        assert_eq!(capture_count(), None);
        assert_eq!(
            cx.update(crate::capture::overlay::active_intents_for_test),
            [crate::capture::intent::CaptureIntent::Recording]
        );
        cancel_overlay(&mut cx);
        *STATE.lock() = None;

        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let gate = std::thread::spawn(move || {
            let _guard = TRANSITION.lock();
            started_tx.send(()).unwrap();
            release_rx
                .recv_timeout(std::time::Duration::from_secs(2))
                .is_ok()
        });
        started_rx.recv().unwrap();
        cx.update(|cx| {
            crate::intents::dispatch(crate::system::tray::Intent::ToggleDesktopIcons, None, cx);
        });
        let _ = release_tx.send(());
        let responsive = gate.join().unwrap();
        cx.run_until_parked();
        assert!(responsive, "the tray action waited for an icon transition");
        *STATE.lock() = None;

        with_state(|state| {
            state.reasons.insert(HideSource::Menu, 1);
            state.hidden = true;
        });
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let gate = std::thread::spawn(move || {
            let _guard = TRANSITION.lock();
            started_tx.send(()).unwrap();
            release_rx
                .recv_timeout(std::time::Duration::from_secs(2))
                .is_ok()
        });
        started_rx.recv().unwrap();
        let generation = service.freeze.state.lock().next_generation;
        cx.update(|cx| {
            crate::capture::start_area_selection(
                crate::capture::intent::CaptureIntent::ScrollCapture,
                cx,
            );
            crate::intents::dispatch(crate::system::tray::Intent::ToggleDesktopIcons, None, cx);
            crate::intents::dispatch(crate::system::tray::Intent::Quit, None, cx);
        });
        let _ = release_tx.send(());
        let responsive = gate.join().unwrap();
        cx.run_until_parked();
        assert!(
            responsive,
            "quitting waited for an icon transition on the UI thread"
        );
        assert!(!cx.update(crate::capture::overlay::parity_is_active));
        assert_eq!(service.freeze.state.lock().next_generation, generation);
        assert!(!hide(&service.daemon, HideSource::Capture));
        *STATE.lock() = None;
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
