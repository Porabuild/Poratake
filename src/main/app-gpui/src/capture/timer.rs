//! Timer capture — port of `capture/timer-capture.ts`: pick an area, run the
//! daemon's countdown panel above it, then capture when it finishes.

use std::sync::atomic::{AtomicBool, Ordering};

use gpui::Hsla;
use herogpui::gpui;
use poratake_daemon_common::contract::{
    TimerShowRequest, TIMER_CONTROL_HEIGHT, TIMER_CONTROL_TOP_MARGIN, TIMER_CONTROL_WIDTH,
};

use crate::capture::overlay::ScreenRect;
use crate::capture::DisplayCapture;
use crate::daemon::DaemonHandle;
use crate::theme::color::Srgba;

pub const TIMER_DURATION: u32 = 5;
static ACTIVE: AtomicBool = AtomicBool::new(false);

#[cfg(all(test, windows))]
static TEST_HIDE_GATE: std::sync::Mutex<
    Option<(
        std::sync::mpsc::SyncSender<()>,
        std::sync::mpsc::Receiver<()>,
    )>,
> = std::sync::Mutex::new(None);

#[cfg(all(test, windows))]
pub(crate) fn gate_hide_for_test(
    entered: std::sync::mpsc::SyncSender<()>,
    release: std::sync::mpsc::Receiver<()>,
) {
    *TEST_HIDE_GATE.lock().unwrap() = Some((entered, release));
}

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

pub fn timer_position(capture: DisplayCapture, placement_bounds: Option<ScreenRect>) -> (i32, i32) {
    let area = capture.rect;
    let scale = if cfg!(target_os = "macos") {
        1.0
    } else {
        capture.scale_factor.max(0.01) as f32
    };
    let width = (TIMER_CONTROL_WIDTH as f32 * scale).round() as i32;
    let height = (TIMER_CONTROL_HEIGHT as f32 * scale).round() as i32;
    let margin = (TIMER_CONTROL_TOP_MARGIN as f32 * scale).round() as i32;
    let centered_x = (area.x as f32 + area.width as f32 / 2.0 - width as f32 / 2.0).round() as i32;
    let preferred_top = area.y.saturating_sub(height).saturating_sub(margin);
    let Some(bounds) = placement_bounds.filter(|bounds| bounds.width > 0 && bounds.height > 0)
    else {
        let origin = capture.display_origin();
        return (
            centered_x.max(origin.x),
            preferred_top.max(origin.y.saturating_add(margin)),
        );
    };
    let max_x = bounds.x.saturating_add((bounds.width - width).max(0));
    let max_y = bounds.y.saturating_add((bounds.height - height).max(0));
    let min_y = bounds.y.saturating_add(margin).min(max_y);
    (
        centered_x.clamp(bounds.x, max_x),
        preferred_top.clamp(min_y, max_y),
    )
}

pub fn placement_bounds(capture: DisplayCapture, cx: &mut gpui::App) -> Option<ScreenRect> {
    let display_id = capture.display_id?;
    let display = cx
        .displays()
        .into_iter()
        .find(|display| u64::from(display.id()) as u32 == display_id)?;
    let scale = capture.scale_factor as f32;
    let bounds = crate::system::work_area::display_bounds(display.as_ref());
    let rect = crate::capture::overlay::physical_rect(bounds, scale);
    if rect.x != capture.display_origin_x
        || rect.y != capture.display_origin_y
        || rect.width <= 0
        || rect.height <= 0
    {
        return None;
    }
    #[cfg(target_os = "macos")]
    let rect = crate::capture::overlay::physical_rect(
        crate::system::work_area::work_area(display.as_ref()),
        scale,
    );
    Some(rect)
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
    placement_bounds: Option<ScreenRect>,
    duration: u32,
    accent: Hsla,
    accent_foreground: Hsla,
) -> bool {
    #[cfg(all(test, windows))]
    if std::env::var_os("PORATAKE_TEST_TIMER_CONTROL").is_some() {
        return true;
    }
    let (x, y) = timer_position(capture, placement_bounds);
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
    #[cfg(all(test, windows))]
    if let Some((entered, release)) = TEST_HIDE_GATE.lock().unwrap().take() {
        entered.send(()).unwrap();
        release.recv().unwrap();
        return;
    }
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
            timer_position(capture(100, 400, 240, 120, 1.0, (0, 0)), None),
            (150, 328)
        );
    }

    #[test]
    fn clamps_the_countdown_to_the_top_margin() {
        assert_eq!(
            timer_position(capture(0, 10, 100, 100, 1.0, (0, 0)), None).1,
            TIMER_CONTROL_TOP_MARGIN
        );
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn scales_the_panel_geometry_on_mixed_dpi_displays() {
        assert_eq!(
            timer_position(capture(200, 800, 480, 240, 2.0, (0, 0)), None),
            (300, 656)
        );
        assert_eq!(
            timer_position(capture(-1600, 600, 501, 200, 1.5, (-1600, 0)), None),
            (-1455, 492)
        );
    }

    #[test]
    fn clamps_to_the_selected_displays_top_edge() {
        assert_eq!(
            timer_position(capture(0, -1070, 200, 100, 1.0, (0, -1080)), None).1,
            -1060
        );
        assert_eq!(
            timer_position(capture(0, 1100, 200, 100, 1.0, (0, 1080)), None).1,
            1100
        );
    }

    #[test]
    fn keeps_the_countdown_in_the_inset_work_area() {
        let work_area = ScreenRect {
            x: -1140,
            y: -267,
            width: 1060,
            height: 823,
        };
        for (x, y, expected) in [
            (-1200, -300, (-1140, -247)),
            (-10, -300, (-220, -247)),
            (-10, 590, (-220, 504)),
        ] {
            assert_eq!(
                timer_position(capture(x, y, 10, 10, 1.0, (-1200, -300)), Some(work_area)),
                expected
            );
        }
    }

    #[test]
    fn keeps_the_countdown_inside_the_selected_display_at_both_edges() {
        let display = ScreenRect {
            x: -1200,
            y: -300,
            width: 1200,
            height: 900,
        };
        assert_eq!(
            timer_position(
                capture(-1200, 100, 10, 100, 1.0, (-1200, -300)),
                Some(display)
            )
            .0,
            -1200
        );
        assert_eq!(
            timer_position(
                capture(-10, 100, 10, 100, 1.0, (-1200, -300)),
                Some(display)
            )
            .0,
            -140
        );
        assert_eq!(
            timer_position(
                capture(-600, 100, 200, 100, 1.0, (-1200, -300)),
                Some(display)
            )
            .0,
            -570
        );
        assert_eq!(
            timer_position(capture(-1200, 100, 10, 100, 1.0, (-1200, -300)), None).0,
            -1200
        );
    }

    #[test]
    fn aligns_to_a_display_narrower_than_the_control() {
        let display = ScreenRect {
            x: 100,
            y: 0,
            width: 100,
            height: 400,
        };
        assert_eq!(
            timer_position(capture(145, 100, 10, 100, 1.0, (100, 0)), Some(display)).0,
            100
        );
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn uses_scaled_control_width_for_display_edges() {
        let display = ScreenRect {
            x: -1600,
            y: 0,
            width: 1600,
            height: 900,
        };
        assert_eq!(
            timer_position(capture(-1600, 600, 10, 100, 1.5, (-1600, 0)), Some(display)).0,
            -1600
        );
        assert_eq!(
            timer_position(capture(-10, 600, 10, 100, 1.5, (-1600, 0)), Some(display)).0,
            -210
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
