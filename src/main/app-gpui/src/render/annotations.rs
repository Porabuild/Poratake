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

const HIT_TEST_RAW_POINT_BUDGET: usize = 4_096;
const HIT_TEST_OUTLINE_VERTEX_BUDGET: usize = 65_536;
const HIT_TEST_CURVE_STEP_BUDGET: usize = 262_144;
const HIT_TEST_CURVE_FLATNESS_SCREEN_PX: f64 = 0.25;
const HIT_TEST_CURVE_MAX_DEPTH: u8 = 16;

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
    annotations: impl Iterator<Item = &'a Annotation>,
) -> Vec<ScreenshotPaint<'a>> {
    let mut paints = Vec::with_capacity(annotations.size_hint().1.unwrap_or(0));
    for annotation in annotations {
        paints.push(ScreenshotPaint::Annotation(annotation));
        if matches!(annotation, Annotation::Redact { style, .. } if style == "blackout") {
            paints.push(ScreenshotPaint::BlackoutOverlay(annotation));
        }
    }
    paints
}

pub fn screenshot_annotation_at(
    annotations: &[Annotation],
    point: (f64, f64),
    zoom: f32,
) -> Option<&Annotation> {
    screenshot_annotation_at_with_budget(annotations, point, zoom, &mut HitTestBudget::default())
}

#[derive(Default)]
struct HitTestBudget {
    raw_points: usize,
    outline_vertices: usize,
    curve_steps: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AnnotationHit {
    Hit,
    Miss,
    Indeterminate,
}

impl HitTestBudget {
    fn charge(&mut self, raw_points: usize, outline_vertices: usize) -> bool {
        if raw_points > HIT_TEST_RAW_POINT_BUDGET.saturating_sub(self.raw_points)
            || outline_vertices
                > HIT_TEST_OUTLINE_VERTEX_BUDGET.saturating_sub(self.outline_vertices)
        {
            return false;
        }
        self.raw_points += raw_points;
        self.outline_vertices += outline_vertices;
        true
    }

    fn spend_curve_step(&mut self) -> bool {
        if self.curve_steps == HIT_TEST_CURVE_STEP_BUDGET {
            return false;
        }
        self.curve_steps += 1;
        true
    }
}

fn screenshot_annotation_at_with_budget<'a>(
    annotations: &'a [Annotation],
    point: (f64, f64),
    zoom: f32,
    budget: &mut HitTestBudget,
) -> Option<&'a Annotation> {
    let mut fallback = None;
    for annotation in screenshot_paints(annotations.iter())
        .into_iter()
        .rev()
        .map(ScreenshotPaint::annotation)
    {
        match hit_test_annotation(annotation, point, zoom, budget) {
            AnnotationHit::Hit => return Some(annotation),
            AnnotationHit::Miss => {}
            AnnotationHit::Indeterminate if fallback.is_none() => fallback = Some(annotation),
            AnnotationHit::Indeterminate => {}
        }
    }
    fallback
}

fn hit_test_annotation(
    annotation: &Annotation,
    point: (f64, f64),
    zoom: f32,
    budget: &mut HitTestBudget,
) -> AnnotationHit {
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
            if rounded_rectangle_hit(
                point,
                (left, top, width, height),
                1.0,
                fill,
                stroke_width.abs() / 2.0 + 4.0 / zoom.max(0.01) as f64,
            ) {
                AnnotationHit::Hit
            } else {
                AnnotationHit::Miss
            }
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
                return AnnotationHit::Miss;
            }
            let distance = (point.0 - x).hypot(point.1 - y);
            if fill
                .as_deref()
                .is_some_and(|fill| !fill.eq_ignore_ascii_case("none"))
                && distance <= *radius
            {
                return AnnotationHit::Hit;
            }
            if (distance - radius).abs() <= stroke_width.abs() / 2.0 + 4.0 / zoom.max(0.01) as f64 {
                AnnotationHit::Hit
            } else {
                AnnotationHit::Miss
            }
        }
        Annotation::Line {
            points,
            stroke_width,
            ..
        } => {
            if distance_to_segment(point, (points[0], points[1]), (points[2], points[3]))
                <= stroke_width.abs() / 2.0 + 4.0 / zoom.max(0.01) as f64
            {
                AnnotationHit::Hit
            } else {
                AnnotationHit::Miss
            }
        }
        Annotation::Arrow {
            points,
            stroke_width,
            arrow_style,
            bend_offset,
            ..
        } => match arrow_hit_test(
            *points,
            *stroke_width,
            arrow_style.as_deref(),
            *bend_offset,
            point,
            zoom,
            budget,
        ) {
            Some(true) => AnnotationHit::Hit,
            Some(false) => AnnotationHit::Miss,
            None => legacy_indeterminate_annotation_hit(annotation, point),
        },
        Annotation::Pen {
            points,
            stroke_width,
            ..
        } => hit_test_pen(points, *stroke_width, point, zoom, budget),
        Annotation::Highlight {
            points,
            stroke_width,
            ..
        } => hit_test_highlight(points, *stroke_width, point, zoom, budget),
        _ => legacy_annotation_hit(annotation, point),
    }
}

fn hit_test_pen(
    points: &[f64],
    stroke_width: f64,
    point: (f64, f64),
    zoom: f32,
    budget: &mut HitTestBudget,
) -> AnnotationHit {
    let samples = points.len() / 2;
    if !budget.charge(samples, 0) {
        return legacy_pen_bounds_hit(points, point);
    }
    if !stroke_width.is_finite()
        || stroke_width <= 0.0
        || !point.0.is_finite()
        || !point.1.is_finite()
        || points
            .as_chunks::<2>()
            .0
            .iter()
            .any(|pair| !pair[0].is_finite() || !pair[1].is_finite())
    {
        return legacy_pen_bounds_hit(points, point);
    }
    let Some((left, top, right, bottom)) = point_bounds(points) else {
        return AnnotationHit::Miss;
    };
    let slop = 4.0 / f64::from(zoom.max(0.01));
    let radius = stroke_width * 1.5 + slop;
    if point.0 < left - radius
        || point.0 > right + radius
        || point.1 < top - radius
        || point.1 > bottom + radius
    {
        return AnnotationHit::Miss;
    }
    let remaining_outline = HIT_TEST_OUTLINE_VERTEX_BUDGET - budget.outline_vertices;
    if remaining_outline == 0 {
        return legacy_pen_bounds_hit(points, point);
    }
    let Some(outline) = freehand::stroke_outline_limited(
        points,
        &freehand::Options::for_pen(stroke_width),
        remaining_outline,
    ) else {
        budget.outline_vertices = HIT_TEST_OUTLINE_VERTEX_BUDGET;
        return legacy_pen_bounds_hit(points, point);
    };
    budget.outline_vertices += outline.len();
    match pen_outline_hit(&outline, point, slop, zoom, budget) {
        Some(true) => AnnotationHit::Hit,
        Some(false) => AnnotationHit::Miss,
        None => legacy_pen_bounds_hit(points, point),
    }
}

fn hit_test_highlight(
    points: &[f64],
    stroke_width: f64,
    point: (f64, f64),
    zoom: f32,
    budget: &mut HitTestBudget,
) -> AnnotationHit {
    let samples = points.len() / 2;
    let Some(outline_bound) = samples.checked_mul(2) else {
        return legacy_highlight_bounds_hit(points, point);
    };
    if !budget.charge(samples, 0) {
        return legacy_highlight_bounds_hit(points, point);
    }
    if !point.0.is_finite()
        || !point.1.is_finite()
        || !stroke_width.is_finite()
        || points
            .as_chunks::<2>()
            .0
            .iter()
            .any(|pair| !pair[0].is_finite() || !pair[1].is_finite())
    {
        return legacy_highlight_bounds_hit(points, point);
    }
    let Some((left, top, right, bottom)) = point_bounds(points) else {
        return AnnotationHit::Miss;
    };
    let slop = 4.0 / f64::from(zoom.max(0.01));
    let radius = stroke_width.abs() / 2.0 + slop;
    if point.0 < left - radius
        || point.0 > right + radius
        || point.1 < top - radius
        || point.1 > bottom + radius
    {
        return AnnotationHit::Miss;
    }
    if !budget.charge(0, outline_bound) {
        return legacy_highlight_bounds_hit(points, point);
    }
    let outline = crate::render::annotations::highlighter_outline(points, stroke_width);
    if polygon_hit(&outline, point, slop) {
        AnnotationHit::Hit
    } else {
        AnnotationHit::Miss
    }
}

fn legacy_annotation_hit(annotation: &Annotation, point: (f64, f64)) -> AnnotationHit {
    let (left, top, right, bottom) = annotation.bounds();
    if bounds_hit(left, top, right, bottom, point) {
        AnnotationHit::Hit
    } else {
        AnnotationHit::Miss
    }
}

fn legacy_indeterminate_annotation_hit(
    annotation: &Annotation,
    point: (f64, f64),
) -> AnnotationHit {
    let (left, top, right, bottom) = annotation.bounds();
    if bounds_hit(left, top, right, bottom, point) {
        AnnotationHit::Indeterminate
    } else {
        AnnotationHit::Miss
    }
}

fn legacy_pen_bounds_hit(points: &[f64], point: (f64, f64)) -> AnnotationHit {
    if points.as_chunks::<2>().0.is_empty() {
        return if bounds_hit(0.0, 0.0, 0.0, 0.0, point) {
            AnnotationHit::Indeterminate
        } else {
            AnnotationHit::Miss
        };
    }
    if point_bounds(points)
        .is_some_and(|(left, top, right, bottom)| bounds_hit(left, top, right, bottom, point))
    {
        AnnotationHit::Indeterminate
    } else {
        AnnotationHit::Miss
    }
}

fn legacy_highlight_bounds_hit(points: &[f64], point: (f64, f64)) -> AnnotationHit {
    legacy_pen_bounds_hit(points, point)
}

fn bounds_hit(left: f64, top: f64, right: f64, bottom: f64, point: (f64, f64)) -> bool {
    const TOLERANCE: f64 = 4.0;
    point.0 >= left - TOLERANCE
        && point.0 <= right + TOLERANCE
        && point.1 >= top - TOLERANCE
        && point.1 <= bottom + TOLERANCE
}

fn point_bounds(points: &[f64]) -> Option<(f64, f64, f64, f64)> {
    let first = points.as_chunks::<2>().0.first()?;
    let (mut left, mut top, mut right, mut bottom) = (first[0], first[1], first[0], first[1]);
    for pair in points.as_chunks::<2>().0.iter().skip(1) {
        left = left.min(pair[0]);
        top = top.min(pair[1]);
        right = right.max(pair[0]);
        bottom = bottom.max(pair[1]);
    }
    Some((left, top, right, bottom))
}

fn pen_outline_hit(
    outline: &[Vec2],
    point: (f64, f64),
    slop: f64,
    zoom: f32,
    budget: &mut HitTestBudget,
) -> Option<bool> {
    let Some(first) = outline.first().copied() else {
        return Some(false);
    };
    let mut current = first;
    let mut winding = 0i32;
    for index in 0..outline.len() {
        let control = outline[index];
        let next = outline[(index + 1) % outline.len()];
        let end = ((control.0 + next.0) / 2.0, (control.1 + next.1) / 2.0);
        let hit = scan_quadratic_outline(
            point,
            current,
            control,
            end,
            slop,
            f64::from(zoom.max(0.01)),
            HIT_TEST_CURVE_MAX_DEPTH,
            budget,
            &mut winding,
        )?;
        if hit {
            return Some(true);
        }
        current = end;
    }
    if distance_to_segment(point, current, first) <= slop {
        return Some(true);
    }
    update_winding(point, current, first, &mut winding);
    Some(winding != 0)
}

fn scan_quadratic_outline(
    point: (f64, f64),
    start: Vec2,
    control: Vec2,
    end: Vec2,
    slop: f64,
    zoom: f64,
    depth: u8,
    budget: &mut HitTestBudget,
    winding: &mut i32,
) -> Option<bool> {
    if !budget.spend_curve_step() {
        return None;
    }
    if ![
        point.0, point.1, start.0, start.1, control.0, control.1, end.0, end.1,
    ]
    .into_iter()
    .all(f64::is_finite)
    {
        return None;
    }
    let flatness = distance_to_segment(control, start, end);
    if !flatness.is_finite() {
        return None;
    }
    if flatness * zoom <= HIT_TEST_CURVE_FLATNESS_SCREEN_PX {
        let hit_radius = slop + flatness;
        if distance_to_segment(point, start, end) <= hit_radius {
            return Some(true);
        }
        update_winding(point, start, end, winding);
        return Some(false);
    }
    if depth == 0 {
        return None;
    }
    let midpoint = |a: Vec2, b: Vec2| ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
    let start_control = midpoint(start, control);
    let control_end = midpoint(control, end);
    let center = midpoint(start_control, control_end);
    if scan_quadratic_outline(
        point,
        start,
        start_control,
        center,
        slop,
        zoom,
        depth - 1,
        budget,
        winding,
    )? {
        return Some(true);
    }
    scan_quadratic_outline(
        point,
        center,
        control_end,
        end,
        slop,
        zoom,
        depth - 1,
        budget,
        winding,
    )
}

fn polygon_hit(outline: &[Vec2], point: (f64, f64), slop: f64) -> bool {
    let Some(mut previous) = outline.last().copied() else {
        return false;
    };
    let mut winding = 0i32;
    let mut near = false;
    for current in outline.iter().copied() {
        if distance_to_segment(point, previous, current) <= slop {
            near = true;
        }
        update_winding(point, previous, current, &mut winding);
        previous = current;
    }
    near || winding != 0
}

fn update_winding(point: (f64, f64), start: (f64, f64), end: (f64, f64), winding: &mut i32) {
    if (start.1 <= point.1 && end.1 > point.1) || (start.1 > point.1 && end.1 <= point.1) {
        let cross =
            (end.0 - start.0) * (point.1 - start.1) - (point.0 - start.0) * (end.1 - start.1);
        if end.1 > start.1 && cross > 0.0 {
            *winding += 1;
        }
        if end.1 < start.1 && cross < 0.0 {
            *winding -= 1;
        }
    }
}

fn distance_to_segment(point: (f64, f64), start: (f64, f64), end: (f64, f64)) -> f64 {
    if ![point.0, point.1, start.0, start.1, end.0, end.1]
        .into_iter()
        .all(f64::is_finite)
    {
        return f64::INFINITY;
    }
    let delta = (end.0 - start.0, end.1 - start.1);
    let length_squared = delta.0 * delta.0 + delta.1 * delta.1;
    if !length_squared.is_finite() {
        return f64::INFINITY;
    }
    if length_squared == 0.0 {
        return (point.0 - start.0).hypot(point.1 - start.1);
    }
    let progress = (((point.0 - start.0) * delta.0 + (point.1 - start.1) * delta.1)
        / length_squared)
        .clamp(0.0, 1.0);
    (point.0 - (start.0 + delta.0 * progress)).hypot(point.1 - (start.1 + delta.1 * progress))
}

fn arrow_hit_test(
    points: [f64; 4],
    stroke_width: f64,
    arrow_style: Option<&str>,
    bend: Option<Offset>,
    point: (f64, f64),
    zoom: f32,
    budget: &mut HitTestBudget,
) -> Option<bool> {
    if !points.into_iter().all(f64::is_finite)
        || !stroke_width.is_finite()
        || !point.0.is_finite()
        || !point.1.is_finite()
    {
        return Some(false);
    }
    let (x1, y1, x2, y2) = (points[0], points[1], points[2], points[3]);
    let zoom = f64::from(zoom.max(0.01));
    let hit_radius = stroke_width.abs() / 2.0 + 4.0 / zoom;
    if !hit_radius.is_finite() {
        return Some(false);
    }
    let control = arrow_curve_control(&points, arrow_style, bend);
    if let Some((control_x, control_y)) = control {
        if !control_x.is_finite() || !control_y.is_finite() {
            return Some(false);
        }
        if quadratic_stroke_hit(
            point,
            (x1, y1),
            (control_x, control_y),
            (x2, y2),
            hit_radius,
            zoom,
            16,
            budget,
        )? {
            return Some(true);
        }
    } else if distance_to_segment(point, (x1, y1), (x2, y2)) <= hit_radius {
        return Some(true);
    }

    let head_size = arrow_head_size(stroke_width);
    if !head_size.is_finite() {
        return Some(false);
    }
    let end_angle = control.map_or_else(
        || (y2 - y1).atan2(x2 - x1),
        |(control_x, control_y)| (y2 - control_y).atan2(x2 - control_x),
    );
    if arrow_head_hit(point, x2, y2, end_angle, head_size, hit_radius) {
        return Some(true);
    }
    if matches!(arrow_style, Some("double" | "double-curved")) {
        let start_angle = control.map_or_else(
            || (y1 - y2).atan2(x1 - x2),
            |(control_x, control_y)| (y1 - control_y).atan2(x1 - control_x),
        );
        return Some(arrow_head_hit(
            point,
            x1,
            y1,
            start_angle,
            head_size,
            hit_radius,
        ));
    }
    Some(false)
}

fn arrow_head_hit(
    point: (f64, f64),
    x: f64,
    y: f64,
    angle: f64,
    head_size: f64,
    hit_radius: f64,
) -> bool {
    let (left, right) = arrow_head_points(x, y, angle, head_size);
    distance_to_segment(point, (x, y), left) <= hit_radius
        || distance_to_segment(point, (x, y), right) <= hit_radius
}

fn quadratic_stroke_hit(
    point: (f64, f64),
    start: (f64, f64),
    control: (f64, f64),
    end: (f64, f64),
    hit_radius: f64,
    zoom: f64,
    depth: u8,
    budget: &mut HitTestBudget,
) -> Option<bool> {
    if !budget.spend_curve_step() {
        return None;
    }
    if ![start.0, start.1, control.0, control.1, end.0, end.1]
        .into_iter()
        .all(f64::is_finite)
    {
        return Some(false);
    }
    let min_x = start.0.min(control.0).min(end.0) - hit_radius;
    let max_x = start.0.max(control.0).max(end.0) + hit_radius;
    let min_y = start.1.min(control.1).min(end.1) - hit_radius;
    let max_y = start.1.max(control.1).max(end.1) + hit_radius;
    if point.0 < min_x || point.0 > max_x || point.1 < min_y || point.1 > max_y {
        return Some(false);
    }
    let flatness = distance_to_segment(control, start, end);
    if !flatness.is_finite() {
        return Some(false);
    }
    if flatness * zoom <= HIT_TEST_CURVE_FLATNESS_SCREEN_PX {
        return Some(distance_to_segment(point, start, end) <= hit_radius + flatness);
    }
    if depth == 0 {
        return None;
    }
    let midpoint = |a: (f64, f64), b: (f64, f64)| ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
    let start_control = midpoint(start, control);
    let control_end = midpoint(control, end);
    let center = midpoint(start_control, control_end);
    if quadratic_stroke_hit(
        point,
        start,
        start_control,
        center,
        hit_radius,
        zoom,
        depth - 1,
        budget,
    )? {
        return Some(true);
    }
    quadratic_stroke_hit(
        point,
        center,
        control_end,
        end,
        hit_radius,
        zoom,
        depth - 1,
        budget,
    )
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
            ..
        } if style == "blackout" => {
            let (left, top, width, height) =
                screenshot_blackout_rect(*x, *y, *width, *height, canvas.pixel_grid_origin());
            if let Some(rect) =
                Rect::from_xywh(left as f32, top as f32, width as f32, height as f32)
            {
                canvas.save();
                canvas.set_shadow(None);
                canvas.fill_rect(rect, Color::BLACK);
                canvas.restore();
            }
        }
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

pub fn screenshot_blackout_rect(
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    origin: (f64, f64),
) -> (f64, f64, f64, f64) {
    let (x, y, width, height) = normalize_rect(x, y, width, height);
    let left = (x + origin.0).floor();
    let top = (y + origin.1).floor();
    if width <= 0. || height <= 0. {
        return (left - origin.0, top - origin.1, 0., 0.);
    }
    let right = (x + origin.0 + width).ceil();
    let bottom = (y + origin.1 + height).ceil();
    (left - origin.0, top - origin.1, right - left, bottom - top)
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
    clamp_region_to_bounds(
        canvas,
        x,
        y,
        width,
        height,
        (0, 0, canvas.width() as i32, canvas.height() as i32),
    )
}

fn clamp_region_to_bounds(
    canvas: &Canvas,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    bounds: (i32, i32, i32, i32),
) -> Option<Region> {
    if width <= 0.0 || height <= 0.0 {
        return None;
    }
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
    let clamped_x = floored_x.max(bounds.0 as f32);
    let clamped_y = floored_y.max(bounds.1 as f32);
    let clamped_width = (origin_x + width as f32 * scale_x)
        .ceil()
        .min(bounds.2 as f32)
        - clamped_x;
    let clamped_height = (origin_y + height as f32 * scale_y)
        .ceil()
        .min(bounds.3 as f32)
        - clamped_y;
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
    let mut columns: Vec<(usize, usize, usize)> = Vec::new();
    for x in 0..source_width {
        let sample = ((((x * 2 + 1) * u64::from(small_width)) - 1) / (source_width * 2)) as usize;
        if let Some((_, end, _)) = columns
            .last_mut()
            .filter(|(_, _, column)| *column == sample)
        {
            *end = x as usize + 1;
            continue;
        }
        columns.push((x as usize, x as usize + 1, sample));
    }
    let small_pixels = small.data();
    for (y, row) in source
        .data_mut()
        .chunks_exact_mut(source_width as usize * 4)
        .enumerate()
    {
        let sample_y =
            ((((y as u64 * 2 + 1) * u64::from(small_height)) - 1) / (source_height * 2)) as usize;
        let row = row.as_chunks_mut::<4>().0;
        for &(start, end, column) in &columns {
            let sample = (sample_y * small_width as usize + column) * 4;
            let color = small_pixels[sample..sample + 4].try_into().unwrap();
            row[start..end].fill(color);
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
    let Some(full_region) =
        clamp_region_to_bounds(canvas, x, y, width, height, canvas.screenshot_surface())
    else {
        return;
    };
    let origin = (full_region.device_x, full_region.device_y);
    pixelate_screenshot_region(canvas, region, block, origin);
}

fn pixelate_screenshot_region(canvas: &mut Canvas, region: Region, block: f64, origin: (i32, i32)) {
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
    let phase_x =
        (i64::from(region.device_x) - i64::from(origin.0)).rem_euclid(block_width as i64) as usize;
    let phase_y =
        (i64::from(region.device_y) - i64::from(origin.1)).rem_euclid(block_height as i64) as usize;
    let pixels = source.data_mut();
    let mut top = 0;
    while top < height {
        let bottom = (top
            + if top == 0 {
                block_height - phase_y
            } else {
                block_height
            })
        .min(height);
        let mut left = 0;
        while left < width {
            let right = (left
                + if left == 0 {
                    block_width - phase_x
                } else {
                    block_width
                })
            .min(width);
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
            left = right;
        }
        top = bottom;
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
    let bounds = if screenshot {
        canvas.screenshot_surface()
    } else {
        (0, 0, canvas.width() as i32, canvas.height() as i32)
    };
    let Some(full_region) = clamp_region_to_bounds(canvas, x, y, width, height, bounds) else {
        return;
    };
    let full_x = (full_region.device_x - padding).max(bounds.0);
    let full_y = (full_region.device_y - padding).max(bounds.1);
    let full_width =
        ((full_region.device_width as i32 + padding * 2).min(bounds.2 - full_x)).max(1);
    let full_height =
        ((full_region.device_height as i32 + padding * 2).min(bounds.3 - full_y)).max(1);
    let source_x = full_x.max(0);
    let source_y = full_y.max(0);
    let source_width = (full_x + full_width).min(canvas.width() as i32) - source_x;
    let source_height = (full_y + full_height).min(canvas.height() as i32) - source_y;
    if source_width <= 0 || source_height <= 0 {
        return;
    }

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
        blur::blur_transparent(&mut source, sigma);
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

fn downsample(source: &Pixmap, width: u32, height: u32) -> Option<Pixmap> {
    let mut target = Pixmap::new(width, height)?;
    let source_width = source.width();
    let source_height = source.height();
    let data = source.data();
    let out = target.data_mut();

    if source_width.is_multiple_of(width) && source_height.is_multiple_of(height) {
        let block_width = source_width / width;
        let block_height = source_height / height;
        let divisor = u64::from(block_width) * u64::from(block_height);
        for y in 0..height {
            for x in 0..width {
                let mut sums = [0u64; 4];
                for row in y * block_height..(y + 1) * block_height {
                    let start = (row as usize * source_width as usize
                        + x as usize * block_width as usize)
                        * 4;
                    let end = start + block_width as usize * 4;
                    for pixel in data[start..end].as_chunks::<4>().0 {
                        for channel in 0..4 {
                            sums[channel] += u64::from(pixel[channel]);
                        }
                    }
                }
                let index = ((y * width + x) * 4) as usize;
                for channel in 0..4 {
                    out[index + channel] = ((sums[channel] + divisor / 2) / divisor) as u8;
                }
            }
        }
        return Some(target);
    }

    let divisor = u64::from(source_width) * u64::from(source_height);
    for y in 0..height {
        let top = u64::from(y) * u64::from(source_height);
        let bottom = u64::from(y + 1) * u64::from(source_height);
        let y0 = top / u64::from(height);
        let y1 = bottom.div_ceil(u64::from(height));
        for x in 0..width {
            let left = u64::from(x) * u64::from(source_width);
            let right = u64::from(x + 1) * u64::from(source_width);
            let x0 = left / u64::from(width);
            let x1 = right.div_ceil(u64::from(width));
            let mut sums = [0u64; 4];
            for sample_y in y0..y1 {
                let weight_y = bottom.min((sample_y + 1) * u64::from(height))
                    - top.max(sample_y * u64::from(height));
                for sample_x in x0..x1 {
                    let weight_x = right.min((sample_x + 1) * u64::from(width))
                        - left.max(sample_x * u64::from(width));
                    let weight = weight_x * weight_y;
                    let index = ((sample_y * u64::from(source_width) + sample_x) * 4) as usize;
                    for channel in 0..4 {
                        sums[channel] += u64::from(data[index + channel]) * weight;
                    }
                }
            }
            let index = ((y * width + x) * 4) as usize;
            for channel in 0..4 {
                out[index + channel] = ((sums[channel] + divisor / 2) / divisor) as u8;
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

    fn test_hit_test(annotation: &Annotation, point: (f64, f64), zoom: f32) -> bool {
        hit_test_annotation(annotation, point, zoom, &mut HitTestBudget::default())
            == AnnotationHit::Hit
    }

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

    fn u_points() -> Vec<f64> {
        vec![10.0, 10.0, 90.0, 10.0, 90.0, 90.0, 10.0, 90.0]
    }

    fn pen(id: &str, points: Vec<f64>) -> Annotation {
        Annotation::Pen {
            id: id.into(),
            points,
            stroke: "#000000".into(),
            stroke_width: 8.0,
        }
    }

    #[test]
    fn hollow_and_filled_shapes_hit_their_rendered_regions() {
        let hollow = outline_rect("hollow", None);
        let filled = outline_rect("filled", Some("#ff0000"));
        assert!(!test_hit_test(&hollow, (50.0, 50.0), 1.0));
        assert!(test_hit_test(&hollow, (10.0, 50.0), 1.0));
        assert!(test_hit_test(&filled, (50.0, 50.0), 1.0));

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
        assert!(!test_hit_test(&hollow_circle, (50.0, 50.0), 1.0));
        assert!(test_hit_test(&hollow_circle, (70.0, 50.0), 1.0));
        assert!(test_hit_test(&filled_circle, (50.0, 50.0), 1.0));
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
        assert!(!test_hit_test(&rectangle, (10.2, 10.2), zoom));
        assert!(test_hit_test(&rectangle, (10.5, 10.5), zoom));

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
        assert!(test_hit_test(&hollow, (5.0, 50.0), 0.5));
        assert!(!test_hit_test(&hollow, (5.0, 50.0), 4.0));

        let line = Annotation::Line {
            id: "line".into(),
            points: [10.0, 10.0, 90.0, 90.0],
            stroke: "#000000".into(),
            stroke_width: 2.0,
        };
        assert!(test_hit_test(&line, (50.0, 45.0), 0.5));
        assert!(!test_hit_test(&line, (50.0, 45.0), 4.0));
        assert!(!test_hit_test(&line, (10.0, 90.0), 1.0));
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

    #[test]
    fn pen_and_highlight_hit_tests_match_rendered_outline_and_gaps() {
        let points = u_points();
        let pen = pen("pen-u", points.clone());
        let highlight = Annotation::Highlight {
            id: "highlight-u".into(),
            points,
            fill: "#000000".into(),
            opacity: 0.5,
            stroke_width: 12.0,
        };
        let mut pen_canvas = Canvas::new(120, 120).expect("pen canvas");
        draw(&mut pen_canvas, &pen);
        let mut highlight_canvas = Canvas::new(120, 120).expect("highlight canvas");
        highlight_canvas.fill_all(Color::WHITE);
        draw(&mut highlight_canvas, &highlight);
        let pixel = |canvas: &Canvas, x: usize, y: usize| {
            let index = (y * canvas.width() as usize + x) * 4;
            let data = &canvas.pixmap().data()[index..index + 4];
            [data[0], data[1], data[2], data[3]]
        };

        assert!(pixel(&pen_canvas, 50, 10)[3] > 0);
        assert_eq!(pixel(&pen_canvas, 50, 50)[3], 0);
        assert!(test_hit_test(&pen, (50.0, 10.0), 1.0));
        assert!(!test_hit_test(&pen, (50.0, 50.0), 1.0));

        assert_ne!(pixel(&highlight_canvas, 50, 10), [255, 255, 255, 255]);
        assert_eq!(pixel(&highlight_canvas, 50, 50), [255, 255, 255, 255]);
        assert!(test_hit_test(&highlight, (50.0, 10.0), 1.0));
        assert!(!test_hit_test(&highlight, (50.0, 50.0), 1.0));
    }

    #[test]
    fn pen_and_highlight_hit_tests_follow_the_rendered_outline() {
        let points = vec![10.0, 10.0, 90.0, 10.0, 90.0, 90.0, 10.0, 90.0];
        let pen = Annotation::Pen {
            id: "pen-u".into(),
            points: points.clone(),
            stroke: "#ff0000".into(),
            stroke_width: 8.0,
        };
        let highlight = Annotation::Highlight {
            id: "highlight-u".into(),
            points,
            fill: "#ff0000".into(),
            opacity: 0.5,
            stroke_width: 12.0,
        };
        let mut pen_canvas = Canvas::new(120, 120).expect("pen canvas");
        draw(&mut pen_canvas, &pen);
        let mut highlight_canvas = Canvas::new(120, 120).expect("highlight canvas");
        draw(&mut highlight_canvas, &highlight);
        let alpha_at = |canvas: &Canvas, x: usize, y: usize| {
            canvas.pixmap().data()[(y * canvas.width() as usize + x) * 4 + 3]
        };

        assert!(alpha_at(&pen_canvas, 50, 10) > 0);
        assert_eq!(alpha_at(&pen_canvas, 50, 50), 0);
        assert!(test_hit_test(&pen, (50.0, 10.0), 1.0));
        assert!(!test_hit_test(&pen, (50.0, 50.0), 1.0));

        assert!(alpha_at(&highlight_canvas, 50, 10) > 0);
        assert_eq!(alpha_at(&highlight_canvas, 50, 50), 0);
        assert!(test_hit_test(&highlight, (50.0, 10.0), 1.0));
        assert!(!test_hit_test(&highlight, (50.0, 50.0), 1.0));
    }

    #[test]
    fn hit_test_budget_falls_back_to_the_legacy_bounds() {
        let points = (0..=HIT_TEST_RAW_POINT_BUDGET)
            .flat_map(|index| {
                if index % 2 == 0 {
                    [10.0, 10.0]
                } else {
                    [90.0, 90.0]
                }
            })
            .collect::<Vec<_>>();
        let oversized = Annotation::Pen {
            id: "oversized".into(),
            points: points.clone(),
            stroke: "#000000".into(),
            stroke_width: 2.0,
        };
        let mut budget = HitTestBudget::default();
        assert_eq!(
            screenshot_annotation_at_with_budget(&[oversized], (10.0, 90.0), 1.0, &mut budget,)
                .map(Annotation::id),
            Some("oversized")
        );
        assert_eq!(budget.raw_points, 0);
        assert_eq!(budget.outline_vertices, 0);
    }

    #[test]
    fn confirmed_lower_hit_beats_an_indeterminate_top_bbox() {
        let points = (0..=HIT_TEST_RAW_POINT_BUDGET)
            .flat_map(|index| {
                if index % 2 == 0 {
                    [10.0, 10.0]
                } else {
                    [90.0, 90.0]
                }
            })
            .collect::<Vec<_>>();
        let annotations = [
            Annotation::Rectangle {
                id: "lower-fill".into(),
                x: 0.0,
                y: 80.0,
                width: 20.0,
                height: 20.0,
                stroke: "#ff0000".into(),
                stroke_width: 2.0,
                fill: Some("#ff0000".into()),
            },
            pen("over-budget-top", points),
        ];

        assert_eq!(
            screenshot_annotation_at(&annotations, (10.0, 90.0), 1.0).map(Annotation::id),
            Some("lower-fill")
        );
        assert_eq!(
            screenshot_annotation_at(&annotations[1..], (10.0, 90.0), 1.0).map(Annotation::id),
            Some("over-budget-top")
        );
    }

    #[test]
    fn unchanged_annotation_types_keep_their_bbox_hit_priority() {
        let lower = outline_rect("lower", Some("#ff0000"));
        let text = Annotation::Text {
            id: "text".into(),
            x: 30.0,
            y: 30.0,
            text: "MMMM".into(),
            font_size: 20.0,
            fill: "#000000".into(),
            font_family: None,
            background_color: None,
            background_opacity: None,
            background_padding: None,
            background_radius: None,
            rotation: None,
        };
        let (_, _, text_right, text_bottom) = text.bounds();
        let text_point = (text_right - 0.1, text_bottom - 0.1);
        assert_eq!(
            screenshot_annotation_at(&[lower.clone(), text], text_point, 1.0).map(Annotation::id),
            Some("text")
        );

        let number = Annotation::Number {
            id: "number".into(),
            x: 60.0,
            y: 50.0,
            value: 1.0,
            display_value: "1".into(),
            fill: "#000000".into(),
            size: "small".into(),
        };
        assert_eq!(
            screenshot_annotation_at(&[lower.clone(), number], (72.0, 62.0), 1.0)
                .map(Annotation::id),
            Some("number")
        );

        let redact = Annotation::Redact {
            id: "redact".into(),
            x: 40.0,
            y: 40.0,
            width: 30.0,
            height: 30.0,
            style: "blackout".into(),
            intensity: 5.0,
        };
        assert_eq!(
            screenshot_annotation_at(&[lower, redact], (50.0, 50.0), 1.0).map(Annotation::id),
            Some("redact")
        );
    }

    #[test]
    fn hit_test_budget_falls_back_after_exact_outline_budget_is_spent() {
        let mut points = Vec::with_capacity(2_000);
        for step in 0..250 {
            points.extend([10.0 + step as f64 * 80.0 / 249.0, 10.0]);
        }
        for step in 0..250 {
            points.extend([90.0, 10.0 + step as f64 * 80.0 / 249.0]);
        }
        for step in 0..250 {
            points.extend([90.0 - step as f64 * 80.0 / 249.0, 90.0]);
        }
        for step in 0..250 {
            points.extend([10.0, 90.0 - step as f64 * 80.0 / 249.0]);
        }
        let outline_vertices = crate::render::annotations::pen_outline(&points, 8.0).len();
        let annotations = [pen("fallback", points.clone()), pen("exact", points)];
        let mut budget = HitTestBudget {
            raw_points: 0,
            outline_vertices: HIT_TEST_OUTLINE_VERTEX_BUDGET - outline_vertices - 1,
            curve_steps: 0,
        };

        assert_eq!(
            screenshot_annotation_at_with_budget(&annotations, (50.0, 50.0), 1.0, &mut budget)
                .map(Annotation::id),
            Some("fallback")
        );
        assert_eq!(budget.raw_points, 2_000);
        assert_eq!(budget.outline_vertices, HIT_TEST_OUTLINE_VERTEX_BUDGET);
    }

    #[test]
    fn hit_test_budget_is_shared_across_overlapping_pen_annotations() {
        let points = (0..1_500)
            .flat_map(|index| {
                if index % 2 == 0 {
                    [10.0, 10.0]
                } else {
                    [90.0, 90.0]
                }
            })
            .collect::<Vec<_>>();
        let annotation = |id: &str| Annotation::Pen {
            id: id.into(),
            points: points.clone(),
            stroke: "#000000".into(),
            stroke_width: 2.0,
        };
        let annotations = [annotation("fallback"), annotation("exact")];
        let mut budget = HitTestBudget::default();

        assert_eq!(
            screenshot_annotation_at_with_budget(&annotations, (10.0, 90.0), 1.0, &mut budget)
                .map(Annotation::id),
            Some("fallback")
        );
        assert!(budget.raw_points > 0);
        assert!(budget.outline_vertices > 0);
    }

    #[test]
    fn exhausted_curve_budget_falls_back_to_legacy_bounds() {
        let annotation = pen("stroke", u_points());
        let mut budget = HitTestBudget {
            raw_points: 0,
            outline_vertices: 0,
            curve_steps: HIT_TEST_CURVE_STEP_BUDGET,
        };
        assert_eq!(
            hit_test_annotation(&annotation, (50.0, 50.0), 1.0, &mut budget),
            AnnotationHit::Indeterminate
        );
        assert_eq!(budget.curve_steps, HIT_TEST_CURVE_STEP_BUDGET);
    }

    #[test]
    fn adaptive_curve_hit_tracks_an_extreme_rendered_quadratic() {
        let start = (0.0, 0.0);
        let control = (5_000.0, 10_000.0);
        let end = (10_000.0, 0.0);
        let t: f64 = 0.37;
        let one = 1.0 - t;
        let point = (
            one * one * start.0 + 2.0 * one * t * control.0 + t * t * end.0,
            one * one * start.1 + 2.0 * one * t * control.1 + t * t * end.1,
        );
        let mut budget = HitTestBudget::default();
        let mut winding = 0;

        assert_eq!(
            scan_quadratic_outline(
                point,
                start,
                control,
                end,
                1.0,
                4.0,
                HIT_TEST_CURVE_MAX_DEPTH,
                &mut budget,
                &mut winding,
            ),
            Some(true)
        );
        assert!(budget.curve_steps > 1);
        assert!(budget.curve_steps < HIT_TEST_CURVE_STEP_BUDGET);
    }

    #[test]
    fn curved_arrow_and_both_heads_hit_outside_endpoint_bounds() {
        let curved = Annotation::Arrow {
            id: "curved".into(),
            points: [10.0, 10.0, 90.0, 10.0],
            stroke: "#000000".into(),
            stroke_width: 2.0,
            arrow_style: Some("curved".into()),
            bend_offset: None,
        };
        assert!(test_hit_test(&curved, (50.0, 18.0), 1.0));
        let bounds = curved.bounds();
        assert!(18.0 > bounds.3 + 4.0);

        let double = Annotation::Arrow {
            id: "double".into(),
            points: [10.0, 10.0, 90.0, 10.0],
            stroke: "#000000".into(),
            stroke_width: 2.0,
            arrow_style: Some("double".into()),
            bend_offset: None,
        };
        assert!(test_hit_test(&double, (76.1436, 18.0), 1.0));
        assert!(test_hit_test(&double, (23.8564, 18.0), 1.0));
    }

    #[test]
    fn arrow_bbox_gaps_and_non_finite_paths_do_not_hit() {
        let arrow = Annotation::Arrow {
            id: "diagonal".into(),
            points: [10.0, 10.0, 90.0, 90.0],
            stroke: "#000000".into(),
            stroke_width: 2.0,
            arrow_style: Some("standard".into()),
            bend_offset: None,
        };
        assert!(!test_hit_test(&arrow, (10.0, 90.0), 1.0));
        assert!(!arrow_hit_test(
            [f64::MAX, f64::MAX, -f64::MAX, -f64::MAX],
            2.0,
            Some("curved"),
            None,
            (0.0, 0.0),
            1.0,
            &mut HitTestBudget::default(),
        )
        .unwrap_or(false));
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
    fn screenshot_blackout_covers_fractional_boundary_pixels() {
        for (offset_x, offset_y, x, y, width, height, left, right, row) in [
            (0., 0., 8.5, 3.25, 12.3, 12.4, 8, 20, 9),
            (5.25, 4.75, 8.5, 3.25, 12.3, 12.4, 13, 25, 14),
            (0., 0., 20.8, 15.65, -12.3, -12.4, 8, 20, 9),
            (0., 0., -2.5, 3.25, 12.3, 12.4, 0, 9, 9),
            (0., 0., 8.8, 3.8, 12.3, 12.3, 8, 21, 9),
        ] {
            let mut canvas = Canvas::new(40, 32).unwrap();
            canvas.fill_all(Color::WHITE);
            canvas.translate(offset_x, offset_y);
            let blackout = Annotation::Redact {
                id: "blackout".into(),
                x,
                y,
                width,
                height,
                style: "blackout".into(),
                intensity: 5.,
            };
            draw_all_screenshot(&mut canvas, &[blackout], 1.);
            for column in left..=right {
                assert_eq!(
                    canvas.pixmap().pixel(column, row).unwrap().red(),
                    0,
                    "origin ({offset_x}, {offset_y}), rect ({x}, {y}, {width}, {height}), pixel ({column}, {row})"
                );
            }
        }
    }

    #[test]
    fn screenshot_blackout_covers_bottom_edge_without_extending_past_it() {
        let mut canvas = Canvas::new(32, 24).unwrap();
        canvas.fill_all(Color::WHITE);
        draw_all_screenshot(
            &mut canvas,
            &[Annotation::Redact {
                id: "blackout".into(),
                x: 8.8,
                y: 3.8,
                width: 12.3,
                height: 12.3,
                style: "blackout".into(),
                intensity: 5.,
            }],
            1.,
        );
        assert_eq!(canvas.pixmap().pixel(12, 16).unwrap().red(), 0);
        assert_eq!(canvas.pixmap().pixel(12, 17).unwrap().red(), 255);
        assert_eq!(canvas.pixmap().pixel(22, 9).unwrap().red(), 255);
    }

    #[test]
    fn empty_screenshot_blackout_does_not_paint_a_pixel() {
        for (width, height) in [(0., 12.3), (12.3, 0.)] {
            let mut canvas = Canvas::new(32, 24).unwrap();
            canvas.fill_all(Color::WHITE);
            draw_all_screenshot(
                &mut canvas,
                &[Annotation::Redact {
                    id: "empty".into(),
                    x: 8.8,
                    y: 3.8,
                    width,
                    height,
                    style: "blackout".into(),
                    intensity: 5.,
                }],
                1.,
            );
            assert!(canvas
                .pixmap()
                .pixels()
                .iter()
                .all(|pixel| pixel.red() == 255));
        }
    }

    #[test]
    fn screenshot_selection_follows_redaction_and_annotation_order() {
        let shape = Annotation::Rectangle {
            id: "shape".into(),
            x: 4.,
            y: 4.,
            width: 16.,
            height: 16.,
            stroke: "#ff0000".into(),
            stroke_width: 1.,
            fill: Some("#ff0000".into()),
        };
        for style in ["blackout", "pixelate", "blur"] {
            let redact = Annotation::Redact {
                id: "redact".into(),
                x: 4.,
                y: 4.,
                width: 16.,
                height: 16.,
                style: style.into(),
                intensity: 5.,
            };
            assert_eq!(
                screenshot_annotation_at(&[shape.clone(), redact.clone()], (10., 10.), 1.)
                    .map(Annotation::id),
                Some("redact"),
                "{style}"
            );
            assert_eq!(
                screenshot_annotation_at(&[redact, shape.clone()], (10., 10.), 1.)
                    .map(Annotation::id),
                Some("shape"),
                "{style}"
            );
        }
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
    fn video_blur_matches_electron_on_translucent_clipped_regions() {
        for (x, y, width, height, samples) in [
            (
                0.,
                0.,
                128.,
                128.,
                vec![
                    (0, 0, [65, 128, 192, 145]),
                    (0, 64, [65, 128, 192, 161]),
                    (10, 64, [63, 128, 192, 186]),
                    (64, 64, [64, 128, 191, 192]),
                    (127, 64, [65, 128, 192, 161]),
                ],
            ),
            (
                -10.,
                32.,
                50.,
                64.,
                vec![
                    (0, 64, [65, 128, 192, 161]),
                    (10, 64, [63, 128, 192, 186]),
                    (39, 64, [64, 128, 191, 192]),
                    (63, 64, [64, 128, 191, 128]),
                ],
            ),
            (
                96.,
                32.,
                50.,
                64.,
                vec![
                    (80, 64, [64, 128, 191, 128]),
                    (96, 64, [63, 127, 191, 191]),
                    (112, 64, [63, 128, 191, 190]),
                    (127, 64, [65, 128, 192, 161]),
                ],
            ),
        ] {
            let mut canvas = Canvas::new(128, 128).unwrap();
            canvas.fill_all(Color::from_rgba8(64, 128, 192, 128));
            draw(
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
            );
            let rendered = crate::editor::export::to_rgba(canvas.pixmap());
            for (sample_x, sample_y, expected) in samples {
                let actual = rendered.get_pixel(sample_x, sample_y).0;
                assert!(
                    actual.iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 2),
                    "video redaction {x},{y}, pixel {sample_x},{sample_y}: {actual:?} vs {expected:?}"
                );
            }
        }
    }

    #[test]
    fn video_blur_matches_electron_at_opaque_canvas_edges() {
        for (x, stripe_x, samples) in [
            (-10., 0., vec![(0, 211), (10, 72), (31, 0), (39, 0)]),
            (96., 120., vec![(96, 0), (112, 38), (127, 211)]),
        ] {
            let mut canvas = Canvas::new(128, 128).unwrap();
            canvas.fill_all(Color::BLACK);
            canvas.fill_rect(
                Rect::from_xywh(stripe_x, 0., 8., 128.).unwrap(),
                Color::WHITE,
            );
            draw(
                &mut canvas,
                &Annotation::Redact {
                    id: "blur".into(),
                    x,
                    y: 32.,
                    width: 50.,
                    height: 64.,
                    style: "blur".into(),
                    intensity: 2.,
                },
            );
            for (sample_x, expected) in samples {
                let actual = canvas.pixmap().pixel(sample_x, 64).unwrap().red();
                assert!(
                    actual.abs_diff(expected) <= 3,
                    "pixel {sample_x}: {actual} vs {expected}"
                );
            }
        }
    }

    #[test]
    fn video_blur_keeps_its_apparent_size_at_preview_scale() {
        let render = |scale: f32| {
            let side = (128. * scale).round() as u32;
            let mut canvas = Canvas::new(side, side).unwrap();
            canvas.scale(scale, scale);
            canvas.fill_all(Color::BLACK);
            canvas.fill_rect(Rect::from_xywh(0., 0., 8., 128.).unwrap(), Color::WHITE);
            draw(
                &mut canvas,
                &Annotation::Redact {
                    id: "blur".into(),
                    x: -10.,
                    y: 32.,
                    width: 50.,
                    height: 64.,
                    style: "blur".into(),
                    intensity: 2.,
                },
            );
            canvas
        };
        let full = render(1.);
        let half = render(0.5);
        for x in [0, 2, 4, 6, 8, 10, 12, 16, 19] {
            let actual = half.pixmap().pixel(x, 32).unwrap().red();
            let left = full.pixmap().pixel(x * 2, 64).unwrap().red();
            let right = full.pixmap().pixel(x * 2 + 1, 64).unwrap().red();
            let expected = (u16::from(left) + u16::from(right)).div_ceil(2) as u8;
            assert!(
                actual.abs_diff(expected) <= 8,
                "pixel {x}: {actual} vs {expected}"
            );
        }
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
    fn pixelation_weights_partial_source_pixels_by_their_coverage() {
        let mut source = Pixmap::new(3, 1).unwrap();
        source
            .data_mut()
            .copy_from_slice(&[0, 0, 0, 255, 0, 0, 0, 255, 255, 120, 60, 255]);
        let reduced = downsample(&source, 2, 1).unwrap();
        assert_eq!(reduced.data(), &[0, 0, 0, 255, 170, 80, 40, 255]);
    }

    #[test]
    fn pixelation_averages_aligned_blocks_without_changing_channels() {
        let mut source = Pixmap::new(4, 4).unwrap();
        for (index, pixel) in source
            .data_mut()
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .enumerate()
        {
            let x = index % 4;
            let y = index / 4;
            pixel.copy_from_slice(&[(x * 50) as u8, (y * 50) as u8, ((x + y) * 20) as u8, 255]);
        }
        assert_eq!(
            downsample(&source, 2, 2).unwrap().data(),
            &[25, 25, 20, 255, 125, 25, 60, 255, 25, 125, 60, 255, 125, 125, 100, 255]
        );
    }

    #[test]
    fn screenshot_pixelation_keeps_its_block_grid_across_cropped_surfaces() {
        let mut source = Pixmap::new(53, 41).unwrap();
        for (index, pixel) in source
            .data_mut()
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .enumerate()
        {
            let x = index % 53;
            let y = index / 53;
            let alpha = ((x * 11 + y * 7) % 256) as u8;
            *pixel = [alpha / 2, (x as u8).min(alpha), (y as u8).min(alpha), alpha];
        }

        for (x, y, width, height) in [(3.25, 7.5, 42., 27.), (-7.25, -3.5, 43., 37.)] {
            let mut full = Canvas::from_pixmap(source.clone());
            let full_region = clamp_region(&full, x, y, width, height).unwrap();
            let (left, top) = (full_region.device_x, full_region.device_y);
            let right = left + full_region.device_width as i32;
            let bottom = top + full_region.device_height as i32;
            pixelate_screenshot(&mut full, x, y, width, height, 8.);

            for (tile_x, tile_y, tile_width, tile_height) in
                [(0, 0, 29, 25), (15, 11, 28, 27), (27, 21, 26, 20)]
            {
                let rect =
                    tiny_skia::IntRect::from_xywh(tile_x, tile_y, tile_width, tile_height).unwrap();
                let mut tile = Canvas::from_pixmap(source.clone_rect(rect).unwrap());
                tile.translate(-(tile_x as f32), -(tile_y as f32));
                let Some(region) = clamp_region(&tile, x, y, width, height) else {
                    continue;
                };
                pixelate_screenshot_region(&mut tile, region, 8., (left - tile_x, top - tile_y));

                let mut checked = 0;
                for py in top.max(tile_y)..bottom.min(tile_y + tile_height as i32) {
                    for px in left.max(tile_x)..right.min(tile_x + tile_width as i32) {
                        let block_left = left + ((px - left) / 8) * 8;
                        let block_top = top + ((py - top) / 8) * 8;
                        if block_left < tile_x
                            || block_top < tile_y
                            || (block_left + 8).min(right) > tile_x + tile_width as i32
                            || (block_top + 8).min(bottom) > tile_y + tile_height as i32
                        {
                            continue;
                        }
                        assert_eq!(
                            tile.pixmap()
                                .pixel((px - tile_x) as u32, (py - tile_y) as u32),
                            full.pixmap().pixel(px as u32, py as u32),
                            "tile ({tile_x}, {tile_y}), pixel ({px}, {py}), region ({x}, {y})"
                        );
                        checked += 1;
                    }
                }
                assert!(checked > 0);
            }
        }
    }

    #[test]
    fn screenshot_redactions_replay_inside_a_cropped_surface() {
        let mut source = Pixmap::new(240, 200).unwrap();
        for (index, pixel) in source
            .data_mut()
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .enumerate()
        {
            let x = index % 240;
            let y = index / 240;
            let alpha = ((x * 11 + y * 7) % 256) as u8;
            *pixel = [alpha / 2, (x as u8).min(alpha), (y as u8).min(alpha), alpha];
        }
        for (x, y) in [(3.25, 7.5), (-7.25, -3.5)] {
            let annotations = [
                Annotation::Redact {
                    id: "pixelate".into(),
                    x,
                    y,
                    width: 210.,
                    height: 180.,
                    style: "pixelate".into(),
                    intensity: 5.,
                },
                Annotation::Redact {
                    id: "blur".into(),
                    x: x + 3.,
                    y: y + 2.,
                    width: 200.,
                    height: 170.,
                    style: "blur".into(),
                    intensity: 1.,
                },
            ];
            for count in 1..=annotations.len() {
                let mut full = Canvas::from_pixmap(source.clone());
                draw_all_screenshot(&mut full, &annotations[..count], 1.);
                let mut tile = Canvas::from_pixmap(
                    source
                        .clone_rect(tiny_skia::IntRect::from_xywh(45, 30, 140, 130).unwrap())
                        .unwrap(),
                );
                tile.translate(-45., -30.);
                tile.set_screenshot_surface((-45, -30, 195, 170));
                draw_all_screenshot(&mut tile, &annotations[..count], 1.);
                for py in 60..130 {
                    for px in 75..155 {
                        assert_eq!(
                            tile.pixmap().pixel(px - 45, py - 30),
                            full.pixmap().pixel(px, py),
                            "pixel ({px}, {py}), region ({x}, {y}), {count} effects"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn pixelation_averages_high_frequency_stripes_across_block_edges() {
        let mut source = Pixmap::new(30, 1).unwrap();
        for (x, pixel) in source
            .data_mut()
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .enumerate()
        {
            *pixel = [if x % 2 == 0 { 0 } else { 255 }, 0, 0, 255];
        }
        let reduced = downsample(&source, 4, 1).unwrap();
        let red: Vec<_> = reduced
            .data()
            .as_chunks::<4>()
            .0
            .iter()
            .map(|pixel| pixel[0])
            .collect();
        assert_eq!(red, [119, 119, 136, 136]);
    }

    #[test]
    fn pixelation_area_sampling_is_consistent_for_scaled_source_pixels() {
        let mut small = Pixmap::new(3, 3).unwrap();
        let mut large = Pixmap::new(6, 6).unwrap();
        for y in 0..3 {
            for x in 0..3 {
                let color = [(x * 60) as u8, (y * 50) as u8, ((x + y) * 30) as u8, 255];
                let offset = ((y * 3 + x) * 4) as usize;
                small.data_mut()[offset..offset + 4].copy_from_slice(&color);
                for dy in 0..2 {
                    for dx in 0..2 {
                        let offset = (((y * 2 + dy) * 6 + x * 2 + dx) * 4) as usize;
                        large.data_mut()[offset..offset + 4].copy_from_slice(&color);
                    }
                }
            }
        }
        assert_eq!(
            downsample(&small, 2, 2).unwrap().data(),
            downsample(&large, 2, 2).unwrap().data()
        );
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
                    8..=14 => 19,
                    15..=22 => 30,
                    _ => 39,
                };
                assert_eq!(canvas.pixmap().data()[(x + i) * 4], expected, "{x}+{i}");
            }
        }
    }

    #[test]
    fn pixelation_preserves_nearest_block_samples_for_varied_sizes() {
        for (width, height, block) in [(7, 5, 2), (30, 11, 8), (47, 23, 6), (64, 32, 8)] {
            let mut source = Pixmap::new(width, height).unwrap();
            for (index, pixel) in source
                .data_mut()
                .as_chunks_mut::<4>()
                .0
                .iter_mut()
                .enumerate()
            {
                let x = index % width as usize;
                let y = index / width as usize;
                pixel.copy_from_slice(&[
                    (x * 13 % 256) as u8,
                    (y * 31 % 256) as u8,
                    ((x + y) * 7 % 256) as u8,
                    255,
                ]);
            }
            let small_width = (width as f32 / block as f32).round().max(1.0) as u32;
            let small_height = (height as f32 / block as f32).round().max(1.0) as u32;
            let small = downsample(&source, small_width, small_height).unwrap();
            let mut canvas = Canvas::new(width + 4, height + 4).unwrap();
            canvas.draw_pixmap(source.as_ref(), 2.0, 2.0, width as f32, height as f32);
            pixelate_region(
                &mut canvas,
                2.0,
                2.0,
                width as f64,
                height as f64,
                block as f64,
            );
            for y in 0..height {
                for x in 0..width {
                    let sx = (((2 * x + 1) * small_width - 1) / (2 * width)) as usize;
                    let sy = (((2 * y + 1) * small_height - 1) / (2 * height)) as usize;
                    let expected = (sy * small_width as usize + sx) * 4;
                    let actual = (((y + 2) * (width + 4) + x + 2) * 4) as usize;
                    assert_eq!(
                        &canvas.pixmap().data()[actual..actual + 4],
                        &small.data()[expected..expected + 4],
                        "{width}x{height}, {x},{y}"
                    );
                }
            }
        }
    }

    #[test]
    fn fractional_pixelation_covers_the_last_partially_selected_pixel() {
        let mut canvas = Canvas::new(12, 12).unwrap();
        canvas.fill_all(Color::from_rgba8(0, 0, 0, 255));
        canvas.fill_rect(
            Rect::from_xywh(9., 9., 1., 1.).unwrap(),
            Color::from_rgba8(255, 255, 255, 255),
        );

        pixelate_region(&mut canvas, 1.5, 1.5, 8., 8., 8.);

        let offset = (9 * 12 + 9) * 4;
        assert!(canvas.pixmap().data()[offset] < 255);
    }

    #[test]
    fn fractional_blur_covers_the_last_partially_selected_pixel() {
        let mut canvas = Canvas::new(12, 12).unwrap();
        canvas.fill_all(Color::from_rgba8(0, 0, 0, 255));
        canvas.fill_rect(
            Rect::from_xywh(9., 9., 1., 1.).unwrap(),
            Color::from_rgba8(255, 255, 255, 255),
        );

        blur_region(&mut canvas, 1.5, 1.5, 8., 8., 2., false);

        let offset = (9 * 12 + 9) * 4;
        assert!(canvas.pixmap().data()[offset] < 255);
    }

    #[test]
    fn fractional_redaction_regions_clip_after_scaling() {
        let mut canvas = Canvas::new(100, 100).unwrap();
        for (x, y, width, height, expected) in [
            (10.5, 20.5, 8., 8., (10, 20, 9, 9)),
            (-0.5, -0.5, 8., 8., (0, 0, 8, 8)),
            (95.5, 96.5, 8., 8., (95, 96, 5, 4)),
        ] {
            let region = clamp_region(&canvas, x, y, width, height).unwrap();
            assert_eq!(
                (
                    region.device_x,
                    region.device_y,
                    region.device_width,
                    region.device_height
                ),
                expected
            );
        }
        canvas.scale(2., 2.);
        let region = clamp_region(&canvas, 10.25, 20.25, 8., 8.).unwrap();
        assert_eq!(
            (
                region.device_x,
                region.device_y,
                region.device_width,
                region.device_height
            ),
            (20, 40, 17, 17)
        );
        assert!(clamp_region(&canvas, 10.25, 20.25, 0., 8.).is_none());
        assert!(clamp_region(&canvas, 10.25, 20.25, 8., 0.).is_none());
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
