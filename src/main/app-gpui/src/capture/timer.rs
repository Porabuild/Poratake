//! Timer capture — port of `capture/timer-capture.ts`: pick an area, run the
//! daemon's countdown panel above it, then capture when it finishes.

use std::sync::atomic::{AtomicBool, Ordering};

use gpui::Hsla;
use poratake_daemon_common::contract::{
    TimerShowRequest, TIMER_CONTROL_HEIGHT, TIMER_CONTROL_TOP_MARGIN, TIMER_CONTROL_WIDTH,
};

use crate::capture::DisplayCapture;
use crate::daemon::DaemonHandle;
use crate::theme::color::Srgba;

pub const TIMER_DURATION: u32 = 5;
static ACTIVE: AtomicBool = AtomicBool::new(false);

pub struct Session;

impl Drop for Session {
    fn drop(&mut self) {
        ACTIVE.store(false, Ordering::Release);
    }
}

pub fn is_active() -> bool {
    ACTIVE.load(Ordering::Acquire)
}

pub fn begin() -> Option<Session> {
    ACTIVE
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .ok()
        .map(|_| Session)
}

/// Port of `calculateTimerPosition`.
pub fn timer_position(capture: DisplayCapture) -> (i32, i32) {
    let area = capture.rect;
    let scale = if cfg!(target_os = "macos") {
        1.0
    } else {
        capture.scale_factor.max(0.01) as f32
    };
    let width = (TIMER_CONTROL_WIDTH as f32 * scale).round() as i32;
    let height = (TIMER_CONTROL_HEIGHT as f32 * scale).round() as i32;
    let margin = (TIMER_CONTROL_TOP_MARGIN as f32 * scale).round() as i32;
    let x = (area.x as f32 + area.width as f32 / 2.0 - width as f32 / 2.0).round() as i32;
    let y = area.y - height - margin;
    (x, y.max(capture.display_origin().y + margin))
}

fn color_hex(color: Hsla) -> String {
    let color = Srgba::from_hsla(color);
    format!(
        "#{:02x}{:02x}{:02x}",
        (color.r * 255.0).round().clamp(0.0, 255.0) as u8,
        (color.g * 255.0).round().clamp(0.0, 255.0) as u8,
        (color.b * 255.0).round().clamp(0.0, 255.0) as u8
    )
}

pub fn show(
    daemon: &DaemonHandle,
    capture: DisplayCapture,
    duration: u32,
    accent: Hsla,
    accent_foreground: Hsla,
) -> bool {
    let (x, y) = timer_position(capture);
    match daemon.timer_control().show(&TimerShowRequest {
        x,
        y,
        duration: i32::try_from(duration).unwrap_or(i32::MAX),
        color: color_hex(accent),
        foreground_color: color_hex(accent_foreground),
    }) {
        Ok(_) => true,
        Err(error) => {
            eprintln!("[timer] failed to show control: {error}");
            false
        }
    }
}

pub fn hide(daemon: &DaemonHandle) {
    if let Err(error) = daemon.timer_control().hide() {
        eprintln!("[timer] failed to hide control: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::overlay::ScreenRect;
    use poratake_daemon_common::geometry::DisplayOrigin;

    fn capture(
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        scale: f32,
        display_origin: (i32, i32),
    ) -> DisplayCapture {
        DisplayCapture::new(
            ScreenRect {
                x,
                y,
                width,
                height,
            },
            f64::from(scale),
            DisplayOrigin {
                x: display_origin.0,
                y: display_origin.1,
            },
            None,
        )
    }

    #[test]
    fn centres_the_countdown_above_the_selection() {
        assert_eq!(
            timer_position(capture(100, 400, 240, 120, 1.0, (0, 0))),
            (150, 328)
        );
    }

    #[test]
    fn clamps_the_countdown_to_the_top_margin() {
        assert_eq!(
            timer_position(capture(0, 10, 100, 100, 1.0, (0, 0))).1,
            TIMER_CONTROL_TOP_MARGIN
        );
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn scales_the_panel_geometry_on_mixed_dpi_displays() {
        assert_eq!(
            timer_position(capture(200, 800, 480, 240, 2.0, (0, 0))),
            (300, 656)
        );
        assert_eq!(
            timer_position(capture(-1600, 600, 501, 200, 1.5, (-1600, 0))),
            (-1455, 492)
        );
    }

    #[test]
    fn clamps_to_the_selected_displays_top_edge() {
        assert_eq!(
            timer_position(capture(0, -1070, 200, 100, 1.0, (0, -1080))).1,
            -1060
        );
        assert_eq!(
            timer_position(capture(0, 1100, 200, 100, 1.0, (0, 1080))).1,
            1100
        );
    }

    #[test]
    fn serializes_theme_colors_for_the_daemon() {
        assert_eq!(color_hex(Srgba::parse("#8892ef").to_hsla()), "#8892ef");
        assert_eq!(color_hex(Srgba::parse("#0a0a12").to_hsla()), "#0a0a12");
    }

    #[test]
    fn only_one_countdown_session_can_be_active() {
        let first = begin().expect("first timer session");
        assert!(is_active());
        assert!(begin().is_none());
        drop(first);
        assert!(!is_active());
        assert!(begin().is_some());
    }
}
