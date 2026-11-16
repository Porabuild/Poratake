use tiny_skia::{BlendMode, Pixmap};

use crate::editor::annotations::{normalize_rect, redact_intensity, Annotation};
use crate::render::canvas::Canvas;

use super::{Engine, Frames};

#[derive(Clone, Copy)]
struct Bounds {
    left: u32,
    top: u32,
    right: u32,
    bottom: u32,
}

impl Bounds {
    fn width(self) -> u32 {
        self.right - self.left
    }

    fn height(self) -> u32 {
        self.bottom - self.top
    }

    fn union(self, other: Self) -> Self {
        Self {
            left: self.left.min(other.left),
            top: self.top.min(other.top),
            right: self.right.max(other.right),
            bottom: self.bottom.max(other.bottom),
        }
    }

    fn intersects(self, other: Self) -> bool {
        self.left < other.right
            && other.left < self.right
            && self.top < other.bottom
            && other.top < self.bottom
    }
}

struct Batch {
    source: Bounds,
    targets: Vec<Bounds>,
}

fn batches(engine: &Engine, time: f64, output_width: u32, output_height: u32) -> Vec<Batch> {
    let (width, height) = engine.dimensions();
    let scale_x = output_width as f64 / width as f64;
    let scale_y = output_height as f64 / height as f64;
    let mut batches: Vec<Batch> = Vec::new();
    for segment in &engine.config().state.drawing_segments {
        if time < segment.start_time
            || time > segment.end_time
            || !segment.canvas_width.is_finite()
            || !segment.canvas_height.is_finite()
            || segment.canvas_width <= 0.0
            || segment.canvas_height <= 0.0
        {
            continue;
        }
        let author_x = width as f64 / segment.canvas_width;
        let author_y = height as f64 / segment.canvas_height;
        for annotation in &segment.annotations {
            let Annotation::Redact {
                x,
                y,
                width: rect_width,
                height: rect_height,
                style,
                intensity,
                ..
            } = annotation
            else {
                continue;
            };
            if style != "pixelate" && style != "blur" {
                continue;
            }
            let (x, y, rect_width, rect_height) = normalize_rect(
                x * author_x,
                y * author_y,
                rect_width * author_x,
                rect_height * author_y,
            );
            if ![x, y, rect_width, rect_height, *intensity]
                .into_iter()
                .all(f64::is_finite)
                || rect_width <= 0.0
                || rect_height <= 0.0
            {
                continue;
            }
            let x = x as f32;
            let y = y as f32;
            let rect_width = rect_width as f32;
            let rect_height = rect_height as f32;
            let region = Bounds {
                left: x.floor().clamp(0.0, width as f32) as u32,
                top: y.floor().clamp(0.0, height as f32) as u32,
                right: (x + rect_width).ceil().clamp(0.0, width as f32) as u32,
                bottom: (y + rect_height).ceil().clamp(0.0, height as f32) as u32,
            };
            if region.left >= region.right || region.top >= region.bottom {
                continue;
            }
            let target = Bounds {
                left: ((region.left as f64 * scale_x).floor() as u32).saturating_sub(1),
                top: ((region.top as f64 * scale_y).floor() as u32).saturating_sub(1),
                right: ((region.right as f64 * scale_x).ceil() as u32)
                    .saturating_add(1)
                    .min(output_width),
                bottom: ((region.bottom as f64 * scale_y).ceil() as u32)
                    .saturating_add(1)
                    .min(output_height),
            };
            let mut source = Bounds {
                left: ((target.left as f64 / scale_x).floor() as u32).saturating_sub(1),
                top: ((target.top as f64 / scale_y).floor() as u32).saturating_sub(1),
                right: ((target.right as f64 / scale_x).ceil() as u32)
                    .saturating_add(1)
                    .min(width),
                bottom: ((target.bottom as f64 / scale_y).ceil() as u32)
                    .saturating_add(1)
                    .min(height),
            }
            .union(region);
            if style == "blur" {
                let padding = (redact_intensity(*intensity).1.max(1.0) * 2.0).ceil() as u32;
                let left = region.left.saturating_sub(padding);
                let top = region.top.saturating_sub(padding);
                source = source.union(Bounds {
                    left,
                    top,
                    right: left
                        .saturating_add(region.width())
                        .saturating_add(padding.saturating_mul(2))
                        .min(width),
                    bottom: top
                        .saturating_add(region.height())
                        .saturating_add(padding.saturating_mul(2))
                        .min(height),
                });
            }
            let mut batch = Batch {
                source,
                targets: vec![target],
            };
            let mut index = 0;
            while index < batches.len() {
                if !batch.source.intersects(batches[index].source) {
                    index += 1;
                    continue;
                }
                let other = batches.swap_remove(index);
                batch.source = batch.source.union(other.source);
                batch.targets.extend(other.targets);
                index = 0;
            }
            batches.push(batch);
        }
    }
    batches
}

pub(super) fn apply(
    engine: &mut Engine,
    preview: Pixmap,
    time: f64,
    frames: Frames<'_>,
) -> Option<Pixmap> {
    let (width, height) = engine.dimensions();
    let scale_x = preview.width() as f32 / width as f32;
    let scale_y = preview.height() as f32 / height as f32;
    let batches = batches(engine, time, preview.width(), preview.height());
    if batches.is_empty() {
        return Some(preview);
    }
    let mut preview = Canvas::from_pixmap(preview);
    for batch in batches {
        let mut source = Canvas::new(batch.source.width(), batch.source.height())?;
        source.translate(-(batch.source.left as f32), -(batch.source.top as f32));
        engine.render_into(&mut source, time, frames);
        for target in batch.targets {
            let mut patch = Canvas::new(target.width(), target.height())?;
            patch.translate(-(target.left as f32), -(target.top as f32));
            patch.draw_pixmap_blended(
                source.pixmap().as_ref(),
                batch.source.left as f32 * scale_x,
                batch.source.top as f32 * scale_y,
                batch.source.width() as f32 * scale_x,
                batch.source.height() as f32 * scale_y,
                BlendMode::Source,
            );
            preview.draw_pixmap_blended(
                patch.pixmap().as_ref(),
                target.left as f32,
                target.top as f32,
                target.width() as f32,
                target.height() as f32,
                BlendMode::Source,
            );
        }
    }
    Some(preview.into_pixmap())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::video::composition::Config;
    use crate::windows::video_editor::model::{DrawingSegment, VideoEditorState};

    fn pixelate(id: &str, x: f64, width: f64) -> Annotation {
        Annotation::Redact {
            id: id.into(),
            x,
            y: 30.0,
            width,
            height: 20.0,
            style: "pixelate".into(),
            intensity: 5.0,
        }
    }

    fn engine(annotations: Vec<Annotation>) -> Engine {
        let state = VideoEditorState {
            drawing_segments: vec![DrawingSegment {
                id: "drawing".into(),
                start_time: 0.0,
                end_time: 3.0,
                canvas_width: 256.0,
                canvas_height: 144.0,
                annotations,
            }],
            ..VideoEditorState::default()
        };
        Engine::new(Config::new(256.0, 144.0, state))
    }

    #[test]
    fn separate_effects_use_separate_source_crops() {
        let engine = engine(vec![
            pixelate("left", 10.0, 15.0),
            pixelate("right", 210.0, 15.0),
        ]);
        let groups = batches(&engine, 1.0, 128, 72);
        assert_eq!(groups.len(), 2);
        assert!(groups.iter().all(|group| group.source.width() < 40));
        assert!(batches(&engine, 4.0, 128, 72).is_empty());
    }

    #[test]
    fn an_overlapping_effect_merges_both_source_crops() {
        let engine = engine(vec![
            pixelate("left", 10.0, 15.0),
            pixelate("right", 210.0, 15.0),
            pixelate("bridge", 24.0, 187.0),
        ]);
        let groups = batches(&engine, 1.0, 128, 72);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].targets.len(), 3);
    }
}
