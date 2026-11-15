use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::store::config_dir;

static HISTORY_MUTATION_LOCK: parking_lot::Mutex<()> = parking_lot::const_mutex(());

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HistoryItemType {
    #[default]
    Screenshot,
    Video,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryItem {
    pub id: String,
    pub timestamp: i64,
    pub original_path: String,
    #[serde(rename = "type", default)]
    pub r#type: HistoryItemType,
    #[serde(default)]
    pub editor_state: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeleteOutcome {
    Deleted,
    DeletedWithBackupWarning(String),
    SourceDeletionFailed,
    HistoryPersistenceFailed(String),
    HistoryUnavailable,
    NotFound,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpdatePathOutcome {
    Updated,
    UpdatedWithBackupWarning(String),
    Untracked,
    HistoryUnavailable(String),
    PersistenceFailed(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ClearOutcome {
    pub retained: Vec<HistoryItem>,
    pub source_deletion_failures: usize,
    pub history_updated: bool,
    pub history_error: Option<String>,
    pub backup_warning: Option<String>,
}

pub fn history_file_path() -> std::path::PathBuf {
    config_dir().join("history.json")
}

pub fn load_history() -> Vec<HistoryItem> {
    load_history_from_locked(&history_file_path())
}

fn load_history_from_locked(path: &Path) -> Vec<HistoryItem> {
    let _lock = HISTORY_MUTATION_LOCK.lock();
    match load_history_from(path) {
        Ok(items) => items,
        Err(error) => {
            eprintln!(
                "[history] failed to load {}: {error}",
                history_file_path().display()
            );
            Vec::new()
        }
    }
}

fn load_history_from(path: &Path) -> io::Result<Vec<HistoryItem>> {
    let path = resolve_history_target(path)?;
    let content = match fs::read_to_string(&path) {
        Ok(content) => content,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };

    serde_json::from_str(&content)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

#[cfg(feature = "parity")]
pub fn save_history(items: &[HistoryItem]) -> bool {
    let _lock = HISTORY_MUTATION_LOCK.lock();
    match save_history_to(&history_file_path(), items) {
        Ok(warning) => {
            log_backup_warning(warning.as_deref());
            true
        }
        Err(error) => {
            eprintln!(
                "[history] failed to save {}: {error}",
                history_file_path().display()
            );
            false
        }
    }
}

fn save_history_to(path: &Path, items: &[HistoryItem]) -> io::Result<Option<String>> {
    save_history_to_with(path, items, write_history_file)
}

fn save_history_to_with(
    path: &Path,
    items: &[HistoryItem],
    write_content: impl FnOnce(&Path, &[u8]) -> io::Result<Option<String>>,
) -> io::Result<Option<String>> {
    let path = resolve_history_target(path)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_vec_pretty(items)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    write_content(&path, &content)
}

fn resolve_history_target(path: &Path) -> io::Result<PathBuf> {
    match fs::symlink_metadata(path) {
        Ok(_) => fs::canonicalize(path),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(path.to_path_buf()),
        Err(error) => Err(error),
    }
}

fn log_backup_warning(warning: Option<&str>) {
    if let Some(path) = warning {
        eprintln!("[history] recovery copy remains at {path}");
    }
}

fn write_history_file(path: &Path, content: &[u8]) -> io::Result<Option<String>> {
    #[cfg(windows)]
    {
        crate::editor::screenshot_save::write(path, content)
    }
    #[cfg(unix)]
    {
        crate::editor::screenshot_save_unix::write(path, content)
    }
}

pub fn add_item(item: HistoryItem, max_items: usize, enabled: bool) -> bool {
    if !enabled {
        return true;
    }
    let _lock = HISTORY_MUTATION_LOCK.lock();
    let mut items = match load_history_from(&history_file_path()) {
        Ok(items) => items,
        Err(error) => {
            eprintln!("[history] failed to load before adding item: {error}");
            return false;
        }
    };
    items.insert(0, item);
    while items.len() > max_items {
        let Some(removed) = items.pop() else {
            break;
        };
        if cleanup_item(&removed).is_err() {
            items.push(removed);
            break;
        }
    }
    match save_history_to(&history_file_path(), &items) {
        Ok(warning) => {
            log_backup_warning(warning.as_deref());
            true
        }
        Err(error) => {
            eprintln!("[history] failed to save after adding item: {error}");
            false
        }
    }
}

pub fn delete_item(id: &str) -> DeleteOutcome {
    let _lock = HISTORY_MUTATION_LOCK.lock();
    delete_item_with(&history_file_path(), id, save_history_to)
}

fn delete_item_with(
    path: &Path,
    id: &str,
    save: impl FnOnce(&Path, &[HistoryItem]) -> io::Result<Option<String>>,
) -> DeleteOutcome {
    let mut items = match load_history_from(path) {
        Ok(items) => items,
        Err(error) => {
            eprintln!("[history] failed to load before deleting item: {error}");
            return DeleteOutcome::HistoryUnavailable;
        }
    };
    let Some(index) = items.iter().position(|item| item.id == id) else {
        return DeleteOutcome::NotFound;
    };
    if cleanup_item(&items[index]).is_err() {
        return DeleteOutcome::SourceDeletionFailed;
    }
    items.remove(index);
    match save(path, &items) {
        Ok(Some(warning)) => DeleteOutcome::DeletedWithBackupWarning(warning),
        Ok(None) => DeleteOutcome::Deleted,
        Err(error) => {
            eprintln!("[history] failed to persist index after deleting item: {error}");
            DeleteOutcome::HistoryPersistenceFailed(error.to_string())
        }
    }
}

pub fn delete_path(path: &Path, kind: HistoryItemType) -> DeleteOutcome {
    let _lock = HISTORY_MUTATION_LOCK.lock();
    delete_path_with(&history_file_path(), path, kind, save_history_to)
}

fn delete_path_with(
    history_path: &Path,
    path: &Path,
    kind: HistoryItemType,
    save: impl FnOnce(&Path, &[HistoryItem]) -> io::Result<Option<String>>,
) -> DeleteOutcome {
    let path_string = path.to_string_lossy();
    let mut items = match load_history_from(history_path) {
        Ok(items) => items,
        Err(error) => {
            eprintln!("[history] failed to load before deleting path: {error}");
            return DeleteOutcome::HistoryUnavailable;
        }
    };
    if let Some(index) = items
        .iter()
        .position(|item| item.original_path == path_string)
    {
        if cleanup_item(&items[index]).is_err() {
            return DeleteOutcome::SourceDeletionFailed;
        }
        items.remove(index);
        return match save(history_path, &items) {
            Ok(Some(warning)) => DeleteOutcome::DeletedWithBackupWarning(warning),
            Ok(None) => DeleteOutcome::Deleted,
            Err(error) => {
                eprintln!("[history] failed to persist index after deleting path: {error}");
                DeleteOutcome::HistoryPersistenceFailed(error.to_string())
            }
        };
    }
    if cleanup_path(path, kind).is_ok() {
        DeleteOutcome::Deleted
    } else {
        DeleteOutcome::SourceDeletionFailed
    }
}

pub fn clear_history() -> io::Result<ClearOutcome> {
    let _lock = HISTORY_MUTATION_LOCK.lock();
    clear_history_with(&history_file_path(), save_history_to)
}

fn clear_history_with(
    path: &Path,
    save: impl FnOnce(&Path, &[HistoryItem]) -> io::Result<Option<String>>,
) -> io::Result<ClearOutcome> {
    let items = load_history_from(path)?;
    let mut retained = Vec::new();
    for item in items {
        if cleanup_item(&item).is_err() {
            retained.push(item);
        }
    }
    let source_deletion_failures = retained.len();
    let (history_updated, history_error, backup_warning) = match save(path, &retained) {
        Ok(warning) => (true, None, warning),
        Err(error) => {
            eprintln!("[history] failed to persist index after clearing: {error}");
            (false, Some(error.to_string()), None)
        }
    };
    Ok(ClearOutcome {
        retained,
        source_deletion_failures,
        history_updated,
        history_error,
        backup_warning,
    })
}

pub fn editor_state_for_path(path: &Path) -> Option<Value> {
    let path = path.to_string_lossy();
    load_history()
        .into_iter()
        .find(|item| item.original_path == path)
        .and_then(|item| item.editor_state)
}

pub fn update_editor_state_by_path(path: &Path, state: Value) -> bool {
    let _lock = HISTORY_MUTATION_LOCK.lock();
    let path = path.to_string_lossy();
    let history_path = history_file_path();
    let mut items = match load_history_from(&history_path) {
        Ok(items) => items,
        Err(error) => {
            eprintln!("[history] failed to load before updating editor state: {error}");
            return false;
        }
    };
    let Some(item) = items.iter_mut().find(|item| item.original_path == path) else {
        return false;
    };
    item.editor_state = Some(state);
    match save_history_to(&history_path, &items) {
        Ok(warning) => {
            log_backup_warning(warning.as_deref());
            true
        }
        Err(error) => {
            eprintln!("[history] failed to persist editor state: {error}");
            false
        }
    }
}

pub fn update_item_path(old_path: &Path, new_path: &Path) -> UpdatePathOutcome {
    let _lock = HISTORY_MUTATION_LOCK.lock();
    update_item_path_with(&history_file_path(), old_path, new_path, save_history_to)
}

fn update_item_path_with(
    history_path: &Path,
    old_path: &Path,
    new_path: &Path,
    save: impl FnOnce(&Path, &[HistoryItem]) -> io::Result<Option<String>>,
) -> UpdatePathOutcome {
    let old_path = old_path.to_string_lossy();
    let mut items = match load_history_from(history_path) {
        Ok(items) => items,
        Err(error) => {
            eprintln!("[history] failed to load before updating path: {error}");
            return UpdatePathOutcome::HistoryUnavailable(error.to_string());
        }
    };
    let Some(item) = items.iter_mut().find(|item| item.original_path == old_path) else {
        return UpdatePathOutcome::Untracked;
    };
    item.original_path = new_path.to_string_lossy().to_string();
    match save(history_path, &items) {
        Ok(Some(warning)) => {
            log_backup_warning(Some(&warning));
            UpdatePathOutcome::UpdatedWithBackupWarning(warning)
        }
        Ok(None) => UpdatePathOutcome::Updated,
        Err(error) => {
            eprintln!("[history] failed to persist updated path: {error}");
            UpdatePathOutcome::PersistenceFailed(error.to_string())
        }
    }
}

fn cleanup_item(item: &HistoryItem) -> std::io::Result<()> {
    cleanup_path(Path::new(&item.original_path), item.r#type)
}

fn cleanup_path(path: &Path, kind: HistoryItemType) -> std::io::Result<()> {
    if kind == HistoryItemType::Screenshot {
        remove_file_if_exists(path)?;
        crate::thumbnails::remove(path);
        return Ok(());
    }
    if let Some(project) = crate::video::project::project_folder(path) {
        if project.exists() {
            std::fs::remove_dir_all(project)?;
        }
        crate::thumbnails::remove(path);
        return Ok(());
    }
    for asset in [
        crate::video::project::recording_video_path(path),
        crate::video::project::system_audio_path(path),
        crate::video::project::mic_audio_path(path),
        crate::video::project::cursor_path(path),
        crate::video::project::camera_video_path(path),
        crate::video::project::camera_meta_path(path),
        crate::video::project::keys_path(path),
        crate::video::project::editor_state_path(path),
        crate::video::project::subtitle_path(path),
    ] {
        remove_file_if_exists(&asset)?;
    }
    crate::thumbnails::remove(path);
    Ok(())
}

fn remove_file_if_exists(path: &Path) -> std::io::Result<()> {
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, path: &Path) -> HistoryItem {
        HistoryItem {
            id: id.into(),
            timestamp: 1,
            original_path: path.to_string_lossy().into_owned(),
            r#type: HistoryItemType::Screenshot,
            editor_state: None,
            duration: None,
        }
    }

    #[test]
    fn first_history_write_creates_a_missing_profile_directory() {
        let directory = tempfile::tempdir().unwrap();
        let history_path = directory.path().join("new-profile").join("history.json");
        let entries = vec![item("first", Path::new("first.png"))];

        save_history_to(&history_path, &entries).unwrap();

        assert_eq!(load_history_from(&history_path).unwrap(), entries);
    }

    #[test]
    fn public_history_reader_waits_for_in_process_mutations() {
        use std::sync::mpsc;
        use std::time::Duration;

        let directory = tempfile::tempdir().unwrap();
        let history_path = directory.path().join("history.json");
        let entries = vec![item("stable", Path::new("stable.png"))];
        save_history_to(&history_path, &entries).unwrap();
        let guard = HISTORY_MUTATION_LOCK.lock();
        let (started_tx, started_rx) = mpsc::channel();
        let (read_tx, read_rx) = mpsc::channel();
        let reader_path = history_path.clone();
        let reader = std::thread::spawn(move || {
            started_tx.send(()).unwrap();
            read_tx
                .send(load_history_from_locked(&reader_path))
                .unwrap();
        });

        started_rx.recv().unwrap();
        assert!(read_rx.recv_timeout(Duration::from_millis(50)).is_err());
        drop(guard);
        assert_eq!(
            read_rx.recv_timeout(Duration::from_secs(1)).unwrap(),
            entries
        );
        reader.join().unwrap();
    }

    #[test]
    fn screenshot_cleanup_removes_the_original_file() {
        let directory = tempfile::tempdir().expect("temp directory");
        let screenshot = directory.path().join("capture.png");
        std::fs::write(&screenshot, b"image").expect("write screenshot");

        cleanup_path(&screenshot, HistoryItemType::Screenshot).expect("clean screenshot");

        assert!(!screenshot.exists());
    }

    #[test]
    fn video_cleanup_removes_the_entire_project() {
        let directory = tempfile::tempdir().expect("temp directory");
        let project = directory.path().join("Recording.poratake");
        std::fs::create_dir(&project).expect("create project");
        std::fs::write(project.join(crate::video::project::RECORDING), b"video")
            .expect("write recording");

        cleanup_path(&project, HistoryItemType::Video).expect("clean project");

        assert!(!project.exists());
    }

    #[test]
    fn loose_video_cleanup_removes_every_sidecar() {
        let directory = tempfile::tempdir().expect("temp directory");
        let video = directory.path().join("recording.mp4");
        let assets = [
            video.clone(),
            crate::video::project::system_audio_path(&video),
            crate::video::project::mic_audio_path(&video),
            crate::video::project::cursor_path(&video),
            crate::video::project::camera_video_path(&video),
            crate::video::project::camera_meta_path(&video),
            crate::video::project::keys_path(&video),
            crate::video::project::editor_state_path(&video),
            crate::video::project::subtitle_path(&video),
        ];
        for asset in &assets {
            std::fs::write(asset, b"asset").expect("write asset");
        }

        cleanup_path(&video, HistoryItemType::Video).expect("clean recording");

        assert!(assets.iter().all(|asset| !asset.exists()));
    }

    #[test]
    fn failed_history_write_preserves_original_index() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("history.json");
        let previous = vec![item("old", Path::new("old.png"))];
        save_history_to(&path, &previous).unwrap();
        let original = fs::read(&path).unwrap();
        let next = vec![item("new", Path::new("new.png"))];

        let result = save_history_to_with(&path, &next, |_, _| {
            Err(io::Error::other("injected history write failure"))
        });

        assert!(result.is_err());
        assert_eq!(fs::read(&path).unwrap(), original);
        assert_eq!(load_history_from(&path).unwrap(), previous);
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn malformed_history_does_not_delete_sources_or_overwrite_index() {
        let directory = tempfile::tempdir().unwrap();
        let history_path = directory.path().join("history.json");
        let source = directory.path().join("capture.png");
        fs::write(&source, b"image").unwrap();
        let malformed = b"{malformed history";
        fs::write(&history_path, malformed).unwrap();

        let outcome = delete_path_with(
            &history_path,
            &source,
            HistoryItemType::Screenshot,
            save_history_to,
        );

        assert_eq!(outcome, DeleteOutcome::HistoryUnavailable);
        assert!(source.exists());
        assert_eq!(fs::read(&history_path).unwrap(), malformed);
    }

    #[test]
    fn deleted_source_with_failed_history_save_keeps_original_index() {
        let directory = tempfile::tempdir().unwrap();
        let history_path = directory.path().join("history.json");
        let source = directory.path().join("capture.png");
        fs::write(&source, b"image").unwrap();
        let previous = vec![item("capture", &source)];
        save_history_to(&history_path, &previous).unwrap();
        let original = fs::read(&history_path).unwrap();

        let outcome = delete_item_with(&history_path, "capture", |_, _| {
            Err(io::Error::other("injected history write failure"))
        });

        assert_eq!(
            outcome,
            DeleteOutcome::HistoryPersistenceFailed("injected history write failure".into())
        );
        assert!(!source.exists());
        assert_eq!(fs::read(&history_path).unwrap(), original);
        assert_eq!(load_history_from(&history_path).unwrap(), previous);
    }

    #[test]
    fn deletion_reports_retained_history_recovery_copy() {
        let directory = tempfile::tempdir().unwrap();
        let history_path = directory.path().join("history.json");
        let source = directory.path().join("capture.png");
        fs::write(&source, b"image").unwrap();
        let previous = vec![item("capture", &source)];
        save_history_to(&history_path, &previous).unwrap();

        let outcome = delete_item_with(&history_path, "capture", |_, _| {
            Ok(Some("recovery-copy.json".into()))
        });

        assert_eq!(
            outcome,
            DeleteOutcome::DeletedWithBackupWarning("recovery-copy.json".into())
        );
        assert!(!source.exists());
    }

    #[test]
    fn clear_reports_source_deletion_failure_and_persists_retained_items() {
        let directory = tempfile::tempdir().unwrap();
        let history_path = directory.path().join("history.json");
        let source_directory = directory.path().join("not-a-file.png");
        fs::create_dir(&source_directory).unwrap();
        let previous = vec![item("directory", &source_directory)];
        save_history_to(&history_path, &previous).unwrap();

        let outcome = clear_history_with(&history_path, save_history_to).unwrap();

        assert_eq!(outcome.retained, previous);
        assert_eq!(outcome.source_deletion_failures, 1);
        assert!(outcome.history_updated);
        assert!(outcome.history_error.is_none());
        assert!(outcome.backup_warning.is_none());
        assert!(source_directory.exists());
        assert_eq!(load_history_from(&history_path).unwrap(), outcome.retained);
    }

    #[test]
    fn clear_reports_failed_history_persistence_after_source_deletion() {
        let directory = tempfile::tempdir().unwrap();
        let history_path = directory.path().join("history.json");
        let source = directory.path().join("capture.png");
        fs::write(&source, b"image").unwrap();
        let previous = vec![item("capture", &source)];
        save_history_to(&history_path, &previous).unwrap();
        let original = fs::read(&history_path).unwrap();

        let outcome = clear_history_with(&history_path, |_, _| {
            Err(io::Error::other("injected history write failure"))
        })
        .unwrap();

        assert!(outcome.retained.is_empty());
        assert_eq!(outcome.source_deletion_failures, 0);
        assert!(!outcome.history_updated);
        assert_eq!(
            outcome.history_error.as_deref(),
            Some("injected history write failure")
        );
        assert!(!source.exists());
        assert_eq!(fs::read(&history_path).unwrap(), original);
        assert_eq!(load_history_from(&history_path).unwrap(), previous);
    }

    #[test]
    fn clear_reports_retained_history_recovery_copy() {
        let directory = tempfile::tempdir().unwrap();
        let history_path = directory.path().join("history.json");
        let source = directory.path().join("capture.png");
        fs::write(&source, b"image").unwrap();
        let previous = vec![item("capture", &source)];
        save_history_to(&history_path, &previous).unwrap();

        let outcome =
            clear_history_with(&history_path, |_, _| Ok(Some("recovery-copy.json".into())))
                .unwrap();

        assert!(outcome.history_updated);
        assert_eq!(outcome.history_error, None);
        assert_eq!(
            outcome.backup_warning.as_deref(),
            Some("recovery-copy.json")
        );
        assert!(!source.exists());
    }

    #[test]
    fn tracked_rename_write_failure_keeps_old_history_path() {
        let directory = tempfile::tempdir().unwrap();
        let history_path = directory.path().join("history.json");
        let old_path = directory.path().join("Old.poratake");
        let new_path = directory.path().join("New.poratake");
        let previous = vec![item("recording", &old_path)];
        save_history_to(&history_path, &previous).unwrap();
        let original = fs::read(&history_path).unwrap();

        let outcome = update_item_path_with(&history_path, &old_path, &new_path, |_, _| {
            Err(io::Error::other("injected history write failure"))
        });

        assert_eq!(
            outcome,
            UpdatePathOutcome::PersistenceFailed("injected history write failure".into())
        );
        assert_eq!(fs::read(&history_path).unwrap(), original);
        assert_eq!(load_history_from(&history_path).unwrap(), previous);
    }

    #[test]
    fn untracked_rename_does_not_attempt_history_persistence() {
        use std::cell::Cell;

        let directory = tempfile::tempdir().unwrap();
        let history_path = directory.path().join("history.json");
        let old_path = directory.path().join("Untracked.poratake");
        let new_path = directory.path().join("Renamed.poratake");
        let save_called = Cell::new(false);

        let outcome = update_item_path_with(&history_path, &old_path, &new_path, |_, _| {
            save_called.set(true);
            Ok(None)
        });

        assert_eq!(outcome, UpdatePathOutcome::Untracked);
        assert!(!save_called.get());
        assert!(!history_path.exists());
    }

    #[cfg(unix)]
    #[test]
    fn valid_history_symlink_keeps_link_and_updates_its_target() {
        use std::os::unix::fs::{symlink, PermissionsExt};

        let directory = tempfile::tempdir().unwrap();
        let profile = directory.path().join("linked-profile");
        fs::create_dir(&profile).unwrap();
        let target = profile.join("history.json");
        let link = directory.path().join("history.json");
        let previous = vec![item("old", Path::new("old.png"))];
        let next = vec![item("new", Path::new("new.png"))];
        save_history_to(&target, &previous).unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o640)).unwrap();
        symlink(&target, &link).unwrap();

        assert_eq!(load_history_from(&link).unwrap(), previous);
        save_history_to(&link, &next).unwrap();

        assert!(fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(load_history_from(&target).unwrap(), next);
        assert_eq!(
            fs::metadata(&target).unwrap().permissions().mode() & 0o777,
            0o640
        );
    }

    #[cfg(unix)]
    #[test]
    fn dangling_or_looping_history_symlink_fails_before_source_deletion() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("capture.png");
        fs::write(&source, b"image").unwrap();
        let missing_target = directory.path().join("missing-history.json");
        let dangling_link = directory.path().join("dangling-history.json");
        symlink(&missing_target, &dangling_link).unwrap();

        assert!(save_history_to(&dangling_link, &[]).is_err());
        assert_eq!(
            delete_path_with(
                &dangling_link,
                &source,
                HistoryItemType::Screenshot,
                save_history_to,
            ),
            DeleteOutcome::HistoryUnavailable
        );
        assert!(source.exists());
        assert!(dangling_link
            .symlink_metadata()
            .unwrap()
            .file_type()
            .is_symlink());

        let first = directory.path().join("loop-a.json");
        let second = directory.path().join("loop-b.json");
        symlink(&second, &first).unwrap();
        symlink(&first, &second).unwrap();
        assert_eq!(
            delete_path_with(
                &first,
                &source,
                HistoryItemType::Screenshot,
                save_history_to,
            ),
            DeleteOutcome::HistoryUnavailable
        );
        assert!(source.exists());
    }
}
