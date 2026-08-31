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
    let response = daemon.desktop_wallpaper().get().ok()?;
    from_response(&response)
}

/// Splits the daemon's `{ type, value }` answer into something loadable, or
/// `None` when the file it points at is gone.
pub fn from_response(
    response: &poratake_daemon_common::contract::DesktopWallpaperResult,
) -> Option<String> {
    match response {
        poratake_daemon_common::contract::DesktopWallpaperResult::Data(value) => {
            Some(value.clone())
        }
        poratake_daemon_common::contract::DesktopWallpaperResult::Path(value) => {
            PathBuf::from(value).is_file().then(|| value.clone())
        }
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
        let response = poratake_daemon_common::contract::DesktopWallpaperResult::Data(
            "data:image/png;base64,AAAA".into(),
        );
        assert_eq!(
            from_response(&response).as_deref(),
            Some("data:image/png;base64,AAAA")
        );
    }

    #[test]
    fn a_missing_file_path_yields_nothing() {
        let response = poratake_daemon_common::contract::DesktopWallpaperResult::Path(
            "/definitely/not/here.png".into(),
        );
        assert!(from_response(&response).is_none());
    }

    #[test]
    fn an_existing_file_path_is_returned() {
        let path = std::env::temp_dir().join("poratake-wallpaper-test.png");
        std::fs::write(&path, b"not really a png").expect("write");
        let response = poratake_daemon_common::contract::DesktopWallpaperResult::Path(
            path.to_string_lossy().into_owned(),
        );
        assert_eq!(
            from_response(&response),
            Some(path.to_string_lossy().to_string())
        );
        let _ = std::fs::remove_file(&path);
    }
}
