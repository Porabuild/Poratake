use std::cell::{Cell, RefCell};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
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
const MAX_INTERACTIVE_BLUR_PIXELS: f64 = 131_072.0;
const CACHE_TTL_FRAMES: u64 = 4;
const DRAG_CACHE_TTL_FRAMES: u64 = 1;

#[derive(Clone, Copy)]
pub struct RoundedClip<'a> {
    pub width: u32,
    pub height: u32,
    pub offset_x: i64,
    pub offset_y: i64,
    pub edges: &'a [crate::editor::export::PreviewEdgeImage],
}

#[derive(Clone, Copy)]
pub struct Surface<'a> {
    pub image_id: Option<gpui::ImageId>,
    pub width: u32,
    pub height: u32,
    pub offset_x: f64,
    pub offset_y: f64,
    pub source_is_composed: bool,
    pub align_to_composition_grid: bool,
    pub native_scale: f64,
    pub rounded_clip: Option<RoundedClip<'a>>,
    pub pixels: Option<&'a [u8]>,
    pub matte: Option<[u8; 3]>,
    pub matte_image: Option<(&'a RenderImage, f64, f64)>,
    pub composite_backdrop: bool,
}

impl Surface<'_> {
    fn content_bounds(&self) -> (f64, f64, f64, f64) {
        self.rounded_clip
            .map(|clip| (0.0, 0.0, f64::from(clip.width), f64::from(clip.height)))
            .unwrap_or_else(|| self.bounds())
    }

    fn bounds(&self) -> (f64, f64, f64, f64) {
        if let Some((image, x, y)) = self
            .matte_image
            .filter(|_| self.composite_backdrop || self.rounded_clip.is_some())
        {
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

#[cfg(test)]
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

#[cfg(test)]
#[derive(Clone, Copy)]
pub struct SamplingGrid {
    pub origin: (f32, f32),
    pub stage_bounds: (f32, f32, f32, f32),
    pub viewport: Option<(f32, f32, f32, f32)>,
    pub stage_size: (f64, f64),
    pub offset: (f64, f64),
    pub zoom: f32,
    pub dpi: f32,
}

#[cfg(test)]
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

#[cfg(test)]
fn replay_bounds<'a>(
    target: (f64, f64, f64, f64),
    paints: impl DoubleEndedIterator<Item = ScreenshotPaint<'a>>,
    surface: &Surface,
) -> (f64, f64, f64, f64) {
    let mut sample = target;
    let limits = surface.bounds();
    let content = surface.content_bounds();
    for paint in paints.rev() {
        let ScreenshotPaint::Annotation(
            annotation @ Annotation::Redact {
                style, intensity, ..
            },
        ) = paint
        else {
            continue;
        };
        let effect = padded(annotation.bounds(), 2.);
        let overlap = (
            sample.0.max(effect.0),
            sample.1.max(effect.1),
            sample.2.min(effect.2),
            sample.3.min(effect.3),
        );
        if overlap.2 <= overlap.0 || overlap.3 <= overlap.1 {
            continue;
        }
        let (block, radius) = crate::editor::annotations::redact_intensity(*intensity);
        let padding = match style.as_str() {
            "pixelate" => (block * surface.native_scale).round().max(1.) + 2.,
            "blur" => {
                let sigma = (radius * surface.native_scale).round().max(1.) as f32;
                (crate::render::blur::box_radius(sigma) * 3 + 2) as f64
            }
            _ => continue,
        };
        let dependency = padded(overlap, padding);
        let dependency = (
            dependency.0.max(content.0),
            dependency.1.max(content.1),
            dependency.2.min(content.2),
            dependency.3.min(content.3),
        );
        if dependency.2 <= dependency.0 || dependency.3 <= dependency.1 {
            continue;
        }
        sample = (
            sample.0.min(dependency.0).max(limits.0),
            sample.1.min(dependency.1).max(limits.1),
            sample.2.max(dependency.2).min(limits.2),
            sample.3.max(dependency.3).min(limits.3),
        );
    }
    sample
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
            let (left_limit, top_limit, right_limit, bottom_limit) = surface.content_bounds();
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

#[cfg(test)]
fn bounds_overlap(a: (f64, f64, f64, f64), b: (f64, f64, f64, f64)) -> bool {
    a.0 < b.2 && a.2 > b.0 && a.1 < b.3 && a.3 > b.1
}

fn fit_scale(width: f64, height: f64, scale: f64) -> f64 {
    let pixels = width * scale * height * scale;
    if pixels <= MAX_PATCH_PIXELS {
        return scale;
    }
    scale * (MAX_PATCH_PIXELS / pixels).sqrt()
}

#[cfg(test)]
pub fn patch(
    paint: ScreenshotPaint<'_>,
    beneath: &[ScreenshotPaint<'_>],
    surface: &Surface,
    scale: f64,
) -> Option<Patch> {
    patch_padded(paint, beneath, surface, scale, 0.)
}

#[cfg(test)]
fn paint_target_bounds(
    annotation: &Annotation,
    bounds: (f64, f64, f64, f64),
    composition_origin: (f64, f64),
) -> (f64, f64, f64, f64) {
    match annotation {
        Annotation::Redact {
            x,
            y,
            width,
            height,
            style,
            ..
        } if style == "blackout" => {
            let target = annotation.bounds();
            let (x, y, width, height) =
                renderer::screenshot_blackout_rect(*x, *y, *width, *height, composition_origin);
            (
                target.0.min(x),
                target.1.min(y),
                target.2.max(x + width),
                target.3.max(y + height),
            )
        }
        Annotation::Redact { .. } => annotation.bounds(),
        _ => bounds,
    }
}

#[cfg(test)]
fn patch_padded(
    paint: ScreenshotPaint<'_>,
    beneath: &[ScreenshotPaint<'_>],
    surface: &Surface,
    scale: f64,
    padding: f64,
) -> Option<Patch> {
    let bounds = patch_bounds(paint, beneath, surface, scale, padding)?;
    render_patch(paint, beneath, surface, bounds.sample, bounds.target, scale)
}

#[cfg(test)]
struct PatchBounds {
    sample: (f64, f64, f64, f64),
    target: (f64, f64, f64, f64),
}

#[cfg(test)]
fn patch_bounds(
    paint: ScreenshotPaint<'_>,
    beneath: &[ScreenshotPaint<'_>],
    surface: &Surface,
    scale: f64,
    padding: f64,
) -> Option<PatchBounds> {
    if scale <= 0.0 {
        return None;
    }
    let (surface_left, surface_top, surface_right, surface_bottom) = surface.bounds();
    let (surface_width, surface_height) =
        (surface_right - surface_left, surface_bottom - surface_top);
    let minimum_scale = fit_scale(surface_width, surface_height, scale)
        .min((MAX_PATCH_DIMENSION - 1.0) / surface_width)
        .min((MAX_PATCH_DIMENSION - 1.0) / surface_height);
    let composition_origin = surface
        .matte_image
        .filter(|_| surface.composite_backdrop)
        .map(|(_, x, y)| (x, y))
        .unwrap_or((surface.offset_x, surface.offset_y));
    let annotation = paint.annotation();
    let bounds = raster_bounds(annotation, surface, minimum_scale)?;
    let target = paint_target_bounds(annotation, bounds, composition_origin);
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
    Some(PatchBounds { sample, target })
}

#[cfg(test)]
fn clipped_patch_bounds(surface: &Surface, bounds: PatchBounds) -> Option<PatchBounds> {
    let PatchBounds { sample, target } = bounds;
    let (surface_left, surface_top, surface_right, surface_bottom) = surface.bounds();
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
    Some(PatchBounds {
        sample: (left, top, right, bottom),
        target,
    })
}

#[cfg(test)]
fn patch_scale(bounds: &PatchBounds, scale: f64) -> f64 {
    fit_scale(
        bounds.sample.2 - bounds.sample.0,
        bounds.sample.3 - bounds.sample.1,
        scale,
    )
    .min((MAX_PATCH_DIMENSION - 1.0) / (bounds.target.2 - bounds.target.0))
    .min((MAX_PATCH_DIMENSION - 1.0) / (bounds.target.3 - bounds.target.1))
}

#[cfg(test)]
fn render_patch(
    paint: ScreenshotPaint<'_>,
    beneath: &[ScreenshotPaint<'_>],
    surface: &Surface,
    sample: (f64, f64, f64, f64),
    target: (f64, f64, f64, f64),
    scale: f64,
) -> Option<Patch> {
    let bounds = clipped_patch_bounds(surface, PatchBounds { sample, target })?;
    let scale = patch_scale(&bounds, scale);
    let (left, top, right, bottom) = bounds.sample;
    let target = bounds.target;
    let (surface_left, surface_top, surface_right, surface_bottom) = surface.content_bounds();
    let composition_origin = surface
        .matte_image
        .filter(|_| surface.composite_backdrop)
        .map(|(_, x, y)| (x, y))
        .unwrap_or((surface.offset_x, surface.offset_y));
    let (phase_x, phase_y) = if surface.align_to_composition_grid && surface.rounded_clip.is_none()
    {
        (
            (composition_origin.0 * scale).rem_euclid(1.0),
            (composition_origin.1 * scale).rem_euclid(1.0),
        )
    } else {
        (0.0, 0.0)
    };
    let sample_x = (left * scale + phase_x).floor();
    let sample_y = (top * scale + phase_y).floor();
    let device_x = sample_x - phase_x;
    let device_y = sample_y - phase_y;
    let width = ((right * scale + phase_x).ceil() - sample_x).max(1.0) as u32;
    let height = ((bottom * scale + phase_y).ceil() - sample_y).max(1.0) as u32;
    let (render_x, render_y, render_width, render_height) = if let Some(clip) = surface.rounded_clip
    {
        let x = device_x.max(0.);
        let y = device_y.max(0.);
        let right = (device_x + f64::from(width)).min((f64::from(clip.width) * scale).ceil());
        let bottom = (device_y + f64::from(height)).min((f64::from(clip.height) * scale).ceil());
        if right <= x || bottom <= y {
            return None;
        }
        (x, y, (right - x) as u32, (bottom - y) as u32)
    } else {
        (device_x, device_y, width, height)
    };

    let mut canvas = Canvas::new(render_width, render_height)?;
    canvas.translate(-render_x as f32, -render_y as f32);
    canvas.scale(scale as f32, scale as f32);
    canvas.set_screenshot_surface((
        (surface_left * scale - render_x).floor() as i32,
        (surface_top * scale - render_y).floor() as i32,
        (surface_right * scale - render_x).ceil() as i32,
        (surface_bottom * scale - render_y).ceil() as i32,
    ));
    canvas.set_pixel_grid_origin(composition_origin);
    canvas.set_patch_path_anchor(
        (
            (composition_origin.0 * scale) as f32,
            (composition_origin.1 * scale) as f32,
        ),
        (render_x as f32, render_y as f32),
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
            source_is_composed: true,
            align_to_composition_grid: false,
            native_scale: surface.native_scale,
            rounded_clip: None,
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
            if let Some(other) = raster_bounds(earlier.annotation(), surface, scale) {
                if !bounds_overlap(other, bounds.sample) {
                    continue;
                }
            }
            renderer::draw_screenshot_paint(&mut canvas, *earlier, surface.native_scale);
        }
    }
    renderer::draw_screenshot_paint(&mut canvas, paint, surface.native_scale);
    if let Some(clip) = surface.rounded_clip {
        let rendered = canvas.into_pixmap();
        canvas = Canvas::new(width, height)?;
        canvas.translate(-device_x as f32, -device_y as f32);
        canvas.scale(scale as f32, scale as f32);
        let (offset_x, offset_y) = (
            (render_x - device_x) as usize,
            (render_y - device_y) as usize,
        );
        let row_width = render_width as usize * 4;
        for row in 0..render_height as usize {
            let source = row * row_width;
            let target = ((row + offset_y) * width as usize + offset_x) * 4;
            canvas.pixmap_mut().data_mut()[target..target + row_width]
                .copy_from_slice(&rendered.data()[source..source + row_width]);
        }
        let mut mask = tiny_skia::Mask::new(canvas.width(), canvas.height())?;
        let edges = clip
            .edges
            .iter()
            .map(|edge| Some((edge, edge.image.as_bytes(0)?)))
            .collect::<Option<Vec<_>>>()?;
        let stride = canvas.width() as usize;
        let columns: Vec<_> = (0..stride)
            .map(|x| ((x as f64 + 0.5 + device_x) / scale).floor() as i64)
            .collect();
        for (row_index, row) in mask.data_mut().chunks_exact_mut(stride).enumerate() {
            let y = ((row_index as f64 + 0.5 + device_y) / scale).floor() as i64;
            if y < 0 || y >= i64::from(clip.height) {
                continue;
            }
            let y = y + clip.offset_y;
            for (&x, alpha) in columns.iter().zip(row) {
                if x < 0 || x >= i64::from(clip.width) {
                    continue;
                }
                *alpha = 255;
                let x = x + clip.offset_x;
                for (edge, pixels) in &edges {
                    let (left, top) = (i64::from(edge.x), i64::from(edge.y));
                    if x < left
                        || y < top
                        || x >= left + i64::from(edge.width)
                        || y >= top + i64::from(edge.height)
                    {
                        continue;
                    }
                    let size = edge.image.size(0);
                    let offset = (((y - top) as usize + edge.inset_y as usize)
                        * u32::from(size.width) as usize
                        + (x - left) as usize
                        + edge.inset_x as usize)
                        * 4
                        + 3;
                    *alpha = 255 - *pixels.get(offset)?;
                    break;
                }
            }
        }
        canvas.pixmap_mut().apply_mask(&mask);
    }
    if !surface.composite_backdrop {
        draw_matte(&mut canvas);
    }

    let target_left = (target.0 * scale + phase_x).floor().max(sample_x);
    let target_top = (target.1 * scale + phase_y).floor().max(sample_y);
    let target_right = (target.2 * scale + phase_x)
        .ceil()
        .min(sample_x + f64::from(width));
    let target_bottom = (target.3 * scale + phase_y)
        .ceil()
        .min(sample_y + f64::from(height));
    let target_width = (target_right - target_left) as u32;
    let target_height = (target_bottom - target_top) as u32;
    let rect = tiny_skia::IntRect::from_xywh(
        (target_left - sample_x) as i32,
        (target_top - sample_y) as i32,
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
        x: (target_left - phase_x) / scale,
        y: (target_top - phase_y) / scale,
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
    let rounding = if surface.source_is_composed { 127 } else { 0 };
    for row in 0..crop_height as usize {
        let from = (y0 as usize + row) * stride + x0 as usize * 4;
        let to = row * crop_width as usize * 4;
        for column in 0..crop_width as usize {
            let source = from + column * 4;
            let pixel = to + column * 4;
            let alpha = pixels[source + 3];
            if alpha == 255 {
                target[pixel] = pixels[source + 2];
                target[pixel + 1] = pixels[source + 1];
                target[pixel + 2] = pixels[source];
                target[pixel + 3] = 255;
            } else if alpha == 0 {
                target[pixel..pixel + 4].fill(0);
            } else {
                let alpha_u32 = alpha as u32;
                let premultiply = |value: u8| ((value as u32 * alpha_u32 + rounding) / 255) as u8;
                target[pixel] = premultiply(pixels[source + 2]);
                target[pixel + 1] = premultiply(pixels[source + 1]);
                target[pixel + 2] = premultiply(pixels[source]);
                target[pixel + 3] = alpha;
            }
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LayerSlot {
    Committed,
    Below,
    Above,
}

struct CompositeEntry {
    pixmap: Pixmap,
    image: Arc<RenderImage>,
    device_x: f64,
    device_y: f64,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    scale: f64,
    surface_key: u64,
    paint_hashes: Vec<u64>,
    content_hash: u64,
    frame: u64,
}

struct SpriteEntry {
    image: Arc<RenderImage>,
    local_x: f64,
    local_y: f64,
    width: f64,
    height: f64,
    frame: u64,
}

thread_local! {
    static CACHE: RefCell<HashMap<u64, Entry>> = RefCell::new(HashMap::new());
    static SPRITE_CACHE: RefCell<HashMap<u64, SpriteEntry>> = RefCell::new(HashMap::new());
    static DRAG_CACHE: RefCell<HashMap<u64, Entry>> = RefCell::new(HashMap::new());
    static COMPOSITE_CACHE: RefCell<HashMap<(Option<gpui::ImageId>, LayerSlot), CompositeEntry>> =
        RefCell::new(HashMap::new());
    static RETIRED_IMAGES: RefCell<Vec<Arc<RenderImage>>> = const { RefCell::new(Vec::new()) };
    static FRAME: Cell<u64> = const { Cell::new(0) };
}

pub fn advance_frame() {
    FRAME.with(|frame| frame.set(frame.get().wrapping_add(1)));
}

#[cfg(test)]
pub struct FrameKeys {
    hashes: Vec<u64>,
}

#[cfg(test)]
pub fn begin_frame(visible: &[ScreenshotPaint<'_>]) -> FrameKeys {
    advance_frame();
    let hashes: Vec<u64> = visible.iter().map(|paint| hash_of(paint)).collect();
    FrameKeys { hashes }
}

pub fn clear_cache() {
    CACHE.with(|cache| cache.borrow_mut().clear());
    SPRITE_CACHE.with(|cache| cache.borrow_mut().clear());
    DRAG_CACHE.with(|cache| cache.borrow_mut().clear());
    COMPOSITE_CACHE.with(|cache| cache.borrow_mut().clear());
    RETIRED_IMAGES.with(|retired| retired.borrow_mut().clear());
}

fn evict_stale_entries(
    cache: &mut HashMap<u64, Entry>,
    frame: u64,
    ttl: u64,
    out: &mut Vec<Arc<RenderImage>>,
) {
    let stale: Vec<u64> = cache
        .iter()
        .filter(|(_, entry)| frame.saturating_sub(entry.frame) >= ttl)
        .map(|(key, _)| *key)
        .collect();
    for key in stale {
        if let Some(entry) = cache.remove(&key) {
            out.push(entry.patch.image);
        }
    }
}

pub fn end_frame(window: &mut gpui::Window) {
    let frame = FRAME.with(|frame| frame.get());
    let mut evicted: Vec<Arc<RenderImage>> = Vec::new();
    RETIRED_IMAGES.with(|retired| evicted.extend(retired.borrow_mut().drain(..)));
    CACHE.with(|cache| {
        evict_stale_entries(
            &mut cache.borrow_mut(),
            frame,
            CACHE_TTL_FRAMES,
            &mut evicted,
        );
    });
    DRAG_CACHE.with(|cache| {
        evict_stale_entries(
            &mut cache.borrow_mut(),
            frame,
            DRAG_CACHE_TTL_FRAMES,
            &mut evicted,
        );
    });
    SPRITE_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        let stale: Vec<u64> = cache
            .iter()
            .filter(|(_, entry)| frame.saturating_sub(entry.frame) >= CACHE_TTL_FRAMES)
            .map(|(key, _)| *key)
            .collect();
        for key in stale {
            if let Some(entry) = cache.remove(&key) {
                evicted.push(entry.image);
            }
        }
    });
    COMPOSITE_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        let stale: Vec<(Option<gpui::ImageId>, LayerSlot)> = cache
            .iter()
            .filter(|(_, entry)| frame.saturating_sub(entry.frame) >= CACHE_TTL_FRAMES)
            .map(|(key, _)| *key)
            .collect();
        for key in stale {
            if let Some(entry) = cache.remove(&key) {
                evicted.push(entry.image);
            }
        }
    });
    for image in evicted {
        let _ = window.drop_image(image);
    }
}

pub fn clear(window: &mut gpui::Window) {
    let mut evicted: Vec<Arc<RenderImage>> = Vec::new();
    RETIRED_IMAGES.with(|retired| evicted.extend(retired.borrow_mut().drain(..)));
    CACHE.with(|cache| {
        evicted.extend(
            cache
                .borrow_mut()
                .drain()
                .map(|(_, entry)| entry.patch.image),
        );
    });
    DRAG_CACHE.with(|cache| {
        evicted.extend(
            cache
                .borrow_mut()
                .drain()
                .map(|(_, entry)| entry.patch.image),
        );
    });
    SPRITE_CACHE.with(|cache| {
        evicted.extend(cache.borrow_mut().drain().map(|(_, entry)| entry.image));
    });
    COMPOSITE_CACHE.with(|cache| {
        evicted.extend(cache.borrow_mut().drain().map(|(_, entry)| entry.image));
    });
    for image in evicted {
        let _ = window.drop_image(image);
    }
}

fn reads_background(paint: ScreenshotPaint<'_>) -> bool {
    match paint.annotation() {
        Annotation::Highlight { .. } => true,
        Annotation::Redact { style, .. } => style != "blackout",
        _ => false,
    }
}

fn hash_rounded_clip(clip: Option<RoundedClip<'_>>, hasher: &mut DefaultHasher) {
    if let Some(clip) = clip {
        hasher.write_u8(1);
        hasher.write_u32(clip.width);
        hasher.write_u32(clip.height);
        hasher.write_i64(clip.offset_x);
        hasher.write_i64(clip.offset_y);
        for edge in clip.edges {
            edge.image.id.hash(hasher);
        }
    } else {
        hasher.write_u8(0);
    }
}

fn composite_surface_key(surface: &Surface, reads_surface: bool, transparent_only: bool) -> u64 {
    let mut hasher = DefaultHasher::new();
    hasher.write_u8(u8::from(transparent_only));
    hasher.write_u8(u8::from(reads_surface));
    hasher.write_u64(surface.native_scale.to_bits());
    hash_rounded_clip(surface.rounded_clip, &mut hasher);
    if reads_surface {
        surface.image_id.hash(&mut hasher);
        hasher.write_u32(surface.width);
        hasher.write_u32(surface.height);
        hasher.write_u64(surface.offset_x.to_bits());
        hasher.write_u64(surface.offset_y.to_bits());
        hasher.write_u8(u8::from(surface.source_is_composed));
        hasher.write_u8(u8::from(surface.pixels.is_some()));
        surface.matte.hash(&mut hasher);
        surface.composite_backdrop.hash(&mut hasher);
        if let Some((image, x, y)) = surface.matte_image {
            image.id.hash(&mut hasher);
            hasher.write_u64(x.to_bits());
            hasher.write_u64(y.to_bits());
        }
    }
    hasher.finish()
}

fn blit_pixmap_region(
    src: &Pixmap,
    src_origin: (i64, i64),
    dst: &mut Pixmap,
    dst_origin: (i64, i64),
) {
    let src_w = i64::from(src.width());
    let src_h = i64::from(src.height());
    let dst_w = i64::from(dst.width());
    let dst_h = i64::from(dst.height());
    let x0 = src_origin.0.max(dst_origin.0);
    let y0 = src_origin.1.max(dst_origin.1);
    let x1 = (src_origin.0 + src_w).min(dst_origin.0 + dst_w);
    let y1 = (src_origin.1 + src_h).min(dst_origin.1 + dst_h);
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    let copy_bytes = (x1 - x0) as usize * 4;
    let src_stride = src_w as usize * 4;
    let dst_stride = dst_w as usize * 4;
    let src_x = (x0 - src_origin.0) as usize * 4;
    let dst_x = (x0 - dst_origin.0) as usize * 4;
    let src_data = src.data();
    let dst_data = dst.data_mut();
    for y in y0..y1 {
        let sy = (y - src_origin.1) as usize * src_stride + src_x;
        let dy = (y - dst_origin.1) as usize * dst_stride + dst_x;
        dst_data[dy..dy + copy_bytes].copy_from_slice(&src_data[sy..sy + copy_bytes]);
    }
}

fn configure_composite_canvas(
    canvas: &mut Canvas,
    surface: &Surface,
    device_x: f64,
    device_y: f64,
    scale: f64,
) {
    let (surface_left, surface_top, surface_right, surface_bottom) = surface.content_bounds();
    let composition_origin = surface
        .matte_image
        .filter(|_| surface.composite_backdrop)
        .map(|(_, x, y)| (x, y))
        .unwrap_or((surface.offset_x, surface.offset_y));
    canvas.translate(-device_x as f32, -device_y as f32);
    canvas.scale(scale as f32, scale as f32);
    canvas.set_screenshot_surface((
        (surface_left * scale - device_x).floor() as i32,
        (surface_top * scale - device_y).floor() as i32,
        (surface_right * scale - device_x).ceil() as i32,
        (surface_bottom * scale - device_y).ceil() as i32,
    ));
    canvas.set_pixel_grid_origin(composition_origin);
    if surface.align_to_composition_grid {
        canvas.set_patch_path_anchor(
            (
                (composition_origin.0 * scale) as f32,
                (composition_origin.1 * scale) as f32,
            ),
            (device_x as f32, device_y as f32),
        );
    }
}

fn seed_surface_pixels(canvas: &mut Canvas, surface: &Surface, bounds: (f64, f64, f64, f64)) {
    if let Some(pixels) = surface.pixels {
        draw_backdrop(
            canvas,
            surface,
            pixels,
            bounds,
            tiny_skia::BlendMode::DestinationOver,
        );
    }
    if surface.composite_backdrop || surface.matte_image.is_some() {
        if let Some((image, offset_x, offset_y)) = surface.matte_image {
            if let Some(pixels) = image.as_bytes(0) {
                let background = Surface {
                    image_id: Some(image.id),
                    width: image.size(0).width.into(),
                    height: image.size(0).height.into(),
                    offset_x,
                    offset_y,
                    source_is_composed: true,
                    align_to_composition_grid: false,
                    native_scale: surface.native_scale,
                    rounded_clip: None,
                    pixels: Some(pixels),
                    matte: None,
                    matte_image: None,
                    composite_backdrop: false,
                };
                draw_backdrop(
                    canvas,
                    &background,
                    pixels,
                    bounds,
                    tiny_skia::BlendMode::DestinationOver,
                );
            }
        }
    }
}

fn draw_paint_onto_composite(
    canvas: &mut Canvas,
    paint: ScreenshotPaint<'_>,
    surface: &Surface,
    canvas_device_x: f64,
    canvas_device_y: f64,
    scale: f64,
    transparent_only: bool,
) {
    let annotation = paint.annotation();
    if transparent_only {
        if matches!(annotation, Annotation::Highlight { .. }) {
            draw_sprite_annotation(canvas, annotation);
            return;
        }
        if matches!(annotation, Annotation::Redact { style, .. } if style != "blackout") {
            return;
        }
    }
    match annotation {
        Annotation::Highlight { .. } if surface.pixels.is_some() => {
            if let Some(h_bounds) = raster_bounds(annotation, surface, scale) {
                let limits = surface.bounds();
                let clipped = (
                    h_bounds.0.max(limits.0),
                    h_bounds.1.max(limits.1),
                    h_bounds.2.min(limits.2),
                    h_bounds.3.min(limits.3),
                );
                if clipped.2 > clipped.0 && clipped.3 > clipped.1 {
                    seed_surface_pixels(canvas, surface, clipped);
                }
            }
            renderer::draw_screenshot_paint(canvas, paint, surface.native_scale);
        }
        Annotation::Redact { style, .. }
            if (style == "blur" || style == "pixelate") && surface.pixels.is_some() =>
        {
            let Some(sample) = raster_bounds(annotation, surface, scale) else {
                return;
            };
            let limits = surface.bounds();
            let sample = (
                sample.0.max(limits.0),
                sample.1.max(limits.1),
                sample.2.min(limits.2),
                sample.3.min(limits.3),
            );
            if sample.2 <= sample.0 || sample.3 <= sample.1 {
                return;
            }
            let target = annotation.bounds();
            let target = (
                target.0.max(sample.0),
                target.1.max(sample.1),
                target.2.min(sample.2),
                target.3.min(sample.3),
            );
            if target.2 <= target.0 || target.3 <= target.1 {
                return;
            }
            let sx0 = (sample.0 * scale).floor();
            let sy0 = (sample.1 * scale).floor();
            let sw = ((sample.2 * scale).ceil() - sx0).max(1.0) as u32;
            let sh = ((sample.3 * scale).ceil() - sy0).max(1.0) as u32;
            let Some(mut scratch) = Canvas::new(sw, sh) else {
                return;
            };
            blit_pixmap_region(
                canvas.pixmap(),
                (canvas_device_x as i64, canvas_device_y as i64),
                scratch.pixmap_mut(),
                (sx0 as i64, sy0 as i64),
            );
            configure_composite_canvas(&mut scratch, surface, sx0, sy0, scale);
            seed_surface_pixels(&mut scratch, surface, sample);
            renderer::draw_screenshot_paint(&mut scratch, paint, surface.native_scale);
            let tx0 = (target.0 * scale).floor().max(sx0);
            let ty0 = (target.1 * scale).floor().max(sy0);
            let tx1 = (target.2 * scale).ceil().min(sx0 + f64::from(sw));
            let ty1 = (target.3 * scale).ceil().min(sy0 + f64::from(sh));
            if tx1 <= tx0 || ty1 <= ty0 {
                return;
            }
            let Some(rect) = tiny_skia::IntRect::from_xywh(
                (tx0 - sx0) as i32,
                (ty0 - sy0) as i32,
                (tx1 - tx0) as u32,
                (ty1 - ty0) as u32,
            ) else {
                return;
            };
            if let Some(mut cropped) = scratch.into_pixmap().clone_rect(rect) {
                if let Some([red, green, blue]) = surface.matte {
                    if let Some(fill_rect) = tiny_skia::Rect::from_xywh(
                        0.,
                        0.,
                        cropped.width() as f32,
                        cropped.height() as f32,
                    ) {
                        let mut matte_paint = tiny_skia::Paint::default();
                        matte_paint.set_color_rgba8(red, green, blue, 255);
                        matte_paint.blend_mode = tiny_skia::BlendMode::DestinationOver;
                        cropped.fill_rect(
                            fill_rect,
                            &matte_paint,
                            tiny_skia::Transform::identity(),
                            None,
                        );
                    }
                }
                blit_pixmap_region(
                    &cropped,
                    (tx0 as i64, ty0 as i64),
                    canvas.pixmap_mut(),
                    (canvas_device_x as i64, canvas_device_y as i64),
                );
            }
        }
        _ => renderer::draw_screenshot_paint(canvas, paint, surface.native_scale),
    }
}

fn apply_rounded_clip_to_pixmap(
    pixmap: &Pixmap,
    clip: RoundedClip<'_>,
    device_x: f64,
    device_y: f64,
    scale: f64,
) -> Option<Pixmap> {
    let mut masked = pixmap.clone();
    let mut mask = tiny_skia::Mask::new(masked.width(), masked.height())?;
    let edges = clip
        .edges
        .iter()
        .map(|edge| Some((edge, edge.image.as_bytes(0)?)))
        .collect::<Option<Vec<_>>>()?;
    let stride = masked.width() as usize;
    let columns: Vec<_> = (0..stride)
        .map(|x| ((x as f64 + 0.5 + device_x) / scale).floor() as i64)
        .collect();
    for (row_index, row) in mask.data_mut().chunks_exact_mut(stride).enumerate() {
        let y = ((row_index as f64 + 0.5 + device_y) / scale).floor() as i64;
        if y < 0 || y >= i64::from(clip.height) {
            continue;
        }
        let y = y + clip.offset_y;
        for (&x, alpha) in columns.iter().zip(row) {
            if x < 0 || x >= i64::from(clip.width) {
                continue;
            }
            *alpha = 255;
            let x = x + clip.offset_x;
            for (edge, pixels) in &edges {
                let (left, top) = (i64::from(edge.x), i64::from(edge.y));
                if x < left
                    || y < top
                    || x >= left + i64::from(edge.width)
                    || y >= top + i64::from(edge.height)
                {
                    continue;
                }
                let size = edge.image.size(0);
                let offset = (((y - top) as usize + edge.inset_y as usize)
                    * u32::from(size.width) as usize
                    + (x - left) as usize
                    + edge.inset_x as usize)
                    * 4
                    + 3;
                *alpha = 255 - *pixels.get(offset)?;
                break;
            }
        }
    }
    masked.apply_mask(&mask);
    Some(masked)
}

fn composite_render_image(
    pixmap: &Pixmap,
    surface: &Surface,
    device_x: f64,
    device_y: f64,
    scale: f64,
) -> Arc<RenderImage> {
    if let Some(clip) = surface.rounded_clip {
        if let Some(masked) = apply_rounded_clip_to_pixmap(pixmap, clip, device_x, device_y, scale)
        {
            return render_image(&masked);
        }
    }
    render_image(pixmap)
}

pub fn composite_layer(
    editor_id: Option<gpui::ImageId>,
    slot: LayerSlot,
    paints: &[ScreenshotPaint<'_>],
    surface: &Surface,
    clip_bounds: (f64, f64, f64, f64),
    transparent_only: bool,
) -> Option<PatchImage> {
    if paints.is_empty() {
        return None;
    }
    let paint_hashes: Vec<u64> = paints.iter().map(hash_of).collect();
    let limits = clip_bounds;
    let mut union: Option<(f64, f64, f64, f64)> = None;
    for paint in paints {
        let Some(bounds) = raster_bounds(paint.annotation(), surface, 1.0) else {
            continue;
        };
        let bounds = (
            bounds.0.max(limits.0),
            bounds.1.max(limits.1),
            bounds.2.min(limits.2),
            bounds.3.min(limits.3),
        );
        if bounds.2 <= bounds.0 || bounds.3 <= bounds.1 {
            continue;
        }
        union = Some(match union {
            Some(existing) => (
                existing.0.min(bounds.0),
                existing.1.min(bounds.1),
                existing.2.max(bounds.2),
                existing.3.max(bounds.3),
            ),
            None => bounds,
        });
    }
    let union = union?;
    let raw_w = union.2 - union.0;
    let raw_h = union.3 - union.1;
    let composite_scale = fit_scale(raw_w, raw_h, 1.0)
        .min((MAX_PATCH_DIMENSION - 1.0) / raw_w)
        .min((MAX_PATCH_DIMENSION - 1.0) / raw_h);
    if composite_scale <= 0.0 {
        return None;
    }
    let device_x = (union.0 * composite_scale).floor();
    let device_y = (union.1 * composite_scale).floor();
    let width = ((union.2 * composite_scale).ceil() - device_x).max(1.0) as u32;
    let height = ((union.3 * composite_scale).ceil() - device_y).max(1.0) as u32;
    let reads_surface = !transparent_only && paints.iter().any(|paint| reads_background(*paint));
    let surface_key = composite_surface_key(surface, reads_surface, transparent_only);
    let cache_key = (editor_id, slot);
    let frame = FRAME.with(|frame| frame.get());

    enum Action {
        Hit(PatchImage),
        Append(usize, Pixmap, f64, f64, Arc<RenderImage>),
        Rebuild(Option<Arc<RenderImage>>),
    }

    let action = COMPOSITE_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(entry) = cache.get_mut(&cache_key) {
            if entry.surface_key == surface_key
                && entry.scale == composite_scale
                && entry.paint_hashes == paint_hashes
                && entry.device_x == device_x
                && entry.device_y == device_y
                && entry.pixmap.width() == width
                && entry.pixmap.height() == height
            {
                entry.frame = frame;
                return Action::Hit(PatchImage {
                    image: entry.image.clone(),
                    x: entry.x,
                    y: entry.y,
                    width: entry.width,
                    height: entry.height,
                });
            }
            if entry.surface_key == surface_key
                && entry.scale == composite_scale
                && entry.paint_hashes.len() < paint_hashes.len()
                && paint_hashes[..entry.paint_hashes.len()] == entry.paint_hashes[..]
            {
                let old = cache.remove(&cache_key).unwrap();
                return Action::Append(
                    old.paint_hashes.len(),
                    old.pixmap,
                    old.device_x,
                    old.device_y,
                    old.image,
                );
            }
            let old = cache.remove(&cache_key).unwrap();
            return Action::Rebuild(Some(old.image));
        }
        Action::Rebuild(None)
    });

    let (pixmap, retired_image) = match action {
        Action::Hit(patch) => return Some(patch),
        Action::Append(old_count, old_pixmap, old_dx, old_dy, old_image) => {
            let mut canvas = if old_dx == device_x
                && old_dy == device_y
                && old_pixmap.width() == width
                && old_pixmap.height() == height
            {
                Canvas::from_pixmap(old_pixmap)
            } else {
                let mut grown = Pixmap::new(width, height)?;
                blit_pixmap_region(
                    &old_pixmap,
                    (old_dx as i64, old_dy as i64),
                    &mut grown,
                    (device_x as i64, device_y as i64),
                );
                Canvas::from_pixmap(grown)
            };
            configure_composite_canvas(&mut canvas, surface, device_x, device_y, composite_scale);
            for paint in &paints[old_count..] {
                draw_paint_onto_composite(
                    &mut canvas,
                    *paint,
                    surface,
                    device_x,
                    device_y,
                    composite_scale,
                    transparent_only,
                );
            }
            (canvas.into_pixmap(), Some(old_image))
        }
        Action::Rebuild(old_image) => {
            let mut canvas = Canvas::new(width, height)?;
            configure_composite_canvas(&mut canvas, surface, device_x, device_y, composite_scale);
            for paint in paints {
                draw_paint_onto_composite(
                    &mut canvas,
                    *paint,
                    surface,
                    device_x,
                    device_y,
                    composite_scale,
                    transparent_only,
                );
            }
            (canvas.into_pixmap(), old_image)
        }
    };

    if let Some(old_image) = retired_image {
        RETIRED_IMAGES.with(|retired| retired.borrow_mut().push(old_image));
    }

    let image = composite_render_image(&pixmap, surface, device_x, device_y, composite_scale);
    let x = device_x / composite_scale;
    let y = device_y / composite_scale;
    let patch_width = f64::from(width) / composite_scale;
    let patch_height = f64::from(height) / composite_scale;
    let mut content_hasher = DefaultHasher::new();
    surface_key.hash(&mut content_hasher);
    paint_hashes.hash(&mut content_hasher);
    let content_hash = content_hasher.finish();
    let result = PatchImage {
        image: image.clone(),
        x,
        y,
        width: patch_width,
        height: patch_height,
    };
    COMPOSITE_CACHE.with(|cache| {
        cache.borrow_mut().insert(
            cache_key,
            CompositeEntry {
                pixmap,
                image,
                device_x,
                device_y,
                x,
                y,
                width: patch_width,
                height: patch_height,
                scale: composite_scale,
                surface_key,
                paint_hashes,
                content_hash,
                frame,
            },
        );
    });
    Some(result)
}

pub fn effect_over_layer(
    editor_id: Option<gpui::ImageId>,
    base_slot: Option<LayerSlot>,
    paint: ScreenshotPaint<'_>,
    surface: &Surface,
    scale: f64,
    interactive: bool,
) -> Option<PatchImage> {
    let annotation = paint.annotation();
    let fast_surface = if interactive {
        Surface {
            rounded_clip: None,
            ..*surface
        }
    } else {
        *surface
    };
    let render_scale = match annotation {
        Annotation::Redact { style, .. } if style == "blur" => {
            if interactive {
                let base = scale.clamp(0.25, 1.0);
                if let Some(bounds) = raster_bounds(annotation, &fast_surface, base) {
                    let w = (bounds.2 - bounds.0).max(1.0);
                    let h = (bounds.3 - bounds.1).max(1.0);
                    let pixels = w * base * h * base;
                    if pixels > MAX_INTERACTIVE_BLUR_PIXELS {
                        (base * (MAX_INTERACTIVE_BLUR_PIXELS / pixels).sqrt()).max(0.25)
                    } else {
                        base
                    }
                } else {
                    base
                }
            } else {
                1.0
            }
        }
        Annotation::Redact { .. } => 1.0,
        _ => scale.clamp(0.25, 1.0),
    };

    let base_hash = base_slot
        .and_then(|slot| {
            COMPOSITE_CACHE.with(|cache| {
                cache
                    .borrow()
                    .get(&(editor_id, slot))
                    .map(|entry| entry.content_hash)
            })
        })
        .unwrap_or(0);
    let mut hasher = DefaultHasher::new();
    hasher.write_u8(201);
    hasher.write_u64(hash_of(&paint));
    hasher.write_u64(base_hash);
    hasher.write_u64(composite_surface_key(&fast_surface, true, false));
    hasher.write_u64(render_scale.to_bits());
    hasher.write_u8(u8::from(interactive));
    let key = hasher.finish();
    let frame = FRAME.with(|frame| frame.get());

    let cached = if interactive {
        DRAG_CACHE.with(|cache| {
            cache.borrow_mut().get_mut(&key).map(|entry| {
                entry.frame = frame;
                entry.patch.clone()
            })
        })
    } else {
        CACHE.with(|cache| {
            cache.borrow_mut().get_mut(&key).map(|entry| {
                entry.frame = frame;
                entry.patch.clone()
            })
        })
    };
    if let Some(cached) = cached {
        return Some(cached);
    }

    let sample = raster_bounds(annotation, &fast_surface, render_scale)?;
    let limits = fast_surface.bounds();
    let sample = (
        sample.0.max(limits.0),
        sample.1.max(limits.1),
        sample.2.min(limits.2),
        sample.3.min(limits.3),
    );
    if sample.2 <= sample.0 || sample.3 <= sample.1 {
        return None;
    }
    let target = if matches!(annotation, Annotation::Redact { .. }) {
        let t = annotation.bounds();
        (
            t.0.max(sample.0),
            t.1.max(sample.1),
            t.2.min(sample.2),
            t.3.min(sample.3),
        )
    } else {
        sample
    };
    if target.2 <= target.0 || target.3 <= target.1 {
        return None;
    }
    let raw_w = sample.2 - sample.0;
    let raw_h = sample.3 - sample.1;
    let actual_scale = fit_scale(raw_w, raw_h, render_scale)
        .min((MAX_PATCH_DIMENSION - 1.0) / raw_w)
        .min((MAX_PATCH_DIMENSION - 1.0) / raw_h);
    if actual_scale <= 0.0 {
        return None;
    }
    let sx0 = (sample.0 * actual_scale).floor();
    let sy0 = (sample.1 * actual_scale).floor();
    let sw = ((sample.2 * actual_scale).ceil() - sx0).max(1.0) as u32;
    let sh = ((sample.3 * actual_scale).ceil() - sy0).max(1.0) as u32;
    let mut scratch = Canvas::new(sw, sh)?;
    configure_composite_canvas(&mut scratch, &fast_surface, sx0, sy0, actual_scale);

    if let Some(slot) = base_slot {
        COMPOSITE_CACHE.with(|cache| {
            let cache = cache.borrow();
            if let Some(base) = cache.get(&(editor_id, slot)) {
                if (base.scale - actual_scale).abs() < 1e-6 {
                    blit_pixmap_region(
                        &base.pixmap,
                        (base.device_x as i64, base.device_y as i64),
                        scratch.pixmap_mut(),
                        (sx0 as i64, sy0 as i64),
                    );
                } else {
                    scratch.draw_pixmap(
                        base.pixmap.as_ref(),
                        base.x as f32,
                        base.y as f32,
                        base.width as f32,
                        base.height as f32,
                    );
                }
            }
        });
    }
    seed_surface_pixels(&mut scratch, &fast_surface, sample);
    renderer::draw_screenshot_paint(&mut scratch, paint, fast_surface.native_scale);

    let tx0 = (target.0 * actual_scale).floor().max(sx0);
    let ty0 = (target.1 * actual_scale).floor().max(sy0);
    let tx1 = (target.2 * actual_scale).ceil().min(sx0 + f64::from(sw));
    let ty1 = (target.3 * actual_scale).ceil().min(sy0 + f64::from(sh));
    if tx1 <= tx0 || ty1 <= ty0 {
        return None;
    }
    let rect = tiny_skia::IntRect::from_xywh(
        (tx0 - sx0) as i32,
        (ty0 - sy0) as i32,
        (tx1 - tx0) as u32,
        (ty1 - ty0) as u32,
    )?;
    let mut pixmap = scratch.into_pixmap().clone_rect(rect)?;
    if let Some([red, green, blue]) = fast_surface.matte {
        let mut matte_paint = tiny_skia::Paint::default();
        matte_paint.set_color_rgba8(red, green, blue, 255);
        matte_paint.blend_mode = tiny_skia::BlendMode::DestinationOver;
        pixmap.fill_rect(
            tiny_skia::Rect::from_xywh(0., 0., pixmap.width() as f32, pixmap.height() as f32)?,
            &matte_paint,
            tiny_skia::Transform::identity(),
            None,
        );
    }
    let image = composite_render_image(&pixmap, &fast_surface, tx0, ty0, actual_scale);
    let value = PatchImage {
        image,
        x: tx0 / actual_scale,
        y: ty0 / actual_scale,
        width: f64::from(pixmap.width()) / actual_scale,
        height: f64::from(pixmap.height()) / actual_scale,
    };
    if interactive {
        DRAG_CACHE.with(|cache| {
            cache.borrow_mut().insert(
                key,
                Entry {
                    patch: value.clone(),
                    frame,
                },
            );
        });
    } else {
        CACHE.with(|cache| {
            cache.borrow_mut().insert(
                key,
                Entry {
                    patch: value.clone(),
                    frame,
                },
            );
        });
    }
    Some(value)
}

#[cfg(test)]
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
    let key = fingerprint(keys, index, paint, beneath, surface, scale, 0.)?;
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

fn empty_surface() -> Surface<'static> {
    Surface {
        image_id: None,
        width: 0,
        height: 0,
        offset_x: 0.0,
        offset_y: 0.0,
        source_is_composed: false,
        align_to_composition_grid: false,
        native_scale: 1.0,
        rounded_clip: None,
        pixels: None,
        matte: None,
        matte_image: None,
        composite_backdrop: false,
    }
}

fn quantize_coord(value: f64) -> i64 {
    (value * 1024.0).round() as i64
}

fn hash_sprite_geometry(annotation: &Annotation, hasher: &mut DefaultHasher) -> bool {
    let (ax, ay) = annotation.anchor();
    match annotation {
        Annotation::Pen {
            points,
            stroke,
            stroke_width,
            ..
        } => {
            hasher.write_u8(1);
            stroke.hash(hasher);
            hasher.write_u64(stroke_width.to_bits());
            hasher.write_usize(points.len());
            for pair in points.as_chunks::<2>().0 {
                hasher.write_i64(quantize_coord(pair[0] - ax));
                hasher.write_i64(quantize_coord(pair[1] - ay));
            }
            true
        }
        Annotation::Highlight {
            points,
            fill,
            opacity,
            stroke_width,
            ..
        } => {
            hasher.write_u8(2);
            fill.hash(hasher);
            hasher.write_u64(opacity.to_bits());
            hasher.write_u64(stroke_width.to_bits());
            hasher.write_usize(points.len());
            for pair in points.as_chunks::<2>().0 {
                hasher.write_i64(quantize_coord(pair[0] - ax));
                hasher.write_i64(quantize_coord(pair[1] - ay));
            }
            true
        }
        Annotation::Rectangle {
            width,
            height,
            stroke,
            stroke_width,
            fill,
            ..
        } => {
            hasher.write_u8(3);
            hasher.write_i64(quantize_coord(*width));
            hasher.write_i64(quantize_coord(*height));
            stroke.hash(hasher);
            hasher.write_u64(stroke_width.to_bits());
            fill.hash(hasher);
            true
        }
        Annotation::Circle {
            radius,
            stroke,
            stroke_width,
            fill,
            ..
        } => {
            hasher.write_u8(4);
            hasher.write_i64(quantize_coord(*radius));
            stroke.hash(hasher);
            hasher.write_u64(stroke_width.to_bits());
            fill.hash(hasher);
            true
        }
        Annotation::Line {
            points,
            stroke,
            stroke_width,
            ..
        } => {
            hasher.write_u8(5);
            hasher.write_i64(quantize_coord(points[2] - points[0]));
            hasher.write_i64(quantize_coord(points[3] - points[1]));
            stroke.hash(hasher);
            hasher.write_u64(stroke_width.to_bits());
            true
        }
        Annotation::Arrow {
            points,
            stroke,
            stroke_width,
            arrow_style,
            bend_offset,
            ..
        } => {
            hasher.write_u8(6);
            hasher.write_i64(quantize_coord(points[2] - points[0]));
            hasher.write_i64(quantize_coord(points[3] - points[1]));
            stroke.hash(hasher);
            hasher.write_u64(stroke_width.to_bits());
            arrow_style.hash(hasher);
            match bend_offset {
                Some(offset) => {
                    hasher.write_u8(1);
                    hasher.write_i64(quantize_coord(offset.x));
                    hasher.write_i64(quantize_coord(offset.y));
                }
                None => hasher.write_u8(0),
            }
            true
        }
        Annotation::Text {
            text,
            font_size,
            fill,
            font_family,
            background_color,
            background_opacity,
            background_padding,
            background_radius,
            rotation,
            ..
        } => {
            hasher.write_u8(7);
            text.hash(hasher);
            hasher.write_u64(font_size.to_bits());
            fill.hash(hasher);
            font_family.hash(hasher);
            background_color.hash(hasher);
            background_opacity.map(f64::to_bits).hash(hasher);
            match background_padding {
                Some(offset) => {
                    hasher.write_u8(1);
                    hasher.write_i64(quantize_coord(offset.x));
                    hasher.write_i64(quantize_coord(offset.y));
                }
                None => hasher.write_u8(0),
            }
            background_radius.map(f64::to_bits).hash(hasher);
            rotation.map(quantize_coord).hash(hasher);
            true
        }
        Annotation::Number {
            display_value,
            fill,
            size,
            ..
        } => {
            hasher.write_u8(8);
            display_value.hash(hasher);
            fill.hash(hasher);
            size.hash(hasher);
            true
        }
        Annotation::Redact { .. } => false,
    }
}

fn draw_sprite_annotation(canvas: &mut Canvas, annotation: &Annotation) {
    if let Annotation::Highlight {
        points,
        fill,
        opacity,
        stroke_width,
        ..
    } = annotation
    {
        let outline = renderer::highlighter_outline(points, *stroke_width);
        let Some((first, rest)) = outline.split_first() else {
            return;
        };
        let mut builder = tiny_skia::PathBuilder::new();
        builder.move_to(first.0 as f32, first.1 as f32);
        for point in rest {
            builder.line_to(point.0 as f32, point.1 as f32);
        }
        builder.close();
        let Some(path) = builder.finish() else {
            return;
        };
        canvas.save();
        let previous_alpha = canvas.global_alpha();
        canvas.set_global_alpha(previous_alpha * *opacity as f32);
        canvas.fill_path(
            &path,
            crate::render::color::parse_or(fill, tiny_skia::Color::BLACK),
            tiny_skia::FillRule::Winding,
        );
        canvas.restore();
        return;
    }
    renderer::draw(canvas, annotation);
}

pub fn sprite_image(annotation: &Annotation, scale: f64) -> Option<PatchImage> {
    if scale <= 0.0 || matches!(annotation, Annotation::Redact { .. }) {
        return None;
    }
    let mut hasher = DefaultHasher::new();
    hasher.write_u8(0);
    if !hash_sprite_geometry(annotation, &mut hasher) {
        return None;
    }
    hasher.write_u64(scale.to_bits());
    let key = hasher.finish();
    let (ax, ay) = annotation.anchor();
    let frame = FRAME.with(|frame| frame.get());
    let cached = SPRITE_CACHE.with(|cache| {
        cache.borrow_mut().get_mut(&key).map(|entry| {
            entry.frame = frame;
            PatchImage {
                image: entry.image.clone(),
                x: ax + entry.local_x,
                y: ay + entry.local_y,
                width: entry.width,
                height: entry.height,
            }
        })
    });
    if let Some(cached) = cached {
        return Some(cached);
    }

    let mut local = annotation.clone();
    local.translate(-ax, -ay);
    let surface = empty_surface();
    let bounds = raster_bounds(&local, &surface, scale)?;
    let raw_width = (bounds.2 - bounds.0).max(1.0);
    let raw_height = (bounds.3 - bounds.1).max(1.0);
    let sprite_scale = fit_scale(raw_width, raw_height, scale)
        .min((MAX_PATCH_DIMENSION - 1.0) / raw_width)
        .min((MAX_PATCH_DIMENSION - 1.0) / raw_height);
    if sprite_scale <= 0.0 {
        return None;
    }
    let device_x = (bounds.0 * sprite_scale).floor();
    let device_y = (bounds.1 * sprite_scale).floor();
    let width = ((bounds.2 * sprite_scale).ceil() - device_x).max(1.0) as u32;
    let height = ((bounds.3 * sprite_scale).ceil() - device_y).max(1.0) as u32;
    let mut canvas = Canvas::new(width, height)?;
    canvas.translate(-device_x as f32, -device_y as f32);
    canvas.scale(sprite_scale as f32, sprite_scale as f32);
    draw_sprite_annotation(&mut canvas, &local);
    let image = render_image(canvas.pixmap());
    let local_x = device_x / sprite_scale;
    let local_y = device_y / sprite_scale;
    let patch_width = f64::from(width) / sprite_scale;
    let patch_height = f64::from(height) / sprite_scale;
    SPRITE_CACHE.with(|cache| {
        cache.borrow_mut().insert(
            key,
            SpriteEntry {
                image: image.clone(),
                local_x,
                local_y,
                width: patch_width,
                height: patch_height,
                frame,
            },
        );
    });
    Some(PatchImage {
        image,
        x: ax + local_x,
        y: ay + local_y,
        width: patch_width,
        height: patch_height,
    })
}

pub fn halo_sprite_image(
    annotation: &Annotation,
    scale: f64,
    halo_stroke_width: f32,
    rgba: [u8; 4],
) -> Option<PatchImage> {
    if scale <= 0.0
        || !matches!(
            annotation,
            Annotation::Pen { .. } | Annotation::Highlight { .. }
        )
    {
        return None;
    }
    let mut hasher = DefaultHasher::new();
    hasher.write_u8(1);
    if !hash_sprite_geometry(annotation, &mut hasher) {
        return None;
    }
    hasher.write_u64(scale.to_bits());
    hasher.write_u32(halo_stroke_width.to_bits());
    hasher.write(&rgba);
    let key = hasher.finish();
    let (ax, ay) = annotation.anchor();
    let frame = FRAME.with(|frame| frame.get());
    let cached = SPRITE_CACHE.with(|cache| {
        cache.borrow_mut().get_mut(&key).map(|entry| {
            entry.frame = frame;
            PatchImage {
                image: entry.image.clone(),
                x: ax + entry.local_x,
                y: ay + entry.local_y,
                width: entry.width,
                height: entry.height,
            }
        })
    });
    if let Some(cached) = cached {
        return Some(cached);
    }

    let mut local = annotation.clone();
    local.translate(-ax, -ay);
    let surface = empty_surface();
    let bounds = padded(
        raster_bounds(&local, &surface, scale)?,
        f64::from(halo_stroke_width) + 2.0,
    );
    let raw_width = (bounds.2 - bounds.0).max(1.0);
    let raw_height = (bounds.3 - bounds.1).max(1.0);
    let sprite_scale = fit_scale(raw_width, raw_height, scale)
        .min((MAX_PATCH_DIMENSION - 1.0) / raw_width)
        .min((MAX_PATCH_DIMENSION - 1.0) / raw_height);
    if sprite_scale <= 0.0 {
        return None;
    }
    let device_x = (bounds.0 * sprite_scale).floor();
    let device_y = (bounds.1 * sprite_scale).floor();
    let width = ((bounds.2 * sprite_scale).ceil() - device_x).max(1.0) as u32;
    let height = ((bounds.3 * sprite_scale).ceil() - device_y).max(1.0) as u32;
    let mut canvas = Canvas::new(width, height)?;
    canvas.translate(-device_x as f32, -device_y as f32);
    canvas.scale(sprite_scale as f32, sprite_scale as f32);
    let color = tiny_skia::Color::from_rgba8(rgba[0], rgba[1], rgba[2], rgba[3]);
    let stroke = tiny_skia::Stroke {
        width: halo_stroke_width,
        line_cap: tiny_skia::LineCap::Round,
        line_join: tiny_skia::LineJoin::Round,
        ..tiny_skia::Stroke::default()
    };
    let path = match &local {
        Annotation::Pen {
            points,
            stroke_width,
            ..
        } => {
            let outline = renderer::pen_outline(points, *stroke_width);
            let first = outline.first()?;
            let mut builder = tiny_skia::PathBuilder::new();
            builder.move_to(first.0 as f32, first.1 as f32);
            for (control, end) in renderer::freehand_segments(&outline) {
                builder.quad_to(
                    control.0 as f32,
                    control.1 as f32,
                    end.0 as f32,
                    end.1 as f32,
                );
            }
            builder.close();
            builder.finish()?
        }
        Annotation::Highlight {
            points,
            stroke_width,
            ..
        } => {
            let outline = renderer::highlighter_outline(points, *stroke_width);
            let (first, rest) = outline.split_first()?;
            let mut builder = tiny_skia::PathBuilder::new();
            builder.move_to(first.0 as f32, first.1 as f32);
            for point in rest {
                builder.line_to(point.0 as f32, point.1 as f32);
            }
            builder.close();
            builder.finish()?
        }
        _ => return None,
    };
    canvas.stroke_path(&path, color, &stroke);
    let image = render_image(canvas.pixmap());
    let local_x = device_x / sprite_scale;
    let local_y = device_y / sprite_scale;
    let patch_width = f64::from(width) / sprite_scale;
    let patch_height = f64::from(height) / sprite_scale;
    SPRITE_CACHE.with(|cache| {
        cache.borrow_mut().insert(
            key,
            SpriteEntry {
                image: image.clone(),
                local_x,
                local_y,
                width: patch_width,
                height: patch_height,
                frame,
            },
        );
    });
    Some(PatchImage {
        image,
        x: ax + local_x,
        y: ay + local_y,
        width: patch_width,
        height: patch_height,
    })
}

#[cfg(test)]
pub fn interactive_redact_image(
    keys: &FrameKeys,
    index: usize,
    paint: ScreenshotPaint<'_>,
    beneath: &[ScreenshotPaint<'_>],
    surface: &Surface,
    scale: f64,
) -> Option<PatchImage> {
    let fast_surface = Surface {
        rounded_clip: None,
        ..*surface
    };
    let render_scale = match paint.annotation() {
        Annotation::Redact { style, .. } if style == "blur" => {
            let base = scale.clamp(0.25, 1.0);
            if let Some(bounds) = raster_bounds(paint.annotation(), &fast_surface, base) {
                let width = (bounds.2 - bounds.0).max(1.0);
                let height = (bounds.3 - bounds.1).max(1.0);
                let pixels = width * base * height * base;
                if pixels > MAX_INTERACTIVE_BLUR_PIXELS {
                    (base * (MAX_INTERACTIVE_BLUR_PIXELS / pixels).sqrt()).max(0.25)
                } else {
                    base
                }
            } else {
                base
            }
        }
        _ => 1.0,
    };
    let key = fingerprint(keys, index, paint, beneath, &fast_surface, render_scale, 0.)?;
    let frame = FRAME.with(|frame| frame.get());
    let cached = DRAG_CACHE.with(|cache| {
        cache.borrow_mut().get_mut(&key).map(|entry| {
            entry.frame = frame;
            entry.patch.clone()
        })
    });
    if let Some(cached) = cached {
        return Some(cached);
    }

    let patch = patch(paint, beneath, &fast_surface, render_scale)?;
    let value = PatchImage {
        image: render_image(&patch.pixmap),
        x: patch.x,
        y: patch.y,
        width: patch.width,
        height: patch.height,
    };
    DRAG_CACHE.with(|cache| {
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

#[cfg(test)]
#[derive(Clone, Copy)]
struct AlignedBounds {
    left: f64,
    top: f64,
    scale_x: f64,
    scale_y: f64,
    pixels: (f64, f64, f64, f64),
}

#[cfg(test)]
fn aligned_bounds(
    paint: ScreenshotPaint<'_>,
    surface: &Surface,
    grid: SamplingGrid,
) -> Option<AlignedBounds> {
    let snap = |value: f32| {
        let value = value * grid.dpi;
        (value.abs() - 0.5).ceil().copysign(value) as f64
    };
    let left = snap(grid.stage_bounds.0);
    let top = snap(grid.stage_bounds.1);
    let scale_x = (snap(grid.stage_bounds.0 + grid.stage_bounds.2) - left) / grid.stage_size.0;
    let scale_y = (snap(grid.stage_bounds.1 + grid.stage_bounds.3) - top) / grid.stage_size.1;
    if scale_x <= 0. || scale_y <= 0. {
        return None;
    }
    let annotation = paint.annotation();
    let bounds = raster_bounds(annotation, surface, 1.)?;
    let composition_origin = surface
        .matte_image
        .filter(|_| surface.composite_backdrop)
        .map(|(_, x, y)| (x, y))
        .unwrap_or((surface.offset_x, surface.offset_y));
    let target = padded(
        paint_target_bounds(annotation, bounds, composition_origin),
        1.,
    );
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
    let mut x0 = (left + (target.0 + grid.offset.0) * scale_x).floor();
    let mut y0 = (top + (target.1 + grid.offset.1) * scale_y).floor();
    let mut x1 = (left + (target.2 + grid.offset.0) * scale_x).ceil();
    let mut y1 = (top + (target.3 + grid.offset.1) * scale_y).ceil();
    if let Some((x, y, width, height)) = grid.viewport {
        x0 = x0.max(f64::from(x * grid.dpi).floor() - 1.);
        y0 = y0.max(f64::from(y * grid.dpi).floor() - 1.);
        x1 = x1.min(f64::from((x + width) * grid.dpi).ceil() + 1.);
        y1 = y1.min(f64::from((y + height) * grid.dpi).ceil() + 1.);
    }
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    Some(AlignedBounds {
        left,
        top,
        scale_x,
        scale_y,
        pixels: (x0, y0, x1, y1),
    })
}

#[cfg(test)]
pub fn aligned_image(
    keys: &FrameKeys,
    index: usize,
    paint: ScreenshotPaint<'_>,
    beneath: &[ScreenshotPaint<'_>],
    surface: &Surface,
    grid: SamplingGrid,
) -> Option<PatchImage> {
    aligned_tile(keys, index, paint, beneath, surface, grid, None)
}

#[cfg(test)]
fn aligned_images_with_limit(
    keys: &FrameKeys,
    index: usize,
    paint: ScreenshotPaint<'_>,
    beneath: &[ScreenshotPaint<'_>],
    surface: &Surface,
    grid: SamplingGrid,
    max_pixels: f64,
) -> Option<Vec<PatchImage>> {
    if !max_pixels.is_finite() || max_pixels < 1. {
        return None;
    }
    let bounds = aligned_bounds(paint, surface, grid)?.pixels;
    let width = bounds.2 - bounds.0;
    let height = bounds.3 - bounds.1;
    if width <= MAX_PATCH_DIMENSION && height <= MAX_PATCH_DIMENSION && width * height <= max_pixels
    {
        return Some(vec![aligned_image(
            keys, index, paint, beneath, surface, grid,
        )?]);
    }
    let tile_size = max_pixels.sqrt().floor().min(MAX_PATCH_DIMENSION);
    let mut patches = Vec::new();
    let mut y = bounds.1;
    while y < bounds.3 {
        let bottom = (y + tile_size).min(bounds.3);
        let mut x = bounds.0;
        while x < bounds.2 {
            let right = (x + tile_size).min(bounds.2);
            patches.push(aligned_tile(
                keys,
                index,
                paint,
                beneath,
                surface,
                grid,
                Some((x, y, right, bottom)),
            )?);
            x = right;
        }
        y = bottom;
    }
    Some(patches)
}

#[cfg(test)]
fn aligned_tile(
    keys: &FrameKeys,
    index: usize,
    paint: ScreenshotPaint<'_>,
    beneath: &[ScreenshotPaint<'_>],
    surface: &Surface,
    grid: SamplingGrid,
    clip: Option<(f64, f64, f64, f64)>,
) -> Option<PatchImage> {
    surface.pixels?;
    let snap = |value: f32| {
        let value = value * grid.dpi;
        (value.abs() - 0.5).ceil().copysign(value) as f64
    };
    let left_snapped = snap(grid.stage_bounds.0);
    let top_snapped = snap(grid.stage_bounds.1);
    let scale_x =
        (snap(grid.stage_bounds.0 + grid.stage_bounds.2) - left_snapped) / grid.stage_size.0;
    let scale_y =
        (snap(grid.stage_bounds.1 + grid.stage_bounds.3) - top_snapped) / grid.stage_size.1;
    if scale_x <= 0. || scale_y <= 0. {
        return None;
    }
    let guard = (1. / scale_x.min(scale_y)).ceil() + 2.;
    let mut hasher = DefaultHasher::new();
    hasher.write_u64(fingerprint(
        keys, index, paint, beneath, surface, 1., guard,
    )?);
    hasher.write_u8(1);
    hasher.write_u64((left_snapped - snap(grid.origin.0)).to_bits());
    hasher.write_u64((top_snapped - snap(grid.origin.1)).to_bits());
    hasher.write_u64((snap(grid.stage_bounds.0 + grid.stage_bounds.2) - left_snapped).to_bits());
    hasher.write_u64((snap(grid.stage_bounds.1 + grid.stage_bounds.3) - top_snapped).to_bits());
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
    if let Some((x, y, width, height)) = grid.viewport {
        hasher.write_u8(1);
        for value in [x - grid.origin.0, y - grid.origin.1, width, height] {
            hasher.write_u32(value.to_bits());
        }
    } else {
        hasher.write_u8(0);
    }
    if let Some((left, top, right, bottom)) = clip {
        hasher.write_u8(1);
        for value in [
            left - snap(grid.origin.0),
            top - snap(grid.origin.1),
            right - left,
            bottom - top,
        ] {
            hasher.write_u64(value.to_bits());
        }
    } else {
        hasher.write_u8(0);
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
    let bounds = aligned_bounds(paint, surface, grid)?;
    let AlignedBounds {
        left,
        top,
        scale_x,
        scale_y,
        pixels: (mut x0, mut y0, mut x1, mut y1),
    } = bounds;
    if let Some((clip_left, clip_top, clip_right, clip_bottom)) = clip {
        x0 = x0.max(clip_left);
        y0 = y0.max(clip_top);
        x1 = x1.min(clip_right);
        y1 = y1.min(clip_bottom);
    }
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
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
    let bounds = clipped_patch_bounds(
        &raw_surface,
        patch_bounds(paint, beneath, &raw_surface, 1., guard)?,
    )?;
    let mut out = image::RgbaImage::new(width, height);
    let sampling = AlignedSampling {
        left,
        top,
        x0,
        y0,
        scale_x,
        scale_y,
        offset: grid.offset,
        matte: surface.matte,
    };
    if patch_scale(&bounds, 1.) >= 1. {
        let patch = render_patch(
            paint,
            beneath,
            &raw_surface,
            bounds.sample,
            bounds.target,
            1.,
        )?;
        sampling.sample(&mut out, &patch, (0, 0, width, height));
    } else {
        sampling.compose_tiles(&mut out, paint, beneath, &raw_surface, MAX_PATCH_PIXELS)?;
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

#[cfg(test)]
struct AlignedSampling {
    left: f64,
    top: f64,
    x0: f64,
    y0: f64,
    scale_x: f64,
    scale_y: f64,
    offset: (f64, f64),
    matte: Option<[u8; 3]>,
}

#[cfg(test)]
impl AlignedSampling {
    fn compose_tiles(
        &self,
        out: &mut image::RgbaImage,
        paint: ScreenshotPaint<'_>,
        beneath: &[ScreenshotPaint<'_>],
        surface: &Surface,
        max_pixels: f64,
    ) -> Option<()> {
        let limits = surface.bounds();
        let mut pending = vec![(0, 0, out.width(), out.height())];
        while let Some((x0, y0, x1, y1)) = pending.pop() {
            let source_x = |x: u32| {
                (self.x0 + f64::from(x) + 0.5 - self.left) / self.scale_x - self.offset.0 - 0.5
            };
            let source_y = |y: u32| {
                (self.y0 + f64::from(y) + 0.5 - self.top) / self.scale_y - self.offset.1 - 0.5
            };
            let left = source_x(x0)
                .floor()
                .clamp(limits.0.floor(), limits.2.ceil() - 1.);
            let top = source_y(y0)
                .floor()
                .clamp(limits.1.floor(), limits.3.ceil() - 1.);
            let right = (source_x(x1 - 1).floor() + 2.).clamp(left + 1., limits.2.ceil());
            let bottom = (source_y(y1 - 1).floor() + 2.).clamp(top + 1., limits.3.ceil());
            let sample = replay_bounds(
                (left, top, right, bottom),
                beneath.iter().copied().chain(std::iter::once(paint)),
                surface,
            );
            let sample_width = sample.2.ceil() - sample.0.floor();
            let sample_height = sample.3.ceil() - sample.1.floor();
            if sample_width > MAX_PATCH_DIMENSION - 1.
                || sample_height > MAX_PATCH_DIMENSION - 1.
                || sample_width * sample_height > max_pixels
            {
                if x1 - x0 >= y1 - y0 && x1 - x0 > 1 {
                    let middle = x0 + (x1 - x0) / 2;
                    pending.extend([(x0, y0, middle, y1), (middle, y0, x1, y1)]);
                    continue;
                }
                if y1 - y0 > 1 {
                    let middle = y0 + (y1 - y0) / 2;
                    pending.extend([(x0, y0, x1, middle), (x0, middle, x1, y1)]);
                    continue;
                }
                return None;
            }
            let patch = render_patch(paint, beneath, surface, sample, sample, 1.)?;
            if f64::from(patch.pixmap.width()) != patch.width
                || f64::from(patch.pixmap.height()) != patch.height
            {
                return None;
            }
            self.sample(out, &patch, (x0, y0, x1, y1));
        }
        Some(())
    }

    fn sample(&self, out: &mut image::RgbaImage, patch: &Patch, bounds: (u32, u32, u32, u32)) {
        let Self {
            left,
            top,
            x0,
            y0,
            scale_x,
            scale_y,
            offset,
            matte,
        } = *self;
        let width = out.width();
        let source = crate::editor::export::to_rgba(&patch.pixmap);
        let xs = (bounds.0..bounds.2)
            .map(|x| {
                let sx = (x0 + f64::from(x) + 0.5 - left) / scale_x - offset.0 - patch.x - 0.5;
                let ix = sx.floor();
                let first = ix.max(0.).min(f64::from(source.width() - 1)) as usize;
                let second = (ix + 1.).max(0.).min(f64::from(source.width() - 1)) as usize;
                (first, second, (sx - ix) as f32)
            })
            .collect::<Vec<_>>();
        let stride = source.width() as usize;
        let source_pixels = source.as_raw().as_chunks::<4>().0;
        let premultiply = source_pixels.iter().any(|pixel| pixel[3] < 255);
        let output: &mut [u8] = out.as_mut();
        for (y, row) in output
            .as_chunks_mut::<4>()
            .0
            .chunks_exact_mut(width as usize)
            .enumerate()
            .skip(bounds.1 as usize)
            .take((bounds.3 - bounds.1) as usize)
        {
            let sy = (y0 + y as f64 + 0.5 - top) / scale_y - offset.1 - patch.y - 0.5;
            let iy = sy.floor();
            let fy = (sy - iy) as f32;
            let first_y = iy.max(0.).min(f64::from(source.height() - 1)) as usize * stride;
            let second_y = (iy + 1.).max(0.).min(f64::from(source.height() - 1)) as usize * stride;
            for (pixel, &(first_x, second_x, fx)) in row[bounds.0 as usize..bounds.2 as usize]
                .iter_mut()
                .zip(&xs)
            {
                let pixels = [
                    source_pixels[first_y + first_x],
                    source_pixels[first_y + second_x],
                    source_pixels[second_y + first_x],
                    source_pixels[second_y + second_x],
                ];
                let mut sample = [0.0_f32; 4];
                for c in 0..4 {
                    let channel = |pixel: [u8; 4]| {
                        let value = f32::from(pixel[c]);
                        if premultiply && c < 3 {
                            value * f32::from(pixel[3]) / 255.
                        } else {
                            value
                        }
                    };
                    let a = channel(pixels[0]) * (1. - fx) + channel(pixels[1]) * fx;
                    let b = channel(pixels[2]) * (1. - fx) + channel(pixels[3]) * fx;
                    sample[c] = a * (1. - fy) + b * fy;
                }
                if premultiply && sample[3] > 0. {
                    for c in 0..3 {
                        sample[c] *= 255. / sample[3];
                    }
                }
                if let Some(background) = matte {
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
    }
}

fn hash_of(paint: &ScreenshotPaint<'_>) -> u64 {
    let mut hasher = DefaultHasher::new();
    match paint {
        ScreenshotPaint::Annotation(annotation) => {
            hasher.write_u8(0);
            hash_annotation(annotation, &mut hasher);
        }
        ScreenshotPaint::BlackoutOverlay(annotation) => {
            hasher.write_u8(1);
            hash_annotation(annotation, &mut hasher);
        }
    }
    hasher.finish()
}

fn hash_annotation(annotation: &Annotation, hasher: &mut DefaultHasher) {
    match annotation {
        Annotation::Pen {
            id,
            points,
            stroke,
            stroke_width,
        } => {
            hasher.write_u8(1);
            id.hash(hasher);
            hasher.write_usize(points.len());
            for value in points {
                hasher.write_u64(value.to_bits());
            }
            stroke.hash(hasher);
            hasher.write_u64(stroke_width.to_bits());
        }
        Annotation::Highlight {
            id,
            points,
            fill,
            opacity,
            stroke_width,
        } => {
            hasher.write_u8(2);
            id.hash(hasher);
            hasher.write_usize(points.len());
            for value in points {
                hasher.write_u64(value.to_bits());
            }
            fill.hash(hasher);
            hasher.write_u64(opacity.to_bits());
            hasher.write_u64(stroke_width.to_bits());
        }
        Annotation::Rectangle {
            id,
            x,
            y,
            width,
            height,
            stroke,
            stroke_width,
            fill,
        } => {
            hasher.write_u8(3);
            id.hash(hasher);
            hasher.write_u64(x.to_bits());
            hasher.write_u64(y.to_bits());
            hasher.write_u64(width.to_bits());
            hasher.write_u64(height.to_bits());
            stroke.hash(hasher);
            hasher.write_u64(stroke_width.to_bits());
            fill.hash(hasher);
        }
        Annotation::Circle {
            id,
            x,
            y,
            radius,
            stroke,
            stroke_width,
            fill,
        } => {
            hasher.write_u8(4);
            id.hash(hasher);
            hasher.write_u64(x.to_bits());
            hasher.write_u64(y.to_bits());
            hasher.write_u64(radius.to_bits());
            stroke.hash(hasher);
            hasher.write_u64(stroke_width.to_bits());
            fill.hash(hasher);
        }
        Annotation::Line {
            id,
            points,
            stroke,
            stroke_width,
        } => {
            hasher.write_u8(5);
            id.hash(hasher);
            for value in points {
                hasher.write_u64(value.to_bits());
            }
            stroke.hash(hasher);
            hasher.write_u64(stroke_width.to_bits());
        }
        Annotation::Arrow {
            id,
            points,
            stroke,
            stroke_width,
            arrow_style,
            bend_offset,
        } => {
            hasher.write_u8(6);
            id.hash(hasher);
            for value in points {
                hasher.write_u64(value.to_bits());
            }
            stroke.hash(hasher);
            hasher.write_u64(stroke_width.to_bits());
            arrow_style.hash(hasher);
            match bend_offset {
                Some(offset) => {
                    hasher.write_u8(1);
                    hasher.write_u64(offset.x.to_bits());
                    hasher.write_u64(offset.y.to_bits());
                }
                None => hasher.write_u8(0),
            }
        }
        Annotation::Text {
            id,
            x,
            y,
            text,
            font_size,
            fill,
            font_family,
            background_color,
            background_opacity,
            background_padding,
            background_radius,
            rotation,
        } => {
            hasher.write_u8(7);
            id.hash(hasher);
            hasher.write_u64(x.to_bits());
            hasher.write_u64(y.to_bits());
            text.hash(hasher);
            hasher.write_u64(font_size.to_bits());
            fill.hash(hasher);
            font_family.hash(hasher);
            background_color.hash(hasher);
            background_opacity.map(f64::to_bits).hash(hasher);
            match background_padding {
                Some(offset) => {
                    hasher.write_u8(1);
                    hasher.write_u64(offset.x.to_bits());
                    hasher.write_u64(offset.y.to_bits());
                }
                None => hasher.write_u8(0),
            }
            background_radius.map(f64::to_bits).hash(hasher);
            rotation.map(f64::to_bits).hash(hasher);
        }
        Annotation::Number {
            id,
            x,
            y,
            value,
            display_value,
            fill,
            size,
        } => {
            hasher.write_u8(8);
            id.hash(hasher);
            hasher.write_u64(x.to_bits());
            hasher.write_u64(y.to_bits());
            hasher.write_u64(value.to_bits());
            display_value.hash(hasher);
            fill.hash(hasher);
            size.hash(hasher);
        }
        Annotation::Redact {
            id,
            x,
            y,
            width,
            height,
            style,
            intensity,
        } => {
            hasher.write_u8(9);
            id.hash(hasher);
            hasher.write_u64(x.to_bits());
            hasher.write_u64(y.to_bits());
            hasher.write_u64(width.to_bits());
            hasher.write_u64(height.to_bits());
            style.hash(hasher);
            hasher.write_u64(intensity.to_bits());
        }
    }
}

#[cfg(test)]
fn dependency_hash(
    keys: &FrameKeys,
    paint: ScreenshotPaint<'_>,
    beneath: &[ScreenshotPaint<'_>],
    surface: &Surface,
    scale: f64,
    padding: f64,
) -> Option<u64> {
    if surface.pixels.is_none() || beneath.is_empty() {
        return Some(0);
    }
    let sample = patch_bounds(paint, beneath, surface, scale, padding)
        .and_then(|bounds| clipped_patch_bounds(surface, bounds))
        .map(|bounds| bounds.sample);
    let mut running = 0u64;
    for (earlier_index, earlier) in beneath.iter().enumerate() {
        if let Some(sample) = sample {
            if let Some(other) = raster_bounds(earlier.annotation(), surface, scale) {
                if !bounds_overlap(other, sample) {
                    continue;
                }
            }
        }
        running = running.rotate_left(7) ^ *keys.hashes.get(earlier_index)?;
    }
    Some(running)
}

#[cfg(test)]
fn fingerprint(
    keys: &FrameKeys,
    index: usize,
    paint: ScreenshotPaint<'_>,
    beneath: &[ScreenshotPaint<'_>],
    surface: &Surface,
    scale: f64,
    padding: f64,
) -> Option<u64> {
    let mut hasher = DefaultHasher::new();
    hasher.write_u64(*keys.hashes.get(index)?);
    hasher.write_u64(dependency_hash(
        keys, paint, beneath, surface, scale, padding,
    )?);
    hasher.write_u64(scale.to_bits());
    hasher.write_u32(surface.width);
    hasher.write_u32(surface.height);
    hasher.write_u64(surface.offset_x.to_bits());
    hasher.write_u64(surface.offset_y.to_bits());
    hasher.write_u8(u8::from(surface.source_is_composed));
    hasher.write_u8(u8::from(surface.align_to_composition_grid));
    hasher.write_u64(surface.native_scale.to_bits());
    if let Some(clip) = surface.rounded_clip {
        hasher.write_u8(1);
        hasher.write_u32(clip.width);
        hasher.write_u32(clip.height);
        hasher.write_i64(clip.offset_x);
        hasher.write_i64(clip.offset_y);
        for edge in clip.edges {
            edge.image.id.hash(&mut hasher);
        }
    } else {
        hasher.write_u8(0);
    }
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
    let mut buffer = image::RgbaImage::new(pixmap.width(), pixmap.height());
    for (pixel, output) in pixmap
        .data()
        .as_chunks::<4>()
        .0
        .iter()
        .zip(buffer.as_chunks_mut::<4>().0.iter_mut())
    {
        let alpha = pixel[3];
        *output = if alpha == 0 {
            [0, 0, 0, 0]
        } else if alpha == 255 {
            [pixel[2], pixel[1], pixel[0], 255]
        } else {
            let alpha_u32 = alpha as u32;
            let half = alpha_u32 / 2;
            let unpremultiply =
                |value: u8| -> u8 { ((value as u32 * 255 + half) / alpha_u32).min(255) as u8 };
            [
                unpremultiply(pixel[2]),
                unpremultiply(pixel[1]),
                unpremultiply(pixel[0]),
                alpha,
            ]
        };
    }
    Arc::new(RenderImage::new(smallvec::smallvec![image::Frame::new(
        buffer
    )]))
}

#[cfg(test)]
mod rounded_tests;

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

    #[test]
    fn aligned_output_tiles_cover_the_single_patch_without_seams() {
        let base = base_pixmap(120, 90);
        let image = render_image(&base);
        let surface = Surface {
            image_id: Some(image.id),
            width: 120,
            height: 90,
            offset_x: 0.,
            offset_y: 0.,
            source_is_composed: true,
            align_to_composition_grid: false,
            native_scale: 1.,
            rounded_clip: None,
            pixels: image.as_bytes(0),
            matte: None,
            matte_image: None,
            composite_backdrop: false,
        };
        let grid = SamplingGrid {
            origin: (100.25, 50.75),
            stage_bounds: (100.25, 50.75, 150., 112.5),
            viewport: Some((100., 50., 150., 113.)),
            stage_size: (120., 90.),
            offset: (0., 0.),
            zoom: 1.25,
            dpi: 1.,
        };
        for style in ["pixelate", "blur"] {
            let annotation = Annotation::Redact {
                id: style.into(),
                x: 0.,
                y: 0.,
                width: 120.,
                height: 90.,
                style: style.into(),
                intensity: 2.,
            };
            let paint = ScreenshotPaint::Annotation(&annotation);
            let keys = super::begin_frame(&[paint]);
            let full = super::aligned_image(&keys, 0, paint, &[], &surface, grid).unwrap();
            let tiles =
                super::aligned_images_with_limit(&keys, 0, paint, &[], &surface, grid, 400.)
                    .unwrap();
            let repeated =
                super::aligned_images_with_limit(&keys, 0, paint, &[], &surface, grid, 400.)
                    .unwrap();
            assert!(tiles.len() > 1);
            let size = full.image.size(0);
            let width = u32::from(size.width) as usize;
            let height = u32::from(size.height) as usize;
            let source = full.image.as_bytes(0).unwrap().as_chunks::<4>().0;
            let mut coverage = vec![false; width * height];
            for (tile, cached) in tiles.iter().zip(&repeated) {
                assert!(Arc::ptr_eq(&tile.image, &cached.image));
                let size = tile.image.size(0);
                let tile_width = u32::from(size.width) as usize;
                let tile_height = u32::from(size.height) as usize;
                let x = ((tile.x - full.x) * f64::from(grid.zoom * grid.dpi)).round() as usize;
                let y = ((tile.y - full.y) * f64::from(grid.zoom * grid.dpi)).round() as usize;
                let pixels = tile.image.as_bytes(0).unwrap().as_chunks::<4>().0;
                for row in 0..tile_height {
                    for column in 0..tile_width {
                        let index = (y + row) * width + x + column;
                        assert!(!coverage[index]);
                        coverage[index] = true;
                        assert_eq!(
                            pixels[row * tile_width + column],
                            source[index],
                            "{style}, pixel {index}"
                        );
                    }
                }
            }
            assert!(coverage.into_iter().all(|covered| covered));
        }
    }

    #[test]
    fn transparent_source_redactions_match_the_export_pixels() {
        let rgba = image::RgbaImage::from_fn(64, 48, |x, y| {
            image::Rgba([
                if (x / 3 + y / 5) % 2 == 0 { 24 } else { 224 },
                (y % 32 * 6) as u8,
                80,
                ((x + y * 3) % 256) as u8,
            ])
        });
        let mut bgra = rgba.clone();
        for pixel in bgra.as_chunks_mut::<4>().0 {
            pixel.swap(0, 2);
        }
        let source = Arc::new(RenderImage::new(smallvec::smallvec![image::Frame::new(
            bgra
        )]));
        let surface = Surface {
            image_id: Some(source.id),
            width: 64,
            height: 48,
            offset_x: 0.,
            offset_y: 0.,
            source_is_composed: false,
            align_to_composition_grid: false,
            native_scale: 1.,
            rounded_clip: None,
            pixels: source.as_bytes(0),
            matte: None,
            matte_image: None,
            composite_backdrop: false,
        };
        for style in ["pixelate", "blur"] {
            let annotation = Annotation::Redact {
                id: style.into(),
                x: 0.,
                y: 0.,
                width: 64.,
                height: 48.,
                style: style.into(),
                intensity: 5.,
            };
            let mut expected = Canvas::new(64, 48).unwrap();
            let image = crate::editor::export::from_rgba(&rgba).unwrap();
            expected.draw_pixmap(image.as_ref(), 0., 0., 64., 48.);
            renderer::draw_screenshot(&mut expected, &annotation, 1.);
            let actual = patch(&annotation, &[], &surface, 1.).unwrap();
            assert_eq!(actual.pixmap.data(), expected.pixmap().data(), "{style}");
        }
    }

    #[test]
    fn bounded_redaction_replay_matches_a_full_resolution_patch() {
        let base = base_pixmap(120, 90);
        let bgra = to_bgra(&base);
        let surface = Surface {
            image_id: None,
            width: 120,
            height: 90,
            offset_x: 0.,
            offset_y: 0.,
            source_is_composed: true,
            align_to_composition_grid: false,
            native_scale: 1.,
            rounded_clip: None,
            pixels: Some(&bgra),
            matte: None,
            matte_image: None,
            composite_backdrop: false,
        };
        for styles in [
            ["pixelate", "blur"],
            ["blur", "pixelate"],
            ["pixelate", "pixelate"],
        ] {
            let annotations: Vec<_> = styles
                .iter()
                .enumerate()
                .map(|(index, style)| Annotation::Redact {
                    id: index.to_string(),
                    x: -1.,
                    y: -1.,
                    width: 122.,
                    height: 92.,
                    style: (*style).into(),
                    intensity: 1.,
                })
                .collect();
            let paints = renderer::screenshot_paints(annotations.iter());
            for scale in [0.4, 0.75, 1.25] {
                let width = (120. * scale) as u32;
                let height = (90. * scale) as u32;
                let sampling = AlignedSampling {
                    left: 0.,
                    top: 0.,
                    x0: 0.,
                    y0: 0.,
                    scale_x: scale,
                    scale_y: scale,
                    offset: (0., 0.),
                    matte: None,
                };
                let patch = super::patch_padded(
                    paints[1],
                    &paints[..1],
                    &surface,
                    1.,
                    (1. / scale).ceil() + 2.,
                )
                .unwrap();
                let mut expected = image::RgbaImage::new(width, height);
                sampling.sample(&mut expected, &patch, (0, 0, width, height));
                let mut tiled = image::RgbaImage::new(width, height);
                sampling
                    .compose_tiles(&mut tiled, paints[1], &paints[..1], &surface, 2500.)
                    .unwrap();
                assert_eq!(tiled.as_raw(), expected.as_raw(), "{styles:?} at {scale}");
            }
        }
    }

    #[test]
    fn bounded_redaction_replay_preserves_transparency_and_offsets() {
        let mut bgra = to_bgra(&base_pixmap(120, 90));
        for (index, pixel) in bgra.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            pixel[3] = ((index * 17) % 256) as u8;
        }
        for (offset_x, offset_y) in [(7., 13.), (2.5, 4.5)] {
            let surface = Surface {
                image_id: None,
                width: 120,
                height: 90,
                offset_x,
                offset_y,
                source_is_composed: true,
                align_to_composition_grid: false,
                native_scale: 1.,
                rounded_clip: None,
                pixels: Some(&bgra),
                matte: None,
                matte_image: None,
                composite_backdrop: false,
            };
            let annotations = [
                Annotation::Rectangle {
                    id: "shape".into(),
                    x: 17.25,
                    y: 11.75,
                    width: 65.,
                    height: 49.,
                    stroke: "#ff0000".into(),
                    stroke_width: 3.,
                    fill: Some("#00ff00".into()),
                },
                Annotation::Redact {
                    id: "pixelate".into(),
                    x: -offset_x - 1.,
                    y: -offset_y - 1.,
                    width: 122.,
                    height: 92.,
                    style: "pixelate".into(),
                    intensity: 2.,
                },
                Annotation::Redact {
                    id: "blur".into(),
                    x: -offset_x - 1.,
                    y: -offset_y - 1.,
                    width: 122.,
                    height: 92.,
                    style: "blur".into(),
                    intensity: 1.,
                },
            ];
            let paints = renderer::screenshot_paints(annotations.iter());
            for scale in [0.4, 1.25] {
                let width = (120. * scale) as u32;
                let height = (90. * scale) as u32;
                let sampling = AlignedSampling {
                    left: 0.,
                    top: 0.,
                    x0: 0.,
                    y0: 0.,
                    scale_x: scale,
                    scale_y: scale,
                    offset: (offset_x, offset_y),
                    matte: Some([28, 32, 36]),
                };
                let patch = super::patch_padded(
                    paints[2],
                    &paints[..2],
                    &surface,
                    1.,
                    (1. / scale).ceil() + 2.,
                )
                .unwrap();
                let mut expected = image::RgbaImage::new(width, height);
                sampling.sample(&mut expected, &patch, (0, 0, width, height));
                let mut tiled = image::RgbaImage::new(width, height);
                sampling
                    .compose_tiles(&mut tiled, paints[2], &paints[..2], &surface, 2500.)
                    .unwrap();
                assert_eq!(
                    tiled.as_raw(),
                    expected.as_raw(),
                    "offset ({offset_x}, {offset_y}) at {scale}"
                );
            }
        }
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
            source_is_composed: true,
            align_to_composition_grid: false,
            native_scale: 1.,
            rounded_clip: None,
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
        let all_paints = paints(&refs);
        let keys = begin_frame(&refs);
        for (index, annotation) in annotations.iter().enumerate() {
            assert!(is_rasterized(annotation));
            assert!(fingerprint(
                &keys,
                index,
                all_paints[index],
                &all_paints[..index],
                &surface,
                scale,
                0.,
            )
            .is_some());
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
            source_is_composed: true,
            align_to_composition_grid: false,
            native_scale: 1.,
            rounded_clip: None,
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
            source_is_composed: true,
            align_to_composition_grid: false,
            native_scale: 1.0,
            rounded_clip: None,
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
    fn zero_padding_framed_attachment_redaction_patches_match_export() {
        use crate::editor::layers::{Edge, ImageLayer};

        let directory = tempfile::tempdir().unwrap();
        let attachment_path = directory.path().join("right.png");
        image::RgbaImage::from_pixel(30, 60, image::Rgba([72, 192, 112, 255]))
            .save(&attachment_path)
            .unwrap();
        let layers = [ImageLayer {
            id: "right".into(),
            image_url: attachment_path.to_string_lossy().into_owned(),
            natural_width: 30.0,
            natural_height: 60.0,
            edge: Edge::Right,
        }];
        let base = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            160,
            120,
            image::Rgba([64, 112, 192, 255]),
        ));
        let annotations = [
            Annotation::Rectangle {
                id: "shape".into(),
                x: 138.0,
                y: 36.0,
                width: 40.0,
                height: 20.0,
                stroke: "#f43f5e".into(),
                stroke_width: 3.0,
                fill: Some("#f0c040".into()),
            },
            Annotation::Redact {
                id: "blackout".into(),
                x: 152.0,
                y: 42.0,
                width: 20.0,
                height: 12.0,
                style: "blackout".into(),
                intensity: 5.0,
            },
        ];
        let mut wallpaper = crate::editor::wallpaper::WallpaperSettings::default();
        wallpaper.window_frame.style = "windows-dark".into();
        let export = crate::editor::export::compose_with_layers(
            Some(&base),
            160,
            120,
            &annotations,
            &wallpaper,
            &layers,
            1.0,
        );
        let (backdrop, _, composite) = crate::editor::export::render_backdrop(
            &wallpaper,
            export.width(),
            export.height(),
            160.0,
            120.0,
            None,
            &layers,
            Some(&base),
            1.0,
            [18, 18, 20],
        )
        .unwrap();
        assert!(composite);
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
        let mut backdrop_rgba = backdrop.as_bytes(0).unwrap().to_vec();
        for pixel in backdrop_rgba.as_chunks_mut::<4>().0 {
            pixel.swap(0, 2);
        }
        let mut backdrop_bgra = backdrop_rgba.clone();
        for pixel in backdrop_bgra.as_chunks_mut::<4>().0 {
            pixel.swap(0, 2);
        }
        let surface = Surface {
            image_id: None,
            width: export.width(),
            height: export.height(),
            offset_x: primary_origin.0,
            offset_y: primary_origin.1,
            source_is_composed: true,
            align_to_composition_grid: false,
            native_scale: 1.0,
            rounded_clip: None,
            pixels: Some(&backdrop_bgra),
            matte: Some([18, 18, 20]),
            matte_image: None,
            composite_backdrop: false,
        };
        let operations = renderer::screenshot_paints(annotations.iter());
        let seam_sample = (
            (primary_origin.0 + 162.0) as u32,
            (primary_origin.1 + 48.0) as u32,
        );
        let seam_offset = ((seam_sample.1 * export.width() + seam_sample.0) * 4) as usize;
        assert_eq!(
            &backdrop_rgba[seam_offset..seam_offset + 4],
            &[72, 192, 112, 255]
        );
        let backdrop_rgba =
            image::RgbaImage::from_raw(export.width(), export.height(), backdrop_rgba).unwrap();
        let mut preview = crate::editor::export::from_rgba(&backdrop_rgba).unwrap();
        let paint = PixmapPaint {
            opacity: 1.0,
            blend_mode: BlendMode::Source,
            quality: FilterQuality::Nearest,
        };
        let mut touched = vec![false; export.as_raw().len() / 4];
        for (index, operation) in operations.iter().enumerate() {
            let patch = super::patch(*operation, &operations[..index], &surface, 1.0).unwrap();
            let patch_x = (primary_origin.0 + patch.x).round() as i32;
            let patch_y = (primary_origin.1 + patch.y).round() as i32;
            for y in 0..patch.pixmap.height() {
                for x in 0..patch.pixmap.width() {
                    let output_x = patch_x + x as i32;
                    let output_y = patch_y + y as i32;
                    if output_x >= 0
                        && output_y >= 0
                        && output_x < export.width() as i32
                        && output_y < export.height() as i32
                    {
                        touched[(output_y as u32 * export.width() + output_x as u32) as usize] =
                            true;
                    }
                }
            }
            preview.draw_pixmap(
                patch_x,
                patch_y,
                patch.pixmap.as_ref(),
                &paint,
                Transform::identity(),
                None,
            );
        }
        let preview_rgba = crate::editor::export::to_rgba(&preview);
        for (pixel_index, (&is_touched, (actual, expected))) in touched
            .iter()
            .zip(
                preview_rgba
                    .as_raw()
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .zip(export.as_raw().as_chunks::<4>().0),
            )
            .enumerate()
        {
            if is_touched {
                for (channel, (actual, expected)) in actual.iter().zip(expected).enumerate() {
                    assert!(
                        actual.abs_diff(*expected) <= 2,
                        "zero-padding framed attachment pixel {pixel_index}, channel {channel}: {actual} vs {expected}"
                    );
                }
            }
        }
        assert_eq!(
            export.get_pixel(seam_sample.0, seam_sample.1).0,
            [0, 0, 0, 255]
        );
        assert_eq!(
            preview_rgba.get_pixel(seam_sample.0, seam_sample.1).0,
            [0, 0, 0, 255]
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
            source_is_composed: true,
            align_to_composition_grid: false,
            native_scale: 1.0,
            rounded_clip: None,
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
            source_is_composed: true,
            align_to_composition_grid: false,
            native_scale: 1.,
            rounded_clip: None,
            pixels: first.as_bytes(0),
            matte: Some([240, 240, 240]),
            matte_image: None,
            composite_backdrop: false,
        };
        let grid = SamplingGrid {
            origin: (100., 50.),
            stage_bounds: (100., 50., 270., 210.),
            viewport: None,
            stage_size: (180., 140.),
            offset: (0., 0.),
            zoom: 1.5,
            dpi: 1.,
        };
        let keys = begin_frame(&[&annotation]);
        let first_patch = aligned_image(&keys, 0, &annotation, &[], &surface, grid).unwrap();
        let repeated = aligned_image(&keys, 0, &annotation, &[], &surface, grid).unwrap();
        assert!(Arc::ptr_eq(&first_patch.image, &repeated.image));
        let viewport = SamplingGrid {
            viewport: Some((150., 90., 36., 32.)),
            ..grid
        };
        let clipped = aligned_image(&keys, 0, &annotation, &[], &surface, viewport).unwrap();
        assert!(clipped.width < first_patch.width);
        let full_size = first_patch.image.size(0);
        let clipped_size = clipped.image.size(0);
        let full_width = u32::from(full_size.width) as usize;
        let clipped_width = u32::from(clipped_size.width) as usize;
        let clipped_height = u32::from(clipped_size.height) as usize;
        let x = ((clipped.x - first_patch.x) * f64::from(grid.zoom * grid.dpi)).round() as usize;
        let y = ((clipped.y - first_patch.y) * f64::from(grid.zoom * grid.dpi)).round() as usize;
        let full_pixels = first_patch.image.as_bytes(0).unwrap().as_chunks::<4>().0;
        let clipped_pixels = clipped.image.as_bytes(0).unwrap().as_chunks::<4>().0;
        for row in 0..clipped_height {
            assert_eq!(
                &clipped_pixels[row * clipped_width..(row + 1) * clipped_width],
                &full_pixels
                    [(row + y) * full_width + x..(row + y) * full_width + x + clipped_width],
            );
        }
        let shifted_viewport = aligned_image(
            &keys,
            0,
            &annotation,
            &[],
            &surface,
            SamplingGrid {
                viewport: Some((160., 90., 36., 32.)),
                ..grid
            },
        )
        .unwrap();
        assert!(!Arc::ptr_eq(&clipped.image, &shifted_viewport.image));
        assert!(shifted_viewport.x > clipped.x);
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
            source_is_composed: true,
            align_to_composition_grid: false,
            native_scale: 1.,
            rounded_clip: None,
            pixels: Some(&[]),
            matte: None,
            matte_image: None,
            composite_backdrop: false,
        };
        let grid = SamplingGrid {
            origin: (0., 0.),
            stage_bounds: (0., 0., 19_200., 160.),
            viewport: None,
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
            source_is_composed: true,
            align_to_composition_grid: false,
            native_scale: 1.,
            rounded_clip: None,
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
    fn fractional_blackout_boundary_matches_export_with_composition_offsets() {
        let base = base_pixmap(80, 60);
        let bgra = to_bgra(&base);
        let annotations = [Annotation::Redact {
            id: "blackout".into(),
            x: 8.8,
            y: 3.8,
            width: 12.3,
            height: 12.3,
            style: "blackout".into(),
            intensity: 5.,
        }];
        let operations = renderer::screenshot_paints(annotations.iter());
        let paint = PixmapPaint {
            opacity: 1.,
            blend_mode: BlendMode::Source,
            quality: FilterQuality::Nearest,
        };
        for (offset_x, offset_y, scale) in
            [(0., 0., 1.), (5., 4., 1.), (4., 4., 1.25), (4.5, 3.5, 2.)]
        {
            let surface = Surface {
                image_id: None,
                width: base.width(),
                height: base.height(),
                offset_x,
                offset_y,
                source_is_composed: true,
                align_to_composition_grid: false,
                native_scale: 1.,
                rounded_clip: None,
                pixels: Some(&bgra),
                matte: None,
                matte_image: None,
                composite_backdrop: false,
            };
            let mut expected = Canvas::new((80. * scale) as u32, (60. * scale) as u32).unwrap();
            expected.scale(scale as f32, scale as f32);
            expected.draw_pixmap(base.as_ref(), 0., 0., 80., 60.);
            let mut actual = expected.pixmap().clone();
            expected.translate(offset_x as f32, offset_y as f32);
            for (index, operation) in operations.iter().enumerate() {
                renderer::draw_screenshot_paint(&mut expected, *operation, 1.);
                let patch =
                    super::patch(*operation, &operations[..index], &surface, scale).unwrap();
                actual.draw_pixmap(
                    ((patch.x + offset_x) * scale).round() as i32,
                    ((patch.y + offset_y) * scale).round() as i32,
                    patch.pixmap.as_ref(),
                    &paint,
                    Transform::identity(),
                    None,
                );
                for (channel, (actual, expected)) in actual
                    .data()
                    .iter()
                    .zip(expected.pixmap().data())
                    .enumerate()
                {
                    assert!(
                        actual.abs_diff(*expected) <= 2,
                        "operation {index}, offset ({offset_x}, {offset_y}), {scale}x, channel {channel}: {actual} vs {expected}"
                    );
                }
            }
        }
    }

    #[test]
    fn blackout_and_shape_patches_match_export_at_fractional_origins() {
        let scales: [f64; 5] = [0.73, 1., 1.25, 1.5, 2.];
        let origins = [(32.25, 22.375), (32.5, 22.5)];
        let paint = PixmapPaint {
            opacity: 1.,
            blend_mode: BlendMode::Source,
            quality: FilterQuality::Nearest,
        };
        for scale in scales {
            for &(x, y) in &origins {
                let width = (100. * scale).ceil() as u32;
                let height = (80. * scale).ceil() as u32;
                let mut base = Pixmap::new(width, height).expect("base");
                base.fill(tiny_skia::Color::from_rgba8(64, 128, 192, 255));
                let bgra = to_bgra(&base);
                let surface = Surface {
                    image_id: None,
                    width,
                    height,
                    offset_x: 0.,
                    offset_y: 0.,
                    source_is_composed: true,
                    align_to_composition_grid: false,
                    native_scale: 1.,
                    rounded_clip: None,
                    pixels: Some(&bgra),
                    matte: None,
                    matte_image: None,
                    composite_backdrop: false,
                };
                let shape = Annotation::Rectangle {
                    id: "shape".into(),
                    x: (x - 2.) * scale,
                    y: (y - 2.) * scale,
                    width: 24. * scale,
                    height: 24. * scale,
                    stroke: "#ff00ff".into(),
                    stroke_width: scale,
                    fill: Some("#ff00ff".into()),
                };
                let blackout = Annotation::Redact {
                    id: "blackout".into(),
                    x: x * scale,
                    y: y * scale,
                    width: 20. * scale,
                    height: 20. * scale,
                    style: "blackout".into(),
                    intensity: 2.,
                };
                for (order, annotations) in [
                    ("shape-blackout", vec![shape.clone(), blackout.clone()]),
                    ("blackout-shape", vec![blackout.clone(), shape.clone()]),
                ] {
                    let mut exported = Canvas::new(width, height).unwrap();
                    exported.draw_pixmap(base.as_ref(), 0., 0., width as f32, height as f32);
                    let mut preview = exported.pixmap().clone();
                    renderer::draw_all_screenshot(&mut exported, &annotations, 1.);
                    let operations = renderer::screenshot_paints(annotations.iter());
                    for (index, operation) in operations.iter().enumerate() {
                        let patch =
                            super::patch(*operation, &operations[..index], &surface, 1.).unwrap();
                        preview.draw_pixmap(
                            patch.x.round() as i32,
                            patch.y.round() as i32,
                            patch.pixmap.as_ref(),
                            &paint,
                            Transform::identity(),
                            None,
                        );
                    }
                    let exported = exported.into_pixmap();
                    assert_eq!(
                        (exported.width(), exported.height()),
                        (preview.width(), preview.height())
                    );
                    for (index, (actual, expected)) in
                        preview.data().iter().zip(exported.data()).enumerate()
                    {
                        assert!(
                            actual.abs_diff(*expected) <= 2,
                            "{order}, {scale}x, origin ({x},{y}), channel {index}: {actual} vs {expected}"
                        );
                    }
                }
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
            source_is_composed: true,
            align_to_composition_grid: false,
            native_scale: 1.,
            rounded_clip: None,
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
            source_is_composed: true,
            align_to_composition_grid: false,
            native_scale: 1.,
            rounded_clip: None,
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
            source_is_composed: true,
            align_to_composition_grid: false,
            native_scale: 1.,
            rounded_clip: None,
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
                    source_is_composed: true,
                    align_to_composition_grid: false,
                    native_scale: 1.,
                    rounded_clip: None,
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
                source_is_composed: true,
                align_to_composition_grid: false,
                native_scale: 1.,
                rounded_clip: None,
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
            source_is_composed: true,
            align_to_composition_grid: false,
            native_scale: 2.,
            rounded_clip: None,
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
                source_is_composed: true,
                align_to_composition_grid: false,
                native_scale: 1.25,
                rounded_clip: None,
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
                source_is_composed: true,
                align_to_composition_grid: false,
                native_scale: 1.,
                rounded_clip: None,
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
            source_is_composed: true,
            align_to_composition_grid: false,
            native_scale: 1.,
            rounded_clip: None,
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

    #[test]
    fn moving_disjoint_earlier_annotation_preserves_later_cache_entries() {
        let base = base_pixmap(320, 200);
        let source = render_image(&base);
        let surface = Surface {
            image_id: Some(source.id),
            width: 320,
            height: 200,
            offset_x: 0.,
            offset_y: 0.,
            source_is_composed: true,
            align_to_composition_grid: false,
            native_scale: 1.,
            rounded_clip: None,
            pixels: source.as_bytes(0),
            matte: None,
            matte_image: None,
            composite_backdrop: false,
        };
        let grid = SamplingGrid {
            origin: (0., 0.),
            stage_bounds: (0., 0., 480., 300.),
            viewport: None,
            stage_size: (320., 200.),
            offset: (0., 0.),
            zoom: 1.5,
            dpi: 1.,
        };
        let mut first = Annotation::Rectangle {
            id: "first".into(),
            x: 10.,
            y: 10.,
            width: 40.,
            height: 40.,
            stroke: "#ff0000".into(),
            stroke_width: 2.,
            fill: Some("#ff0000".into()),
        };
        let second = Annotation::Rectangle {
            id: "second".into(),
            x: 200.,
            y: 110.,
            width: 60.,
            height: 50.,
            stroke: "#00ff00".into(),
            stroke_width: 2.,
            fill: None,
        };
        let keys = begin_frame(&[&first, &second]);
        let direct_before = image(&keys, 1, &second, &[&first], &surface, 1.).unwrap();
        let aligned_before = aligned_image(&keys, 1, &second, &[&first], &surface, grid).unwrap();

        first.translate(30., 20.);
        let keys = begin_frame(&[&first, &second]);
        let direct_after = image(&keys, 1, &second, &[&first], &surface, 1.).unwrap();
        let aligned_after = aligned_image(&keys, 1, &second, &[&first], &surface, grid).unwrap();
        assert!(Arc::ptr_eq(&direct_before.image, &direct_after.image));
        assert!(Arc::ptr_eq(&aligned_before.image, &aligned_after.image));

        first.translate(170., 90.);
        let keys = begin_frame(&[&first, &second]);
        let direct_overlapped = image(&keys, 1, &second, &[&first], &surface, 1.).unwrap();
        let aligned_overlapped =
            aligned_image(&keys, 1, &second, &[&first], &surface, grid).unwrap();
        assert!(!Arc::ptr_eq(&direct_before.image, &direct_overlapped.image));
        assert!(!Arc::ptr_eq(
            &aligned_before.image,
            &aligned_overlapped.image
        ));
    }

    #[test]
    fn sprite_cache_reuses_same_texture_across_translations() {
        let mut shapes = vec![
            pen(5.0),
            highlight(),
            number(),
            Annotation::Rectangle {
                id: "rect".into(),
                x: 20.,
                y: 30.,
                width: 60.,
                height: 40.,
                stroke: "#ff3b30".into(),
                stroke_width: 3.,
                fill: None,
            },
            Annotation::Circle {
                id: "circle".into(),
                x: 50.,
                y: 50.,
                radius: 24.,
                stroke: "#34c759".into(),
                stroke_width: 3.,
                fill: None,
            },
            Annotation::Line {
                id: "line".into(),
                points: [10., 15., 90., 45.],
                stroke: "#0a84ff".into(),
                stroke_width: 4.,
            },
            Annotation::Arrow {
                id: "arrow".into(),
                points: [10., 15., 90., 65.],
                stroke: "#ff9f0a".into(),
                stroke_width: 4.,
                arrow_style: Some("curved".into()),
                bend_offset: Some(crate::editor::annotations::Offset { x: 8., y: -6. }),
            },
        ];
        if crate::editor::text_render::is_available() {
            shapes.push(text(Some(15.0)));
        }

        for shape in &mut shapes {
            let before = sprite_image(shape, 1.5).expect("sprite before move");
            shape.translate(42.375, -17.625);
            let after = sprite_image(shape, 1.5).expect("sprite after move");
            assert!(
                Arc::ptr_eq(&before.image, &after.image),
                "expected sprite cache hit for {}",
                shape.kind()
            );
            assert!((after.x - before.x - 42.375).abs() < 1e-6);
            assert!((after.y - before.y + 17.625).abs() < 1e-6);
        }

        for freehand in [&mut pen(5.0), &mut highlight()] {
            let before = halo_sprite_image(freehand, 1.5, 2.0, [10, 132, 255, 204]).unwrap();
            freehand.translate(18.25, 27.75);
            let after = halo_sprite_image(freehand, 1.5, 2.0, [10, 132, 255, 204]).unwrap();
            assert!(Arc::ptr_eq(&before.image, &after.image));
            assert!((after.x - before.x - 18.25).abs() < 1e-6);
            assert!((after.y - before.y - 27.75).abs() < 1e-6);
        }
    }

    #[test]
    fn interactive_redact_caps_large_blur_and_tracks_position() {
        let base = base_pixmap(640, 480);
        let source = render_image(&base);
        let surface = Surface {
            image_id: Some(source.id),
            width: 640,
            height: 480,
            offset_x: 0.,
            offset_y: 0.,
            source_is_composed: true,
            align_to_composition_grid: false,
            native_scale: 1.,
            rounded_clip: None,
            pixels: source.as_bytes(0),
            matte: None,
            matte_image: None,
            composite_backdrop: false,
        };
        let mut blur = Annotation::Redact {
            id: "blur-drag".into(),
            x: 20.,
            y: 20.,
            width: 500.,
            height: 400.,
            style: "blur".into(),
            intensity: 5.,
        };
        let keys = begin_frame(&[&blur]);
        let first = interactive_redact_image(
            &keys,
            0,
            ScreenshotPaint::Annotation(&blur),
            &[],
            &surface,
            2.0,
        )
        .unwrap();
        let size = first.image.size(0);
        assert!(
            f64::from(u32::from(size.width)) * f64::from(u32::from(size.height))
                <= MAX_INTERACTIVE_BLUR_PIXELS * 1.25
        );
        blur.translate(40., 25.);
        let keys = begin_frame(&[&blur]);
        let moved = interactive_redact_image(
            &keys,
            0,
            ScreenshotPaint::Annotation(&blur),
            &[],
            &surface,
            2.0,
        )
        .unwrap();
        assert!(!Arc::ptr_eq(&first.image, &moved.image));
        assert!((moved.x - first.x - 40.).abs() <= 2.);
        assert!((moved.y - first.y - 25.).abs() <= 2.);
    }

    #[test]
    fn composite_layer_appends_incrementally_and_matches_export() {
        let base = base_pixmap(240, 180);
        let source = render_image(&base);
        let surface = Surface {
            image_id: Some(source.id),
            width: 240,
            height: 180,
            offset_x: 0.,
            offset_y: 0.,
            source_is_composed: true,
            align_to_composition_grid: false,
            native_scale: 1.,
            rounded_clip: None,
            pixels: source.as_bytes(0),
            matte: None,
            matte_image: None,
            composite_backdrop: false,
        };
        let clip = (0., 0., 240., 180.);
        let first = pen(4.0);
        let second = number();
        let third = Annotation::Redact {
            id: "pix".into(),
            x: 16.,
            y: 18.,
            width: 60.,
            height: 44.,
            style: "pixelate".into(),
            intensity: 3.,
        };
        let mut fourth = Annotation::Rectangle {
            id: "grown".into(),
            x: 150.,
            y: 110.,
            width: 60.,
            height: 50.,
            stroke: "#0a84ff".into(),
            stroke_width: 3.,
            fill: None,
        };

        let _ = super::begin_frame(&[]);
        let p1 = renderer::screenshot_paints([&first].into_iter());
        let layer1 = composite_layer(
            Some(source.id),
            LayerSlot::Committed,
            &p1,
            &surface,
            clip,
            false,
        )
        .unwrap();
        let layer1_hit = composite_layer(
            Some(source.id),
            LayerSlot::Committed,
            &p1,
            &surface,
            clip,
            false,
        )
        .unwrap();
        assert!(Arc::ptr_eq(&layer1.image, &layer1_hit.image));

        let p2 = renderer::screenshot_paints([&first, &second, &third].into_iter());
        let layer2 = composite_layer(
            Some(source.id),
            LayerSlot::Committed,
            &p2,
            &surface,
            clip,
            false,
        )
        .unwrap();
        assert!(!Arc::ptr_eq(&layer1.image, &layer2.image));

        let p3 = renderer::screenshot_paints([&first, &second, &third, &fourth].into_iter());
        let layer3 = composite_layer(
            Some(source.id),
            LayerSlot::Committed,
            &p3,
            &surface,
            clip,
            false,
        )
        .unwrap();
        assert!(layer3.width > layer2.width || layer3.height > layer2.height);

        let mut expected = Canvas::from_pixmap(base.clone());
        renderer::draw_all_screenshot(
            &mut expected,
            &[first.clone(), second.clone(), third.clone(), fourth.clone()],
            1.0,
        );
        let expected_rgba = crate::editor::export::to_rgba(expected.pixmap());
        let mut actual = Canvas::from_pixmap(base);
        let layer_bgra = layer3.image.as_bytes(0).unwrap();
        let size = layer3.image.size(0);
        let mut layer_rgba = image::RgbaImage::new(u32::from(size.width), u32::from(size.height));
        for (src, dst) in layer_bgra
            .as_chunks::<4>()
            .0
            .iter()
            .zip(layer_rgba.as_chunks_mut::<4>().0.iter_mut())
        {
            *dst = [src[2], src[1], src[0], src[3]];
        }
        let layer_pixmap = crate::editor::export::from_rgba(&layer_rgba).unwrap();
        actual.draw_pixmap(
            layer_pixmap.as_ref(),
            layer3.x as f32,
            layer3.y as f32,
            layer3.width as f32,
            layer3.height as f32,
        );
        let actual_rgba = crate::editor::export::to_rgba(actual.pixmap());
        for (index, (a, e)) in actual_rgba
            .as_raw()
            .iter()
            .zip(expected_rgba.as_raw())
            .enumerate()
        {
            assert!(
                a.abs_diff(*e) <= 2,
                "composite incremental append mismatch at channel {index}: {a} vs {e}"
            );
        }

        fourth.translate(-5., -5.);
        let _ = fourth;
    }

    #[test]
    fn composite_layer_skips_rebuild_when_wallpaper_changes_for_strokes() {
        let first_bg = render_image(&base_pixmap(240, 180));
        let mut second_pix = base_pixmap(240, 180);
        second_pix.fill(tiny_skia::Color::from_rgba8(10, 20, 30, 255));
        let second_bg = render_image(&second_pix);
        let stroke = pen(4.0);
        let paints = renderer::screenshot_paints([&stroke].into_iter());
        let surface_a = Surface {
            image_id: Some(first_bg.id),
            width: 240,
            height: 180,
            offset_x: 0.,
            offset_y: 0.,
            source_is_composed: true,
            align_to_composition_grid: false,
            native_scale: 1.,
            rounded_clip: None,
            pixels: first_bg.as_bytes(0),
            matte: None,
            matte_image: None,
            composite_backdrop: false,
        };
        let surface_b = Surface {
            image_id: Some(second_bg.id),
            pixels: second_bg.as_bytes(0),
            ..surface_a
        };
        let clip = (0., 0., 240., 180.);
        let _ = super::begin_frame(&[]);
        let a = composite_layer(
            Some(first_bg.id),
            LayerSlot::Committed,
            &paints,
            &surface_a,
            clip,
            false,
        )
        .unwrap();
        let b = composite_layer(
            Some(first_bg.id),
            LayerSlot::Committed,
            &paints,
            &surface_b,
            clip,
            false,
        )
        .unwrap();
        assert!(Arc::ptr_eq(&a.image, &b.image));
    }

    #[test]
    fn effect_over_layer_samples_flattened_below_strokes() {
        let base = base_pixmap(200, 160);
        let source = render_image(&base);
        let surface = Surface {
            image_id: Some(source.id),
            width: 200,
            height: 160,
            offset_x: 0.,
            offset_y: 0.,
            source_is_composed: true,
            align_to_composition_grid: false,
            native_scale: 1.,
            rounded_clip: None,
            pixels: source.as_bytes(0),
            matte: None,
            matte_image: None,
            composite_backdrop: false,
        };
        let stroke = Annotation::Rectangle {
            id: "bright".into(),
            x: 20.,
            y: 20.,
            width: 100.,
            height: 80.,
            stroke: "#ff0000".into(),
            stroke_width: 4.,
            fill: Some("#ff0000".into()),
        };
        let redact = Annotation::Redact {
            id: "pixelate-over".into(),
            x: 30.,
            y: 30.,
            width: 50.,
            height: 40.,
            style: "pixelate".into(),
            intensity: 4.,
        };
        let _ = super::begin_frame(&[]);
        let below = renderer::screenshot_paints([&stroke].into_iter());
        let _ = composite_layer(
            Some(source.id),
            LayerSlot::Below,
            &below,
            &surface,
            (0., 0., 200., 160.),
            false,
        )
        .unwrap();
        let over_below = effect_over_layer(
            Some(source.id),
            Some(LayerSlot::Below),
            ScreenshotPaint::Annotation(&redact),
            &surface,
            1.0,
            true,
        )
        .unwrap();
        let pixels = over_below.image.as_bytes(0).unwrap().as_chunks::<4>().0;
        let center = pixels[pixels.len() / 2];
        assert!(center[2] > 200 && center[1] < 20 && center[0] < 20);
    }
}
