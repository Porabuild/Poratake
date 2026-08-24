//! Timer capture — port of `capture/timer-capture.ts`: pick an area, run the
//! daemon's countdown panel above it, then capture when it finishes.

use serde_json::json;

use crate::capture::overlay::ScreenRect;
use crate::daemon::DaemonHandle;

pub const TIMER_DURATION: u32 = 5;
const WINDOW_WIDTH: i32 = 140;
const WINDOW_HEIGHT: i32 = 52;
const TIMER_TOP_MARGIN: i32 = 20;

/// Port of `calculateTimerPosition`.
pub fn timer_position(area: ScreenRect) -> (i32, i32) {
    let x = area.x + area.width / 2 - WINDOW_WIDTH / 2;
    let y = area.y - WINDOW_HEIGHT - TIMER_TOP_MARGIN;
    (x, y.max(TIMER_TOP_MARGIN))
}

pub fn show(daemon: &DaemonHandle, area: ScreenRect, duration: u32) -> bool {
    let (x, y) = timer_position(area);
    if !daemon.is_running() && daemon.start().is_err() {
        return false;
    }
    match daemon.call(
        "timer-control",
        "show",
        Some(json!({ "x": x, "y": y, "duration": duration })),
    ) {
        Ok(_) => true,
        Err(error) => {
            eprintln!("[timer] failed to show control: {error}");
            false
        }
    }
}

pub fn hide(daemon: &DaemonHandle) {
    if !daemon.is_running() {
        return;
    }
    if let Err(error) = daemon.call("timer-control", "hide", None) {
        eprintln!("[timer] failed to hide control: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: i32, y: i32, width: i32, height: i32) -> ScreenRect {
        ScreenRect {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn centres_the_countdown_above_the_selection() {
        assert_eq!(timer_position(rect(100, 400, 240, 120)), (150, 328));
    }

    #[test]
    fn clamps_the_countdown_to_the_top_margin() {
        assert_eq!(timer_position(rect(0, 10, 100, 100)).1, TIMER_TOP_MARGIN);
    }
}
