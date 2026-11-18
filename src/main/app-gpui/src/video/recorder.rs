//! Screen recording — port of `capture/video/recorder.ts`: project creation,
//! the daemon `screen-recorder` contract and the recorder state machine.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU8, Ordering};

use poratake_daemon_common::contract::{
    ScreenRecorderMicrophoneRequest, ScreenRecorderStartRequest,
};

use crate::config::store::ConfigStore;
use crate::daemon::DaemonHandle;
use crate::video::project::PROJECT_EXTENSION;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RecorderState {
    Idle,
    Starting,
    Stopping,
    Recording,
    Paused,
}

static STATE: AtomicU8 = AtomicU8::new(0);

fn store_state(state: RecorderState) {
    STATE.store(
        match state {
            RecorderState::Idle => 0,
            RecorderState::Starting => 3,
            RecorderState::Stopping => 4,
            RecorderState::Recording => 1,
            RecorderState::Paused => 2,
        },
        Ordering::SeqCst,
    );
}

pub fn state() -> RecorderState {
    match STATE.load(Ordering::SeqCst) {
        1 => RecorderState::Recording,
        2 => RecorderState::Paused,
        3 => RecorderState::Starting,
        4 => RecorderState::Stopping,
        _ => RecorderState::Idle,
    }
}

pub fn is_recording() -> bool {
    state() != RecorderState::Idle
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RecordingTarget {
    Screen,
    Area,
    Window,
}

impl RecordingTarget {
    #[allow(dead_code)]
    pub fn label(self) -> &'static str {
        match self {
            Self::Screen => "Screen",
            Self::Area => "Area",
            Self::Window => "Window",
        }
    }
}

#[derive(Clone, Debug)]
pub struct RecordingConfig {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub display_id: Option<u32>,
    pub window_id: Option<i64>,
    pub excluded_window_ids: Vec<i64>,
    pub include_audio: bool,
    pub mic_enabled: bool,
    pub mic_device_id: Option<String>,
    pub camera_enabled: bool,
    pub camera_device_id: Option<String>,
    pub ios_device_id: Option<String>,
    pub ios_device_name: Option<String>,
    pub keyboard_enabled: bool,
    pub frame_rate: u32,
    pub output_path: PathBuf,
}

/// Port of `getRecordingsDir`.
pub fn recordings_dir(store: &ConfigStore) -> PathBuf {
    let config = store.get();
    let custom = config.storage.recordings_path.clone();
    if !custom.is_empty() && std::path::Path::new(&custom).is_dir() {
        return PathBuf::from(custom);
    }
    dirs::video_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Poratake")
}

/// Port of `generateRecordingProjectName`.
pub fn project_name(store: &ConfigStore, at: chrono::DateTime<chrono::Local>) -> String {
    let config = store.get();
    let pattern = if config.storage.naming_pattern.is_empty() {
        "%type %Y-%m-%d at %H.%M.%S"
    } else {
        &config.storage.naming_pattern
    };
    let base = crate::editor::filename::generate_filename(pattern, "Recording", "", at);
    let base = base.trim_end_matches('.').to_string();
    format!("{base}{PROJECT_EXTENSION}")
}

/// Port of `createRecordingProject`: a uniquely named `.poratake` folder.
pub fn create_project(store: &ConfigStore) -> std::io::Result<PathBuf> {
    let directory = recordings_dir(store);
    std::fs::create_dir_all(&directory)?;

    let generated = project_name(store, chrono::Local::now());
    let stem = generated
        .strip_suffix(PROJECT_EXTENSION)
        .unwrap_or(&generated)
        .to_string();

    let mut path = directory.join(&generated);
    let mut suffix = 2;
    loop {
        match std::fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                path = directory.join(format!("{stem} {suffix}{PROJECT_EXTENSION}"));
                suffix += 1;
            }
            Err(error) => return Err(error),
        }
    }
}

pub fn start(
    daemon: &DaemonHandle,
    config: &RecordingConfig,
) -> anyhow::Result<crate::daemon::ScreenRecordingStart> {
    if is_recording() {
        anyhow::bail!("a recording is already active");
    }
    let request = start_request(config);
    #[cfg(any(windows, target_os = "macos"))]
    if let Err(error) = set_camera_content_protection(daemon, true) {
        if let Err(rollback_error) = set_camera_content_protection(daemon, false) {
            eprintln!("[recorder] camera content protection rollback failed: {rollback_error}");
        }
        return Err(error.context("camera preview protection failed"));
    }
    let result = match daemon.screen_recorder().start(&request) {
        Ok(result) => result,
        Err(error) => {
            #[cfg(any(windows, target_os = "macos"))]
            if let Err(error) = set_camera_content_protection(daemon, false) {
                eprintln!("[recorder] camera content protection rollback failed: {error}");
            }
            let context = format!("screen-recorder start failed: {error}");
            return Err(error.context(context));
        }
    };
    match &result {
        crate::daemon::ScreenRecordingStart::Started => store_state(RecorderState::Recording),
        #[cfg(target_os = "macos")]
        crate::daemon::ScreenRecordingStart::Pending(_) => store_state(RecorderState::Starting),
    }
    Ok(result)
}

#[cfg(target_os = "macos")]
pub fn confirm_started() {
    let _ = STATE.compare_exchange(3, 1, Ordering::SeqCst, Ordering::SeqCst);
}

fn start_request(config: &RecordingConfig) -> ScreenRecorderStartRequest {
    ScreenRecorderStartRequest {
        x: Some(config.x),
        y: Some(config.y),
        width: Some(config.width),
        height: Some(config.height),
        display_id: config.display_id,
        window_id: config.window_id,
        excluded_window_ids: config.excluded_window_ids.clone(),
        include_audio: config.include_audio,
        mic_enabled: config.mic_enabled,
        mic_device_id: config.mic_device_id.clone(),
        mic_device_name: None,
        camera_enabled: config.camera_enabled,
        camera_device_id: config.camera_device_id.clone(),
        camera_device_name: None,
        keyboard_enabled: config.keyboard_enabled,
        frame_rate: config.frame_rate,
        output_path: crate::video::project::recording_video_path(&config.output_path),
        ios_device_id: config.ios_device_id.clone(),
        ios_device_name: config.ios_device_name.clone(),
    }
}

pub fn pause(daemon: &DaemonHandle) {
    if state() != RecorderState::Recording {
        return;
    }
    match daemon.screen_recorder().pause() {
        Ok(()) => store_state(RecorderState::Paused),
        Err(error) => eprintln!("[recorder] pause failed: {error}"),
    }
}

pub fn resume(daemon: &DaemonHandle) {
    if state() != RecorderState::Paused {
        return;
    }
    match daemon.screen_recorder().resume() {
        Ok(()) => store_state(RecorderState::Recording),
        Err(error) => eprintln!("[recorder] resume failed: {error}"),
    }
}

pub enum RecordingStop {
    Stopped(crate::daemon::ScreenRecordingStopInfo),
    #[cfg(target_os = "macos")]
    Pending(
        std::sync::mpsc::Receiver<
            std::result::Result<serde_json::Value, crate::daemon::DaemonRpcError>,
        >,
        RecorderState,
    ),
}

pub fn stop(daemon: &DaemonHandle) -> anyhow::Result<RecordingStop> {
    if !is_recording() {
        anyhow::bail!("no recording is active");
    }
    let previous = state();
    #[cfg(not(target_os = "macos"))]
    let _ = previous;
    match daemon.screen_recorder().stop()? {
        crate::daemon::ScreenRecordingStop::Stopped(duration) => {
            complete_stop(daemon);
            Ok(RecordingStop::Stopped(duration))
        }
        #[cfg(target_os = "macos")]
        crate::daemon::ScreenRecordingStop::Pending(response) => {
            store_state(RecorderState::Stopping);
            Ok(RecordingStop::Pending(response, previous))
        }
    }
}

pub fn complete_stop(daemon: &DaemonHandle) {
    if let Err(error) = set_camera_content_protection(daemon, false) {
        eprintln!("[recorder] camera content protection failed: {error}");
    }
    store_state(RecorderState::Idle);
}

#[cfg(target_os = "macos")]
pub fn restore_after_failed_stop(previous: RecorderState) {
    if state() == RecorderState::Stopping {
        store_state(previous);
    }
}

/// Forces idle after a terminal daemon failure, mirroring Electron's
/// `handleTerminalRecordingError` state reset. The daemon already errored
/// out, so no stop call is issued.
pub fn force_idle() {
    store_state(RecorderState::Idle);
}

pub fn set_microphone(
    daemon: &DaemonHandle,
    enabled: bool,
    device_id: Option<&str>,
) -> anyhow::Result<()> {
    daemon
        .screen_recorder()
        .set_microphone(ScreenRecorderMicrophoneRequest {
            enabled,
            device_id: device_id.map(str::to_string),
            device_name: None,
        })
}

#[cfg(not(target_os = "macos"))]
pub fn set_system_audio(daemon: &DaemonHandle, enabled: bool) -> anyhow::Result<()> {
    daemon.screen_recorder().set_system_audio(enabled)
}

pub fn set_camera(daemon: &DaemonHandle, enabled: bool) -> anyhow::Result<()> {
    daemon.screen_recorder().set_camera(enabled)
}

fn set_camera_content_protection(daemon: &DaemonHandle, enabled: bool) -> anyhow::Result<()> {
    daemon.camera_preview().set_content_protection(enabled)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_names_end_with_the_project_extension() {
        let store = ConfigStore::load_at(std::env::temp_dir().join(format!(
            "poratake-recorder-test-{}.json",
            std::process::id()
        )))
        .expect("store");
        let at = chrono::Local::now();
        let name = project_name(&store, at);
        assert!(name.ends_with(PROJECT_EXTENSION), "{name}");
        assert!(name.starts_with("Recording "), "{name}");
        assert!(!name.contains(".poratake."), "{name}");
    }

    #[test]
    fn concurrent_recordings_reserve_distinct_project_directories() {
        let directory = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(
            ConfigStore::load_at(directory.path().join("config.json")).unwrap(),
        );
        store.update(|config| {
            config.storage.recordings_path = directory.path().to_string_lossy().into_owned();
            config.storage.naming_pattern = "Concurrent recording".into();
        });
        let occupied = directory.path().join("Concurrent recording.poratake");
        std::fs::write(&occupied, b"existing data").unwrap();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(24));
        let workers: Vec<_> = (0..24)
            .map(|_| {
                let store = store.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    create_project(&store).unwrap()
                })
            })
            .collect();
        let projects: Vec<_> = workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect();
        assert!(projects.iter().all(|project| project.is_dir()));
        assert_eq!(
            projects
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len(),
            24
        );
        assert_eq!(std::fs::read(occupied).unwrap(), b"existing data");
    }

    #[test]
    fn start_request_keeps_the_selected_display_id() {
        let config = RecordingConfig {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
            display_id: Some(73),
            window_id: None,
            excluded_window_ids: Vec::new(),
            include_audio: true,
            mic_enabled: false,
            mic_device_id: None,
            camera_enabled: false,
            camera_device_id: None,
            ios_device_id: None,
            ios_device_name: None,
            keyboard_enabled: false,
            frame_rate: 60,
            output_path: PathBuf::from("capture.poratake"),
        };

        assert_eq!(start_request(&config).display_id, Some(73));
    }

    #[test]
    fn state_transitions_are_observable() {
        store_state(RecorderState::Idle);
        assert!(!is_recording());
        store_state(RecorderState::Starting);
        assert!(is_recording());
        assert_eq!(state(), RecorderState::Starting);
        #[cfg(target_os = "macos")]
        {
            confirm_started();
            assert_eq!(state(), RecorderState::Recording);
        }
        store_state(RecorderState::Recording);
        assert!(is_recording());
        store_state(RecorderState::Paused);
        assert_eq!(state(), RecorderState::Paused);
        store_state(RecorderState::Idle);
    }

    /// The calls need a live daemon, so this is a source-level regression
    /// test: the call site above the test module must survive refactors. The
    /// module is split off before searching, so this test's own literals can
    /// never satisfy the assertion.
    #[test]
    fn recorder_still_calls_set_content_protection() {
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/video/recorder.rs"),
        )
        .expect("read recorder.rs");
        let production = source.split("#[cfg(test)]").next().expect("test module");
        assert!(production.contains("camera_preview().set_content_protection(enabled)"));
    }

    /// A rename of the command in the Windows daemon would silently break the
    /// feature, so pin its dispatch arm to the name this file calls.
    #[test]
    fn daemon_still_serves_set_content_protection() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .expect("repository root");
        let source =
            std::fs::read_to_string(root.join("src/main/daemon-win/src/modules/camera_preview.rs"))
                .expect("read camera_preview.rs");
        assert!(
            source.contains("Some(CameraPreviewMethod::SetContentProtection)"),
            "camera_preview.rs no longer serves the setContentProtection command"
        );
    }
}
