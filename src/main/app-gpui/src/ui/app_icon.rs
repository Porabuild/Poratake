//! The application icon, as an element.
//!
//! `about-tab.tsx` and `onboarding-window.tsx` both render
//! `<img src={appIcon} className="h-16 w-16 rounded-xl|2xl">` -- the real
//! `build/icon.png`. This shell drew an aperture glyph on an accent-coloured
//! tile instead, which is a different picture in both places.

use std::sync::{Arc, OnceLock};

use gpui::{img, prelude::*, AnyElement, Pixels, RenderImage};

/// The same file the renderer imports as `@build/icon.png`.
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
            "the icon is the square master the renderer imports"
        );
    }

    /// `about-tab.tsx` and `onboarding-window.tsx` both point at the same file,
    /// so this fails if either stops using it and the two pictures drift apart.
    #[test]
    fn both_windows_import_the_same_file_in_the_renderer() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .expect("repository root")
            .to_path_buf();
        for relative in [
            "src/renderer/components/settings/about-tab.tsx",
            "src/renderer/windows/onboarding-window.tsx",
        ] {
            let source = std::fs::read_to_string(root.join(relative)).expect("read the window");
            assert!(
                source.contains("from '@build/icon.png'"),
                "{relative} no longer renders build/icon.png"
            );
        }
    }
}
