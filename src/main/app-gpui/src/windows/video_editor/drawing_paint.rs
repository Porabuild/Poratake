use std::sync::Arc;

use gpui::{px, Hsla, PathBuilder, Pixels, RenderImage, Window};
use herogpui::gpui;

use crate::editor::annotations::{
    arrow_curve_control, arrow_head_points, arrow_head_size, normalize_rect, points_to_coordinates,
    redact_intensity, Annotation,
};
use crate::render::canvas::Canvas;
use crate::theme::color::Srgba;
use crate::windows::video_editor::drawing_overlay::Rect;

#[derive(Clone, PartialEq, Eq)]
pub struct RedactPreviewKey {
    image_id: gpui::ImageId,
    x_bits: u64,
    y_bits: u64,
    width_bits: u64,
    height_bits: u64,
    style: String,
    intensity_bits: u64,
}

#[derive(Clone)]
pub struct RedactPreview {
    pub key: RedactPreviewKey,
    pub image: Arc<RenderImage>,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

pub fn redact_preview_key(
    preview_image: &RenderImage,
    composition: (f64, f64),
    annotation: &Annotation,
) -> Option<RedactPreviewKey> {
    let Annotation::Redact {
        x,
        y,
        width,
        height,
        style,
        intensity,
        ..
    } = annotation
    else {
        return None;
    };
    if style != "pixelate" && style != "blur" {
        return None;
    }
    let (rx, ry, rw, rh) = normalize_rect(*x, *y, *width, *height);
    if ![rx, ry, rw, rh, *intensity, composition.0, composition.1]
        .into_iter()
        .all(f64::is_finite)
        || rw <= 0.0
        || rh <= 0.0
        || composition.0 <= 0.0
        || composition.1 <= 0.0
    {
        return None;
    }
    Some(RedactPreviewKey {
        image_id: preview_image.id,
        x_bits: rx.to_bits(),
        y_bits: ry.to_bits(),
        width_bits: rw.to_bits(),
        height_bits: rh.to_bits(),
        style: style.clone(),
        intensity_bits: intensity.to_bits(),
    })
}

pub fn build_redact_preview(
    preview_image: &RenderImage,
    composition: (f64, f64),
    annotation: &Annotation,
) -> Option<RedactPreview> {
    let key = redact_preview_key(preview_image, composition, annotation)?;
    let Annotation::Redact {
        x,
        y,
        width,
        height,
        style,
        intensity,
        ..
    } = annotation
    else {
        return None;
    };
    let (rx, ry, rw, rh) = normalize_rect(*x, *y, *width, *height);
    let size = preview_image.size(0);
    let (img_w, img_h) = (u32::from(size.width), u32::from(size.height));
    if img_w == 0 || img_h == 0 {
        return None;
    }
    let scale_x = f64::from(img_w) / composition.0;
    let scale_y = f64::from(img_h) / composition.1;
    let tx0 = (rx * scale_x).floor().clamp(0.0, f64::from(img_w)) as u32;
    let ty0 = (ry * scale_y).floor().clamp(0.0, f64::from(img_h)) as u32;
    let tx1 = ((rx + rw) * scale_x).ceil().clamp(0.0, f64::from(img_w)) as u32;
    let ty1 = ((ry + rh) * scale_y).ceil().clamp(0.0, f64::from(img_h)) as u32;
    if tx1 <= tx0 || ty1 <= ty0 {
        return None;
    }

    let (sx0, sy0, sx1, sy1) = if style == "blur" {
        let (_, blur_radius) = redact_intensity(*intensity);
        let pad_x = (blur_radius.max(1.0) * 2.0 * scale_x).ceil() as u32;
        let pad_y = (blur_radius.max(1.0) * 2.0 * scale_y).ceil() as u32;
        (
            tx0.saturating_sub(pad_x),
            ty0.saturating_sub(pad_y),
            tx1.saturating_add(pad_x).min(img_w),
            ty1.saturating_add(pad_y).min(img_h),
        )
    } else {
        (tx0, ty0, tx1, ty1)
    };
    let source_bytes = preview_image.as_bytes(0)?;
    let expected_len = (img_w as usize)
        .checked_mul(img_h as usize)?
        .checked_mul(4)?;
    if source_bytes.len() < expected_len {
        return None;
    }

    let src_w = sx1 - sx0;
    let src_h = sy1 - sy0;
    let mut canvas = Canvas::new(src_w, src_h)?;
    let src_stride = img_w as usize * 4;
    let dst_stride = src_w as usize * 4;
    let dst = canvas.pixmap_mut().data_mut();
    for row in 0..src_h as usize {
        let src_off = (sy0 as usize + row) * src_stride + sx0 as usize * 4;
        let dst_off = row * dst_stride;
        crate::video::composition::opaque::copy_bgra_to_rgba(
            &source_bytes[src_off..src_off + dst_stride],
            &mut dst[dst_off..dst_off + dst_stride],
        );
    }

    canvas.translate(-(sx0 as f32), -(sy0 as f32));
    canvas.scale(scale_x as f32, scale_y as f32);
    crate::render::annotations::draw(&mut canvas, annotation);

    let tw = tx1 - tx0;
    let th = ty1 - ty0;
    let crop_x = (tx0 - sx0) as usize;
    let crop_y = (ty0 - sy0) as usize;
    let pixmap = canvas.pixmap();
    let pixmap_stride = src_w as usize * 4;
    let mut bgra = vec![0u8; tw as usize * th as usize * 4];
    for row in 0..th as usize {
        let from = (crop_y + row) * pixmap_stride + crop_x * 4;
        let to = row * tw as usize * 4;
        crate::video::composition::opaque::copy_rgba_to_opaque_bgra(
            &pixmap.data()[from..from + tw as usize * 4],
            &mut bgra[to..to + tw as usize * 4],
        );
    }
    let buffer = image::RgbaImage::from_raw(tw, th, bgra)?;
    let image = Arc::new(RenderImage::new(smallvec::smallvec![image::Frame::new(
        buffer
    )]));
    Some(RedactPreview {
        key,
        image,
        x: f64::from(tx0) / scale_x,
        y: f64::from(ty0) / scale_y,
        width: f64::from(tw) / scale_x,
        height: f64::from(th) / scale_y,
    })
}

pub fn draw(
    window: &mut Window,
    annotation: &Annotation,
    content: Rect,
    scale: f32,
    redact_preview: Option<&RedactPreview>,
    outline_color: Hsla,
) {
    if scale <= 0.0 {
        return;
    }
    match annotation {
        Annotation::Pen {
            points,
            stroke,
            stroke_width,
            ..
        } => polyline(window, points, stroke, *stroke_width, 1.0, content, scale),
        Annotation::Highlight {
            points,
            fill,
            opacity,
            stroke_width,
            ..
        } => polyline(
            window,
            points,
            fill,
            *stroke_width,
            *opacity as f32,
            content,
            scale,
        ),
        Annotation::Line {
            points,
            stroke,
            stroke_width,
            ..
        } => {
            let mut builder = stroked(*stroke_width as f32 * scale);
            builder.move_to(at(content, scale, points[0], points[1]));
            builder.line_to(at(content, scale, points[2], points[3]));
            paint(window, builder, stroke, 1.0);
        }
        Annotation::Arrow {
            points,
            stroke,
            stroke_width,
            arrow_style,
            bend_offset,
            ..
        } => arrow(
            window,
            points,
            stroke,
            *stroke_width,
            arrow_style.as_deref(),
            *bend_offset,
            content,
            scale,
        ),
        Annotation::Rectangle {
            x,
            y,
            width,
            height,
            stroke,
            stroke_width,
            fill,
            ..
        } => {
            let rect = normalize_rect(*x, *y, *width, *height);
            if let Some(color) = fill {
                let mut builder = PathBuilder::fill();
                rect_path(&mut builder, rect, content, scale);
                paint(window, builder, color, 1.0);
            }
            let mut builder = stroked(*stroke_width as f32 * scale);
            rect_path(&mut builder, rect, content, scale);
            paint(window, builder, stroke, 1.0);
        }
        Annotation::Circle {
            x,
            y,
            radius,
            stroke,
            stroke_width,
            fill,
            ..
        } => {
            if let Some(color) = fill {
                let mut builder = PathBuilder::fill();
                if ellipse_path(&mut builder, *x, *y, *radius, content, scale) {
                    paint(window, builder, color, 1.0);
                }
            }
            let mut builder = stroked(*stroke_width as f32 * scale);
            if ellipse_path(&mut builder, *x, *y, *radius, content, scale) {
                paint(window, builder, stroke, 1.0);
            }
        }
        Annotation::Redact {
            x,
            y,
            width,
            height,
            style,
            ..
        } => {
            let (left, top, width, height) = normalize_rect(*x, *y, *width, *height);
            let max_x = f64::from(content.width / scale);
            let max_y = f64::from(content.height / scale);
            let clamped_left = left.clamp(0.0, max_x);
            let clamped_top = top.clamp(0.0, max_y);
            let clamped_right = (left + width).clamp(0.0, max_x);
            let clamped_bottom = (top + height).clamp(0.0, max_y);
            let clamped_width = clamped_right - clamped_left;
            let clamped_height = clamped_bottom - clamped_top;
            if clamped_width <= 0.0 || clamped_height <= 0.0 {
                return;
            }
            if style == "blackout" {
                let mut builder = PathBuilder::fill();
                rect_path(
                    &mut builder,
                    (clamped_left, clamped_top, clamped_width, clamped_height),
                    content,
                    scale,
                );
                paint(window, builder, "#000000", 1.0);
            } else if let Some(preview) = redact_preview {
                let region = gpui::Bounds {
                    origin: at(content, scale, preview.x, preview.y),
                    size: gpui::size(
                        px(preview.width as f32 * scale),
                        px(preview.height as f32 * scale),
                    ),
                };
                let _ = window.paint_image(
                    region,
                    region,
                    gpui::Corners::default(),
                    preview.image.clone(),
                    0,
                    false,
                );
            }
            let origin = at(content, scale, clamped_left, clamped_top);
            let outline_bounds = gpui::Bounds {
                origin: gpui::point(origin.x - px(1.0), origin.y - px(1.0)),
                size: gpui::size(
                    px(clamped_width as f32 * scale + 2.0),
                    px(clamped_height as f32 * scale + 2.0),
                ),
            };
            window.paint_quad(
                gpui::outline(outline_bounds, outline_color, gpui::BorderStyle::Dashed)
                    .border_widths(px(1.0))
                    .corner_radii(px(2.0 * scale)),
            );
        }
        Annotation::Text { .. } | Annotation::Number { .. } => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn polyline(
    window: &mut Window,
    points: &[f64],
    color: &str,
    stroke_width: f64,
    alpha: f32,
    content: Rect,
    scale: f32,
) {
    let coordinates = points_to_coordinates(points);
    if coordinates.len() < 2 {
        return;
    }
    let mut builder = stroked(stroke_width as f32 * scale);
    for (index, (x, y)) in coordinates.iter().enumerate() {
        let point = at(content, scale, *x, *y);
        match index {
            0 => builder.move_to(point),
            _ => builder.line_to(point),
        }
    }
    paint(window, builder, color, alpha);
}

#[allow(clippy::too_many_arguments)]
fn arrow(
    window: &mut Window,
    points: &[f64; 4],
    color: &str,
    stroke_width: f64,
    arrow_style: Option<&str>,
    bend: Option<crate::editor::annotations::Offset>,
    content: Rect,
    scale: f32,
) {
    let control = arrow_curve_control(points, arrow_style, bend);
    let mut builder = stroked(stroke_width as f32 * scale);
    builder.move_to(at(content, scale, points[0], points[1]));
    match control {
        Some((cx, cy)) => builder.curve_to(
            at(content, scale, points[2], points[3]),
            at(content, scale, cx, cy),
        ),
        None => builder.line_to(at(content, scale, points[2], points[3])),
    }

    let head = arrow_head_size(stroke_width);
    let end_angle = match control {
        Some((cx, cy)) => (points[3] - cy).atan2(points[2] - cx),
        None => (points[3] - points[1]).atan2(points[2] - points[0]),
    };
    head_path(
        &mut builder,
        points[2],
        points[3],
        end_angle,
        head,
        content,
        scale,
    );
    if matches!(arrow_style, Some("double" | "double-curved")) {
        let start_angle = match control {
            Some((cx, cy)) => (points[1] - cy).atan2(points[0] - cx),
            None => (points[1] - points[3]).atan2(points[0] - points[2]),
        };
        head_path(
            &mut builder,
            points[0],
            points[1],
            start_angle,
            head,
            content,
            scale,
        );
    }
    paint(window, builder, color, 1.0);
}

#[allow(clippy::too_many_arguments)]
fn head_path(
    builder: &mut PathBuilder,
    tip_x: f64,
    tip_y: f64,
    angle: f64,
    head: f64,
    content: Rect,
    scale: f32,
) {
    let (left, right) = arrow_head_points(tip_x, tip_y, angle, head);
    let tip = at(content, scale, tip_x, tip_y);
    for (x, y) in [left, right] {
        builder.move_to(tip);
        builder.line_to(at(content, scale, x, y));
    }
}

fn rect_path(builder: &mut PathBuilder, rect: (f64, f64, f64, f64), content: Rect, scale: f32) {
    let (left, top, width, height) = rect;
    let corners = [
        (left, top),
        (left + width, top),
        (left + width, top + height),
        (left, top + height),
        (left, top),
    ];
    for (index, (x, y)) in corners.iter().enumerate() {
        let point = at(content, scale, *x, *y);
        match index {
            0 => builder.move_to(point),
            _ => builder.line_to(point),
        }
    }
}

fn ellipse_path(
    builder: &mut PathBuilder,
    x: f64,
    y: f64,
    radius: f64,
    content: Rect,
    scale: f32,
) -> bool {
    if radius <= 0.0 {
        return false;
    }
    let center = at(content, scale, x, y);
    let radii = gpui::point(px(radius as f32 * scale), px(radius as f32 * scale));
    builder.move_to(gpui::point(center.x - radii.x, center.y));
    builder.arc_to(
        radii,
        px(0.0),
        true,
        true,
        gpui::point(center.x + radii.x, center.y),
    );
    builder.arc_to(
        radii,
        px(0.0),
        true,
        true,
        gpui::point(center.x - radii.x, center.y),
    );
    true
}

fn at(content: Rect, scale: f32, x: f64, y: f64) -> gpui::Point<Pixels> {
    gpui::point(
        px(content.x + x as f32 * scale),
        px(content.y + y as f32 * scale),
    )
}

fn stroked(width: f32) -> PathBuilder {
    let mut builder = PathBuilder::stroke(px(width.max(1.0)));
    if let gpui::PathStyle::Stroke(options) = &mut builder.style {
        let line_width = options.line_width;
        *options = gpui::StrokeOptions::default()
            .with_line_width(line_width)
            .with_line_cap(lyon::path::LineCap::Round)
            .with_line_join(lyon::path::LineJoin::Round);
    }
    builder
}

fn paint(window: &mut Window, builder: PathBuilder, color: &str, alpha: f32) {
    if let Ok(path) = builder.build() {
        window.paint_path(path, Srgba::parse(color).to_hsla().opacity(alpha));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn patterned_frame(width: u32, height: u32) -> RenderImage {
        let mut buffer = image::RgbaImage::new(width, height);
        for y in 0..height {
            for x in 0..width {
                let bright = (x / 2 + y / 2) % 2 == 0;
                let bgra = if bright {
                    [20, 220, 240, 255]
                } else {
                    [220, 40, 20, 255]
                };
                buffer.put_pixel(x, y, image::Rgba(bgra));
            }
        }
        RenderImage::new(smallvec::smallvec![image::Frame::new(buffer)])
    }

    #[test]
    fn pixelate_preview_produces_redacted_pixels_even_for_backwards_drag() {
        let frame = patterned_frame(80, 60);
        let draft = Annotation::Redact {
            id: "r1".into(),
            x: 60.0,
            y: 44.0,
            width: -40.0,
            height: -24.0,
            style: "pixelate".into(),
            intensity: 6.0,
        };
        let preview = build_redact_preview(&frame, (80.0, 60.0), &draft).expect("preview");
        assert_eq!(preview.x, 20.0);
        assert_eq!(preview.y, 20.0);
        assert_eq!(preview.width, 40.0);
        assert_eq!(preview.height, 24.0);
        let bytes = preview.image.as_bytes(0).expect("bytes");
        let pixels = bytes.as_chunks::<4>().0;
        assert!(pixels.iter().any(|p| p[0] > 40 || p[1] > 40 || p[2] > 40));
        assert_eq!(pixels[0], pixels[1]);
    }

    #[test]
    fn blur_preview_blends_patterned_frame_at_scaled_resolution() {
        let frame = patterned_frame(80, 60);
        let draft = Annotation::Redact {
            id: "r2".into(),
            x: 40.0,
            y: 30.0,
            width: 60.0,
            height: 40.0,
            style: "blur".into(),
            intensity: 5.0,
        };
        let preview = build_redact_preview(&frame, (160.0, 120.0), &draft).expect("preview");
        assert_eq!(preview.x, 40.0);
        assert_eq!(preview.y, 30.0);
        assert_eq!(preview.width, 60.0);
        assert_eq!(preview.height, 40.0);
        let bytes = preview.image.as_bytes(0).expect("bytes");
        let center = bytes.as_chunks::<4>().0[bytes.len() / 8];
        assert!(center[0] > 60 && center[0] < 190);
        assert!(center[1] > 60 && center[1] < 190);
        assert!(center[2] > 60 && center[2] < 190);
        assert_eq!(center[3], 255);
    }

    #[test]
    fn non_raster_and_empty_drafts_return_no_preview() {
        let frame = patterned_frame(40, 40);
        let blackout = Annotation::Redact {
            id: "b1".into(),
            x: 10.0,
            y: 10.0,
            width: 10.0,
            height: 10.0,
            style: "blackout".into(),
            intensity: 5.0,
        };
        let empty = Annotation::Redact {
            id: "e1".into(),
            x: 10.0,
            y: 10.0,
            width: 0.0,
            height: 10.0,
            style: "pixelate".into(),
            intensity: 5.0,
        };
        assert!(build_redact_preview(&frame, (40.0, 40.0), &blackout).is_none());
        assert!(build_redact_preview(&frame, (40.0, 40.0), &empty).is_none());
    }
}
