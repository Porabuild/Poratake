use std::sync::{Arc, OnceLock};

use gpui::{img, prelude::*, AnyElement, Pixels, RenderImage};
use herogpui::gpui;

const ICON_PNG: &[u8] = include_bytes!("../../../../../build/icon.png");

fn decode_icon(bytes: &[u8]) -> image::ImageResult<image::RgbaImage> {
    let mut image = image::load_from_memory(bytes)?.into_rgba8();
    for pixel in image.pixels_mut() {
        pixel.0.swap(0, 2);
    }
    Ok(image)
}

/// Decoded once. A 1024x1024 PNG is not something to re-decode per frame.
fn image() -> Option<Arc<RenderImage>> {
    static CACHE: OnceLock<Option<Arc<RenderImage>>> = OnceLock::new();
    CACHE
        .get_or_init(|| {
            let decoded = decode_icon(ICON_PNG)
                .inspect_err(|error| eprintln!("[icon] failed to decode the app icon: {error}"))
                .ok()?;
            let frame = image::Frame::new(decoded);
            Some(Arc::new(RenderImage::new(vec![frame])))
        })
        .clone()
}

/// A square of `size` with corner `radius`, or `None` if the icon could not be
/// decoded -- the caller decides what to fall back to.
pub fn element(size: Pixels, radius: Pixels) -> Option<AnyElement> {
    Some(
        img(image()?)
            .size(size)
            .rounded(radius)
            .object_fit(gpui::ObjectFit::Contain)
            .into_any_element(),
    )
}

#[cfg(test)]
mod tests {
    use image::Rgba;

    use super::{decode_icon, ICON_PNG};

    #[test]
    fn decodes_the_embedded_icon_in_gpui_bgra_order() {
        let image = decode_icon(ICON_PNG).expect("decode build/icon.png");

        assert_eq!(image.get_pixel(512, 512), &Rgba([0xef, 0x92, 0x88, 0xff]));
    }

    /// The bytes are embedded at compile time, so a missing or unreadable file
    /// is a build failure -- but a *corrupt* one would only show up at runtime.
    #[test]
    fn the_embedded_icon_decodes() {
        let decoded = image::load_from_memory(super::ICON_PNG).expect("decode build/icon.png");
        assert_eq!(
            (decoded.width(), decoded.height()),
            (1024, 1024),
            "the icon is the square master"
        );
    }
}
