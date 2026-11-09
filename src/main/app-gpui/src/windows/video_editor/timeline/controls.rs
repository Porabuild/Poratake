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
use herogpui::components::SliderSize;
use herogpui::components::{Button, Size, Tooltip, Variant};

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
    pub pixels_per_second: f32,
    pub scrub_audio_enabled: bool,
    pub is_scrub_audio_available: bool,
}

fn separator(_theme: &ThemeVars) -> AnyElement {
    herogpui::Separator::new()
        .orientation(herogpui::Orientation::Vertical)
        .mx(px(4.0))
        .h(px(20.0))
        .into_any_element()
}

pub fn render(
    state: &ControlsState,
    theme: &ThemeVars,
    cx: &mut Context<VideoEditorWindow>,
) -> AnyElement {
    let can_zoom_out = state.pixels_per_second > MIN_PIXELS_PER_SECOND;
    let can_zoom_in = state.pixels_per_second < MAX_PIXELS_PER_SECOND;
    let play_name = if state.is_playing { "Pause" } else { "Play" };
    let cut_name = "Cut Tool";
    let delete_name = "Delete Segment";

    let mut bar = div()
        .flex()
        .flex_row()
        .items_center()
        .px(px(4.0))
        .py(px(4.0))
        .border_b_1()
        .border_color(theme.border)
        .child(
            Tooltip::new(icon_button::hinted(play_name, "Space")).child(
                Button::new("timeline-play")
                    .child(crate::ui::icon::icon_element(
                        if state.is_playing { "pause" } else { "play" },
                        px(16.0),
                    ))
                    .variant(Variant::Ghost)
                    .size(Size::Md)
                    .is_icon_only(true)
                    .aria_label(play_name)
                    .on_press(cx.listener(|this, _event, _window, cx| this.toggle_playback(cx))),
            ),
        )
        .child(separator(theme))
        .child(
            Tooltip::new(icon_button::hinted(cut_name, "C")).child(
                Button::new("timeline-cut")
                    .child(crate::ui::icon::icon_element("scissors", px(16.0)))
                    .variant(if state.is_cut_tool_active {
                        Variant::Secondary
                    } else {
                        Variant::Ghost
                    })
                    .size(Size::Md)
                    .is_icon_only(true)
                    .aria_label(cut_name)
                    .on_press(cx.listener(|this, _event, _window, cx| this.toggle_cut_tool(cx))),
            ),
        );

    if state.has_selected_segment && state.can_delete_segment {
        bar = bar.child(separator(theme)).child(
            Tooltip::new(icon_button::hinted(delete_name, "Backspace")).child(
                Button::new("timeline-delete-segment")
                    .child(crate::ui::icon::icon_element("trash-2", px(16.0)))
                    .variant(Variant::Ghost)
                    .size(Size::Md)
                    .is_icon_only(true)
                    .aria_label(delete_name)
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
            theme,
            cx,
        ));
    }

    let zoom_out_name = "Zoom Out";
    let zoom_in_name = "Zoom In";
    let fit_name = "Fit to View";
    bar.child(div().flex_1())
        .child(separator(theme))
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(4.0))
                .child(toolbar::tooltip_button(
                    icon_button::compact_sm("timeline-zoom-out", "minus", zoom_out_name)
                        .is_disabled(!can_zoom_out),
                    icon_button::hinted(zoom_out_name, &accelerator::display("CommandOrControl+-")),
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
                            1.0,
                            cx,
                            |this, next, cx| this.set_timeline_zoom(next as f32, cx),
                        )
                        .size(SliderSize::Sm),
                    ),
                )
                .child(toolbar::tooltip_button(
                    icon_button::compact_sm("timeline-zoom-in", "plus", zoom_in_name)
                        .is_disabled(!can_zoom_in),
                    icon_button::hinted(zoom_in_name, &accelerator::display("CommandOrControl+=")),
                    cx,
                    |this, _window, cx| this.zoom_timeline_in(cx),
                ))
                .child(toolbar::tooltip_button(
                    icon_button::compact_sm("timeline-fit", "maximize-2", fit_name),
                    icon_button::hinted(fit_name, "F"),
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
                    .aria_label("Keyboard shortcuts")
                    .recipe("muted"),
            ),
        )
        .into_any_element()
}

fn speed_selector(
    speed: f64,
    theme: &ThemeVars,
    cx: &mut Context<VideoEditorWindow>,
) -> AnyElement {
    let presets = crate::windows::video_editor::timeline::SPEED_PRESETS;
    let at_min = speed <= presets[0];
    let at_max = speed >= presets[presets.len() - 1];
    let down_name = "Decrease speed";
    let up_name = "Increase speed";
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(2.0))
        .child(toolbar::tooltip_button(
            icon_button::compact_sm("timeline-speed-down", "minus", down_name).is_disabled(at_min),
            down_name,
            cx,
            |this, _window, cx| this.step_selected_segment_speed(-1, cx),
        ))
        .child(
            div()
                .w(px(40.0))
                .text_center()
                .text_size(px(chrome::TEXT_XS))
                .text_color(theme.foreground)
                .child(SharedString::from(
                    crate::windows::video_editor::timeline::format_speed(speed),
                )),
        )
        .child(toolbar::tooltip_button(
            icon_button::compact_sm("timeline-speed-up", "plus", up_name).is_disabled(at_max),
            up_name,
            cx,
            |this, _window, cx| this.step_selected_segment_speed(1, cx),
        ))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use crate::windows::video_editor::timeline::SPEED_PRESETS;

    #[test]
    fn the_stepper_offers_the_renderer_presets_and_nothing_below_them() {
        assert_eq!(
            SPEED_PRESETS,
            [0.5, 0.75, 1.0, 1.25, 1.5, 2.0, 3.0, 4.0],
            "0.25x is a gpui-only speed the renderer never offered"
        );
    }
}
