use std::cell::{Cell, RefCell};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::hash::Hasher;
use std::sync::Arc;

use gpui::RenderImage;
use herogpui::gpui;
use tiny_skia::Pixmap;

use crate::editor::annotations::{number_size_config, Annotation};
use crate::render::annotations as renderer;
use crate::render::canvas::Canvas;

const MAX_PATCH_PIXELS: f64 = 8_388_608.0;
const CACHE_TTL_FRAMES: u64 = 4;

pub struct Surface<'a> {
    pub width: u32,
    pub height: u32,
    pub pixels: Option<&'a [u8]>,
}

pub struct Patch {
    pub pixmap: Pixmap,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone)]
pub struct PatchImage {
    pub image: Arc<RenderImage>,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

pub fn is_rasterized(annotation: &Annotation) -> bool {
    matches!(
        annotation,
        Annotation::Pen { .. }
            | Annotation::Highlight { .. }
            | Annotation::Number { .. }
            | Annotation::Text { .. }
    )
}

fn padded(bounds: (f64, f64, f64, f64), padding: f64) -> (f64, f64, f64, f64) {
    (
        bounds.0 - padding,
        bounds.1 - padding,
        bounds.2 + padding,
        bounds.3 + padding,
    )
}

fn raster_bounds(annotation: &Annotation) -> Option<(f64, f64, f64, f64)> {
    match annotation {
        Annotation::Pen { stroke_width, .. } => {
            Some(padded(annotation.bounds(), stroke_width * 2.0 + 2.0))
        }
        Annotation::Highlight { stroke_width, .. } => {
            Some(padded(annotation.bounds(), stroke_width / 2.0 + 2.0))
        }
        Annotation::Number { size, .. } => {
            let (_, font_size) = number_size_config(size);
            Some(padded(annotation.bounds(), font_size))
        }
        Annotation::Text { font_size, .. } => {
            let text_box = annotation.text_box()?;
            let radians = text_box.rotation.to_radians();
            let (sin, cos) = (radians.sin().abs(), radians.cos().abs());
            let width = text_box.width * cos + text_box.height * sin;
            let height = text_box.width * sin + text_box.height * cos;
            Some(padded(
                (
                    text_box.center_x - width / 2.0,
                    text_box.center_y - height / 2.0,
                    text_box.center_x + width / 2.0,
                    text_box.center_y + height / 2.0,
                ),
                font_size / 2.0 + 4.0,
            ))
        }
        _ => None,
    }
}

fn fit_scale(width: f64, height: f64, scale: f64) -> f64 {
    let pixels = width * scale * height * scale;
    if pixels <= MAX_PATCH_PIXELS {
        return scale;
    }
    scale * (MAX_PATCH_PIXELS / pixels).sqrt()
}

pub fn patch(
    annotation: &Annotation,
    beneath: &[&Annotation],
    surface: &Surface,
    scale: f64,
) -> Option<Patch> {
    if scale <= 0.0 {
        return None;
    }
    let (left, top, right, bottom) = raster_bounds(annotation)?;
    let left = left.max(0.0);
    let top = top.max(0.0);
    let right = right.min(f64::from(surface.width));
    let bottom = bottom.min(f64::from(surface.height));
    if right <= left || bottom <= top {
        return None;
    }

    let scale = fit_scale(right - left, bottom - top, scale);
    let device_x = (left * scale).floor();
    let device_y = (top * scale).floor();
    let width = ((right * scale).ceil() - device_x).max(1.0) as u32;
    let height = ((bottom * scale).ceil() - device_y).max(1.0) as u32;

    let mut canvas = Canvas::new(width, height)?;
    canvas.translate(-device_x as f32, -device_y as f32);
    canvas.scale(scale as f32, scale as f32);

    if let Some(pixels) = surface.pixels {
        draw_backdrop(&mut canvas, surface, pixels, left, top, right, bottom);
        for earlier in beneath {
            renderer::draw(&mut canvas, earlier);
        }
    }
    renderer::draw(&mut canvas, annotation);

    Some(Patch {
        pixmap: canvas.into_pixmap(),
        x: device_x / scale,
        y: device_y / scale,
        width: f64::from(width) / scale,
        height: f64::from(height) / scale,
    })
}

fn draw_backdrop(
    canvas: &mut Canvas,
    surface: &Surface,
    pixels: &[u8],
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
) {
    let x0 = (left as u32).saturating_sub(1);
    let y0 = (top as u32).saturating_sub(1);
    let x1 = (right.ceil() as u32 + 1).min(surface.width);
    let y1 = (bottom.ceil() as u32 + 1).min(surface.height);
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    let stride = surface.width as usize * 4;
    if pixels.len() < surface.height as usize * stride {
        return;
    }
    let (crop_width, crop_height) = (x1 - x0, y1 - y0);
    let Some(mut crop) = Pixmap::new(crop_width, crop_height) else {
        return;
    };
    let target = crop.data_mut();
    for row in 0..crop_height as usize {
        let from = (y0 as usize + row) * stride + x0 as usize * 4;
        let to = row * crop_width as usize * 4;
        for column in 0..crop_width as usize {
            let source = from + column * 4;
            let pixel = to + column * 4;
            let alpha = pixels[source + 3] as u32;
            let premultiply = |value: u8| ((value as u32 * alpha + 127) / 255) as u8;
            target[pixel] = premultiply(pixels[source + 2]);
            target[pixel + 1] = premultiply(pixels[source + 1]);
            target[pixel + 2] = premultiply(pixels[source]);
            target[pixel + 3] = alpha as u8;
        }
    }
    canvas.draw_pixmap(
        crop.as_ref(),
        x0 as f32,
        y0 as f32,
        crop_width as f32,
        crop_height as f32,
    );
}

struct Entry {
    patch: PatchImage,
    frame: u64,
}

thread_local! {
    static CACHE: RefCell<HashMap<u64, Entry>> = RefCell::new(HashMap::new());
    static FRAME: Cell<u64> = const { Cell::new(0) };
}

pub struct FrameKeys {
    hashes: Vec<u64>,
    prefixes: Vec<u64>,
}

pub fn begin_frame(visible: &[&Annotation]) -> FrameKeys {
    FRAME.with(|frame| frame.set(frame.get().wrapping_add(1)));
    let hashes: Vec<u64> = visible
        .iter()
        .map(|annotation| hash_of(annotation))
        .collect();
    let mut prefixes = Vec::with_capacity(hashes.len());
    let mut running = 0u64;
    for hash in &hashes {
        prefixes.push(running);
        running = running.rotate_left(7) ^ hash;
    }
    FrameKeys { hashes, prefixes }
}

pub fn end_frame(window: &mut gpui::Window) {
    let frame = FRAME.with(|frame| frame.get());
    let evicted: Vec<Arc<RenderImage>> = CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        let stale: Vec<u64> = cache
            .iter()
            .filter(|(_, entry)| frame.saturating_sub(entry.frame) >= CACHE_TTL_FRAMES)
            .map(|(key, _)| *key)
            .collect();
        stale
            .into_iter()
            .filter_map(|key| cache.remove(&key).map(|entry| entry.patch.image))
            .collect()
    });
    for image in evicted {
        let _ = window.drop_image(image);
    }
}

pub fn image(
    keys: &FrameKeys,
    index: usize,
    annotation: &Annotation,
    beneath: &[&Annotation],
    surface: &Surface,
    scale: f64,
) -> Option<PatchImage> {
    let key = fingerprint(keys, index, surface, scale)?;
    let frame = FRAME.with(|frame| frame.get());
    let cached = CACHE.with(|cache| {
        cache.borrow_mut().get_mut(&key).map(|entry| {
            entry.frame = frame;
            entry.patch.clone()
        })
    });
    if let Some(cached) = cached {
        return Some(cached);
    }

    let patch = patch(annotation, beneath, surface, scale)?;
    let value = PatchImage {
        image: render_image(&patch.pixmap),
        x: patch.x,
        y: patch.y,
        width: patch.width,
        height: patch.height,
    };
    CACHE.with(|cache| {
        cache.borrow_mut().insert(
            key,
            Entry {
                patch: value.clone(),
                frame,
            },
        )
    });
    Some(value)
}

struct Fingerprint(DefaultHasher);

impl std::fmt::Write for Fingerprint {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        self.0.write(text.as_bytes());
        Ok(())
    }
}

fn hash_of(annotation: &Annotation) -> u64 {
    let mut hasher = Fingerprint(DefaultHasher::new());
    let _ = write!(hasher, "{annotation:?}");
    hasher.0.finish()
}

fn fingerprint(keys: &FrameKeys, index: usize, surface: &Surface, scale: f64) -> Option<u64> {
    let mut hasher = DefaultHasher::new();
    hasher.write_u64(*keys.hashes.get(index)?);
    hasher.write_u64(*keys.prefixes.get(index)?);
    hasher.write_u64(scale.to_bits());
    hasher.write_u32(surface.width);
    hasher.write_u32(surface.height);
    hasher.write_u8(u8::from(surface.pixels.is_some()));
    Some(hasher.finish())
}

fn render_image(pixmap: &Pixmap) -> Arc<RenderImage> {
    let mut buffer = crate::editor::export::to_rgba(pixmap);
    for pixel in buffer.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    Arc::new(RenderImage::new(smallvec::smallvec![image::Frame::new(
        buffer
    )]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tiny_skia::{BlendMode, FilterQuality, PixmapPaint, Transform};

    fn base_pixmap(width: u32, height: u32) -> Pixmap {
        let mut pixmap = Pixmap::new(width, height).expect("base");
        for (index, pixel) in pixmap
            .data_mut()
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .enumerate()
        {
            let x = index as u32 % width;
            let y = index as u32 / width;
            *pixel = [
                (40 + x * 3 % 200) as u8,
                (90 + y * 5 % 150) as u8,
                (200 - (x + y) % 180) as u8,
                255,
            ];
        }
        pixmap
    }

    fn to_bgra(pixmap: &Pixmap) -> Vec<u8> {
        let mut bytes = pixmap.data().to_vec();
        for pixel in bytes.as_chunks_mut::<4>().0 {
            pixel.swap(0, 2);
        }
        bytes
    }

    fn export_raster(base: &Pixmap, annotations: &[Annotation], scale: f64) -> Pixmap {
        let width = (f64::from(base.width()) * scale).round() as u32;
        let height = (f64::from(base.height()) * scale).round() as u32;
        let mut canvas = Canvas::new(width, height).expect("export canvas");
        canvas.scale(scale as f32, scale as f32);
        canvas.draw_pixmap(
            base.as_ref(),
            0.0,
            0.0,
            base.width() as f32,
            base.height() as f32,
        );
        for annotation in annotations {
            renderer::draw(&mut canvas, annotation);
        }
        canvas.into_pixmap()
    }

    fn preview_raster(base: &Pixmap, annotations: &[Annotation], scale: f64) -> Pixmap {
        let width = (f64::from(base.width()) * scale).round() as u32;
        let height = (f64::from(base.height()) * scale).round() as u32;
        let mut canvas = Canvas::new(width, height).expect("preview canvas");
        canvas.scale(scale as f32, scale as f32);
        canvas.draw_pixmap(
            base.as_ref(),
            0.0,
            0.0,
            base.width() as f32,
            base.height() as f32,
        );
        let mut composed = canvas.into_pixmap();

        let bgra = to_bgra(base);
        let surface = Surface {
            width: base.width(),
            height: base.height(),
            pixels: Some(&bgra),
        };
        let paint = PixmapPaint {
            opacity: 1.0,
            blend_mode: BlendMode::Source,
            quality: FilterQuality::Nearest,
        };
        let refs: Vec<&Annotation> = annotations.iter().collect();
        let keys = begin_frame(&refs);
        for (index, annotation) in annotations.iter().enumerate() {
            assert!(is_rasterized(annotation));
            assert!(fingerprint(&keys, index, &surface, scale).is_some());
            let patch = patch(annotation, &refs[..index], &surface, scale).expect("patch");
            composed.draw_pixmap(
                (patch.x * scale).round() as i32,
                (patch.y * scale).round() as i32,
                patch.pixmap.as_ref(),
                &paint,
                Transform::identity(),
                None,
            );
        }
        composed
    }

    fn assert_matches(base: &Pixmap, annotations: &[Annotation], scale: f64) {
        let exported = export_raster(base, annotations, scale);
        let previewed = preview_raster(base, annotations, scale);
        assert_eq!(exported.width(), previewed.width());
        assert_eq!(exported.height(), previewed.height());
        for (index, (left, right)) in exported
            .data()
            .iter()
            .zip(previewed.data().iter())
            .enumerate()
        {
            let delta = (*left as i32 - *right as i32).abs();
            assert!(
                delta <= 1,
                "channel {index} differs by {delta} at scale {scale}"
            );
        }
    }

    fn pen(stroke_width: f64) -> Annotation {
        Annotation::Pen {
            id: "pen-1".into(),
            points: vec![
                12.0, 20.0, 24.0, 34.0, 41.0, 28.0, 58.0, 47.0, 74.0, 33.0, 92.0, 52.0,
            ],
            stroke: "#ff3b30".into(),
            stroke_width,
        }
    }

    fn highlight() -> Annotation {
        Annotation::Highlight {
            id: "highlight-1".into(),
            points: vec![10.0, 70.0, 40.0, 64.0, 70.0, 78.0, 100.0, 66.0],
            fill: "#ffd60a".into(),
            opacity: 0.4,
            stroke_width: 18.0,
        }
    }

    fn number() -> Annotation {
        Annotation::Number {
            id: "number-1".into(),
            x: 60.0,
            y: 40.0,
            value: 3.0,
            display_value: "3".into(),
            fill: "#0a84ff".into(),
            size: "medium".into(),
        }
    }

    fn text(rotation: Option<f64>) -> Annotation {
        Annotation::Text {
            id: "text-1".into(),
            x: 18.0,
            y: 30.0,
            text: "Preview".into(),
            fill: "#ffffff".into(),
            font_size: 22.0,
            font_family: Some("mono".into()),
            background_color: Some("#1c1c1e".into()),
            background_opacity: None,
            background_padding: None,
            background_radius: None,
            rotation,
        }
    }

    #[test]
    fn pen_previews_as_it_exports() {
        let base = base_pixmap(140, 110);
        assert_matches(&base, &[pen(6.0)], 1.0);
        assert_matches(&base, &[pen(2.0)], 1.0);
    }

    #[test]
    fn highlight_previews_as_it_exports() {
        let base = base_pixmap(140, 110);
        assert_matches(&base, &[highlight()], 1.0);
    }

    #[test]
    fn overlapping_highlights_preview_as_they_export() {
        let base = base_pixmap(140, 110);
        let mut second = highlight();
        if let Annotation::Highlight { id, points, .. } = &mut second {
            *id = "highlight-2".into();
            *points = vec![20.0, 60.0, 50.0, 80.0, 80.0, 62.0, 110.0, 84.0];
        }
        assert_matches(&base, &[highlight(), second], 1.0);
    }

    #[test]
    fn number_badge_previews_as_it_exports() {
        let base = base_pixmap(140, 110);
        assert_matches(&base, &[number()], 1.0);
    }

    #[test]
    fn text_previews_as_it_exports() {
        if !crate::editor::text_render::is_available() {
            return;
        }
        let base = base_pixmap(180, 110);
        assert_matches(&base, &[text(None)], 1.0);
        assert_matches(&base, &[text(Some(24.0))], 1.0);
    }

    #[test]
    fn annotations_match_at_a_zoomed_scale() {
        if !crate::editor::text_render::is_available() {
            return;
        }
        let base = base_pixmap(140, 110);
        assert_matches(&base, &[pen(6.0)], 2.0);
        assert_matches(&base, &[highlight()], 2.0);
        assert_matches(&base, &[number()], 2.0);
        assert_matches(&base, &[text(None)], 2.0);
        assert_matches(&base, &[pen(6.0), highlight(), number(), text(None)], 2.0);
    }

    #[test]
    fn a_patch_never_exceeds_the_pixel_budget() {
        let scale = fit_scale(8000.0, 8000.0, 4.0);
        assert!(8000.0 * scale * 8000.0 * scale <= MAX_PATCH_PIXELS + 1.0);
        assert_eq!(fit_scale(100.0, 100.0, 2.0), 2.0);
    }
}
