//! Port of `renderGradientToCanvas` and `renderBackgroundImageToCanvas` in
//! `renderer/utils/wallpaper-render.ts` — the wallpaper backgrounds behind a
//! screenshot or a recording.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use sha2::{Digest, Sha256};
use tiny_skia::{
    FillRule, FilterQuality, GradientStop, LinearGradient, Pixmap, PixmapRef, Point, Rect,
    SpreadMode, Transform,
};

use crate::render::canvas::Canvas;
use crate::render::color;

const MAX_IMAGE_CACHE_BYTES: usize = 192 * 1024 * 1024;
const MAX_IMAGE_CACHE_ENTRIES: usize = 8;

#[derive(Clone, PartialEq, Eq)]
enum ImageCacheKey {
    DataUrl {
        len: usize,
        digest: [u8; 32],
    },
    File {
        path: String,
        len: u64,
        modified: Option<SystemTime>,
    },
}

struct ImageCache {
    entries: VecDeque<(ImageCacheKey, Arc<Pixmap>)>,
    bytes: usize,
    max_bytes: usize,
}

impl ImageCache {
    const fn new(max_bytes: usize) -> Self {
        Self {
            entries: VecDeque::new(),
            bytes: 0,
            max_bytes,
        }
    }

    fn get(&mut self, key: &ImageCacheKey) -> Option<Arc<Pixmap>> {
        let index = self.entries.iter().position(|(cached, _)| cached == key)?;
        let entry = self.entries.remove(index)?;
        let pixmap = entry.1.clone();
        self.entries.push_front(entry);
        Some(pixmap)
    }

    fn insert(&mut self, key: ImageCacheKey, pixmap: Arc<Pixmap>) {
        if let Some(index) = self.entries.iter().position(|(cached, _)| cached == &key) {
            if let Some((_, existing)) = self.entries.remove(index) {
                self.bytes = self.bytes.saturating_sub(existing.data().len());
            }
        }
        let bytes = pixmap.data().len();
        let remaining = self.max_bytes.saturating_sub(bytes);
        while self.bytes > remaining || self.entries.len() >= MAX_IMAGE_CACHE_ENTRIES {
            let Some((_, stale)) = self.entries.pop_back() else {
                break;
            };
            self.bytes = self.bytes.saturating_sub(stale.data().len());
        }
        self.bytes += bytes;
        self.entries.push_front((key, pixmap));
    }
}

fn image_cache_key(source: &str) -> Option<ImageCacheKey> {
    if source.contains("base64,") {
        let mut hasher = Sha256::new();
        hasher.update(source.as_bytes());
        let digest: [u8; 32] = hasher.finalize().into();
        return Some(ImageCacheKey::DataUrl {
            len: source.len(),
            digest,
        });
    }
    let metadata = std::fs::metadata(source).ok()?;
    Some(ImageCacheKey::File {
        path: source.to_string(),
        len: metadata.len(),
        modified: metadata.modified().ok(),
    })
}

pub fn premultiply_rgba(source: &image::RgbaImage) -> Option<(Pixmap, bool)> {
    let mut pixmap = Pixmap::new(source.width().max(1), source.height().max(1))?;
    let mut opaque = true;
    for (src, dst) in source
        .as_raw()
        .as_chunks::<4>()
        .0
        .iter()
        .zip(pixmap.data_mut().as_chunks_mut::<4>().0.iter_mut())
    {
        let [r, g, b, a] = *src;
        if a == 255 {
            *dst = [r, g, b, 255];
        } else if a == 0 {
            opaque = false;
            *dst = [0, 0, 0, 0];
        } else {
            opaque = false;
            let alpha = u32::from(a);
            *dst = [
                (u32::from(r) * alpha / 255) as u8,
                (u32::from(g) * alpha / 255) as u8,
                (u32::from(b) * alpha / 255) as u8,
                a,
            ];
        }
    }
    Some((pixmap, opaque))
}

pub fn premultiply_dynamic_image(source: &image::DynamicImage) -> Option<(Pixmap, bool)> {
    match source.as_rgba8() {
        Some(rgba) => premultiply_rgba(rgba),
        None => premultiply_rgba(&source.to_rgba8()),
    }
}

/// A `GradientOption` from `types/editor.ts` as it is persisted.
#[derive(Clone, Debug, Default, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct GradientOption {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub colors: Vec<String>,
    #[serde(default)]
    pub angle: f64,
}

impl GradientOption {
    pub fn is_renderable(&self) -> bool {
        self.colors.len() >= 2 || crate::editor::wallpaper_svg::is_preset(&self.id)
    }
}

/// The gradient's endpoints for a `width` x `height` surface, following the
/// renderer's `angle - 90` convention (0 degrees points up).
pub fn endpoints(angle: f64, width: f32, height: f32) -> ((f32, f32), (f32, f32)) {
    let angle = if angle.is_finite() { angle } else { 0.0 };
    let radians = ((angle - 90.0) * std::f64::consts::PI / 180.0) as f32;
    let (cos, sin) = (radians.cos(), radians.sin());
    let half_width = width / 2.0;
    let half_height = height / 2.0;
    let length = (width * width + height * height).sqrt() / 2.0;
    (
        (half_width - cos * length, half_height - sin * length),
        (half_width + cos * length, half_height + sin * length),
    )
}

pub fn fill(canvas: &mut Canvas, gradient: &GradientOption, width: f32, height: f32) {
    fill_excluding(canvas, gradient, width, height, None);
}

pub fn fill_excluding(
    canvas: &mut Canvas,
    gradient: &GradientOption,
    width: f32,
    height: f32,
    excluded: Option<(u32, u32, u32, u32)>,
) {
    if width <= 0.0 || height <= 0.0 {
        return;
    }
    if let Some(artwork) = crate::editor::wallpaper_svg::cover_pixmap(&gradient.id, width, height) {
        fill_image_with_quality_excluding(
            canvas,
            artwork.as_ref().as_ref(),
            width,
            height,
            FilterQuality::Bilinear,
            excluded,
        );
        return;
    }
    if !gradient.is_renderable() {
        return;
    }
    let (start, end) = endpoints(gradient.angle, width, height);
    let last = (gradient.colors.len() - 1) as f32;
    let stops: Vec<GradientStop> = gradient
        .colors
        .iter()
        .enumerate()
        .map(|(index, value)| {
            GradientStop::new(
                index as f32 / last,
                color::parse_or(value, tiny_skia::Color::TRANSPARENT),
            )
        })
        .collect();

    let Some(shader) = LinearGradient::new(
        Point::from_xy(start.0, start.1),
        Point::from_xy(end.0, end.1),
        stops,
        SpreadMode::Pad,
        Transform::identity(),
    ) else {
        return;
    };
    let Some(rect) = Rect::from_xywh(0.0, 0.0, width, height) else {
        return;
    };
    let mut builder = tiny_skia::PathBuilder::new();
    builder.push_rect(rect);
    if let Some(path) = builder.finish() {
        canvas.fill_path_shader(&path, shader, FillRule::Winding);
    }
}

/// Covers the surface with `image`, cropping the overflowing axis — the
/// `object-fit: cover` behaviour of `renderBackgroundImageToCanvas`.
pub fn fill_image(canvas: &mut Canvas, image: PixmapRef<'_>, width: f32, height: f32) {
    fill_image_with_quality_excluding(canvas, image, width, height, FilterQuality::Bilinear, None);
}

pub fn fill_image_high_quality_excluding(
    canvas: &mut Canvas,
    image: PixmapRef<'_>,
    width: f32,
    height: f32,
    excluded: Option<(u32, u32, u32, u32)>,
) {
    fill_image_with_quality_excluding(
        canvas,
        image,
        width,
        height,
        FilterQuality::Bicubic,
        excluded,
    );
}

fn fill_image_with_quality_excluding(
    canvas: &mut Canvas,
    image: PixmapRef<'_>,
    width: f32,
    height: f32,
    quality: FilterQuality,
    excluded: Option<(u32, u32, u32, u32)>,
) {
    if image.width() == 0 || image.height() == 0 || width <= 0.0 || height <= 0.0 {
        return;
    }
    let image_aspect = image.width() as f32 / image.height() as f32;
    let surface_aspect = width / height;

    let (draw_width, draw_height) = if image_aspect > surface_aspect {
        (height * image_aspect, height)
    } else {
        (width, width / image_aspect)
    };
    let x = (width - draw_width) / 2.0;
    let y = (height - draw_height) / 2.0;
    canvas.draw_pixmap_with_quality_excluding(
        image,
        x,
        y,
        draw_width,
        draw_height,
        quality,
        excluded,
    );
}

/// Port of `renderNoise` in `renderer/utils/noise.ts` — a grain overlay over
/// the background. `seed` keeps the grain stable between renders, so a preview
/// and its export get the same pixels.
pub fn overlay_noise(canvas: &mut Canvas, width: f32, height: f32, noise: f64, seed: u64) {
    if noise <= 0.0 || width <= 0.0 || height <= 0.0 {
        return;
    }
    let opacity = ((noise / 100.0) * 0.3).clamp(0.0, 1.0) as f32;
    let (columns, rows) = (width.ceil() as u32, height.ceil() as u32);
    let Some(mut grain) = Pixmap::new(columns.max(1), rows.max(1)) else {
        return;
    };

    // A xorshift keeps the grain reproducible without pulling in a random
    // number generator; the renderer's `Math.random()` only has to look random.
    let mut state = seed | 1;
    for pixel in grain.data_mut().as_chunks_mut::<4>().0 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let value = (state >> 24) as u8;
        pixel[0] = value;
        pixel[1] = value;
        pixel[2] = value;
        pixel[3] = 255;
    }

    canvas.save();
    canvas.set_shadow(None);
    canvas.set_global_alpha(opacity);
    canvas.draw_pixmap_blended(
        grain.as_ref(),
        0.0,
        0.0,
        width,
        height,
        tiny_skia::BlendMode::Overlay,
    );
    canvas.restore();
}

static IMAGE_CACHE: Mutex<ImageCache> = Mutex::new(ImageCache::new(MAX_IMAGE_CACHE_BYTES));

pub fn clear_image_cache() {
    if let Ok(mut cache) = IMAGE_CACHE.lock() {
        cache.entries.clear();
        cache.bytes = 0;
    }
}

pub fn load_image_cached(source: &str) -> Option<Arc<Pixmap>> {
    let key = image_cache_key(source)?;
    if let Some(cached) = IMAGE_CACHE
        .lock()
        .ok()
        .and_then(|mut cache| cache.get(&key))
    {
        return Some(cached);
    }
    let decoded = super::images::load_source(source)?.into_rgba8();
    let (pixmap, _) = premultiply_rgba(&decoded)?;
    let pixmap = Arc::new(pixmap);
    if let Ok(mut cache) = IMAGE_CACHE.lock() {
        cache.insert(key, pixmap.clone());
    }
    Some(pixmap)
}

/// Decodes a `data:` URL or a file path into a pixmap, which is how both the
/// wallpaper background and the first-frame image arrive from `state.json`.
pub fn load_image(source: &str) -> Option<Pixmap> {
    load_image_cached(source).map(|pixmap| (*pixmap).clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_zero_degree_gradient_runs_bottom_to_top() {
        let (start, end) = endpoints(0.0, 100.0, 100.0);
        assert!(start.1 > end.1, "{start:?} -> {end:?}");
        assert!((start.0 - 50.0).abs() < 0.001);
    }

    #[test]
    fn a_ninety_degree_gradient_runs_left_to_right() {
        let (start, end) = endpoints(90.0, 100.0, 100.0);
        assert!(start.0 < end.0, "{start:?} -> {end:?}");
        assert!((start.1 - 50.0).abs() < 0.001);
    }

    #[test]
    fn a_single_colour_gradient_is_not_renderable() {
        let gradient = GradientOption {
            colors: vec!["#fff".into()],
            ..GradientOption::default()
        };
        assert!(!gradient.is_renderable());
    }

    #[test]
    fn fills_the_whole_surface() {
        let mut canvas = Canvas::new(16, 16).expect("canvas");
        let gradient = GradientOption {
            colors: vec!["#ff0000".into(), "#0000ff".into()],
            angle: 90.0,
            ..GradientOption::default()
        };
        fill(&mut canvas, &gradient, 16.0, 16.0);
        assert!(canvas
            .pixmap()
            .data()
            .as_chunks::<4>()
            .0
            .iter()
            .all(|pixel| pixel[3] == 255));
    }

    #[test]
    fn noise_is_reproducible_for_a_seed() {
        let render = |seed: u64| {
            let mut canvas = Canvas::new(16, 16).expect("canvas");
            canvas.fill_all(tiny_skia::Color::from_rgba8(128, 128, 128, 255));
            overlay_noise(&mut canvas, 16.0, 16.0, 100.0, seed);
            canvas.into_pixmap().data().to_vec()
        };
        assert_eq!(render(7), render(7));
        assert_ne!(render(7), render(9));
    }

    #[test]
    fn zero_noise_leaves_the_surface_alone() {
        let mut canvas = Canvas::new(8, 8).expect("canvas");
        canvas.fill_all(tiny_skia::Color::from_rgba8(10, 20, 30, 255));
        let before = canvas.pixmap().data().to_vec();
        overlay_noise(&mut canvas, 8.0, 8.0, 0.0, 1);
        assert_eq!(canvas.pixmap().data(), &before[..]);
    }

    #[test]
    fn covers_with_an_image_without_letterboxing() {
        let mut source = Pixmap::new(4, 2).expect("pixmap");
        source.fill(tiny_skia::Color::from_rgba8(0, 255, 0, 255));
        let mut canvas = Canvas::new(10, 10).expect("canvas");
        fill_image(&mut canvas, source.as_ref(), 10.0, 10.0);
        assert!(canvas
            .pixmap()
            .data()
            .as_chunks::<4>()
            .0
            .iter()
            .all(|pixel| pixel[3] == 255));
    }

    #[test]
    fn cached_image_loader_reuses_decoded_pixmaps_and_invalidates_changed_files() {
        use base64::Engine;

        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("wall.png");
        image::RgbaImage::from_pixel(16, 12, image::Rgba([20, 40, 60, 255]))
            .save(&file)
            .unwrap();
        let path = file.to_string_lossy().into_owned();
        let first = load_image_cached(&path).unwrap();
        let second = load_image_cached(&path).unwrap();
        assert!(Arc::ptr_eq(&first, &second));

        image::RgbaImage::from_pixel(24, 18, image::Rgba([200, 100, 50, 255]))
            .save(&file)
            .unwrap();
        let updated = load_image_cached(&path).unwrap();
        assert!(!Arc::ptr_eq(&first, &updated));
        assert_eq!((updated.width(), updated.height()), (24, 18));

        let mut png = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            8,
            8,
            image::Rgba([10, 200, 30, 128]),
        ))
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
        let data_url = format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(png.into_inner())
        );
        let a = load_image_cached(&data_url).unwrap();
        let b = load_image_cached(&data_url).unwrap();
        assert!(Arc::ptr_eq(&a, &b));
    }

    #[test]
    fn premultiply_rgba_reports_opacity_and_preserves_alpha_math() {
        let opaque_img = image::RgbaImage::from_pixel(4, 4, image::Rgba([10, 20, 30, 255]));
        let (pm, opaque) = premultiply_rgba(&opaque_img).unwrap();
        assert!(opaque);
        assert_eq!(&pm.data()[..4], &[10, 20, 30, 255]);

        let alpha_img = image::RgbaImage::from_pixel(4, 4, image::Rgba([200, 100, 50, 128]));
        let (pm, opaque) = premultiply_rgba(&alpha_img).unwrap();
        assert!(!opaque);
        assert_eq!(
            &pm.data()[..4],
            &[
                (200u32 * 128 / 255) as u8,
                (100u32 * 128 / 255) as u8,
                (50u32 * 128 / 255) as u8,
                128
            ]
        );
    }
}
