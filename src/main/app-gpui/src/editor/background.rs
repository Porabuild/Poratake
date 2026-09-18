//! Background sources for the editor's wallpaper — the desktop wallpaper and
//! a file the user picks. Port of the `wallpaper:getDesktopWallpaper` and
//! `wallpaper:selectImage` handlers in `main/settings/wallpaper-ipc.ts`.

use std::path::PathBuf;
use std::sync::Arc;

use herogpui::gpui;

#[derive(Clone, Default)]
pub enum PreviewState {
    #[default]
    Loading,
    Ready(Arc<gpui::RenderImage>),
    Failed,
}

pub const PREVIEW_SCALE: f32 = 2.0;

#[derive(Default)]
pub struct BackgroundPreviews {
    desktop: Option<PreviewState>,
    customs: std::collections::HashMap<String, PreviewState>,
}

impl BackgroundPreviews {
    pub fn desktop(&self) -> PreviewState {
        self.desktop.clone().unwrap_or_default()
    }

    pub fn desktop_requested(&self) -> bool {
        self.desktop.is_some()
    }

    pub fn custom(&self, id: &str) -> PreviewState {
        self.customs.get(id).cloned().unwrap_or_default()
    }

    pub fn custom_requested(&self, id: &str) -> bool {
        self.customs.contains_key(id)
    }

    pub fn request_desktop(&mut self) {
        self.desktop = Some(PreviewState::Loading);
    }

    pub fn request_custom(&mut self, id: &str) {
        self.customs.insert(id.to_string(), PreviewState::Loading);
    }

    pub fn set_desktop(&mut self, image: Option<Arc<gpui::RenderImage>>) {
        self.desktop = Some(state_for(image));
    }

    pub fn set_custom(&mut self, id: &str, image: Option<Arc<gpui::RenderImage>>) {
        self.customs.insert(id.to_string(), state_for(image));
    }
}

fn state_for(image: Option<Arc<gpui::RenderImage>>) -> PreviewState {
    match image {
        Some(image) => PreviewState::Ready(image),
        None => PreviewState::Failed,
    }
}

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

pub fn preview_image(source: &str, size: f32) -> Option<Arc<gpui::RenderImage>> {
    let decoded = crate::render::gradient::load_image(source)?;
    let target = (size * PREVIEW_SCALE).ceil().max(1.0);
    let scale = (target / decoded.width().max(decoded.height()) as f32).min(1.0);
    if scale >= 1.0 {
        return crate::windows::video_editor::preview::to_render_image(&decoded);
    }
    let width = ((decoded.width() as f32 * scale).ceil() as u32).max(1);
    let height = ((decoded.height() as f32 * scale).ceil() as u32).max(1);
    let mut scaled = tiny_skia::Pixmap::new(width, height)?;
    scaled.draw_pixmap(
        0,
        0,
        decoded.as_ref(),
        &tiny_skia::PixmapPaint {
            quality: tiny_skia::FilterQuality::Bilinear,
            ..Default::default()
        },
        tiny_skia::Transform::from_scale(scale, scale),
        None,
    );
    crate::windows::video_editor::preview::to_render_image(&scaled)
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
        let path = crate::util::test_paths::unique_temp("poratake-wallpaper-test.png");
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
