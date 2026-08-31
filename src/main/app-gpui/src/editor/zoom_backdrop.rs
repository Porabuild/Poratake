//! The blurred backdrop behind the editor's zoom control.
//!
//! `zoom/index.tsx` is `bg-surface/90 backdrop-blur-md`: a 90% surface over a
//! blurred copy of whatever is behind it. gpui composites the 10% for real, so
//! the only missing piece is the low-pass on the canvas showing through.
//!
//! There is no composited canvas buffer to sample — the canvas is an element
//! tree — so this samples the source capture instead. That is exact wherever the
//! bar sits over the capture, and blur is the identity over the flat stage
//! background elsewhere, so the fallback (no backdrop, as before) is already
//! correct there. The one case it does not cover is an annotation directly under
//! the bar, which the blur would smear and this does not.

use std::sync::Arc;

use gpui::{Bounds, Pixels};

/// Tailwind `blur-md` is `blur(12px)`, and CSS filter blur takes a standard
/// deviation, so this is the sigma directly.
const BLUR_SIGMA: f32 = 12.0;

/// What the cached backdrop was built for. A change in any of these invalidates
/// it; nothing else can affect the sampled pixels.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct BackdropKey {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

pub struct ZoomBackdrop {
    key: BackdropKey,
    image: Arc<gpui::RenderImage>,
}

impl ZoomBackdrop {
    pub fn image(&self) -> Arc<gpui::RenderImage> {
        self.image.clone()
    }

    pub fn key(&self) -> BackdropKey {
        self.key
    }
}

/// The bar's rectangle in image pixels, or `None` when it is not entirely over
/// the capture — in which case the backdrop it would blur is flat and the plain
/// surface is already right.
pub fn sample_rect(
    bar: Bounds<Pixels>,
    content: Bounds<Pixels>,
    zoom: f32,
    image_width: f32,
    image_height: f32,
) -> Option<BackdropKey> {
    if zoom <= 0.0 || bar.size.width <= Pixels::ZERO || bar.size.height <= Pixels::ZERO {
        return None;
    }

    let left = (f32::from(bar.origin.x - content.origin.x)) / zoom;
    let top = (f32::from(bar.origin.y - content.origin.y)) / zoom;
    let width = f32::from(bar.size.width) / zoom;
    let height = f32::from(bar.size.height) / zoom;

    // Only when the whole bar is over the capture; a partial overlap would need
    // the wallpaper raster too, and blur over the flat remainder is identity.
    if left < 0.0 || top < 0.0 || left + width > image_width || top + height > image_height {
        return None;
    }

    Some(BackdropKey {
        x: left.floor() as i32,
        y: top.floor() as i32,
        width: (width.ceil() as u32).max(1),
        height: (height.ceil() as u32).max(1),
    })
}

/// Crops the capture to `key`, blurs it, and uploads it. Returns the existing
/// backdrop untouched when the key has not moved.
pub fn build(
    cached: Option<ZoomBackdrop>,
    key: BackdropKey,
    base: &image::DynamicImage,
) -> Option<ZoomBackdrop> {
    if let Some(existing) = cached {
        if existing.key == key {
            return Some(existing);
        }
    }

    let crop = image::imageops::crop_imm(base, key.x as u32, key.y as u32, key.width, key.height)
        .to_image();
    let mut pixmap = crate::editor::export::from_rgba(&crop)?;
    crate::render::blur::blur(&mut pixmap, BLUR_SIGMA);
    let blurred = crate::editor::export::to_rgba(&pixmap);

    let frame = image::Frame::new(blurred);
    Some(ZoomBackdrop {
        key,
        image: Arc::new(gpui::RenderImage::new(vec![frame])),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{px, size};

    fn bounds(x: f32, y: f32, w: f32, h: f32) -> Bounds<Pixels> {
        Bounds {
            origin: gpui::point(px(x), px(y)),
            size: size(px(w), px(h)),
        }
    }

    #[test]
    fn maps_the_bar_into_image_space_through_the_zoom() {
        let content = bounds(100.0, 50.0, 800.0, 600.0);
        let bar = bounds(760.0, 590.0, 110.0, 36.0);
        // At 1x the offset is a plain subtraction.
        let key = sample_rect(bar, content, 1.0, 800.0, 600.0).expect("over the capture");
        assert_eq!((key.x, key.y), (660, 540));
        assert_eq!((key.width, key.height), (110, 36));
    }

    #[test]
    fn a_zoomed_canvas_samples_a_proportionally_smaller_region() {
        let content = bounds(0.0, 0.0, 1600.0, 1200.0);
        let bar = bounds(1400.0, 1100.0, 110.0, 36.0);
        let key = sample_rect(bar, content, 2.0, 800.0, 600.0).expect("over the capture");
        assert_eq!((key.x, key.y), (700, 550));
        // Half the screen size, because each image pixel covers two on screen.
        assert_eq!((key.width, key.height), (55, 18));
    }

    #[test]
    fn declines_when_the_bar_is_not_entirely_over_the_capture() {
        let content = bounds(0.0, 0.0, 800.0, 600.0);
        // Hanging off the right edge, e.g. over a wallpaper margin.
        assert!(sample_rect(
            bounds(750.0, 500.0, 110.0, 36.0),
            content,
            1.0,
            800.0,
            600.0
        )
        .is_none());
        // Above the capture entirely.
        assert!(
            sample_rect(bounds(10.0, -80.0, 110.0, 36.0), content, 1.0, 800.0, 600.0).is_none()
        );
        // A degenerate bar, before the first layout has measured it.
        assert!(sample_rect(bounds(0.0, 0.0, 0.0, 0.0), content, 1.0, 800.0, 600.0).is_none());
    }

    #[test]
    fn the_blur_is_rebuilt_only_when_the_sampled_region_moves() {
        let base = image::DynamicImage::ImageRgba8(image::RgbaImage::new(200, 200));
        let key = BackdropKey {
            x: 10,
            y: 10,
            width: 40,
            height: 20,
        };
        let first = build(None, key, &base).expect("built");
        // Held for the whole test: without a live strong reference the first
        // allocation can be freed and the rebuilt one handed the same address,
        // which would make the final assertion fail at random.
        let held = first.image();
        let pointer = Arc::as_ptr(&held);
        // Same key: the identical upload is reused.
        let again = build(Some(first), key, &base).expect("reused");
        assert_eq!(Arc::as_ptr(&again.image), pointer);
        // Moved: a fresh one.
        let moved = BackdropKey { x: 20, ..key };
        let rebuilt = build(Some(again), moved, &base).expect("rebuilt");
        assert_ne!(Arc::as_ptr(&rebuilt.image), pointer);
        drop(held);
    }
}
