use std::time::Duration;

pub const ACTIVE_PREVIEW_SIZES: [(u32, u32); 7] = [
    (384, 216),
    (432, 243),
    (480, 270),
    (640, 360),
    (854, 480),
    (960, 540),
    (1280, 720),
];
pub const INITIAL_PREVIEW_QUALITY: usize = 3;
pub const PREVIEW_QUALITY_RAISE_FRAMES: u16 = 120;

const MIN_VIEWPORT_PREVIEW_EDGE: u32 = 320;

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
    if active_motion {
        let (active_w, active_h) =
            ACTIVE_PREVIEW_SIZES[quality_index.min(ACTIVE_PREVIEW_SIZES.len() - 1)];
        return match viewport {
            Some((vp_w, vp_h)) => Some((active_w.min(vp_w), active_h.min(vp_h))),
            None => Some((active_w, active_h)),
        };
    }
    viewport
}

pub fn adjust_preview_quality(
    index: usize,
    fast_frames: u16,
    elapsed: Duration,
    decode_wait: Duration,
    budget: Duration,
) -> (usize, u16) {
    let adjustable_elapsed = if decode_wait >= budget {
        elapsed.saturating_sub(decode_wait)
    } else {
        elapsed
    };
    if adjustable_elapsed > budget && index > 0 {
        return (index - 1, 0);
    }
    if elapsed >= budget.mul_f32(0.65) {
        return (index, 0);
    }
    let fast_frames = fast_frames.saturating_add(1);
    if fast_frames < PREVIEW_QUALITY_RAISE_FRAMES || index + 1 >= ACTIVE_PREVIEW_SIZES.len() {
        return (index, fast_frames);
    }
    (index + 1, 0)
}

pub fn active_preview_budget(frame_rate: f64) -> Duration {
    playback_tick(frame_rate).mul_f32(0.5)
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
        assert_eq!(preview_max_dimensions(false, 3, None), None);
        assert_eq!(
            preview_max_dimensions(false, 3, Some((1200, 800))),
            Some((1200, 800))
        );
        assert_eq!(
            preview_max_dimensions(false, 3, Some((100, 100))),
            Some((MIN_VIEWPORT_PREVIEW_EDGE, MIN_VIEWPORT_PREVIEW_EDGE))
        );
    }

    #[test]
    fn active_preview_never_exceeds_viewport_or_quality_tier() {
        assert_eq!(
            preview_max_dimensions(true, 6, Some((900, 500))),
            Some((900, 500))
        );
        assert_eq!(
            preview_max_dimensions(true, 3, Some((1920, 1080))),
            Some((640, 360))
        );
        assert_eq!(preview_max_dimensions(true, 3, None), Some((640, 360)));
    }
}
