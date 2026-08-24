use std::fs;

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

pub fn add_item(item: HistoryItem, max_items: usize) {
    let mut items = load_history();
    items.insert(0, item);
    items.truncate(max_items);
    save_history(&items);
}

pub fn remove_item(id: &str) {
    let mut items = load_history();
    let original_len = items.len();
    items.retain(|item| item.id != id);
    if items.len() != original_len {
        save_history(&items);
    }
}
