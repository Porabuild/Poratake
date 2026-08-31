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
    Recording,
    Paused,
}

static STATE: AtomicU8 = AtomicU8::new(0);

fn store_state(state: RecorderState) {
    STATE.store(
        match state {
            RecorderState::Idle => 0,
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
    while path.exists() {
        path = directory.join(format!("{stem} {suffix}{PROJECT_EXTENSION}"));
        suffix += 1;
    }
    std::fs::create_dir_all(&path)?;
    Ok(path)
}

pub fn start(daemon: &DaemonHandle, config: &RecordingConfig) -> anyhow::Result<()> {
    if is_recording() {
        anyhow::bail!("a recording is already active");
    }
    let request = start_request(config);

    daemon
        .screen_recorder()
        .start(&request)
        .map_err(|error| anyhow::anyhow!("screen-recorder start failed: {error}"))?;
    if config.camera_enabled {
        if let Err(error) = set_camera_content_protection(daemon, true) {
            eprintln!("[recorder] camera content protection failed: {error}");
        }
    }
    store_state(RecorderState::Recording);
    Ok(())
}

fn start_request(config: &RecordingConfig) -> ScreenRecorderStartRequest {
    ScreenRecorderStartRequest {
        x: Some(config.x),
        y: Some(config.y),
        width: Some(config.width),
        height: Some(config.height),
        display_id: config.display_id,
        window_id: config.window_id,
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

pub fn stop(daemon: &DaemonHandle) -> bool {
    if !is_recording() {
        return false;
    }
    let stopped = daemon.screen_recorder().stop();
    match stopped {
        Ok(_) => {
            if let Err(error) = set_camera_content_protection(daemon, false) {
                eprintln!("[recorder] camera content protection failed: {error}");
            }
            store_state(RecorderState::Idle);
            true
        }
        Err(error) => {
            eprintln!("[recorder] stop failed: {error}");
            false
        }
    }
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

pub fn set_system_audio(daemon: &DaemonHandle, enabled: bool) -> anyhow::Result<()> {
    daemon.screen_recorder().set_system_audio(enabled)
}

pub fn set_camera(daemon: &DaemonHandle, enabled: bool) -> anyhow::Result<()> {
    daemon.screen_recorder().set_camera(enabled)?;
    if let Err(error) = set_camera_content_protection(daemon, enabled) {
        // The camera was switched but its capture exclusion could not be, so
        // the daemon is rolled back to the state the caller still believes in.
        if enabled {
            let _ = daemon.screen_recorder().set_camera(false);
        }
        return Err(error);
    }
    Ok(())
}

/// The daemon's `camera_preview` module defaults `content_protected` to false
/// and its `show` command takes no flag, so — just like Electron's
/// `recording-actions.ts` — the shell must switch the capture affinity
/// explicitly on start and toggle, and back off on stop. Without this the
/// preview bubble is burned into the recording.
fn set_camera_content_protection(daemon: &DaemonHandle, enabled: bool) -> anyhow::Result<()> {
    daemon.camera_preview().set_content_protection(enabled)
}

/// Port of `formatDuration` in the recording control bar: `M:SS`.
pub fn format_elapsed(seconds: u64) -> String {
    format!("{}:{:02}", seconds / 60, seconds % 60)
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
    fn formats_elapsed_time() {
        assert_eq!(format_elapsed(0), "0:00");
        assert_eq!(format_elapsed(9), "0:09");
        assert_eq!(format_elapsed(75), "1:15");
        assert_eq!(format_elapsed(3600), "60:00");
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
