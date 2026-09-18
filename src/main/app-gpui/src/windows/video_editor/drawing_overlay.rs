use std::cell::RefCell;
use std::rc::Rc;

use gpui::{canvas, div, prelude::*, AnyElement, Bounds, Context, MouseDownEvent, Pixels};
use herogpui::gpui;

use crate::editor::annotations::{
    Annotation, Offset, Point, TEXT_BG_COLOR, TEXT_BG_PADDING_X, TEXT_BG_PADDING_Y, TEXT_BG_RADIUS,
};
use crate::editor::options::{number_display_value, EditorOption};
use crate::windows::video_editor::model::{DrawingSegment, VideoEditorState};
use crate::windows::video_editor::styles::DrawingToolSettings;
use crate::windows::video_editor::timeline::edit::MIN_DRAWING_SEGMENT_DURATION;
use crate::windows::video_editor::{drawing_paint, VideoEditorWindow};

pub const DEFAULT_SEGMENT_DURATION: f64 = 3.0;
const MIN_DRAW_DISTANCE: f64 = 5.0;
const HIGHLIGHT_STROKE_WIDTH: f64 = 20.0;

pub type BoundsCell = Rc<RefCell<Option<Bounds<Pixels>>>>;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub fn from_bounds(bounds: Bounds<Pixels>) -> Self {
        Self {
            x: f32::from(bounds.origin.x),
            y: f32::from(bounds.origin.y),
            width: f32::from(bounds.size.width),
            height: f32::from(bounds.size.height),
        }
    }

    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && y >= self.y && x <= self.x + self.width && y <= self.y + self.height
    }
}

pub fn contain_rect(container: Rect, composition: (f64, f64)) -> Option<Rect> {
    if container.width <= 0.0
        || container.height <= 0.0
        || composition.0 <= 0.0
        || composition.1 <= 0.0
    {
        return None;
    }
    let image_ratio = (composition.0 / composition.1) as f32;
    let (width, height) = if container.width / container.height > image_ratio {
        (container.height * image_ratio, container.height)
    } else {
        (container.width, container.width / image_ratio)
    };
    Some(Rect {
        x: container.x + (container.width - width) / 2.0,
        y: container.y + (container.height - height) / 2.0,
        width,
        height,
    })
}

pub fn to_composition(
    position: (f32, f32),
    content: Rect,
    composition: (f64, f64),
) -> Option<Point> {
    if content.width <= 0.0 || content.height <= 0.0 {
        return None;
    }
    Some(Point {
        x: (position.0 - content.x) / content.width * composition.0 as f32,
        y: (position.1 - content.y) / content.height * composition.1 as f32,
    })
}

pub struct Stroke {
    start: Point,
    id: String,
    draft: Option<Annotation>,
}

impl Stroke {
    pub fn begin(tools: &DrawingToolSettings, point: Point, id: String) -> Option<Self> {
        let deferred = matches!(tools.active_tool.as_str(), "line" | "arrow");
        let draft = create(tools, point, id.clone())?;
        Some(Self {
            start: point,
            id,
            draft: (!deferred).then_some(draft),
        })
    }

    pub fn draft(&self) -> Option<&Annotation> {
        self.draft.as_ref()
    }

    pub fn finish(self) -> Option<Annotation> {
        self.draft
    }

    pub fn update(&mut self, tools: &DrawingToolSettings, point: Point, shift: bool) {
        if self.draft.is_none() {
            let dx = f64::from(point.x - self.start.x);
            let dy = f64::from(point.y - self.start.y);
            if (dx * dx + dy * dy).sqrt() < MIN_DRAW_DISTANCE {
                return;
            }
            self.draft = create(tools, self.start, self.id.clone());
        }
        let start = self.start;
        let Some(draft) = self.draft.as_mut() else {
            return;
        };
        match draft {
            Annotation::Pen { .. } | Annotation::Highlight { .. } => {
                if shift {
                    draft.constrain_to_axis(point);
                    return;
                }
                draft.push_point(point);
            }
            Annotation::Rectangle {
                x,
                y,
                width,
                height,
                ..
            }
            | Annotation::Redact {
                x,
                y,
                width,
                height,
                ..
            } => {
                *width = f64::from(point.x) - *x;
                *height = f64::from(point.y) - *y;
            }
            Annotation::Circle { x, y, radius, .. } => {
                let dx = f64::from(point.x - start.x);
                let dy = f64::from(point.y - start.y);
                *x = f64::from(start.x + point.x) / 2.0;
                *y = f64::from(start.y + point.y) / 2.0;
                *radius = (dx * dx + dy * dy).sqrt() / 2.0;
            }
            Annotation::Line { points, .. } | Annotation::Arrow { points, .. } => {
                points[2] = f64::from(point.x);
                points[3] = f64::from(point.y);
            }
            Annotation::Text { .. } | Annotation::Number { .. } => {}
        }
    }
}

fn create(tools: &DrawingToolSettings, point: Point, id: String) -> Option<Annotation> {
    let (x, y) = (f64::from(point.x), f64::from(point.y));
    let stroke = tools.selected_color.clone();
    let stroke_width = tools.stroke_width;
    let fill = (tools.shape_fill_mode == "filled").then(|| stroke.clone());
    match tools.active_tool.as_str() {
        "pen" => Some(Annotation::Pen {
            id,
            points: vec![x, y],
            stroke,
            stroke_width,
        }),
        "highlight" => Some(Annotation::Highlight {
            id,
            points: vec![x, y],
            fill: tools.highlight_color.clone(),
            opacity: tools.highlight_opacity,
            stroke_width: HIGHLIGHT_STROKE_WIDTH,
        }),
        "rectangle" => Some(Annotation::Rectangle {
            id,
            x,
            y,
            width: 0.0,
            height: 0.0,
            stroke,
            stroke_width,
            fill,
        }),
        "circle" => Some(Annotation::Circle {
            id,
            x,
            y,
            radius: 0.0,
            stroke,
            stroke_width,
            fill,
        }),
        "line" => Some(Annotation::Line {
            id,
            points: [x, y, x, y],
            stroke,
            stroke_width,
        }),
        "arrow" => Some(Annotation::Arrow {
            id,
            points: [x, y, x, y],
            stroke,
            stroke_width,
            arrow_style: Some(tools.arrow_style.clone()),
            bend_offset: None,
        }),
        "redact" => Some(Annotation::Redact {
            id,
            x,
            y,
            width: 0.0,
            height: 0.0,
            style: tools.redact_style.clone(),
            intensity: tools.redact_intensity,
        }),
        _ => None,
    }
}

pub fn place(
    tools: &DrawingToolSettings,
    point: Point,
    id: String,
    number_value: f64,
) -> Option<Annotation> {
    let (x, y) = (f64::from(point.x), f64::from(point.y));
    match tools.active_tool.as_str() {
        "text" => Some(Annotation::Text {
            id,
            x,
            y,
            text: "Text".to_string(),
            font_size: tools.text_font_size,
            fill: tools.selected_color.clone(),
            font_family: Some(tools.text_font_family.clone()),
            background_color: tools.text_background.then(|| TEXT_BG_COLOR.to_string()),
            background_opacity: None,
            background_padding: tools.text_background.then_some(Offset {
                x: TEXT_BG_PADDING_X,
                y: TEXT_BG_PADDING_Y,
            }),
            background_radius: tools.text_background.then_some(TEXT_BG_RADIUS),
            rotation: None,
        }),
        "number" => Some(Annotation::Number {
            id,
            x,
            y,
            value: number_value,
            display_value: number_display_value(number_value, &tools.number_style),
            fill: tools.selected_color.clone(),
            size: tools.number_size.clone(),
        }),
        _ => None,
    }
}

pub fn next_number_value(state: &VideoEditorState, tools: &DrawingToolSettings) -> f64 {
    let placed = state
        .drawing_segments
        .iter()
        .flat_map(|segment| &segment.annotations)
        .filter(|annotation| matches!(annotation, Annotation::Number { .. }))
        .count();
    tools.number_start_value + placed as f64
}

pub fn segment_times(timeline_position: f64, total_duration: f64) -> Option<(f64, f64)> {
    if total_duration <= 0.0 {
        return None;
    }
    let duration = DEFAULT_SEGMENT_DURATION.min(total_duration);
    let max_start = (total_duration - duration).max(0.0);
    let start = timeline_position.clamp(0.0, max_start);
    let end = (start + duration).min(total_duration);
    (end - start >= MIN_DRAWING_SEGMENT_DURATION).then_some((start, end))
}

pub fn attach(
    state: &mut VideoEditorState,
    selected: Option<&str>,
    annotation: Annotation,
    timeline_position: f64,
    total_duration: f64,
    composition: (f64, f64),
    segment_id: &str,
) -> Option<String> {
    if let Some(segment) = selected.and_then(|id| {
        state
            .drawing_segments
            .iter_mut()
            .find(|segment| segment.id == id)
    }) {
        if segment.annotations.is_empty()
            && timeline_position >= segment.start_time
            && timeline_position <= segment.end_time
        {
            segment.canvas_width = composition.0;
            segment.canvas_height = composition.1;
            segment.annotations.push(annotation);
            return Some(segment.id.clone());
        }
    }

    let (start_time, end_time) = segment_times(timeline_position, total_duration)?;
    state.drawing_segments.push(DrawingSegment {
        id: segment_id.to_string(),
        start_time,
        end_time,
        canvas_width: composition.0,
        canvas_height: composition.1,
        annotations: vec![annotation],
    });
    Some(segment_id.to_string())
}

pub struct DisplayedStyle {
    pub config_type: String,
    pub color: String,
    pub stroke_width: f64,
    pub arrow_style: String,
    pub highlight_opacity: f64,
    pub shape_fill_mode: String,
    pub number_size: String,
    pub text_font_size: f64,
    pub text_background: bool,
    pub redact_style: String,
    pub redact_intensity: f64,
}

pub fn displayed_style(
    annotation: Option<&Annotation>,
    tools: &DrawingToolSettings,
) -> DisplayedStyle {
    let mut style = DisplayedStyle {
        config_type: annotation
            .map(|annotation| annotation.kind().to_string())
            .unwrap_or_else(|| tools.active_tool.clone()),
        color: if annotation.is_none() && tools.active_tool == "highlight" {
            tools.highlight_color.clone()
        } else {
            tools.selected_color.clone()
        },
        stroke_width: tools.stroke_width,
        arrow_style: tools.arrow_style.clone(),
        highlight_opacity: tools.highlight_opacity,
        shape_fill_mode: tools.shape_fill_mode.clone(),
        number_size: tools.number_size.clone(),
        text_font_size: tools.text_font_size,
        text_background: tools.text_background,
        redact_style: tools.redact_style.clone(),
        redact_intensity: tools.redact_intensity,
    };
    let Some(annotation) = annotation else {
        return style;
    };
    match annotation {
        Annotation::Pen {
            stroke,
            stroke_width,
            ..
        }
        | Annotation::Line {
            stroke,
            stroke_width,
            ..
        } => {
            style.color = stroke.clone();
            style.stroke_width = *stroke_width;
        }
        Annotation::Arrow {
            stroke,
            stroke_width,
            arrow_style,
            ..
        } => {
            style.color = stroke.clone();
            style.stroke_width = *stroke_width;
            style.arrow_style = arrow_style.clone().unwrap_or(style.arrow_style);
        }
        Annotation::Rectangle {
            stroke,
            stroke_width,
            fill,
            ..
        }
        | Annotation::Circle {
            stroke,
            stroke_width,
            fill,
            ..
        } => {
            style.color = stroke.clone();
            style.stroke_width = *stroke_width;
            style.shape_fill_mode = if fill.is_some() { "filled" } else { "outline" }.to_string();
        }
        Annotation::Highlight {
            fill,
            opacity,
            stroke_width,
            ..
        } => {
            style.color = fill.clone();
            style.stroke_width = *stroke_width;
            style.highlight_opacity = *opacity;
        }
        Annotation::Text {
            fill,
            font_size,
            background_color,
            ..
        } => {
            style.color = fill.clone();
            style.text_font_size = *font_size;
            style.text_background = background_color.is_some();
        }
        Annotation::Number { fill, size, .. } => {
            style.color = fill.clone();
            style.number_size = size.clone();
        }
        Annotation::Redact {
            style: redact_style,
            intensity,
            ..
        } => {
            style.redact_style = redact_style.clone();
            style.redact_intensity = *intensity;
        }
    }
    style
}

pub fn apply_option(annotation: &mut Annotation, option: &EditorOption) -> bool {
    if let (EditorOption::HighlightOpacity(value), Annotation::Highlight { opacity, .. }) =
        (option, &mut *annotation)
    {
        *opacity = *value;
        return true;
    }
    annotation.apply_option(option)
}

pub fn render(view: &VideoEditorWindow, cx: &mut Context<VideoEditorWindow>) -> Option<AnyElement> {
    let composition = view.composition_size?;
    let bounds = view.preview_bounds.clone();
    let draft = view
        .drawing_stroke
        .as_ref()
        .and_then(Stroke::draft)
        .cloned();
    let interactive = view.drawing_tools.active_tool != "select";

    let surface = canvas(
        move |bounds_taken, _window, _cx| {
            *bounds.borrow_mut() = Some(bounds_taken);
        },
        move |bounds_taken, (), window, _cx| {
            let Some(draft) = &draft else {
                return;
            };
            let Some(content) = contain_rect(Rect::from_bounds(bounds_taken), composition) else {
                return;
            };
            drawing_paint::draw(window, draft, content, content.width / composition.0 as f32);
        },
    )
    .absolute()
    .inset_0();

    let mut overlay = div().absolute().inset_0().child(surface);
    if interactive {
        overlay = overlay.cursor(gpui::CursorStyle::Crosshair).on_mouse_down(
            gpui::MouseButton::Left,
            cx.listener(|this, event: &MouseDownEvent, window, cx| {
                this.begin_drawing_stroke(event.position, window, cx);
                cx.stop_propagation();
            }),
        );
    }
    Some(overlay.into_any_element())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::video::composition::drawing;
    use crate::windows::video_editor::styles::DrawingToolSettings;

    fn tools(tool: &str) -> DrawingToolSettings {
        DrawingToolSettings {
            active_tool: tool.to_string(),
            ..DrawingToolSettings::default()
        }
    }

    #[test]
    fn a_preview_wider_than_the_composition_is_letterboxed_on_the_sides() {
        let container = Rect {
            x: 0.0,
            y: 0.0,
            width: 1000.0,
            height: 400.0,
        };
        let content = contain_rect(container, (1920.0, 1080.0)).expect("content");
        assert_eq!(content.height, 400.0);
        assert!((content.width - 400.0 * 16.0 / 9.0).abs() < 0.01);
        assert!((content.x - (1000.0 - content.width) / 2.0).abs() < 0.01);
        assert_eq!(content.y, 0.0);
    }

    #[test]
    fn a_preview_taller_than_the_composition_is_letterboxed_top_and_bottom() {
        let container = Rect {
            x: 10.0,
            y: 20.0,
            width: 640.0,
            height: 800.0,
        };
        let content = contain_rect(container, (1920.0, 1080.0)).expect("content");
        assert_eq!(content.width, 640.0);
        assert!((content.height - 360.0).abs() < 0.01);
        assert_eq!(content.x, 10.0);
        assert!((content.y - (20.0 + (800.0 - 360.0) / 2.0)).abs() < 0.01);
    }

    #[test]
    fn a_pointer_position_maps_through_the_preview_scale() {
        let content = Rect {
            x: 0.0,
            y: 0.0,
            width: 960.0,
            height: 540.0,
        };
        let point = to_composition((480.0, 270.0), content, (1920.0, 1080.0)).expect("point");
        assert!((point.x - 960.0).abs() < 0.01);
        assert!((point.y - 540.0).abs() < 0.01);
    }

    #[test]
    fn a_letterboxed_preview_subtracts_its_offset_before_scaling() {
        let container = Rect {
            x: 0.0,
            y: 0.0,
            width: 1000.0,
            height: 400.0,
        };
        let content = contain_rect(container, (800.0, 400.0)).expect("content");
        assert_eq!(content.x, 100.0);
        let origin = to_composition((100.0, 0.0), content, (800.0, 400.0)).expect("origin");
        assert!(origin.x.abs() < 0.01);
        let middle = to_composition((500.0, 200.0), content, (800.0, 400.0)).expect("middle");
        assert!((middle.x - 400.0).abs() < 0.01);
        assert!((middle.y - 200.0).abs() < 0.01);
    }

    #[test]
    fn a_drag_becomes_a_stroke_with_every_sampled_point() {
        let tools = tools("pen");
        let mut stroke =
            Stroke::begin(&tools, Point { x: 10.0, y: 10.0 }, "a-1".into()).expect("stroke");
        stroke.update(&tools, Point { x: 20.0, y: 30.0 }, false);
        let annotation = stroke.finish().expect("annotation");
        let Annotation::Pen {
            points,
            stroke: color,
            stroke_width,
            ..
        } = annotation
        else {
            panic!("expected a pen");
        };
        assert_eq!(points, vec![10.0, 10.0, 20.0, 30.0]);
        assert_eq!(color, tools.selected_color);
        assert_eq!(stroke_width, tools.stroke_width);
    }

    #[test]
    fn a_line_waits_for_the_minimum_drag_distance() {
        let tools = tools("line");
        let mut stroke =
            Stroke::begin(&tools, Point { x: 0.0, y: 0.0 }, "a-1".into()).expect("stroke");
        stroke.update(&tools, Point { x: 1.0, y: 1.0 }, false);
        assert!(stroke.draft().is_none());
        stroke.update(&tools, Point { x: 40.0, y: 0.0 }, false);
        let Some(Annotation::Line { points, .. }) = stroke.draft() else {
            panic!("expected a line");
        };
        assert_eq!(*points, [0.0, 0.0, 40.0, 0.0]);
    }

    #[test]
    fn the_select_tool_starts_no_stroke() {
        assert!(Stroke::begin(&tools("select"), Point { x: 1.0, y: 1.0 }, "a".into()).is_none());
        assert!(Stroke::begin(&tools("text"), Point { x: 1.0, y: 1.0 }, "a".into()).is_none());
    }

    #[test]
    fn an_annotation_lands_on_a_new_segment_at_the_playhead() {
        let mut state = VideoEditorState::default();
        let annotation = Annotation::Line {
            id: "ann-1".into(),
            points: [0.0, 0.0, 10.0, 10.0],
            stroke: "#ff0000".into(),
            stroke_width: 4.0,
        };
        let id = attach(
            &mut state,
            None,
            annotation,
            2.0,
            10.0,
            (1920.0, 1080.0),
            "drawing-1",
        )
        .expect("segment");
        assert_eq!(id, "drawing-1");
        let segment = &state.drawing_segments[0];
        assert_eq!(segment.start_time, 2.0);
        assert_eq!(segment.end_time, 5.0);
        assert_eq!(segment.canvas_width, 1920.0);
        assert_eq!(segment.canvas_height, 1080.0);
        assert_eq!(segment.annotations.len(), 1);
    }

    #[test]
    fn a_timeline_added_clip_becomes_a_real_segment_instead_of_a_second_one() {
        let mut state = VideoEditorState {
            drawing_segments: vec![DrawingSegment {
                id: "drawing-timeline".into(),
                start_time: 1.0,
                end_time: 4.0,
                ..DrawingSegment::default()
            }],
            ..VideoEditorState::default()
        };
        let id = attach(
            &mut state,
            Some("drawing-timeline"),
            Annotation::Line {
                id: "ann-1".into(),
                points: [0.0, 0.0, 10.0, 10.0],
                stroke: "#ff0000".into(),
                stroke_width: 4.0,
            },
            2.0,
            10.0,
            (1280.0, 720.0),
            "drawing-2",
        )
        .expect("segment");
        assert_eq!(id, "drawing-timeline");
        assert_eq!(state.drawing_segments.len(), 1);
        let segment = &state.drawing_segments[0];
        assert_eq!(segment.canvas_width, 1280.0);
        assert_eq!(segment.annotations.len(), 1);
    }

    #[test]
    fn a_segment_that_already_has_annotations_is_never_appended_to() {
        let mut state = VideoEditorState {
            drawing_segments: vec![DrawingSegment {
                id: "drawing-1".into(),
                start_time: 0.0,
                end_time: 4.0,
                canvas_width: 100.0,
                canvas_height: 100.0,
                annotations: vec![Annotation::Line {
                    id: "ann-1".into(),
                    points: [0.0, 0.0, 1.0, 1.0],
                    stroke: "#fff".into(),
                    stroke_width: 1.0,
                }],
            }],
            ..VideoEditorState::default()
        };
        attach(
            &mut state,
            Some("drawing-1"),
            Annotation::Line {
                id: "ann-2".into(),
                points: [0.0, 0.0, 2.0, 2.0],
                stroke: "#fff".into(),
                stroke_width: 1.0,
            },
            1.0,
            10.0,
            (100.0, 100.0),
            "drawing-2",
        );
        assert_eq!(state.drawing_segments.len(), 2);
    }

    #[test]
    fn a_stroke_authored_on_a_scaled_preview_exports_where_it_was_drawn() {
        let composition = (200.0, 200.0);
        let container = Rect {
            x: 0.0,
            y: 0.0,
            width: 400.0,
            height: 500.0,
        };
        let content = contain_rect(container, composition).expect("content");
        let point = to_composition((200.0, 250.0), content, composition).expect("point");
        assert!((point.x - 100.0).abs() < 0.01);
        assert!((point.y - 100.0).abs() < 0.01);

        let mut state = VideoEditorState::default();
        attach(
            &mut state,
            None,
            Annotation::Redact {
                id: "ann-1".into(),
                x: f64::from(point.x) - 10.0,
                y: f64::from(point.y) - 10.0,
                width: 20.0,
                height: 20.0,
                style: "blackout".into(),
                intensity: 5.0,
            },
            0.0,
            10.0,
            composition,
            "drawing-1",
        );

        let mut canvas = crate::render::canvas::Canvas::new(200, 200).expect("canvas");
        canvas.fill_all(tiny_skia::Color::from_rgba8(255, 255, 255, 255));
        drawing::render(
            &mut canvas,
            &state.drawing_segments,
            0.0,
            composition.0,
            composition.1,
            false,
        );
        let pixel = |x: u32, y: u32| canvas.pixmap().data()[((y * 200 + x) * 4) as usize];
        assert_eq!(pixel(100, 100), 0);
        assert_eq!(pixel(10, 10), 255);
    }

    #[test]
    fn the_panel_shows_the_selected_annotations_own_values() {
        let tools = tools("pen");
        let annotation = Annotation::Arrow {
            id: "ann-1".into(),
            points: [0.0, 0.0, 1.0, 1.0],
            stroke: "#00ff00".into(),
            stroke_width: 9.0,
            arrow_style: Some("curved".into()),
            bend_offset: None,
        };
        let style = displayed_style(Some(&annotation), &tools);
        assert_eq!(style.config_type, "arrow");
        assert_eq!(style.color, "#00ff00");
        assert_eq!(style.stroke_width, 9.0);
        assert_eq!(style.arrow_style, "curved");

        let empty = displayed_style(None, &tools);
        assert_eq!(empty.config_type, "pen");
        assert_eq!(empty.color, tools.selected_color);
    }

    #[test]
    fn the_highlight_tool_shows_its_own_colour_when_nothing_is_selected() {
        let tools = tools("highlight");
        let style = displayed_style(None, &tools);
        assert_eq!(style.color, tools.highlight_color);
    }

    #[test]
    fn panel_edits_land_on_the_selected_annotation() {
        let mut arrow = Annotation::Arrow {
            id: "ann-1".into(),
            points: [0.0, 0.0, 1.0, 1.0],
            stroke: "#ff0000".into(),
            stroke_width: 4.0,
            arrow_style: Some("standard".into()),
            bend_offset: None,
        };
        assert!(apply_option(
            &mut arrow,
            &EditorOption::Color("#123456".into())
        ));
        assert!(apply_option(&mut arrow, &EditorOption::StrokeWidth(11.0)));
        assert!(apply_option(
            &mut arrow,
            &EditorOption::ArrowStyle("double".into())
        ));
        let style = displayed_style(Some(&arrow), &tools("pen"));
        assert_eq!(style.color, "#123456");
        assert_eq!(style.stroke_width, 11.0);
        assert_eq!(style.arrow_style, "double");

        let mut highlight = Annotation::Highlight {
            id: "ann-2".into(),
            points: vec![0.0, 0.0],
            fill: "#FFFF00".into(),
            opacity: 0.4,
            stroke_width: 20.0,
        };
        assert!(apply_option(
            &mut highlight,
            &EditorOption::HighlightOpacity(0.6)
        ));
        assert!(apply_option(
            &mut highlight,
            &EditorOption::HighlightColor("#00BFFF".into())
        ));
        let style = displayed_style(Some(&highlight), &tools("pen"));
        assert_eq!(style.highlight_opacity, 0.6);
        assert_eq!(style.color, "#00BFFF");

        let mut shape = Annotation::Rectangle {
            id: "ann-3".into(),
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
            stroke: "#ff0000".into(),
            stroke_width: 4.0,
            fill: None,
        };
        assert!(apply_option(
            &mut shape,
            &EditorOption::ShapeFillMode("filled".into())
        ));
        assert_eq!(
            displayed_style(Some(&shape), &tools("pen")).shape_fill_mode,
            "filled"
        );

        let mut redact = Annotation::Redact {
            id: "ann-4".into(),
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
            style: "pixelate".into(),
            intensity: 5.0,
        };
        assert!(apply_option(
            &mut redact,
            &EditorOption::RedactStyle("blackout".into())
        ));
        assert!(apply_option(
            &mut redact,
            &EditorOption::RedactIntensity(9.0)
        ));
        let style = displayed_style(Some(&redact), &tools("pen"));
        assert_eq!(style.redact_style, "blackout");
        assert_eq!(style.redact_intensity, 9.0);
    }

    #[test]
    fn number_values_continue_from_what_is_already_placed() {
        let state = VideoEditorState {
            drawing_segments: vec![DrawingSegment {
                id: "d".into(),
                start_time: 0.0,
                end_time: 1.0,
                canvas_width: 10.0,
                canvas_height: 10.0,
                annotations: vec![Annotation::Number {
                    id: "n".into(),
                    x: 0.0,
                    y: 0.0,
                    value: 1.0,
                    display_value: "1".into(),
                    fill: "#fff".into(),
                    size: "medium".into(),
                }],
            }],
            ..VideoEditorState::default()
        };
        assert_eq!(next_number_value(&state, &tools("number")), 2.0);
    }

    #[test]
    fn a_placed_text_annotation_carries_the_tool_defaults() {
        let tools = tools("text");
        let Some(Annotation::Text {
            text,
            font_size,
            font_family,
            background_color,
            ..
        }) = place(&tools, Point { x: 5.0, y: 6.0 }, "ann-1".into(), 1.0)
        else {
            panic!("expected a text annotation");
        };
        assert_eq!(text, "Text");
        assert_eq!(font_size, tools.text_font_size);
        assert_eq!(
            font_family.as_deref(),
            Some(tools.text_font_family.as_str())
        );
        assert!(background_color.is_some());
    }

    #[test]
    fn a_segment_never_runs_past_the_timeline() {
        assert_eq!(segment_times(9.0, 10.0), Some((7.0, 10.0)));
        assert_eq!(segment_times(0.0, 2.0), Some((0.0, 2.0)));
        assert_eq!(segment_times(1.0, 0.0), None);
    }
}
