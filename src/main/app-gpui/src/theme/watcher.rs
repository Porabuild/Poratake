//! Follows the OS light/dark switch while the app runs.
//!
#[cfg(any(windows, target_os = "linux"))]
use std::thread;

#[cfg(any(windows, target_os = "linux"))]
use smol::channel::Receiver;

use crate::theme::presets::ThemeMode;
use crate::theme::vars;

/// Whether a registry notification warrants re-applying the theme: the user
/// follows the system and the freshly-read mode differs from the one already
/// applied.
pub fn needs_refresh(selected: ThemeMode, applied: ThemeMode, current: ThemeMode) -> bool {
    if selected != ThemeMode::System {
        return false;
    }
    applied != current
}

/// Re-applies the theme from the freshly-read OS mode, but only when the user
/// follows the system and the mode actually changed. An explicit light/dark
/// choice must survive OS changes underneath it.
#[cfg(any(windows, target_os = "linux"))]
pub enum SystemAppearanceEvent {
    Mode(ThemeMode),
    Contrast,
}

#[cfg(any(windows, target_os = "linux"))]
pub fn apply_system_event(event: SystemAppearanceEvent, cx: &mut gpui::App) {
    match event {
        SystemAppearanceEvent::Mode(mode) => apply_system_mode(mode, cx),
        SystemAppearanceEvent::Contrast => cx.refresh_windows(),
    }
}

pub fn apply_system_mode(mode: ThemeMode, cx: &mut gpui::App) {
    let store = crate::state::state(cx).config;
    let config = store.get();
    let selected = ThemeMode::parse(&config.appearance.mode);
    if !needs_refresh(selected, vars::active_mode(cx), mode) {
        return;
    }
    vars::update_theme(cx, mode, &config.appearance.theme);
    if crate::state::try_native(cx).is_some() {
        crate::intents::refresh_shell(cx);
    }
}

#[cfg(any(target_os = "macos", test))]
pub fn window_theme_mode(appearance: gpui::WindowAppearance) -> ThemeMode {
    match appearance {
        gpui::WindowAppearance::Light | gpui::WindowAppearance::VibrantLight => ThemeMode::Light,
        gpui::WindowAppearance::Dark | gpui::WindowAppearance::VibrantDark => ThemeMode::Dark,
    }
}

/// Starts the watcher thread and returns the channel it reports the OS theme
/// mode on. Each notification re-reads the mode and sends it, changed or not;
/// the decision to repaint is `needs_refresh` on the main thread.
#[cfg(any(windows, target_os = "linux"))]
pub fn spawn() -> Receiver<SystemAppearanceEvent> {
    let (tx, rx) = smol::channel::unbounded();
    let mode_tx = tx.clone();
    thread::Builder::new()
        .name("theme-watcher".into())
        .spawn(move || watch(mode_tx))
        .ok();
    #[cfg(windows)]
    thread::Builder::new()
        .name("contrast-watcher".into())
        .spawn(move || watch_high_contrast(tx))
        .ok();
    #[cfg(not(windows))]
    drop(tx);
    rx
}

#[cfg(target_os = "linux")]
fn watch(tx: smol::channel::Sender<SystemAppearanceEvent>) {
    use std::time::Duration;

    use dbus::blocking::Connection;
    use dbus::channel::MatchingReceiver as _;
    use dbus::message::MatchRule;

    let Ok(connection) = Connection::new_session() else {
        return;
    };
    let mut rule = MatchRule::new_signal("org.freedesktop.portal.Settings", "SettingChanged");
    rule.path = Some("/org/freedesktop/portal/desktop".into());
    let sender = tx.clone();
    let _token = connection.start_receive(
        rule,
        Box::new(move |message, _| {
            let mut items = message.iter_init();
            let namespace = items.read::<String>().ok();
            let key = items.read::<String>().ok();
            if namespace.as_deref() == Some("org.freedesktop.appearance") {
                let event = match key.as_deref() {
                    Some("contrast") => Some(SystemAppearanceEvent::Contrast),
                    Some("color-scheme") => Some(SystemAppearanceEvent::Mode(
                        crate::theme::presets::system_theme_mode(),
                    )),
                    _ => None,
                };
                if let Some(event) = event {
                    let _ = sender.send_blocking(event);
                }
            }
            true
        }),
    );
    let _ = connection.add_match_no_cb(
        "type='signal',interface='org.freedesktop.portal.Settings',member='SettingChanged',path='/org/freedesktop/portal/desktop'",
    );
    while !tx.is_closed() {
        if connection.process(Duration::from_secs(2)).is_err() {
            return;
        }
    }
}

#[cfg(windows)]
fn watch(tx: smol::channel::Sender<SystemAppearanceEvent>) {
    use windows::core::w;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegNotifyChangeKeyValue, RegOpenKeyExW, HKEY, HKEY_CURRENT_USER, KEY_READ,
        REG_NOTIFY_CHANGE_LAST_SET,
    };

    loop {
        let mut key = HKEY::default();
        // SAFETY: the key handle is initialised before use, closed on every
        // path below, and the synchronous wait needs no event handle.
        unsafe {
            if RegOpenKeyExW(
                HKEY_CURRENT_USER,
                w!(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize"),
                None,
                KEY_READ,
                &mut key,
            )
            .is_err()
            {
                return;
            }

            let result =
                RegNotifyChangeKeyValue(key, false, REG_NOTIFY_CHANGE_LAST_SET, None, false);
            let _ = RegCloseKey(key);
            if result.is_err() {
                eprintln!("[theme] registry watch failed: {}", result.0);
                return;
            }
        }

        if tx
            .send_blocking(SystemAppearanceEvent::Mode(
                crate::theme::presets::system_theme_mode(),
            ))
            .is_err()
        {
            return;
        }
    }
}

#[cfg(windows)]
fn watch_high_contrast(tx: smol::channel::Sender<SystemAppearanceEvent>) {
    use windows::core::w;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegNotifyChangeKeyValue, RegOpenKeyExW, HKEY, HKEY_CURRENT_USER, KEY_READ,
        REG_NOTIFY_CHANGE_LAST_SET,
    };

    loop {
        let mut key = HKEY::default();
        unsafe {
            if RegOpenKeyExW(
                HKEY_CURRENT_USER,
                w!(r"Control Panel\Accessibility\HighContrast"),
                None,
                KEY_READ,
                &mut key,
            )
            .is_err()
            {
                return;
            }
            let result =
                RegNotifyChangeKeyValue(key, false, REG_NOTIFY_CHANGE_LAST_SET, None, false);
            let _ = RegCloseKey(key);
            if result.is_err() {
                return;
            }
        }
        if tx.send_blocking(SystemAppearanceEvent::Contrast).is_err() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn follows_the_os_when_the_user_matches_system() {
        assert!(needs_refresh(
            ThemeMode::System,
            ThemeMode::Light,
            ThemeMode::Dark
        ));
        assert!(needs_refresh(
            ThemeMode::System,
            ThemeMode::Dark,
            ThemeMode::Light
        ));
    }

    /// Accent-colour and wallpaper writes fire the key too; the value read
    /// back is unchanged, so nothing repaints.
    #[test]
    fn ignores_events_that_do_not_change_the_mode() {
        assert!(!needs_refresh(
            ThemeMode::System,
            ThemeMode::Light,
            ThemeMode::Light
        ));
        assert!(!needs_refresh(
            ThemeMode::System,
            ThemeMode::Dark,
            ThemeMode::Dark
        ));
    }

    #[test]
    fn an_explicit_light_or_dark_choice_never_follows_the_os() {
        assert!(!needs_refresh(
            ThemeMode::Light,
            ThemeMode::Light,
            ThemeMode::Dark
        ));
        assert!(!needs_refresh(
            ThemeMode::Dark,
            ThemeMode::Dark,
            ThemeMode::Light
        ));
        assert!(!needs_refresh(
            ThemeMode::Light,
            ThemeMode::Dark,
            ThemeMode::Dark
        ));
        assert!(!needs_refresh(
            ThemeMode::Dark,
            ThemeMode::Light,
            ThemeMode::Light
        ));
    }

    #[test]
    fn maps_native_window_appearances_to_theme_modes() {
        assert_eq!(
            window_theme_mode(gpui::WindowAppearance::VibrantLight),
            ThemeMode::Light
        );
        assert_eq!(
            window_theme_mode(gpui::WindowAppearance::VibrantDark),
            ThemeMode::Dark
        );
    }
}
