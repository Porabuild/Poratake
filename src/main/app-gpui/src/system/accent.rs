//! The operating system's accent colour.
//!
//! `useAccentColor` overrides `--primary` with `systemPreferences.getAccentColor()`
//! on every window, so in Electron the area-overlay selection frame, the
//! about-tab progress bar and the device level meters follow the colour the user
//! picked in Windows -- not the app theme's accent. Chromium reads that value
//! from the DWM key, so this reads the same one.

/// `'#' + color.substring(0, 6)` in `preferences.ts`, i.e. RGB with the alpha
/// dropped, and `'#007AFF'` when the lookup fails.
const FALLBACK: &str = "#007AFF";

/// The accent as `#rrggbb`.
pub fn system_accent() -> String {
    read_dwm_accent()
        .map(|(r, g, b)| format!("#{r:02x}{g:02x}{b:02x}"))
        .unwrap_or_else(|| FALLBACK.to_string())
}

/// `HKCU\Software\Microsoft\Windows\DWM\AccentColor` is a `DWORD` in `0xAABBGGRR`
/// order, which is why the bytes come out reversed from the hex string.
fn read_dwm_accent() -> Option<(u8, u8, u8)> {
    use windows::core::w;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ, REG_DWORD,
    };

    let mut key = HKEY::default();
    // SAFETY: every out-parameter is initialised here and the key is closed on
    // both paths below.
    unsafe {
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            w!(r"Software\Microsoft\Windows\DWM"),
            None,
            KEY_READ,
            &mut key,
        )
        .is_err()
        {
            return None;
        }

        let mut value: u32 = 0;
        let mut size = std::mem::size_of::<u32>() as u32;
        let mut kind = REG_DWORD;
        let result = RegQueryValueExW(
            key,
            w!("AccentColor"),
            None,
            Some(&mut kind),
            Some(&mut value as *mut u32 as *mut u8),
            Some(&mut size),
        );
        let _ = RegCloseKey(key);
        if result.is_err() || kind != REG_DWORD {
            return None;
        }

        let [r, g, b, _a] = value.to_le_bytes();
        Some((r, g, b))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Whatever the machine is set to, the result has to be parseable as a
    /// colour -- that is the contract `ThemeVars` relies on.
    #[test]
    fn the_accent_is_always_a_usable_hex_colour() {
        let accent = system_accent();
        assert_eq!(accent.len(), 7, "got {accent}");
        assert!(accent.starts_with('#'), "got {accent}");
        assert!(
            accent[1..].chars().all(|c| c.is_ascii_hexdigit()),
            "got {accent}"
        );
        let parsed = crate::theme::color::Srgba::parse(&accent);
        assert!(parsed.a > 0.0, "{accent} parsed to something transparent");
    }

    #[test]
    fn the_fallback_matches_the_electron_default() {
        assert_eq!(FALLBACK, "#007AFF");
    }
}
