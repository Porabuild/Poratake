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
use crate::render::annotations::ScreenshotPaint;
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
    pub composite_backdrop: bool,
}

impl Surface<'_> {
    fn bounds(&self) -> (f64, f64, f64, f64) {
        if let Some((image, x, y)) = self.matte_image.filter(|_| self.composite_backdrop) {
            let size = image.size(0);
            return (
                -x,
                -y,
                f64::from(u32::from(size.width)) - x,
                f64::from(u32::from(size.height)) - y,
            );
        }
        (
            -self.offset_x,
            -self.offset_y,
            f64::from(self.width) - self.offset_x,
            f64::from(self.height) - self.offset_y,
        )
    }
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

#[derive(Clone, Copy)]
pub struct SamplingGrid {
    pub origin: (f32, f32),
    pub stage_bounds: (f32, f32, f32, f32),
    pub stage_size: (f64, f64),
    pub offset: (f64, f64),
    pub zoom: f32,
    pub dpi: f32,
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

fn raster_bounds(
    annotation: &Annotation,
    surface: &Surface,
    scale: f64,
) -> Option<(f64, f64, f64, f64)> {
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
            if style != "blur" {
                return Some(padded(annotation.bounds(), 1.0));
            }
            let padding = (crate::editor::annotations::redact_intensity(*intensity).1
                * surface.native_scale)
                .round()
                .max(1.0)
                * 2.0
                + 2.0 / scale;
            let target = annotation.bounds();
            let mut bounds = padded(target, padding);
            let (left_limit, top_limit, right_limit, bottom_limit) = surface.bounds();
            let left = target.0.floor().max(left_limit);
            let top = target.1.floor().max(top_limit);
            let right = target.2.ceil().min(right_limit);
            let bottom = target.3.ceil().min(bottom_limit);
            if right > left {
                let source_left = (left - padding).max(left_limit);
                bounds.2 = bounds
                    .2
                    .max((source_left + right - left + padding * 2.0).min(right_limit));
            }
            if bottom > top {
                let source_top = (top - padding).max(top_limit);
                bounds.3 = bounds
                    .3
                    .max((source_top + bottom - top + padding * 2.0).min(bottom_limit));
            }
            Some(bounds)
        }
    }
}

pub fn overlaps_redaction(
    annotation: &Annotation,
    beneath: &[ScreenshotPaint<'_>],
    surface: &Surface,
) -> bool {
    let Some(bounds) = raster_bounds(annotation, surface, 1.) else {
        return false;
    };
    beneath.iter().any(|earlier| {
        let earlier = earlier.annotation();
        if !matches!(earlier, Annotation::Redact { .. }) {
            return false;
        }
        let redaction = padded(earlier.bounds(), 2.);
        bounds.0 < redaction.2
            && bounds.2 > redaction.0
            && bounds.1 < redaction.3
            && bounds.3 > redaction.1
    })
}

fn fit_scale(width: f64, height: f64, scale: f64) -> f64 {
    let pixels = width * scale * height * scale;
    if pixels <= MAX_PATCH_PIXELS {
        return scale;
    }
    scale * (MAX_PATCH_PIXELS / pixels).sqrt()
}

pub fn patch(
    paint: ScreenshotPaint<'_>,
    beneath: &[ScreenshotPaint<'_>],
    surface: &Surface,
    scale: f64,
) -> Option<Patch> {
    patch_padded(paint, beneath, surface, scale, 0.)
}

fn patch_padded(
    paint: ScreenshotPaint<'_>,
    beneath: &[ScreenshotPaint<'_>],
    surface: &Surface,
    scale: f64,
    padding: f64,
) -> Option<Patch> {
    if scale <= 0.0 {
        return None;
    }
    let (surface_left, surface_top, surface_right, surface_bottom) = surface.bounds();
    let (surface_width, surface_height) =
        (surface_right - surface_left, surface_bottom - surface_top);
    let minimum_scale = fit_scale(surface_width, surface_height, scale)
        .min((MAX_PATCH_DIMENSION - 1.0) / surface_width)
        .min((MAX_PATCH_DIMENSION - 1.0) / surface_height);
    let annotation = paint.annotation();
    let bounds = raster_bounds(annotation, surface, minimum_scale)?;
    let target = if matches!(annotation, Annotation::Redact { .. }) {
        annotation.bounds()
    } else {
        bounds
    };
    let target = padded(target, padding);
    let mut sample = (
        bounds.0.min(target.0),
        bounds.1.min(target.1),
        bounds.2.max(target.2),
        bounds.3.max(target.3),
    );
    loop {
        let previous = sample;
        for earlier in beneath {
            let earlier = earlier.annotation();
            if !matches!(earlier, Annotation::Redact { .. }) {
                continue;
            }
            let Some(other) = raster_bounds(earlier, surface, minimum_scale) else {
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
    let left = sample.0.max(surface_left);
    let top = sample.1.max(surface_top);
    let right = sample.2.min(surface_right);
    let bottom = sample.3.min(surface_bottom);
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
    let composition_origin = surface
        .matte_image
        .filter(|_| surface.composite_backdrop)
        .map(|(_, x, y)| (x, y))
        .unwrap_or((surface.offset_x, surface.offset_y));
    canvas.set_patch_path_anchor(
        (
            (composition_origin.0 * scale) as f32,
            (composition_origin.1 * scale) as f32,
        ),
        (device_x as f32, device_y as f32),
    );

    let draw_matte = |canvas: &mut Canvas| {
        let Some((image, offset_x, offset_y)) = surface.matte_image else {
            return;
        };
        let Some(pixels) = image.as_bytes(0) else {
            return;
        };
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
            composite_backdrop: false,
        };
        draw_backdrop(
            canvas,
            &background,
            pixels,
            (left, top, right, bottom),
            tiny_skia::BlendMode::DestinationOver,
        );
    };
    if let Some(pixels) = surface.pixels {
        draw_backdrop(
            &mut canvas,
            surface,
            pixels,
            (left, top, right, bottom),
            tiny_skia::BlendMode::SourceOver,
        );
    }
    if surface.composite_backdrop {
        draw_matte(&mut canvas);
    }
    if surface.pixels.is_some() {
        for earlier in beneath {
            renderer::draw_screenshot_paint(&mut canvas, *earlier, surface.native_scale);
        }
    }
    renderer::draw_screenshot_paint(&mut canvas, paint, surface.native_scale);
    if !surface.composite_backdrop {
        draw_matte(&mut canvas);
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

pub fn begin_frame(visible: &[ScreenshotPaint<'_>]) -> FrameKeys {
    FRAME.with(|frame| frame.set(frame.get().wrapping_add(1)));
    let hashes: Vec<u64> = visible.iter().map(|paint| hash_of(paint)).collect();
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
    paint: ScreenshotPaint<'_>,
    beneath: &[ScreenshotPaint<'_>],
    surface: &Surface,
    scale: f64,
) -> Option<PatchImage> {
    let scale = if matches!(paint.annotation(), Annotation::Redact { style, .. } if style == "pixelate")
    {
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

    let patch = patch(paint, beneath, surface, scale)?;
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

pub fn aligned_image(
    keys: &FrameKeys,
    index: usize,
    paint: ScreenshotPaint<'_>,
    beneath: &[ScreenshotPaint<'_>],
    surface: &Surface,
    grid: SamplingGrid,
) -> Option<PatchImage> {
    surface.pixels?;
    let snap = |value: f32| {
        let value = value * grid.dpi;
        (value.abs() - 0.5).ceil().copysign(value) as f64
    };
    let mut hasher = DefaultHasher::new();
    hasher.write_u64(fingerprint(keys, index, surface, 1.)?);
    hasher.write_u8(1);
    hasher.write_u64((snap(grid.stage_bounds.0) - snap(grid.origin.0)).to_bits());
    hasher.write_u64((snap(grid.stage_bounds.1) - snap(grid.origin.1)).to_bits());
    hasher.write_u64(
        (snap(grid.stage_bounds.0 + grid.stage_bounds.2) - snap(grid.stage_bounds.0)).to_bits(),
    );
    hasher.write_u64(
        (snap(grid.stage_bounds.1 + grid.stage_bounds.3) - snap(grid.stage_bounds.1)).to_bits(),
    );
    for value in [
        grid.stage_bounds.2,
        grid.stage_bounds.3,
        grid.zoom,
        grid.dpi,
    ] {
        hasher.write_u32(value.to_bits());
    }
    for value in [
        grid.stage_size.0,
        grid.stage_size.1,
        grid.offset.0,
        grid.offset.1,
    ] {
        hasher.write_u64(value.to_bits());
    }
    let key = hasher.finish();
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
    let stage_x = grid.stage_bounds.0;
    let stage_y = grid.stage_bounds.1;
    let left = snap(stage_x);
    let top = snap(stage_y);
    let scale_x = (snap(stage_x + grid.stage_bounds.2) - left) / grid.stage_size.0;
    let scale_y = (snap(stage_y + grid.stage_bounds.3) - top) / grid.stage_size.1;
    if scale_x <= 0. || scale_y <= 0. {
        return None;
    }
    let annotation = paint.annotation();
    let bounds = raster_bounds(annotation, surface, 1.)?;
    let target = if matches!(annotation, Annotation::Redact { .. }) {
        annotation.bounds()
    } else {
        bounds
    };
    let target = padded(target, 1.);
    let (surface_left, surface_top, surface_right, surface_bottom) = surface.bounds();
    let target = (
        target.0.max(surface_left),
        target.1.max(surface_top),
        target.2.min(surface_right),
        target.3.min(surface_bottom),
    );
    if target.2 <= target.0 || target.3 <= target.1 {
        return None;
    }
    let x0 = (left + (target.0 + grid.offset.0) * scale_x).floor();
    let y0 = (top + (target.1 + grid.offset.1) * scale_y).floor();
    let x1 = (left + (target.2 + grid.offset.0) * scale_x).ceil();
    let y1 = (top + (target.3 + grid.offset.1) * scale_y).ceil();
    let (width, height) = ((x1 - x0) as u32, (y1 - y0) as u32);
    if width == 0
        || height == 0
        || f64::from(width) > MAX_PATCH_DIMENSION
        || f64::from(height) > MAX_PATCH_DIMENSION
        || f64::from(width) * f64::from(height) > MAX_PATCH_PIXELS
    {
        return None;
    }
    let guard = (1. / scale_x.min(scale_y)).ceil() + 2.;
    let raw_surface = Surface {
        matte: None,
        ..*surface
    };
    let patch = patch_padded(paint, beneath, &raw_surface, 1., guard)?;
    if f64::from(patch.pixmap.width()) != patch.width
        || f64::from(patch.pixmap.height()) != patch.height
    {
        return None;
    }
    let source = crate::editor::export::to_rgba(&patch.pixmap);
    let mut out = image::RgbaImage::new(width, height);
    let xs = (0..width)
        .map(|x| {
            let sx = (x0 + f64::from(x) + 0.5 - left) / scale_x - grid.offset.0 - patch.x - 0.5;
            let ix = sx.floor();
            let first = ix.max(0.).min(f64::from(source.width() - 1)) as usize;
            let second = (ix + 1.).max(0.).min(f64::from(source.width() - 1)) as usize;
            (first, second, (sx - ix) as f32)
        })
        .collect::<Vec<_>>();
    let stride = source.width() as usize;
    let source_pixels = source.as_raw().as_chunks::<4>().0;
    let output: &mut [u8] = out.as_mut();
    for (y, row) in output
        .as_chunks_mut::<4>()
        .0
        .chunks_exact_mut(width as usize)
        .enumerate()
    {
        let sy = (y0 + y as f64 + 0.5 - top) / scale_y - grid.offset.1 - patch.y - 0.5;
        let iy = sy.floor();
        let fy = (sy - iy) as f32;
        let first_y = iy.max(0.).min(f64::from(source.height() - 1)) as usize * stride;
        let second_y = (iy + 1.).max(0.).min(f64::from(source.height() - 1)) as usize * stride;
        for (pixel, &(first_x, second_x, fx)) in row.iter_mut().zip(&xs) {
            let pixels = [
                source_pixels[first_y + first_x],
                source_pixels[first_y + second_x],
                source_pixels[second_y + first_x],
                source_pixels[second_y + second_x],
            ];
            let mut sample = [0.0_f32; 4];
            for c in 0..4 {
                let a = f32::from(pixels[0][c]) * (1. - fx) + f32::from(pixels[1][c]) * fx;
                let b = f32::from(pixels[2][c]) * (1. - fx) + f32::from(pixels[3][c]) * fx;
                sample[c] = a * (1. - fy) + b * fy;
            }
            if let Some(background) = surface.matte {
                let alpha = sample[3] / 255.;
                for c in 0..3 {
                    sample[c] = sample[c] * alpha + f32::from(background[c]) * (1. - alpha);
                }
                sample[3] = 255.;
            }
            let mut color = sample.map(|value| value.round() as u8);
            color.swap(0, 2);
            *pixel = color;
        }
    }
    let value = PatchImage {
        image: Arc::new(RenderImage::new(smallvec::smallvec![image::Frame::new(
            out
        )])),
        x: (x0 - snap(grid.origin.0)) / f64::from(grid.dpi * grid.zoom),
        y: (y0 - snap(grid.origin.1)) / f64::from(grid.dpi * grid.zoom),
        width: f64::from(width) / f64::from(grid.dpi * grid.zoom),
        height: f64::from(height) / f64::from(grid.dpi * grid.zoom),
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

fn hash_of(paint: &ScreenshotPaint<'_>) -> u64 {
    let mut hasher = Fingerprint(DefaultHasher::new());
    let _ = write!(hasher, "{paint:?}");
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
    surface.composite_backdrop.hash(&mut hasher);
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

    fn paints<'a>(annotations: &[&'a Annotation]) -> Vec<ScreenshotPaint<'a>> {
        annotations
            .iter()
            .map(|annotation| ScreenshotPaint::Annotation(annotation))
            .collect()
    }

    fn begin_frame(annotations: &[&Annotation]) -> FrameKeys {
        super::begin_frame(&paints(annotations))
    }

    fn patch(
        annotation: &Annotation,
        beneath: &[&Annotation],
        surface: &Surface,
        scale: f64,
    ) -> Option<Patch> {
        super::patch(
            ScreenshotPaint::Annotation(annotation),
            &paints(beneath),
            surface,
            scale,
        )
    }

    fn image(
        keys: &FrameKeys,
        index: usize,
        annotation: &Annotation,
        beneath: &[&Annotation],
        surface: &Surface,
        scale: f64,
    ) -> Option<PatchImage> {
        super::image(
            keys,
            index,
            ScreenshotPaint::Annotation(annotation),
            &paints(beneath),
            surface,
            scale,
        )
    }

    fn aligned_image(
        keys: &FrameKeys,
        index: usize,
        annotation: &Annotation,
        beneath: &[&Annotation],
        surface: &Surface,
        grid: SamplingGrid,
    ) -> Option<PatchImage> {
        super::aligned_image(
            keys,
            index,
            ScreenshotPaint::Annotation(annotation),
            &paints(beneath),
            surface,
            grid,
        )
    }

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
            composite_backdrop: false,
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
            composite_backdrop: false,
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
    fn framed_attachment_rectangle_patch_matches_composed_export() {
        use crate::editor::layers::{Edge, ImageLayer};

        let directory = tempfile::tempdir().unwrap();
        let top_path = directory.path().join("top.png");
        image::RgbaImage::from_pixel(80, 24, image::Rgba([240, 80, 64, 255]))
            .save(&top_path)
            .unwrap();
        let right_path = directory.path().join("right.png");
        image::RgbaImage::from_pixel(30, 60, image::Rgba([72, 192, 112, 255]))
            .save(&right_path)
            .unwrap();
        let layers = [
            ImageLayer {
                id: "top".into(),
                image_url: top_path.to_string_lossy().into_owned(),
                natural_width: 80.0,
                natural_height: 24.0,
                edge: Edge::Top,
            },
            ImageLayer {
                id: "right".into(),
                image_url: right_path.to_string_lossy().into_owned(),
                natural_width: 30.0,
                natural_height: 60.0,
                edge: Edge::Right,
            },
        ];
        let base = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            160,
            120,
            image::Rgba([64, 112, 192, 255]),
        ));
        let annotation = Annotation::Rectangle {
            id: "rectangle".into(),
            x: 38.0,
            y: 36.0,
            width: 20.0,
            height: 20.0,
            stroke: "#f43f5e".into(),
            stroke_width: 3.0,
            fill: Some("#f0c040".into()),
        };
        let mut wallpaper = crate::editor::wallpaper::WallpaperSettings::default();
        wallpaper.window_frame.style = "windows-dark".into();
        let export = crate::editor::export::compose_with_layers(
            Some(&base),
            160,
            120,
            std::slice::from_ref(&annotation),
            &wallpaper,
            &layers,
            1.0,
        );
        let backdrop = crate::editor::export::compose_with_layers(
            Some(&base),
            160,
            120,
            &[],
            &wallpaper,
            &layers,
            1.0,
        );
        assert_eq!(export.dimensions(), (210, 252));

        let title_bar = crate::render::window_frame::title_bar_height("windows-dark", 1.0);
        let group = crate::editor::layers::compute_with_title_bar(
            160.0,
            120.0,
            &layers,
            wallpaper.spacing.max(0.0),
            wallpaper.layer_inset(true),
            title_bar,
        );
        let (_, (x, y, _, _)) =
            crate::editor::wallpaper::layout(&wallpaper, group.width, group.height, 1.0, true);
        let primary_origin = (
            x.round() + group.primary.x + wallpaper.layer_inset(true),
            y.round() + group.primary.y + title_bar + wallpaper.layer_inset(true),
        );
        let mut backdrop_bgra = backdrop.as_raw().to_vec();
        for pixel in backdrop_bgra.as_chunks_mut::<4>().0 {
            pixel.swap(0, 2);
        }
        let surface = Surface {
            image_id: None,
            width: backdrop.width(),
            height: backdrop.height(),
            offset_x: primary_origin.0,
            offset_y: primary_origin.1,
            native_scale: 1.0,
            pixels: Some(&backdrop_bgra),
            matte: Some([18, 18, 20]),
            matte_image: None,
            composite_backdrop: false,
        };
        let preview = patch(&annotation, &[], &surface, 1.0).unwrap();
        let mut mismatches = Vec::new();
        for y in 0..preview.pixmap.height() {
            for x in 0..preview.pixmap.width() {
                let actual = preview.pixmap.pixel(x, y).unwrap();
                let export_x = (primary_origin.0 + preview.x + f64::from(x)) as u32;
                let export_y = (primary_origin.1 + preview.y + f64::from(y)) as u32;
                let expected = export.get_pixel(export_x, export_y).0;
                let actual = [actual.red(), actual.green(), actual.blue(), actual.alpha()];
                if actual != expected {
                    mismatches.push((x, y, actual, expected));
                }
            }
        }
        assert!(
            mismatches.is_empty(),
            "CPU patch differs from the framed export: {mismatches:?}"
        );
    }

    #[test]
    fn anchored_preview_stroke_matches_composition_with_scale_and_clip() {
        use tiny_skia::{Color, Rect};

        let annotation = Annotation::Rectangle {
            id: "rectangle".into(),
            x: 38.0,
            y: 36.0,
            width: 20.0,
            height: 20.0,
            stroke: "#f43f5e".into(),
            stroke_width: 3.0,
            fill: Some("#f0c040".into()),
        };
        let blue = Color::from_rgba8(64, 112, 192, 255);

        for scale in [1.0_f32, 1.25, 2.0] {
            let composition_origin = (0.0, 132.0 * scale);
            let crop_origin = ((32.0 * scale).floor(), (32.0 * scale).floor());
            let clip = Rect::from_xywh(40.0, 40.0, 14.0, 10.0).unwrap();
            let mut full = Canvas::new(280, 400).unwrap();
            full.pixmap_mut().fill(blue);
            full.translate(composition_origin.0, composition_origin.1);
            full.scale(scale, scale);
            full.clip_rect(clip);
            renderer::draw_screenshot(&mut full, &annotation, 1.0);
            let full = full.into_pixmap();

            let mut patch = Canvas::new(52, 52).unwrap();
            patch.pixmap_mut().fill(blue);
            patch.translate(-crop_origin.0, -crop_origin.1);
            patch.scale(scale, scale);
            patch.set_patch_path_anchor(composition_origin, crop_origin);
            patch.clip_rect(clip);
            renderer::draw_screenshot(&mut patch, &annotation, 1.0);
            let patch = patch.into_pixmap();
            for y in 0..patch.height() {
                for x in 0..patch.width() {
                    let actual = patch.pixel(x, y).unwrap();
                    let expected = full
                        .pixel(
                            (composition_origin.0 + crop_origin.0) as u32 + x,
                            (composition_origin.1 + crop_origin.1) as u32 + y,
                        )
                        .unwrap();
                    assert_eq!(
                        [actual.red(), actual.green(), actual.blue(), actual.alpha()],
                        [
                            expected.red(),
                            expected.green(),
                            expected.blue(),
                            expected.alpha()
                        ],
                        "scale {scale} pixel {x},{y}"
                    );
                }
            }
        }
    }

    #[test]
    fn thin_stroke_patch_keeps_the_existing_hairline_rendering() {
        let annotation = Annotation::Rectangle {
            id: "thin-rectangle".into(),
            x: 20.0,
            y: 20.0,
            width: 30.0,
            height: 24.0,
            stroke: "#f43f5e".into(),
            stroke_width: 0.5,
            fill: None,
        };
        let surface = Surface {
            image_id: None,
            width: 100,
            height: 80,
            offset_x: 0.25,
            offset_y: 0.25,
            native_scale: 1.0,
            pixels: None,
            matte: None,
            matte_image: None,
            composite_backdrop: false,
        };
        let scale = 1.25;
        let preview = patch(&annotation, &[], &surface, scale).unwrap();
        let mut direct = Canvas::new(preview.pixmap.width(), preview.pixmap.height()).unwrap();
        direct.translate(
            -((preview.x * scale).floor() as f32),
            -((preview.y * scale).floor() as f32),
        );
        direct.scale(scale as f32, scale as f32);
        renderer::draw_screenshot(&mut direct, &annotation, 1.0);

        assert_eq!(preview.pixmap.data(), direct.into_pixmap().data());
    }

    #[test]
    fn aligned_blur_cache_tracks_the_image_and_sampling_grid() {
        let first = render_image(&base_pixmap(180, 140));
        let mut other = base_pixmap(180, 140);
        other.fill(tiny_skia::Color::from_rgba8(180, 200, 220, 255));
        let second = render_image(&other);
        let annotation = Annotation::Redact {
            id: "blur".into(),
            x: 20.,
            y: 20.,
            width: 80.,
            height: 60.,
            style: "blur".into(),
            intensity: 2.,
        };
        let surface = Surface {
            image_id: Some(first.id),
            width: 180,
            height: 140,
            offset_x: 0.,
            offset_y: 0.,
            native_scale: 1.,
            pixels: first.as_bytes(0),
            matte: Some([240, 240, 240]),
            matte_image: None,
            composite_backdrop: false,
        };
        let grid = SamplingGrid {
            origin: (100., 50.),
            stage_bounds: (100., 50., 270., 210.),
            stage_size: (180., 140.),
            offset: (0., 0.),
            zoom: 1.5,
            dpi: 1.,
        };
        let keys = begin_frame(&[&annotation]);
        let first_patch = aligned_image(&keys, 0, &annotation, &[], &surface, grid).unwrap();
        let repeated = aligned_image(&keys, 0, &annotation, &[], &surface, grid).unwrap();
        assert!(Arc::ptr_eq(&first_patch.image, &repeated.image));
        let shifted = aligned_image(
            &keys,
            0,
            &annotation,
            &[],
            &surface,
            SamplingGrid {
                origin: (101., 51.),
                stage_bounds: (101., 51., 270., 210.),
                ..grid
            },
        )
        .unwrap();
        assert!(Arc::ptr_eq(&first_patch.image, &shifted.image));
        let resized = aligned_image(
            &keys,
            0,
            &annotation,
            &[],
            &surface,
            SamplingGrid {
                stage_bounds: (100., 50., 271., 210.),
                ..grid
            },
        )
        .unwrap();
        assert!(!Arc::ptr_eq(&first_patch.image, &resized.image));
        let scaled_grid = SamplingGrid {
            origin: (100., 50.4),
            stage_bounds: (100., 50.4, 216., 168.),
            zoom: 1.2,
            dpi: 1.25,
            ..grid
        };
        let scaled = aligned_image(&keys, 0, &annotation, &[], &surface, scaled_grid).unwrap();
        let moved_grid = SamplingGrid {
            origin: (100.8, 51.2),
            stage_bounds: (100.8, 51.2, 216., 168.),
            ..scaled_grid
        };
        let moved = aligned_image(&keys, 0, &annotation, &[], &surface, moved_grid).unwrap();
        assert!(Arc::ptr_eq(&scaled.image, &moved.image));
        CACHE.with(|cache| cache.borrow_mut().clear());
        let rebuilt = aligned_image(&keys, 0, &annotation, &[], &surface, moved_grid).unwrap();
        assert_eq!(scaled.image.as_bytes(0), rebuilt.image.as_bytes(0));
        assert_eq!((scaled.x, scaled.y), (rebuilt.x, rebuilt.y));
        let negative_grid = SamplingGrid {
            origin: (-100., -50.4),
            stage_bounds: (-100., -50.4, 216., 168.),
            ..scaled_grid
        };
        let negative = aligned_image(&keys, 0, &annotation, &[], &surface, negative_grid).unwrap();
        let moved_negative_grid = SamplingGrid {
            origin: (-100.8, -51.2),
            stage_bounds: (-100.8, -51.2, 216., 168.),
            ..negative_grid
        };
        let moved_negative =
            aligned_image(&keys, 0, &annotation, &[], &surface, moved_negative_grid).unwrap();
        assert!(Arc::ptr_eq(&negative.image, &moved_negative.image));
        CACHE.with(|cache| cache.borrow_mut().clear());
        let rebuilt_negative =
            aligned_image(&keys, 0, &annotation, &[], &surface, moved_negative_grid).unwrap();
        assert_eq!(
            negative.image.as_bytes(0),
            rebuilt_negative.image.as_bytes(0)
        );
        assert_eq!(
            (negative.x, negative.y),
            (rebuilt_negative.x, rebuilt_negative.y)
        );
        let fractional_grid = SamplingGrid {
            origin: (80., 80.),
            stage_bounds: (80., 80., 131.36, 102.2),
            zoom: 0.73,
            dpi: 1.25,
            ..grid
        };
        let before = aligned_image(&keys, 0, &annotation, &[], &surface, fractional_grid).unwrap();
        let after = aligned_image(
            &keys,
            0,
            &annotation,
            &[],
            &surface,
            SamplingGrid {
                origin: (80.25, 80.),
                stage_bounds: (80.25, 80., 131.36, 102.2),
                ..fractional_grid
            },
        )
        .unwrap();
        assert!(!Arc::ptr_eq(&before.image, &after.image));
        let other_surface = Surface {
            image_id: Some(second.id),
            pixels: second.as_bytes(0),
            ..surface
        };
        let other_patch = aligned_image(&keys, 0, &annotation, &[], &other_surface, grid).unwrap();
        assert_ne!(first_patch.image.as_bytes(0), other_patch.image.as_bytes(0));

        let mut earlier = Annotation::Rectangle {
            id: "earlier".into(),
            x: 32.,
            y: 30.,
            width: 20.,
            height: 20.,
            stroke: "#ff0000".into(),
            stroke_width: 1.,
            fill: Some("#ff0000".into()),
        };
        let keys = begin_frame(&[&earlier, &annotation]);
        let red = aligned_image(&keys, 1, &annotation, &[&earlier], &surface, grid).unwrap();
        if let Annotation::Rectangle { stroke, fill, .. } = &mut earlier {
            *stroke = "#00ff00".into();
            *fill = Some("#00ff00".into());
        }
        let keys = begin_frame(&[&earlier, &annotation]);
        let green = aligned_image(&keys, 1, &annotation, &[&earlier], &surface, grid).unwrap();
        assert_ne!(red.image.as_bytes(0), green.image.as_bytes(0));
    }

    #[test]
    fn aligned_blur_declines_an_oversized_texture() {
        let annotation = Annotation::Redact {
            id: "wide".into(),
            x: 0.,
            y: 0.,
            width: 17_000.,
            height: 160.,
            style: "blur".into(),
            intensity: 2.,
        };
        let surface = Surface {
            image_id: None,
            width: 19_200,
            height: 160,
            offset_x: 0.,
            offset_y: 0.,
            native_scale: 1.,
            pixels: Some(&[]),
            matte: None,
            matte_image: None,
            composite_backdrop: false,
        };
        let grid = SamplingGrid {
            origin: (0., 0.),
            stage_bounds: (0., 0., 19_200., 160.),
            stage_size: (19_200., 160.),
            offset: (0., 0.),
            zoom: 1.,
            dpi: 1.,
        };
        let keys = begin_frame(&[&annotation]);
        assert!(aligned_image(&keys, 0, &annotation, &[], &surface, grid).is_none());
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
            composite_backdrop: false,
        };
        let paint = PixmapPaint {
            opacity: 1.0,
            blend_mode: BlendMode::Source,
            quality: FilterQuality::Nearest,
        };
        let paints = renderer::screenshot_paints(annotations.iter());
        for scale in [1., 1.25, 2.] {
            let mut exported = Canvas::new((180. * scale) as u32, (140. * scale) as u32).unwrap();
            exported.scale(scale as f32, scale as f32);
            exported.draw_pixmap(base.as_ref(), 0., 0., 180., 140.);
            let mut preview = exported.pixmap().clone();
            exported.translate(surface.offset_x as f32, surface.offset_y as f32);
            renderer::draw_all_screenshot(&mut exported, &annotations, surface.native_scale);
            let exported = exported.into_pixmap();
            for (index, operation) in paints.iter().enumerate() {
                let patch = super::patch(*operation, &paints[..index], &surface, scale).unwrap();
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
    fn ordered_redaction_and_highlight_patches_match_export() {
        let base = base_pixmap(100, 80);
        let source = render_image(&base);
        let surface = Surface {
            image_id: Some(source.id),
            width: 100,
            height: 80,
            offset_x: 0.,
            offset_y: 0.,
            native_scale: 1.,
            pixels: source.as_bytes(0),
            matte: None,
            matte_image: None,
            composite_backdrop: false,
        };
        let shape = Annotation::Rectangle {
            id: "shape".into(),
            x: 30.,
            y: 20.,
            width: 24.,
            height: 24.,
            stroke: "#ff00ff".into(),
            stroke_width: 1.,
            fill: Some("#ff00ff".into()),
        };
        let redact = |style: &str| Annotation::Redact {
            id: style.into(),
            x: 32.,
            y: 22.,
            width: 20.,
            height: 20.,
            style: style.into(),
            intensity: 2.,
        };
        let highlight = Annotation::Highlight {
            id: "highlight".into(),
            points: vec![25., 30., 55., 30.],
            fill: "#ffff00".into(),
            opacity: 0.5,
            stroke_width: 10.,
        };
        let paint = PixmapPaint {
            opacity: 1.,
            blend_mode: BlendMode::Source,
            quality: FilterQuality::Nearest,
        };
        for annotations in [
            vec![shape.clone(), redact("blur"), highlight.clone()],
            vec![highlight.clone(), redact("pixelate"), shape.clone()],
            vec![shape.clone(), redact("blackout"), highlight.clone()],
            vec![highlight.clone(), redact("blackout"), shape.clone()],
        ] {
            for scale in [1., 1.25, 2.] {
                let mut exported =
                    Canvas::new((100. * scale) as u32, (80. * scale) as u32).unwrap();
                exported.scale(scale as f32, scale as f32);
                exported.draw_pixmap(base.as_ref(), 0., 0., 100., 80.);
                let mut preview = exported.pixmap().clone();
                renderer::draw_all_screenshot(&mut exported, &annotations, 1.);
                let operations = renderer::screenshot_paints(annotations.iter());
                for (index, operation) in operations.iter().enumerate() {
                    let patch =
                        super::patch(*operation, &operations[..index], &surface, scale).unwrap();
                    preview.draw_pixmap(
                        (patch.x * scale).round() as i32,
                        (patch.y * scale).round() as i32,
                        patch.pixmap.as_ref(),
                        &paint,
                        Transform::identity(),
                        None,
                    );
                }
                for (index, (actual, expected)) in preview
                    .data()
                    .iter()
                    .zip(exported.pixmap().data())
                    .enumerate()
                {
                    assert!(
                        actual.abs_diff(*expected) <= 2,
                        "{annotations:?}, {scale}x, channel {index}: {actual} vs {expected}"
                    );
                }
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
            composite_backdrop: false,
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
            composite_backdrop: false,
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
    fn redaction_patch_samples_the_backdrop_before_filtering() {
        let foreground =
            crate::editor::export::from_rgba(&image::RgbaImage::from_fn(100, 80, |x, y| {
                image::Rgba([
                    220,
                    70,
                    90,
                    if (30..70).contains(&x) && (20..60).contains(&y) {
                        128
                    } else {
                        255
                    },
                ])
            }))
            .unwrap();
        let source = render_image(&foreground);
        for transparent in [false, true] {
            let background = if transparent {
                crate::editor::export::from_rgba(&image::RgbaImage::from_fn(124, 104, |x, y| {
                    image::Rgba([
                        80,
                        110,
                        150,
                        if (4..120).contains(&x) && (4..100).contains(&y) {
                            255
                        } else {
                            0
                        },
                    ])
                }))
                .unwrap()
            } else {
                base_pixmap(124, 104)
            };
            let backdrop = render_image(&background);
            for matte in [[240, 240, 240], [24, 24, 27]] {
                let surface = Surface {
                    image_id: Some(source.id),
                    width: 100,
                    height: 80,
                    offset_x: 0.,
                    offset_y: 0.,
                    native_scale: 1.,
                    pixels: source.as_bytes(0),
                    matte: Some(matte),
                    matte_image: Some((&backdrop, 12., 12.)),
                    composite_backdrop: true,
                };
                for style in ["blur", "pixelate", "blackout"] {
                    for (left, top) in [(32., 22.), (-5., 22.), (90., 22.), (32., -5.), (32., 70.)]
                    {
                        let annotation = Annotation::Redact {
                            id: style.into(),
                            x: left,
                            y: top,
                            width: 40.,
                            height: 30.,
                            style: style.into(),
                            intensity: 2.,
                        };
                        let mut expected = Canvas::from_pixmap(background.clone());
                        expected.draw_pixmap(foreground.as_ref(), 12., 12., 100., 80.);
                        expected.translate(12., 12.);
                        renderer::draw_screenshot(&mut expected, &annotation, 1.);
                        let mut expected = expected.into_pixmap();
                        let mut paint = tiny_skia::Paint::default();
                        paint.set_color_rgba8(matte[0], matte[1], matte[2], 255);
                        paint.blend_mode = BlendMode::DestinationOver;
                        expected.fill_rect(
                            tiny_skia::Rect::from_xywh(0., 0., 124., 104.).unwrap(),
                            &paint,
                            Transform::identity(),
                            None,
                        );
                        let actual = patch(&annotation, &[], &surface, 1.).unwrap();
                        for y in 0..actual.pixmap.height() {
                            for x in 0..actual.pixmap.width() {
                                let pixel = actual.pixmap.pixel(x, y).unwrap();
                                let expected = expected
                                    .pixel(
                                        (actual.x as i32 + x as i32 + 12) as u32,
                                        (actual.y as i32 + y as i32 + 12) as u32,
                                    )
                                    .unwrap();
                                assert!(
                                    pixel.red().abs_diff(expected.red()) <= 2
                                        && pixel.green().abs_diff(expected.green()) <= 2
                                        && pixel.blue().abs_diff(expected.blue()) <= 2
                                        && pixel.alpha() == expected.alpha(),
                                    "transparent {transparent}, matte {matte:?}, {style} at {left},{top}, pixel {x},{y}: {pixel:?} vs {expected:?}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn fractional_layout_redaction_sampling_matches_export() {
        let settings = crate::editor::wallpaper::WallpaperSettings {
            padding: 20.,
            inset: 8.,
            aspect_ratio: "4:3".into(),
            ..Default::default()
        };
        let ((width, height), (x, y, _, _)) =
            crate::editor::wallpaper::layout(&settings, 100., 80., 1., false);
        assert_ne!(x, x.round());
        let (width, height) = (width.round() as u32, height.round() as u32);
        let (snapped_x, snapped_y) = (x.round(), y.round());
        let foreground = crate::editor::export::from_rgba(&image::RgbaImage::from_pixel(
            100,
            80,
            image::Rgba([220, 70, 90, 128]),
        ))
        .unwrap();
        let background =
            crate::editor::export::from_rgba(&image::RgbaImage::from_fn(width, height, |x, y| {
                image::Rgba([
                    80,
                    110,
                    150,
                    if (33..149).contains(&x) && (20..116).contains(&y) {
                        255
                    } else {
                        0
                    },
                ])
            }))
            .unwrap();
        let source = render_image(&foreground);
        let backdrop = render_image(&background);
        let annotation = Annotation::Redact {
            id: "fractional-offset".into(),
            x: -12.,
            y: 22.,
            width: 40.,
            height: 30.,
            style: "blur".into(),
            intensity: 2.,
        };
        let mut expected = Canvas::from_pixmap(background);
        expected.draw_pixmap(
            foreground.as_ref(),
            snapped_x as f32,
            snapped_y as f32,
            100.,
            80.,
        );
        expected.translate(snapped_x as f32, snapped_y as f32);
        renderer::draw_screenshot(&mut expected, &annotation, 1.);
        for offset in [x, snapped_x] {
            let surface = Surface {
                image_id: Some(source.id),
                width: 100,
                height: 80,
                offset_x: 0.,
                offset_y: 0.,
                native_scale: 1.,
                pixels: source.as_bytes(0),
                matte: None,
                matte_image: Some((&backdrop, offset, snapped_y)),
                composite_backdrop: true,
            };
            let actual = patch(&annotation, &[], &surface, 1.).unwrap();
            let mut worst = 0;
            for row in 0..actual.pixmap.height() {
                for column in 0..actual.pixmap.width() {
                    let actual_pixel = actual.pixmap.pixel(column, row).unwrap();
                    let expected_pixel = expected
                        .pixmap()
                        .pixel(
                            (actual.x as i32 + column as i32 + snapped_x as i32) as u32,
                            (actual.y as i32 + row as i32 + snapped_y as i32) as u32,
                        )
                        .unwrap();
                    for (actual, expected) in [
                        (actual_pixel.red(), expected_pixel.red()),
                        (actual_pixel.green(), expected_pixel.green()),
                        (actual_pixel.blue(), expected_pixel.blue()),
                        (actual_pixel.alpha(), expected_pixel.alpha()),
                    ] {
                        worst = worst.max(actual.abs_diff(expected));
                    }
                }
            }
            assert!(worst <= 2, "offset {offset}, worst {worst}");
        }
    }

    #[test]
    fn interior_blur_sampling_stays_within_the_patch_budget() {
        let surface = Surface {
            image_id: None,
            width: 4000,
            height: 3000,
            offset_x: 0.,
            offset_y: 0.,
            native_scale: 2.,
            pixels: None,
            matte: None,
            matte_image: None,
            composite_backdrop: false,
        };
        let annotation = Annotation::Redact {
            id: "large-blur".into(),
            x: 400.,
            y: 400.,
            width: 3000.,
            height: 1800.,
            style: "blur".into(),
            intensity: 10.,
        };
        let (left, top, right, bottom) = raster_bounds(&annotation, &surface, 1.).unwrap();
        assert!((right - left) * (bottom - top) < MAX_PATCH_PIXELS);
    }

    #[test]
    fn blur_patches_sample_at_low_zoom() {
        let mut base = Pixmap::new(256, 256).unwrap();
        for y in 0..256 {
            for x in 0..256 {
                let value = if (x / 3 + y / 5) % 2 == 0 { 255 } else { 0 };
                let index = (y * 256 + x) as usize * 4;
                base.data_mut()[index..index + 4].copy_from_slice(&[value, value, value, 255]);
            }
        }
        let bgra = to_bgra(&base);
        let scale = 0.3125;
        let mut cases = (40..48)
            .map(|x| (f64::from(x), 40., f64::from(x), 48.))
            .collect::<Vec<_>>();
        cases.extend([
            (-8., 40., 0., 48.),
            (40., -8., 48., 0.),
            (236., 40., 236., 48.),
            (40., 236., 48., 236.),
            (-8., -8., 0., 0.),
            (236., 236., 236., 236.),
        ]);
        for (offset_x, offset_y) in [(0., 0.), (16., -16.)] {
            let surface = Surface {
                image_id: None,
                width: 256,
                height: 256,
                offset_x,
                offset_y,
                native_scale: 1.25,
                pixels: Some(&bgra),
                matte: None,
                matte_image: None,
                composite_backdrop: false,
            };
            for &(blur_x, blur_y, rectangle_x, rectangle_y) in &cases {
                let blur = Annotation::Redact {
                    id: "blur".into(),
                    x: blur_x - offset_x,
                    y: blur_y - offset_y,
                    width: 100.,
                    height: 100.,
                    style: "blur".into(),
                    intensity: 1.,
                };
                let rectangle = Annotation::Rectangle {
                    id: "rectangle".into(),
                    x: rectangle_x - offset_x,
                    y: rectangle_y - offset_y,
                    width: 30.,
                    height: 30.,
                    stroke: "#ff00ff".into(),
                    stroke_width: 1.,
                    fill: None,
                };
                for later in [false, true] {
                    let actual = if later {
                        patch(&rectangle, &[&blur], &surface, scale)
                    } else {
                        patch(&blur, &[], &surface, scale)
                    }
                    .unwrap();
                    let mut expected = Canvas::new(80, 80).unwrap();
                    expected.scale(scale as f32, scale as f32);
                    expected.draw_pixmap(base.as_ref(), 0., 0., 256., 256.);
                    expected.translate(offset_x as f32, offset_y as f32);
                    renderer::draw_screenshot(&mut expected, &blur, surface.native_scale);
                    if later {
                        renderer::draw_screenshot(&mut expected, &rectangle, surface.native_scale);
                    }
                    let expected = expected.into_pixmap();
                    let left = ((actual.x + offset_x) * scale).round() as u32;
                    let top = ((actual.y + offset_y) * scale).round() as u32;
                    for y in 0..actual.pixmap.height() {
                        for x in 0..actual.pixmap.width() {
                            let actual = actual.pixmap.pixel(x, y).unwrap();
                            let expected = expected.pixel(left + x, top + y).unwrap();
                            for (actual, expected) in [
                                (actual.red(), expected.red()),
                                (actual.green(), expected.green()),
                                (actual.blue(), expected.blue()),
                                (actual.alpha(), expected.alpha()),
                            ] {
                                assert!(
                                    actual.abs_diff(expected) <= 1,
                                    "later {later}, surface {offset_x},{offset_y}, blur {blur_x},{blur_y}, pixel {x},{y}: {actual} vs {expected}"
                                );
                            }
                        }
                    }
                }
            }
        }
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
                composite_backdrop: false,
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
            composite_backdrop: false,
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
