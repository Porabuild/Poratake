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

pub fn display_bounds(display: &dyn gpui::PlatformDisplay) -> Bounds<Pixels> {
    #[cfg(target_os = "macos")]
    unsafe {
        let bounds = CGDisplayBounds(u32::from(display.id()));
        return Bounds {
            origin: gpui::point(px(bounds.origin.x as f32), px(bounds.origin.y as f32)),
            size: gpui::size(px(bounds.size.width as f32), px(bounds.size.height as f32)),
        };
    }
    #[cfg(not(target_os = "macos"))]
    display.bounds()
}

pub fn capture_scale_factor(display: &dyn gpui::PlatformDisplay, cx: &mut gpui::App) -> f32 {
    #[cfg(target_os = "macos")]
    if let Some(scale) = macos_display_scale_factor(u32::from(display.id())) {
        return scale;
    }
    #[cfg(windows)]
    {
        let bounds = display.bounds();
        let Some((monitor, _)) = monitor_rects(bounds) else {
            return 1.0;
        };
        if let Some(scale) =
            scale_from_widths(f32::from(bounds.size.width), (monitor.2 - monitor.0) as f32)
        {
            return scale;
        }
    }
    #[cfg(target_os = "linux")]
    if crate::system::linux_session::current() == crate::system::linux_session::LinuxSession::X11 {
        let physical = crate::state::state(cx)
            .daemon
            .screenshot()
            .list_displays()
            .unwrap_or_default();
        if let Some(scale) = x11_capture_scale_factor(display, &physical) {
            return scale;
        }
    }
    #[cfg(not(target_os = "linux"))]
    let _ = cx;
    1.0
}

#[cfg(target_os = "linux")]
pub fn x11_capture_scale_factor(
    display: &dyn gpui::PlatformDisplay,
    physical: &[poratake_daemon_common::geometry::DisplayInfo],
) -> Option<f32> {
    x11_scale_for_width(f32::from(display.bounds().size.width), physical)
}

#[cfg(any(target_os = "linux", test))]
fn x11_scale_for_width(
    logical_width: f32,
    physical: &[poratake_daemon_common::geometry::DisplayInfo],
) -> Option<f32> {
    let left = physical.iter().map(|display| display.rect.x).min()?;
    let right = physical
        .iter()
        .map(|display| display.rect.x + display.rect.width)
        .max()?;
    scale_from_widths(logical_width, (right - left) as f32)
}

fn scale_from_widths(logical: f32, physical: f32) -> Option<f32> {
    (logical > 0.0 && physical > 0.0).then_some(physical / logical)
}

#[cfg(any(target_os = "macos", test))]
fn scale_for_display_id(display_id: u32, scales: &[(u32, f32)]) -> Option<f32> {
    scales
        .iter()
        .find_map(|(candidate, scale)| (*candidate == display_id).then_some(*scale))
        .filter(|scale| scale.is_finite() && *scale > 0.0)
}

#[cfg(target_os = "macos")]
#[allow(unexpected_cfgs)]
fn macos_display_scale_factor(display_id: u32) -> Option<f32> {
    use cocoa::appkit::NSScreen;
    use cocoa::base::nil;
    use cocoa::foundation::{NSArray, NSDictionary, NSString};
    use objc::{msg_send, sel, sel_impl};

    unsafe {
        let screens = NSScreen::screens(nil);
        let key = NSString::alloc(nil).init_str("NSScreenNumber");
        let count = NSArray::count(screens) as usize;
        let mut scales = Vec::with_capacity(count);
        for index in 0..count {
            let screen = screens.objectAtIndex(index);
            let number = screen.deviceDescription().objectForKey_(key);
            if number == nil {
                continue;
            }
            let candidate: u32 = msg_send![number, unsignedIntValue];
            scales.push((candidate, screen.backingScaleFactor() as f32));
        }
        let _: () = msg_send![key, release];
        scale_for_display_id(display_id, &scales)
    }
}

pub fn local_window_bounds(
    bounds: Bounds<Pixels>,
    display: &dyn gpui::PlatformDisplay,
) -> Bounds<Pixels> {
    #[cfg(target_os = "macos")]
    {
        let display = display_bounds(display);
        return Bounds {
            origin: gpui::point(
                px(f32::from(bounds.origin.x - display.origin.x)),
                px(f32::from(bounds.origin.y - display.origin.y)),
            ),
            size: bounds.size,
        };
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = display;
        bounds
    }
}

#[cfg(target_os = "macos")]
#[repr(C)]
struct CgPoint {
    x: f64,
    y: f64,
}

#[cfg(target_os = "macos")]
#[repr(C)]
struct CgSize {
    width: f64,
    height: f64,
}

#[cfg(target_os = "macos")]
#[repr(C)]
struct CgRect {
    origin: CgPoint,
    size: CgSize,
}

#[cfg(target_os = "macos")]
#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGDisplayBounds(display: u32) -> CgRect;
}

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

#[cfg(target_os = "macos")]
#[allow(unexpected_cfgs)]
fn monitor_rects(display: Bounds<Pixels>) -> Option<(Rect, Rect)> {
    use cocoa::appkit::NSScreen;
    use cocoa::base::nil;
    use cocoa::foundation::{NSArray, NSDictionary, NSString};
    use objc::{msg_send, sel, sel_impl};

    unsafe {
        let screens = NSScreen::screens(nil);
        let key = NSString::alloc(nil).init_str("NSScreenNumber");
        for index in 0..NSArray::count(screens) {
            let screen = screens.objectAtIndex(index);
            let number = screen.deviceDescription().objectForKey_(key);
            if number == nil {
                continue;
            }
            let display_id: u32 = msg_send![number, unsignedIntValue];
            let cg = CGDisplayBounds(display_id);
            if cg.origin.x as f32 != f32::from(display.origin.x)
                || cg.origin.y as f32 != f32::from(display.origin.y)
                || cg.size.width as f32 != f32::from(display.size.width)
                || cg.size.height as f32 != f32::from(display.size.height)
            {
                continue;
            }
            let frame = screen.frame();
            let visible = screen.visibleFrame();
            let monitor = (
                cg.origin.x.round() as i32,
                cg.origin.y.round() as i32,
                (cg.origin.x + cg.size.width).round() as i32,
                (cg.origin.y + cg.size.height).round() as i32,
            );
            let work = appkit_visible_work_rect(
                monitor,
                (
                    frame.origin.x,
                    frame.origin.y,
                    frame.size.width,
                    frame.size.height,
                ),
                (
                    visible.origin.x,
                    visible.origin.y,
                    visible.size.width,
                    visible.size.height,
                ),
            );
            let _: () = msg_send![key, release];
            return Some((monitor, work));
        }
        let _: () = msg_send![key, release];
    }
    None
}

#[cfg(any(target_os = "macos", test))]
fn appkit_visible_work_rect(
    monitor: Rect,
    frame: (f64, f64, f64, f64),
    visible: (f64, f64, f64, f64),
) -> Rect {
    let (frame_x, frame_y, frame_width, frame_height) = frame;
    let (visible_x, visible_y, visible_width, visible_height) = visible;
    let left = visible_x - frame_x;
    let right = frame_width - left - visible_width;
    let top = frame_y + frame_height - visible_y - visible_height;
    let bottom = visible_y - frame_y;
    (
        (monitor.0 as f64 + left).round() as i32,
        (monitor.1 as f64 + top).round() as i32,
        (monitor.2 as f64 - right).round() as i32,
        (monitor.3 as f64 - bottom).round() as i32,
    )
}

#[cfg(not(any(windows, target_os = "macos")))]
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

    #[test]
    fn capture_scale_uses_the_selected_displays_physical_width() {
        assert_eq!(scale_from_widths(1280.0, 1920.0), Some(1.5));
        assert_eq!(scale_from_widths(1920.0, 1920.0), Some(1.0));
    }

    #[test]
    fn x11_capture_scale_uses_the_full_randr_desktop() {
        use poratake_daemon_common::geometry::{CaptureRect, DisplayInfo};

        let displays = [
            DisplayInfo {
                rect: CaptureRect {
                    x: -1920,
                    y: 0,
                    width: 1920,
                    height: 1080,
                },
                primary: false,
            },
            DisplayInfo {
                rect: CaptureRect {
                    x: 0,
                    y: 0,
                    width: 2560,
                    height: 1440,
                },
                primary: true,
            },
        ];

        assert_eq!(x11_scale_for_width(2240.0, &displays), Some(2.0));
    }

    #[test]
    fn capture_scale_uses_the_selected_macos_display_id() {
        let scales = [(17, 1.0), (73, 2.0)];
        assert_eq!(scale_for_display_id(73, &scales), Some(2.0));
        assert_eq!(scale_for_display_id(99, &scales), None);
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

    #[test]
    fn appkit_menu_bar_and_bottom_dock_map_to_top_left_coordinates() {
        assert_eq!(
            appkit_visible_work_rect(
                (0, 0, 1440, 900),
                (0.0, 0.0, 1440.0, 900.0),
                (0.0, 50.0, 1440.0, 826.0),
            ),
            (0, 24, 1440, 850)
        );
    }
}
