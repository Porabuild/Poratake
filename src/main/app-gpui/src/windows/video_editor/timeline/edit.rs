//! Port of `components/video-editor/timeline-split.ts` and the segment
//! operations the timeline's context menus expose: split, delete, move and
//! resize on every track.

use crate::windows::video_editor::model::{
    CameraSegment, DrawingSegment, MusicTrack, Segment, VideoEditorState, ZoomSegment,
};

pub const MIN_SPLIT_DURATION: f64 = 0.1;
/// `MIN_DRAWING_SEGMENT_DURATION` in `types/drawing.ts`.
pub const MIN_DRAWING_SEGMENT_DURATION: f64 = 0.1;

/// Ids are derived from the cut so a split is reproducible; the renderer uses
/// `crypto.randomUUID`, which only has to be unique within the document.
fn split_id(id: &str, cut_time: f64) -> String {
    format!("{id}-{}", (cut_time * 1000.0).round() as i64)
}

fn can_split(start: f64, end: f64, cut_time: f64, minimum: f64) -> bool {
    cut_time > start && cut_time < end && cut_time - start >= minimum && end - cut_time >= minimum
}

/// A track whose clips are plain timeline ranges — zoom, camera and, with its
/// annotations carried along, drawing.
pub trait TimelineRange: Clone {
    fn id(&self) -> &str;
    fn set_id(&mut self, id: String);
    fn start(&self) -> f64;
    fn end(&self) -> f64;
    fn set_range(&mut self, start: f64, end: f64);
    fn minimum_duration() -> f64 {
        MIN_SPLIT_DURATION
    }
}

impl TimelineRange for ZoomSegment {
    fn id(&self) -> &str {
        &self.id
    }
    fn set_id(&mut self, id: String) {
        self.id = id;
    }
    fn start(&self) -> f64 {
        self.start_time
    }
    fn end(&self) -> f64 {
        self.end_time
    }
    fn set_range(&mut self, start: f64, end: f64) {
        self.start_time = start;
        self.end_time = end;
    }
}

impl TimelineRange for CameraSegment {
    fn id(&self) -> &str {
        &self.id
    }
    fn set_id(&mut self, id: String) {
        self.id = id;
    }
    fn start(&self) -> f64 {
        self.start_time
    }
    fn end(&self) -> f64 {
        self.end_time
    }
    fn set_range(&mut self, start: f64, end: f64) {
        self.start_time = start;
        self.end_time = end;
    }
}

impl TimelineRange for DrawingSegment {
    fn id(&self) -> &str {
        &self.id
    }
    fn set_id(&mut self, id: String) {
        self.id = id;
    }
    fn start(&self) -> f64 {
        self.start_time
    }
    fn end(&self) -> f64 {
        self.end_time
    }
    fn set_range(&mut self, start: f64, end: f64) {
        self.start_time = start;
        self.end_time = end;
    }
    fn minimum_duration() -> f64 {
        MIN_DRAWING_SEGMENT_DURATION
    }
}

impl TimelineRange for MusicTrack {
    fn id(&self) -> &str {
        &self.id
    }
    fn set_id(&mut self, id: String) {
        self.id = id;
    }
    fn start(&self) -> f64 {
        self.start_time
    }
    fn end(&self) -> f64 {
        self.end_time
    }
    fn set_range(&mut self, start: f64, end: f64) {
        self.start_time = start;
        self.end_time = end;
    }
}

/// `splitTrackSegments` — splits the first clip the cut falls inside.
pub fn split_ranges<T: TimelineRange>(items: &mut Vec<T>, cut_time: f64) -> bool {
    let Some(index) = items
        .iter()
        .position(|item| can_split(item.start(), item.end(), cut_time, T::minimum_duration()))
    else {
        return false;
    };

    let mut right = items[index].clone();
    let start = items[index].start();
    let end = items[index].end();
    items[index].set_range(start, cut_time);
    right.set_id(split_id(right.id(), cut_time));
    right.set_range(cut_time, end);
    items.insert(index + 1, right);
    true
}

/// `splitMusicTrack` also moves the trim so the right half plays on from where
/// the left half stopped.
pub fn split_music(tracks: &mut Vec<MusicTrack>, cut_time: f64) -> bool {
    let Some(index) = tracks.iter().position(|track| {
        can_split(
            track.start_time,
            track.end_time,
            cut_time,
            MIN_SPLIT_DURATION,
        )
    }) else {
        return false;
    };

    let original = tracks[index].clone();
    tracks[index].end_time = cut_time;
    tracks[index].trim_end = original.trim_end + (original.end_time - cut_time) * original.speed;

    let mut right = original.clone();
    right.id = split_id(&original.id, cut_time);
    right.start_time = cut_time;
    right.trim_start = original.trim_start + (cut_time - original.start_time) * original.speed;
    tracks.insert(index + 1, right);
    true
}

/// `splitVideoSegments`. The cut is in video time, not timeline time.
pub fn split_video(segments: &mut Vec<Segment>, cut_video_time: f64) -> bool {
    let Some(index) = segments.iter().position(|segment| {
        can_split(
            segment.original_start,
            segment.original_end,
            cut_video_time,
            MIN_SPLIT_DURATION,
        )
    }) else {
        return false;
    };

    let original = segments[index].clone();
    segments[index].original_end = cut_video_time;
    segments[index].trim_max_end = cut_video_time;

    segments.insert(
        index + 1,
        Segment {
            id: split_id(&original.id, cut_video_time),
            original_start: cut_video_time,
            original_end: original.original_end,
            trim_min_start: cut_video_time,
            trim_max_end: original.trim_max_end,
            speed: original.speed,
        },
    );
    true
}

/// `handleCutAll` — the cut tool splits every track at once.
pub fn split_all(state: &mut VideoEditorState, cut_time: f64, cut_video_time: f64) -> bool {
    let mut changed = split_video(&mut state.segments, cut_video_time);
    changed |= split_ranges(&mut state.zoom_segments, cut_time);
    changed |= split_ranges(&mut state.camera_segments, cut_time);
    changed |= split_ranges(&mut state.drawing_segments, cut_time);
    changed |= split_music(&mut state.music_tracks, cut_time);
    changed
}

/// Moves a clip, keeping its duration and staying inside `[0, total]`.
pub fn move_range<T: TimelineRange>(items: &mut [T], id: &str, start: f64, total: f64) -> bool {
    let Some(item) = items.iter_mut().find(|item| item.id() == id) else {
        return false;
    };
    let duration = item.end() - item.start();
    let start = start.clamp(0.0, (total - duration).max(0.0));
    item.set_range(start, start + duration);
    true
}

/// Resizes a clip by dragging an edge, keeping at least the track's minimum
/// duration and staying inside `[0, total]`.
pub fn resize_range<T: TimelineRange>(
    items: &mut [T],
    id: &str,
    start: f64,
    end: f64,
    total: f64,
) -> bool {
    let Some(item) = items.iter_mut().find(|item| item.id() == id) else {
        return false;
    };
    let minimum = T::minimum_duration();
    let start = start.clamp(0.0, (total - minimum).max(0.0));
    let end = end.clamp(start + minimum, total.max(start + minimum));
    item.set_range(start, end);
    true
}

pub fn remove_range<T: TimelineRange>(items: &mut Vec<T>, id: &str) -> bool {
    let before = items.len();
    items.retain(|item| item.id() != id);
    items.len() != before
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zoom(id: &str, start: f64, end: f64) -> ZoomSegment {
        ZoomSegment {
            id: id.into(),
            start_time: start,
            end_time: end,
            zoom_level: 1.5,
            ..ZoomSegment::default()
        }
    }

    fn segment(id: &str, start: f64, end: f64) -> Segment {
        Segment {
            id: id.into(),
            original_start: start,
            original_end: end,
            trim_min_start: start,
            trim_max_end: end,
            speed: None,
        }
    }

    #[test]
    fn a_cut_inside_a_clip_splits_it_in_two() {
        let mut segments = vec![zoom("a", 0.0, 4.0)];
        assert!(split_ranges(&mut segments, 1.5));
        assert_eq!(segments.len(), 2);
        assert_eq!((segments[0].start_time, segments[0].end_time), (0.0, 1.5));
        assert_eq!((segments[1].start_time, segments[1].end_time), (1.5, 4.0));
        assert_ne!(segments[0].id, segments[1].id);
        assert_eq!(segments[1].zoom_level, 1.5);
    }

    #[test]
    fn a_cut_too_close_to_an_edge_is_refused() {
        let mut segments = vec![zoom("a", 0.0, 4.0)];
        assert!(!split_ranges(&mut segments, 0.05));
        assert!(!split_ranges(&mut segments, 3.98));
        assert!(!split_ranges(&mut segments, 9.0));
        assert_eq!(segments.len(), 1);
    }

    #[test]
    fn splitting_a_video_segment_keeps_the_trim_bounds_consistent() {
        let mut segments = vec![segment("a", 0.0, 10.0)];
        assert!(split_video(&mut segments, 4.0));
        assert_eq!(segments[0].original_end, 4.0);
        assert_eq!(segments[0].trim_max_end, 4.0);
        assert_eq!(segments[1].original_start, 4.0);
        assert_eq!(segments[1].trim_min_start, 4.0);
        assert_eq!(segments[1].original_end, 10.0);
    }

    #[test]
    fn splitting_a_music_track_moves_the_trim_with_the_cut() {
        let mut tracks = vec![MusicTrack {
            id: "m".into(),
            start_time: 0.0,
            end_time: 4.0,
            original_duration: 8.0,
            trim_start: 1.0,
            trim_end: 3.0,
            speed: 1.0,
            ..MusicTrack::default()
        }];
        assert!(split_music(&mut tracks, 1.0));
        assert_eq!(tracks[0].end_time, 1.0);
        assert_eq!(tracks[0].trim_end, 6.0);
        assert_eq!(tracks[1].start_time, 1.0);
        assert_eq!(tracks[1].trim_start, 2.0);
    }

    #[test]
    fn the_cut_tool_splits_every_track_at_once() {
        let mut state = VideoEditorState {
            segments: vec![segment("v", 0.0, 10.0)],
            zoom_segments: vec![zoom("z", 0.0, 10.0)],
            ..VideoEditorState::default()
        };
        assert!(split_all(&mut state, 4.0, 4.0));
        assert_eq!(state.segments.len(), 2);
        assert_eq!(state.zoom_segments.len(), 2);
    }

    #[test]
    fn a_cut_that_lands_on_no_clip_changes_nothing() {
        let mut state = VideoEditorState {
            segments: vec![segment("v", 0.0, 1.0)],
            ..VideoEditorState::default()
        };
        assert!(!split_all(&mut state, 5.0, 5.0));
        assert_eq!(state.segments.len(), 1);
    }

    #[test]
    fn moving_a_clip_keeps_its_duration_inside_the_timeline() {
        let mut segments = vec![zoom("a", 1.0, 3.0)];
        assert!(move_range(&mut segments, "a", 6.0, 8.0));
        assert_eq!((segments[0].start_time, segments[0].end_time), (6.0, 8.0));

        // Dragging past the end parks it against the end.
        assert!(move_range(&mut segments, "a", 100.0, 8.0));
        assert_eq!((segments[0].start_time, segments[0].end_time), (6.0, 8.0));

        assert!(move_range(&mut segments, "a", -5.0, 8.0));
        assert_eq!((segments[0].start_time, segments[0].end_time), (0.0, 2.0));
        assert!(!move_range(&mut segments, "missing", 1.0, 8.0));
    }

    #[test]
    fn resizing_respects_the_minimum_duration() {
        let mut segments = vec![zoom("a", 0.0, 4.0)];
        assert!(resize_range(&mut segments, "a", 1.0, 1.0, 8.0));
        assert_eq!(segments[0].start_time, 1.0);
        assert_eq!(segments[0].end_time, 1.0 + MIN_SPLIT_DURATION);
    }

    #[test]
    fn removing_a_clip_drops_only_that_one() {
        let mut segments = vec![zoom("a", 0.0, 1.0), zoom("b", 1.0, 2.0)];
        assert!(remove_range(&mut segments, "a"));
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].id, "b");
        assert!(!remove_range(&mut segments, "a"));
    }
}
