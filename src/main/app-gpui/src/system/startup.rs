//! The "Start on login" setting on Windows.
//!
//! The entry lives in `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`
//! under the value name `electron.app.Poratake`. That name is the contract
//! between the two shells and it is **not** the product name: Electron's
//! `app.setLoginItemSettings({ openAtLogin })` defaults the Run value name to
//! the app's AppUserModelID, which is `electron.app.<name>`.
//!
//! Verified on this machine rather than assumed -- `HKCU\...\Run` already
//! contains `electron.app.Loom` pointing at an Electron app's exe, and the
//! same `electron.app.Poratake` string is the AUMID the toast notifications
//! are delivered under (see `system/notification.rs`).
//!
//! Getting this wrong is invisible until it bites: a name of `Poratake` here
//! would sit *alongside* Electron's entry rather than replacing it, so the app
//! would auto-start twice and turning the setting off in one shell would leave
//! the other shell's entry behind.

/// Writes or removes the Run entry so the app starts when the user logs in.
///
/// Mirror of Electron's `app.setLoginItemSettings({ openAtLogin })` at the
/// call sites in `updateConfig` and `applyLoginItemSetting`: the value holds
/// the executable path, quoted when it contains spaces, exactly the shape
/// `setLoginItemSettings` produces.
#[cfg(windows)]
pub fn set_open_at_login(enabled: bool) {
    use windows::core::w;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
        KEY_SET_VALUE, REG_SZ,
    };

    let Ok(exe) = std::env::current_exe() else {
        eprintln!("[startup] cannot resolve the current executable path");
        return;
    };
    let path = quote_path(&exe.to_string_lossy());

    let mut key = HKEY::default();
    // SAFETY: `key` is initialised here, the value is written before the key
    // is closed on every path below, and the buffer handed to `RegSetValueExW`
    // stays borrowed from the `wide` vec for the duration of the call.
    unsafe {
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            w!(r"Software\Microsoft\Windows\CurrentVersion\Run"),
            None,
            KEY_SET_VALUE,
            &mut key,
        )
        .is_err()
        {
            eprintln!("[startup] cannot open the Run registry key");
            return;
        }

        if !enabled {
            // Deleting a value that was never set reports FILE_NOT_FOUND and
            // is a no-op, exactly like `deleteItem` on the Electron side, so
            // the error is not worth reporting.
            let _ = RegDeleteValueW(key, w!("electron.app.Poratake"));
            let _ = RegCloseKey(key);
            return;
        }

        let mut wide: Vec<u16> = path.encode_utf16().collect();
        wide.push(0);
        // SAFETY: the byte slice borrows the `wide` vec above, which outlives
        // the call, and u8 alignment is weaker than u16 so the pointer cast is
        // valid.
        let bytes = std::slice::from_raw_parts(wide.as_ptr() as *const u8, wide.len() * 2);
        let result = RegSetValueExW(key, w!("electron.app.Poratake"), None, REG_SZ, Some(bytes));
        let _ = RegCloseKey(key);
        if result.is_err() {
            eprintln!("[startup] failed to write the Run registry value");
        }
    }
}

/// Whether the Run entry for this app exists right now.
///
/// Read-only counterpart to [`set_open_at_login`], mirroring the Electron
/// shell's `app.getLoginItemSettings`.
#[cfg(windows)]
pub fn is_open_at_login() -> bool {
    use windows::core::w;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ,
    };

    let mut key = HKEY::default();
    // SAFETY: `key` is initialised here and closed on every path below; the
    // zero-sized query only asks whether the value exists.
    unsafe {
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            w!(r"Software\Microsoft\Windows\CurrentVersion\Run"),
            None,
            KEY_READ,
            &mut key,
        )
        .is_err()
        {
            return false;
        }

        let mut size: u32 = 0;
        // A `None` data buffer with a size slot is a pure existence probe:
        // success means the value is there (reporting its size), FILE_NOT_FOUND
        // means it is not.
        let result = RegQueryValueExW(
            key,
            w!("electron.app.Poratake"),
            None,
            None,
            None,
            Some(&mut size),
        );
        let _ = RegCloseKey(key);
        !result.is_err()
    }
}

/// `setLoginItemSettings` quotes the executable path when it contains spaces,
/// so the auto-run command line stays one argument; a path without spaces is
/// written bare, and one that is already quoted is not quoted twice.
#[cfg(any(windows, test))]
fn quote_path(path: &str) -> String {
    if path.contains(' ') && !path.starts_with('"') {
        format!("\"{path}\"")
    } else {
        path.to_string()
    }
}

#[cfg(target_os = "macos")]
pub fn set_open_at_login(enabled: bool) {
    let Some(path) = dirs::home_dir().map(|home| {
        home.join("Library")
            .join("LaunchAgents")
            .join("com.porabuild.poratake-gpui.plist")
    }) else {
        return;
    };
    if !enabled {
        let _ = std::fs::remove_file(path);
        return;
    }
    let Ok(executable) = std::env::current_exe() else {
        return;
    };
    let Some(parent) = path.parent() else {
        return;
    };
    if std::fs::create_dir_all(parent).is_err() {
        return;
    }
    let executable = xml_escape(executable.to_string_lossy().as_ref());
    let plist = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\"><plist version=\"1.0\"><dict><key>Label</key><string>com.porabuild.poratake-gpui</string><key>ProgramArguments</key><array><string>{executable}</string></array><key>RunAtLoad</key><true/></dict></plist>"
    );
    if let Err(error) = std::fs::write(path, plist) {
        eprintln!("[startup] failed to write launch agent: {error}");
    }
}

#[cfg(target_os = "macos")]
pub fn is_open_at_login() -> bool {
    dirs::home_dir().is_some_and(|home| {
        home.join("Library")
            .join("LaunchAgents")
            .join("com.porabuild.poratake-gpui.plist")
            .is_file()
    })
}

#[cfg(target_os = "linux")]
pub fn set_open_at_login(enabled: bool) {
    let Some(path) = dirs::config_dir().map(|config| {
        config
            .join("autostart")
            .join("com.porabuild.poratake-gpui.desktop")
    }) else {
        return;
    };
    if !enabled {
        let _ = std::fs::remove_file(path);
        return;
    }
    let Ok(executable) = std::env::current_exe() else {
        return;
    };
    let Some(parent) = path.parent() else {
        return;
    };
    if std::fs::create_dir_all(parent).is_err() {
        return;
    }
    let entry = format!(
        "[Desktop Entry]\nType=Application\nName=Poratake\nExec={}\nTerminal=false\nX-GNOME-Autostart-enabled=true\n",
        quote_desktop_exec(executable.to_string_lossy().as_ref())
    );
    if let Err(error) = std::fs::write(path, entry) {
        eprintln!("[startup] failed to write autostart entry: {error}");
    }
}

#[cfg(target_os = "linux")]
pub fn is_open_at_login() -> bool {
    dirs::config_dir().is_some_and(|config| {
        config
            .join("autostart")
            .join("com.porabuild.poratake-gpui.desktop")
            .is_file()
    })
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
pub fn set_open_at_login(_enabled: bool) {}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
pub fn is_open_at_login() -> bool {
    false
}

#[cfg(any(target_os = "macos", test))]
fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(any(target_os = "linux", test))]
fn quote_desktop_exec(value: &str) -> String {
    format!(
        "\"{}\"",
        value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('$', "\\$")
            .replace('`', "\\`")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_run_value_quotes_paths_with_spaces() {
        assert_eq!(
            quote_path("C:\\Poratake\\poratake.exe"),
            "C:\\Poratake\\poratake.exe"
        );
        assert_eq!(
            quote_path("C:\\Program Files\\Poratake\\poratake.exe"),
            "\"C:\\Program Files\\Poratake\\poratake.exe\""
        );
        // An already-quoted path is left alone.
        assert_eq!(
            quote_path("\"C:\\Program Files\\Poratake\\poratake.exe\""),
            "\"C:\\Program Files\\Poratake\\poratake.exe\""
        );
    }

    #[test]
    fn platform_startup_files_escape_executable_paths() {
        assert_eq!(xml_escape("A&B<\"C\""), "A&amp;B&lt;&quot;C&quot;");
        assert_eq!(
            quote_desktop_exec("/opt/Poratake App/poratake\"gpui"),
            "\"/opt/Poratake App/poratake\\\"gpui\""
        );
    }
}
