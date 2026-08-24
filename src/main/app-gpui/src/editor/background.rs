//! Background sources for the editor's wallpaper — the desktop wallpaper and
//! a file the user picks. Port of the `wallpaper:getDesktopWallpaper` and
//! `wallpaper:selectImage` handlers in `main/settings/wallpaper-ipc.ts`.

use std::path::PathBuf;

/// The extensions the renderer's image picker offers.
pub const IMAGE_EXTENSIONS: [&str; 8] = ["png", "jpg", "jpeg", "jfif", "svg", "webp", "gif", "bmp"];

/// Reads the desktop wallpaper through the daemon's `desktop-wallpaper`
/// module. The daemon answers with either a file path or an inline data URL,
/// both of which the rasterizer can load.
pub fn desktop_wallpaper(daemon: &crate::daemon::DaemonHandle) -> Option<String> {
    let response = daemon.call("desktop-wallpaper", "get", None).ok()?;
    from_response(&response)
}

/// Splits the daemon's `{ type, value }` answer into something loadable, or
/// `None` when the file it points at is gone.
pub fn from_response(response: &serde_json::Value) -> Option<String> {
    let kind = response.get("type")?.as_str()?;
    let value = response.get("value")?.as_str()?;
    match kind {
        "data" => Some(value.to_string()),
        "path" => PathBuf::from(value).is_file().then(|| value.to_string()),
        _ => None,
    }
}

/// Opens the picker the renderer's "Add Background" button opens.
pub fn pick_image() -> Option<String> {
    rfd::FileDialog::new()
        .set_title("Choose background image")
        .add_filter("Images", &IMAGE_EXTENSIONS)
        .pick_file()
        .map(|path| path.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_data_url_is_used_as_is() {
        let response = serde_json::json!({ "type": "data", "value": "data:image/png;base64,AAAA" });
        assert_eq!(
            from_response(&response).as_deref(),
            Some("data:image/png;base64,AAAA")
        );
    }

    #[test]
    fn a_missing_file_path_yields_nothing() {
        let response = serde_json::json!({
            "type": "path",
            "value": "/definitely/not/here.png"
        });
        assert!(from_response(&response).is_none());
    }

    #[test]
    fn an_existing_file_path_is_returned() {
        let path = std::env::temp_dir().join("poratake-wallpaper-test.png");
        std::fs::write(&path, b"not really a png").expect("write");
        let response = serde_json::json!({
            "type": "path",
            "value": path.to_string_lossy(),
        });
        assert_eq!(
            from_response(&response),
            Some(path.to_string_lossy().to_string())
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn an_unknown_kind_is_ignored() {
        let response = serde_json::json!({ "type": "url", "value": "https://example.com/a.png" });
        assert!(from_response(&response).is_none());
        assert!(from_response(&serde_json::json!({})).is_none());
    }
}
