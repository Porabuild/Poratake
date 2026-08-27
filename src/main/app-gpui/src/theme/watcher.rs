//! Follows the Windows OS light/dark switch while the app runs.
//!
//! Electron subscribes to `nativeTheme.on('updated', ...)` and both shells
//! offer the same `system` appearance mode, so this shell has to live-follow
//! too. The OS theme is the `AppsUseLightTheme` DWORD under
//! `HKCU\...\Themes\Personalize` (read by `presets::system_theme_mode`); a
//! background thread blocks on `RegNotifyChangeKeyValue` for that key and
//! reports the re-read mode over a channel. The mode only reaches the windows
//! when the user actually follows the system and the value changed — the key
//! also fires for accent-colour and wallpaper writes, which must not repaint
//! anything.

#[cfg(windows)]
use std::thread;

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

/// Starts the watcher thread and returns the channel it reports the OS theme
/// mode on. Each notification re-reads the mode and sends it, changed or not;
/// the decision to repaint is `needs_refresh` on the main thread.
#[cfg(windows)]
pub fn spawn() -> Receiver<ThemeMode> {
    let (tx, rx) = smol::channel::unbounded();
    thread::Builder::new()
        .name("theme-watcher".into())
        .spawn(move || watch(tx))
        .ok();
    rx
}

#[cfg(windows)]
fn watch(tx: smol::channel::Sender<ThemeMode>) {
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
            .send_blocking(crate::theme::presets::system_theme_mode())
            .is_err()
        {
            return;
        }
    }
}

/// No registry to watch off Windows; the receiver simply never fires.
#[cfg(not(windows))]
pub fn spawn() -> Receiver<ThemeMode> {
    smol::channel::unbounded().1
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
}
