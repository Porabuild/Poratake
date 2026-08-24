//! Port of `composition/subtitle-canvas-renderer.ts` — the word-by-word
//! caption box.

use tiny_skia::{Color, FillRule};

use crate::render::canvas::{rounded_rect_path, Canvas};
use crate::render::text;
use crate::video::composition::segments::{map_timeline_to_video_time, VideoSegment};
use crate::video::sidecars::SubtitleData;
use crate::windows::video_editor::styles::SubtitleStyle;

/// `CANVAS_CONSTANTS` in `components/video-editor/constants.ts`.
pub const PADDING_VERTICAL: f64 = 32.0;
pub const PADDING_HORIZONTAL: f64 = 48.0;
pub const MARGIN_EDGE: f64 = 40.0;
pub const CORNER_RADIUS: f64 = 40.0;

/// `FONT_SIZES` — the caption sizes are absolute, matched to a 4K composition.
pub fn font_size(size: &str) -> f64 {
    match size {
        "small" => 108.0,
        "large" => 180.0,
        _ => 144.0,
    }
}

/// The renderer asks for the platform UI font; the export resolves the same
/// family through `render::text`.
const FONT_FAMILY: &str = "sans";

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Bounds {
    pub y: f64,
    pub height: f64,
}

/// `getSubtitleBounds` — the keyboard overlay stacks above this.
pub fn bounds(style: &SubtitleStyle, video_height: f64) -> Bounds {
    let size = font_size(&style.font_size);
    let line_height = size * 1.3;
    let height = line_height + PADDING_VERTICAL * 2.0;
    let y = if style.position == "top" {
        MARGIN_EDGE
    } else {
        video_height - MARGIN_EDGE - height
    };
    Bounds { y, height }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ActiveSubtitle {
    pub words: Vec<String>,
    pub highlighted_count: usize,
}

/// `getActiveSubtitle`.
pub fn active_subtitle(
    data: &SubtitleData,
    segments: &[VideoSegment],
    timeline_time: f64,
) -> Option<ActiveSubtitle> {
    let video_time = map_timeline_to_video_time(timeline_time, segments)?;
    for segment in &data.segments {
        let first_word_start = segment
            .words
            .as_ref()
            .and_then(|words| words.first())
            .map(|word| word.start)
            .unwrap_or(segment.start);
        if video_time < first_word_start || video_time > segment.end {
            continue;
        }

        let (words, highlighted_count) = match segment.words.as_ref().filter(|w| !w.is_empty()) {
            Some(timed) => {
                let mut highlighted = 0;
                for word in timed {
                    if video_time >= word.start {
                        highlighted += 1;
                    } else {
                        break;
                    }
                }
                (
                    timed
                        .iter()
                        .map(|word| word.text.clone())
                        .collect::<Vec<_>>(),
                    highlighted,
                )
            }
            None => {
                let words: Vec<String> = segment
                    .text
                    .split(' ')
                    .filter(|word| !word.is_empty())
                    .map(str::to_string)
                    .collect();
                let count = words.len();
                (words, count)
            }
        };

        if words.is_empty() {
            return None;
        }
        return Some(ActiveSubtitle {
            words,
            highlighted_count,
        });
    }
    None
}

#[derive(Clone, Debug, PartialEq)]
pub struct WrappedLine {
    pub text: String,
    pub words: Vec<String>,
}

/// `wrapTextFromWords`.
pub fn wrap_words(words: &[String], size: f64, max_width: f64) -> Vec<WrappedLine> {
    let mut lines: Vec<WrappedLine> = Vec::new();
    let mut current_words: Vec<String> = Vec::new();
    let mut current_text = String::new();

    for word in words {
        let candidate = if current_text.is_empty() {
            word.clone()
        } else {
            format!("{current_text} {word}")
        };
        let width = text::measure(&candidate, FONT_FAMILY, size as f32).width as f64;
        if width > max_width && !current_text.is_empty() {
            lines.push(WrappedLine {
                text: std::mem::take(&mut current_text),
                words: std::mem::take(&mut current_words),
            });
            current_words = vec![word.clone()];
            current_text = word.clone();
        } else {
            current_words.push(word.clone());
            current_text = candidate;
        }
    }

    if !current_text.is_empty() {
        lines.push(WrappedLine {
            text: current_text,
            words: current_words,
        });
    }
    lines
}

pub struct RenderConfig<'a> {
    pub subtitle_data: &'a SubtitleData,
    pub subtitle_style: &'a SubtitleStyle,
    pub segments: &'a [VideoSegment],
    pub video_width: f64,
    pub video_height: f64,
}

/// Port of `renderSubtitle`.
pub fn render(canvas: &mut Canvas, timeline_time: f64, config: &RenderConfig<'_>) {
    if !config.subtitle_style.visible {
        return;
    }
    let Some(active) = active_subtitle(config.subtitle_data, config.segments, timeline_time) else {
        return;
    };

    let size = font_size(&config.subtitle_style.font_size);
    let max_text_width = config.video_width - MARGIN_EDGE * 2.0 - PADDING_HORIZONTAL * 2.0;
    let lines = wrap_words(&active.words, size, max_text_width);
    if lines.is_empty() {
        return;
    }

    let line_height = size * 1.3;
    let box_height = lines.len() as f64 * line_height + PADDING_VERTICAL * 2.0;
    let widest = lines
        .iter()
        .map(|line| text::measure(&line.text, FONT_FAMILY, size as f32).width as f64)
        .fold(0.0_f64, f64::max);
    let box_width = (widest + PADDING_HORIZONTAL * 2.0).min(config.video_width - MARGIN_EDGE * 2.0);
    let box_x = (config.video_width - box_width) / 2.0;
    let box_y = if config.subtitle_style.position == "top" {
        MARGIN_EDGE
    } else {
        config.video_height - MARGIN_EDGE - box_height
    };

    canvas.save();

    if config.subtitle_style.background_color != "none" {
        let background = if config.subtitle_style.background_color == "light" {
            Color::from_rgba8(255, 255, 255, 255)
        } else {
            Color::from_rgba8(0, 0, 0, 255)
        };
        if let Some(path) = rounded_rect_path(
            box_x as f32,
            box_y as f32,
            box_width as f32,
            box_height as f32,
            CORNER_RADIUS as f32,
        ) {
            canvas.fill_path(&path, background, FillRule::Winding);
        }
    }

    let channel: u8 = if config.subtitle_style.background_color == "light" {
        0
    } else {
        255
    };
    let highlight_alpha = config.subtitle_style.opacity.clamp(0.0, 1.0);
    let muted_alpha = (highlight_alpha * 0.35).max(0.25).min(1.0);
    let muted = Color::from_rgba8(channel, channel, channel, (muted_alpha * 255.0) as u8);
    let highlighted = Color::from_rgba8(channel, channel, channel, (highlight_alpha * 255.0) as u8);

    let text_start_y = box_y + PADDING_VERTICAL + line_height / 2.0;
    let mut word_offset = 0usize;

    for (index, line) in lines.iter().enumerate() {
        let line_y = text_start_y + index as f64 * line_height;
        let line_width = text::measure(&line.text, FONT_FAMILY, size as f32).width as f64;
        let line_x = (config.video_width - line_width) / 2.0;

        text::fill_text(
            canvas,
            &line.text,
            FONT_FAMILY,
            size as f32,
            line_x as f32,
            line_y as f32,
            muted,
            text::Align::Left,
            text::Baseline::Middle,
        );

        let in_line = active
            .highlighted_count
            .saturating_sub(word_offset)
            .min(line.words.len());
        if in_line > 0 {
            let spoken = line.words[..in_line].join(" ");
            text::fill_text(
                canvas,
                &spoken,
                FONT_FAMILY,
                size as f32,
                line_x as f32,
                line_y as f32,
                highlighted,
                text::Align::Left,
                text::Baseline::Middle,
            );
        }
        word_offset += line.words.len();
    }

    canvas.restore();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::video::sidecars::{SubtitleSegment, SubtitleWord};

    fn data() -> SubtitleData {
        SubtitleData {
            segments: vec![SubtitleSegment {
                start: 0.0,
                end: 4.0,
                text: "hello brave new world".into(),
                words: Some(vec![
                    SubtitleWord {
                        text: "hello".into(),
                        start: 0.0,
                        end: 1.0,
                    },
                    SubtitleWord {
                        text: "brave".into(),
                        start: 1.0,
                        end: 2.0,
                    },
                    SubtitleWord {
                        text: "new".into(),
                        start: 2.0,
                        end: 3.0,
                    },
                    SubtitleWord {
                        text: "world".into(),
                        start: 3.0,
                        end: 4.0,
                    },
                ]),
            }],
            ..SubtitleData::default()
        }
    }

    fn segments() -> Vec<VideoSegment> {
        vec![VideoSegment {
            start_time: 0.0,
            end_time: 10.0,
            timeline_start: 0.0,
            speed: 1.0,
        }]
    }

    #[test]
    fn words_light_up_as_they_are_spoken() {
        let data = data();
        let segments = segments();
        let at = |time: f64| {
            active_subtitle(&data, &segments, time)
                .map(|active| active.highlighted_count)
                .unwrap_or_default()
        };
        assert_eq!(at(0.0), 1);
        assert_eq!(at(2.5), 3);
        assert_eq!(at(3.9), 4);
    }

    #[test]
    fn a_segment_without_word_timings_highlights_everything() {
        let mut data = data();
        data.segments[0].words = None;
        let active = active_subtitle(&data, &segments(), 1.0).expect("active");
        assert_eq!(active.words.len(), 4);
        assert_eq!(active.highlighted_count, 4);
    }

    #[test]
    fn nothing_is_active_outside_the_segment() {
        assert!(active_subtitle(&data(), &segments(), 5.0).is_none());
    }

    #[test]
    fn bounds_sit_against_the_chosen_edge() {
        let mut style = SubtitleStyle::default();
        let bottom = bounds(&style, 2160.0);
        assert!(bottom.y > 1000.0);
        style.position = "top".into();
        assert_eq!(bounds(&style, 2160.0).y, MARGIN_EDGE);
    }

    #[test]
    fn wrapping_splits_a_long_run_into_lines() {
        let words: Vec<String> = (0..20).map(|index| format!("word{index}")).collect();
        let lines = wrap_words(&words, 48.0, 300.0);
        assert!(lines.len() > 1);
        assert_eq!(
            lines.iter().map(|line| line.words.len()).sum::<usize>(),
            words.len()
        );
    }

    #[test]
    fn drawing_fills_the_caption_box() {
        let mut canvas = Canvas::new(1920, 1080).expect("canvas");
        let style = SubtitleStyle {
            font_size: "small".into(),
            ..SubtitleStyle::default()
        };
        render(
            &mut canvas,
            1.0,
            &RenderConfig {
                subtitle_data: &data(),
                subtitle_style: &style,
                segments: &segments(),
                video_width: 1920.0,
                video_height: 1080.0,
            },
        );
        let covered = canvas
            .pixmap()
            .data()
            .chunks_exact(4)
            .filter(|pixel| pixel[3] > 0)
            .count();
        assert!(covered > 1000, "{covered}");
    }
}
