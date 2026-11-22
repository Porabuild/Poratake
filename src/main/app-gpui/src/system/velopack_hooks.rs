use std::path::{Path, PathBuf};

use windows::core::{w, PCWSTR};
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteTreeW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW,
    RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE,
    REG_SZ,
};

const AUMID: &str = "electron.app.Poratake";
const LEGACY_NSIS_UNINST_KEY: &str =
    r"Software\Microsoft\Windows\CurrentVersion\Uninstall\com.porabuild.poratake";
const LEGACY_ELECTRON_UNINST_KEY: &str =
    r"Software\Microsoft\Windows\CurrentVersion\Uninstall\e53962ac-3318-5756-a837-be6d81ff8e94";

pub fn init() {
    velopack::VelopackApp::build()
        .set_app_user_model_id(AUMID)
        .on_after_install_fast_callback(|_| on_after_install())
        .on_after_update_fast_callback(|_| on_after_update())
        .on_before_uninstall_fast_callback(|_| on_before_uninstall())
        .run();
}

fn on_after_install() {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let Some(dir) = exe.parent() else {
        return;
    };
    register_shell_integration(&exe, dir);
    migrate_autostart_entry(&exe);
    cleanup_legacy_install(dir);
}

fn on_after_update() {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let Some(dir) = exe.parent() else {
        return;
    };
    register_shell_integration(&exe, dir);
    migrate_autostart_entry(&exe);
}

fn on_before_uninstall() {
    delete_reg_value(r"Software\Microsoft\Windows\CurrentVersion\Run", AUMID);
    delete_reg_tree(r"Software\Classes\AppUserModelId\electron.app.Poratake");
    delete_reg_tree(r"Software\Classes\.poratake");
    delete_reg_tree(r"Software\Classes\Poratake.Recording");
}

fn register_shell_integration(exe: &Path, dir: &Path) {
    let exe_str = exe.to_string_lossy();
    let ico_str = dir.join("poratake.ico").to_string_lossy().into_owned();
    let aumid_key = r"Software\Classes\AppUserModelId\electron.app.Poratake";
    set_reg_sz(aumid_key, "DisplayName", "Poratake");
    set_reg_sz(aumid_key, "IconUri", &ico_str);

    set_reg_sz(r"Software\Classes\.poratake", "", "Poratake.Recording");
    set_reg_sz(
        r"Software\Classes\Poratake.Recording",
        "",
        "Poratake Recording",
    );
    set_reg_sz(
        r"Software\Classes\Poratake.Recording\DefaultIcon",
        "",
        &format!("{exe_str},0"),
    );
    set_reg_sz(
        r"Software\Classes\Poratake.Recording\shell\open\command",
        "",
        &format!("\"{exe_str}\" \"%1\""),
    );
}

fn migrate_autostart_entry(exe: &Path) {
    let Some(existing) = read_reg_sz(r"Software\Microsoft\Windows\CurrentVersion\Run", AUMID)
    else {
        return;
    };
    let lower = existing.to_ascii_lowercase();
    if !lower.contains("poratake.exe") && !lower.contains("poratake-gpui.exe") {
        return;
    }
    let exe_str = exe.to_string_lossy();
    let suffix = existing
        .find(" --")
        .map(|idx| &existing[idx..])
        .unwrap_or("");
    let updated = format!("\"{exe_str}\"{suffix}");
    if updated != existing {
        set_reg_sz(
            r"Software\Microsoft\Windows\CurrentVersion\Run",
            AUMID,
            &updated,
        );
    }
}

fn cleanup_legacy_install(current_dir: &Path) {
    let legacy_dirs: Vec<PathBuf> = [
        read_reg_sz(LEGACY_NSIS_UNINST_KEY, "InstallLocation").map(PathBuf::from),
        read_reg_sz(LEGACY_ELECTRON_UNINST_KEY, "InstallLocation").map(PathBuf::from),
        std::env::var_os("LOCALAPPDATA")
            .map(|root| PathBuf::from(root).join("Programs").join("Poratake")),
    ]
    .into_iter()
    .flatten()
    .collect();

    let current_canon = current_dir
        .canonicalize()
        .unwrap_or_else(|_| current_dir.to_path_buf());

    for legacy_dir in legacy_dirs {
        if !legacy_dir.exists() {
            continue;
        }
        let legacy_canon = legacy_dir
            .canonicalize()
            .unwrap_or_else(|_| legacy_dir.clone());
        if legacy_canon == current_canon || current_canon.starts_with(&legacy_canon) {
            continue;
        }
        if !legacy_dir.join("Poratake.exe").exists()
            && !legacy_dir.join("poratake-gpui.exe").exists()
        {
            continue;
        }
        let _ = std::fs::remove_dir_all(&legacy_dir);
    }

    delete_reg_tree(LEGACY_NSIS_UNINST_KEY);
    delete_reg_tree(LEGACY_ELECTRON_UNINST_KEY);

    if let Some(appdata) = std::env::var_os("APPDATA") {
        let legacy_start_folder = PathBuf::from(appdata)
            .join("Microsoft")
            .join("Windows")
            .join("Start Menu")
            .join("Programs")
            .join("Poratake");
        let _ = std::fs::remove_dir_all(legacy_start_folder);
    }
}

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn set_reg_sz(subkey: &str, name: &str, value: &str) {
    let subkey_w = to_wide(subkey);
    let name_w = to_wide(name);
    let value_w = to_wide(value);
    let mut key = HKEY::default();
    unsafe {
        if RegCreateKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey_w.as_ptr()),
            None,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            None,
            &mut key,
            None,
        )
        .is_err()
        {
            return;
        }
        let bytes = std::slice::from_raw_parts(value_w.as_ptr() as *const u8, value_w.len() * 2);
        let name_ptr = if name.is_empty() {
            w!("")
        } else {
            PCWSTR(name_w.as_ptr())
        };
        let _ = RegSetValueExW(key, name_ptr, None, REG_SZ, Some(bytes));
        let _ = RegCloseKey(key);
    }
}

fn read_reg_sz(subkey: &str, name: &str) -> Option<String> {
    let subkey_w = to_wide(subkey);
    let name_w = to_wide(name);
    let mut key = HKEY::default();
    unsafe {
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey_w.as_ptr()),
            None,
            KEY_READ,
            &mut key,
        )
        .is_err()
        {
            return None;
        }
        let mut size = 0u32;
        let query = RegQueryValueExW(
            key,
            PCWSTR(name_w.as_ptr()),
            None,
            None,
            None,
            Some(&mut size),
        );
        if query.is_err() || size < 2 {
            let _ = RegCloseKey(key);
            return None;
        }
        let mut buf = vec![0u8; size as usize];
        let mut kind = REG_SZ;
        let ok = RegQueryValueExW(
            key,
            PCWSTR(name_w.as_ptr()),
            None,
            Some(&mut kind),
            Some(buf.as_mut_ptr()),
            Some(&mut size),
        )
        .is_ok();
        let _ = RegCloseKey(key);
        if !ok || kind != REG_SZ {
            return None;
        }
        let u16s: Vec<u16> = buf[..size as usize]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&chunk| u16::from_le_bytes(chunk))
            .take_while(|&ch| ch != 0)
            .collect();
        String::from_utf16(&u16s).ok()
    }
}

fn delete_reg_value(subkey: &str, name: &str) {
    let subkey_w = to_wide(subkey);
    let name_w = to_wide(name);
    let mut key = HKEY::default();
    unsafe {
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey_w.as_ptr()),
            None,
            KEY_SET_VALUE,
            &mut key,
        )
        .is_ok()
        {
            let _ = RegDeleteValueW(key, PCWSTR(name_w.as_ptr()));
            let _ = RegCloseKey(key);
        }
    }
}

fn delete_reg_tree(subkey: &str) {
    let subkey_w = to_wide(subkey);
    unsafe {
        let _ = RegDeleteTreeW(HKEY_CURRENT_USER, PCWSTR(subkey_w.as_ptr()));
    }
}
