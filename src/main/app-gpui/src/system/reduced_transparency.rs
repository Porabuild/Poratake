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
fn high_contrast_enabled() -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{
        SystemParametersInfoW, SPI_GETHIGHCONTRAST, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
    };

    #[repr(C)]
    struct HighContrastW {
        cb_size: u32,
        dw_flags: u32,
        lpsz_default_scheme: *mut u16,
    }

    const HCF_HIGHCONTRASTON: u32 = 0x0000_0001;
    let mut hc = HighContrastW {
        cb_size: std::mem::size_of::<HighContrastW>() as u32,
        dw_flags: 0,
        lpsz_default_scheme: std::ptr::null_mut(),
    };
    unsafe {
        SystemParametersInfoW(
            SPI_GETHIGHCONTRAST,
            hc.cb_size,
            Some((&mut hc as *mut HighContrastW).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
        .is_ok()
            && (hc.dw_flags & HCF_HIGHCONTRASTON) != 0
    }
}

#[cfg(all(windows, not(test)))]
fn platform_reduced_transparency() -> bool {
    use windows::core::w;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ,
    };

    if high_contrast_enabled() {
        return true;
    }

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

#[cfg(all(target_os = "linux", not(test)))]
fn platform_reduced_transparency() -> bool {
    use std::time::Duration;

    use dbus::arg::{RefArg, Variant};
    use dbus::blocking::Connection;

    let Ok(connection) = Connection::new_session() else {
        return false;
    };
    let proxy = connection.with_proxy(
        "org.freedesktop.portal.Desktop",
        "/org/freedesktop/portal/desktop",
        Duration::from_millis(500),
    );
    let result: Result<(Variant<Box<dyn RefArg>>,), _> = proxy.method_call(
        "org.freedesktop.portal.Settings",
        "Read",
        ("org.freedesktop.appearance", "contrast"),
    );
    result.is_ok_and(|(outer,)| {
        outer
            .0
            .as_iter()
            .and_then(|mut iter| iter.next())
            .and_then(|inner| inner.as_u64())
            .or_else(|| outer.0.as_u64())
            == Some(1)
    })
}

#[cfg(any(test, not(any(windows, target_os = "macos", target_os = "linux"))))]
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
