use anyhow::{anyhow, Result};
use gpui::{ClipboardEntry, ClipboardItem, Image, ImageFormat};
use image::{DynamicImage, RgbaImage};

pub struct ClipboardService;

impl ClipboardService {
    pub fn write_text(cx: &gpui::App, text: String) {
        cx.write_to_clipboard(ClipboardItem::new_string(text));
    }

    pub fn read_text(cx: &gpui::App) -> Option<String> {
        cx.read_from_clipboard()?.text()
    }

    pub fn write_png(cx: &gpui::App, bytes: Vec<u8>) {
        let image = Image::from_bytes(ImageFormat::Png, bytes);
        cx.write_to_clipboard(ClipboardItem::new_image(&image));
    }

    pub fn write_rgba(cx: &gpui::App, width: u32, height: u32, bytes: Vec<u8>) -> Result<()> {
        let rgba = RgbaImage::from_raw(width, height, bytes)
            .ok_or_else(|| anyhow!("clipboard image dimensions were invalid"))?;
        let mut png = std::io::Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(rgba).write_to(&mut png, image::ImageFormat::Png)?;
        Self::write_png(cx, png.into_inner());
        Ok(())
    }

    pub fn read_image(cx: &gpui::App) -> Option<Image> {
        cx.read_from_clipboard()?
            .into_entries()
            .find_map(|entry| match entry {
                ClipboardEntry::Image(image) => Some(image),
                ClipboardEntry::String(_) => None,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[gpui::test]
    fn text_uses_the_app_clipboard(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| {
            ClipboardService::write_text(cx, "Poratake".to_string());
            assert_eq!(ClipboardService::read_text(cx).as_deref(), Some("Poratake"));
        });
    }

    #[gpui::test]
    fn images_use_the_app_clipboard(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| {
            ClipboardService::write_png(cx, vec![1, 2, 3]);
            assert_eq!(
                ClipboardService::read_image(cx).unwrap().bytes,
                vec![1, 2, 3]
            );
        });
    }
}
