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

        let (config, existed) = match fs::read_to_string(&path) {
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

        let state = std::sync::Arc::new(RwLock::new(StoreState {
            config,
            revision: 0,
            persisted_revision: 0,
        }));

        if !existed {
            // A fresh install writes its defaults immediately so the Electron
            // shell finds a valid config too.
            let snapshot = state.read().config.clone();
            let _ = write_sync(&snapshot, &path);
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
            .spawn(move || loop {
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
                    let (snapshot, revision) = {
                        let state = shared.read();
                        if state.revision == state.persisted_revision {
                            break;
                        }
                        (state.config.clone(), state.revision)
                    };
                    if !write_sync(&snapshot, &path) {
                        break;
                    }
                    let mut state = shared.write();
                    state.persisted_revision = state.persisted_revision.max(revision);
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
        let (snapshot, revision) = {
            let state = self.state.read();
            if state.revision == state.persisted_revision {
                return;
            }
            (state.config.clone(), state.revision)
        };
        if write_sync(&snapshot, &self.path) {
            let mut state = self.state.write();
            state.persisted_revision = state.persisted_revision.max(revision);
        }
    }
}

static WRITE_SIGNAL: parking_lot::Mutex<Option<Sender<()>>> = parking_lot::Mutex::new(None);

fn write_sync(config: &SettingsConfig, path: &Path) -> bool {
    let temp = path.with_extension("json.tmp");
    let payload = serde_json::to_string_pretty(config).unwrap_or_else(|_| "{}".to_string());
    let saved = fs::write(&temp, payload)
        .and_then(|_| replace_file(&temp, path))
        .is_ok();
    if !saved {
        eprintln!("[settings] failed to save config");
    }
    saved
}

#[cfg(windows)]
fn replace_file(temp: &Path, path: &Path) -> std::io::Result<()> {
    use windows::core::HSTRING;
    use windows::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };

    unsafe {
        MoveFileExW(
            &HSTRING::from(temp.as_os_str()),
            &HSTRING::from(path.as_os_str()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    }
    .map_err(std::io::Error::other)
}

#[cfg(not(windows))]
fn replace_file(temp: &Path, path: &Path) -> std::io::Result<()> {
    fs::rename(temp, path)
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
        let scratch = std::env::temp_dir().join("poratake-parity-profile");

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
}
