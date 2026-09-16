use gpui::{div, prelude::*, px, AnyElement, Context, SharedString, Styled};
use herogpui::gpui;

use crate::system::accelerator;
use crate::theme::vars::ThemeVars;
use crate::ui::chrome;
use crate::ui::icon_button;
use crate::ui::rows;
use crate::ui::toolbar;
use crate::util::format::format_time;
use crate::windows::video_editor::timeline::{MAX_PIXELS_PER_SECOND, MIN_PIXELS_PER_SECOND};
use crate::windows::video_editor::VideoEditorWindow;
use herogpui::components::{Button, Size, Tooltip, Variant};
use herogpui::components::{Slider, SliderSize};

const CUT_TOOL_HINT: &str =
    "Click a track to cut all tracks at that position | Shift+Click to cut a single track";
const DEFAULT_HINT: &str = "Drag edges to trim | Hover to scrub | Click to select | Backspace to delete | \u{2190}/\u{2192} seek 1s (Shift 5s) | , . step frame | Home/End jump | F fit to view";

pub struct ControlsState {
    pub is_playing: bool,
    pub is_cut_tool_active: bool,
    pub has_selected_segment: bool,
    pub can_delete_segment: bool,
    pub timeline_position: f64,
    pub total_duration: f64,
    pub segment_count: usize,
    pub selected_segment_speed: f64,
    pub speed_selector_open: bool,
    pub pixels_per_second: f32,
    pub scrub_audio_enabled: bool,
    pub is_scrub_audio_available: bool,
}

fn separator(_theme: &ThemeVars) -> AnyElement {
    herogpui::Separator::new()
        .orientation(herogpui::Orientation::Vertical)
        .mx(px(4.0))
        .sx(|el| el.h(px(20.0)))
        .into_any_element()
}

pub fn render(
    state: &ControlsState,
    theme: &ThemeVars,
    cx: &mut Context<VideoEditorWindow>,
) -> AnyElement {
    let can_zoom_out = state.pixels_per_second > MIN_PIXELS_PER_SECOND;
    let can_zoom_in = state.pixels_per_second < MAX_PIXELS_PER_SECOND;

    let mut bar = div()
        .flex()
        .flex_row()
        .items_center()
        .px(px(4.0))
        .py(px(4.0))
        .border_b_1()
        .border_color(theme.border)
        .child(
            Tooltip::new(if state.is_playing {
                "Pause (Space)"
            } else {
                "Play (Space)"
            })
            .child(
                Button::new("timeline-play")
                    .child(crate::ui::icon::icon_element(
                        if state.is_playing { "pause" } else { "play" },
                        px(16.0),
                    ))
                    .variant(Variant::Ghost)
                    .size(Size::Md)
                    .is_icon_only(true)
                    .on_press(cx.listener(|this, _event, _window, cx| this.toggle_playback(cx))),
            ),
        )
        .child(separator(theme))
        .child(
            Tooltip::new("Cut Tool (C)").child(
                Button::new("timeline-cut")
                    .child(crate::ui::icon::icon_element("scissors", px(16.0)))
                    .variant(if state.is_cut_tool_active {
                        Variant::Secondary
                    } else {
                        Variant::Ghost
                    })
                    .size(Size::Md)
                    .is_icon_only(true)
                    .on_press(cx.listener(|this, _event, _window, cx| this.toggle_cut_tool(cx))),
            ),
        );

    if state.has_selected_segment && state.can_delete_segment {
        bar = bar.child(separator(theme)).child(
            Tooltip::new("Delete Segment (Backspace)").child(
                Button::new("timeline-delete-segment")
                    .child(crate::ui::icon::icon_element("trash-2", px(16.0)))
                    .variant(Variant::Ghost)
                    .size(Size::Md)
                    .is_icon_only(true)
                    .recipe("danger-text")
                    .on_press(
                        cx.listener(|this, _event, _window, cx| this.delete_selected_segment(cx)),
                    ),
            ),
        );
    }

    let mut readout = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.0))
        .text_size(px(13.0))
        .text_color(theme.muted_foreground)
        .child(format!(
            "{} / {}",
            format_time(state.timeline_position),
            format_time(state.total_duration)
        ));
    if state.segment_count > 1 {
        readout = readout.child(
            div()
                .text_color(crate::theme::color::Srgba::parse("#f59e0b").to_hsla())
                .child(format!("({} clips)", state.segment_count)),
        );
    }

    bar = bar.child(separator(theme)).child(readout);

    if state.has_selected_segment {
        bar = bar.child(separator(theme)).child(speed_selector(
            state.selected_segment_speed,
            state.speed_selector_open,
            theme,
            cx,
        ));
    }

    bar.child(div().flex_1())
        .child(separator(theme))
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(4.0))
                .child(toolbar::tooltip_button(
                    icon_button::compact_sm("timeline-zoom-out", "minus")
                        .is_disabled(!can_zoom_out),
                    format!("Zoom Out ({})", accelerator::display("CommandOrControl+-")),
                    cx,
                    |this, _window, cx| this.zoom_timeline_out(cx),
                ))
                .child(
                    div().w(px(96.0)).child(
                        rows::slider_control(
                            "timeline-zoom",
                            state.pixels_per_second as f64,
                            MIN_PIXELS_PER_SECOND as f64,
                            MAX_PIXELS_PER_SECOND as f64,
                            0.0,
                            cx,
                            |this, next, cx| this.set_timeline_zoom(next as f32, cx),
                        )
                        .size(SliderSize::Sm),
                    ),
                )
                .child(toolbar::tooltip_button(
                    icon_button::compact_sm("timeline-zoom-in", "plus").is_disabled(!can_zoom_in),
                    format!("Zoom In ({})", accelerator::display("CommandOrControl+=")),
                    cx,
                    |this, _window, cx| this.zoom_timeline_in(cx),
                ))
                .child(toolbar::tooltip_button(
                    icon_button::compact_sm("timeline-fit", "maximize-2"),
                    "Fit to View (F)",
                    cx,
                    |this, _window, cx| this.fit_timeline_to_view(cx),
                )),
        )
        .child(separator(theme))
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(theme.muted_foreground)
                        .child("Scrub Audio"),
                )
                .child(
                    rows::switch(
                        "timeline-scrub-audio",
                        state.scrub_audio_enabled,
                        cx,
                        |this, enabled, cx| this.set_scrub_audio(enabled, cx),
                    )
                    .size(Size::Sm)
                    .is_disabled(!state.is_scrub_audio_available),
                ),
        )
        .child(separator(theme))
        .child(
            Tooltip::new(if state.is_cut_tool_active {
                CUT_TOOL_HINT
            } else {
                DEFAULT_HINT
            })
            .child(
                Button::new("timeline-help")
                    .child(crate::ui::icon::icon_element("help-circle", px(16.0)))
                    .variant(Variant::Ghost)
                    .size(Size::Md)
                    .is_icon_only(true)
                    .recipe("muted"),
            ),
        )
        .into_any_element()
}

const MIN_SPEED: f64 = 0.25;
const NORMAL_SPEED: f64 = 1.0;
const MAX_SPEED: f64 = 4.0;
const SPEED_PRESETS: [f64; 9] = [0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 2.0, 3.0, 4.0];

fn speed_position(speed: f64) -> f32 {
    let speed = speed.clamp(MIN_SPEED, MAX_SPEED);
    if speed <= NORMAL_SPEED {
        (((speed - MIN_SPEED) / (NORMAL_SPEED - MIN_SPEED)) * 0.5) as f32
    } else {
        (0.5 + ((speed - NORMAL_SPEED) / (MAX_SPEED - NORMAL_SPEED)) * 0.5) as f32
    }
}

fn speed_at_position(position: f32) -> f64 {
    let position = f64::from(position.clamp(0.0, 1.0));
    let speed = if position <= 0.5 {
        MIN_SPEED + (position / 0.5) * (NORMAL_SPEED - MIN_SPEED)
    } else {
        NORMAL_SPEED + ((position - 0.5) / 0.5) * (MAX_SPEED - NORMAL_SPEED)
    };
    (speed * 20.0).round() / 20.0
}

fn speed_selector(
    speed: f64,
    open: bool,
    theme: &ThemeVars,
    cx: &mut Context<VideoEditorWindow>,
) -> AnyElement {
    let drag_view = cx.entity().downgrade();
    let drop_view = drag_view.clone();
    let radius = px(6.0);
    let speed_label: SharedString = format!("{}x", (speed * 20.0).round() / 20.0).into();
    let content_label = speed_label.clone();
    let mut selector = div().id("timeline-speed-selector").relative().child(
        herogpui::components::Tooltip::new("Playback Speed").child(
            Button::new("timeline-speed")
                .variant(if open {
                    Variant::Secondary
                } else {
                    Variant::Ghost
                })
                .label(speed_label)
                .content(move |_| {
                    crate::ui::primitives::icon_label(
                        "chevron-down",
                        content_label.clone(),
                        px(chrome::BUTTON_XS_ICON),
                        px(8.0),
                        true,
                    )
                })
                .recipe("compact")
                .recipe("muted")
                .sx(move |el| el.rounded(radius))
                .on_press(cx.listener(|this, _event, _window, cx| this.press_speed_selector(cx))),
        ),
    );
    if open {
        selector = selector.child(
            div()
                .absolute()
                .bottom(px(34.0))
                .left(px(-72.0))
                .w(px(208.0))
                .flex()
                .flex_col()
                .gap(px(8.0))
                .rounded(px(8.0))
                .border_1()
                .border_color(theme.border)
                .bg(theme.card)
                .shadow_lg()
                .p(px(12.0))
                .occlude()
                .on_mouse_down_out(
                    cx.listener(|this, _event, _window, cx| this.dismiss_speed_selector(cx)),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .text_size(px(11.0))
                        .text_color(theme.muted_foreground)
                        .child("0.25x")
                        .child("1x")
                        .child("4x"),
                )
                .child(
                    Slider::new("timeline-speed-slider", speed_position(speed))
                        .min_value(0.0)
                        .max_value(1.0)
                        .size(SliderSize::Sm)
                        .steps(SPEED_PRESETS.map(speed_position))
                        .on_drag_start(move |_window, cx| {
                            let _ = drag_view.update(cx, |this, _cx| this.begin_slider_gesture());
                        })
                        .on_drag_end(move |_window, cx| {
                            let _ = drop_view.update(cx, |this, cx| this.end_slider_gesture(cx));
                        })
                        .on_change(cx.listener(|this, value: &f32, _window, cx| {
                            this.set_selected_segment_speed(speed_at_position(*value), cx)
                        })),
                ),
        );
    }
    selector.into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_speed_is_the_slider_midpoint() {
        assert_eq!(speed_position(1.0), 0.5);
        assert_eq!(speed_at_position(0.5), 1.0);
        assert_eq!(speed_at_position(0.0), 0.25);
        assert_eq!(speed_at_position(1.0), 4.0);
    }

    #[test]
    fn speed_presets_keep_normal_speed_in_the_middle() {
        let positions = SPEED_PRESETS.map(speed_position);
        assert_eq!(positions[3], 0.5);
        assert!(positions.windows(2).all(|steps| steps[0] < steps[1]));
    }
}
