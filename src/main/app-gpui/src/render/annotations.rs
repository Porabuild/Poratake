//! Port of `components/video-editor/composition/drawing-canvas-renderer.ts`.
//! Both the image editor's export and the video editor's drawing overlay draw
//! through this, which is what keeps a saved file identical to the preview.

use tiny_skia::{
    BlendMode, Color, FillRule, LineCap, LineJoin, PathBuilder, Pixmap, PixmapPaint, Rect, Stroke,
    Transform,
};

use crate::editor::annotations::{
    arrow_curve_control, arrow_head_points, arrow_head_size, normalize_rect, number_size_config,
    points_to_coordinates, redact_intensity, Annotation, Offset, DEFAULT_TEXT_FONT, TEXT_BG_RADIUS,
};
use crate::render::canvas::{circle_path, rounded_rect_path, Canvas};
use crate::render::freehand::{self, Vec2};
use crate::render::{blur, color, text};

#[derive(Clone, Copy, Debug)]
pub enum ScreenshotPaint<'a> {
    Annotation(&'a Annotation),
    BlackoutOverlay(&'a Annotation),
}

impl<'a> ScreenshotPaint<'a> {
    pub fn annotation(self) -> &'a Annotation {
        match self {
            Self::Annotation(annotation) | Self::BlackoutOverlay(annotation) => annotation,
        }
    }
}

pub fn screenshot_paints<'a>(
    annotations: impl Iterator<Item = &'a Annotation> + Clone,
) -> Vec<ScreenshotPaint<'a>> {
    let mut paints = Vec::with_capacity(annotations.size_hint().1.unwrap_or(0));
    for annotation in annotations.clone() {
        if matches!(annotation, Annotation::Redact { .. }) {
            paints.push(ScreenshotPaint::Annotation(annotation));
        }
    }
    for annotation in annotations.clone() {
        match annotation {
            Annotation::Redact { style, .. } if style == "blackout" => {
                paints.push(ScreenshotPaint::BlackoutOverlay(annotation));
            }
            Annotation::Redact { .. } | Annotation::Highlight { .. } => {}
            _ => paints.push(ScreenshotPaint::Annotation(annotation)),
        }
    }
    for annotation in annotations {
        if matches!(annotation, Annotation::Highlight { .. }) {
            paints.push(ScreenshotPaint::Annotation(annotation));
        }
    }
    paints
}

pub fn screenshot_annotation_at(
    annotations: &[Annotation],
    point: (f64, f64),
    zoom: f32,
) -> Option<&Annotation> {
    screenshot_paints(annotations.iter())
        .into_iter()
        .rev()
        .map(ScreenshotPaint::annotation)
        .find(|annotation| hit_test_annotation(annotation, point, zoom))
}

fn hit_test_annotation(annotation: &Annotation, point: (f64, f64), zoom: f32) -> bool {
    match annotation {
        Annotation::Rectangle {
            x,
            y,
            width,
            height,
            stroke_width,
            fill,
            ..
        } => {
            let (left, top, width, height) = normalize_rect(*x, *y, *width, *height);
            let fill = fill
                .as_deref()
                .is_some_and(|fill| !fill.eq_ignore_ascii_case("none"));
            rounded_rectangle_hit(
                point,
                (left, top, width, height),
                1.0,
                fill,
                stroke_width.abs() / 2.0 + 4.0 / zoom.max(0.01) as f64,
            )
        }
        Annotation::Circle {
            x,
            y,
            radius,
            stroke_width,
            fill,
            ..
        } => {
            if *radius <= 0.0 {
                return false;
            }
            let distance = (point.0 - x).hypot(point.1 - y);
            if fill
                .as_deref()
                .is_some_and(|fill| !fill.eq_ignore_ascii_case("none"))
                && distance <= *radius
            {
                return true;
            }
            (distance - radius).abs() <= stroke_width.abs() / 2.0 + 4.0 / zoom.max(0.01) as f64
        }
        Annotation::Line {
            points,
            stroke_width,
            ..
        } => {
            distance_to_segment(point, (points[0], points[1]), (points[2], points[3]))
                <= stroke_width.abs() / 2.0 + 4.0 / zoom.max(0.01) as f64
        }
        _ => {
            const TOLERANCE: f64 = 4.0;
            let (left, top, right, bottom) = annotation.bounds();
            point.0 >= left - TOLERANCE
                && point.0 <= right + TOLERANCE
                && point.1 >= top - TOLERANCE
                && point.1 <= bottom + TOLERANCE
        }
    }
}

fn distance_to_segment(point: (f64, f64), start: (f64, f64), end: (f64, f64)) -> f64 {
    let delta = (end.0 - start.0, end.1 - start.1);
    let length_squared = delta.0 * delta.0 + delta.1 * delta.1;
    if length_squared == 0.0 {
        return (point.0 - start.0).hypot(point.1 - start.1);
    }
    let progress = (((point.0 - start.0) * delta.0 + (point.1 - start.1) * delta.1)
        / length_squared)
        .clamp(0.0, 1.0);
    (point.0 - (start.0 + delta.0 * progress)).hypot(point.1 - (start.1 + delta.1 * progress))
}

fn rounded_rectangle_hit(
    point: (f64, f64),
    rect: (f64, f64, f64, f64),
    radius: f64,
    filled: bool,
    stroke_band: f64,
) -> bool {
    let (left, top, width, height) = rect;
    if width <= 0.0 || height <= 0.0 {
        return false;
    }
    if filled && point_in_rounded_rectangle(point, rect, radius) {
        return true;
    }
    if stroke_band <= 0.0 {
        return false;
    }
    let outer = (
        left - stroke_band,
        top - stroke_band,
        width + stroke_band * 2.0,
        height + stroke_band * 2.0,
    );
    if !point_in_rounded_rectangle(point, outer, radius + stroke_band) {
        return false;
    }
    let inner = (
        left + stroke_band,
        top + stroke_band,
        width - stroke_band * 2.0,
        height - stroke_band * 2.0,
    );
    inner.2 <= 0.0
        || inner.3 <= 0.0
        || !point_in_rounded_rectangle(point, inner, (radius - stroke_band).max(0.0))
}

fn point_in_rounded_rectangle(point: (f64, f64), rect: (f64, f64, f64, f64), radius: f64) -> bool {
    let (left, top, width, height) = rect;
    if width <= 0.0
        || height <= 0.0
        || point.0 < left
        || point.0 > left + width
        || point.1 < top
        || point.1 > top + height
    {
        return false;
    }
    let radius = radius.min(width / 2.0).min(height / 2.0).max(0.0);
    if radius == 0.0 {
        return true;
    }
    let center_x = point.0.clamp(left + radius, left + width - radius);
    let center_y = point.1.clamp(top + radius, top + height - radius);
    (point.0 - center_x).hypot(point.1 - center_y) <= radius
}

pub fn draw_all_screenshot(canvas: &mut Canvas, annotations: &[Annotation], native_scale: f64) {
    for paint in screenshot_paints(annotations.iter()) {
        draw_screenshot_paint(canvas, paint, native_scale);
    }
}

pub fn draw_screenshot_paint(canvas: &mut Canvas, paint: ScreenshotPaint<'_>, native_scale: f64) {
    match paint {
        ScreenshotPaint::Annotation(annotation) => {
            draw_screenshot(canvas, annotation, native_scale)
        }
        ScreenshotPaint::BlackoutOverlay(Annotation::Redact {
            x,
            y,
            width,
            height,
            ..
        }) => {
            let (x, y, width, height) = normalize_rect(*x, *y, *width, *height);
            if let Some(path) =
                rounded_rect_path(x as f32, y as f32, width as f32, height as f32, 2.)
            {
                canvas.fill_path(&path, Color::BLACK, FillRule::Winding);
            }
        }
        ScreenshotPaint::BlackoutOverlay(_) => {}
    }
}

pub fn draw_screenshot(canvas: &mut Canvas, annotation: &Annotation, native_scale: f64) {
    match annotation {
        Annotation::Redact {
            x,
            y,
            width,
            height,
            style,
            intensity,
            ..
        } if style == "pixelate" => {
            let (x, y, width, height) = normalize_rect(*x, *y, *width, *height);
            let block = (redact_intensity(*intensity).0 * native_scale)
                .round()
                .max(1.0);
            pixelate_screenshot(canvas, x, y, width, height, block);
        }
        Annotation::Redact {
            x,
            y,
            width,
            height,
            style,
            intensity,
            ..
        } if style == "blur" => {
            let (x, y, width, height) = normalize_rect(*x, *y, *width, *height);
            let radius = (redact_intensity(*intensity).1 * native_scale)
                .round()
                .max(1.0);
            blur_region(canvas, x, y, width, height, radius, true);
        }
        _ => draw(canvas, annotation),
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

pub fn pen_outline(points: &[f64], stroke_width: f64) -> Vec<Vec2> {
    let coordinates = points_to_coordinates(points);
    if coordinates.is_empty() {
        return Vec::new();
    }
    freehand::stroke(&coordinates, &freehand::Options::for_pen(stroke_width))
}

pub fn freehand_segments(points: &[Vec2]) -> Vec<(Vec2, Vec2)> {
    (0..points.len())
        .map(|index| {
            let (x0, y0) = points[index];
            let (x1, y1) = points[(index + 1) % points.len()];
            ((x0, y0), ((x0 + x1) / 2.0, (y0 + y1) / 2.0))
        })
        .collect()
}

/// `drawFreehandPath` — the outline is closed with quadratic segments through
/// each pair's midpoint, exactly as `getSvgPathFromStroke` builds it.
fn freehand_path(points: &[Vec2]) -> Option<tiny_skia::Path> {
    if points.is_empty() {
        return None;
    }
    let mut builder = PathBuilder::new();
    builder.move_to(points[0].0 as f32, points[0].1 as f32);
    for (control, end) in freehand_segments(points) {
        builder.quad_to(
            control.0 as f32,
            control.1 as f32,
            end.0 as f32,
            end.1 as f32,
        );
    }
    builder.close();
    builder.finish()
}

fn draw_pen(canvas: &mut Canvas, points: &[f64], stroke: &str, stroke_width: f64) {
    let outline = pen_outline(points, stroke_width);
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

pub fn highlighter_outline(points: &[f64], stroke_width: f64) -> Vec<Vec2> {
    let coordinates = points_to_coordinates(points);
    if coordinates.len() < 2 {
        return Vec::new();
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

    upper.extend(lower.iter().rev());
    upper
}

fn draw_highlight(
    canvas: &mut Canvas,
    points: &[f64],
    fill: &str,
    opacity: f64,
    stroke_width: f64,
) {
    let outline = highlighter_outline(points, stroke_width);
    let Some((first, rest)) = outline.split_first() else {
        return;
    };

    let mut builder = PathBuilder::new();
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
    match arrow_curve_control(points, arrow_style, bend) {
        Some(control) => (control.0, control.1, true),
        None => ((x1 + x2) / 2.0, (y1 + y2) / 2.0, false),
    }
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
    text::fill_text_weighted(
        canvas,
        display_value,
        DEFAULT_TEXT_FONT,
        text::Weight::Bold,
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
        "blur" => blur_region(
            canvas,
            left,
            top,
            width,
            height,
            blur_radius.max(1.0),
            false,
        ),
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
    let Some(mut source) = canvas.read_region(
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
    let source_width = source.width() as u64;
    let source_height = source.height() as u64;
    let columns: Vec<usize> = (0..source_width)
        .map(|x| ((((x * 2 + 1) * u64::from(small_width)) - 1) / (source_width * 2)) as usize)
        .collect();
    let small_pixels = small.data();
    for (y, row) in source
        .data_mut()
        .chunks_exact_mut(source_width as usize * 4)
        .enumerate()
    {
        let sample_y =
            ((((y as u64 * 2 + 1) * u64::from(small_height)) - 1) / (source_height * 2)) as usize;
        for (x, pixel) in row.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let sample = (sample_y * small_width as usize + columns[x]) * 4;
            pixel.copy_from_slice(&small_pixels[sample..sample + 4]);
        }
    }

    canvas.save();
    canvas.set_shadow(None);
    canvas.set_global_alpha(1.0);
    canvas.draw_pixmap_nearest(
        source.as_ref(),
        region.x,
        region.y,
        region.width,
        region.height,
    );
    canvas.restore();
}

fn pixelate_screenshot(canvas: &mut Canvas, x: f64, y: f64, width: f64, height: f64, block: f64) {
    let Some(region) = clamp_region(canvas, x, y, width, height) else {
        return;
    };
    let Some(mut source) = canvas.read_region(
        region.device_x,
        region.device_y,
        region.device_width,
        region.device_height,
    ) else {
        return;
    };
    let width = region.device_width as usize;
    let height = region.device_height as usize;
    let block_width = ((block as f32 * region.scale_x).round() as usize).max(1);
    let block_height = ((block as f32 * region.scale_y).round() as usize).max(1);
    let pixels = source.data_mut();
    for top in (0..height).step_by(block_height) {
        for left in (0..width).step_by(block_width) {
            let bottom = (top + block_height).min(height);
            let right = (left + block_width).min(width);
            let count = ((bottom - top) * (right - left)) as u64;
            let mut sums = [0_u64; 4];
            for y in top..bottom {
                for x in left..right {
                    let offset = (y * width + x) * 4;
                    let alpha = u64::from(pixels[offset + 3]);
                    match alpha {
                        0 => {}
                        255 => {
                            for channel in 0..3 {
                                sums[channel] += u64::from(pixels[offset + channel]);
                            }
                        }
                        _ => {
                            for channel in 0..3 {
                                sums[channel] +=
                                    (u64::from(pixels[offset + channel]) * 255 + alpha / 2) / alpha;
                            }
                        }
                    }
                    sums[3] += alpha;
                }
            }
            let alpha = (sums[3] + count / 2) / count;
            let color = [
                ((((sums[0] + count / 2) / count) * alpha + 127) / 255) as u8,
                ((((sums[1] + count / 2) / count) * alpha + 127) / 255) as u8,
                ((((sums[2] + count / 2) / count) * alpha + 127) / 255) as u8,
                alpha as u8,
            ];
            for y in top..bottom {
                for x in left..right {
                    let offset = (y * width + x) * 4;
                    pixels[offset..offset + 4].copy_from_slice(&color);
                }
            }
        }
    }
    let target_width = canvas.width() as usize;
    let target = canvas.pixmap_mut().data_mut();
    for y in 0..height {
        let from = y * width * 4;
        let to = ((region.device_y as usize + y) * target_width + region.device_x as usize) * 4;
        target[to..to + width * 4].copy_from_slice(&pixels[from..from + width * 4]);
    }
}

fn blur_region(
    canvas: &mut Canvas,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    radius: f64,
    screenshot: bool,
) {
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

    let sigma = radius as f32 * region.scale_x;
    let Some(mut source) = canvas.read_region(
        source_x,
        source_y,
        source_width as u32,
        source_height as u32,
    ) else {
        return;
    };
    if screenshot {
        let mut filtered = source.clone();
        blur::blur_transparent(&mut filtered, sigma);
        source.draw_pixmap(
            0,
            0,
            filtered.as_ref(),
            &PixmapPaint::default(),
            Transform::identity(),
            None,
        );
    } else {
        blur::blur(&mut source, sigma);
    }

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
        source.width() as f32 / region.scale_x,
        source.height() as f32 / region.scale_y,
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
        let y1 = ((y + 1) * source_height)
            .div_ceil(height)
            .min(source_height);
        for x in 0..width {
            let x0 = (x * source_width / width).min(source_width - 1);
            let x1 = ((x + 1) * source_width).div_ceil(width).min(source_width);
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

    fn outline_rect(id: &str, fill: Option<&str>) -> Annotation {
        Annotation::Rectangle {
            id: id.into(),
            x: 10.0,
            y: 10.0,
            width: 100.0,
            height: 80.0,
            stroke: "#000000".into(),
            stroke_width: 2.0,
            fill: fill.map(str::to_string),
        }
    }

    #[test]
    fn hollow_and_filled_shapes_hit_their_rendered_regions() {
        let hollow = outline_rect("hollow", None);
        let filled = outline_rect("filled", Some("#ff0000"));
        assert!(!hit_test_annotation(&hollow, (50.0, 50.0), 1.0));
        assert!(hit_test_annotation(&hollow, (10.0, 50.0), 1.0));
        assert!(hit_test_annotation(&filled, (50.0, 50.0), 1.0));

        let hollow_circle = Annotation::Circle {
            id: "hollow-circle".into(),
            x: 50.0,
            y: 50.0,
            radius: 20.0,
            stroke: "#000000".into(),
            stroke_width: 2.0,
            fill: None,
        };
        let filled_circle = Annotation::Circle {
            id: "filled-circle".into(),
            x: 50.0,
            y: 50.0,
            radius: 20.0,
            stroke: "#000000".into(),
            stroke_width: 2.0,
            fill: Some("#ff0000".into()),
        };
        assert!(!hit_test_annotation(&hollow_circle, (50.0, 50.0), 1.0));
        assert!(hit_test_annotation(&hollow_circle, (70.0, 50.0), 1.0));
        assert!(hit_test_annotation(&filled_circle, (50.0, 50.0), 1.0));
    }

    #[test]
    fn rectangle_corner_hits_match_the_rendered_one_pixel_radius() {
        let rectangle = Annotation::Rectangle {
            id: "rounded".into(),
            x: 10.0,
            y: 10.0,
            width: 20.0,
            height: 20.0,
            stroke: "#000000".into(),
            stroke_width: 0.0,
            fill: Some("#ff0000".into()),
        };
        let zoom = 10_000.0;
        assert!(!hit_test_annotation(&rectangle, (10.2, 10.2), zoom));
        assert!(hit_test_annotation(&rectangle, (10.5, 10.5), zoom));

        let mut canvas = Canvas::new(640, 640).expect("canvas");
        canvas.scale(16.0, 16.0);
        draw(&mut canvas, &rectangle);
        let alpha_at = |x: f64, y: f64| {
            let pixel = (y * 16.0).floor() as usize * 640 + (x * 16.0).floor() as usize;
            canvas.pixmap().data()[pixel * 4 + 3]
        };
        assert_eq!(alpha_at(10.2, 10.2), 0);
        assert!(alpha_at(10.5, 10.5) > 0);
    }

    #[test]
    fn shape_selection_slop_stays_four_screen_pixels_at_each_zoom() {
        let hollow = outline_rect("hollow", None);
        assert!(hit_test_annotation(&hollow, (5.0, 50.0), 0.5));
        assert!(!hit_test_annotation(&hollow, (5.0, 50.0), 4.0));

        let line = Annotation::Line {
            id: "line".into(),
            points: [10.0, 10.0, 90.0, 90.0],
            stroke: "#000000".into(),
            stroke_width: 2.0,
        };
        assert!(hit_test_annotation(&line, (50.0, 45.0), 0.5));
        assert!(!hit_test_annotation(&line, (50.0, 45.0), 4.0));
        assert!(!hit_test_annotation(&line, (10.0, 90.0), 1.0));
    }

    #[test]
    fn transparent_top_shape_does_not_hide_a_filled_shape_below_it() {
        let annotations = [
            outline_rect("filled", Some("#ff0000")),
            outline_rect("hollow", None),
        ];
        assert_eq!(
            screenshot_annotation_at(&annotations, (50.0, 50.0), 1.0).map(Annotation::id),
            Some("filled")
        );
        assert_eq!(
            screenshot_annotation_at(&annotations, (10.0, 50.0), 1.0).map(Annotation::id),
            Some("hollow")
        );

        let annotations = [
            outline_rect("filled", Some("#ff0000")),
            outline_rect("top", Some("#00ff00")),
        ];
        assert_eq!(
            screenshot_annotation_at(&annotations, (50.0, 50.0), 1.0).map(Annotation::id),
            Some("top")
        );
    }

    fn covered(canvas: &Canvas) -> usize {
        canvas
            .pixmap()
            .data()
            .as_chunks::<4>()
            .0
            .iter()
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
    fn screenshot_blur_expands_with_native_scale() {
        let annotation = Annotation::Redact {
            id: "blur".into(),
            x: 16.0,
            y: 8.0,
            width: 96.0,
            height: 48.0,
            style: "blur".into(),
            intensity: 2.0,
        };
        let sample = |native_scale| {
            let mut canvas = Canvas::new(128, 64).unwrap();
            canvas.fill_all(Color::BLACK);
            canvas.fill_rect(Rect::from_xywh(64., 0., 64., 64.).unwrap(), Color::WHITE);
            draw_screenshot(&mut canvas, &annotation, native_scale);
            canvas.pixmap().pixel(54, 32).unwrap().red()
        };
        let samples = [sample(1.0), sample(1.25), sample(2.0)];
        assert!(
            samples[0] < samples[1] && samples[1] < samples[2],
            "{samples:?}"
        );
    }

    #[test]
    fn screenshot_blur_preserves_a_tiny_solid_capture() {
        let mut canvas = Canvas::new(2, 2).unwrap();
        canvas.fill_all(Color::from_rgba8(160, 80, 40, 255));
        let before = canvas.pixmap().data().to_vec();
        draw_screenshot(
            &mut canvas,
            &Annotation::Redact {
                id: "blur".into(),
                x: 0.,
                y: 0.,
                width: 2.,
                height: 2.,
                style: "blur".into(),
                intensity: 10.,
            },
            2.,
        );
        assert_eq!(canvas.pixmap().data(), &before);
    }

    #[test]
    fn screenshot_blur_matches_electron_on_translucent_clipped_regions() {
        for (x, y, width, height, samples) in [
            (
                32.,
                32.,
                64.,
                64.,
                vec![
                    (32, 64, [63, 128, 191, 223]),
                    (64, 64, [64, 128, 191, 224]),
                    (95, 64, [63, 128, 191, 223]),
                    (20, 64, [64, 128, 191, 128]),
                ],
            ),
            (
                -10.,
                32.,
                50.,
                64.,
                vec![
                    (0, 64, [65, 128, 191, 208]),
                    (10, 64, [65, 129, 192, 220]),
                    (39, 64, [64, 128, 191, 224]),
                    (50, 64, [64, 128, 191, 128]),
                ],
            ),
            (
                96.,
                32.,
                50.,
                64.,
                vec![
                    (96, 64, [63, 128, 191, 223]),
                    (112, 64, [63, 127, 190, 223]),
                    (127, 64, [65, 128, 191, 208]),
                    (80, 64, [64, 128, 191, 128]),
                ],
            ),
            (
                0.,
                0.,
                128.,
                128.,
                vec![
                    (0, 0, [65, 128, 191, 200]),
                    (64, 64, [64, 128, 191, 224]),
                    (127, 64, [65, 128, 191, 208]),
                ],
            ),
        ] {
            let mut canvas = Canvas::new(128, 128).unwrap();
            canvas.fill_all(Color::from_rgba8(64, 128, 192, 128));
            draw_screenshot(
                &mut canvas,
                &Annotation::Redact {
                    id: "blur".into(),
                    x,
                    y,
                    width,
                    height,
                    style: "blur".into(),
                    intensity: 2.,
                },
                1.,
            );
            let rendered = crate::editor::export::to_rgba(canvas.pixmap());
            for (sample_x, sample_y, expected) in samples {
                let actual = rendered.get_pixel(sample_x, sample_y).0;
                assert!(
                    actual.iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 2),
                    "redaction {x},{y}, pixel {sample_x},{sample_y}: {actual:?} vs {expected:?}"
                );
            }
        }
    }

    #[test]
    fn screenshot_blur_approximates_electron_edge_profiles() {
        for (intensity, native_scale, expected) in [
            (1., 1., [0, 2, 33, 140, 234, 254, 255]),
            (2., 1., [2, 30, 73, 134, 192, 231, 254]),
            (2., 1.25, [6, 43, 83, 132, 180, 217, 250]),
            (4., 1., [28, 71, 99, 131, 162, 189, 229]),
            (5., 1., [42, 81, 105, 130, 155, 178, 216]),
            (4., 2., [69, 98, 113, 129, 145, 160, 188]),
        ] {
            let mut canvas = Canvas::new(512, 512).unwrap();
            canvas.fill_all(Color::BLACK);
            canvas.fill_rect(Rect::from_xywh(256., 0., 256., 512.).unwrap(), Color::WHITE);
            draw_screenshot(
                &mut canvas,
                &Annotation::Redact {
                    id: "blur".into(),
                    x: 128.,
                    y: 128.,
                    width: 256.,
                    height: 256.,
                    style: "blur".into(),
                    intensity,
                },
                native_scale,
            );
            for (offset, expected) in [-20, -10, -5, 0, 5, 10, 20].into_iter().zip(expected) {
                let actual = canvas
                    .pixmap()
                    .pixel((256 + offset) as u32, 256)
                    .unwrap()
                    .red();
                assert!(
                    actual.abs_diff(expected) <= 3,
                    "intensity={intensity}, scale={native_scale}, offset={offset}: {actual} vs {expected}"
                );
            }
        }
    }

    #[test]
    fn screenshot_blur_uses_electrons_padded_source_bounds() {
        for (stripe_x, stripe_width, expected) in
            [(0., 32., [0, 0, 0, 0, 0]), (32., 8., [32, 26, 14, 1, 0])]
        {
            let mut canvas = Canvas::new(128, 128).unwrap();
            canvas.fill_all(Color::BLACK);
            canvas.fill_rect(
                Rect::from_xywh(stripe_x, 0., stripe_width, 128.).unwrap(),
                Color::WHITE,
            );
            draw_screenshot(
                &mut canvas,
                &Annotation::Redact {
                    id: "blur".into(),
                    x: 48.,
                    y: 32.,
                    width: 32.,
                    height: 64.,
                    style: "blur".into(),
                    intensity: 2.,
                },
                1.,
            );
            for (x, expected) in [48, 49, 52, 60, 79].into_iter().zip(expected) {
                let actual = canvas.pixmap().pixel(x, 64).unwrap().red();
                assert!(
                    actual.abs_diff(expected) <= 3,
                    "stripe at {stripe_x}, pixel {x}: {actual} vs {expected}"
                );
            }
        }
    }

    #[test]
    fn pixelation_uses_the_same_nearest_boundaries_at_each_position() {
        let mut source = Pixmap::new(30, 1).unwrap();
        for (x, pixel) in source
            .data_mut()
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .enumerate()
        {
            let red = match x {
                0..=7 => 10,
                8..=14 => 20,
                15..=22 => 30,
                _ => 40,
            };
            *pixel = [red, 0, 0, 255];
        }
        let mut canvas = Canvas::new(160, 1).unwrap();
        for x in [1., 105.] {
            canvas.draw_pixmap(source.as_ref(), x, 0., 30., 1.);
            pixelate_region(&mut canvas, f64::from(x), 0., 30., 1., 8.);
        }
        for x in [1, 105] {
            for i in 0..30 {
                let expected = match i {
                    0..=7 => 10,
                    8..=14 => 18,
                    15..=22 => 30,
                    _ => 38,
                };
                assert_eq!(canvas.pixmap().data()[(x + i) * 4], expected, "{x}+{i}");
            }
        }
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
