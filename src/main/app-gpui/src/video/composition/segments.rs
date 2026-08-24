//! Port of `convertSegmentsToVideoSegments` and `mapTimelineToVideoTime` in
//! `composition/types.ts`. Trimming and per-segment speed mean timeline time
//! and video time differ, and every overlay is sampled in video time.

use crate::windows::video_editor::model::Segment;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct VideoSegment {
    pub start_time: f64,
    pub end_time: f64,
    pub timeline_start: f64,
    pub speed: f64,
}

pub fn to_video_segments(segments: &[Segment]) -> Vec<VideoSegment> {
    let mut result = Vec::with_capacity(segments.len());
    let mut timeline_start = 0.0;
    for segment in segments {
        let speed = segment.speed.unwrap_or(1.0);
        let effective_duration = (segment.original_end - segment.original_start) / speed.max(0.01);
        result.push(VideoSegment {
            start_time: segment.original_start,
            end_time: segment.original_end,
            timeline_start,
            speed,
        });
        timeline_start += effective_duration;
    }
    result
}

/// Returns the video time a timeline position maps to, or `None` when the
/// position falls in a gap — the renderer skips overlays there.
pub fn map_timeline_to_video_time(timeline_time: f64, segments: &[VideoSegment]) -> Option<f64> {
    for segment in segments {
        let speed = if segment.speed == 0.0 {
            1.0
        } else {
            segment.speed
        };
        let effective_duration = (segment.end_time - segment.start_time) / speed;
        let segment_end = segment.timeline_start + effective_duration;
        if timeline_time >= segment.timeline_start && timeline_time < segment_end {
            let offset = timeline_time - segment.timeline_start;
            return Some(segment.start_time + offset * speed);
        }
    }
    None
}

/// The total timeline duration of `segments`, falling back to the source
/// duration when the project has not been split yet.
pub fn total_duration(segments: &[Segment], fallback: f64) -> f64 {
    if segments.is_empty() {
        return fallback.max(0.0);
    }
    segments.iter().map(Segment::timeline_duration).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn segment(start: f64, end: f64, speed: Option<f64>) -> Segment {
        Segment {
            id: format!("{start}-{end}"),
            original_start: start,
            original_end: end,
            trim_min_start: start,
            trim_max_end: end,
            speed,
        }
    }

    #[test]
    fn timeline_starts_accumulate_effective_durations() {
        let converted = to_video_segments(&[
            segment(0.0, 4.0, None),
            segment(10.0, 14.0, Some(2.0)),
            segment(20.0, 21.0, None),
        ]);
        assert_eq!(converted[0].timeline_start, 0.0);
        assert_eq!(converted[1].timeline_start, 4.0);
        assert_eq!(converted[2].timeline_start, 6.0);
    }

    #[test]
    fn a_double_speed_segment_advances_video_time_twice_as_fast() {
        let converted =
            to_video_segments(&[segment(0.0, 4.0, None), segment(10.0, 14.0, Some(2.0))]);
        assert_eq!(map_timeline_to_video_time(1.0, &converted), Some(1.0));
        assert_eq!(map_timeline_to_video_time(5.0, &converted), Some(12.0));
    }

    #[test]
    fn a_position_past_the_last_segment_has_no_video_time() {
        let converted = to_video_segments(&[segment(0.0, 4.0, None)]);
        assert_eq!(map_timeline_to_video_time(4.0, &converted), None);
        assert_eq!(map_timeline_to_video_time(-1.0, &converted), None);
    }

    #[test]
    fn an_empty_timeline_falls_back_to_the_source_duration() {
        assert_eq!(total_duration(&[], 12.5), 12.5);
        assert_eq!(total_duration(&[segment(0.0, 4.0, Some(2.0))], 0.0), 2.0);
    }
}
