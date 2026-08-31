//! Fitting the capture to the window.
//!
//! `calculateOptimalZoom` in `screenshot-window.tsx` runs on the first frame,
//! on every window resize, and whenever the wallpaper sheet opens or closes.
//! This shell had no equivalent: the editor opened at 100% and a capture larger
//! than the window was simply clipped, with no indication that more of it
//! existed.

use gpui::{Pixels, Size};

/// A fit never enlarges past 2x, however small the capture.
pub const MAX_FIT_ZOOM: f32 = 2.0;

/// `const viewportPadding = 80`.
const VIEWPORT_PADDING: f32 = 80.0;
/// `window.innerHeight - 40` -- the editor's own toolbar.
const TOOLBAR_HEIGHT: f32 = 40.0;
/// `window.innerWidth - 320` while the wallpaper sheet is open.
const SHEET_WIDTH: f32 = 320.0;

/// The zoom that fits `canvas` inside `viewport`, rounded to whole percent and
/// clamped exactly as Electron clamps it.
pub fn optimal_zoom(
    viewport: Size<Pixels>,
    canvas_width: f32,
    canvas_height: f32,
    sheet_open: bool,
) -> f32 {
    if canvas_width <= 0.0 || canvas_height <= 0.0 {
        return 1.0;
    }

    let inner_width = f32::from(viewport.width);
    let inner_height = f32::from(viewport.height);

    let available_width = if sheet_open {
        inner_width - SHEET_WIDTH - VIEWPORT_PADDING
    } else {
        inner_width - VIEWPORT_PADDING
    };
    let available_height = inner_height - TOOLBAR_HEIGHT - VIEWPORT_PADDING;

    let zoom_x = available_width / canvas_width;
    let zoom_y = available_height / canvas_height;
    // `Math.round(Math.min(zoomX, zoomY) * 100) / 100`.
    let fitted = (zoom_x.min(zoom_y) * 100.0).round() / 100.0;

    fitted.clamp(crate::editor::options::MIN_ZOOM, MAX_FIT_ZOOM)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{px, size};

    #[test]
    fn a_capture_larger_than_the_window_is_scaled_down_to_fit() {
        // 1280x738 viewport, 1200x800 capture: available 1200x618, so the
        // height binds at 618/800 = 0.7725 -> 0.77.
        let zoom = optimal_zoom(size(px(1280.0), px(738.0)), 1200.0, 800.0, false);
        assert_eq!(zoom, 0.77);
    }

    #[test]
    fn the_wallpaper_sheet_takes_320_pixels_off_the_width() {
        let viewport = size(px(1280.0), px(2000.0));
        // Tall viewport, so the width binds in both cases.
        let open = optimal_zoom(viewport, 1200.0, 100.0, true);
        let closed = optimal_zoom(viewport, 1200.0, 100.0, false);
        assert_eq!(closed, ((1200.0f32 / 1200.0) * 100.0).round() / 100.0);
        assert_eq!(open, ((880.0f32 / 1200.0) * 100.0).round() / 100.0);
        assert!(open < closed);
    }

    #[test]
    fn a_small_capture_is_enlarged_but_never_past_the_fit_ceiling() {
        let zoom = optimal_zoom(size(px(1920.0), px(1080.0)), 100.0, 100.0, false);
        assert_eq!(zoom, MAX_FIT_ZOOM, "clamped to 2x rather than 9x");
    }

    #[test]
    fn an_enormous_capture_stops_at_the_minimum_zoom() {
        let zoom = optimal_zoom(size(px(400.0), px(400.0)), 100_000.0, 100_000.0, false);
        assert_eq!(zoom, crate::editor::options::MIN_ZOOM);
    }

    /// Before the first layout there is no viewport and no image; 1.0 is the
    /// same neutral value `calculateOptimalZoom` returns for a missing size.
    #[test]
    fn a_degenerate_canvas_or_viewport_is_left_at_one() {
        assert_eq!(
            optimal_zoom(size(px(800.0), px(600.0)), 0.0, 0.0, false),
            1.0
        );
        assert_eq!(
            optimal_zoom(size(px(0.0), px(0.0)), 100.0, 100.0, false),
            crate::editor::options::MIN_ZOOM,
            "a zero viewport still clamps rather than returning a negative zoom"
        );
    }
}

#[cfg(test)]
mod window_tests {
    /// The editor has to fit the capture on the first frame. Nothing else in the
    /// suite would notice if the hook stopped firing -- the maths above would
    /// still pass while the window sat at 100%.
    #[gpui::test]
    fn opening_a_capture_fits_it_to_the_window(cx: &mut gpui::TestAppContext) {
        use crate::editor::window::EditorWindow;

        let dir = tempfile::tempdir().expect("temp dir");
        let store = std::sync::Arc::new(
            crate::config::store::ConfigStore::load_at(dir.path().join("config.json"))
                .expect("load config"),
        );
        cx.update(|cx| crate::state::set_test_state(cx, store));

        // Deliberately larger than any test window, so a fit must scale down.
        let path = dir.path().join("capture.png");
        image::RgbaImage::from_pixel(2400, 1600, image::Rgba([10, 20, 30, 255]))
            .save(&path)
            .expect("write a capture");
        let path = path.to_string_lossy().to_string();

        let window = cx.add_window(|window, cx| EditorWindow::from_file(&path, window, cx));
        cx.refresh().expect("schedule a redraw");
        cx.run_until_parked();

        let zoom = window
            .update(cx, |view, _window, _cx| view.zoom)
            .expect("read the zoom");
        assert!(
            zoom < 1.0,
            "a 2400x1600 capture has to be scaled down to fit, got {zoom}"
        );
        assert!(
            zoom >= crate::editor::options::MIN_ZOOM,
            "and not below the floor, got {zoom}"
        );
    }

    #[gpui::test]
    fn secondary_scroll_zooms_the_editor(cx: &mut gpui::TestAppContext) {
        use gpui::{point, px, Modifiers, ScrollDelta, ScrollWheelEvent, TouchPhase};

        use crate::editor::window::EditorWindow;

        let dir = tempfile::tempdir().expect("temp dir");
        let store = std::sync::Arc::new(
            crate::config::store::ConfigStore::load_at(dir.path().join("config.json"))
                .expect("load config"),
        );
        cx.update(|cx| crate::state::set_test_state(cx, store));
        let path = dir.path().join("capture.png");
        image::RgbaImage::from_pixel(600, 400, image::Rgba([10, 20, 30, 255]))
            .save(&path)
            .expect("write a capture");
        let path = path.to_string_lossy().to_string();
        let (editor, cx) =
            cx.add_window_view(|window, cx| EditorWindow::from_file(&path, window, cx));
        cx.refresh().expect("schedule a redraw");
        cx.run_until_parked();
        let initial_zoom = editor.read_with(cx, |editor, _| editor.zoom);

        cx.simulate_event(ScrollWheelEvent {
            position: point(px(400.0), px(300.0)),
            delta: ScrollDelta::Lines(point(0.0, -1.0)),
            touch_phase: TouchPhase::Moved,
            modifiers: Modifiers::secondary_key(),
        });

        let zoom = editor.read_with(cx, |editor, _| editor.zoom);
        assert!(
            zoom > initial_zoom,
            "{initial_zoom} should increase, got {zoom}"
        );
    }
}
