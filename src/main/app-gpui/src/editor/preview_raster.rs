use std::cell::{Cell, RefCell};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use gpui::RenderImage;
use herogpui::gpui;
use tiny_skia::Pixmap;

use crate::editor::annotations::{
    arrow_curve_control, arrow_head_size, number_size_config, Annotation,
};
use crate::render::annotations as renderer;
use crate::render::canvas::Canvas;

const MAX_PATCH_PIXELS: f64 = 8_388_608.0;
const MAX_PATCH_DIMENSION: f64 = 16_384.0;
const CACHE_TTL_FRAMES: u64 = 4;

#[derive(Clone, Copy)]
pub struct Surface<'a> {
    pub image_id: Option<gpui::ImageId>,
    pub width: u32,
    pub height: u32,
    pub offset_x: f64,
    pub offset_y: f64,
    pub native_scale: f64,
    pub pixels: Option<&'a [u8]>,
    pub matte: Option<[u8; 3]>,
    pub matte_image: Option<(&'a RenderImage, f64, f64)>,
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
            | Annotation::Rectangle { .. }
            | Annotation::Circle { .. }
            | Annotation::Line { .. }
            | Annotation::Arrow { .. }
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
        Annotation::Rectangle { stroke_width, .. }
        | Annotation::Circle { stroke_width, .. }
        | Annotation::Line { stroke_width, .. } => {
            Some(padded(annotation.bounds(), stroke_width / 2.0 + 2.0))
        }
        Annotation::Arrow {
            points,
            stroke_width,
            arrow_style,
            bend_offset,
            ..
        } => {
            let mut bounds = annotation.bounds();
            if let Some((x, y)) = arrow_curve_control(points, arrow_style.as_deref(), *bend_offset)
            {
                bounds = (
                    bounds.0.min(x),
                    bounds.1.min(y),
                    bounds.2.max(x),
                    bounds.3.max(y),
                );
            }
            Some(padded(
                bounds,
                arrow_head_size(*stroke_width) + stroke_width / 2.0 + 2.0,
            ))
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
        Annotation::Redact {
            style, intensity, ..
        } => {
            let padding = if style == "blur" {
                crate::editor::annotations::redact_intensity(*intensity).1 * 2.0 + 2.0
            } else {
                1.0
            };
            Some(padded(annotation.bounds(), padding))
        }
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
    let bounds = raster_bounds(annotation)?;
    let target = if matches!(annotation, Annotation::Redact { .. }) {
        annotation.bounds()
    } else {
        bounds
    };
    let mut sample = bounds;
    loop {
        let previous = sample;
        for earlier in beneath {
            if !matches!(earlier, Annotation::Redact { .. }) {
                continue;
            }
            let Some(other) = raster_bounds(earlier) else {
                continue;
            };
            if sample.0 < other.2 && sample.2 > other.0 && sample.1 < other.3 && sample.3 > other.1
            {
                sample = (
                    sample.0.min(other.0),
                    sample.1.min(other.1),
                    sample.2.max(other.2),
                    sample.3.max(other.3),
                );
            }
        }
        if sample == previous {
            break;
        }
    }
    let left = sample.0.max(-surface.offset_x);
    let top = sample.1.max(-surface.offset_y);
    let right = sample.2.min(f64::from(surface.width) - surface.offset_x);
    let bottom = sample.3.min(f64::from(surface.height) - surface.offset_y);
    if right <= left || bottom <= top {
        return None;
    }

    let target = (
        target.0.max(left),
        target.1.max(top),
        target.2.min(right),
        target.3.min(bottom),
    );
    if target.2 <= target.0 || target.3 <= target.1 {
        return None;
    }
    let scale = fit_scale(right - left, bottom - top, scale)
        .min((MAX_PATCH_DIMENSION - 1.0) / (target.2 - target.0))
        .min((MAX_PATCH_DIMENSION - 1.0) / (target.3 - target.1));
    let device_x = (left * scale).floor();
    let device_y = (top * scale).floor();
    let width = ((right * scale).ceil() - device_x).max(1.0) as u32;
    let height = ((bottom * scale).ceil() - device_y).max(1.0) as u32;

    let mut canvas = Canvas::new(width, height)?;
    canvas.translate(-device_x as f32, -device_y as f32);
    canvas.scale(scale as f32, scale as f32);

    if let Some(pixels) = surface.pixels {
        draw_backdrop(
            &mut canvas,
            surface,
            pixels,
            (left, top, right, bottom),
            tiny_skia::BlendMode::SourceOver,
        );
        for earlier in beneath {
            renderer::draw_screenshot(&mut canvas, earlier, surface.native_scale);
        }
    }
    renderer::draw_screenshot(&mut canvas, annotation, surface.native_scale);
    if let Some((image, offset_x, offset_y)) = surface.matte_image {
        if let Some(pixels) = image.as_bytes(0) {
            let background = Surface {
                image_id: Some(image.id),
                width: image.size(0).width.into(),
                height: image.size(0).height.into(),
                offset_x,
                offset_y,
                native_scale: surface.native_scale,
                pixels: Some(pixels),
                matte: None,
                matte_image: None,
            };
            draw_backdrop(
                &mut canvas,
                &background,
                pixels,
                (left, top, right, bottom),
                tiny_skia::BlendMode::DestinationOver,
            );
        }
    }

    let target_left = (target.0 * scale).floor().max(device_x);
    let target_top = (target.1 * scale).floor().max(device_y);
    let target_right = (target.2 * scale).ceil().min(device_x + f64::from(width));
    let target_bottom = (target.3 * scale).ceil().min(device_y + f64::from(height));
    let target_width = (target_right - target_left) as u32;
    let target_height = (target_bottom - target_top) as u32;
    let rect = tiny_skia::IntRect::from_xywh(
        (target_left - device_x) as i32,
        (target_top - device_y) as i32,
        target_width,
        target_height,
    )?;
    let mut pixmap = canvas.into_pixmap().clone_rect(rect)?;
    if let Some([red, green, blue]) = surface.matte {
        let mut paint = tiny_skia::Paint::default();
        paint.set_color_rgba8(red, green, blue, 255);
        paint.blend_mode = tiny_skia::BlendMode::DestinationOver;
        pixmap.fill_rect(
            tiny_skia::Rect::from_xywh(0., 0., target_width as f32, target_height as f32)?,
            &paint,
            tiny_skia::Transform::identity(),
            None,
        );
    }
    Some(Patch {
        pixmap,
        x: target_left / scale,
        y: target_top / scale,
        width: f64::from(target_width) / scale,
        height: f64::from(target_height) / scale,
    })
}

fn draw_backdrop(
    canvas: &mut Canvas,
    surface: &Surface,
    pixels: &[u8],
    bounds: (f64, f64, f64, f64),
    blend: tiny_skia::BlendMode,
) {
    let (left, top, right, bottom) = bounds;
    let x0 = ((left + surface.offset_x) as u32).saturating_sub(1);
    let y0 = ((top + surface.offset_y) as u32).saturating_sub(1);
    let x1 = ((right + surface.offset_x).ceil() as u32 + 1).min(surface.width);
    let y1 = ((bottom + surface.offset_y).ceil() as u32 + 1).min(surface.height);
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
    canvas.draw_pixmap_blended(
        crop.as_ref(),
        (f64::from(x0) - surface.offset_x) as f32,
        (f64::from(y0) - surface.offset_y) as f32,
        crop_width as f32,
        crop_height as f32,
        blend,
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
    let scale = if matches!(annotation, Annotation::Redact { style, .. } if style == "pixelate") {
        1.0
    } else {
        scale
    };
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
    hasher.write_u64(surface.offset_x.to_bits());
    hasher.write_u64(surface.offset_y.to_bits());
    hasher.write_u64(surface.native_scale.to_bits());
    hasher.write_u8(u8::from(surface.pixels.is_some()));
    surface.matte.hash(&mut hasher);
    surface.image_id.hash(&mut hasher);
    if let Some((image, x, y)) = surface.matte_image {
        image.id.hash(&mut hasher);
        hasher.write_u64(x.to_bits());
        hasher.write_u64(y.to_bits());
    }
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
            image_id: None,
            width: base.width(),
            height: base.height(),
            offset_x: 0.0,
            offset_y: 0.0,
            native_scale: 1.,
            pixels: Some(&bgra),
            matte: None,
            matte_image: None,
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
    fn cached_shapes_keep_each_editors_capture_pixels() {
        let mut first = base_pixmap(180, 140);
        let mut second = first.clone();
        first.fill(tiny_skia::Color::from_rgba8(20, 40, 80, 255));
        second.fill(tiny_skia::Color::from_rgba8(180, 200, 220, 255));
        let first = render_image(&first);
        let second = render_image(&second);
        let annotation = Annotation::Rectangle {
            id: "ann-1".into(),
            x: 20.,
            y: 20.,
            width: 80.,
            height: 60.,
            stroke: "#f43f5e".into(),
            stroke_width: 3.,
            fill: None,
        };
        let first_surface = Surface {
            image_id: Some(first.id),
            width: 180,
            height: 140,
            offset_x: 0.0,
            offset_y: 0.0,
            native_scale: 1.,
            pixels: first.as_bytes(0),
            matte: None,
            matte_image: None,
        };
        let second_surface = Surface {
            image_id: Some(second.id),
            pixels: second.as_bytes(0),
            ..first_surface
        };
        let keys = begin_frame(&[&annotation]);
        let first_patch = image(&keys, 0, &annotation, &[], &first_surface, 1.).unwrap();
        let repeated = image(&keys, 0, &annotation, &[], &first_surface, 1.).unwrap();
        let second_patch = image(&keys, 0, &annotation, &[], &second_surface, 1.).unwrap();
        assert!(Arc::ptr_eq(&first_patch.image, &repeated.image));
        assert_ne!(
            first_patch.image.as_bytes(0),
            second_patch.image.as_bytes(0)
        );
    }

    #[test]
    fn offset_layered_surface_previews_annotations_and_redactions() {
        let base = base_pixmap(180, 140);
        let annotations = vec![
            Annotation::Rectangle {
                id: "edge".into(),
                x: -10.,
                y: 20.,
                width: 55.,
                height: 35.,
                stroke: "#ff00ff".into(),
                stroke_width: 2.,
                fill: Some("#ff00ff".into()),
            },
            Annotation::Redact {
                id: "pixelate".into(),
                x: 25.,
                y: 25.,
                width: 35.,
                height: 30.,
                style: "pixelate".into(),
                intensity: 8.,
            },
            Annotation::Redact {
                id: "blur".into(),
                x: 75.,
                y: 50.,
                width: 32.,
                height: 28.,
                style: "blur".into(),
                intensity: 2.,
            },
            Annotation::Redact {
                id: "clipped".into(),
                x: -56.,
                y: -24.,
                width: 35.,
                height: 35.,
                style: "pixelate".into(),
                intensity: 5.,
            },
        ];
        let bgra = to_bgra(&base);
        let surface = Surface {
            image_id: None,
            width: base.width(),
            height: base.height(),
            offset_x: 48.,
            offset_y: 20.,
            native_scale: 1.,
            pixels: Some(&bgra),
            matte: None,
            matte_image: None,
        };
        let paint = PixmapPaint {
            opacity: 1.0,
            blend_mode: BlendMode::Source,
            quality: FilterQuality::Nearest,
        };
        let refs: Vec<&Annotation> = annotations.iter().collect();
        for scale in [1., 1.25, 2.] {
            let mut exported = Canvas::new((180. * scale) as u32, (140. * scale) as u32).unwrap();
            exported.scale(scale as f32, scale as f32);
            exported.draw_pixmap(base.as_ref(), 0., 0., 180., 140.);
            let mut preview = exported.pixmap().clone();
            exported.translate(surface.offset_x as f32, surface.offset_y as f32);
            renderer::draw_all_screenshot(&mut exported, &annotations, surface.native_scale);
            let exported = exported.into_pixmap();
            for (index, annotation) in annotations.iter().enumerate() {
                let patch = patch(annotation, &refs[..index], &surface, scale).unwrap();
                preview.draw_pixmap(
                    ((patch.x + surface.offset_x) * scale).round() as i32,
                    ((patch.y + surface.offset_y) * scale).round() as i32,
                    patch.pixmap.as_ref(),
                    &paint,
                    Transform::identity(),
                    None,
                );
            }
            for (index, (actual, expected)) in
                preview.data().iter().zip(exported.data()).enumerate()
            {
                assert!(
                    actual.abs_diff(*expected) <= 1,
                    "channel {index} at {scale}x differs: {actual} vs {expected}"
                );
            }
        }
    }

    #[test]
    fn offset_pixelation_matches_export_at_block_boundaries() {
        let source = image::RgbaImage::from_fn(260, 120, |x, y| {
            if x < 120 {
                image::Rgba([240, 80, 64, 255])
            } else if (130..250).contains(&x) && (10..110).contains(&y) {
                image::Rgba([64, 112, 192, 255])
            } else {
                image::Rgba([230, 230, 230, 255])
            }
        });
        let base = crate::editor::export::from_rgba(&source).unwrap();
        let bgra = to_bgra(&base);
        let surface = Surface {
            image_id: None,
            width: 260,
            height: 120,
            offset_x: 90.,
            offset_y: -20.,
            native_scale: 1.,
            pixels: Some(&bgra),
            matte: None,
            matte_image: None,
        };
        let annotation = Annotation::Redact {
            id: "left".into(),
            x: 15.,
            y: 80.,
            width: 30.,
            height: 20.,
            style: "pixelate".into(),
            intensity: 5.,
        };
        let patch = patch(&annotation, &[], &surface, 1.).unwrap();
        let mut expected = Canvas::new(260, 120).unwrap();
        expected.draw_pixmap(base.as_ref(), 0., 0., 260., 120.);
        expected.translate(90., -20.);
        renderer::draw_screenshot(&mut expected, &annotation, surface.native_scale);
        let expected = expected.into_pixmap();
        let mut actual = base.clone();
        actual.draw_pixmap(
            (patch.x + surface.offset_x) as i32,
            (patch.y + surface.offset_y) as i32,
            patch.pixmap.as_ref(),
            &PixmapPaint {
                opacity: 1.,
                blend_mode: BlendMode::Source,
                quality: FilterQuality::Nearest,
            },
            Transform::identity(),
            None,
        );
        for (index, (actual, expected)) in actual.data().iter().zip(expected.data()).enumerate() {
            assert!(
                actual.abs_diff(*expected) <= 1,
                "channel {index}: {actual} vs {expected}"
            );
        }
    }

    #[test]
    fn translucent_redaction_uses_the_export_alpha_before_the_matte() {
        let mut base = base_pixmap(100, 80);
        for y in 0..80 {
            for x in 50..100 {
                let index = (y * 100 + x) * 4;
                base.data_mut()[index..index + 4].copy_from_slice(&[20, 95, 40, 128]);
            }
        }
        let source = render_image(&base);
        let annotation = Annotation::Redact {
            id: "translucent-blur".into(),
            x: 40.,
            y: 20.,
            width: 30.,
            height: 30.,
            style: "blur".into(),
            intensity: 2.,
        };
        let surface = Surface {
            image_id: Some(source.id),
            width: 100,
            height: 80,
            offset_x: 0.,
            offset_y: 0.,
            native_scale: 1.,
            pixels: source.as_bytes(0),
            matte: Some([240, 240, 240]),
            matte_image: None,
        };
        let mut exported = Canvas::from_pixmap(base);
        renderer::draw_screenshot(&mut exported, &annotation, surface.native_scale);
        let mut exported = exported.into_pixmap();
        let mut paint = tiny_skia::Paint::default();
        paint.set_color_rgba8(240, 240, 240, 255);
        paint.blend_mode = BlendMode::DestinationOver;
        exported.fill_rect(
            tiny_skia::Rect::from_xywh(0., 0., 100., 80.).unwrap(),
            &paint,
            Transform::identity(),
            None,
        );
        let actual = patch(&annotation, &[], &surface, 1.).unwrap();
        for y in 0..30 {
            for x in 0..30 {
                let expected = exported.pixel(x + 40, y + 20).unwrap();
                let actual = actual.pixmap.pixel(x, y).unwrap();
                assert!(actual.red().abs_diff(expected.red()) <= 1);
                assert!(actual.green().abs_diff(expected.green()) <= 1);
                assert!(actual.blue().abs_diff(expected.blue()) <= 1);
                assert_eq!(actual.alpha(), 255);
            }
        }
        let keys = begin_frame(&[&annotation]);
        let light = image(&keys, 0, &annotation, &[], &surface, 1.).unwrap();
        let dark_surface = Surface {
            matte: Some([24, 24, 27]),
            ..surface
        };
        let dark = image(&keys, 0, &annotation, &[], &dark_surface, 1.).unwrap();
        assert_ne!(light.image.id, dark.image.id);
    }

    #[test]
    fn highlight_previews_as_it_exports() {
        let base = base_pixmap(140, 110);
        assert_matches(&base, &[highlight()], 1.0);
    }

    #[test]
    fn shapes_preview_as_they_export_at_each_dpi_scale() {
        let base = base_pixmap(180, 140);
        let mut shapes = vec![
            Annotation::Rectangle {
                id: "rectangle".into(),
                x: 110.5,
                y: 95.5,
                width: -90.,
                height: -65.,
                stroke: "#f43f5e".into(),
                stroke_width: 3.,
                fill: Some("#40a08080".into()),
            },
            Annotation::Circle {
                id: "circle".into(),
                x: 85.5,
                y: 65.5,
                radius: 35.,
                stroke: "#f43f5e".into(),
                stroke_width: 3.,
                fill: None,
            },
            Annotation::Line {
                id: "line".into(),
                points: [10.5, 20.5, 150.5, 100.5],
                stroke: "#f43f5e".into(),
                stroke_width: 2.5,
            },
        ];
        for style in ["standard", "double", "curved", "double-curved"] {
            for bend_offset in [
                None,
                Some(crate::editor::annotations::Offset { x: 0., y: -100. }),
            ] {
                shapes.push(Annotation::Arrow {
                    id: format!("arrow-{style}-{bend_offset:?}"),
                    points: [15., 75., 155., 85.],
                    stroke: "#f43f5e".into(),
                    stroke_width: 4.,
                    arrow_style: Some(style.into()),
                    bend_offset,
                });
            }
        }
        for scale in [1., 1.25, 1.5, 2.] {
            for shape in &shapes {
                assert_matches(&base, std::slice::from_ref(shape), scale);
            }
            assert_matches(&base, &shapes, scale);
        }
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

    #[test]
    fn long_patch_dimensions_fit_the_gpu_without_shrinking_small_targets() {
        for (width, height) in [(20000., 8.), (8., 20000.)] {
            let surface = Surface {
                image_id: None,
                width: width as u32,
                height: height as u32,
                offset_x: 0.,
                offset_y: 0.,
                native_scale: 1.,
                pixels: None,
                matte: None,
                matte_image: None,
            };
            let annotation = Annotation::Rectangle {
                id: "long".into(),
                x: 0.25,
                y: 0.25,
                width: width - 1.,
                height: height - 1.,
                stroke: "#ff00ff".into(),
                stroke_width: 1.,
                fill: None,
            };
            let image = patch(&annotation, &[], &surface, 1.).unwrap().pixmap;
            assert!(image.width() <= 16384 && image.height() <= 16384);
            assert!(image.width().max(image.height()) > 16000);
        }
        let surface = Surface {
            image_id: None,
            width: 20000,
            height: 20,
            offset_x: 0.,
            offset_y: 0.,
            native_scale: 1.,
            pixels: None,
            matte: None,
            matte_image: None,
        };
        let preceding = Annotation::Redact {
            id: "long-redaction".into(),
            x: 0.,
            y: 0.,
            width: 20000.,
            height: 20.,
            style: "black".into(),
            intensity: 1.,
        };
        let current = Annotation::Redact {
            id: "small-redaction".into(),
            x: 10.,
            y: 4.,
            width: 10.,
            height: 10.,
            style: "black".into(),
            intensity: 1.,
        };
        let image = patch(&current, &[&preceding], &surface, 1.).unwrap().pixmap;
        assert_eq!((image.width(), image.height()), (10, 10));
    }
}
