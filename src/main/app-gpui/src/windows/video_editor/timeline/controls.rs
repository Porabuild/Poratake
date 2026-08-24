use gpui::{div, prelude::*, px, AnyElement, Context, Styled};

use crate::system::accelerator;
use crate::theme::vars::ThemeVars;
use crate::ui::button::{Button, ButtonSize, ButtonVariant};
use crate::ui::primitives::Separator;
use crate::ui::slider::Slider;
use crate::ui::switch::{Switch, SwitchSize};
use crate::windows::video_editor::model::format_time;
use crate::windows::video_editor::timeline::{MAX_PIXELS_PER_SECOND, MIN_PIXELS_PER_SECOND};
use crate::windows::video_editor::VideoEditorWindow;

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
    Separator::vertical(px(20.0))
        .inset(px(4.0))
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
            Button::new("timeline-play")
                .variant(ButtonVariant::Ghost)
                .size(ButtonSize::Icon)
                .icon(if state.is_playing { "pause" } else { "play" })
                .tooltip(if state.is_playing {
                    "Pause (Space)"
                } else {
                    "Play (Space)"
                })
                .on_click(cx.listener(|this, _event, _window, cx| this.toggle_playback(cx))),
        )
        .child(separator(theme))
        .child(
            Button::new("timeline-cut")
                .variant(ButtonVariant::Ghost)
                .selected(state.is_cut_tool_active)
                .size(ButtonSize::Icon)
                .icon("scissors")
                .tooltip("Cut Tool (C)")
                .on_click(cx.listener(|this, _event, _window, cx| this.toggle_cut_tool(cx))),
        );

    if state.has_selected_segment && state.can_delete_segment {
        bar = bar.child(separator(theme)).child(
            Button::new("timeline-delete-segment")
                .variant(ButtonVariant::Ghost)
                .size(ButtonSize::Icon)
                .icon("trash-2")
                .foreground(theme.destructive)
                .tooltip("Delete Segment (Backspace)")
                .on_click(
                    cx.listener(|this, _event, _window, cx| this.delete_selected_segment(cx)),
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

    bar.child(div().flex_1())
        .child(separator(theme))
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(4.0))
                .child(
                    Button::new("timeline-zoom-out")
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::IconXs)
                        .icon("minus")
                        .disabled(!can_zoom_out)
                        .tooltip(format!(
                            "Zoom Out ({})",
                            accelerator::display("CommandOrControl+-")
                        ))
                        .on_click(
                            cx.listener(|this, _event, _window, cx| this.zoom_timeline_out(cx)),
                        ),
                )
                .child(
                    div().w(px(96.0)).child(
                        Slider::new(
                            "timeline-zoom",
                            state.pixels_per_second,
                            MIN_PIXELS_PER_SECOND,
                            MAX_PIXELS_PER_SECOND,
                        )
                        .small()
                        .on_change(cx.listener(
                            |this, value: &f32, _window, cx| {
                                this.set_timeline_zoom(*value, cx);
                            },
                        )),
                    ),
                )
                .child(
                    Button::new("timeline-zoom-in")
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::IconXs)
                        .icon("plus")
                        .disabled(!can_zoom_in)
                        .tooltip(format!(
                            "Zoom In ({})",
                            accelerator::display("CommandOrControl+=")
                        ))
                        .on_click(
                            cx.listener(|this, _event, _window, cx| this.zoom_timeline_in(cx)),
                        ),
                )
                .child(
                    Button::new("timeline-fit")
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::IconXs)
                        .icon("maximize-2")
                        .tooltip("Fit to View (F)")
                        .on_click(
                            cx.listener(|this, _event, _window, cx| this.fit_timeline_to_view(cx)),
                        ),
                ),
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
                    Switch::new("timeline-scrub-audio", state.scrub_audio_enabled)
                        .size(SwitchSize::Sm)
                        .disabled(!state.is_scrub_audio_available)
                        .on_change(cx.listener(|this, value: &bool, _window, cx| {
                            this.set_scrub_audio(*value, cx)
                        })),
                ),
        )
        .child(separator(theme))
        .child(
            Button::new("timeline-help")
                .variant(ButtonVariant::Ghost)
                .size(ButtonSize::Icon)
                .icon("help-circle")
                .foreground(theme.muted_foreground)
                .tooltip(if state.is_cut_tool_active {
                    CUT_TOOL_HINT
                } else {
                    DEFAULT_HINT
                }),
        )
        .into_any_element()
}

const SPEED_OPTIONS: [f64; 6] = [0.25, 0.5, 1.0, 1.5, 2.0, 4.0];

fn speed_selector(
    speed: f64,
    theme: &ThemeVars,
    cx: &mut Context<VideoEditorWindow>,
) -> AnyElement {
    let mut row = div().flex().flex_row().items_center().gap(px(2.0));
    for option in SPEED_OPTIONS {
        let active = (option - speed).abs() < 0.001;
        row = row.child(
            Button::new(gpui::SharedString::from(format!(
                "timeline-speed-{}",
                (option * 100.0) as i32
            )))
            .variant(ButtonVariant::Ghost)
            .selected(active)
            .size(ButtonSize::Xs)
            .radius(px(6.0))
            .label(format!("{option}x"))
            .foreground(if active {
                theme.foreground
            } else {
                theme.muted_foreground
            })
            .on_click(cx.listener(move |this, _event, _window, cx| {
                this.set_selected_segment_speed(option, cx)
            })),
        );
    }
    row.into_any_element()
}
