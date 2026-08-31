use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::store::config_dir;

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

pub fn history_file_path() -> std::path::PathBuf {
    config_dir().join("history.json")
}

pub fn load_history() -> Vec<HistoryItem> {
    let path = history_file_path();
    let content = match fs::read_to_string(&path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(error) => {
            eprintln!("[history] failed to load {}: {error}", path.display());
            return Vec::new();
        }
    };

    match serde_json::from_str(&content) {
        Ok(items) => items,
        Err(error) => {
            eprintln!("[history] failed to parse {}: {error}", path.display());
            Vec::new()
        }
    }
}

pub fn save_history(items: &[HistoryItem]) {
    let path = history_file_path();
    let Some(parent) = path.parent() else {
        return;
    };

    if let Err(error) = fs::create_dir_all(parent) {
        eprintln!("[history] failed to create {}: {error}", parent.display());
        return;
    }

    let content = match serde_json::to_string_pretty(items) {
        Ok(content) => content,
        Err(error) => {
            eprintln!("[history] failed to serialize history: {error}");
            return;
        }
    };

    if let Err(error) = fs::write(&path, content) {
        eprintln!("[history] failed to save {}: {error}", path.display());
    }
}

pub fn add_item(item: HistoryItem, max_items: usize, enabled: bool) {
    if !enabled {
        return;
    }
    let mut items = load_history();
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
    save_history(&items);
}

pub fn delete_item(id: &str) -> bool {
    let mut items = load_history();
    let Some(index) = items.iter().position(|item| item.id == id) else {
        return false;
    };
    if cleanup_item(&items[index]).is_err() {
        return false;
    }
    items.remove(index);
    save_history(&items);
    true
}

pub fn delete_path(path: &Path, kind: HistoryItemType) -> bool {
    let path_string = path.to_string_lossy();
    let mut items = load_history();
    if let Some(index) = items
        .iter()
        .position(|item| item.original_path == path_string)
    {
        if cleanup_item(&items[index]).is_err() {
            return false;
        }
        items.remove(index);
        save_history(&items);
        return true;
    }
    cleanup_path(path, kind).is_ok()
}

pub fn clear_history() -> bool {
    let items = load_history();
    let retained = items
        .into_iter()
        .filter(|item| cleanup_item(item).is_err())
        .collect::<Vec<_>>();
    let cleared = retained.is_empty();
    save_history(&retained);
    cleared
}

pub fn editor_state_for_path(path: &Path) -> Option<Value> {
    let path = path.to_string_lossy();
    load_history()
        .into_iter()
        .find(|item| item.original_path == path)
        .and_then(|item| item.editor_state)
}

pub fn update_editor_state_by_path(path: &Path, state: Value) -> bool {
    let path = path.to_string_lossy();
    let mut items = load_history();
    let Some(item) = items.iter_mut().find(|item| item.original_path == path) else {
        return false;
    };
    item.editor_state = Some(state);
    save_history(&items);
    true
}

pub fn update_item_path(old_path: &Path, new_path: &Path) -> bool {
    let old_path = old_path.to_string_lossy();
    let mut items = load_history();
    let Some(item) = items.iter_mut().find(|item| item.original_path == old_path) else {
        return false;
    };
    item.original_path = new_path.to_string_lossy().to_string();
    save_history(&items);
    true
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
}
