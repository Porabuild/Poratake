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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AddOutcome {
    Disabled,
    Added {
        retained_entries: Vec<String>,
        backup_warnings: Vec<String>,
        minimum_one_retained: bool,
    },
    HistoryUnavailable(String),
    PersistenceFailed(String),
    CleanupRepairFailed {
        cleanup_failed_paths: Vec<String>,
        error: String,
        backup_warnings: Vec<String>,
        minimum_one_retained: bool,
    },
}

impl AddOutcome {
    pub fn warning(&self, capture_label: &str) -> Option<(String, String)> {
        match self {
            Self::Disabled => None,
            Self::Added {
                retained_entries,
                backup_warnings,
                minimum_one_retained,
            } if retained_entries.is_empty()
                && backup_warnings.is_empty()
                && !minimum_one_retained =>
            {
                None
            }
            Self::Added {
                retained_entries,
                backup_warnings,
                minimum_one_retained,
            } => {
                let mut details = Vec::new();
                if *minimum_one_retained {
                    details.push(
                        "The configured limit was zero, so the new capture was retained".into(),
                    );
                }
                if !retained_entries.is_empty() {
                    details.push(format!(
                        "{} older history item(s) could not be cleaned up and remain indexed: {}",
                        retained_entries.len(),
                        retained_entries.join(", ")
                    ));
                }
                details.extend(
                    backup_warnings
                        .iter()
                        .map(|path| format!("A history recovery copy remains at {path}")),
                );
                Some(("History update warning".into(), details.join(". ")))
            }
            Self::HistoryUnavailable(error) | Self::PersistenceFailed(error) => Some((
                "History update failed".into(),
                format!("The {capture_label} was saved but could not be added to history: {error}"),
            )),
            Self::CleanupRepairFailed {
                cleanup_failed_paths,
                error,
                backup_warnings,
                minimum_one_retained,
            } => {
                let mut details = Vec::new();
                if *minimum_one_retained {
                    details.push(
                        "The configured limit was zero, so the new capture was retained".into(),
                    );
                }
                details.push(format!(
                    "The {capture_label} was saved, but cleanup failed for {} older item(s); their assets may be partially removed and their history entries could not be restored: {}. {error}",
                    cleanup_failed_paths.len(),
                    cleanup_failed_paths.join(", ")
                ));
                details.extend(
                    backup_warnings
                        .iter()
                        .map(|path| format!("A history recovery copy remains at {path}")),
                );
                Some(("History update failed".into(), details.join(". ")))
            }
        }
    }
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

fn comparison_path(path: &Path) -> Option<PathBuf> {
    use std::path::Component;

    let mut current = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().ok()?.join(path)
    };
    let mut suffix = Vec::new();
    loop {
        match fs::canonicalize(&current) {
            Ok(mut resolved) => {
                for component in suffix.iter().rev() {
                    resolved.push(component);
                }
                return Some(resolved);
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                if current
                    .components()
                    .any(|component| component == Component::ParentDir)
                {
                    return None;
                }
                suffix.push(current.file_name()?.to_os_string());
                if !current.pop() {
                    return None;
                }
            }
            Err(_) => return None,
        }
    }
}

fn cleanup_targets(item: &HistoryItem) -> Vec<(PathBuf, bool)> {
    let path = Path::new(&item.original_path);
    if item.r#type == HistoryItemType::Screenshot {
        return vec![(path.to_path_buf(), false)];
    }
    if let Some(project) = crate::video::project::project_folder(path) {
        return vec![(project, true)];
    }
    [
        crate::video::project::recording_video_path(path),
        crate::video::project::system_audio_path(path),
        crate::video::project::mic_audio_path(path),
        crate::video::project::cursor_path(path),
        crate::video::project::camera_video_path(path),
        crate::video::project::camera_meta_path(path),
        crate::video::project::keys_path(path),
        crate::video::project::editor_state_path(path),
        crate::video::project::subtitle_path(path),
    ]
    .into_iter()
    .map(|path| (path, false))
    .collect()
}

fn comparison_cleanup_targets(item: &HistoryItem) -> Result<Vec<(PathBuf, bool)>, ()> {
    cleanup_targets(item)
        .into_iter()
        .map(|(path, recursive)| {
            comparison_path(&path)
                .map(|path| (path, recursive))
                .ok_or(())
        })
        .collect()
}

fn cleanup_scopes_intersect(left: &(PathBuf, bool), right: &(PathBuf, bool)) -> bool {
    match (left.1, right.1) {
        (true, true) => left.0.starts_with(&right.0) || right.0.starts_with(&left.0),
        (true, false) => right.0.starts_with(&left.0),
        (false, true) => left.0.starts_with(&right.0),
        (false, false) => left.0 == right.0,
    }
}

fn cleanup_is_referenced(
    item: &HistoryItem,
    candidates: &[Vec<(PathBuf, bool)>],
) -> Result<bool, ()> {
    let targets = comparison_cleanup_targets(item)?;
    for target in &targets {
        for candidate in candidates.iter().flatten() {
            if cleanup_scopes_intersect(target, candidate) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

pub fn add_item(item: HistoryItem, max_items: usize, enabled: bool) -> AddOutcome {
    if !enabled {
        return AddOutcome::Disabled;
    }
    let _lock = HISTORY_MUTATION_LOCK.lock();
    add_item_with(&history_file_path(), item, max_items, save_history_to)
}

fn add_item_with(
    history_path: &Path,
    item: HistoryItem,
    max_items: usize,
    mut save: impl FnMut(&Path, &[HistoryItem]) -> io::Result<Option<String>>,
) -> AddOutcome {
    let mut items = match load_history_from(history_path) {
        Ok(items) => items,
        Err(error) => {
            eprintln!("[history] failed to load before adding item: {error}");
            return AddOutcome::HistoryUnavailable(error.to_string());
        }
    };
    items.insert(0, item);
    let minimum_one_retained = max_items == 0;
    let mut evicted = Vec::new();
    while items.len() > max_items.max(1) {
        let Some(removed) = items.pop() else {
            break;
        };
        evicted.push(removed);
    }
    let mut backup_warnings = Vec::new();
    match save(history_path, &items) {
        Ok(warning) => backup_warnings.extend(warning),
        Err(error) => {
            eprintln!("[history] failed to save after adding item: {error}");
            return AddOutcome::PersistenceFailed(error.to_string());
        }
    }
    if evicted.is_empty() {
        for warning in &backup_warnings {
            log_backup_warning(Some(warning));
        }
        return AddOutcome::Added {
            retained_entries: Vec::new(),
            backup_warnings,
            minimum_one_retained,
        };
    }
    let mut retained = Vec::new();
    let candidate_cleanup_scopes: Result<Vec<_>, _> =
        items.iter().map(comparison_cleanup_targets).collect();
    for removed in evicted {
        let cleanup_referenced = candidate_cleanup_scopes
            .as_ref()
            .map_err(|_| ())
            .and_then(|candidates| cleanup_is_referenced(&removed, candidates));
        match cleanup_referenced {
            Ok(true) => continue,
            Err(()) => {
                retained.push(removed);
                continue;
            }
            Ok(false) => {}
        }
        if cleanup_item(&removed).is_err() {
            retained.push(removed);
        }
    }
    if retained.is_empty() {
        for warning in &backup_warnings {
            log_backup_warning(Some(warning));
        }
        return AddOutcome::Added {
            retained_entries: Vec::new(),
            backup_warnings,
            minimum_one_retained,
        };
    }
    let retained_entries = retained
        .iter()
        .map(|item| item.original_path.clone())
        .collect::<Vec<_>>();
    retained.reverse();
    items.extend(retained);
    match save(history_path, &items) {
        Ok(warning) => {
            backup_warnings.extend(warning);
            for warning in &backup_warnings {
                log_backup_warning(Some(warning));
            }
            AddOutcome::Added {
                retained_entries,
                backup_warnings,
                minimum_one_retained,
            }
        }
        Err(error) => {
            eprintln!("[history] failed to restore cleanup-retained entries: {error}");
            AddOutcome::CleanupRepairFailed {
                cleanup_failed_paths: retained_entries,
                error: error.to_string(),
                backup_warnings,
                minimum_one_retained,
            }
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

    fn video_item(id: &str, path: &Path) -> HistoryItem {
        HistoryItem {
            r#type: HistoryItemType::Video,
            ..item(id, path)
        }
    }

    fn added_without_warnings(outcome: AddOutcome) {
        assert_eq!(
            outcome,
            AddOutcome::Added {
                retained_entries: Vec::new(),
                backup_warnings: Vec::new(),
                minimum_one_retained: false,
            }
        );
    }

    #[test]
    fn failed_candidate_save_does_not_evict_old_or_new_sources() {
        let directory = tempfile::tempdir().unwrap();
        let history_path = directory.path().join("history.json");
        let old_source = directory.path().join("old.png");
        let new_source = directory.path().join("new.png");
        fs::write(&old_source, b"old").unwrap();
        fs::write(&new_source, b"new").unwrap();
        let previous = vec![item("old", &old_source)];
        save_history_to(&history_path, &previous).unwrap();
        let original = fs::read(&history_path).unwrap();

        let outcome = add_item_with(&history_path, item("new", &new_source), 1, |_, _| {
            Err(io::Error::other("injected candidate write failure"))
        });

        assert_eq!(
            outcome,
            AddOutcome::PersistenceFailed("injected candidate write failure".into())
        );
        assert!(old_source.exists());
        assert!(new_source.exists());
        assert_eq!(fs::read(&history_path).unwrap(), original);
        assert_eq!(load_history_from(&history_path).unwrap(), previous);
    }

    #[test]
    fn add_below_limit_persists_without_eviction() {
        let directory = tempfile::tempdir().unwrap();
        let history_path = directory.path().join("history.json");
        let old_source = directory.path().join("old.png");
        let new_source = directory.path().join("new.png");
        fs::write(&old_source, b"old").unwrap();
        fs::write(&new_source, b"new").unwrap();
        save_history_to(&history_path, &[item("old", &old_source)]).unwrap();

        added_without_warnings(add_item_with(
            &history_path,
            item("new", &new_source),
            2,
            save_history_to,
        ));

        assert!(old_source.exists());
        assert!(new_source.exists());
        assert_eq!(
            load_history_from(&history_path).unwrap(),
            [item("new", &new_source), item("old", &old_source)]
        );
    }

    #[test]
    fn successful_limit_eviction_saves_new_item_and_removes_old_source() {
        let directory = tempfile::tempdir().unwrap();
        let history_path = directory.path().join("history.json");
        let old_source = directory.path().join("old.png");
        let new_source = directory.path().join("new.png");
        fs::write(&old_source, b"old").unwrap();
        fs::write(&new_source, b"new").unwrap();
        save_history_to(&history_path, &[item("old", &old_source)]).unwrap();

        added_without_warnings(add_item_with(
            &history_path,
            item("new", &new_source),
            1,
            save_history_to,
        ));

        assert!(!old_source.exists());
        assert!(new_source.exists());
        assert_eq!(
            load_history_from(&history_path).unwrap(),
            [item("new", &new_source)]
        );
    }

    #[test]
    fn failed_eviction_cleanup_is_compensated_by_retaining_its_history_entry() {
        let directory = tempfile::tempdir().unwrap();
        let history_path = directory.path().join("history.json");
        let old_source = directory.path().join("old.png");
        let new_source = directory.path().join("new.png");
        fs::create_dir(&old_source).unwrap();
        fs::write(&new_source, b"new").unwrap();
        save_history_to(&history_path, &[item("old", &old_source)]).unwrap();

        let outcome = add_item_with(&history_path, item("new", &new_source), 1, save_history_to);

        assert_eq!(
            outcome,
            AddOutcome::Added {
                retained_entries: vec![old_source.to_string_lossy().into_owned()],
                backup_warnings: Vec::new(),
                minimum_one_retained: false,
            }
        );
        assert!(old_source.is_dir());
        assert!(new_source.exists());
        assert_eq!(
            load_history_from(&history_path).unwrap(),
            [item("new", &new_source), item("old", &old_source)]
        );
    }

    #[test]
    fn failed_eviction_repair_keeps_assets_and_reports_unindexed_path() {
        let directory = tempfile::tempdir().unwrap();
        let history_path = directory.path().join("history.json");
        let old_source = directory.path().join("old.png");
        let new_source = directory.path().join("new.png");
        fs::create_dir(&old_source).unwrap();
        fs::write(&new_source, b"new").unwrap();
        let previous = vec![item("old", &old_source)];
        save_history_to(&history_path, &previous).unwrap();
        let writes = std::cell::Cell::new(0);

        let outcome = add_item_with(&history_path, item("new", &new_source), 1, |path, items| {
            let write_index = writes.get();
            writes.set(write_index + 1);
            if write_index == 0 {
                save_history_to(path, items)
            } else {
                Err(io::Error::other("injected repair write failure"))
            }
        });

        assert_eq!(
            outcome,
            AddOutcome::CleanupRepairFailed {
                cleanup_failed_paths: vec![old_source.to_string_lossy().into_owned()],
                error: "injected repair write failure".into(),
                backup_warnings: Vec::new(),
                minimum_one_retained: false,
            }
        );
        assert!(old_source.is_dir());
        assert!(new_source.exists());
        assert_eq!(
            load_history_from(&history_path).unwrap(),
            [item("new", &new_source)]
        );
    }

    #[test]
    fn zero_limit_preserves_the_new_capture_and_removes_old_items() {
        let directory = tempfile::tempdir().unwrap();
        let history_path = directory.path().join("history.json");
        let old_source = directory.path().join("old.png");
        let new_source = directory.path().join("new.png");
        fs::write(&old_source, b"old").unwrap();
        fs::write(&new_source, b"new").unwrap();
        save_history_to(&history_path, &[item("old", &old_source)]).unwrap();

        let outcome = add_item_with(&history_path, item("new", &new_source), 0, save_history_to);

        assert_eq!(
            outcome,
            AddOutcome::Added {
                retained_entries: Vec::new(),
                backup_warnings: Vec::new(),
                minimum_one_retained: true,
            }
        );
        assert!(outcome
            .warning("capture")
            .unwrap()
            .1
            .contains("limit was zero"));
        assert!(!old_source.exists());
        assert!(new_source.exists());
        assert_eq!(
            load_history_from(&history_path).unwrap(),
            [item("new", &new_source)]
        );
    }

    #[test]
    fn evicting_duplicate_path_never_deletes_a_retained_source() {
        let directory = tempfile::tempdir().unwrap();
        let history_path = directory.path().join("history.json");
        let source = directory.path().join("capture.png");
        fs::write(&source, b"capture").unwrap();
        save_history_to(&history_path, &[item("old", &source)]).unwrap();

        added_without_warnings(add_item_with(
            &history_path,
            item("new", &source),
            1,
            save_history_to,
        ));

        assert!(source.exists());
        assert_eq!(
            load_history_from(&history_path).unwrap(),
            [item("new", &source)]
        );
    }

    #[test]
    fn evicting_a_path_alias_never_deletes_a_retained_source() {
        let directory = tempfile::tempdir().unwrap();
        let history_path = directory.path().join("history.json");
        let source = directory.path().join("capture.png");
        let alias = directory.path().join(".").join("capture.png");
        fs::write(&source, b"capture").unwrap();
        save_history_to(&history_path, &[item("old", &source)]).unwrap();

        added_without_warnings(add_item_with(
            &history_path,
            item("new", &alias),
            1,
            save_history_to,
        ));

        assert!(source.exists());
        assert_eq!(
            load_history_from(&history_path).unwrap(),
            [item("new", &alias)]
        );
    }

    #[test]
    fn video_eviction_preserves_retained_project_and_loose_sidecar_paths() {
        let directory = tempfile::tempdir().unwrap();
        let history_path = directory.path().join("history.json");

        let project = directory.path().join("Folder.poratake");
        fs::create_dir(&project).unwrap();
        let recording = crate::video::project::recording_video_path(&project);
        fs::write(&recording, b"recording").unwrap();
        save_history_to(&history_path, &[video_item("old-project", &project)]).unwrap();
        added_without_warnings(add_item_with(
            &history_path,
            video_item("new-recording", &recording),
            1,
            save_history_to,
        ));
        assert!(project.exists());
        assert!(recording.exists());
        assert_eq!(
            load_history_from(&history_path).unwrap(),
            [video_item("new-recording", &recording)]
        );

        save_history_to(&history_path, &[video_item("old-recording", &recording)]).unwrap();
        added_without_warnings(add_item_with(
            &history_path,
            video_item("new-project", &project),
            1,
            save_history_to,
        ));
        assert!(project.exists());
        assert!(recording.exists());
        assert_eq!(
            load_history_from(&history_path).unwrap(),
            [video_item("new-project", &project)]
        );

        let loose_video = directory.path().join("loose.mp4");
        let system_audio = crate::video::project::system_audio_path(&loose_video);
        fs::write(&loose_video, b"video").unwrap();
        fs::write(&system_audio, b"audio").unwrap();
        save_history_to(
            &history_path,
            &[video_item("old-loose-video", &loose_video)],
        )
        .unwrap();
        added_without_warnings(add_item_with(
            &history_path,
            video_item("retained-audio-sidecar", &system_audio),
            1,
            save_history_to,
        ));
        assert!(loose_video.exists());
        assert!(system_audio.exists());
        assert_eq!(
            load_history_from(&history_path).unwrap(),
            [video_item("retained-audio-sidecar", &system_audio)]
        );
    }

    #[test]
    fn evicted_screenshot_sidecar_is_protected_by_retained_loose_video_scope() {
        let directory = tempfile::tempdir().unwrap();
        let history_path = directory.path().join("history.json");
        let video = directory.path().join("retained.mp4");
        let sidecar = crate::video::project::system_audio_path(&video);
        let new_capture = directory.path().join("new-capture.png");
        fs::write(&video, b"video").unwrap();
        fs::write(&sidecar, b"audio").unwrap();
        fs::write(&new_capture, b"new").unwrap();
        save_history_to(
            &history_path,
            &[
                video_item("retained-video", &video),
                item("evicted-screenshot", &sidecar),
            ],
        )
        .unwrap();

        added_without_warnings(add_item_with(
            &history_path,
            item("new-capture", &new_capture),
            2,
            save_history_to,
        ));

        assert!(video.exists());
        assert!(sidecar.exists());
        assert_eq!(
            load_history_from(&history_path).unwrap(),
            [
                item("new-capture", &new_capture),
                video_item("retained-video", &video)
            ]
        );
    }

    #[cfg(unix)]
    #[test]
    fn project_alias_with_parent_component_protects_canonical_retained_recording() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let history_path = directory.path().join("history.json");
        let real_root = directory.path().join("real");
        let project = real_root.join("Project.poratake");
        std::fs::create_dir_all(project.join("inside")).unwrap();
        let link_parent = directory.path().join("alias");
        std::fs::create_dir(&link_parent).unwrap();
        symlink(real_root.join("through-link"), link_parent.join("link")).unwrap();
        std::fs::create_dir(real_root.join("through-link")).unwrap();
        let removed_alias = link_parent.join("link").join("..").join("Project.poratake");
        let recording = crate::video::project::recording_video_path(&project);
        fs::write(&recording, b"recording").unwrap();
        save_history_to(&history_path, &[video_item("old-alias", &removed_alias)]).unwrap();

        added_without_warnings(add_item_with(
            &history_path,
            video_item("retained-recording", &recording),
            1,
            save_history_to,
        ));

        assert!(project.exists());
        assert!(recording.exists());
        assert_eq!(
            load_history_from(&history_path).unwrap(),
            [video_item("retained-recording", &recording)]
        );
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
