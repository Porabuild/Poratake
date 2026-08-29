//! Timer capture — port of `capture/timer-capture.ts`: pick an area, run the
//! daemon's countdown panel above it, then capture when it finishes.

use gpui::Hsla;
use serde_json::json;

use crate::capture::overlay::ScreenRect;
use crate::daemon::DaemonHandle;
use crate::theme::color::Srgba;

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
    area: ScreenRect,
    duration: u32,
    accent: Hsla,
    accent_foreground: Hsla,
) -> bool {
    let (x, y) = timer_position(area);
    if !daemon.is_running() && daemon.start().is_err() {
        return false;
    }
    match daemon.call(
        "timer-control",
        "show",
        Some(json!({
            "x": x,
            "y": y,
            "duration": duration,
            "color": color_hex(accent),
            "foregroundColor": color_hex(accent_foreground),
        })),
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

    #[test]
    fn serializes_theme_colors_for_the_daemon() {
        assert_eq!(color_hex(Srgba::parse("#8892ef").to_hsla()), "#8892ef");
        assert_eq!(color_hex(Srgba::parse("#0a0a12").to_hsla()), "#0a0a12");
    }
}
