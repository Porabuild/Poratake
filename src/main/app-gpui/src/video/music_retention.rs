use std::collections::HashSet;
#[cfg(unix)]
use std::fs::File;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::video::{music, project};
use crate::windows::video_editor::model::MusicTrack;

const FOLDER: &str = ".poratake-music-retention";
const SESSIONS: &str = ".poratake-music-sessions";
const COLLECTING_PREFIX: &str = ".collecting-";
const LEGACY_COLLECTING: &str = ".collecting";
static NEXT_SESSION: AtomicU64 = AtomicU64::new(0);

fn marker_name(file_name: &str) -> String {
    let hash = Sha256::digest(file_name.as_bytes());
    let mut name = String::with_capacity(69);
    for byte in hash {
        use std::fmt::Write as _;
        write!(name, "{byte:02x}").expect("writing to a string cannot fail");
    }
    name.push_str(".json");
    name
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Candidate {
    file_name: String,
}

#[derive(Deserialize, Serialize)]
struct SessionRecord {
    pid: u32,
}

pub struct MusicRetentionSession {
    project: PathBuf,
    marker: PathBuf,
}

impl MusicRetentionSession {
    pub fn open(project_or_video: &Path) -> io::Result<Option<Self>> {
        let Some(project) = project::project_folder(project_or_video) else {
            return Ok(None);
        };
        let session = Self::create(project)?;
        collect_candidates(&session.project, Some(&session.marker));
        Ok(Some(session))
    }

    fn create(project: PathBuf) -> io::Result<Self> {
        if !fs::metadata(&project)?.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid recording project folder",
            ));
        }
        let directory = project.join(SESSIONS);
        match fs::create_dir(&directory) {
            Ok(()) => sync_directory(&project)?,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
        if !fs::symlink_metadata(&directory)?.file_type().is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid music session folder",
            ));
        }
        if collector_is_active(&directory)? {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "recording music is being cleaned up",
            ));
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let marker = directory.join(format!(
            "session-{}-{now}-{}.json",
            std::process::id(),
            NEXT_SESSION.fetch_add(1, Ordering::Relaxed)
        ));
        let contents = serde_json::to_vec(&SessionRecord {
            pid: std::process::id(),
        })?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&marker)?;
        if let Err(error) = file.write_all(&contents).and_then(|_| file.sync_all()) {
            drop(file);
            discard_session_marker(&marker, &directory);
            return Err(error);
        }
        drop(file);
        if let Err(error) = sync_directory(&directory) {
            discard_session_marker(&marker, &directory);
            return Err(error);
        }
        let collector_active = match collector_is_active(&directory) {
            Ok(active) => active,
            Err(error) => {
                discard_session_marker(&marker, &directory);
                return Err(error);
            }
        };
        if collector_active {
            discard_session_marker(&marker, &directory);
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "recording music is being cleaned up",
            ));
        }
        let session = Self { project, marker };
        Ok(session)
    }

    pub fn lease(&self) -> io::Result<Self> {
        Self::create(self.project.clone())
    }

    pub fn update_project(&mut self, project_or_video: &Path) {
        let Some(project) = project::project_folder(project_or_video) else {
            return;
        };
        let Some(name) = self.marker.file_name() else {
            return;
        };
        self.marker = project.join(SESSIONS).join(name);
        self.project = project;
    }

    pub fn close(self) -> Option<std::thread::JoinHandle<()>> {
        if fs::remove_file(&self.marker).is_err() {
            return None;
        }
        let _ = sync_directory(&self.project.join(SESSIONS));
        let project = self.project.clone();
        std::thread::Builder::new()
            .spawn(move || collect_candidates(&project, None))
            .ok()
    }
}

fn collector_is_active(directory: &Path) -> io::Result<bool> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            return Ok(true);
        };
        if name == LEGACY_COLLECTING {
            if !entry.file_type()?.is_file() {
                return Ok(true);
            }
            let Ok(contents) = fs::read(&path) else {
                return Ok(true);
            };
            let Ok(record) = serde_json::from_slice::<SessionRecord>(&contents) else {
                return Ok(true);
            };
            if process_is_alive(record.pid) {
                return Ok(true);
            }
            continue;
        }
        let Some(pid) = collector_process_id(name) else {
            if name.starts_with(COLLECTING_PREFIX) {
                return Ok(true);
            }
            continue;
        };
        if !entry.file_type()?.is_file() || process_is_alive(pid) {
            return Ok(true);
        }
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(false)
}

fn discard_session_marker(marker: &Path, directory: &Path) {
    if fs::remove_file(marker).is_ok() {
        let _ = sync_directory(directory);
    }
}

fn collector_process_id(name: &str) -> Option<u32> {
    let owner = name.strip_prefix(COLLECTING_PREFIX)?;
    let (pid, nonce) = owner.split_once('-')?;
    if nonce.is_empty() {
        return None;
    }
    let pid = pid.parse::<u32>().ok()?;
    (pid != 0).then_some(pid)
}

fn session_process_id(path: &Path, name: &str) -> Option<u32> {
    if let Some(owner) = name
        .strip_prefix("session-")
        .and_then(|name| name.strip_suffix(".json"))
    {
        if let Some((pid, nonce)) = owner.split_once('-') {
            let pid = pid.parse::<u32>().ok()?;
            if pid == 0 || nonce.is_empty() {
                return None;
            }
            if let Ok(contents) = fs::read(path) {
                if let Ok(record) = serde_json::from_slice::<SessionRecord>(&contents) {
                    if record.pid != pid {
                        return None;
                    }
                }
            }
            return Some(pid);
        }
    }
    fs::read(path)
        .ok()
        .and_then(|contents| serde_json::from_slice::<SessionRecord>(&contents).ok())
        .map(|record| record.pid)
}

fn process_is_alive(pid: u32) -> bool {
    if pid == 0 {
        return true;
    }
    #[cfg(unix)]
    {
        if pid > i32::MAX as u32 {
            return false;
        }
        unsafe extern "C" {
            fn kill(pid: i32, signal: i32) -> i32;
        }
        let result = unsafe { kill(pid as i32, 0) };
        result == 0 || io::Error::last_os_error().raw_os_error() != Some(3)
    }
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::{CloseHandle, STILL_ACTIVE};
        use windows::Win32::System::Threading::{
            GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
        };
        let process = match unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) } {
            Ok(process) => process,
            Err(error) => return error.code().0 != 0x8007_0057u32 as i32,
        };
        let mut status = 0;
        let alive = unsafe { GetExitCodeProcess(process, &mut status) }.is_err()
            || status == STILL_ACTIVE.0 as u32;
        unsafe {
            let _ = CloseHandle(process);
        }
        alive
    }
    #[cfg(not(any(unix, windows)))]
    {
        true
    }
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> io::Result<()> {
    File::open(path)?.sync_all()
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> io::Result<()> {
    Ok(())
}

struct CollectorLock(PathBuf);

impl Drop for CollectorLock {
    fn drop(&mut self) {
        if fs::remove_file(&self.0).is_ok() {
            if let Some(parent) = self.0.parent() {
                let _ = sync_directory(parent);
            }
        }
    }
}

fn collect_candidates(project: &Path, ignored_session: Option<&Path>) {
    let sessions = project.join(SESSIONS);
    let Ok(metadata) = fs::symlink_metadata(&sessions) else {
        return;
    };
    if !metadata.file_type().is_dir() {
        return;
    }
    if !matches!(collector_is_active(&sessions), Ok(false)) {
        return;
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let lock_path = sessions.join(format!(
        "{COLLECTING_PREFIX}{}-{now}-{}",
        std::process::id(),
        NEXT_SESSION.fetch_add(1, Ordering::Relaxed)
    ));
    let Ok(lock_file) = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock_path)
    else {
        return;
    };
    let _lock = CollectorLock(lock_path.clone());
    if lock_file.sync_all().is_err() || sync_directory(&sessions).is_err() {
        return;
    }
    drop(lock_file);
    let Ok(entries) = fs::read_dir(&sessions) else {
        return;
    };
    for entry in entries {
        let Ok(entry) = entry else {
            return;
        };
        let path = entry.path();
        if path == lock_path || ignored_session.is_some_and(|ignored| ignored == path) {
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            return;
        };
        if name == LEGACY_COLLECTING || name.starts_with(COLLECTING_PREFIX) {
            continue;
        }
        if !name.starts_with("session-") || !name.ends_with(".json") {
            return;
        }
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            return;
        };
        if !metadata.file_type().is_file() {
            return;
        }
        let Some(pid) = session_process_id(&path, name) else {
            return;
        };
        if process_is_alive(pid) {
            return;
        }
        if fs::remove_file(path).is_err() {
            return;
        }
    }
    let state_path = project::editor_state_path(project);
    let Ok(state_metadata) = fs::symlink_metadata(&state_path) else {
        return;
    };
    if !state_metadata.file_type().is_file() {
        return;
    }
    let Ok(contents) = fs::read(&state_path) else {
        return;
    };
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(&contents) else {
        return;
    };
    let folder = project.join(project::MUSIC_FOLDER);
    let Ok(folder_metadata) = fs::symlink_metadata(&folder) else {
        return;
    };
    if !folder_metadata.file_type().is_dir() {
        return;
    }
    if !valid_saved_state(&value, &folder) {
        return;
    }
    let Ok(state) =
        serde_json::from_value::<crate::windows::video_editor::model::VideoEditorState>(value)
    else {
        return;
    };
    let Ok(records_metadata) = fs::symlink_metadata(project.join(FOLDER)) else {
        return;
    };
    if !records_metadata.file_type().is_dir() {
        return;
    }
    let retained: HashSet<String> = state
        .music_tracks
        .iter()
        .filter(|track| track.source == "music")
        .map(|track| track.file_name.clone())
        .collect();
    let Ok(records) = fs::read_dir(project.join(FOLDER)) else {
        return;
    };
    for entry in records {
        let Ok(entry) = entry else {
            return;
        };
        let marker = entry.path();
        let Ok(metadata) = fs::symlink_metadata(&marker) else {
            continue;
        };
        if !metadata.file_type().is_file() {
            continue;
        }
        let Ok(contents) = fs::read(&marker) else {
            continue;
        };
        let Ok(candidate) = serde_json::from_slice::<Candidate>(&contents) else {
            continue;
        };
        let expected_name = marker_name(&candidate.file_name);
        if marker.file_name() != Some(expected_name.as_ref()) {
            continue;
        }
        if retained.contains(&candidate.file_name) {
            continue;
        }
        let Some(source) = music::file_path(&folder, &candidate.file_name) else {
            continue;
        };
        match fs::symlink_metadata(&source) {
            Ok(metadata) if metadata.file_type().is_file() => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                if fs::remove_file(&marker).is_ok() {
                    let _ = sync_directory(project.join(FOLDER).as_path());
                }
                continue;
            }
            _ => continue,
        };
        if state.music_tracks.iter().any(|track| {
            if track.source != "music" {
                return false;
            }
            let Some(reference) = music::file_path(&folder, &track.file_name) else {
                return false;
            };
            same_file_path(&source, &reference)
        }) {
            continue;
        }
        if fs::remove_file(&source).is_err() {
            continue;
        }
        if fs::remove_file(marker).is_ok() {
            let _ = sync_directory(&folder);
            let _ = sync_directory(project.join(FOLDER).as_path());
        }
    }
}

fn same_file_path(left: &Path, right: &Path) -> bool {
    crate::util::file_identity::paths_match(left, right)
}

fn valid_saved_state(value: &serde_json::Value, music_folder: &Path) -> bool {
    let Some(state) = value.as_object() else {
        return false;
    };
    if !matches!(
        state.get("version").and_then(serde_json::Value::as_u64),
        Some(1 | 2)
    ) || state
        .get("savedAt")
        .and_then(serde_json::Value::as_str)
        .is_none()
        || !state
            .get("segments")
            .is_some_and(serde_json::Value::is_array)
        || !state
            .get("cursorStyle")
            .is_some_and(serde_json::Value::is_object)
        || !state
            .get("cameraStyle")
            .is_some_and(serde_json::Value::is_object)
        || !state
            .get("keyboardStyle")
            .is_some_and(serde_json::Value::is_object)
        || !state
            .get("zoomSegments")
            .is_some_and(serde_json::Value::is_array)
        || !state
            .get("zoomSettings")
            .is_some_and(serde_json::Value::is_object)
        || !state.get("ui").is_some_and(serde_json::Value::is_object)
    {
        return false;
    }
    let Some(tracks) = state.get("musicTracks") else {
        return true;
    };
    let Some(tracks) = tracks.as_array() else {
        return false;
    };
    tracks.iter().all(|track| {
        let Some(track) = track.as_object() else {
            return false;
        };
        let Some(source) = track.get("source").and_then(serde_json::Value::as_str) else {
            return false;
        };
        if source != "music" {
            return source == "mic" || source == "system";
        }
        let Some(file_name) = track.get("fileName").and_then(serde_json::Value::as_str) else {
            return false;
        };
        music::file_path(music_folder, file_name).is_some()
    })
}

pub fn reserved(folder: &Path, file_name: &str) -> io::Result<bool> {
    let parent = folder
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "music folder has no parent"))?;
    let records = parent.join(FOLDER);
    match fs::symlink_metadata(&records) {
        Ok(metadata) if !metadata.file_type().is_dir() => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid retention folder",
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    }
    match fs::symlink_metadata(records.join(marker_name(file_name))) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

pub fn record(project_path: &Path, file_name: &str) -> io::Result<()> {
    let folder = project::music_folder(project_path)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "not a recording project"))?;
    let source = music::file_path(&folder, file_name)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid music path"))?;
    if !fs::symlink_metadata(source)?.file_type().is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "not a music file",
        ));
    }
    let parent = folder
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "music folder has no parent"))?;
    let records = parent.join(FOLDER);
    match fs::create_dir(&records) {
        Ok(()) => sync_directory(parent)?,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error),
    }
    if !fs::symlink_metadata(&records)?.file_type().is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid retention folder",
        ));
    }
    let path = records.join(marker_name(file_name));
    let contents = serde_json::to_vec(&Candidate {
        file_name: file_name.to_string(),
    })?;
    if let Ok(metadata) = fs::symlink_metadata(&path) {
        if !metadata.file_type().is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid music retention record",
            ));
        }
        if let Ok(existing) = fs::read(&path).and_then(|contents| {
            serde_json::from_slice::<Candidate>(&contents)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
        }) {
            if existing.file_name != file_name {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "music retention marker name collision",
                ));
            }
            return sync_directory(&records);
        }
    }
    crate::util::atomic_file::write(&path, &contents)
}

pub fn record_removed(project_path: &Path, before: &[MusicTrack], after: &[MusicTrack]) {
    let retained: HashSet<&str> = after
        .iter()
        .filter(|track| track.source == "music")
        .map(|track| track.file_name.as_str())
        .collect();
    let mut recorded = HashSet::new();
    for track in before.iter().filter(|track| track.source == "music") {
        if retained.contains(track.file_name.as_str()) || !recorded.insert(&track.file_name) {
            continue;
        }
        if let Err(error) = record(project_path, &track.file_name) {
            eprintln!("[video-editor] could not record retained music: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::windows::video_editor::model::{MusicTrack, VideoEditorState};

    fn save_tracks(project: &Path, tracks: Vec<MusicTrack>) {
        fs::create_dir_all(project.join("music")).unwrap();
        let state = VideoEditorState {
            music_tracks: tracks,
            ..Default::default()
        };
        crate::windows::video_editor::model::save_state(project, &state);
    }

    fn track(name: &str) -> MusicTrack {
        MusicTrack {
            id: format!("music-{name}"),
            group_id: format!("music-{name}"),
            file_name: name.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn records_only_files_no_longer_referenced() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("take.poratake");
        let folder = project.join("music");
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("shared.wav"), b"shared").unwrap();
        fs::write(folder.join("removed.wav"), b"removed").unwrap();
        let shared = MusicTrack {
            file_name: "shared.wav".into(),
            ..Default::default()
        };
        let removed = MusicTrack {
            file_name: "removed.wav".into(),
            ..Default::default()
        };
        record_removed(&project, &[shared.clone(), removed], &[shared]);
        let records = fs::read_dir(project.join(FOLDER))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(
            records[0].file_name(),
            "d8fd9d421e70915368b8ec0f69334c92576e067aa4336e4e9537feeaa55862c0.json"
        );
        let candidate: Candidate =
            serde_json::from_slice(&fs::read(records[0].path()).unwrap()).unwrap();
        assert_eq!(candidate.file_name, "removed.wav");
        assert_eq!(fs::read(folder.join("removed.wav")).unwrap(), b"removed");
    }

    #[test]
    fn repairs_an_interrupted_candidate_record() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("take.poratake");
        fs::create_dir_all(project.join("music")).unwrap();
        fs::write(project.join("music/song.wav"), b"song").unwrap();
        let marker = project.join(FOLDER).join(marker_name("song.wav"));
        fs::create_dir(project.join(FOLDER)).unwrap();
        fs::write(&marker, b"{").unwrap();

        record(&project, "song.wav").unwrap();

        let candidate: Candidate = serde_json::from_slice(&fs::read(marker).unwrap()).unwrap();
        assert_eq!(candidate.file_name, "song.wav");
    }

    #[test]
    fn rejects_paths_outside_the_music_folder() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("take.poratake");
        fs::create_dir_all(project.join("music")).unwrap();
        fs::write(dir.path().join("outside.wav"), b"outside").unwrap();
        assert!(record(&project, "../../outside.wav").is_err());
        assert!(!project.join(FOLDER).exists());
        assert_eq!(
            fs::read(dir.path().join("outside.wav")).unwrap(),
            b"outside"
        );
    }

    #[test]
    fn an_existing_record_reserves_the_name_even_if_the_file_is_missing() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("take.poratake");
        let folder = project.join("music");
        fs::create_dir_all(&folder).unwrap();
        let file = folder.join("song.wav");
        fs::write(&file, b"song").unwrap();
        record(&project, "song.wav").unwrap();
        fs::remove_file(file).unwrap();
        assert_eq!(
            music::unique_file_name(&folder, "song.wav").unwrap(),
            "song (1).wav"
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_retention_and_music_folders() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("take.poratake");
        let outside = dir.path().join("outside");
        fs::create_dir_all(project.join("music")).unwrap();
        fs::create_dir(&outside).unwrap();
        fs::write(project.join("music/song.wav"), b"song").unwrap();
        std::os::unix::fs::symlink(&outside, project.join(FOLDER)).unwrap();
        assert!(record(&project, "song.wav").is_err());
        assert_eq!(fs::read_dir(&outside).unwrap().count(), 0);
        fs::remove_file(project.join(FOLDER)).unwrap();
        fs::remove_dir_all(project.join("music")).unwrap();
        std::os::unix::fs::symlink(&outside, project.join("music")).unwrap();
        fs::write(outside.join("song.wav"), b"song").unwrap();
        assert!(record(&project, "song.wav").is_err());
        assert!(!project.join(FOLDER).exists());
    }

    #[test]
    fn an_open_editor_and_export_lease_keep_undo_music_until_the_last_close() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("take.poratake");
        let folder = project.join("music");
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("song.wav"), b"song").unwrap();
        save_tracks(&project, vec![track("song.wav")]);
        let editor = MusicRetentionSession::open(&project).unwrap().unwrap();
        let export = editor.lease().unwrap();
        save_tracks(&project, Vec::new());
        record(&project, "song.wav").unwrap();
        let overlapping_editor = MusicRetentionSession::open(&project).unwrap().unwrap();
        assert!(folder.join("song.wav").is_file());
        overlapping_editor.close().unwrap().join().unwrap();
        assert!(folder.join("song.wav").is_file());
        editor.close().unwrap().join().unwrap();
        assert!(folder.join("song.wav").is_file());
        export.close().unwrap().join().unwrap();
        assert!(!folder.join("song.wav").exists());
        assert!(!project.join(FOLDER).join(marker_name("song.wav")).exists());
    }

    #[test]
    fn recovers_stale_collectors_and_interrupted_session_markers() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("take.poratake");
        fs::create_dir_all(project.join("music")).unwrap();
        fs::write(project.join("music/orphan.wav"), b"orphan").unwrap();
        save_tracks(&project, Vec::new());
        record(&project, "orphan.wav").unwrap();
        let sessions = project.join(SESSIONS);
        fs::create_dir(&sessions).unwrap();
        let dead_pid = 2_147_483_647;
        let stale_collector = sessions.join(format!("{COLLECTING_PREFIX}{dead_pid}-crashed"));
        let interrupted_session = sessions.join(format!("session-{dead_pid}-crashed.json"));
        fs::write(&stale_collector, b"").unwrap();
        fs::write(&interrupted_session, b"").unwrap();

        let session = MusicRetentionSession::open(&project).unwrap().unwrap();

        assert!(!stale_collector.exists());
        assert!(!interrupted_session.exists());
        assert!(!project.join("music/orphan.wav").exists());
        session.close().unwrap().join().unwrap();
    }

    #[test]
    fn refuses_to_open_while_a_pid_qualified_collector_is_active() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("take.poratake");
        fs::create_dir_all(project.join("music")).unwrap();
        let sessions = project.join(SESSIONS);
        fs::create_dir(&sessions).unwrap();
        fs::write(
            sessions.join(format!("{COLLECTING_PREFIX}{}-active", std::process::id())),
            b"",
        )
        .unwrap();

        let error = match MusicRetentionSession::open(&project) {
            Ok(_) => panic!("open accepted an active collector"),
            Err(error) => error,
        };

        assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
    }

    #[test]
    fn refuses_to_open_with_an_ambiguous_legacy_collector_marker() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("take.poratake");
        fs::create_dir_all(project.join("music")).unwrap();
        let sessions = project.join(SESSIONS);
        fs::create_dir(&sessions).unwrap();
        fs::write(sessions.join(LEGACY_COLLECTING), b"").unwrap();

        let error = match MusicRetentionSession::open(&project) {
            Ok(_) => panic!("open accepted an ambiguous legacy collector marker"),
            Err(error) => error,
        };

        assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
    }

    #[cfg(unix)]
    #[test]
    fn opens_a_project_through_a_directory_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("take.poratake");
        fs::create_dir_all(project.join("music")).unwrap();
        let alias = dir.path().join("linked.poratake");
        std::os::unix::fs::symlink(&project, &alias).unwrap();

        let session = MusicRetentionSession::open(&alias).unwrap().unwrap();

        assert!(project.join(SESSIONS).is_dir());
        session.close().unwrap().join().unwrap();
    }

    #[test]
    fn a_saved_music_reference_prevents_collection() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("take.poratake");
        fs::create_dir_all(project.join("music")).unwrap();
        fs::write(project.join("music/song.wav"), b"song").unwrap();
        save_tracks(&project, vec![track("song.wav")]);
        record(&project, "song.wav").unwrap();
        let session = MusicRetentionSession::open(&project).unwrap().unwrap();
        session.close().unwrap().join().unwrap();
        assert!(project.join("music/song.wav").is_file());
        assert!(project.join(FOLDER).join(marker_name("song.wav")).is_file());
    }

    #[test]
    fn invalid_saved_state_prevents_collection() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("take.poratake");
        fs::create_dir_all(project.join("music")).unwrap();
        fs::write(project.join("music/song.wav"), b"song").unwrap();
        fs::write(project.join(project::EDITOR_STATE), b"{").unwrap();
        record(&project, "song.wav").unwrap();
        let session = MusicRetentionSession::open(&project).unwrap().unwrap();
        session.close().unwrap().join().unwrap();
        assert!(project.join("music/song.wav").is_file());
    }

    #[test]
    fn incomplete_saved_state_prevents_collection() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("take.poratake");
        fs::create_dir_all(project.join("music")).unwrap();
        fs::write(project.join("music/song.wav"), b"song").unwrap();
        fs::write(project.join(project::EDITOR_STATE), b"{}").unwrap();
        record(&project, "song.wav").unwrap();
        let session = MusicRetentionSession::open(&project).unwrap().unwrap();
        session.close().unwrap().join().unwrap();
        assert!(project.join("music/song.wav").is_file());
    }

    #[cfg(any(target_os = "macos", windows))]
    #[test]
    fn preserves_a_saved_filename_that_aliases_the_candidate_on_this_filesystem() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("take.poratake");
        fs::create_dir_all(project.join("music")).unwrap();
        fs::write(project.join("music/song.wav"), b"song").unwrap();
        save_tracks(&project, vec![track("SONG.WAV")]);
        record(&project, "song.wav").unwrap();
        let session = MusicRetentionSession::open(&project).unwrap().unwrap();
        session.close().unwrap().join().unwrap();
        assert!(project.join("music/song.wav").is_file());
    }

    #[test]
    fn a_stale_crashed_session_is_recovered_on_the_next_open() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("take.poratake");
        fs::create_dir_all(project.join("music")).unwrap();
        fs::write(project.join("music/song.wav"), b"song").unwrap();
        save_tracks(&project, Vec::new());
        record(&project, "song.wav").unwrap();
        let sessions = project.join(SESSIONS);
        fs::create_dir(&sessions).unwrap();
        fs::write(
            sessions.join("session-crashed.json"),
            r#"{"pid":2147483647}"#,
        )
        .unwrap();
        let session = MusicRetentionSession::open(&project).unwrap().unwrap();
        assert!(!sessions.join("session-crashed.json").exists());
        assert!(!project.join("music/song.wav").exists());
        session.close().unwrap().join().unwrap();
    }
}
