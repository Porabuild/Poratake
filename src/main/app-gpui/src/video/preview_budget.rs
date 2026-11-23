use std::time::Duration;

pub const ACTIVE_PREVIEW_SCALES: [f32; 4] = [0.55, 0.70, 0.85, 1.0];
pub const INITIAL_PREVIEW_QUALITY: usize = ACTIVE_PREVIEW_SCALES.len() - 1;
pub const PREVIEW_QUALITY_RAISE_FRAMES: u16 = 30;

const MIN_VIEWPORT_PREVIEW_EDGE: u32 = 320;
const FALLBACK_ACTIVE_PREVIEW_SIZE: (u32, u32) = (1920, 1080);

fn scale_edge(edge: u32, scale: f32) -> u32 {
    if scale >= 1.0 {
        return edge;
    }
    ((edge as f32 * scale).round() as u32)
        .max(MIN_VIEWPORT_PREVIEW_EDGE)
        .min(edge)
}

pub fn preview_max_dimensions(
    active_motion: bool,
    quality_index: usize,
    viewport_physical: Option<(u32, u32)>,
) -> Option<(u32, u32)> {
    let viewport = viewport_physical.map(|(w, h)| {
        (
            w.max(MIN_VIEWPORT_PREVIEW_EDGE),
            h.max(MIN_VIEWPORT_PREVIEW_EDGE),
        )
    });
    if !active_motion {
        return viewport;
    }
    let scale = ACTIVE_PREVIEW_SCALES[quality_index.min(ACTIVE_PREVIEW_SCALES.len() - 1)];
    let (base_w, base_h) = viewport.unwrap_or(FALLBACK_ACTIVE_PREVIEW_SIZE);
    Some((scale_edge(base_w, scale), scale_edge(base_h, scale)))
}

pub fn adjust_preview_quality(
    index: usize,
    fast_frames: u16,
    elapsed: Duration,
    decode_wait: Duration,
    budget: Duration,
) -> (usize, u16) {
    let render_elapsed = elapsed.saturating_sub(decode_wait);
    if render_elapsed > budget && index > 0 {
        return (index - 1, 0);
    }
    if render_elapsed >= budget.mul_f32(0.65) {
        return (index, 0);
    }
    let fast_frames = fast_frames.saturating_add(1);
    if fast_frames < PREVIEW_QUALITY_RAISE_FRAMES || index + 1 >= ACTIVE_PREVIEW_SCALES.len() {
        return (index, fast_frames);
    }
    (index + 1, 0)
}

pub fn active_preview_budget(frame_rate: f64) -> Duration {
    playback_tick(frame_rate).mul_f32(0.75)
}

pub fn playback_tick(frame_rate: f64) -> Duration {
    let frame_rate = if frame_rate.is_finite() && frame_rate > 0.0 {
        frame_rate.min(240.0)
    } else {
        60.0
    };
    Duration::from_secs_f64(1.0 / frame_rate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paused_preview_caps_to_physical_viewport_when_known() {
        assert_eq!(
            preview_max_dimensions(false, INITIAL_PREVIEW_QUALITY, None),
            None
        );
        assert_eq!(
            preview_max_dimensions(false, INITIAL_PREVIEW_QUALITY, Some((1200, 800))),
            Some((1200, 800))
        );
        assert_eq!(
            preview_max_dimensions(false, INITIAL_PREVIEW_QUALITY, Some((100, 100))),
            Some((MIN_VIEWPORT_PREVIEW_EDGE, MIN_VIEWPORT_PREVIEW_EDGE))
        );
    }

    #[test]
    fn active_preview_starts_at_full_viewport_and_scales_relative_to_it() {
        assert_eq!(
            preview_max_dimensions(true, INITIAL_PREVIEW_QUALITY, Some((1440, 900))),
            Some((1440, 900))
        );
        assert_eq!(
            preview_max_dimensions(true, INITIAL_PREVIEW_QUALITY, Some((2560, 1440))),
            Some((2560, 1440))
        );
        assert_eq!(
            preview_max_dimensions(true, INITIAL_PREVIEW_QUALITY, None),
            Some(FALLBACK_ACTIVE_PREVIEW_SIZE)
        );
        assert_eq!(
            preview_max_dimensions(true, 1, Some((1400, 800))),
            Some((980, 560))
        );
        assert_eq!(
            preview_max_dimensions(true, 0, Some((400, 340))),
            Some((MIN_VIEWPORT_PREVIEW_EDGE, MIN_VIEWPORT_PREVIEW_EDGE))
        );
    }
}
