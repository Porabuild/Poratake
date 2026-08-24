//! Annotation model — a 1:1 port of the `Annotation` union in
//! `types/editor.ts`, including its wire shape. Video drawing segments persist
//! annotations inside `state.json`, so this has to serialize exactly what the
//! Electron shell writes and reads: flat `points` arrays, a centre-and-radius
//! circle, and signed rectangle extents.

use serde::{Deserialize, Serialize};

/// A point in image coordinates. The wire format stores pairs in a flat
/// `points` array; this is the shape the editor's input handling works in.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

/// `backgroundPadding` in `TextAnnotation`.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Offset {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Annotation {
    #[serde(rename_all = "camelCase")]
    Pen {
        id: String,
        points: Vec<f64>,
        stroke: String,
        stroke_width: f64,
    },
    #[serde(rename_all = "camelCase")]
    Highlight {
        id: String,
        points: Vec<f64>,
        fill: String,
        opacity: f64,
        stroke_width: f64,
    },
    #[serde(rename_all = "camelCase")]
    Rectangle {
        id: String,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        stroke: String,
        stroke_width: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fill: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    Circle {
        id: String,
        x: f64,
        y: f64,
        radius: f64,
        stroke: String,
        stroke_width: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fill: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    Line {
        id: String,
        points: [f64; 4],
        stroke: String,
        stroke_width: f64,
    },
    #[serde(rename_all = "camelCase")]
    Arrow {
        id: String,
        points: [f64; 4],
        stroke: String,
        stroke_width: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        arrow_style: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bend_offset: Option<Offset>,
    },
    #[serde(rename_all = "camelCase")]
    Text {
        id: String,
        x: f64,
        y: f64,
        text: String,
        font_size: f64,
        fill: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        font_family: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        background_color: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        background_opacity: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        background_padding: Option<Offset>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        background_radius: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rotation: Option<f64>,
    },
    #[serde(rename_all = "camelCase")]
    Number {
        id: String,
        x: f64,
        y: f64,
        value: f64,
        display_value: String,
        fill: String,
        size: String,
    },
    #[serde(rename_all = "camelCase")]
    Redact {
        id: String,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        style: String,
        intensity: f64,
    },
}

/// `TEXT_BG_*` in `renderer/components/editor/text/text-utils.ts`.
pub const TEXT_BG_COLOR: &str = "rgba(0, 0, 0, 0.75)";
pub const TEXT_BG_PADDING_X: f64 = 8.0;
pub const TEXT_BG_PADDING_Y: f64 = 4.0;
pub const TEXT_BG_RADIUS: f64 = 4.0;
/// The family the renderer resolves for an unset `fontFamily`.
pub const DEFAULT_TEXT_FONT: &str = "sans";

/// `NUMBER_SIZE_CONFIG` in `renderer/utils/annotation-geometry.ts`.
pub fn number_size_config(size: &str) -> (f64, f64) {
    match size {
        "small" => (14.0, 14.0),
        "large" => (24.0, 24.0),
        _ => (18.0, 18.0),
    }
}

/// `REDACT_INTENSITY_MAP` in `renderer/utils/redact.ts`, returning
/// `(pixelSize, blurRadius)`.
pub fn redact_intensity(intensity: f64) -> (f64, f64) {
    match intensity.round().clamp(1.0, 10.0) as u32 {
        1 => (2.0, 4.0),
        2 => (3.0, 8.0),
        3 => (4.0, 12.0),
        4 => (6.0, 16.0),
        6 => (10.0, 24.0),
        7 => (12.0, 30.0),
        8 => (14.0, 36.0),
        9 => (16.0, 44.0),
        10 => (20.0, 52.0),
        _ => (8.0, 20.0),
    }
}

/// `normalizeNegativeRect` — rectangles and redactions keep the signed extent
/// they were dragged with, and are normalized at render time.
pub fn normalize_rect(x: f64, y: f64, width: f64, height: f64) -> (f64, f64, f64, f64) {
    (
        if width < 0.0 { x + width } else { x },
        if height < 0.0 { y + height } else { y },
        width.abs(),
        height.abs(),
    )
}

/// `pointsToCoordinates` — the flat wire array as pairs.
pub fn points_to_coordinates(points: &[f64]) -> Vec<(f64, f64)> {
    points
        .chunks_exact(2)
        .map(|pair| (pair[0], pair[1]))
        .collect()
}

const ARROW_HEAD_ANGLE: f64 = std::f64::consts::PI / 6.0;
const ARROW_CURVE_RATIO: f64 = 0.2;
const ARROW_BEND_THRESHOLD: f64 = 1.0;

/// `arrowHeadSize` in `renderer/utils/annotation-geometry.ts`.
pub fn arrow_head_size(stroke_width: f64) -> f64 {
    (stroke_width * 5.0).max(16.0)
}

/// `hasArrowBend`.
pub fn has_arrow_bend(bend: Option<Offset>) -> bool {
    bend.is_some_and(|offset| {
        offset.x.abs() > ARROW_BEND_THRESHOLD || offset.y.abs() > ARROW_BEND_THRESHOLD
    })
}

/// `arrowHeadPoints`, returning the two wing endpoints.
pub fn arrow_head_points(
    tip_x: f64,
    tip_y: f64,
    angle: f64,
    head_size: f64,
) -> ((f64, f64), (f64, f64)) {
    (
        (
            tip_x - head_size * (angle - ARROW_HEAD_ANGLE).cos(),
            tip_y - head_size * (angle - ARROW_HEAD_ANGLE).sin(),
        ),
        (
            tip_x - head_size * (angle + ARROW_HEAD_ANGLE).cos(),
            tip_y - head_size * (angle + ARROW_HEAD_ANGLE).sin(),
        ),
    )
}

/// `curvedControlPoint`.
pub fn curved_control_point(x1: f64, y1: f64, x2: f64, y2: f64) -> (f64, f64) {
    let distance = ((x2 - x1).powi(2) + (y2 - y1).powi(2)).sqrt();
    let offset = distance * ARROW_CURVE_RATIO;
    let divisor = if distance == 0.0 { 1.0 } else { distance };
    let perpendicular_x = -(y2 - y1) / divisor;
    let perpendicular_y = (x2 - x1) / divisor;
    (
        (x1 + x2) / 2.0 + perpendicular_x * offset,
        (y1 + y2) / 2.0 + perpendicular_y * offset,
    )
}

impl Annotation {
    pub fn id(&self) -> &str {
        match self {
            Self::Pen { id, .. }
            | Self::Rectangle { id, .. }
            | Self::Circle { id, .. }
            | Self::Line { id, .. }
            | Self::Arrow { id, .. }
            | Self::Highlight { id, .. }
            | Self::Number { id, .. }
            | Self::Redact { id, .. }
            | Self::Text { id, .. } => id,
        }
    }

    /// The tool name, matching the `type` discriminant on the wire.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Pen { .. } => "pen",
            Self::Highlight { .. } => "highlight",
            Self::Rectangle { .. } => "rectangle",
            Self::Circle { .. } => "circle",
            Self::Line { .. } => "line",
            Self::Arrow { .. } => "arrow",
            Self::Text { .. } => "text",
            Self::Number { .. } => "number",
            Self::Redact { .. } => "redact",
        }
    }

    /// Bounding box in image coordinates, as `(left, top, right, bottom)`.
    pub fn bounds(&self) -> (f64, f64, f64, f64) {
        match self {
            Self::Pen { points, .. } | Self::Highlight { points, .. } => {
                let coordinates = points_to_coordinates(points);
                let Some(first) = coordinates.first().copied() else {
                    return (0.0, 0.0, 0.0, 0.0);
                };
                let mut min = first;
                let mut max = first;
                for (x, y) in coordinates {
                    min.0 = min.0.min(x);
                    min.1 = min.1.min(y);
                    max.0 = max.0.max(x);
                    max.1 = max.1.max(y);
                }
                (min.0, min.1, max.0, max.1)
            }
            Self::Rectangle {
                x,
                y,
                width,
                height,
                ..
            }
            | Self::Redact {
                x,
                y,
                width,
                height,
                ..
            } => {
                let (left, top, width, height) = normalize_rect(*x, *y, *width, *height);
                (left, top, left + width, top + height)
            }
            Self::Circle { x, y, radius, .. } => (x - radius, y - radius, x + radius, y + radius),
            Self::Number { x, y, size, .. } => {
                let (radius, _) = number_size_config(size);
                (x - radius, y - radius, x + radius, y + radius)
            }
            Self::Text {
                x,
                y,
                text,
                font_size,
                font_family,
                ..
            } => {
                let family = font_family.as_deref().unwrap_or(DEFAULT_TEXT_FONT);
                let width = crate::editor::text_render::measure(text, family, *font_size as f32)
                    .map(|metrics| metrics.width as f64)
                    .unwrap_or(text.chars().count() as f64 * font_size * 0.55);
                (*x, *y, x + width, y + font_size)
            }
            Self::Line { points, .. } | Self::Arrow { points, .. } => (
                points[0].min(points[2]),
                points[1].min(points[3]),
                points[0].max(points[2]),
                points[1].max(points[3]),
            ),
        }
    }

    /// Translates the annotation by a delta — used by crop adjustment.
    pub fn translate(&mut self, dx: f64, dy: f64) {
        match self {
            Self::Pen { points, .. } | Self::Highlight { points, .. } => {
                for (index, value) in points.iter_mut().enumerate() {
                    *value += if index % 2 == 0 { dx } else { dy };
                }
            }
            Self::Rectangle { x, y, .. }
            | Self::Circle { x, y, .. }
            | Self::Redact { x, y, .. }
            | Self::Number { x, y, .. }
            | Self::Text { x, y, .. } => {
                *x += dx;
                *y += dy;
            }
            Self::Line { points, .. } | Self::Arrow { points, .. } => {
                points[0] += dx;
                points[1] += dy;
                points[2] += dx;
                points[3] += dy;
            }
        }
    }

    /// Appends a sampled point to a freehand annotation.
    pub fn push_point(&mut self, point: Point) {
        if let Self::Pen { points, .. } | Self::Highlight { points, .. } = self {
            points.push(point.x as f64);
            points.push(point.y as f64);
        }
    }
}

/// Undo/redo history over the annotation list — port of `useHistory`.
#[derive(Default)]
pub struct AnnotationHistory {
    stack: Vec<Vec<Annotation>>,
    index: usize,
}

impl AnnotationHistory {
    pub fn new(initial: Vec<Annotation>) -> Self {
        Self {
            stack: vec![initial],
            index: 0,
        }
    }

    pub fn current(&self) -> &[Annotation] {
        &self.stack[self.index]
    }

    pub fn push(&mut self, next: Vec<Annotation>) {
        self.stack.truncate(self.index + 1);
        self.stack.push(next);
        self.index += 1;
    }

    /// Replaces the current revision, for a gesture that has already pushed
    /// its entry and is now refining it.
    pub fn replace_current(&mut self, next: Vec<Annotation>) {
        self.stack[self.index] = next;
    }

    pub fn can_undo(&self) -> bool {
        self.index > 0
    }

    pub fn can_redo(&self) -> bool {
        self.index + 1 < self.len()
    }

    pub fn len(&self) -> usize {
        self.stack.len()
    }

    pub fn undo(&mut self) -> bool {
        if !self.can_undo() {
            return false;
        }
        self.index -= 1;
        true
    }

    pub fn redo(&mut self) -> bool {
        if !self.can_redo() {
            return false;
        }
        self.index += 1;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_the_shape_the_electron_shell_persists() {
        let pen = Annotation::Pen {
            id: "pen-1".into(),
            points: vec![1.0, 2.0, 3.0, 4.0],
            stroke: "#FF3B30".into(),
            stroke_width: 4.0,
        };
        let json = serde_json::to_value(&pen).expect("pen");
        assert_eq!(json["type"], "pen");
        assert_eq!(json["strokeWidth"], 4.0);
        assert_eq!(json["points"], serde_json::json!([1.0, 2.0, 3.0, 4.0]));

        let circle = Annotation::Circle {
            id: "circle-1".into(),
            x: 10.0,
            y: 12.0,
            radius: 5.0,
            stroke: "#fff".into(),
            stroke_width: 2.0,
            fill: None,
        };
        let json = serde_json::to_value(&circle).expect("circle");
        assert_eq!(json["radius"], 5.0);
        assert!(json.get("fill").is_none());
    }

    #[test]
    fn round_trips_an_arrow_with_a_bend() {
        let arrow = Annotation::Arrow {
            id: "arrow-1".into(),
            points: [0.0, 0.0, 10.0, 10.0],
            stroke: "#000".into(),
            stroke_width: 3.0,
            arrow_style: Some("curved".into()),
            bend_offset: Some(Offset { x: 4.0, y: -2.0 }),
        };
        let json = serde_json::to_string(&arrow).expect("arrow");
        assert!(json.contains("\"arrowStyle\":\"curved\""));
        assert!(json.contains("\"bendOffset\""));
        let parsed: Annotation = serde_json::from_str(&json).expect("parse");
        assert_eq!(parsed, arrow);
    }

    #[test]
    fn parses_a_text_annotation_written_by_the_renderer() {
        let json = r##"{
            "id": "text-1",
            "type": "text",
            "x": 4,
            "y": 8,
            "text": "hello",
            "fontSize": 28,
            "fill": "#ffffff",
            "backgroundColor": "rgba(0, 0, 0, 0.75)",
            "backgroundPadding": { "x": 8, "y": 4 },
            "backgroundRadius": 4
        }"##;
        let parsed: Annotation = serde_json::from_str(json).expect("text");
        let Annotation::Text {
            background_color,
            background_padding,
            ..
        } = &parsed
        else {
            panic!("expected text");
        };
        assert_eq!(background_color.as_deref(), Some(TEXT_BG_COLOR));
        assert_eq!(background_padding.map(|padding| padding.x), Some(8.0));
    }

    #[test]
    fn normalizes_negative_extents() {
        assert_eq!(normalize_rect(10.0, 10.0, -4.0, -6.0), (6.0, 4.0, 4.0, 6.0));
        assert_eq!(normalize_rect(1.0, 2.0, 3.0, 4.0), (1.0, 2.0, 3.0, 4.0));
    }

    #[test]
    fn redaction_intensity_matches_the_renderer_table() {
        assert_eq!(redact_intensity(1.0), (2.0, 4.0));
        assert_eq!(redact_intensity(5.0), (8.0, 20.0));
        assert_eq!(redact_intensity(10.0), (20.0, 52.0));
        assert_eq!(redact_intensity(99.0), (20.0, 52.0));
    }

    #[test]
    fn a_bend_under_a_pixel_is_not_a_bend() {
        assert!(!has_arrow_bend(Some(Offset { x: 0.5, y: -0.5 })));
        assert!(has_arrow_bend(Some(Offset { x: 2.0, y: 0.0 })));
        assert!(!has_arrow_bend(None));
    }

    #[test]
    fn bounds_cover_every_variant() {
        let pen = Annotation::Pen {
            id: "p".into(),
            points: vec![0.0, 0.0, 10.0, 20.0],
            stroke: "#000".into(),
            stroke_width: 2.0,
        };
        assert_eq!(pen.bounds(), (0.0, 0.0, 10.0, 20.0));

        let redact = Annotation::Redact {
            id: "r".into(),
            x: 10.0,
            y: 10.0,
            width: -4.0,
            height: 6.0,
            style: "blur".into(),
            intensity: 5.0,
        };
        assert_eq!(redact.bounds(), (6.0, 10.0, 10.0, 16.0));
    }

    #[test]
    fn translating_shifts_flat_point_arrays_by_axis() {
        let mut pen = Annotation::Pen {
            id: "p".into(),
            points: vec![0.0, 0.0, 10.0, 20.0],
            stroke: "#000".into(),
            stroke_width: 2.0,
        };
        pen.translate(2.0, -3.0);
        let Annotation::Pen { points, .. } = &pen else {
            panic!("expected pen");
        };
        assert_eq!(points, &vec![2.0, -3.0, 12.0, 17.0]);
    }
}
