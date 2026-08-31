//! Port of `composition/cursor-logic.ts` and `cursor-canvas-renderer.ts` — the
//! recorded pointer, its click bounce, idle fade and motion blur.

use std::sync::Arc;

use tiny_skia::Pixmap;

use crate::render::canvas::Canvas;
use crate::video::composition::cursor_sprites;
use crate::video::composition::segments::{map_timeline_to_video_time, VideoSegment};
use crate::video::sidecars::{CursorData, CursorEvent};
use crate::windows::video_editor::styles::CursorStyle;

/// `LIFE_SIZE_SPRITE_PX` and `MAX_DISPLAY_SCALE`.
const LIFE_SIZE_SPRITE_PX: f64 = 49.0;
const MAX_DISPLAY_SCALE: f64 = 2.5;

const MOTION_BLUR_SAMPLES: usize = 9;
const MOTION_BLUR_SHUTTER: f64 = 0.04;
const MOTION_BLUR_MIN_TRAVEL: f64 = 1.5;

const CLICK_MERGE_THRESHOLD: f64 = 0.3;
const CLICK_ANIMATION_DURATION: f64 = 0.35;

/// `findSurroundingEvents`.
fn surrounding_events(events: &[CursorEvent], timestamp: f64) -> Option<(usize, usize)> {
    if events.is_empty() {
        return None;
    }
    let after = events.partition_point(|event| event.timestamp < timestamp);
    if after == events.len() {
        let last = events.len() - 1;
        return Some((last, last));
    }
    if events[after].timestamp > timestamp {
        if after == 0 {
            return Some((0, 0));
        }
        return Some((after - 1, after));
    }
    Some((after, after))
}

fn lerp_between(events: &[CursorEvent], indices: (usize, usize), timestamp: f64) -> (f64, f64) {
    let before = &events[indices.0];
    let after = &events[indices.1];
    if indices.0 == indices.1 || after.timestamp == before.timestamp {
        return (before.x, before.y);
    }
    let t = (timestamp - before.timestamp) / (after.timestamp - before.timestamp);
    (
        before.x + (after.x - before.x) * t,
        before.y + (after.y - before.y) * t,
    )
}

/// `interpolatePosition` — the plain lerp the zoom follow uses.
pub fn interpolate_position(events: &[CursorEvent], timestamp: f64) -> Option<(f64, f64)> {
    if events.is_empty() {
        return None;
    }
    let indices = surrounding_events(events, timestamp)?;
    Some(lerp_between(events, indices, timestamp))
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MouseState {
    pub is_down: bool,
    pub click_progress: f64,
    pub button: Option<String>,
    pub cursor_type: String,
}

/// `getMouseState`.
pub fn mouse_state(events: &[CursorEvent], timestamp: f64) -> MouseState {
    let mut is_down = false;
    let mut button: Option<String> = None;
    let mut cursor_type = "arrow".to_string();
    let mut last_click: Option<f64> = None;
    let mut sequence_start = 0.0;

    for event in events {
        if event.timestamp > timestamp {
            break;
        }
        if let Some(cursor) = &event.cursor {
            cursor_type.clone_from(cursor);
        }
        if event.kind == "down" {
            is_down = true;
            sequence_start = match last_click {
                Some(previous) if event.timestamp - previous < CLICK_MERGE_THRESHOLD => {
                    sequence_start
                }
                _ => event.timestamp,
            };
            last_click = Some(event.timestamp);
            button = event.button.clone();
        } else if event.kind == "up" {
            is_down = false;
            button = None;
        }
    }

    if last_click.is_none() {
        return MouseState {
            is_down,
            click_progress: 0.0,
            button,
            cursor_type,
        };
    }

    let elapsed = timestamp - sequence_start;
    let click_progress = if elapsed > CLICK_ANIMATION_DURATION {
        0.0
    } else {
        1.0 - elapsed / CLICK_ANIMATION_DURATION
    };

    MouseState {
        is_down,
        click_progress,
        button,
        cursor_type,
    }
}

/// `applySmoothing` — a Gaussian window around the sampled position.
pub fn apply_smoothing(
    events: &[CursorEvent],
    base_index: usize,
    base: (f64, f64),
    timestamp: f64,
    smoothing: f64,
) -> (f64, f64) {
    let window = 0.05 + smoothing * 0.1;
    let window_start = timestamp - window;
    let window_end = timestamp + window * 0.5;
    let sigma = window * 0.5;

    let mut weighted = (0.0, 0.0);
    let mut total_weight = 0.0;
    let start = base_index.saturating_sub(20);
    for event in &events[start..events.len().min(base_index + 11)] {
        if event.timestamp < window_start {
            continue;
        }
        if event.timestamp > window_end {
            break;
        }
        let distance = (event.timestamp - timestamp).abs();
        let weight = (-(distance * distance) / (2.0 * sigma * sigma)).exp();
        weighted.0 += event.x * weight;
        weighted.1 += event.y * weight;
        total_weight += weight;
    }

    if total_weight == 0.0 {
        return base;
    }
    let smoothed = (weighted.0 / total_weight, weighted.1 / total_weight);
    (
        base.0 + (smoothed.0 - base.0) * smoothing,
        base.1 + (smoothed.1 - base.1) * smoothing,
    )
}

#[derive(Clone, Debug, PartialEq)]
pub struct InterpolatedPosition {
    pub x: f64,
    pub y: f64,
    pub click_progress: f64,
    pub cursor_type: String,
}

/// `interpolateCursorPosition`.
pub fn interpolate_cursor_position(
    events: &[CursorEvent],
    timestamp: f64,
    smoothing: f64,
) -> Option<InterpolatedPosition> {
    if events.is_empty() {
        return None;
    }
    let indices = surrounding_events(events, timestamp)?;
    let mut position = lerp_between(events, indices, timestamp);
    if smoothing > 0.0 {
        position = apply_smoothing(events, indices.0, position, timestamp, smoothing);
    }
    let state = mouse_state(events, timestamp);
    Some(InterpolatedPosition {
        x: position.0,
        y: position.1,
        click_progress: state.click_progress,
        cursor_type: state.cursor_type,
    })
}

/// `calculateClickBounceScale`.
pub fn click_bounce_scale(progress: f64) -> f64 {
    if progress <= 0.0 {
        return 1.0;
    }
    let min_scale = 0.7;
    let bell = (std::f64::consts::PI * progress).sin();
    let eased = bell * bell * (3.0 - 2.0 * bell);
    1.0 - (1.0 - min_scale) * eased
}

/// `calculateIdleOpacity`.
pub fn idle_opacity(events: &[CursorEvent], video_time: f64, timeout: f64) -> f64 {
    const FADE_DURATION: f64 = 0.5;
    const MOVEMENT_THRESHOLD: f64 = 0.002;

    let mut last_move_time = 0.0;
    for event in events {
        if event.timestamp > video_time {
            break;
        }
        if matches!(event.kind.as_str(), "move" | "down" | "up") {
            last_move_time = event.timestamp;
        }
    }

    let mut has_moved = false;
    for index in 1..events.len() {
        if events[index].timestamp > video_time {
            break;
        }
        if events[index].timestamp < video_time - timeout - FADE_DURATION {
            continue;
        }
        let dx = events[index].x - events[index - 1].x;
        let dy = events[index].y - events[index - 1].y;
        if (dx * dx + dy * dy).sqrt() > MOVEMENT_THRESHOLD {
            last_move_time = events[index].timestamp;
            has_moved = true;
        }
    }

    if !has_moved && video_time > timeout {
        return 0.0;
    }
    let idle_duration = video_time - last_move_time;
    if idle_duration <= timeout {
        return 1.0;
    }
    1.0 - ((idle_duration - timeout) / FADE_DURATION).min(1.0)
}

/// `resolveCursorSpriteSize` — a recording captured on a HiDPI display draws a
/// proportionally larger pointer, capped so it never dominates the frame.
pub fn sprite_size(size_percent: f64, video_height: f64, recording_height: f64) -> f64 {
    let scale = if recording_height.is_finite() && recording_height > 0.0 {
        video_height / recording_height
    } else {
        1.0
    };
    let display_scale = scale.clamp(1.0, MAX_DISPLAY_SCALE);
    (size_percent / 100.0) * LIFE_SIZE_SPRITE_PX * display_scale
}

pub struct RenderConfig<'a> {
    pub cursor_data: &'a CursorData,
    pub cursor_style: &'a CursorStyle,
    pub segments: &'a [VideoSegment],
    pub video_width: f64,
    pub video_height: f64,
    pub offset_x: f64,
    pub offset_y: f64,
}

/// `getMotionBlurOffset`.
fn motion_blur_offset(
    events: &[CursorEvent],
    video_time: f64,
    smoothing: f64,
    video_width: f64,
    video_height: f64,
    strength: f64,
) -> (f64, f64) {
    let half = (MOTION_BLUR_SHUTTER * strength) / 2.0;
    let start = interpolate_cursor_position(events, (video_time - half).max(0.0), smoothing);
    let end = interpolate_cursor_position(events, video_time + half, smoothing);
    match (start, end) {
        (Some(start), Some(end)) => (
            (end.x - start.x) * video_width,
            (end.y - start.y) * video_height,
        ),
        _ => (0.0, 0.0),
    }
}

/// Port of `renderCursor`.
pub fn render(canvas: &mut Canvas, timeline_time: f64, config: &RenderConfig<'_>) {
    let Some(video_time) = map_timeline_to_video_time(timeline_time, config.segments) else {
        return;
    };
    let events = &config.cursor_data.events;
    let Some(state) =
        interpolate_cursor_position(events, video_time, config.cursor_style.smoothing)
    else {
        return;
    };

    let x = state.x * config.video_width + config.offset_x;
    let y = state.y * config.video_height + config.offset_y;

    let mut opacity = 1.0;
    if config.cursor_style.hide_on_idle {
        opacity = idle_opacity(events, video_time, config.cursor_style.hide_on_idle_timeout);
        if opacity <= 0.0 {
            return;
        }
    }

    let click_scale = if config.cursor_style.show_click_highlight {
        click_bounce_scale(state.click_progress)
    } else {
        1.0
    };
    let size = sprite_size(
        config.cursor_style.size,
        config.video_height,
        config.cursor_data.recording_area.height,
    );
    if size <= 0.0 {
        return;
    }

    // A custom sprite is drawn from its own top-left; the built-in cursors are
    // anchored on their hotspot.
    let (sprite, hotspot) = match config.cursor_style.custom_cursor_image.as_deref() {
        Some(source) => (
            cursor_sprites::decoded_image(source),
            cursor_sprites::Hotspot { x: 0.0, y: 0.0 },
        ),
        None => (
            cursor_sprites::sprite(
                &state.cursor_type,
                &config.cursor_style.color,
                &config.cursor_style.border_color,
                (size * canvas.device_scale() as f64).round().max(1.0) as u32,
            ),
            cursor_sprites::hotspot(&state.cursor_type),
        ),
    };
    let Some(sprite) = sprite else {
        return;
    };

    let (dx, dy) = if config.cursor_style.motion_blur {
        motion_blur_offset(
            events,
            video_time,
            config.cursor_style.smoothing,
            config.video_width,
            config.video_height,
            config.cursor_style.motion_blur_strength,
        )
    } else {
        (0.0, 0.0)
    };

    if dx.hypot(dy) < MOTION_BLUR_MIN_TRAVEL {
        draw_sprite(canvas, &sprite, x, y, size, click_scale, hotspot, opacity);
        return;
    }

    // The blur is a stack of decreasingly opaque copies along the travel
    // vector, matching the renderer's offscreen accumulation.
    canvas.save();
    canvas.set_global_alpha(opacity as f32);
    let center = (MOTION_BLUR_SAMPLES - 1) as f64 / 2.0;
    for index in 0..MOTION_BLUR_SAMPLES {
        let t = (index as f64 - center) / center;
        draw_sprite(
            canvas,
            &sprite,
            x + (dx * t) / 2.0,
            y + (dy * t) / 2.0,
            size,
            click_scale,
            hotspot,
            1.0 / (index + 1) as f64,
        );
    }
    canvas.restore();
}

#[allow(clippy::too_many_arguments)]
fn draw_sprite(
    canvas: &mut Canvas,
    sprite: &Arc<Pixmap>,
    x: f64,
    y: f64,
    size: f64,
    click_scale: f64,
    hotspot: cursor_sprites::Hotspot,
    alpha: f64,
) {
    canvas.save();
    canvas.set_global_alpha((canvas.global_alpha() as f64 * alpha) as f32);
    canvas.translate(x as f32, y as f32);
    canvas.scale(click_scale as f32, click_scale as f32);
    canvas.translate(-(size * hotspot.x) as f32, -(size * hotspot.y) as f32);
    canvas.draw_pixmap(sprite.as_ref().as_ref(), 0.0, 0.0, size as f32, size as f32);
    canvas.restore();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(timestamp: f64, x: f64, y: f64, kind: &str) -> CursorEvent {
        CursorEvent {
            timestamp,
            x,
            y,
            kind: kind.to_string(),
            ..CursorEvent::default()
        }
    }

    #[test]
    fn a_position_between_samples_is_interpolated() {
        let events = [event(0.0, 0.0, 0.0, "move"), event(1.0, 1.0, 0.5, "move")];
        assert_eq!(interpolate_position(&events, 0.5), Some((0.5, 0.25)));
    }

    #[test]
    fn an_exact_duplicate_timestamp_uses_the_first_sample() {
        let events = [event(1.0, 0.2, 0.3, "move"), event(1.0, 0.8, 0.9, "down")];
        assert_eq!(interpolate_position(&events, 1.0), Some((0.2, 0.3)));
    }

    #[test]
    fn a_position_outside_the_samples_clamps_to_the_nearest() {
        let events = [event(1.0, 0.2, 0.3, "move")];
        assert_eq!(interpolate_position(&events, 0.0), Some((0.2, 0.3)));
        assert_eq!(interpolate_position(&events, 9.0), Some((0.2, 0.3)));
        assert_eq!(interpolate_position(&[], 0.0), None);
    }

    #[test]
    fn the_click_bounce_dips_and_recovers() {
        assert_eq!(click_bounce_scale(0.0), 1.0);
        assert!(click_bounce_scale(0.5) < 0.8);
        assert!(click_bounce_scale(0.99) > 0.9);
    }

    #[test]
    fn a_click_sets_the_progress_and_then_decays() {
        let events = [
            event(0.0, 0.0, 0.0, "move"),
            event(1.0, 0.0, 0.0, "down"),
            event(1.05, 0.0, 0.0, "up"),
        ];
        let just_pressed = mouse_state(&events, 1.0);
        assert!(just_pressed.is_down);
        assert!((just_pressed.click_progress - 1.0).abs() < 1e-9);

        let settled = mouse_state(&events, 2.0);
        assert!(!settled.is_down);
        assert_eq!(settled.click_progress, 0.0);
    }

    #[test]
    fn the_cursor_type_is_carried_forward_from_the_last_sample() {
        let mut events = vec![event(0.0, 0.0, 0.0, "move")];
        events[0].cursor = Some("iBeam".into());
        events.push(event(1.0, 0.1, 0.1, "move"));
        assert_eq!(mouse_state(&events, 1.0).cursor_type, "iBeam");
    }

    #[test]
    fn an_idle_pointer_fades_out() {
        let events = [event(0.0, 0.0, 0.0, "move"), event(0.1, 0.5, 0.5, "move")];
        assert_eq!(idle_opacity(&events, 0.1, 2.0), 1.0);
        assert!(idle_opacity(&events, 2.35, 2.0) < 1.0);
        assert_eq!(idle_opacity(&events, 3.0, 2.0), 0.0);
    }

    #[test]
    fn sprite_size_scales_with_the_display_but_is_capped() {
        assert_eq!(sprite_size(100.0, 1080.0, 1080.0), 49.0);
        assert_eq!(sprite_size(50.0, 1080.0, 1080.0), 24.5);
        // A 2x recording downscaled to 1080p never shrinks the pointer.
        assert_eq!(sprite_size(100.0, 1080.0, 2160.0), 49.0);
        // And it never grows past the cap.
        assert_eq!(sprite_size(100.0, 4320.0, 1080.0), 49.0 * 2.5);
    }

    #[test]
    fn smoothing_pulls_the_sample_towards_the_window_mean() {
        let events = [
            event(0.0, 0.0, 0.0, "move"),
            event(0.05, 1.0, 1.0, "move"),
            event(0.1, 0.0, 0.0, "move"),
        ];
        let raw = interpolate_cursor_position(&events, 0.05, 0.0).expect("raw");
        let smoothed = interpolate_cursor_position(&events, 0.05, 1.0).expect("smoothed");
        assert_eq!(raw.x, 1.0);
        assert!(smoothed.x < raw.x, "{} !< {}", smoothed.x, raw.x);
    }

    #[test]
    fn drawing_marks_the_frame_at_the_pointer() {
        let mut canvas = Canvas::new(200, 200).expect("canvas");
        let cursor_data = CursorData {
            recording_area: crate::video::sidecars::Size {
                width: 200.0,
                height: 200.0,
            },
            events: vec![event(0.0, 0.5, 0.5, "move"), event(1.0, 0.5, 0.5, "move")],
            ..CursorData::default()
        };
        let style = CursorStyle {
            motion_blur: false,
            smoothing: 0.0,
            ..CursorStyle::default()
        };
        let segments = [VideoSegment {
            start_time: 0.0,
            end_time: 1.0,
            timeline_start: 0.0,
            speed: 1.0,
        }];
        render(
            &mut canvas,
            0.5,
            &RenderConfig {
                cursor_data: &cursor_data,
                cursor_style: &style,
                segments: &segments,
                video_width: 200.0,
                video_height: 200.0,
                offset_x: 0.0,
                offset_y: 0.0,
            },
        );
        let covered = canvas
            .pixmap()
            .data()
            .chunks_exact(4)
            .filter(|pixel| pixel[3] > 0)
            .count();
        assert!(covered > 50, "{covered}");
    }
}
