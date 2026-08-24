use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::config::store;

pub const SCREENSHOT_EDITOR: &str = "screenshot-editor";
#[allow(dead_code)]
pub const VIDEO_EDITOR: &str = "video-editor";

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
struct WindowSize {
    width: f32,
    height: f32,
}

fn path() -> PathBuf {
    store::config_dir().join("window-state.json")
}

fn load() -> HashMap<String, WindowSize> {
    let Ok(bytes) = fs::read_to_string(path()) else {
        return HashMap::new();
    };
    serde_json::from_str(&bytes).unwrap_or_default()
}

fn save(state: &HashMap<String, WindowSize>) {
    let _ = fs::create_dir_all(store::config_dir());
    let Ok(bytes) = serde_json::to_string_pretty(state) else {
        return;
    };
    let _ = fs::write(path(), bytes);
}

static CACHE: Mutex<Option<HashMap<String, WindowSize>>> = Mutex::new(None);

pub fn get(id: &str) -> Option<(f32, f32)> {
    let mut cache = CACHE.lock();
    if cache.is_none() {
        *cache = Some(load());
    }
    cache
        .as_ref()
        .and_then(|state| state.get(id).map(|size| (size.width, size.height)))
}

pub fn set(id: &str, width: f32, height: f32) {
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return;
    }
    let mut cache = CACHE.lock();
    if cache.is_none() {
        *cache = Some(load());
    }
    let Some(state) = cache.as_mut() else {
        return;
    };
    if let Some(existing) = state.get(id) {
        if (existing.width - width).abs() < f32::EPSILON
            && (existing.height - height).abs() < f32::EPSILON
        {
            return;
        }
    }
    state.insert(id.to_string(), WindowSize { width, height });
    save(state);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screenshot_editor_id_matches_electron() {
        assert_eq!(SCREENSHOT_EDITOR, "screenshot-editor");
        assert_eq!(VIDEO_EDITOR, "video-editor");
    }
}
