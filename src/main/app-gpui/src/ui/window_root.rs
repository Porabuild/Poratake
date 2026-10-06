use gpui::{div, prelude::*, App, Div, Window};
use herogpui::gpui;

pub fn focus_root(content: impl IntoElement, window: &mut Window, cx: &mut App) -> Div {
    herogpui::extend::app_focus_root(div().size_full().child(content), window, cx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{point, px, Context, FocusHandle, Render};
    use herogpui::test::TestWindowExt;

    struct Probe {
        own: FocusHandle,
        stop: FocusHandle,
    }

    impl Render for Probe {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let content = div()
                .id("probe-content")
                .size_full()
                .track_focus(&self.own)
                .child(
                    div()
                        .id("probe-stop")
                        .w(px(40.0))
                        .h(px(40.0))
                        .track_focus(&self.stop),
                );
            focus_root(content, window, cx)
        }
    }

    fn open(cx: &mut gpui::TestAppContext) -> (gpui::Entity<Probe>, &mut gpui::VisualTestContext) {
        herogpui::test::open_window(cx, |window, cx| {
            let own = cx.focus_handle();
            window.focus(&own, cx);
            Probe {
                own,
                stop: cx.focus_handle().tab_stop(true),
            }
        })
    }

    #[test]
    fn every_interactive_window_root_is_a_focus_root() {
        let roots = [
            ("capture/overlay.rs", include_str!("../capture/overlay.rs")),
            ("editor/window.rs", include_str!("../editor/window.rs")),
            (
                "windows/capture_preview.rs",
                include_str!("../windows/capture_preview.rs"),
            ),
            (
                "windows/history/mod.rs",
                include_str!("../windows/history/mod.rs"),
            ),
            (
                "windows/onboarding.rs",
                include_str!("../windows/onboarding.rs"),
            ),
            ("windows/pin.rs", include_str!("../windows/pin.rs")),
            (
                "windows/recording_control.rs",
                include_str!("../windows/recording_control.rs"),
            ),
            (
                "windows/scroll_capture.rs",
                include_str!("../windows/scroll_capture.rs"),
            ),
            (
                "windows/settings/mod.rs",
                include_str!("../windows/settings/mod.rs"),
            ),
            (
                "windows/tray_menu.rs",
                include_str!("../windows/tray_menu.rs"),
            ),
            (
                "windows/video_editor/mod.rs",
                include_str!("../windows/video_editor/mod.rs"),
            ),
        ];
        for (name, source) in roots {
            assert!(
                source.contains("window_root::focus_root("),
                "{name} must wrap its window root in `window_root::focus_root`"
            );
        }
    }

    #[herogpui::test]
    fn a_view_focused_before_its_first_paint_keeps_the_focus(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open(cx);
        cx.settle();
        let kept = cx.update(|window, cx| view.read(cx).own.is_focused(window));
        assert!(
            kept,
            "the focus root must not take the window's initial focus"
        );
    }

    #[herogpui::test]
    fn tab_moves_the_focus_and_only_keyboard_input_shows_the_ring(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open(cx);
        assert!(!cx.update(|_, cx| herogpui::extend::focus_visible(cx)));

        cx.press("tab");
        let (on_stop, visible) = cx.update(|window, cx| {
            (
                view.read(cx).stop.is_focused(window),
                herogpui::extend::focus_visible(cx),
            )
        });
        assert!(on_stop, "Tab reaches the first tab stop");
        assert!(visible, "a Tab turns the keyboard focus ring on");

        cx.click_at(point(px(20.0), px(20.0)));
        assert!(
            !cx.update(|_, cx| herogpui::extend::focus_visible(cx)),
            "a pointer press turns the ring off again"
        );
    }
}
