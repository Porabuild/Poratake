pub fn window_background(
    blurred: gpui::WindowBackgroundAppearance,
) -> gpui::WindowBackgroundAppearance {
    if enabled() {
        gpui::WindowBackgroundAppearance::Opaque
    } else {
        blurred
    }
}

pub fn enabled() -> bool {
    platform_reduced_transparency()
}

#[cfg(all(target_os = "macos", not(test)))]
fn platform_reduced_transparency() -> bool {
    use objc::runtime::{BOOL, NO};
    use objc::{class, msg_send, sel, sel_impl};

    unsafe {
        let workspace: cocoa::base::id = msg_send![class!(NSWorkspace), sharedWorkspace];
        if workspace == cocoa::base::nil {
            return false;
        }
        let reduce: BOOL = msg_send![workspace, accessibilityDisplayShouldReduceTransparency];
        reduce != NO
    }
}

#[cfg(all(windows, not(test)))]
fn platform_reduced_transparency() -> bool {
    use windows::core::w;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ,
    };

    let mut key = HKEY::default();
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
            return false;
        }

        let mut value: u32 = 1;
        let mut size = std::mem::size_of::<u32>() as u32;
        let result = RegQueryValueExW(
            key,
            w!("EnableTransparency"),
            None,
            None,
            Some(std::ptr::addr_of_mut!(value).cast()),
            Some(&mut size),
        );
        let _ = RegCloseKey(key);
        result.is_ok() && value == 0
    }
}

#[cfg(any(test, all(not(windows), not(target_os = "macos"))))]
fn platform_reduced_transparency() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_blurred_surface_stays_blurred_while_transparency_is_allowed() {
        assert!(!enabled());
        assert_eq!(
            window_background(gpui::WindowBackgroundAppearance::Blurred),
            gpui::WindowBackgroundAppearance::Blurred
        );
        assert_eq!(
            window_background(gpui::WindowBackgroundAppearance::Opaque),
            gpui::WindowBackgroundAppearance::Opaque
        );
    }
}
