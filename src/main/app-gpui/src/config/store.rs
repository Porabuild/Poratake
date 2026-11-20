//! Port of `src/main/settings/store.ts` — loads, merges, debounces and
//! atomically persists `config.json` in the same directory the Electron shell
//! uses (`~/.config/poratake[-dev]/config.json`), keeping both shells
//! interchangeable.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender, TryRecvError};
use std::thread;
use std::time::Duration;

use anyhow::{Context as _, Result};
use parking_lot::RwLock;

use crate::config::cloud_secrets::{
    has_unprotected_cloud_secrets, protect_cloud_secrets, reveal_cloud_secrets,
    RetainedCloudSecrets, SecretPolicy,
};
use crate::config::schema::SettingsConfig;

const CONFIG_WRITE_DEBOUNCE_MS: u64 = 150;

/// Overrides the profile directory. A parity or smoke run can point the app at
/// a scratch profile instead of the developer's real config, history and
/// thumbnails.
pub const CONFIG_DIR_ENV: &str = "PORATAKE_CONFIG_DIR";

pub fn config_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os(CONFIG_DIR_ENV) {
        let dir = PathBuf::from(dir);
        if !dir.as_os_str().is_empty() {
            return dir;
        }
    }
    let dir_name = if cfg!(debug_assertions) {
        "poratake-dev"
    } else {
        "poratake"
    };
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".config").join(dir_name)
}

pub fn config_file_path() -> PathBuf {
    config_dir().join("config.json")
}

fn ensure_dir(dir: &Path) -> Result<()> {
    if !dir.exists() {
        fs::create_dir_all(dir)
            .with_context(|| format!("failed to create config directory {}", dir.display()))?;
    }
    Ok(())
}

struct StoreState {
    config: SettingsConfig,
    retained_secrets: RetainedCloudSecrets,
    secret_policy: SecretPolicy,
    revision: u64,
    persisted_revision: u64,
}

pub struct ConfigStore {
    state: std::sync::Arc<RwLock<StoreState>>,
    write_lock: std::sync::Arc<parking_lot::Mutex<()>>,
    path: PathBuf,
}

impl ConfigStore {
    /// Loads the config from disk, falling back to defaults and preserving a
    /// corrupt file for recovery — mirroring the Electron store.
    pub fn load() -> Result<Self> {
        Self::load_at(config_file_path())
    }

    pub fn load_at(path: PathBuf) -> Result<Self> {
        ensure_dir(path.parent().unwrap_or(Path::new(".")))?;

        let (mut config, existed) = match fs::read_to_string(&path) {
            Ok(content) => match serde_json::from_str::<SettingsConfig>(&content) {
                Ok(parsed) => (parsed, true),
                Err(error) => {
                    preserve_corrupt_config(&path, &content);
                    eprintln!("[settings] failed to parse config: {error}");
                    (SettingsConfig::default(), true)
                }
            },
            Err(_) => (SettingsConfig::default(), false),
        };

        let secret_policy = SecretPolicy::for_runtime();
        let needs_secret_upgrade = existed && has_unprotected_cloud_secrets(&config.cloud);
        let mut retained_secrets = reveal_cloud_secrets(&mut config.cloud, secret_policy);

        let state = std::sync::Arc::new(RwLock::new(StoreState {
            config,
            retained_secrets: retained_secrets.clone(),
            secret_policy,
            revision: 0,
            persisted_revision: 0,
        }));

        if !existed || needs_secret_upgrade {
            let snapshot = state.read().config.clone();
            if write_sync(&snapshot, &mut retained_secrets, secret_policy, &path) {
                state.write().retained_secrets = retained_secrets;
            }
        }

        let store = Self {
            state,
            write_lock: std::sync::Arc::new(parking_lot::Mutex::new(())),
            path,
        };
        store.spawn_writer();

        Ok(store)
    }

    fn spawn_writer(&self) {
        let (tx, rx) = mpsc::channel::<()>();
        *WRITE_SIGNAL.lock() = Some(tx);

        let shared = self.state.clone();
        let write_lock = self.write_lock.clone();
        let path = self.path.clone();
        thread::Builder::new()
            .name("config-writer".into())
            .spawn(move || {
                loop {
                    // Wait until an update is requested; exit when the sender is
                    // dropped at process shutdown.
                    if rx.recv().is_err() {
                        break;
                    }
                    // Debounce: keep extending while updates keep arriving so a
                    // burst collapses into one write.
                    loop {
                        thread::sleep(Duration::from_millis(CONFIG_WRITE_DEBOUNCE_MS));
                        match rx.try_recv() {
                            Ok(()) | Err(TryRecvError::Disconnected) => continue,
                            Err(TryRecvError::Empty) => break,
                        }
                    }
                    loop {
                        let _write = write_lock.lock();
                        let (snapshot, mut retained, policy, revision) = {
                            let state = shared.read();
                            if state.revision == state.persisted_revision {
                                break;
                            }
                            (
                                state.config.clone(),
                                state.retained_secrets.clone(),
                                state.secret_policy,
                                state.revision,
                            )
                        };
                        if !write_sync(&snapshot, &mut retained, policy, &path) {
                            break;
                        }
                        let mut state = shared.write();
                        state.retained_secrets = retained;
                        state.persisted_revision = state.persisted_revision.max(revision);
                    }
                }
            })
            .ok();
    }

    pub fn get(&self) -> SettingsConfig {
        self.state.read().config.clone()
    }

    pub fn read<R>(&self, read: impl FnOnce(&SettingsConfig) -> R) -> R {
        read(&self.state.read().config)
    }

    /// Applies a mutation to the whole config and schedules the debounced
    /// save, mirroring `updateConfig`.
    pub fn update<F>(&self, mutate: F)
    where
        F: FnOnce(&mut SettingsConfig),
    {
        {
            let mut guard = self.state.write();
            mutate(&mut guard.config);
            guard.revision = guard.revision.wrapping_add(1);
        }
        if let Some(tx) = WRITE_SIGNAL.lock().as_ref() {
            let _ = tx.send(());
        }
    }

    /// Writes any pending change immediately (used on quit).
    pub fn flush(&self) {
        let _write = self.write_lock.lock();
        let (snapshot, mut retained, policy, revision) = {
            let state = self.state.read();
            if state.revision == state.persisted_revision {
                return;
            }
            (
                state.config.clone(),
                state.retained_secrets.clone(),
                state.secret_policy,
                state.revision,
            )
        };
        if write_sync(&snapshot, &mut retained, policy, &self.path) {
            let mut state = self.state.write();
            state.retained_secrets = retained;
            state.persisted_revision = state.persisted_revision.max(revision);
        }
    }
}

static WRITE_SIGNAL: parking_lot::Mutex<Option<Sender<()>>> = parking_lot::Mutex::new(None);

fn write_sync(
    config: &SettingsConfig,
    retained: &mut RetainedCloudSecrets,
    policy: SecretPolicy,
    path: &Path,
) -> bool {
    let mut disk_config = config.clone();
    if protect_cloud_secrets(&mut disk_config.cloud, retained, policy) {
        eprintln!("[settings] failed to protect cloud credential");
    }
    let temp = path.with_extension("json.tmp");
    let payload = serde_json::to_string_pretty(&disk_config).unwrap_or_else(|_| "{}".to_string());
    let saved = fs::write(&temp, payload)
        .and_then(|_| crate::util::atomic_file::replace(&temp, path))
        .is_ok();
    if !saved {
        eprintln!("[settings] failed to save config");
    }
    saved
}

fn preserve_corrupt_config(path: &Path, content: &str) {
    let recovery = path.with_file_name(format!("config.corrupt-{}.json", timestamp_ms()));
    let _ = fs::write(recovery, content);
}

fn timestamp_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

#[cfg(test)]
mod dir_tests {
    /// The override has to win, and an empty value has to be ignored rather than
    /// resolving the profile to the current directory.
    #[test]
    fn the_environment_override_takes_precedence_when_it_is_not_empty() {
        // Serialised by being the only test that touches this variable.
        let previous = std::env::var_os(super::CONFIG_DIR_ENV);
        let scratch = crate::util::test_paths::unique_temp("poratake-parity-profile");

        unsafe { std::env::set_var(super::CONFIG_DIR_ENV, &scratch) };
        assert_eq!(super::config_dir(), scratch);
        assert_eq!(super::config_file_path(), scratch.join("config.json"));

        unsafe { std::env::set_var(super::CONFIG_DIR_ENV, "") };
        assert!(
            super::config_dir().ends_with("poratake-dev")
                || super::config_dir().ends_with("poratake"),
            "an empty override falls back to the real profile"
        );

        match previous {
            Some(value) => unsafe { std::env::set_var(super::CONFIG_DIR_ENV, value) },
            None => unsafe { std::env::remove_var(super::CONFIG_DIR_ENV) },
        }
    }

    #[test]
    fn flush_replaces_an_existing_config_file() {
        let folder = std::env::temp_dir().join(format!(
            "poratake-config-store-{}-{}",
            std::process::id(),
            super::timestamp_ms()
        ));
        let path = folder.join("config.json");
        let store = super::ConfigStore::load_at(path.clone()).expect("store");
        store.update(|config| config.general.start_on_login = true);
        store.flush();
        store.update(|config| config.general.start_on_login = false);
        store.flush();

        let saved: super::SettingsConfig =
            serde_json::from_str(&std::fs::read_to_string(&path).expect("saved config"))
                .expect("valid config");
        let _ = std::fs::remove_dir_all(folder);
        assert!(!saved.general.start_on_login);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn plaintext_cloud_secrets_are_upgraded_on_load_and_revealed_in_memory() {
        let folder = std::env::temp_dir().join(format!(
            "poratake-config-secrets-{}-{}",
            std::process::id(),
            super::timestamp_ms()
        ));
        std::fs::create_dir_all(&folder).expect("dir");
        let path = folder.join("config.json");
        let mut initial = super::SettingsConfig::default();
        initial.cloud.s3.secret_access_key = "plaintext-s3-key".into();
        std::fs::write(
            &path,
            serde_json::to_string_pretty(&initial).expect("serialize"),
        )
        .expect("write");

        let store = super::ConfigStore::load_at(path.clone()).expect("load");
        assert_eq!(store.get().cloud.s3.secret_access_key, "plaintext-s3-key");

        let saved: super::SettingsConfig =
            serde_json::from_str(&std::fs::read_to_string(&path).expect("saved config"))
                .expect("valid config");
        assert!(saved
            .cloud
            .s3
            .secret_access_key
            .starts_with(super::super::cloud_secrets::GPUI_SECRET_PREFIX));
        assert!(!saved
            .cloud
            .s3
            .secret_access_key
            .contains("plaintext-s3-key"));
        let _ = std::fs::remove_dir_all(folder);
    }

    #[test]
    fn electron_profile_config_migrates_cleanly_to_gpui() {
        let folder = std::env::temp_dir().join(format!(
            "poratake-electron-migrate-{}-{}",
            std::process::id(),
            super::timestamp_ms()
        ));
        std::fs::create_dir_all(&folder).expect("dir");
        let path = folder.join("config.json");
        let locked_electron_secret = format!(
            "{}djEwLWVsZWN0cm9uLXNlY3JldA==",
            super::super::cloud_secrets::SAFE_STORAGE_PREFIX
        );
        let electron_json = serde_json::json!({
            "onboarding": {
                "completed": true
            },
            "general": {
                "startOnLogin": true,
                "playSoundOnScreenshot": false,
                "hideMenuBarIcon": true
            },
            "history": {
                "enabled": true,
                "maxItems": 75
            },
            "appearance": {
                "mode": "light",
                "theme": "sunset"
            },
            "shortcuts": {
                "screenshot": {
                    "area": "CommandOrControl+Shift+4"
                },
                "allInOne": "CommandOrControl+Shift+S"
            },
            "cloud": {
                "enabled": true,
                "activeProvider": "s3",
                "s3": {
                    "endpoint": "https://s3.example.com",
                    "region": "us-east-1",
                    "bucket": "shots",
                    "accessKeyId": "AKIAEXAMPLE",
                    "secretAccessKey": locked_electron_secret,
                    "customDomain": ""
                },
                "rest": {
                    "url": "",
                    "method": "POST",
                    "headers": [
                        { "key": "X-Plain", "value": "legacy-plaintext-token" }
                    ]
                }
            }
        });
        std::fs::write(&path, serde_json::to_string_pretty(&electron_json).unwrap()).unwrap();

        let store = super::ConfigStore::load_at(path.clone()).expect("load electron profile");
        let loaded = store.get();
        assert!(loaded.onboarding.completed);
        assert!(loaded.general.start_on_login);
        assert!(!loaded.general.play_sound_on_screenshot);
        assert!(loaded.general.hide_menu_bar_icon);
        assert_eq!(loaded.history.max_items, 75.0);
        assert_eq!(loaded.appearance.mode, "light");
        assert_eq!(loaded.appearance.theme, "sunset");
        assert_eq!(loaded.shortcuts.screenshot.area, "CommandOrControl+Shift+4");
        assert_eq!(loaded.cloud.rest.headers[0].value, "legacy-plaintext-token");

        store.update(|settings| settings.general.play_sound_on_screenshot = true);
        store.flush();

        let disk: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(
            disk["cloud"]["s3"]["secretAccessKey"].as_str(),
            Some(locked_electron_secret.as_str())
        );
        #[cfg(target_os = "macos")]
        assert!(disk["cloud"]["rest"]["headers"][0]["value"]
            .as_str()
            .unwrap()
            .starts_with(super::super::cloud_secrets::GPUI_SECRET_PREFIX));
        let _ = std::fs::remove_dir_all(folder);
    }
}

#[cfg(all(test, windows))]
mod replacefile_sacl_probe_tests {
    use std::os::windows::ffi::OsStrExt;
    use std::path::{Path, PathBuf};

    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{
        CloseHandle, GetLastError, SetLastError, ERROR_NOT_ALL_ASSIGNED, ERROR_SUCCESS, HANDLE,
        LUID,
    };
    use windows::Win32::Security::{
        AddAuditAccessAceEx, AdjustTokenPrivileges, CreateWellKnownSid, GetAce, GetAclInformation,
        GetFileSecurityW, GetLengthSid, GetSecurityDescriptorControl, GetSecurityDescriptorSacl,
        InitializeAcl, LookupPrivilegeValueW, SetFileSecurityW, SetSecurityDescriptorControl,
        SetSecurityDescriptorSacl, WinWorldSid, ACL, ACL_REVISION, ACL_SIZE_INFORMATION,
        LUID_AND_ATTRIBUTES, PROTECTED_SACL_SECURITY_INFORMATION, SACL_SECURITY_INFORMATION,
        SE_PRIVILEGE_ENABLED, SE_SACL_PROTECTED, SE_SECURITY_NAME, TOKEN_ADJUST_PRIVILEGES,
        TOKEN_PRIVILEGES, TOKEN_QUERY,
    };
    use windows::Win32::Storage::FileSystem::{ReplaceFileW, FILE_EXECUTE};
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    struct PrivilegeGuard {
        token: HANDLE,
        luid: LUID,
        previous: TOKEN_PRIVILEGES,
        restored: bool,
    }

    impl PrivilegeGuard {
        fn set_enabled(&self, enabled: bool) -> std::io::Result<()> {
            let requested = TOKEN_PRIVILEGES {
                PrivilegeCount: 1,
                Privileges: [LUID_AND_ATTRIBUTES {
                    Luid: self.luid,
                    Attributes: if enabled {
                        SE_PRIVILEGE_ENABLED
                    } else {
                        Default::default()
                    },
                }],
            };
            unsafe { SetLastError(ERROR_SUCCESS) };
            unsafe { AdjustTokenPrivileges(self.token, false, Some(&requested), 0, None, None) }
                .map_err(std::io::Error::other)?;
            let status = unsafe { GetLastError() };
            if status != ERROR_SUCCESS {
                return Err(std::io::Error::from_raw_os_error(status.0 as i32));
            }
            Ok(())
        }

        fn restore(&mut self) -> std::io::Result<()> {
            if self.restored {
                return Ok(());
            }
            unsafe { SetLastError(ERROR_SUCCESS) };
            unsafe {
                AdjustTokenPrivileges(
                    self.token,
                    false,
                    Some(&self.previous),
                    std::mem::size_of::<TOKEN_PRIVILEGES>() as u32,
                    None,
                    None,
                )
            }
            .map_err(std::io::Error::other)?;
            let status = unsafe { GetLastError() };
            if status != ERROR_SUCCESS {
                return Err(std::io::Error::from_raw_os_error(status.0 as i32));
            }
            self.restored = true;
            Ok(())
        }
    }

    impl Drop for PrivilegeGuard {
        fn drop(&mut self) {
            let _ = self.restore();
            let _ = unsafe { CloseHandle(self.token) };
        }
    }

    fn enable_sacl_privilege() -> std::io::Result<Option<PrivilegeGuard>> {
        let mut token = HANDLE::default();
        unsafe {
            OpenProcessToken(
                GetCurrentProcess(),
                TOKEN_QUERY | TOKEN_ADJUST_PRIVILEGES,
                &mut token,
            )
        }?;
        let mut luid = LUID::default();
        if let Err(error) = unsafe { LookupPrivilegeValueW(None, SE_SECURITY_NAME, &mut luid) } {
            let _ = unsafe { CloseHandle(token) };
            return Err(std::io::Error::other(error));
        }

        let requested = TOKEN_PRIVILEGES {
            PrivilegeCount: 1,
            Privileges: [LUID_AND_ATTRIBUTES {
                Luid: luid,
                Attributes: SE_PRIVILEGE_ENABLED,
            }],
        };
        let mut previous = TOKEN_PRIVILEGES::default();
        let mut returned = 0;
        unsafe { SetLastError(ERROR_SUCCESS) };
        if let Err(error) = unsafe {
            AdjustTokenPrivileges(
                token,
                false,
                Some(&requested),
                std::mem::size_of::<TOKEN_PRIVILEGES>() as u32,
                Some(&mut previous),
                Some(&mut returned),
            )
        } {
            let _ = unsafe { CloseHandle(token) };
            return Err(std::io::Error::other(error));
        }
        if unsafe { GetLastError() } == ERROR_NOT_ALL_ASSIGNED {
            let _ = unsafe { CloseHandle(token) };
            return Ok(None);
        }
        Ok(Some(PrivilegeGuard {
            token,
            luid,
            previous,
            restored: false,
        }))
    }

    fn wide(path: &Path) -> Vec<u16> {
        path.as_os_str().encode_wide().chain(Some(0)).collect()
    }

    struct SaclSnapshot {
        bytes: Vec<u8>,
        aces: Vec<AceSnapshot>,
        protected: bool,
    }

    #[derive(Debug, PartialEq, Eq)]
    enum AceSnapshot {
        Audit(AuditAce),
        Other(Vec<u8>),
    }

    #[derive(Debug, PartialEq, Eq)]
    struct AuditAce {
        ace_type: u8,
        flags: u8,
        mask: u32,
        sid: Vec<u8>,
    }

    fn write_audit_sacl(path: &Path) -> std::io::Result<Vec<u8>> {
        let mut sid_storage = [0usize; 16];
        let mut sid_size = (sid_storage.len() * std::mem::size_of::<usize>()) as u32;
        unsafe {
            CreateWellKnownSid(
                WinWorldSid,
                None,
                Some(windows::Win32::Security::PSID(
                    sid_storage.as_mut_ptr().cast(),
                )),
                &mut sid_size,
            )
        }
        .map_err(std::io::Error::other)?;

        let mut acl_storage = [0usize; 32];
        let acl = acl_storage.as_mut_ptr().cast::<ACL>();
        unsafe {
            InitializeAcl(
                acl,
                std::mem::size_of_val(&acl_storage) as u32,
                ACL_REVISION,
            )?;
            AddAuditAccessAceEx(
                acl,
                ACL_REVISION,
                Default::default(),
                FILE_EXECUTE.0,
                windows::Win32::Security::PSID(sid_storage.as_mut_ptr().cast()),
                true,
                true,
            )?;
        }

        let mut descriptor = windows::Win32::Security::SECURITY_DESCRIPTOR::default();
        unsafe {
            windows::Win32::Security::InitializeSecurityDescriptor(
                windows::Win32::Security::PSECURITY_DESCRIPTOR(
                    (&mut descriptor as *mut windows::Win32::Security::SECURITY_DESCRIPTOR).cast(),
                ),
                1,
            )?;
            SetSecurityDescriptorSacl(
                windows::Win32::Security::PSECURITY_DESCRIPTOR(
                    (&mut descriptor as *mut windows::Win32::Security::SECURITY_DESCRIPTOR).cast(),
                ),
                true,
                Some(acl),
                false,
            )?;
            SetSecurityDescriptorControl(
                windows::Win32::Security::PSECURITY_DESCRIPTOR(
                    (&mut descriptor as *mut windows::Win32::Security::SECURITY_DESCRIPTOR).cast(),
                ),
                SE_SACL_PROTECTED,
                SE_SACL_PROTECTED,
            )?;
            SetFileSecurityW(
                PCWSTR(wide(path).as_ptr()),
                windows::Win32::Security::OBJECT_SECURITY_INFORMATION(
                    SACL_SECURITY_INFORMATION.0 | PROTECTED_SACL_SECURITY_INFORMATION.0,
                ),
                windows::Win32::Security::PSECURITY_DESCRIPTOR(
                    (&mut descriptor as *mut windows::Win32::Security::SECURITY_DESCRIPTOR).cast(),
                ),
            )
            .ok()?;
            Ok(
                std::slice::from_raw_parts(sid_storage.as_ptr().cast::<u8>(), sid_size as usize)
                    .to_vec(),
            )
        }
    }

    fn acl_bytes(acl: *mut ACL) -> std::io::Result<Vec<u8>> {
        let mut info = ACL_SIZE_INFORMATION::default();
        unsafe {
            GetAclInformation(
                acl,
                (&mut info as *mut ACL_SIZE_INFORMATION).cast(),
                std::mem::size_of::<ACL_SIZE_INFORMATION>() as u32,
                windows::Win32::Security::AclSizeInformation,
            )
            .map_err(std::io::Error::other)?;
            Ok(std::slice::from_raw_parts(acl.cast::<u8>(), info.AclBytesInUse as usize).to_vec())
        }
    }

    fn ace_snapshots(acl: *mut ACL) -> std::io::Result<Vec<AceSnapshot>> {
        let mut info = ACL_SIZE_INFORMATION::default();
        unsafe {
            GetAclInformation(
                acl,
                (&mut info as *mut ACL_SIZE_INFORMATION).cast(),
                std::mem::size_of::<ACL_SIZE_INFORMATION>() as u32,
                windows::Win32::Security::AclSizeInformation,
            )
            .map_err(std::io::Error::other)?;
            let mut result = Vec::with_capacity(info.AceCount as usize);
            for index in 0..info.AceCount {
                let mut ace = std::ptr::null_mut();
                GetAce(acl, index, &mut ace).map_err(std::io::Error::other)?;
                let bytes = ace.cast::<u8>();
                let ace_type = *bytes;
                let flags = *bytes.add(1);
                let ace_size = std::ptr::read_unaligned(bytes.add(2).cast::<u16>()) as usize;
                if ace_size < 4 {
                    return Err(std::io::Error::other("invalid ACE size"));
                }
                if ace_type != 2 {
                    result.push(AceSnapshot::Other(
                        std::slice::from_raw_parts(bytes, ace_size).to_vec(),
                    ));
                    continue;
                }
                if ace_size < 8 {
                    return Err(std::io::Error::other("invalid audit ACE size"));
                }
                let mask = std::ptr::read_unaligned(bytes.add(4).cast::<u32>());
                let sid = bytes.add(8).cast();
                let sid_len = GetLengthSid(windows::Win32::Security::PSID(sid)) as usize;
                if sid_len == 0 || 8 + sid_len != ace_size {
                    return Err(std::io::Error::other("invalid audit ACE SID"));
                }
                result.push(AceSnapshot::Audit(AuditAce {
                    ace_type,
                    flags,
                    mask,
                    sid: std::slice::from_raw_parts(sid.cast::<u8>(), sid_len).to_vec(),
                }));
            }
            Ok(result)
        }
    }

    fn read_sacl(path: &Path) -> std::io::Result<SaclSnapshot> {
        let path = wide(path);
        let mut required = 0;
        let first = unsafe {
            GetFileSecurityW(
                PCWSTR(path.as_ptr()),
                SACL_SECURITY_INFORMATION.0,
                None,
                0,
                &mut required,
            )
        };
        if first.as_bool() || required == 0 {
            return Err(std::io::Error::last_os_error());
        }
        let mut descriptor =
            vec![0usize; required.div_ceil(std::mem::size_of::<usize>() as u32) as usize];
        unsafe {
            GetFileSecurityW(
                PCWSTR(path.as_ptr()),
                SACL_SECURITY_INFORMATION.0,
                Some(windows::Win32::Security::PSECURITY_DESCRIPTOR(
                    descriptor.as_mut_ptr().cast(),
                )),
                (descriptor.len() * std::mem::size_of::<usize>()) as u32,
                &mut required,
            )
            .ok()?;
            let security_descriptor =
                windows::Win32::Security::PSECURITY_DESCRIPTOR(descriptor.as_mut_ptr().cast());
            let mut control = 0u16;
            let mut revision = 0u32;
            GetSecurityDescriptorControl(security_descriptor, &mut control, &mut revision)?;
            let mut present = false.into();
            let mut defaulted = false.into();
            let mut acl = std::ptr::null_mut();
            GetSecurityDescriptorSacl(security_descriptor, &mut present, &mut acl, &mut defaulted)?;
            let bytes = if !present.as_bool() || acl.is_null() {
                Vec::new()
            } else {
                acl_bytes(acl)?
            };
            let aces = if !present.as_bool() || acl.is_null() {
                Vec::new()
            } else {
                ace_snapshots(acl)?
            };
            Ok(SaclSnapshot {
                bytes,
                aces,
                protected: control & SE_SACL_PROTECTED.0 != 0,
            })
        }
    }

    fn write_probe(
        path: &Path,
        status: &str,
        reason: Option<&str>,
        before: Option<&SaclSnapshot>,
        after: Option<&SaclSnapshot>,
    ) {
        let proof = serde_json::json!({
            "status": status,
            "reason": reason,
            "saclBytesBefore": before.map(|sacl| sacl.bytes.len()),
            "saclBytesAfter": after.map(|sacl| sacl.bytes.len()),
            "saclProtectedBefore": before.map(|sacl| sacl.protected),
            "saclProtectedAfter": after.map(|sacl| sacl.protected),
        });
        std::fs::write(path, serde_json::to_vec(&proof).unwrap()).unwrap();
    }

    #[test]
    #[ignore = "scratch-only hosted ReplaceFileW audit-SACL probe"]
    fn replacefile_reports_audit_sacl_behavior_when_privilege_is_available() {
        assert_eq!(std::env::var("GITHUB_ACTIONS").as_deref(), Ok("true"));
        let runner_temp = std::fs::canonicalize(
            std::env::var_os("RUNNER_TEMP")
                .map(PathBuf::from)
                .expect("hosted runner temporary directory"),
        )
        .expect("resolve runner temp");
        let proof_path = std::env::var_os("PORATAKE_REPLACEFILE_SACL_PROOF")
            .map(PathBuf::from)
            .expect("hosted ReplaceFile SACL proof path");
        let proof_parent = std::fs::canonicalize(proof_path.parent().expect("proof parent"))
            .expect("resolve proof parent");
        assert!(proof_parent.starts_with(&runner_temp));
        assert!(
            !proof_path.exists(),
            "refusing to replace an existing proof"
        );

        let privilege = match enable_sacl_privilege().expect("probe process privilege") {
            Some(privilege) => privilege,
            None => {
                write_probe(
                    &proof_path,
                    "unavailable",
                    Some("SeSecurityPrivilege is not assigned to the process token"),
                    None,
                    None,
                );
                eprintln!("ReplaceFileW audit-SACL probe unavailable: privilege not assigned.");
                return;
            }
        };

        let directory = tempfile::Builder::new()
            .prefix("poratake-replacefile-sacl-")
            .tempdir_in(&runner_temp)
            .expect("scratch directory in runner temp");
        let target = directory.path().join("target.png");
        let replacement = directory.path().join("replacement.png");
        let backup = directory.path().join("backup.png");
        std::fs::write(&target, b"original scratch file").unwrap();
        std::fs::write(&replacement, b"replacement scratch file").unwrap();
        let expected_sacl = write_audit_sacl(&target).expect("apply audit ACE to scratch target");
        let original_sacl = read_sacl(&target).expect("read scratch target SACL");
        let expected_sid = expected_sacl;
        assert_eq!(original_sacl.aces.len(), 1, "audit SACL fixture ACE count");
        assert_eq!(
            original_sacl.aces[0],
            AceSnapshot::Audit(AuditAce {
                ace_type: 2,
                flags: 0xC0,
                mask: FILE_EXECUTE.0,
                sid: expected_sid,
            }),
            "audit SACL fixture ACE fields"
        );
        assert!(
            original_sacl.protected,
            "audit SACL fixture is not protected"
        );
        let mut privilege = privilege;
        privilege
            .set_enabled(false)
            .expect("disable SeSecurityPrivilege during ReplaceFileW");

        unsafe {
            ReplaceFileW(
                PCWSTR(wide(&target).as_ptr()),
                PCWSTR(wide(&replacement).as_ptr()),
                PCWSTR(wide(&backup).as_ptr()),
                Default::default(),
                None,
                None,
            )
        }
        .expect("replace scratch file");
        assert_eq!(std::fs::read(&target).unwrap(), b"replacement scratch file");
        assert_eq!(std::fs::read(&backup).unwrap(), b"original scratch file");

        privilege
            .set_enabled(true)
            .expect("enable SeSecurityPrivilege to verify SACL");
        let replaced_sacl = read_sacl(&target).expect("read replaced target SACL");
        let acl_preserved = replaced_sacl.aces == original_sacl.aces;
        let protection_preserved = replaced_sacl.protected == original_sacl.protected;
        let preserved = acl_preserved && protection_preserved;
        privilege
            .restore()
            .expect("restore original process privilege state");
        let status = if preserved {
            "verified"
        } else {
            "not-preserved"
        };
        let reason = (!preserved).then_some(if !acl_preserved {
            "ReplaceFileW changed the scratch audit SACL"
        } else {
            "ReplaceFileW changed the scratch SACL protection bit"
        });
        write_probe(
            &proof_path,
            status,
            reason,
            Some(&original_sacl),
            Some(&replaced_sacl),
        );
        if !preserved {
            eprintln!(
                "ReplaceFileW did not preserve the scratch audit SACL: bytes {} -> {}, protected {} -> {}.",
                original_sacl.bytes.len(),
                replaced_sacl.bytes.len(),
                original_sacl.protected,
                replaced_sacl.protected
            );
        }
    }
}
