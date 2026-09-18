use gpui::{px, PathBuilder, Pixels, Window};
use herogpui::gpui;

use crate::editor::annotations::{
    arrow_curve_control, arrow_head_points, arrow_head_size, normalize_rect, points_to_coordinates,
    Annotation,
};
use crate::theme::color::Srgba;
use crate::windows::video_editor::drawing_overlay::Rect;

const REDACT_PREVIEW_ALPHA: f32 = 0.85;

pub fn draw(window: &mut Window, annotation: &Annotation, content: Rect, scale: f32) {
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
            let mut builder = PathBuilder::fill();
            rect_path(
                &mut builder,
                normalize_rect(*x, *y, *width, *height),
                content,
                scale,
            );
            let alpha = if style == "blackout" {
                1.0
            } else {
                REDACT_PREVIEW_ALPHA
            };
            paint(window, builder, "#000000", alpha);
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
