#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Device {
    Microphone,
    Camera,
}

pub fn ensure_access(device: Device) -> bool {
    #[cfg(windows)]
    if consent_is_denied(read_consent(device).as_deref()) {
        show_permission_dialog(device);
        return false;
    }

    #[cfg(not(windows))]
    let _ = device;

    true
}

pub fn screen_recording_granted() -> bool {
    #[cfg(target_os = "macos")]
    unsafe {
        return CGPreflightScreenCaptureAccess();
    }
    #[cfg(not(target_os = "macos"))]
    true
}

pub fn accessibility_granted() -> bool {
    #[cfg(target_os = "macos")]
    unsafe {
        return AXIsProcessTrusted() != 0;
    }
    #[cfg(not(target_os = "macos"))]
    true
}

pub fn open_screen_recording_preferences() {
    #[cfg(target_os = "macos")]
    unsafe {
        CGRequestScreenCaptureAccess();
    }
    crate::system::desktop::open_url(
        "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture",
    );
}

pub fn open_accessibility_preferences() {
    crate::system::desktop::open_url(
        "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
    );
}

pub fn open_keyboard_shortcut_preferences() {
    crate::system::desktop::open_url(
        "x-apple.systempreferences:com.apple.Keyboard-Settings.extension?Shortcuts",
    );
}

#[cfg(target_os = "macos")]
#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGPreflightScreenCaptureAccess() -> bool;
    fn CGRequestScreenCaptureAccess() -> bool;
}

#[cfg(target_os = "macos")]
#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> u8;
}

#[cfg(any(windows, test))]
fn consent_is_denied(consent: Option<&str>) -> bool {
    // Windows' consent registry values are empty or absent on working machines,
    // so this fail-open behaviour is deliberate; only an explicit `Deny` blocks use.
    matches!(consent, Some("Deny"))
}

#[cfg(any(windows, test))]
fn settings_url(device: Device) -> &'static str {
    match device {
        Device::Microphone => "ms-settings:privacy-microphone",
        Device::Camera => "ms-settings:privacy-webcam",
    }
}

#[cfg(windows)]
fn read_consent(device: Device) -> Option<String> {
    use windows::core::w;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ, REG_SZ,
    };

    let path = match device {
        Device::Microphone => {
            r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\microphone"
        }
        Device::Camera => {
            r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\webcam"
        }
    };
    let mut key = HKEY::default();
    // SAFETY: every out-parameter is initialised here and the key is closed on
    // every path after a successful open.
    unsafe {
        let open_result = match device {
            Device::Microphone => RegOpenKeyExW(
                HKEY_CURRENT_USER,
                w!(
                    r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\microphone"
                ),
                None,
                KEY_READ,
                &mut key,
            ),
            Device::Camera => RegOpenKeyExW(
                HKEY_CURRENT_USER,
                w!(
                    r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\webcam"
                ),
                None,
                KEY_READ,
                &mut key,
            ),
        };
        if open_result.is_err() {
            eprintln!("[permissions] failed to open {path}: {open_result:?}");
            return None;
        }

        let consent = {
            let mut size = 0;
            let size_result = RegQueryValueExW(key, w!("Value"), None, None, None, Some(&mut size));
            if size_result.is_err() {
                eprintln!("[permissions] failed to read {path}\\Value: {size_result:?}");
                None
            } else if size == 0 {
                Some(String::new())
            } else {
                let mut kind = REG_SZ;
                let mut buffer = vec![0u16; (size as usize).div_ceil(2)];
                let result = RegQueryValueExW(
                    key,
                    w!("Value"),
                    None,
                    Some(&mut kind),
                    Some(buffer.as_mut_ptr() as *mut u8),
                    Some(&mut size),
                );
                if result.is_err() {
                    eprintln!("[permissions] failed to read {path}\\Value: {result:?}");
                    None
                } else if kind != REG_SZ {
                    eprintln!("[permissions] unexpected type for {path}\\Value");
                    None
                } else if size % 2 != 0 {
                    eprintln!("[permissions] invalid UTF-16 size for {path}\\Value");
                    None
                } else {
                    let units = (size as usize) / 2;
                    if units > buffer.len() {
                        eprintln!(
                            "[permissions] registry value changed while reading {path}\\Value"
                        );
                        None
                    } else {
                        match String::from_utf16(&buffer[..units]) {
                            Ok(value) => Some(value.trim_end_matches('\0').to_string()),
                            Err(error) => {
                                eprintln!("[permissions] invalid UTF-16 in {path}\\Value: {error}");
                                None
                            }
                        }
                    }
                }
            }
        };
        let _ = RegCloseKey(key);
        consent
    }
}

#[cfg(windows)]
fn show_permission_dialog(device: Device) {
    let (title, message, detail) = match device {
        Device::Camera => (
            "Camera Permission Required",
            "Camera access is not granted.",
            "To record with camera, please allow camera access in Windows Settings.\n\nGo to: Settings > Privacy & security > Camera\nEnable access for desktop apps",
        ),
        Device::Microphone => (
            "Microphone Permission Required",
            "Microphone access is not granted.",
            "To record with microphone, please allow microphone access in Windows Settings.\n\nGo to: Settings > Privacy & security > Microphone\nEnable access for desktop apps",
        ),
    };
    let result = rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_title(title)
        .set_description(format!("{message}\n\n{detail}"))
        .set_buttons(rfd::MessageButtons::OkCancelCustom(
            "Open Settings".into(),
            "Cancel".into(),
        ))
        .show();
    if result == rfd::MessageDialogResult::Custom("Open Settings".into()) {
        crate::system::desktop::open_url(settings_url(device));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deny_consent_is_denied() {
        assert!(consent_is_denied(Some("Deny")));
    }

    #[test]
    fn allow_consent_is_granted() {
        assert!(!consent_is_denied(Some("Allow")));
    }

    #[test]
    fn empty_consent_is_granted() {
        assert!(!consent_is_denied(Some("")));
    }

    #[test]
    fn absent_consent_is_granted() {
        assert!(!consent_is_denied(None));
    }

    #[test]
    fn microphone_uses_microphone_settings_url() {
        assert_eq!(
            settings_url(Device::Microphone),
            "ms-settings:privacy-microphone"
        );
    }

    #[test]
    fn camera_uses_camera_settings_url() {
        assert_eq!(settings_url(Device::Camera), "ms-settings:privacy-webcam");
    }
}
