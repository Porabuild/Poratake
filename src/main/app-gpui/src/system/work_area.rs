//! The usable area of a display, i.e. what is left once the taskbar is taken
//! out.
//!
//! Electron positions the capture previews against `display.workArea`
//! (`capture-preview/index.ts`, `getPreviewPosition`). gpui 0.2.2 has no
//! equivalent -- `PlatformDisplay` exposes only `bounds()`, the full monitor
//! rectangle -- so this shell anchored to the full bounds and every
//! bottom-anchored preview sat a taskbar's height too low, tucked underneath it.
//! Measured on a 3440x1440 display with a 48px taskbar: the preview landed at
//! y=1276 where Electron puts it at y=1228.
//!
//! Windows reports both rectangles for a monitor, so the inset between them is
//! the taskbar (and any other appbar).

use gpui::{px, Bounds, Pixels};

/// The work area of the monitor showing `display`, or `display` unchanged if
/// the platform will not say.
pub fn work_area(display: Bounds<Pixels>) -> Bounds<Pixels> {
    match monitor_rects(display) {
        Some((monitor, work)) => inset_bounds(display, monitor, work),
        None => display,
    }
}

/// A `RECT` as `(left, top, right, bottom)`.
type Rect = (i32, i32, i32, i32);

/// Shrinks `display` by the difference between `monitor` and `work`.
///
/// The two rectangles come from Windows in physical pixels while `display` is
/// in gpui's logical pixels, so the insets are scaled by the ratio between
/// them rather than subtracted directly. At 100% scaling the ratio is 1 and
/// this is a plain subtraction.
fn inset_bounds(display: Bounds<Pixels>, monitor: Rect, work: Rect) -> Bounds<Pixels> {
    let (m_left, m_top, m_right, m_bottom) = monitor;
    let (w_left, w_top, w_right, w_bottom) = work;

    let monitor_width = (m_right - m_left) as f32;
    let monitor_height = (m_bottom - m_top) as f32;
    if monitor_width <= 0.0 || monitor_height <= 0.0 {
        return display;
    }

    let scale_x = f32::from(display.size.width) / monitor_width;
    let scale_y = f32::from(display.size.height) / monitor_height;

    let left = (w_left - m_left) as f32 * scale_x;
    let top = (w_top - m_top) as f32 * scale_y;
    let right = (m_right - w_right) as f32 * scale_x;
    let bottom = (m_bottom - w_bottom) as f32 * scale_y;

    let width = f32::from(display.size.width) - left - right;
    let height = f32::from(display.size.height) - top - bottom;
    // A degenerate result would push windows off-screen; the full bounds are a
    // worse position but never an impossible one.
    if width <= 0.0 || height <= 0.0 {
        return display;
    }

    Bounds {
        origin: gpui::point(
            px(f32::from(display.origin.x) + left),
            px(f32::from(display.origin.y) + top),
        ),
        size: gpui::size(px(width), px(height)),
    }
}

#[cfg(windows)]
fn monitor_rects(display: Bounds<Pixels>) -> Option<(Rect, Rect)> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };

    // The centre of the display, so the lookup cannot land on a neighbouring
    // monitor the way an edge coordinate can.
    let point = POINT {
        x: (f32::from(display.origin.x) + f32::from(display.size.width) / 2.0) as i32,
        y: (f32::from(display.origin.y) + f32::from(display.size.height) / 2.0) as i32,
    };

    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };

    // SAFETY: `cbSize` is set as the API requires and `info` is fully
    // initialised; `MONITOR_DEFAULTTONEAREST` never returns a null handle.
    let ok = unsafe {
        let monitor = MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST);
        GetMonitorInfoW(monitor, &mut info).as_bool()
    };
    if !ok {
        eprintln!("[work-area] GetMonitorInfoW failed; using the full display bounds");
        return None;
    }

    Some((
        (
            info.rcMonitor.left,
            info.rcMonitor.top,
            info.rcMonitor.right,
            info.rcMonitor.bottom,
        ),
        (
            info.rcWork.left,
            info.rcWork.top,
            info.rcWork.right,
            info.rcWork.bottom,
        ),
    ))
}

#[cfg(not(windows))]
fn monitor_rects(_display: Bounds<Pixels>) -> Option<(Rect, Rect)> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{point, size};

    fn bounds(x: f32, y: f32, w: f32, h: f32) -> Bounds<Pixels> {
        Bounds {
            origin: point(px(x), px(y)),
            size: size(px(w), px(h)),
        }
    }

    /// The case this module exists for: a 48px taskbar along the bottom of a
    /// 3440x1440 display. Electron's `workArea` is 3440x1392 there.
    #[test]
    fn a_bottom_taskbar_is_taken_off_the_height() {
        let display = bounds(0.0, 0.0, 3440.0, 1440.0);
        let inset = inset_bounds(display, (0, 0, 3440, 1440), (0, 0, 3440, 1392));
        assert_eq!(f32::from(inset.origin.y), 0.0);
        assert_eq!(f32::from(inset.size.height), 1392.0);
        assert_eq!(f32::from(inset.size.width), 3440.0, "width is untouched");
    }

    /// A taskbar docked to the left moves the origin as well as the size, which
    /// a naive height-only subtraction would miss.
    #[test]
    fn a_left_taskbar_moves_the_origin() {
        let display = bounds(0.0, 0.0, 1920.0, 1080.0);
        let inset = inset_bounds(display, (0, 0, 1920, 1080), (72, 0, 1920, 1080));
        assert_eq!(f32::from(inset.origin.x), 72.0);
        assert_eq!(f32::from(inset.size.width), 1848.0);
    }

    /// A secondary display sits at a non-zero origin in the virtual desktop and
    /// the inset has to be relative to that monitor, not to (0, 0).
    #[test]
    fn insets_are_relative_to_the_monitor_not_the_desktop() {
        let display = bounds(3440.0, 0.0, 1920.0, 1080.0);
        let inset = inset_bounds(display, (3440, 0, 5360, 1080), (3440, 0, 5360, 1040));
        assert_eq!(f32::from(inset.origin.x), 3440.0);
        assert_eq!(f32::from(inset.size.height), 1040.0);
    }

    /// At 150% scaling gpui reports 1280x720 logical for a 1920x1080 monitor,
    /// so a 60px physical taskbar has to become 40 logical pixels.
    #[test]
    fn insets_are_scaled_into_logical_pixels() {
        let display = bounds(0.0, 0.0, 1280.0, 720.0);
        let inset = inset_bounds(display, (0, 0, 1920, 1080), (0, 0, 1920, 1020));
        assert_eq!(
            f32::from(inset.size.height),
            680.0,
            "60 physical -> 40 logical"
        );
    }

    /// Nonsense from the platform must not produce a window placed off-screen.
    #[test]
    fn a_degenerate_rectangle_falls_back_to_the_full_bounds() {
        let display = bounds(0.0, 0.0, 1920.0, 1080.0);
        assert_eq!(inset_bounds(display, (0, 0, 0, 0), (0, 0, 0, 0)), display);
        assert_eq!(
            inset_bounds(display, (0, 0, 1920, 1080), (0, 0, 0, 0)),
            display,
            "a work area with no area at all is not usable"
        );
    }

    /// No taskbar at all leaves the bounds exactly as they were.
    #[test]
    fn an_unobstructed_display_is_unchanged() {
        let display = bounds(0.0, 0.0, 1920.0, 1080.0);
        assert_eq!(
            inset_bounds(display, (0, 0, 1920, 1080), (0, 0, 1920, 1080)),
            display
        );
    }
}
