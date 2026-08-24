//! Port of `composition/zoom-logic.ts` and `zoom-canvas-renderer.ts` — the
//! auto-zoom transform, including the cursor-following viewport simulation.

use std::collections::HashMap;

use crate::video::composition::cursor;
use crate::video::composition::segments::{map_timeline_to_video_time, VideoSegment};
use crate::video::sidecars::CursorData;
use crate::windows::video_editor::model::ZoomSegment;
use crate::windows::video_editor::styles::ZoomSettings;

/// `BOUNDING_RATIO` in `types/zoom.ts`.
const BOUNDING_RATIO: f64 = 0.5;
const KEYFRAME_INTERVAL: f64 = 0.1;
const EDGE_MARGIN: f64 = 0.05;
const MAX_SPEED: f64 = 2.0;
const CURSOR_SAMPLE_RATE: f64 = 30.0;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub scale: f64,
    pub translate_x: f64,
    pub translate_y: f64,
    pub viewport: Option<Position>,
}

impl Transform {
    pub fn identity() -> Self {
        Self {
            scale: 1.0,
            translate_x: 0.0,
            translate_y: 0.0,
            viewport: None,
        }
    }

    pub fn is_identity(&self) -> bool {
        self.scale == 1.0
    }
}

#[derive(Clone, Debug)]
pub struct ZoomState {
    pub scale: f64,
    pub is_zooming: bool,
    pub segment: Option<ZoomSegment>,
    pub transition_progress: f64,
    pub effective_transition_in: f64,
    pub effective_transition_out: f64,
    pub is_transitioning_in: bool,
    pub is_transitioning_out: bool,
    pub zoom_out_progress: f64,
}

impl ZoomState {
    fn idle() -> Self {
        Self {
            scale: 1.0,
            is_zooming: false,
            segment: None,
            transition_progress: 0.0,
            effective_transition_in: 0.0,
            effective_transition_out: 0.0,
            is_transitioning_in: false,
            is_transitioning_out: false,
            zoom_out_progress: 0.0,
        }
    }
}

fn clamp(value: f64, low: f64, high: f64) -> f64 {
    value.max(low).min(high)
}

/// `applyEasing` — the ease-in-out curve every zoom transition uses.
fn apply_easing(t: f64) -> f64 {
    if t < 0.5 {
        2.0 * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
    }
}

/// Port of `getZoomState`.
pub fn zoom_state(
    timeline_time: f64,
    zoom_segments: &[ZoomSegment],
    settings: &ZoomSettings,
) -> ZoomState {
    for segment in zoom_segments {
        let duration = segment.end_time - segment.start_time;
        let transition_in = segment
            .transition_in_duration
            .unwrap_or(settings.transition_in_duration);
        let transition_out = segment
            .transition_out_duration
            .unwrap_or(settings.transition_out_duration);
        let effective_transition_in = transition_in.min(duration / 2.0);
        let effective_transition_out = transition_out.min(duration / 2.0);

        if timeline_time < segment.start_time || timeline_time > segment.end_time {
            continue;
        }

        let time_into_segment = timeline_time - segment.start_time;
        let time_from_end = segment.end_time - timeline_time;

        if time_into_segment < effective_transition_in {
            let progress = time_into_segment / effective_transition_in;
            return ZoomState {
                scale: 1.0 + (segment.zoom_level - 1.0) * apply_easing(progress),
                is_zooming: true,
                segment: Some(segment.clone()),
                transition_progress: progress,
                effective_transition_in,
                effective_transition_out,
                is_transitioning_in: true,
                is_transitioning_out: false,
                zoom_out_progress: 0.0,
            };
        }

        if time_from_end < effective_transition_out {
            let progress = time_from_end / effective_transition_out;
            return ZoomState {
                scale: 1.0 + (segment.zoom_level - 1.0) * apply_easing(progress),
                is_zooming: true,
                segment: Some(segment.clone()),
                transition_progress: 1.0,
                effective_transition_in,
                effective_transition_out,
                is_transitioning_in: false,
                is_transitioning_out: true,
                zoom_out_progress: apply_easing(1.0 - progress),
            };
        }

        return ZoomState {
            scale: segment.zoom_level,
            is_zooming: true,
            segment: Some(segment.clone()),
            transition_progress: 1.0,
            effective_transition_in,
            effective_transition_out,
            is_transitioning_in: false,
            is_transitioning_out: false,
            zoom_out_progress: 0.0,
        };
    }

    ZoomState::idle()
}

/// `getCursorAtTime`.
fn cursor_at_time(
    cursor_data: &CursorData,
    video_segments: &[VideoSegment],
    timeline_time: f64,
) -> Option<Position> {
    let video_time = map_timeline_to_video_time(timeline_time, video_segments)?;
    cursor::interpolate_position(&cursor_data.events, video_time).map(|(x, y)| Position { x, y })
}

/// `calculateOptimalCenter` — the viewport the segment starts from.
pub fn optimal_center(
    cursor_data: &CursorData,
    video_segments: &[VideoSegment],
    segment: &ZoomSegment,
    viewport_size: f64,
    transition_in_duration: f64,
) -> Position {
    let duration = segment.end_time - segment.start_time;
    let interval = 1.0 / CURSOR_SAMPLE_RATE;
    let mut positions = Vec::new();
    let mut offset = 0.0;
    while offset <= duration {
        if let Some(position) =
            cursor_at_time(cursor_data, video_segments, segment.start_time + offset)
        {
            positions.push(position);
        }
        offset += interval;
    }

    if positions.is_empty() {
        return Position { x: 0.5, y: 0.5 };
    }

    let mut min = Position {
        x: f64::MAX,
        y: f64::MAX,
    };
    let mut max = Position {
        x: f64::MIN,
        y: f64::MIN,
    };
    for position in &positions {
        min.x = min.x.min(position.x);
        min.y = min.y.min(position.y);
        max.x = max.x.max(position.x);
        max.y = max.y.max(position.y);
    }

    let bounding_size = viewport_size * BOUNDING_RATIO;
    let transition_end = segment.start_time + transition_in_duration;
    let at_transition_end = cursor_at_time(cursor_data, video_segments, transition_end);

    let center_x = if max.x - min.x <= bounding_size {
        (min.x + max.x) / 2.0
    } else {
        at_transition_end.map_or(positions[0].x, |position| position.x)
    };
    let center_y = if max.y - min.y <= bounding_size {
        (min.y + max.y) / 2.0
    } else {
        at_transition_end.map_or(positions[0].y, |position| position.y)
    };

    let max_viewport = 1.0 - viewport_size;
    Position {
        x: clamp(center_x - viewport_size / 2.0, 0.0, max_viewport),
        y: clamp(center_y - viewport_size / 2.0, 0.0, max_viewport),
    }
}

fn is_cursor_in_bounding_area(cursor: Position, viewport: Position, viewport_size: f64) -> bool {
    let bounding_size = viewport_size * BOUNDING_RATIO;
    let margin = (viewport_size - bounding_size) / 2.0;
    cursor.x >= viewport.x + margin
        && cursor.x <= viewport.x + viewport_size - margin
        && cursor.y >= viewport.y + margin
        && cursor.y <= viewport.y + viewport_size - margin
}

fn follow_target(cursor: Position, viewport: Position, viewport_size: f64) -> Position {
    let bounding_size = viewport_size * BOUNDING_RATIO;
    let margin = (viewport_size - bounding_size) / 2.0;
    let max_viewport = 1.0 - viewport_size;

    let mut target = viewport;
    if cursor.x < viewport.x + margin {
        target.x = cursor.x - margin;
    } else if cursor.x > viewport.x + viewport_size - margin {
        target.x = cursor.x - viewport_size + margin;
    }
    if cursor.y < viewport.y + margin {
        target.y = cursor.y - margin;
    } else if cursor.y > viewport.y + viewport_size - margin {
        target.y = cursor.y - viewport_size + margin;
    }
    Position {
        x: clamp(target.x, 0.0, max_viewport),
        y: clamp(target.y, 0.0, max_viewport),
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct SmoothState {
    position: Position,
    velocity_x: f64,
    velocity_y: f64,
}

/// `smoothDamp` — the critically damped spring the follow uses.
fn smooth_damp(current: f64, target: f64, velocity: f64, smooth_time: f64, dt: f64) -> (f64, f64) {
    let omega = 2.0 / smooth_time;
    let x = omega * dt;
    let exponential = 1.0 / (1.0 + x + 0.48 * x * x + 0.235 * x * x * x);

    let max_delta = MAX_SPEED * smooth_time;
    let delta = clamp(current - target, -max_delta, max_delta);

    let temp = (velocity + omega * delta) * dt;
    let mut new_velocity = (velocity - omega * temp) * exponential;
    let mut new_value = target + (delta + temp) * exponential;

    if (target - current > 0.0) == (new_value > target) {
        new_value = target;
        new_velocity = (new_value - current) / dt;
    }
    (new_value, new_velocity)
}

fn smooth_damp_position(
    state: SmoothState,
    target: Position,
    smooth_time: f64,
    dt: f64,
) -> SmoothState {
    let (x, velocity_x) = smooth_damp(
        state.position.x,
        target.x,
        state.velocity_x,
        smooth_time,
        dt,
    );
    let (y, velocity_y) = smooth_damp(
        state.position.y,
        target.y,
        state.velocity_y,
        smooth_time,
        dt,
    );
    SmoothState {
        position: Position { x, y },
        velocity_x,
        velocity_y,
    }
}

fn is_cursor_near_edge(cursor: Position, viewport: Position, viewport_size: f64) -> bool {
    let margin = viewport_size * EDGE_MARGIN;
    cursor.x < viewport.x + margin
        || cursor.x > viewport.x + viewport_size - margin
        || cursor.y < viewport.y + margin
        || cursor.y > viewport.y + viewport_size - margin
}

fn is_cursor_outside_viewport(cursor: Position, viewport: Position, viewport_size: f64) -> bool {
    cursor.x < viewport.x
        || cursor.x > viewport.x + viewport_size
        || cursor.y < viewport.y
        || cursor.y > viewport.y + viewport_size
}

#[derive(Clone, Copy, Debug)]
struct Keyframe {
    time: f64,
    x: f64,
    y: f64,
}

/// `generateViewportKeyframes`.
fn generate_keyframes(
    cursor_data: &CursorData,
    video_segments: &[VideoSegment],
    segment: &ZoomSegment,
    viewport_size: f64,
    start_viewport: Position,
    follow_smoothness: f64,
    look_ahead: f64,
) -> Vec<Keyframe> {
    let max_viewport = 1.0 - viewport_size;
    let duration = segment.end_time - segment.start_time;
    let mut state = SmoothState {
        position: Position {
            x: clamp(start_viewport.x, 0.0, max_viewport),
            y: clamp(start_viewport.y, 0.0, max_viewport),
        },
        velocity_x: 0.0,
        velocity_y: 0.0,
    };

    let mut keyframes = vec![Keyframe {
        time: segment.start_time,
        x: state.position.x,
        y: state.position.y,
    }];

    let steps = (duration / KEYFRAME_INTERVAL).ceil() as i64;
    for step in 1..=steps {
        let time = segment.start_time + step as f64 * KEYFRAME_INTERVAL;
        if time > segment.end_time {
            break;
        }
        let Some(cursor) = cursor_at_time(
            cursor_data,
            video_segments,
            (time + look_ahead).min(segment.end_time),
        ) else {
            keyframes.push(Keyframe {
                time,
                x: state.position.x,
                y: state.position.y,
            });
            continue;
        };

        let viewport = state.position;
        let outside = is_cursor_outside_viewport(cursor, viewport, viewport_size);
        let near_edge = is_cursor_near_edge(cursor, viewport, viewport_size);
        let in_bounds = is_cursor_in_bounding_area(cursor, viewport, viewport_size);

        if outside || near_edge || !in_bounds {
            let target = follow_target(cursor, viewport, viewport_size);
            let smooth_time = if outside {
                follow_smoothness * 0.27
            } else if near_edge {
                follow_smoothness * 0.5
            } else {
                follow_smoothness
            };
            state = smooth_damp_position(state, target, smooth_time.max(0.04), KEYFRAME_INTERVAL);
        } else {
            state.velocity_x *= 0.8;
            state.velocity_y *= 0.8;
        }

        keyframes.push(Keyframe {
            time,
            x: clamp(state.position.x, 0.0, max_viewport),
            y: clamp(state.position.y, 0.0, max_viewport),
        });
    }
    keyframes
}

fn catmull_rom(p0: f64, p1: f64, p2: f64, p3: f64, t: f64) -> f64 {
    let t2 = t * t;
    let t3 = t2 * t;
    0.5 * (2.0 * p1
        + (-p0 + p2) * t
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
        + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3)
}

fn interpolate_keyframes(keyframes: &[Keyframe], time: f64) -> Position {
    if keyframes.is_empty() {
        return Position { x: 0.5, y: 0.5 };
    }
    if time <= keyframes[0].time {
        return Position {
            x: keyframes[0].x,
            y: keyframes[0].y,
        };
    }
    let last = keyframes[keyframes.len() - 1];
    if time >= last.time {
        return Position {
            x: last.x,
            y: last.y,
        };
    }

    let mut low = 0usize;
    let mut high = keyframes.len() - 1;
    while low < high - 1 {
        let middle = (low + high) / 2;
        if keyframes[middle].time <= time {
            low = middle;
        } else {
            high = middle;
        }
    }

    let t = (time - keyframes[low].time) / (keyframes[high].time - keyframes[low].time);
    let p0 = keyframes[low.saturating_sub(1)];
    let p1 = keyframes[low];
    let p2 = keyframes[high];
    let p3 = keyframes[(high + 1).min(keyframes.len() - 1)];

    Position {
        x: catmull_rom(p0.x, p1.x, p2.x, p3.x, t),
        y: catmull_rom(p0.y, p1.y, p2.y, p3.y, t),
    }
}

/// The per-segment keyframe and optimal-centre caches the renderer keeps in
/// module scope. Simulating a segment walks its whole duration, so this is what
/// keeps scrubbing cheap.
#[derive(Default)]
pub struct ZoomCache {
    keyframes: HashMap<String, Vec<Keyframe>>,
    centers: HashMap<String, Position>,
}

impl ZoomCache {
    pub fn clear(&mut self) {
        self.keyframes.clear();
        self.centers.clear();
    }

    fn center(
        &mut self,
        cursor_data: &CursorData,
        video_segments: &[VideoSegment],
        segment: &ZoomSegment,
        viewport_size: f64,
        transition_in_duration: f64,
    ) -> Position {
        let key = format!(
            "{}-{viewport_size:.4}-{transition_in_duration:.2}",
            segment.id
        );
        if let Some(cached) = self.centers.get(&key) {
            return *cached;
        }
        let center = optimal_center(
            cursor_data,
            video_segments,
            segment,
            viewport_size,
            transition_in_duration,
        );
        self.centers.insert(key, center);
        center
    }

    #[allow(clippy::too_many_arguments)]
    fn viewport(
        &mut self,
        cursor_data: &CursorData,
        video_segments: &[VideoSegment],
        segment: &ZoomSegment,
        current_time: f64,
        viewport_size: f64,
        start_viewport: Position,
        follow_smoothness: f64,
        look_ahead: f64,
    ) -> Position {
        let key = format!(
            "{}-{viewport_size:.4}-{follow_smoothness:.2}-{look_ahead:.2}",
            segment.id
        );
        if !self.keyframes.contains_key(&key) {
            let keyframes = generate_keyframes(
                cursor_data,
                video_segments,
                segment,
                viewport_size,
                start_viewport,
                follow_smoothness,
                look_ahead,
            );
            self.keyframes.insert(key.clone(), keyframes);
        }
        let keyframes = &self.keyframes[&key];
        let interpolated = interpolate_keyframes(keyframes, current_time);
        let max_viewport = 1.0 - viewport_size;
        Position {
            x: clamp(interpolated.x, 0.0, max_viewport),
            y: clamp(interpolated.y, 0.0, max_viewport),
        }
    }
}

/// `calculateManualFocusViewport`.
fn manual_focus_viewport(state: &ZoomState, viewport_size: f64, max_viewport: f64) -> Position {
    let Some(segment) = state.segment.as_ref() else {
        return Position::default();
    };
    let focus = segment
        .focus_point
        .unwrap_or(crate::windows::video_editor::model::FocusPoint { x: 0.5, y: 0.5 });
    let full_viewport_size = 1.0 / segment.zoom_level;
    let target_x = clamp(
        focus.x - full_viewport_size / 2.0,
        0.0,
        1.0 - full_viewport_size,
    );
    let target_y = clamp(
        focus.y - full_viewport_size / 2.0,
        0.0,
        1.0 - full_viewport_size,
    );
    transition_viewport(
        state,
        Position {
            x: target_x,
            y: target_y,
        },
        full_viewport_size,
        viewport_size,
        max_viewport,
    )
}

/// The shared transition maths: while zooming in the target viewport is scaled
/// by how far the zoom has come, and while zooming out its centre eases back to
/// the middle of the frame.
fn transition_viewport(
    state: &ZoomState,
    target: Position,
    full_viewport_size: f64,
    viewport_size: f64,
    max_viewport: f64,
) -> Position {
    if state.is_transitioning_in {
        let eased = apply_easing(state.transition_progress);
        let scale = viewport_size / full_viewport_size;
        return Position {
            x: clamp(target.x * scale * eased, 0.0, max_viewport),
            y: clamp(target.y * scale * eased, 0.0, max_viewport),
        };
    }
    if state.is_transitioning_out {
        let progress = state.zoom_out_progress;
        let start_center_x = target.x + full_viewport_size / 2.0;
        let start_center_y = target.y + full_viewport_size / 2.0;
        let center_x = start_center_x + (0.5 - start_center_x) * progress;
        let center_y = start_center_y + (0.5 - start_center_y) * progress;
        return Position {
            x: clamp(center_x - viewport_size / 2.0, 0.0, max_viewport),
            y: clamp(center_y - viewport_size / 2.0, 0.0, max_viewport),
        };
    }
    Position {
        x: clamp(target.x, 0.0, max_viewport),
        y: clamp(target.y, 0.0, max_viewport),
    }
}

/// `calculateCursorFollowViewport`.
#[allow(clippy::too_many_arguments)]
fn cursor_follow_viewport(
    cache: &mut ZoomCache,
    state: &ZoomState,
    cursor_data: Option<&CursorData>,
    video_segments: &[VideoSegment],
    timeline_time: f64,
    viewport_size: f64,
    max_viewport: f64,
    settings: &ZoomSettings,
) -> Position {
    let Some(segment) = state.segment.as_ref() else {
        return Position::default();
    };
    let full_viewport_size = 1.0 / segment.zoom_level;

    let Some(cursor_data) = cursor_data.filter(|data| !data.is_empty()) else {
        let centered = clamp(
            0.5 - full_viewport_size / 2.0,
            0.0,
            1.0 - full_viewport_size,
        );
        return transition_viewport(
            state,
            Position {
                x: centered,
                y: centered,
            },
            full_viewport_size,
            viewport_size,
            max_viewport,
        );
    };

    let start_viewport = cache.center(
        cursor_data,
        video_segments,
        segment,
        full_viewport_size,
        state.effective_transition_in,
    );
    let query_time = if state.is_transitioning_out {
        segment.end_time - state.effective_transition_out
    } else {
        timeline_time
    };
    let viewport = cache.viewport(
        cursor_data,
        video_segments,
        segment,
        query_time,
        full_viewport_size,
        start_viewport,
        settings.follow_smoothness,
        settings.look_ahead,
    );

    transition_viewport(
        state,
        viewport,
        full_viewport_size,
        viewport_size,
        max_viewport,
    )
}

/// Port of `calculateZoomTransform`.
#[allow(clippy::too_many_arguments)]
pub fn calculate_transform(
    cache: &mut ZoomCache,
    zoom_segments: &[ZoomSegment],
    settings: Option<&ZoomSettings>,
    cursor_data: Option<&CursorData>,
    video_segments: &[VideoSegment],
    timeline_time: f64,
    video_width: f64,
    video_height: f64,
) -> Transform {
    let Some(settings) = settings else {
        return Transform::identity();
    };
    if zoom_segments.is_empty() {
        return Transform::identity();
    }

    let state = zoom_state(timeline_time, zoom_segments, settings);
    if !state.is_zooming || state.scale == 1.0 || state.segment.is_none() {
        return Transform::identity();
    }

    let viewport_size = 1.0 / state.scale;
    let max_viewport = 1.0 - viewport_size;
    let is_manual = state
        .segment
        .as_ref()
        .and_then(|segment| segment.target_mode.as_deref())
        == Some("manual");

    let viewport = if is_manual {
        manual_focus_viewport(&state, viewport_size, max_viewport)
    } else {
        cursor_follow_viewport(
            cache,
            &state,
            cursor_data,
            video_segments,
            timeline_time,
            viewport_size,
            max_viewport,
            settings,
        )
    };

    Transform {
        scale: state.scale,
        translate_x: -viewport.x * video_width * state.scale,
        translate_y: -viewport.y * video_height * state.scale,
        viewport: Some(viewport),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn segment(zoom_level: f64) -> ZoomSegment {
        ZoomSegment {
            id: "zoom-1".into(),
            start_time: 1.0,
            end_time: 5.0,
            zoom_level,
            target_mode: Some("manual".into()),
            transition_in_duration: Some(1.0),
            transition_out_duration: Some(1.0),
            focus_point: None,
        }
    }

    #[test]
    fn outside_a_segment_the_transform_is_identity() {
        let mut cache = ZoomCache::default();
        let transform = calculate_transform(
            &mut cache,
            &[segment(2.0)],
            Some(&ZoomSettings::default()),
            None,
            &[],
            0.5,
            1920.0,
            1080.0,
        );
        assert!(transform.is_identity());
    }

    #[test]
    fn the_scale_eases_in_and_holds_at_the_segment_level() {
        let settings = ZoomSettings::default();
        let segments = [segment(2.0)];

        let entering = zoom_state(1.25, &segments, &settings);
        assert!(entering.is_transitioning_in);
        assert!(entering.scale > 1.0 && entering.scale < 2.0);

        let held = zoom_state(3.0, &segments, &settings);
        assert_eq!(held.scale, 2.0);
        assert!(!held.is_transitioning_in && !held.is_transitioning_out);

        let leaving = zoom_state(4.75, &segments, &settings);
        assert!(leaving.is_transitioning_out);
        assert!(leaving.scale > 1.0 && leaving.scale < 2.0);
    }

    #[test]
    fn a_transition_never_exceeds_half_the_segment() {
        let settings = ZoomSettings::default();
        let short = ZoomSegment {
            start_time: 0.0,
            end_time: 1.0,
            transition_in_duration: Some(10.0),
            transition_out_duration: Some(10.0),
            ..segment(2.0)
        };
        let state = zoom_state(0.25, &[short], &settings);
        assert_eq!(state.effective_transition_in, 0.5);
        assert_eq!(state.effective_transition_out, 0.5);
    }

    #[test]
    fn a_manual_focus_point_drives_the_translation() {
        let mut cache = ZoomCache::default();
        let focused = ZoomSegment {
            focus_point: Some(crate::windows::video_editor::model::FocusPoint { x: 1.0, y: 1.0 }),
            ..segment(2.0)
        };
        let transform = calculate_transform(
            &mut cache,
            &[focused],
            Some(&ZoomSettings::default()),
            None,
            &[],
            3.0,
            1000.0,
            1000.0,
        );
        assert_eq!(transform.scale, 2.0);
        // A bottom-right focus pans the frame up and left by a full viewport.
        assert_eq!(transform.translate_x, -1000.0);
        assert_eq!(transform.translate_y, -1000.0);
        assert_eq!(transform.viewport, Some(Position { x: 0.5, y: 0.5 }));
    }

    #[test]
    fn with_no_cursor_data_the_follow_mode_centres_the_viewport() {
        let mut cache = ZoomCache::default();
        let following = ZoomSegment {
            target_mode: Some("cursor".into()),
            ..segment(2.0)
        };
        let transform = calculate_transform(
            &mut cache,
            &[following],
            Some(&ZoomSettings::default()),
            None,
            &[],
            3.0,
            1000.0,
            1000.0,
        );
        assert_eq!(transform.viewport, Some(Position { x: 0.25, y: 0.25 }));
    }

    #[test]
    fn smooth_damp_converges_on_its_target() {
        let mut value = 0.0;
        let mut velocity = 0.0;
        for _ in 0..200 {
            let (next, next_velocity) = smooth_damp(value, 1.0, velocity, 0.3, 0.1);
            value = next;
            velocity = next_velocity;
        }
        assert!((value - 1.0).abs() < 1e-6, "{value}");
    }

    #[test]
    fn keyframe_interpolation_clamps_to_the_ends() {
        let keyframes = vec![
            Keyframe {
                time: 0.0,
                x: 0.1,
                y: 0.2,
            },
            Keyframe {
                time: 1.0,
                x: 0.3,
                y: 0.4,
            },
        ];
        assert_eq!(interpolate_keyframes(&keyframes, -1.0).x, 0.1);
        assert_eq!(interpolate_keyframes(&keyframes, 9.0).y, 0.4);
    }
}
