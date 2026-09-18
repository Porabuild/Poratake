pub mod controls;
pub mod edit;
pub mod menu;
pub mod pane;
pub mod reorder;
pub mod ruler;
pub mod tracks;

pub const DEFAULT_PIXELS_PER_SECOND: f32 = 100.0;
pub const MIN_PIXELS_PER_SECOND: f32 = 10.0;
pub const MAX_PIXELS_PER_SECOND: f32 = 500.0;
pub const ZOOM_STEP: f32 = 1.25;
pub const TRACK_HEIGHT: f32 = 24.0;
pub const TRACK_GUTTER_WIDTH: f32 = 40.0;
pub const RULER_HEIGHT: f32 = 28.0;

pub const MIN_PANE_TRACKS: f32 = 3.0;
pub const MAX_PANE_TRACKS: f32 = 12.0;
pub const PANE_SCROLLBAR: f32 = 12.0;
const WHEEL_ZOOM_SENSITIVITY: f32 = 0.01;
pub const SCRUB_STEP: f64 = 1.0 / 120.0;
pub const SCROLL_MARGIN: f32 = 100.0;

pub const SPEED_PRESETS: [f64; 8] = [0.5, 0.75, 1.0, 1.25, 1.5, 2.0, 3.0, 4.0];

pub fn format_speed(speed: f64) -> String {
    format!("{}x", (speed * 100.0).round() / 100.0)
}

pub fn format_zoom_level(level: f64) -> String {
    if level.fract() == 0.0 {
        return format!("{}x", level as i64);
    }
    format!("{}x", (level * 10.0).round() / 10.0)
}

pub fn step_speed(speed: f64, delta: isize) -> f64 {
    let index = SPEED_PRESETS
        .iter()
        .position(|preset| (preset - speed).abs() < f64::EPSILON)
        .unwrap_or_else(|| {
            SPEED_PRESETS
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| {
                    (*a - speed)
                        .abs()
                        .partial_cmp(&(*b - speed).abs())
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(index, _)| index)
                .unwrap_or(0)
        });
    let next = (index as isize + delta).clamp(0, SPEED_PRESETS.len() as isize - 1);
    SPEED_PRESETS[next as usize]
}

pub fn pane_height(tracks: f32) -> f32 {
    tracks * TRACK_HEIGHT + PANE_SCROLLBAR
}

pub fn clamp_pane_height(height: f32) -> f32 {
    height.clamp(pane_height(MIN_PANE_TRACKS), pane_height(MAX_PANE_TRACKS))
}

pub fn default_pane_height() -> f32 {
    crate::ui::chrome::video_timeline_tracks_height(TRACK_HEIGHT)
}

pub fn wheel_zoom(
    pixels_per_second: f32,
    delta_y: f32,
    pointer_offset: f32,
    scroll_left: f32,
) -> (f32, f32) {
    let time_at_pointer = (pointer_offset + scroll_left) / pixels_per_second.max(0.01);
    let next = clamp_zoom(pixels_per_second * (1.0 - delta_y * WHEEL_ZOOM_SENSITIVITY));
    (next, (time_at_pointer * next - pointer_offset).max(0.0))
}

pub fn clamp_scroll_left(scroll_left: f32, content_width: f32, viewport_width: f32) -> f32 {
    scroll_left.clamp(0.0, (content_width - viewport_width).max(0.0))
}

pub fn autoscroll_left(playhead_pixels: f32, scroll_left: f32, viewport_width: f32) -> Option<f32> {
    if viewport_width <= 0.0 {
        return None;
    }
    let visible_right = scroll_left + viewport_width;
    if playhead_pixels <= visible_right - SCROLL_MARGIN {
        return None;
    }
    Some((playhead_pixels - viewport_width + SCROLL_MARGIN).max(0.0))
}

pub fn quantize_scrub(time: f64) -> f64 {
    (time.max(0.0) / SCRUB_STEP).floor() * SCRUB_STEP
}

pub fn clamp_zoom(pixels_per_second: f32) -> f32 {
    pixels_per_second.clamp(MIN_PIXELS_PER_SECOND, MAX_PIXELS_PER_SECOND)
}

/// Port of `useTimelineZoom`'s fit-to-view: pick the scale that shows the whole
/// timeline in the available width.
pub fn fit_zoom(total_duration: f64, available_width: f32) -> f32 {
    if total_duration <= 0.0 || available_width <= 0.0 {
        return DEFAULT_PIXELS_PER_SECOND;
    }
    clamp_zoom(available_width / total_duration as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamps_zoom_to_the_renderer_bounds() {
        assert_eq!(clamp_zoom(1.0), MIN_PIXELS_PER_SECOND);
        assert_eq!(clamp_zoom(10_000.0), MAX_PIXELS_PER_SECOND);
        assert_eq!(clamp_zoom(120.0), 120.0);
    }

    #[test]
    fn fits_the_whole_timeline_into_the_available_width() {
        assert_eq!(fit_zoom(10.0, 1000.0), 100.0);
        assert_eq!(fit_zoom(0.0, 1000.0), DEFAULT_PIXELS_PER_SECOND);
        assert_eq!(fit_zoom(1000.0, 100.0), MIN_PIXELS_PER_SECOND);
    }

    #[test]
    fn the_pane_resizes_between_three_and_twelve_track_rows() {
        assert_eq!(clamp_pane_height(0.0), 3.0 * TRACK_HEIGHT + PANE_SCROLLBAR);
        assert_eq!(
            clamp_pane_height(10_000.0),
            12.0 * TRACK_HEIGHT + PANE_SCROLLBAR
        );
        assert_eq!(clamp_pane_height(150.0), 150.0);
        assert_eq!(default_pane_height(), 5.0 * TRACK_HEIGHT + PANE_SCROLLBAR);
    }

    #[test]
    fn a_wheel_zoom_keeps_the_time_under_the_pointer_in_place() {
        let (next, scroll_left) = wheel_zoom(100.0, -100.0, 200.0, 0.0);
        assert_eq!(next, 200.0);
        assert_eq!(scroll_left, 200.0);

        let (out, _) = wheel_zoom(100.0, 50.0, 0.0, 0.0);
        assert_eq!(out, 50.0);
        assert_eq!(wheel_zoom(100.0, 100.0, 0.0, 0.0).0, MIN_PIXELS_PER_SECOND);
        assert_eq!(
            wheel_zoom(MAX_PIXELS_PER_SECOND, -1000.0, 0.0, 0.0).0,
            MAX_PIXELS_PER_SECOND
        );
        assert_eq!(
            wheel_zoom(MIN_PIXELS_PER_SECOND, 1000.0, 0.0, 0.0).0,
            MIN_PIXELS_PER_SECOND
        );
    }

    #[test]
    fn horizontal_scrolling_stops_at_both_ends_of_the_content() {
        assert_eq!(clamp_scroll_left(-40.0, 1000.0, 400.0), 0.0);
        assert_eq!(clamp_scroll_left(5000.0, 1000.0, 400.0), 600.0);
        assert_eq!(clamp_scroll_left(120.0, 1000.0, 400.0), 120.0);
        assert_eq!(clamp_scroll_left(120.0, 100.0, 400.0), 0.0);
    }

    #[test]
    fn playback_scrolls_once_the_playhead_reaches_the_margin() {
        assert_eq!(autoscroll_left(100.0, 0.0, 400.0), None);
        assert_eq!(autoscroll_left(320.0, 0.0, 400.0), Some(20.0));
        assert_eq!(autoscroll_left(320.0, 0.0, 0.0), None);
    }

    #[test]
    fn speed_steps_through_the_renderer_presets_and_stops_at_both_ends() {
        assert_eq!(step_speed(1.0, 1), 1.25);
        assert_eq!(step_speed(1.0, -1), 0.75);
        assert_eq!(step_speed(0.5, -1), 0.5);
        assert_eq!(step_speed(4.0, 1), 4.0);
        assert_eq!(step_speed(0.9, 1), 1.25);
    }

    #[test]
    fn zoom_levels_drop_a_trailing_zero_the_way_the_renderer_does() {
        assert_eq!(format_zoom_level(2.0), "2x");
        assert_eq!(format_zoom_level(1.5), "1.5x");
        assert_eq!(format_zoom_level(1.25), "1.3x");
        assert_eq!(format_speed(2.0), "2x");
        assert_eq!(format_speed(0.75), "0.75x");
    }

    #[test]
    fn the_hover_scrub_lands_on_the_frame_grid() {
        assert_eq!(quantize_scrub(-1.0), 0.0);
        assert_eq!(quantize_scrub(SCRUB_STEP * 3.5), SCRUB_STEP * 3.0);
    }
}

/// The timeline position a pointer at `x` (window space) sits over, given the
/// lane's scroll container. Both the ruler and the track lanes scrub with this,
/// so clicking either lands the playhead on the same frame.
pub fn time_at_position(
    x: gpui::Pixels,
    scroll: &gpui::ScrollHandle,
    pixels_per_second: f32,
    total_duration: f64,
) -> f64 {
    let bounds = scroll.bounds();
    let offset = scroll.offset().x;
    let local = f32::from(x - bounds.left() - offset);
    ((local / pixels_per_second.max(0.01)) as f64).clamp(0.0, total_duration.max(0.0))
}

#[cfg(test)]
mod scrub_tests {
    use super::*;
    use gpui::{px, ScrollHandle};
    use herogpui::gpui;

    #[test]
    fn an_unscrolled_lane_maps_pixels_to_seconds() {
        let scroll = ScrollHandle::new();
        // With no laid-out bounds the origin is zero, which is the case the
        // conversion has to stay well defined for.
        assert_eq!(time_at_position(px(120.0), &scroll, 60.0, 10.0), 2.0);
        assert_eq!(time_at_position(px(-40.0), &scroll, 60.0, 10.0), 0.0);
        assert_eq!(time_at_position(px(6000.0), &scroll, 60.0, 10.0), 10.0);
    }
}
