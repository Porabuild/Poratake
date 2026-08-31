use gpui::{
    div, prelude::*, px, AnyElement, Context, MouseDownEvent, MouseMoveEvent, ScrollHandle, Styled,
};

use crate::theme::vars::ThemeVars;
use crate::windows::video_editor::model::format_time;
use crate::windows::video_editor::timeline::{time_at_position, RULER_HEIGHT, TRACK_GUTTER_WIDTH};
use crate::windows::video_editor::VideoEditorWindow;

const TARGET_PIXELS_BETWEEN_MARKS: f32 = 60.0;
const INTERVALS: [f64; 10] = [0.1, 0.25, 0.5, 1.0, 2.0, 5.0, 10.0, 15.0, 30.0, 60.0];

pub fn mark_interval(pixels_per_second: f32) -> f64 {
    let raw = TARGET_PIXELS_BETWEEN_MARKS as f64 / pixels_per_second.max(0.01) as f64;
    INTERVALS
        .into_iter()
        .find(|interval| raw <= *interval)
        .unwrap_or(60.0)
}

pub fn marks(total_duration: f64, pixels_per_second: f32) -> Vec<f64> {
    if total_duration <= 0.0 {
        return Vec::new();
    }
    let interval = mark_interval(pixels_per_second);
    let count = (total_duration / interval).floor() as usize;
    (0..=count).map(|index| index as f64 * interval).collect()
}

pub fn render(
    total_duration: f64,
    pixels_per_second: f32,
    scroll: &ScrollHandle,
    theme: &ThemeVars,
    cx: &mut Context<VideoEditorWindow>,
) -> AnyElement {
    let total_width = (total_duration as f32 * pixels_per_second).max(0.0);
    let mut lane = div().relative().h_full().w(px(total_width)).flex_shrink_0();

    for time in marks(total_duration, pixels_per_second) {
        let left = time as f32 * pixels_per_second;
        let is_first = time == 0.0;
        lane = lane.child(
            div()
                .absolute()
                .top_0()
                .left(px(left))
                .h_full()
                .flex()
                .flex_col()
                .when(!is_first, |el| el.ml(px(-20.0)).items_center())
                .when(is_first, |el| el.items_start())
                .w(px(40.0))
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(theme.muted_foreground)
                        .child(format_time(time)),
                )
                .child(
                    div()
                        .mt(px(2.0))
                        .h(px(8.0))
                        .w(px(1.0))
                        .bg(theme.muted_foreground.opacity(0.3)),
                ),
        );
    }

    div()
        .flex()
        .flex_row()
        .h(px(RULER_HEIGHT))
        .flex_shrink_0()
        .border_b_1()
        .border_color(theme.border)
        .pt(px(4.0))
        .child(div().w(px(TRACK_GUTTER_WIDTH)).flex_shrink_0())
        .child(
            div()
                .id("timeline-ruler-scroll")
                .track_scroll(scroll)
                .relative()
                .flex_1()
                .overflow_x_scroll()
                .cursor_pointer()
                .on_mouse_down(
                    gpui::MouseButton::Left,
                    cx.listener({
                        let scroll = scroll.clone();
                        move |this, event: &MouseDownEvent, _window, cx| {
                            this.begin_scrub();
                            let time = time_at_position(
                                event.position.x,
                                &scroll,
                                pixels_per_second,
                                total_duration,
                            );
                            this.set_playhead(time, cx);
                        }
                    }),
                )
                .on_mouse_move(cx.listener({
                    let scroll = scroll.clone();
                    move |this, event: &MouseMoveEvent, _window, cx| {
                        if !event.dragging() {
                            return;
                        }
                        let time = time_at_position(
                            event.position.x,
                            &scroll,
                            pixels_per_second,
                            total_duration,
                        );
                        this.set_playhead(time, cx);
                    }
                }))
                .child(lane),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_the_first_interval_that_clears_the_target_spacing() {
        assert_eq!(mark_interval(600.0), 0.1);
        assert_eq!(mark_interval(100.0), 1.0);
        assert_eq!(mark_interval(10.0), 10.0);
        assert_eq!(mark_interval(0.5), 60.0);
    }

    #[test]
    fn emits_marks_up_to_the_total_duration() {
        assert_eq!(marks(5.0, 100.0), vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0]);
        assert!(marks(0.0, 100.0).is_empty());
    }
}
