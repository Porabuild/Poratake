//! Port of `renderDrawings` in `composition/drawing-canvas-renderer.ts`. The
//! annotations themselves are drawn by the shared renderer, so a video drawing
//! looks exactly like the same annotation in the image editor.

use crate::editor::annotations::Annotation;
use crate::render::annotations as annotation_renderer;
use crate::render::canvas::Canvas;
use crate::windows::video_editor::model::DrawingSegment;

/// `getRenderableAnnotations` — a segment only draws inside its own span, and
/// `only_redact` is the export pass that burns in redactions before anything
/// else is composited.
pub fn renderable_annotations(
    segment: &DrawingSegment,
    timeline_time: f64,
    only_redact: bool,
) -> Vec<&Annotation> {
    if timeline_time < segment.start_time || timeline_time > segment.end_time {
        return Vec::new();
    }
    segment
        .annotations
        .iter()
        .filter(|annotation| !only_redact || matches!(annotation, Annotation::Redact { .. }))
        .collect()
}

pub fn render(
    canvas: &mut Canvas,
    segments: &[DrawingSegment],
    timeline_time: f64,
    width: f64,
    height: f64,
    only_redact: bool,
) {
    for segment in segments {
        if segment.canvas_width <= 0.0 || segment.canvas_height <= 0.0 {
            continue;
        }
        let annotations = renderable_annotations(segment, timeline_time, only_redact);
        if annotations.is_empty() {
            continue;
        }
        let scale_x = width / segment.canvas_width;
        let scale_y = height / segment.canvas_height;
        for annotation in annotations {
            let scaled = annotation_renderer::scale_to_composition(annotation, scale_x, scale_y);
            annotation_renderer::draw(canvas, &scaled);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn segment() -> DrawingSegment {
        DrawingSegment {
            id: "draw-1".into(),
            start_time: 1.0,
            end_time: 3.0,
            canvas_width: 100.0,
            canvas_height: 100.0,
            annotations: vec![
                Annotation::Redact {
                    id: "r".into(),
                    x: 10.0,
                    y: 10.0,
                    width: 20.0,
                    height: 20.0,
                    style: "blackout".into(),
                    intensity: 5.0,
                },
                Annotation::Line {
                    id: "l".into(),
                    points: [0.0, 0.0, 50.0, 50.0],
                    stroke: "#ff0000".into(),
                    stroke_width: 4.0,
                },
            ],
        }
    }

    #[test]
    fn a_segment_only_draws_inside_its_span() {
        let segment = segment();
        assert!(renderable_annotations(&segment, 0.5, false).is_empty());
        assert_eq!(renderable_annotations(&segment, 2.0, false).len(), 2);
        assert!(renderable_annotations(&segment, 3.5, false).is_empty());
    }

    #[test]
    fn the_redact_only_pass_skips_everything_else() {
        let segment = segment();
        let only = renderable_annotations(&segment, 2.0, true);
        assert_eq!(only.len(), 1);
        assert!(matches!(only[0], Annotation::Redact { .. }));
    }

    #[test]
    fn annotations_scale_from_the_authoring_canvas() {
        let mut canvas = Canvas::new(200, 200).expect("canvas");
        canvas.fill_all(tiny_skia::Color::from_rgba8(255, 255, 255, 255));
        render(&mut canvas, &[segment()], 2.0, 200.0, 200.0, true);
        // The 10,10 20x20 redaction lands at 20,20 40x40 in the composition.
        let pixel = |x: u32, y: u32| {
            let index = ((y * 200 + x) * 4) as usize;
            canvas.pixmap().data()[index]
        };
        assert_eq!(pixel(40, 40), 0);
        assert_eq!(pixel(10, 10), 255);
    }

    #[test]
    fn a_segment_with_no_authoring_size_is_skipped() {
        let mut canvas = Canvas::new(50, 50).expect("canvas");
        let mut broken = segment();
        broken.canvas_width = 0.0;
        render(&mut canvas, &[broken], 2.0, 50.0, 50.0, false);
        assert!(canvas
            .pixmap()
            .data()
            .chunks_exact(4)
            .all(|pixel| pixel[3] == 0));
    }
}
