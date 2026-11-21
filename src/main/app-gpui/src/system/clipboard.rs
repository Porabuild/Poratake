use std::path::Path;

#[cfg(feature = "parity")]
use anyhow::{anyhow, Result};
use gpui::{ClipboardEntry, ClipboardItem, Image, ImageFormat};
use herogpui::gpui;
#[cfg(feature = "parity")]
use image::RgbaImage;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

#[cfg(any(target_os = "linux", test))]
pub(crate) fn file_uri_for_path(path: &Path) -> String {
    let mut uri = String::from("file://");
    for &byte in path.to_string_lossy().as_bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'_' | b'.' | b'~') {
            uri.push(byte as char);
        } else {
            use std::fmt::Write as _;
            let _ = write!(uri, "%{byte:02X}");
        }
    }
    uri
}

pub struct ClipboardService;

impl ClipboardService {
    pub fn write_text(cx: &gpui::App, text: String) {
        cx.write_to_clipboard(ClipboardItem::new_string(text));
    }

    pub fn write_png(cx: &gpui::App, bytes: Vec<u8>) {
        let image = Image::from_bytes(ImageFormat::Png, bytes);
        cx.write_to_clipboard(ClipboardItem::new_image(&image));
    }

    #[cfg(feature = "parity")]
    pub fn write_rgba(cx: &gpui::App, width: u32, height: u32, bytes: Vec<u8>) -> Result<()> {
        let rgba = RgbaImage::from_raw(width, height, bytes)
            .ok_or_else(|| anyhow!("clipboard image dimensions were invalid"))?;
        Self::write_png(cx, crate::render::images::encode_srgb_png(&rgba)?);
        Ok(())
    }

    pub fn write_file(path: &Path) -> bool {
        #[cfg(target_os = "macos")]
        return macos::write_file(path);
        #[cfg(windows)]
        return windows::write_file(path);
        #[cfg(target_os = "linux")]
        return linux::write_file(path);
        #[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
        {
            let _ = path;
            false
        }
    }

    pub fn read_image(cx: &gpui::App) -> Option<Image> {
        cx.read_from_clipboard()?
            .into_entries()
            .find_map(|entry| match entry {
                ClipboardEntry::Image(image) => Some(image),
                ClipboardEntry::String(_) | ClipboardEntry::ExternalPaths(_) => None,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[herogpui::test]
    fn images_use_the_app_clipboard(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| {
            ClipboardService::write_png(cx, vec![1, 2, 3]);
            assert_eq!(
                ClipboardService::read_image(cx).unwrap().bytes,
                vec![1, 2, 3]
            );
        });
    }

    #[test]
    fn encodes_linux_clipboard_file_uris() {
        assert_eq!(
            file_uri_for_path(Path::new("/tmp/My Captures/shot #1.png")),
            "file:///tmp/My%20Captures/shot%20%231.png"
        );
    }
}
