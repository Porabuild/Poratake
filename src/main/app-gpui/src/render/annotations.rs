//! Port of `components/video-editor/composition/drawing-canvas-renderer.ts`.
//! Both the image editor's export and the video editor's drawing overlay draw
//! through this, which is what keeps a saved file identical to the preview.

use tiny_skia::{BlendMode, Color, FillRule, LineCap, LineJoin, PathBuilder, Pixmap, Rect, Stroke};

use crate::editor::annotations::{
    arrow_head_points, arrow_head_size, curved_control_point, has_arrow_bend, normalize_rect,
    number_size_config, points_to_coordinates, redact_intensity, Annotation, Offset,
    DEFAULT_TEXT_FONT, TEXT_BG_RADIUS,
};
use crate::render::canvas::{circle_path, rounded_rect_path, Canvas};
use crate::render::freehand::{self, Vec2};
use crate::render::{blur, color, text};

pub fn draw_all(canvas: &mut Canvas, annotations: &[Annotation]) {
    for annotation in annotations {
        draw(canvas, annotation);
    }
}

pub fn draw(canvas: &mut Canvas, annotation: &Annotation) {
    match annotation {
        Annotation::Pen {
            points,
            stroke,
            stroke_width,
            ..
        } => draw_pen(canvas, points, stroke, *stroke_width),
        Annotation::Highlight {
            points,
            fill,
            opacity,
            stroke_width,
            ..
        } => draw_highlight(canvas, points, fill, *opacity, *stroke_width),
        Annotation::Rectangle {
            x,
            y,
            width,
            height,
            stroke,
            stroke_width,
            fill,
            ..
        } => draw_rectangle(
            canvas,
            *x,
            *y,
            *width,
            *height,
            stroke,
            *stroke_width,
            fill.as_deref(),
        ),
        Annotation::Circle {
            x,
            y,
            radius,
            stroke,
            stroke_width,
            fill,
            ..
        } => draw_circle(
            canvas,
            *x,
            *y,
            *radius,
            stroke,
            *stroke_width,
            fill.as_deref(),
        ),
        Annotation::Line {
            points,
            stroke,
            stroke_width,
            ..
        } => draw_line(canvas, points, stroke, *stroke_width),
        Annotation::Arrow {
            points,
            stroke,
            stroke_width,
            arrow_style,
            bend_offset,
            ..
        } => draw_arrow(
            canvas,
            points,
            stroke,
            *stroke_width,
            arrow_style.as_deref(),
            *bend_offset,
        ),
        Annotation::Text { .. } => draw_text(canvas, annotation),
        Annotation::Number {
            x,
            y,
            display_value,
            fill,
            size,
            ..
        } => draw_number(canvas, *x, *y, display_value, fill, size),
        Annotation::Redact {
            x,
            y,
            width,
            height,
            style,
            intensity,
            ..
        } => draw_redact(canvas, *x, *y, *width, *height, style, *intensity),
    }
}

/// `drawFreehandPath` — the outline is closed with quadratic segments through
/// each pair's midpoint, exactly as `getSvgPathFromStroke` builds it.
fn freehand_path(points: &[Vec2]) -> Option<tiny_skia::Path> {
    if points.is_empty() {
        return None;
    }
    let mut builder = PathBuilder::new();
    builder.move_to(points[0].0 as f32, points[0].1 as f32);
    for index in 0..points.len() {
        let (x0, y0) = points[index];
        let (x1, y1) = points[(index + 1) % points.len()];
        builder.quad_to(
            x0 as f32,
            y0 as f32,
            ((x0 + x1) / 2.0) as f32,
            ((y0 + y1) / 2.0) as f32,
        );
    }
    builder.close();
    builder.finish()
}

fn draw_pen(canvas: &mut Canvas, points: &[f64], stroke: &str, stroke_width: f64) {
    let coordinates = points_to_coordinates(points);
    if coordinates.is_empty() {
        return;
    }
    let outline = freehand::stroke(&coordinates, &freehand::Options::for_pen(stroke_width));
    let Some(path) = freehand_path(&outline) else {
        return;
    };
    canvas.save();
    canvas.fill_path(
        &path,
        color::parse_or(stroke, Color::BLACK),
        FillRule::Winding,
    );
    canvas.restore();
}

fn draw_highlight(
    canvas: &mut Canvas,
    points: &[f64],
    fill: &str,
    opacity: f64,
    stroke_width: f64,
) {
    let coordinates = points_to_coordinates(points);
    if coordinates.len() < 2 {
        return;
    }
    let half_width = stroke_width / 2.0;
    let mut upper: Vec<Vec2> = Vec::with_capacity(coordinates.len());
    let mut lower: Vec<Vec2> = Vec::with_capacity(coordinates.len());

    for index in 0..coordinates.len() {
        let (x, y) = coordinates[index];
        let (mut dx, mut dy) = (0.0, 1.0);
        if index < coordinates.len() - 1 {
            let (next_x, next_y) = coordinates[index + 1];
            let length = ((next_x - x).powi(2) + (next_y - y).powi(2)).sqrt();
            if length > 0.0 {
                dx = -(next_y - y) / length;
                dy = (next_x - x) / length;
            }
        } else if index > 0 {
            let (previous_x, previous_y) = coordinates[index - 1];
            let length = ((x - previous_x).powi(2) + (y - previous_y).powi(2)).sqrt();
            if length > 0.0 {
                dx = -(y - previous_y) / length;
                dy = (x - previous_x) / length;
            }
        }
        upper.push((x + dx * half_width, y + dy * half_width));
        lower.push((x - dx * half_width, y - dy * half_width));
    }

    let mut builder = PathBuilder::new();
    builder.move_to(upper[0].0 as f32, upper[0].1 as f32);
    for point in upper.iter().skip(1) {
        builder.line_to(point.0 as f32, point.1 as f32);
    }
    for point in lower.iter().rev() {
        builder.line_to(point.0 as f32, point.1 as f32);
    }
    builder.close();
    let Some(path) = builder.finish() else {
        return;
    };

    canvas.save();
    let previous_alpha = canvas.global_alpha();
    canvas.set_global_alpha(previous_alpha * opacity as f32);
    canvas.fill_path_blended(
        &path,
        color::parse_or(fill, Color::BLACK),
        FillRule::Winding,
        BlendMode::Multiply,
    );
    canvas.restore();
}

fn shape_stroke(stroke_width: f64) -> Stroke {
    Stroke {
        width: stroke_width as f32,
        line_cap: LineCap::Butt,
        line_join: LineJoin::Miter,
        ..Stroke::default()
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_rectangle(
    canvas: &mut Canvas,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    stroke: &str,
    stroke_width: f64,
    fill: Option<&str>,
) {
    let (left, top, width, height) = normalize_rect(x, y, width, height);
    // The renderer draws rectangles with a 1px corner radius.
    let Some(path) = rounded_rect_path(left as f32, top as f32, width as f32, height as f32, 1.0)
    else {
        return;
    };
    canvas.save();
    if let Some(fill) = fill {
        canvas.fill_path(
            &path,
            color::parse_or(fill, Color::TRANSPARENT),
            FillRule::Winding,
        );
    }
    canvas.stroke_path(
        &path,
        color::parse_or(stroke, Color::BLACK),
        &shape_stroke(stroke_width),
    );
    canvas.restore();
}

fn draw_circle(
    canvas: &mut Canvas,
    x: f64,
    y: f64,
    radius: f64,
    stroke: &str,
    stroke_width: f64,
    fill: Option<&str>,
) {
    let Some(path) = circle_path(x as f32, y as f32, radius as f32) else {
        return;
    };
    canvas.save();
    if let Some(fill) = fill {
        canvas.fill_path(
            &path,
            color::parse_or(fill, Color::TRANSPARENT),
            FillRule::Winding,
        );
    }
    canvas.stroke_path(
        &path,
        color::parse_or(stroke, Color::BLACK),
        &shape_stroke(stroke_width),
    );
    canvas.restore();
}

fn draw_line(canvas: &mut Canvas, points: &[f64; 4], stroke: &str, stroke_width: f64) {
    let mut builder = PathBuilder::new();
    builder.move_to(points[0] as f32, points[1] as f32);
    builder.line_to(points[2] as f32, points[3] as f32);
    let Some(path) = builder.finish() else {
        return;
    };
    canvas.save();
    canvas.stroke_path(
        &path,
        color::parse_or(stroke, Color::BLACK),
        &Stroke {
            width: stroke_width as f32,
            line_cap: LineCap::Round,
            ..Stroke::default()
        },
    );
    canvas.restore();
}

/// `getArrowControlPoint` — an explicit bend wins over the style's own curve.
fn arrow_control_point(
    points: &[f64; 4],
    arrow_style: Option<&str>,
    bend: Option<Offset>,
) -> (f64, f64, bool) {
    let [x1, y1, x2, y2] = *points;
    let middle = ((x1 + x2) / 2.0, (y1 + y2) / 2.0);

    if has_arrow_bend(bend) {
        let bend = bend.unwrap_or(Offset { x: 0.0, y: 0.0 });
        return (middle.0 + bend.x, middle.1 + bend.y, true);
    }
    let style = arrow_style.unwrap_or("standard");
    if style != "curved" && style != "double-curved" {
        return (middle.0, middle.1, false);
    }
    let control = curved_control_point(x1, y1, x2, y2);
    (control.0, control.1, true)
}

fn draw_arrow(
    canvas: &mut Canvas,
    points: &[f64; 4],
    stroke: &str,
    stroke_width: f64,
    arrow_style: Option<&str>,
    bend: Option<Offset>,
) {
    let [x1, y1, x2, y2] = *points;
    let (control_x, control_y, has_curve) = arrow_control_point(points, arrow_style, bend);
    let style = arrow_style.unwrap_or("standard");
    let head_size = arrow_head_size(stroke_width);
    let paint = color::parse_or(stroke, Color::BLACK);
    let pen = Stroke {
        width: stroke_width as f32,
        line_cap: LineCap::Round,
        line_join: LineJoin::Round,
        ..Stroke::default()
    };

    canvas.save();

    let mut builder = PathBuilder::new();
    builder.move_to(x1 as f32, y1 as f32);
    if has_curve {
        builder.quad_to(control_x as f32, control_y as f32, x2 as f32, y2 as f32);
    } else {
        builder.line_to(x2 as f32, y2 as f32);
    }
    if let Some(path) = builder.finish() {
        canvas.stroke_path(&path, paint, &pen);
    }

    let end_angle = if has_curve {
        (y2 - control_y).atan2(x2 - control_x)
    } else {
        (y2 - y1).atan2(x2 - x1)
    };
    stroke_arrow_head(canvas, x2, y2, end_angle, head_size, paint, &pen);

    if style == "double" || style == "double-curved" {
        let start_angle = if has_curve {
            (y1 - control_y).atan2(x1 - control_x)
        } else {
            (y1 - y2).atan2(x1 - x2)
        };
        stroke_arrow_head(canvas, x1, y1, start_angle, head_size, paint, &pen);
    }

    canvas.restore();
}

fn stroke_arrow_head(
    canvas: &mut Canvas,
    x: f64,
    y: f64,
    angle: f64,
    head_size: f64,
    paint: Color,
    pen: &Stroke,
) {
    let (left, right) = arrow_head_points(x, y, angle, head_size);
    let mut builder = PathBuilder::new();
    builder.move_to(x as f32, y as f32);
    builder.line_to(left.0 as f32, left.1 as f32);
    builder.move_to(x as f32, y as f32);
    builder.line_to(right.0 as f32, right.1 as f32);
    if let Some(path) = builder.finish() {
        canvas.stroke_path(&path, paint, pen);
    }
}

fn draw_text(canvas: &mut Canvas, annotation: &Annotation) {
    let Annotation::Text {
        x,
        y,
        text: content,
        font_size,
        fill,
        font_family,
        background_color,
        background_padding,
        background_radius,
        rotation,
        ..
    } = annotation
    else {
        return;
    };
    if content.is_empty() {
        return;
    }

    let family = font_family.as_deref().unwrap_or(DEFAULT_TEXT_FONT);
    let padding = background_padding.unwrap_or(Offset { x: 0.0, y: 0.0 });
    let radius = background_radius.unwrap_or(TEXT_BG_RADIUS);

    let metrics = text::measure(content, family, *font_size as f32);
    let text_height = if metrics.height() > 0.0 {
        metrics.height() as f64
    } else {
        *font_size
    };
    let box_width = metrics.width as f64 + padding.x * 2.0;
    let box_height = text_height + padding.y * 2.0;
    let center_x = x - padding.x + box_width / 2.0;
    let center_y = y - padding.y + box_height / 2.0;

    canvas.save();
    let radians = (rotation.unwrap_or(0.0) * std::f64::consts::PI / 180.0) as f32;
    canvas.translate(center_x as f32, center_y as f32);
    canvas.rotate(radians);
    canvas.translate(-center_x as f32, -center_y as f32);

    if let Some(background) = background_color {
        if let Some(path) = rounded_rect_path(
            (x - padding.x) as f32,
            (y - padding.y) as f32,
            box_width as f32,
            box_height as f32,
            radius as f32,
        ) {
            canvas.fill_path(
                &path,
                color::parse_or(background, Color::TRANSPARENT),
                FillRule::Winding,
            );
        }
    }

    text::fill_text(
        canvas,
        content,
        family,
        *font_size as f32,
        *x as f32,
        (y + metrics.ascent as f64) as f32,
        color::parse_or(fill, Color::BLACK),
        text::Align::Left,
        text::Baseline::Alphabetic,
    );
    canvas.restore();
}

fn draw_number(canvas: &mut Canvas, x: f64, y: f64, display_value: &str, fill: &str, size: &str) {
    let (radius, font_size) = number_size_config(size);
    canvas.save();
    if let Some(path) = circle_path(x as f32, y as f32, radius as f32) {
        canvas.fill_path(
            &path,
            color::parse_or(fill, Color::BLACK),
            FillRule::Winding,
        );
    }
    text::fill_text(
        canvas,
        display_value,
        DEFAULT_TEXT_FONT,
        font_size as f32,
        x as f32,
        y as f32,
        color::contrast_color(fill),
        text::Align::Center,
        text::Baseline::Middle,
    );
    canvas.restore();
}

/// The device-space and user-space views of a region, clamped to the surface —
/// port of `clampRegion`.
struct Region {
    device_x: i32,
    device_y: i32,
    device_width: u32,
    device_height: u32,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    scale_x: f32,
    scale_y: f32,
}

fn clamp_region(canvas: &Canvas, x: f64, y: f64, width: f64, height: f64) -> Option<Region> {
    let transform = canvas.transform();
    let scale_x = if transform.sx == 0.0 {
        1.0
    } else {
        transform.sx
    };
    let scale_y = if transform.sy == 0.0 {
        1.0
    } else {
        transform.sy
    };
    let origin_x = x as f32 * scale_x + transform.tx;
    let origin_y = y as f32 * scale_y + transform.ty;

    let floored_x = origin_x.floor();
    let floored_y = origin_y.floor();
    let clamped_x = floored_x.max(0.0);
    let clamped_y = floored_y.max(0.0);
    let clamped_width = ((width as f32 * scale_x).ceil() - (clamped_x - floored_x))
        .min(canvas.width() as f32 - clamped_x);
    let clamped_height = ((height as f32 * scale_y).ceil() - (clamped_y - floored_y))
        .min(canvas.height() as f32 - clamped_y);
    if clamped_width <= 0.0 || clamped_height <= 0.0 {
        return None;
    }

    Some(Region {
        device_x: clamped_x as i32,
        device_y: clamped_y as i32,
        device_width: clamped_width as u32,
        device_height: clamped_height as u32,
        x: (clamped_x - transform.tx) / scale_x,
        y: (clamped_y - transform.ty) / scale_y,
        width: clamped_width / scale_x,
        height: clamped_height / scale_y,
        scale_x,
        scale_y,
    })
}

fn draw_redact(
    canvas: &mut Canvas,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    style: &str,
    intensity: f64,
) {
    let (left, top, width, height) = normalize_rect(x, y, width, height);
    let (pixel_size, blur_radius) = redact_intensity(intensity);

    match style {
        "pixelate" => pixelate_region(canvas, left, top, width, height, pixel_size.max(1.0)),
        "blur" => blur_region(canvas, left, top, width, height, blur_radius.max(1.0)),
        _ => {
            let Some(rect) = Rect::from_xywh(left as f32, top as f32, width as f32, height as f32)
            else {
                return;
            };
            canvas.save();
            canvas.set_shadow(None);
            canvas.fill_rect(rect, Color::from_rgba8(0, 0, 0, 255));
            canvas.restore();
        }
    }
}

fn pixelate_region(canvas: &mut Canvas, x: f64, y: f64, width: f64, height: f64, block: f64) {
    let Some(region) = clamp_region(canvas, x, y, width, height) else {
        return;
    };
    let Some(source) = canvas.read_region(
        region.device_x,
        region.device_y,
        region.device_width,
        region.device_height,
    ) else {
        return;
    };

    let block_width = (block as f32 * region.scale_x).max(1.0);
    let block_height = (block as f32 * region.scale_y).max(1.0);
    let small_width = ((region.device_width as f32 / block_width).round() as u32).max(1);
    let small_height = ((region.device_height as f32 / block_height).round() as u32).max(1);
    let Some(small) = downsample(&source, small_width, small_height) else {
        return;
    };

    canvas.save();
    canvas.set_shadow(None);
    canvas.set_global_alpha(1.0);
    canvas.draw_pixmap_nearest(
        small.as_ref(),
        region.x,
        region.y,
        region.width,
        region.height,
    );
    canvas.restore();
}

fn blur_region(canvas: &mut Canvas, x: f64, y: f64, width: f64, height: f64, radius: f64) {
    let Some(region) = clamp_region(canvas, x, y, width, height) else {
        return;
    };
    // Sample beyond the region so the blur has real pixels to pull in at the
    // edges instead of smearing transparency inwards.
    let padding = (radius as f32 * 2.0 * region.scale_x).ceil() as i32;
    let source_x = (region.device_x - padding).max(0);
    let source_y = (region.device_y - padding).max(0);
    let source_width =
        ((region.device_width as i32 + padding * 2).min(canvas.width() as i32 - source_x)).max(1);
    let source_height =
        ((region.device_height as i32 + padding * 2).min(canvas.height() as i32 - source_y)).max(1);

    let Some(mut source) = canvas.read_region(
        source_x,
        source_y,
        source_width as u32,
        source_height as u32,
    ) else {
        return;
    };
    blur::blur(&mut source, radius as f32 * region.scale_x);

    canvas.save();
    canvas.set_shadow(None);
    canvas.set_global_alpha(1.0);
    if let Some(rect) = Rect::from_xywh(region.x, region.y, region.width, region.height) {
        canvas.clip_rect(rect);
    }
    canvas.draw_pixmap(
        source.as_ref(),
        (source_x as f32 - canvas.transform().tx) / region.scale_x,
        (source_y as f32 - canvas.transform().ty) / region.scale_y,
        source_width as f32 / region.scale_x,
        source_height as f32 / region.scale_y,
    );
    canvas.restore();
}

/// Box-averaged downsample, matching the browser's high-quality path when the
/// pixelate redaction shrinks a region before scaling it back up.
fn downsample(source: &Pixmap, width: u32, height: u32) -> Option<Pixmap> {
    let mut target = Pixmap::new(width, height)?;
    let source_width = source.width();
    let source_height = source.height();
    let data = source.data();
    let out = target.data_mut();

    for y in 0..height {
        let y0 = (y * source_height / height).min(source_height - 1);
        let y1 = (((y + 1) * source_height + height - 1) / height).min(source_height);
        for x in 0..width {
            let x0 = (x * source_width / width).min(source_width - 1);
            let x1 = (((x + 1) * source_width + width - 1) / width).min(source_width);
            let mut sums = [0u32; 4];
            let mut count = 0u32;
            for sample_y in y0..y1.max(y0 + 1) {
                for sample_x in x0..x1.max(x0 + 1) {
                    let index = ((sample_y * source_width + sample_x) * 4) as usize;
                    for channel in 0..4 {
                        sums[channel] += data[index + channel] as u32;
                    }
                    count += 1;
                }
            }
            if count == 0 {
                continue;
            }
            let index = ((y * width + x) * 4) as usize;
            for channel in 0..4 {
                out[index + channel] = (sums[channel] / count) as u8;
            }
        }
    }
    Some(target)
}

/// Port of `scaleAnnotationToComposition` — drawing segments are authored
/// against the canvas the user drew on and replayed at composition size.
pub fn scale_to_composition(annotation: &Annotation, scale_x: f64, scale_y: f64) -> Annotation {
    let scale = (scale_x + scale_y) / 2.0;
    let scale_points = |points: &[f64]| -> Vec<f64> {
        points
            .iter()
            .enumerate()
            .map(|(index, value)| value * if index % 2 == 0 { scale_x } else { scale_y })
            .collect()
    };

    match annotation.clone() {
        Annotation::Pen {
            id,
            points,
            stroke,
            stroke_width,
        } => Annotation::Pen {
            id,
            points: scale_points(&points),
            stroke,
            stroke_width: stroke_width * scale,
        },
        Annotation::Highlight {
            id,
            points,
            fill,
            opacity,
            stroke_width,
        } => Annotation::Highlight {
            id,
            points: scale_points(&points),
            fill,
            opacity,
            stroke_width: stroke_width * scale,
        },
        Annotation::Rectangle {
            id,
            x,
            y,
            width,
            height,
            stroke,
            stroke_width,
            fill,
        } => Annotation::Rectangle {
            id,
            x: x * scale_x,
            y: y * scale_y,
            width: width * scale_x,
            height: height * scale_y,
            stroke,
            stroke_width,
            fill,
        },
        Annotation::Redact {
            id,
            x,
            y,
            width,
            height,
            style,
            intensity,
        } => Annotation::Redact {
            id,
            x: x * scale_x,
            y: y * scale_y,
            width: width * scale_x,
            height: height * scale_y,
            style,
            intensity,
        },
        Annotation::Circle {
            id,
            x,
            y,
            radius,
            stroke,
            stroke_width,
            fill,
        } => Annotation::Circle {
            id,
            x: x * scale_x,
            y: y * scale_y,
            radius: radius * scale,
            stroke,
            stroke_width,
            fill,
        },
        Annotation::Line {
            id,
            points,
            stroke,
            stroke_width,
        } => Annotation::Line {
            id,
            points: [
                points[0] * scale_x,
                points[1] * scale_y,
                points[2] * scale_x,
                points[3] * scale_y,
            ],
            stroke,
            stroke_width: stroke_width * scale,
        },
        Annotation::Arrow {
            id,
            points,
            stroke,
            stroke_width,
            arrow_style,
            bend_offset,
        } => Annotation::Arrow {
            id,
            points: [
                points[0] * scale_x,
                points[1] * scale_y,
                points[2] * scale_x,
                points[3] * scale_y,
            ],
            stroke,
            stroke_width: stroke_width * scale,
            arrow_style,
            bend_offset: bend_offset.map(|offset| Offset {
                x: offset.x * scale_x,
                y: offset.y * scale_y,
            }),
        },
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
        } => Annotation::Text {
            id,
            x: x * scale_x,
            y: y * scale_y,
            text,
            font_size: font_size * scale,
            fill,
            font_family,
            background_color,
            background_opacity,
            background_padding: background_padding.map(|padding| Offset {
                x: padding.x * scale,
                y: padding.y * scale,
            }),
            background_radius: background_radius.map(|radius| radius * scale),
            rotation,
        },
        Annotation::Number {
            id,
            x,
            y,
            value,
            display_value,
            fill,
            size,
        } => Annotation::Number {
            id,
            x: x * scale_x,
            y: y * scale_y,
            value,
            display_value,
            fill,
            size,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn covered(canvas: &Canvas) -> usize {
        canvas
            .pixmap()
            .data()
            .chunks_exact(4)
            .filter(|pixel| pixel[3] > 0)
            .count()
    }

    #[test]
    fn a_pen_stroke_covers_pixels_along_its_path() {
        let mut canvas = Canvas::new(120, 60).expect("canvas");
        let points: Vec<f64> = (0..30).flat_map(|step| [step as f64 * 4.0, 30.0]).collect();
        draw(
            &mut canvas,
            &Annotation::Pen {
                id: "pen".into(),
                points,
                stroke: "#ff0000".into(),
                stroke_width: 4.0,
            },
        );
        assert!(covered(&canvas) > 200);
    }

    #[test]
    fn a_blackout_redaction_fills_the_normalized_rect() {
        let mut canvas = Canvas::new(20, 20).expect("canvas");
        canvas.fill_all(Color::from_rgba8(255, 255, 255, 255));
        draw(
            &mut canvas,
            &Annotation::Redact {
                id: "redact".into(),
                x: 12.0,
                y: 12.0,
                width: -6.0,
                height: -6.0,
                style: "blackout".into(),
                intensity: 5.0,
            },
        );
        let pixel = |x: u32, y: u32| {
            let index = ((y * 20 + x) * 4) as usize;
            canvas.pixmap().data()[index]
        };
        assert_eq!(pixel(8, 8), 0);
        assert_eq!(pixel(2, 2), 255);
    }

    #[test]
    fn pixelating_flattens_detail_but_keeps_coverage() {
        let mut canvas = Canvas::new(32, 32).expect("canvas");
        canvas.fill_all(Color::from_rgba8(10, 20, 30, 255));
        draw(
            &mut canvas,
            &Annotation::Redact {
                id: "redact".into(),
                x: 4.0,
                y: 4.0,
                width: 24.0,
                height: 24.0,
                style: "pixelate".into(),
                intensity: 5.0,
            },
        );
        assert_eq!(covered(&canvas), 32 * 32);
    }

    #[test]
    fn scaling_moves_every_coordinate_into_composition_space() {
        let scaled = scale_to_composition(
            &Annotation::Line {
                id: "line".into(),
                points: [1.0, 2.0, 3.0, 4.0],
                stroke: "#000".into(),
                stroke_width: 2.0,
            },
            2.0,
            4.0,
        );
        let Annotation::Line {
            points,
            stroke_width,
            ..
        } = scaled
        else {
            panic!("expected line");
        };
        assert_eq!(points, [2.0, 8.0, 6.0, 16.0]);
        assert_eq!(stroke_width, 6.0);
    }

    #[test]
    fn a_circle_scales_by_the_average_axis_factor() {
        let scaled = scale_to_composition(
            &Annotation::Circle {
                id: "circle".into(),
                x: 10.0,
                y: 10.0,
                radius: 5.0,
                stroke: "#000".into(),
                stroke_width: 2.0,
                fill: None,
            },
            2.0,
            4.0,
        );
        let Annotation::Circle { x, y, radius, .. } = scaled else {
            panic!("expected circle");
        };
        assert_eq!((x, y, radius), (20.0, 40.0, 15.0));
    }

    #[test]
    fn an_arrow_head_is_drawn_at_the_tip() {
        let mut canvas = Canvas::new(80, 80).expect("canvas");
        draw(
            &mut canvas,
            &Annotation::Arrow {
                id: "arrow".into(),
                points: [10.0, 40.0, 70.0, 40.0],
                stroke: "#00ff00".into(),
                stroke_width: 3.0,
                arrow_style: None,
                bend_offset: None,
            },
        );
        let index = |x: u32, y: u32| ((y * 80 + x) * 4 + 3) as usize;
        // The wings splay back from the tip at 30 degrees, so the rows well
        // above and below the shaft are only covered near the head.
        let data = canvas.pixmap().data();
        assert!(data[index(58, 33)] > 0, "upper wing");
        assert!(data[index(58, 47)] > 0, "lower wing");
        assert_eq!(data[index(20, 33)], 0, "no wing at the tail");
    }
}
