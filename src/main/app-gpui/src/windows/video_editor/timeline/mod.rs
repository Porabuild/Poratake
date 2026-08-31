pub mod controls;
pub mod edit;
pub mod ruler;
pub mod tracks;

pub const DEFAULT_PIXELS_PER_SECOND: f32 = 100.0;
pub const MIN_PIXELS_PER_SECOND: f32 = 10.0;
pub const MAX_PIXELS_PER_SECOND: f32 = 500.0;
pub const ZOOM_STEP: f32 = 1.25;
pub const TRACK_HEIGHT: f32 = 24.0;
pub const TRACK_GUTTER_WIDTH: f32 = 40.0;
pub const RULER_HEIGHT: f32 = 28.0;

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
