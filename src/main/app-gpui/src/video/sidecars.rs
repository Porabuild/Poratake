//! The recording sidecars a project carries next to `recording.mov`:
//! `cursor.json`, `keys.json`, `subtitle.json` and `camera.json`. These are
//! ports of `types/cursor.ts`, `keyboard.ts`, `subtitle.ts` and `camera.ts`,
//! so both shells read and write the same files.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::video::project;

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct Size {
    #[serde(default)]
    pub width: f64,
    #[serde(default)]
    pub height: f64,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct ScrollDelta {
    #[serde(default)]
    pub x: f64,
    #[serde(default)]
    pub y: f64,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorEvent {
    #[serde(default)]
    pub timestamp: f64,
    #[serde(default)]
    pub x: f64,
    #[serde(default)]
    pub y: f64,
    #[serde(default, rename = "type")]
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub button: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scroll_delta: Option<ScrollDelta>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingMeta {
    #[serde(default)]
    pub start_time: String,
    #[serde(default)]
    pub duration: f64,
    #[serde(default)]
    pub sample_rate: f64,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorData {
    #[serde(default)]
    pub recording_area: Size,
    #[serde(default)]
    pub events: Vec<CursorEvent>,
    #[serde(default)]
    pub meta: RecordingMeta,
}

impl CursorData {
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyboardKeyEvent {
    #[serde(default)]
    pub timestamp: f64,
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub key_code: i64,
    #[serde(default)]
    pub modifiers: Vec<String>,
    #[serde(default, rename = "type")]
    pub kind: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyboardMeta {
    #[serde(default)]
    pub start_time: String,
    #[serde(default)]
    pub duration: f64,
    #[serde(default)]
    pub sample_rate: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyboardData {
    #[serde(default)]
    pub events: Vec<KeyboardKeyEvent>,
    #[serde(default)]
    pub meta: KeyboardMeta,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleWord {
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub start: f64,
    #[serde(default)]
    pub end: f64,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleSegment {
    #[serde(default)]
    pub start: f64,
    #[serde(default)]
    pub end: f64,
    #[serde(default)]
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub words: Option<Vec<SubtitleWord>>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleMeta {
    #[serde(default)]
    pub generated_at: String,
    #[serde(default)]
    pub language: String,
    #[serde(default)]
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleData {
    #[serde(default)]
    pub segments: Vec<SubtitleSegment>,
    #[serde(default)]
    pub meta: SubtitleMeta,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct CameraVisibleRange {
    #[serde(default)]
    pub start: f64,
    #[serde(default)]
    pub end: f64,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraRecordingMeta {
    #[serde(default)]
    pub device_id: String,
    #[serde(default)]
    pub device_name: String,
    #[serde(default)]
    pub width: f64,
    #[serde(default)]
    pub height: f64,
    #[serde(default)]
    pub duration: f64,
    #[serde(default)]
    pub start_time: String,
    #[serde(default)]
    pub frame_rate: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible_ranges: Option<Vec<CameraVisibleRange>>,
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Option<T> {
    let contents = std::fs::read_to_string(path).ok()?;
    match serde_json::from_str(&contents) {
        Ok(value) => Some(value),
        Err(error) => {
            eprintln!("[video] failed to parse {}: {error}", path.display());
            None
        }
    }
}

pub fn load_cursor(project_or_video: &Path) -> Option<CursorData> {
    read_json(&project::cursor_path(project_or_video))
}

pub fn load_keyboard(project_or_video: &Path) -> Option<KeyboardData> {
    read_json(&project::keys_path(project_or_video))
}

pub fn load_subtitle(project_or_video: &Path) -> Option<SubtitleData> {
    read_json(&project::subtitle_path(project_or_video))
}

pub fn load_camera_meta(project_or_video: &Path) -> Option<CameraRecordingMeta> {
    #[derive(Deserialize)]
    struct Wrapper {
        #[serde(default)]
        meta: Option<CameraRecordingMeta>,
    }
    // `camera.json` is written as `{ videoFile, meta }`; older takes stored the
    // meta at the top level.
    let path = project::camera_meta_path(project_or_video);
    let contents = std::fs::read_to_string(&path).ok()?;
    if let Ok(wrapper) = serde_json::from_str::<Wrapper>(&contents) {
        if let Some(meta) = wrapper.meta {
            return Some(meta);
        }
    }
    serde_json::from_str(&contents).ok()
}

/// Port of `mapVideoRangesToCameraSegments` in `types/camera.ts`: the camera's
/// on-periods are recorded in video time and mapped onto the timeline.
pub fn map_visible_ranges_to_segments(
    ranges: Option<&[CameraVisibleRange]>,
    segments: &[crate::windows::video_editor::model::Segment],
    total_duration: f64,
) -> Vec<crate::windows::video_editor::model::CameraSegment> {
    use crate::windows::video_editor::model::CameraSegment;

    const MIN_SEGMENT_DURATION: f64 = 0.1;

    let video_duration = if segments.is_empty() {
        total_duration
    } else {
        segments
            .iter()
            .map(|segment| segment.original_end)
            .fold(f64::MIN, f64::max)
    };

    let mut effective_ranges: Vec<CameraVisibleRange> = match ranges {
        Some(ranges) if !ranges.is_empty() => ranges.to_vec(),
        _ => vec![CameraVisibleRange {
            start: 0.0,
            end: video_duration,
        }],
    };
    effective_ranges.sort_by(|a, b| a.start.total_cmp(&b.start));

    let fallback = vec![crate::windows::video_editor::model::Segment {
        id: String::new(),
        original_start: 0.0,
        original_end: total_duration,
        trim_min_start: 0.0,
        trim_max_end: total_duration,
        speed: None,
    }];
    let effective_segments: &[_] = if segments.is_empty() {
        &fallback
    } else {
        segments
    };

    let mut result = Vec::new();
    let mut timeline_start = 0.0;
    for (index, segment) in effective_segments.iter().enumerate() {
        let speed = segment.speed.unwrap_or(1.0).max(0.01);
        for (range_index, range) in effective_ranges.iter().enumerate() {
            let start = range.start.max(segment.original_start);
            let end = range.end.min(segment.original_end);
            if end - start < MIN_SEGMENT_DURATION {
                continue;
            }
            result.push(CameraSegment {
                id: format!("camera-{index}-{range_index}"),
                start_time: timeline_start + (start - segment.original_start) / speed,
                end_time: timeline_start + (end - segment.original_start) / speed,
            });
        }
        timeline_start += (segment.original_end - segment.original_start) / speed;
    }
    result
}

/// Port of `isCameraVisibleAt`.
pub fn is_camera_visible_at(
    segments: Option<&[crate::windows::video_editor::model::CameraSegment]>,
    timeline_time: f64,
) -> bool {
    let Some(segments) = segments else {
        return true;
    };
    segments
        .iter()
        .any(|segment| timeline_time >= segment.start_time && timeline_time < segment.end_time)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::windows::video_editor::model::Segment;

    #[test]
    fn parses_a_cursor_file_written_by_the_daemon() {
        let json = r##"{
            "recordingArea": { "width": 2560, "height": 1440 },
            "events": [
                { "timestamp": 0, "x": 0.5, "y": 0.5, "type": "move", "cursor": "arrow" },
                { "timestamp": 0.1, "x": 0.6, "y": 0.4, "type": "down", "button": "left" }
            ],
            "meta": { "startTime": "now", "duration": 1.5, "sampleRate": 60 }
        }"##;
        let parsed: CursorData = serde_json::from_str(json).expect("cursor");
        assert_eq!(parsed.recording_area.width, 2560.0);
        assert_eq!(parsed.events.len(), 2);
        assert_eq!(parsed.events[1].kind, "down");
        assert_eq!(parsed.events[1].button.as_deref(), Some("left"));
        assert_eq!(parsed.meta.sample_rate, 60.0);
    }

    #[test]
    fn parses_camera_meta_in_both_shapes() {
        let wrapped = r##"{"videoFile":"camera.mov","meta":{"deviceName":"FaceTime","width":1280,"height":720,"visibleRanges":[{"start":0,"end":2}]}}"##;
        let meta: CameraRecordingMeta = serde_json::from_str::<serde_json::Value>(wrapped)
            .ok()
            .and_then(|value| serde_json::from_value(value["meta"].clone()).ok())
            .expect("meta");
        assert_eq!(meta.device_name, "FaceTime");
        assert_eq!(meta.visible_ranges.as_ref().map(Vec::len), Some(1));
    }

    #[test]
    fn maps_camera_ranges_through_segment_speeds() {
        let segments = vec![Segment {
            id: "a".into(),
            original_start: 0.0,
            original_end: 10.0,
            trim_min_start: 0.0,
            trim_max_end: 10.0,
            speed: Some(2.0),
        }];
        let ranges = [CameraVisibleRange {
            start: 2.0,
            end: 6.0,
        }];
        let mapped = map_visible_ranges_to_segments(Some(&ranges), &segments, 10.0);
        assert_eq!(mapped.len(), 1);
        assert_eq!(mapped[0].start_time, 1.0);
        assert_eq!(mapped[0].end_time, 3.0);
    }

    #[test]
    fn no_camera_segments_means_always_visible() {
        assert!(is_camera_visible_at(None, 5.0));
        assert!(!is_camera_visible_at(Some(&[]), 5.0));
    }

    #[test]
    fn a_range_shorter_than_the_minimum_is_dropped() {
        let segments = vec![Segment {
            id: "a".into(),
            original_start: 0.0,
            original_end: 10.0,
            trim_min_start: 0.0,
            trim_max_end: 10.0,
            speed: None,
        }];
        let ranges = [CameraVisibleRange {
            start: 1.0,
            end: 1.05,
        }];
        assert!(map_visible_ranges_to_segments(Some(&ranges), &segments, 10.0).is_empty());
    }
}
