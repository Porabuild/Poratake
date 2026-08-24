//! Headless render coverage for the windows that cannot be driven from a test
//! harness any other way.
//!
//! These open each window in gpui's test app, draw it, and let any panic on a
//! render path fail the build. That is not a substitute for looking at the
//! pixels, but it is what catches the class of defect that shipped here twice:
//! `Entity::read` inside a view's own render, which panics rather than
//! degrading, and which no amount of type-checking finds.

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use gpui::TestAppContext;

    use crate::config::store::ConfigStore;
    use crate::windows::settings::registry::Category;
    use crate::windows::settings::SettingsWindow;

    /// A store backed by a throwaway file, so a test never reads or writes the
    /// developer's real configuration.
    fn scratch_store() -> (tempfile::TempDir, Arc<ConfigStore>) {
        let dir = tempfile::tempdir().expect("temp dir");
        let store = ConfigStore::load_at(dir.path().join("config.json")).expect("load config");
        (dir, Arc::new(store))
    }

    /// The devices category reads the capture service off the global, so it has
    /// to be present. The daemon is never started, which is a state the app
    /// itself has to survive: the device lists come back empty.
    fn install_state(cx: &mut TestAppContext, config: Arc<ConfigStore>) {
        cx.update(|cx| {
            crate::state::set_test_state(cx, config);
        });
    }

    /// Every settings category has to survive a draw. The shortcuts category is
    /// the one that used to panic, so the loop covers all of them rather than
    /// just the default.
    #[gpui::test]
    fn every_settings_category_renders(cx: &mut TestAppContext) {
        let (_dir, store) = scratch_store();
        install_state(cx, store.clone());

        for category in Category::supported() {
            let _window =
                cx.add_window(|_window, cx| SettingsWindow::new(store.clone(), category, cx));
            // `refresh` schedules a redraw of every window; parking the
            // executor is what actually runs the paint, and a panic in a render
            // path surfaces here.
            cx.refresh().expect("schedule a redraw");
            cx.run_until_parked();
        }
    }

    /// The history popover renders its own item cards, toolbar and empty state.
    /// Both layouts are covered because they are separate render paths.
    #[gpui::test]
    fn the_history_window_renders_in_both_layouts(cx: &mut TestAppContext) {
        use crate::windows::history::HistoryWindow;

        let (_dir, store) = scratch_store();
        install_state(cx, store.clone());

        let window = cx.add_window(|window, cx| HistoryWindow::new(store, window, cx));
        cx.refresh().expect("schedule a redraw");
        cx.run_until_parked();

        window
            .update(cx, |view, _window, cx| view.toggle_layout(cx))
            .expect("switch layout");
        cx.refresh().expect("schedule a redraw");
        cx.run_until_parked();

        // Every filter is a separate visible set, including the empty states.
        for _ in 0..3 {
            window
                .update(cx, |view, _window, cx| view.cycle_filter_for_test(cx))
                .expect("switch filter");
            cx.refresh().expect("schedule a redraw");
            cx.run_until_parked();
        }
    }

    /// Every video editor sidebar panel has to survive a draw. Seven of these
    /// panels read the owning entity mid-render before this test existed, which
    /// would have panicked the moment the panel was opened.
    #[gpui::test]
    fn every_video_editor_panel_renders(cx: &mut TestAppContext) {
        use crate::windows::video_editor::sidebar::SidebarTab;
        use crate::windows::video_editor::VideoEditorWindow;

        let (_dir, store) = scratch_store();
        install_state(cx, store);

        // No project: the panels render against their defaults, which is the
        // state that exercises every control without decoding a video.
        let window = cx.add_window(|_window, cx| VideoEditorWindow::new_for_test(None, cx));

        for tab in SidebarTab::ALL {
            window
                .update(cx, |view, _window, cx| view.open_tab_for_test(tab, cx))
                .expect("open the panel");
            cx.refresh().expect("schedule a redraw");
            cx.run_until_parked();
        }
    }

    /// The image editor, including the zoom control's blurred backdrop, which is
    /// built from the previous frame's measurements and so only exercised by
    /// actually drawing more than one frame.
    #[gpui::test]
    fn the_image_editor_renders_and_measures_its_zoom_bar(cx: &mut TestAppContext) {
        use crate::editor::window::EditorWindow;

        let (dir, store) = scratch_store();
        install_state(cx, store);

        let path = dir.path().join("capture.png");
        image::RgbaImage::from_pixel(600, 400, image::Rgba([90, 120, 200, 255]))
            .save(&path)
            .expect("write a capture to open");
        let path = path.to_string_lossy().to_string();

        let window = cx.add_window(|window, cx| EditorWindow::from_file(&path, window, cx));
        cx.refresh().expect("schedule a redraw");
        cx.run_until_parked();

        // The bar measures itself during prepaint and asks for one more frame;
        // if that plumbing breaks, the backdrop can never be sampled.
        let measured = window
            .update(cx, |view, _window, _cx| {
                view.zoom_bar_bounds.borrow().is_some()
            })
            .expect("read the measurement");
        assert!(measured, "the zoom control recorded its own rect");

        // A zoom change moves the sampled region, which is the rebuild path.
        for zoom in [1.5_f32, 0.5] {
            window
                .update(cx, |view, _window, cx| {
                    view.zoom = zoom;
                    cx.notify();
                })
                .expect("change the zoom");
            cx.refresh().expect("schedule a redraw");
            cx.run_until_parked();
        }
    }

    /// The default window has to survive a draw on its own too.
    #[gpui::test]
    fn the_settings_window_opens(cx: &mut TestAppContext) {
        let (_dir, store) = scratch_store();
        install_state(cx, store.clone());
        let window = cx.add_window(|_window, cx| SettingsWindow::new(store, Category::General, cx));
        cx.refresh().expect("schedule a redraw");
        cx.run_until_parked();
        assert!(
            window.update(cx, |_, _, _| true).unwrap_or(false),
            "the window survived the draw"
        );
    }
}
