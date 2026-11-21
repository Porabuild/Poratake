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

#[cfg(windows)]
fn read_windows_run_value() -> Option<String> {
    use windows::core::w;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ, REG_SZ,
    };

    let mut key = HKEY::default();
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
            return None;
        }

        let mut size: u32 = 0;
        let mut kind = REG_SZ;
        let probe = RegQueryValueExW(
            key,
            w!("electron.app.Poratake"),
            None,
            Some(&mut kind),
            None,
            Some(&mut size),
        );
        if probe.is_err() || kind != REG_SZ || size == 0 {
            let _ = RegCloseKey(key);
            return None;
        }
        let mut buffer = vec![0u16; (size as usize).div_ceil(2)];
        let read = RegQueryValueExW(
            key,
            w!("electron.app.Poratake"),
            None,
            Some(&mut kind),
            Some(buffer.as_mut_ptr().cast()),
            Some(&mut size),
        );
        let _ = RegCloseKey(key);
        if read.is_err() {
            return None;
        }
        let end = buffer
            .iter()
            .position(|&unit| unit == 0)
            .unwrap_or(buffer.len());
        Some(String::from_utf16_lossy(&buffer[..end]))
    }
}

#[cfg(any(windows, test))]
fn windows_run_value_matches(stored: &str, executable: &str) -> bool {
    let expected = quote_path(executable);
    stored.trim().eq_ignore_ascii_case(expected.trim())
        || stored
            .trim()
            .trim_matches('"')
            .eq_ignore_ascii_case(executable.trim())
}

pub fn reconcile_open_at_login(enabled: bool) {
    #[cfg(windows)]
    {
        let current = read_windows_run_value();
        if !enabled {
            if current.is_some() {
                set_open_at_login(false);
            }
            return;
        }
        let Ok(exe) = std::env::current_exe() else {
            return;
        };
        let exe_str = exe.to_string_lossy();
        let already_current = current
            .as_deref()
            .is_some_and(|stored| windows_run_value_matches(stored, &exe_str));
        if !already_current {
            set_open_at_login(true);
        }
    }
    #[cfg(target_os = "linux")]
    {
        remove_legacy_linux_autostart();
        if enabled {
            set_open_at_login(true);
        } else if is_open_at_login() {
            set_open_at_login(false);
        }
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        if is_open_at_login() != enabled {
            set_open_at_login(enabled);
        }
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
fn legacy_launch_agent_path() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|home| {
        home.join("Library")
            .join("LaunchAgents")
            .join("com.porabuild.poratake-gpui.plist")
    })
}

#[cfg(target_os = "macos")]
fn packaged_app_bundle() -> bool {
    std::env::current_exe().is_ok_and(|exe| crate::update::path_is_packaged(&exe))
}

#[cfg(target_os = "macos")]
mod sm_app_service {
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send};

    #[link(name = "ServiceManagement", kind = "framework")]
    unsafe extern "C" {}

    const STATUS_ENABLED: isize = 1;

    fn main_app_service() -> *mut AnyObject {
        unsafe { msg_send![class!(SMAppService), mainAppService] }
    }

    pub fn is_enabled() -> Option<bool> {
        let service = main_app_service();
        if service.is_null() {
            return None;
        }
        let status: isize = unsafe { msg_send![service, status] };
        Some(status == STATUS_ENABLED)
    }

    pub fn set_enabled(enabled: bool) -> bool {
        let service = main_app_service();
        if service.is_null() {
            return false;
        }
        let mut error: *mut AnyObject = std::ptr::null_mut();
        let ok: objc2::runtime::Bool = if enabled {
            unsafe { msg_send![service, registerAndReturnError: &mut error] }
        } else {
            unsafe { msg_send![service, unregisterAndReturnError: &mut error] }
        };
        if !ok.as_bool() && !error.is_null() {
            let desc: *const AnyObject = unsafe { msg_send![error, localizedDescription] };
            if !desc.is_null() {
                let utf8: *const std::ffi::c_char = unsafe { msg_send![desc, UTF8String] };
                if !utf8.is_null() {
                    let message = unsafe { std::ffi::CStr::from_ptr(utf8) }.to_string_lossy();
                    eprintln!("[startup] SMAppService failed: {message}");
                }
            }
        }
        ok.as_bool()
    }
}

#[cfg(target_os = "macos")]
pub fn set_open_at_login(enabled: bool) {
    if packaged_app_bundle() && sm_app_service::set_enabled(enabled) {
        if let Some(legacy) = legacy_launch_agent_path() {
            let _ = std::fs::remove_file(legacy);
        }
        return;
    }
    let Some(path) = legacy_launch_agent_path() else {
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
    if packaged_app_bundle() {
        if let Some(enabled) = sm_app_service::is_enabled() {
            return enabled;
        }
    }
    legacy_launch_agent_path().is_some_and(|path| path.is_file())
}

#[cfg(target_os = "linux")]
fn linux_autostart_path() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|config| {
        config
            .join("autostart")
            .join("com.porabuild.poratake-gpui.desktop")
    })
}

#[cfg(target_os = "linux")]
fn write_startup_file(path: &std::path::Path, contents: &str) -> std::io::Result<()> {
    crate::util::atomic_file::write(&resolve_startup_target(path), contents.as_bytes())
}

#[cfg(target_os = "linux")]
fn resolve_startup_target(path: &std::path::Path) -> std::path::PathBuf {
    let mut current = path.to_path_buf();
    for _ in 0..8 {
        let Ok(metadata) = std::fs::symlink_metadata(&current) else {
            break;
        };
        if !metadata.file_type().is_symlink() {
            break;
        }
        let Ok(target) = std::fs::read_link(&current) else {
            break;
        };
        current = if target.is_absolute() {
            target
        } else {
            current
                .parent()
                .map(|parent| parent.join(&target))
                .unwrap_or(target)
        };
    }
    current
}

#[cfg(any(target_os = "linux", test))]
const HYPRLAND_AUTOSTART_BEGIN: &str = "# >>> Poratake start on login";
#[cfg(any(target_os = "linux", test))]
const HYPRLAND_AUTOSTART_END: &str = "# <<< Poratake start on login";

#[cfg(target_os = "linux")]
fn hyprland_autostart_path() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|config| config.join("hypr").join("autostart.conf"))
}

#[cfg(target_os = "linux")]
fn hyprland_session() -> bool {
    std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some()
        || std::env::var("XDG_CURRENT_DESKTOP").is_ok_and(|name| {
            name.to_ascii_lowercase()
                .split(':')
                .any(|part| part == "hyprland")
        })
}

#[cfg(target_os = "linux")]
fn hyprland_launcher() -> &'static str {
    use std::os::unix::fs::PermissionsExt as _;

    let available = std::env::var_os("PATH").is_some_and(|paths| {
        std::env::split_paths(&paths).any(|directory| {
            let candidate = directory.join("uwsm-app");
            candidate.metadata().is_ok_and(|metadata| {
                metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
            })
        })
    });
    if available {
        "uwsm-app -- "
    } else {
        ""
    }
}

#[cfg(any(target_os = "linux", test))]
fn hyprland_autostart_block(executable: &str, launcher: &str) -> String {
    format!(
        "{HYPRLAND_AUTOSTART_BEGIN}\nexec-once = {launcher}{executable}\n{HYPRLAND_AUTOSTART_END}\n"
    )
}

#[cfg(any(target_os = "linux", test))]
fn replace_hyprland_autostart_block(contents: &str, block: Option<&str>) -> String {
    let lines: Vec<&str> = contents.lines().collect();
    let mut output = String::new();
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index].trim();
        if line == HYPRLAND_AUTOSTART_BEGIN {
            let end = lines[index + 1..]
                .iter()
                .position(|candidate| candidate.trim() == HYPRLAND_AUTOSTART_END);
            match end {
                Some(offset) => {
                    index += offset + 2;
                    continue;
                }
                None => {
                    index += 1;
                    continue;
                }
            }
        }
        if line == HYPRLAND_AUTOSTART_END {
            index += 1;
            continue;
        }
        output.push_str(lines[index]);
        output.push('\n');
        index += 1;
    }
    if let Some(block) = block {
        output.push_str(block);
    }
    output
}

#[cfg(any(target_os = "linux", test))]
fn hyprland_autostart_entries(contents: &str) -> Vec<String> {
    let mut entries = Vec::new();
    let mut inside = false;
    for line in contents.lines() {
        let line = line.trim();
        if line == HYPRLAND_AUTOSTART_BEGIN {
            inside = true;
            continue;
        }
        if line == HYPRLAND_AUTOSTART_END {
            inside = false;
            continue;
        }
        if !inside {
            continue;
        }
        let Some(command) = line.strip_prefix("exec-once =") else {
            continue;
        };
        entries.push(command.trim().to_string());
    }
    entries
}

#[cfg(target_os = "linux")]
fn remove_legacy_linux_autostart() {
    let Some(autostart) = dirs::config_dir().map(|config| config.join("autostart")) else {
        return;
    };
    for legacy in ["Poratake.desktop", "poratake.desktop"] {
        let path = autostart.join(legacy);
        if path.is_file() && legacy_desktop_belongs_to_poratake(&path) {
            let _ = std::fs::remove_file(path);
        }
    }
}

#[cfg(target_os = "linux")]
fn legacy_desktop_belongs_to_poratake(path: &std::path::Path) -> bool {
    let Ok(contents) = std::fs::read_to_string(path) else {
        return false;
    };
    legacy_desktop_contents_belong_to_poratake(&contents)
}

#[cfg(any(target_os = "linux", test))]
fn legacy_desktop_contents_belong_to_poratake(contents: &str) -> bool {
    contents.lines().any(|line| {
        let Some(exec) = line.trim().strip_prefix("Exec=") else {
            return false;
        };
        exec.to_ascii_lowercase().contains("poratake")
    })
}

#[cfg(target_os = "linux")]
fn linux_autostart_matches_current() -> bool {
    let Some(path) = linux_autostart_path() else {
        return false;
    };
    let Ok(contents) = std::fs::read_to_string(path) else {
        return false;
    };
    let Ok(executable) = std::env::current_exe() else {
        return false;
    };
    let expected = format!(
        "Exec={}",
        quote_desktop_exec(executable.to_string_lossy().as_ref())
    );
    contents.lines().any(|line| line.trim() == expected)
}

#[cfg(target_os = "linux")]
fn set_hyprland_open_at_login(enabled: bool) {
    let Some(path) = hyprland_autostart_path() else {
        return;
    };
    let contents = std::fs::read_to_string(&path).unwrap_or_default();
    if !hyprland_session() {
        if hyprland_autostart_entries(&contents).is_empty() {
            return;
        }
        let updated = replace_hyprland_autostart_block(&contents, None);
        if let Err(error) = write_startup_file(&path, &updated) {
            eprintln!("[startup] failed to remove the Hyprland autostart entry: {error}");
        }
        return;
    }
    let Ok(executable) = std::env::current_exe() else {
        return;
    };
    let executable = executable.to_string_lossy();
    if executable.chars().any(char::is_whitespace) {
        eprintln!("[startup] Hyprland autostart needs an executable path without spaces");
        return;
    }
    let block = enabled.then(|| hyprland_autostart_block(&executable, hyprland_launcher()));
    let updated = replace_hyprland_autostart_block(&contents, block.as_deref());
    if updated == contents {
        return;
    }
    let Some(parent) = path.parent() else {
        return;
    };
    if std::fs::create_dir_all(parent).is_err() {
        return;
    }
    if let Err(error) = write_startup_file(&path, &updated) {
        eprintln!("[startup] failed to write the Hyprland autostart entry: {error}");
    }
}

#[cfg(target_os = "linux")]
fn set_linux_autostart_file(enabled: bool) {
    let Some(path) = linux_autostart_path() else {
        return;
    };
    if !enabled {
        let _ = std::fs::remove_file(path);
        return;
    }
    if linux_autostart_matches_current() {
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
    if let Err(error) = write_startup_file(&path, &entry) {
        eprintln!("[startup] failed to write autostart entry: {error}");
    }
}

#[cfg(any(target_os = "linux", test))]
fn linux_autostart_targets(enabled: bool, hyprland: bool) -> (bool, bool) {
    (enabled && !hyprland, enabled && hyprland)
}

#[cfg(target_os = "linux")]
pub fn set_open_at_login(enabled: bool) {
    remove_legacy_linux_autostart();
    let (xdg, hyprland) = linux_autostart_targets(enabled, hyprland_session());
    set_linux_autostart_file(xdg);
    set_hyprland_open_at_login(hyprland);
}

#[cfg(target_os = "linux")]
pub fn is_open_at_login() -> bool {
    let hyprland = hyprland_autostart_path()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .is_some_and(|contents| !hyprland_autostart_entries(&contents).is_empty());
    hyprland || linux_autostart_path().is_some_and(|path| path.is_file())
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
    fn legacy_autostart_removal_keeps_unrelated_desktop_files() {
        assert!(legacy_desktop_contents_belong_to_poratake(
            "[Desktop Entry]\nExec=/usr/bin/poratake\n"
        ));
        assert!(legacy_desktop_contents_belong_to_poratake(
            "Exec=\"/opt/Poratake/Poratake.exe\"\n"
        ));
        assert!(!legacy_desktop_contents_belong_to_poratake(
            "[Desktop Entry]\nExec=/usr/bin/gedit\n"
        ));
    }

    #[test]
    fn platform_startup_files_escape_executable_paths() {
        assert_eq!(xml_escape("A&B<\"C\""), "A&amp;B&lt;&quot;C&quot;");
        assert_eq!(
            quote_desktop_exec("/opt/Poratake App/poratake\"gpui"),
            "\"/opt/Poratake App/poratake\\\"gpui\""
        );
    }

    #[test]
    fn hyprland_registers_only_the_native_autostart_entry() {
        assert_eq!(linux_autostart_targets(true, true), (false, true));
        assert_eq!(linux_autostart_targets(true, false), (true, false));
        assert_eq!(linux_autostart_targets(false, true), (false, false));
        assert_eq!(linux_autostart_targets(false, false), (false, false));
    }

    #[test]
    fn hyprland_autostart_blocks_leave_user_configuration_untouched() {
        let original = "# user config\nexec-once = waybar\nexec-once = mako\n";
        let executable = "/home/me/.local/opt/poratake/poratake-gpui";
        let block = hyprland_autostart_block(executable, "uwsm-app -- ");
        let updated = replace_hyprland_autostart_block(original, Some(&block));
        assert!(updated.starts_with(original));
        assert!(updated.contains("exec-once = waybar"));
        assert_eq!(
            hyprland_autostart_entries(&updated),
            vec![format!("uwsm-app -- {executable}")]
        );
        assert_eq!(replace_hyprland_autostart_block(&updated, None), original);
    }

    #[test]
    fn hyprland_autostart_keeps_configuration_after_an_unterminated_block() {
        let original =
            format!("{HYPRLAND_AUTOSTART_BEGIN}\nexec-once = waybar\nexec-once = mako\n");
        let cleaned = replace_hyprland_autostart_block(&original, None);
        assert_eq!(cleaned, "exec-once = waybar\nexec-once = mako\n");
    }

    #[test]
    fn hyprland_autostart_blocks_replace_stale_entries_once() {
        let first = hyprland_autostart_block("/old/poratake-gpui", "");
        let second = hyprland_autostart_block("/new/poratake-gpui", "");
        let combined = replace_hyprland_autostart_block(&first, Some(&second));
        assert_eq!(
            hyprland_autostart_entries(&combined),
            vec!["/new/poratake-gpui"]
        );
        assert!(!combined.contains("/old/poratake-gpui"));
    }

    #[test]
    fn windows_run_value_reconciliation_detects_electron_to_gpui_migration() {
        assert!(windows_run_value_matches(
            "\"C:\\Program Files\\Poratake\\poratake-gpui.exe\"",
            "C:\\Program Files\\Poratake\\poratake-gpui.exe"
        ));
        assert!(windows_run_value_matches(
            "c:\\poratake\\poratake-gpui.exe",
            "C:\\Poratake\\poratake-gpui.exe"
        ));
        assert!(!windows_run_value_matches(
            "\"C:\\Users\\user\\AppData\\Local\\Programs\\Poratake\\Poratake.exe\"",
            "C:\\Users\\user\\AppData\\Local\\Programs\\Poratake\\poratake-gpui.exe"
        ));
    }
}
