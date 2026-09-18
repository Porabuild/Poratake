use gpui::{
    div, linear_color_stop, linear_gradient, point, prelude::*, px, AnyElement, Context, Hsla,
    MouseDownEvent, MouseMoveEvent, ScrollHandle, ScrollWheelEvent, SharedString, Styled,
};
use herogpui::gpui;

use crate::theme::color::Srgba;
use crate::theme::vars::ThemeVars;
use crate::ui::icon::icon_element;
use crate::ui::menu::MenuHandle;
use crate::windows::video_editor::timeline::{
    menu, quantize_scrub, reorder, time_at_position, TRACK_GUTTER_WIDTH, TRACK_HEIGHT,
};
use crate::windows::video_editor::{ClipDrag, DragMode, DrawDrag, ReorderDrag, VideoEditorWindow};

const RESIZE_HANDLE: f32 = 12.0;
const EDGE_RESIZE_RATIO: f64 = 0.15;
const MAX_EDGE_THRESHOLD: f64 = 0.3;
const CLIP_GAP: f32 = 2.0;
const CLIP_LABEL_TEXT: f32 = 14.0;
const PLAYHEAD_COLOR: &str = "#ef4444";
const CUT_MARKER_COLOR: &str = "#f59e0b";

pub fn can_add_to(kind: TrackKind) -> bool {
    matches!(
        kind,
        TrackKind::Zoom | TrackKind::Camera | TrackKind::Drawing
    )
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TrackKind {
    Video,
    Zoom,
    Camera,
    Drawing,
    Music,
}

impl TrackKind {
    pub fn id(self) -> &'static str {
        match self {
            Self::Video => "video",
            Self::Zoom => "zoom",
            Self::Camera => "camera",
            Self::Drawing => "drawing",
            Self::Music => "music",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Self::Video => "film",
            Self::Zoom => "zoom-in",
            Self::Camera => "camera",
            Self::Drawing => "pen-line",
            Self::Music => "volume-2",
        }
    }

    pub(crate) fn gradient(self) -> (&'static str, &'static str) {
        match self {
            Self::Video => ("#d97706", "#b45309"),
            Self::Zoom => ("#818cf8", "#4f46e5"),
            Self::Camera => ("#f472b6", "#be185d"),
            Self::Drawing => ("#2dd4bf", "#0f766e"),
            Self::Music => ("#c084fc", "#7e22ce"),
        }
    }

    fn empty_text(self) -> Option<&'static str> {
        match self {
            Self::Zoom => Some("Click or drag to add zoom"),
            Self::Camera => Some("Drag to show camera"),
            _ => None,
        }
    }

    fn label_rule(self) -> LabelRule {
        match self {
            Self::Video => LabelRule {
                icon_always: false,
                text_at: 100.0,
                detail_at: None,
            },
            Self::Zoom => LabelRule {
                icon_always: true,
                text_at: 50.0,
                detail_at: None,
            },
            Self::Camera | Self::Drawing => LabelRule {
                icon_always: true,
                text_at: 100.0,
                detail_at: None,
            },
            Self::Music => LabelRule {
                icon_always: true,
                text_at: 100.0,
                detail_at: Some(140.0),
            },
        }
    }
}

pub fn source_icon(source: &str) -> &'static str {
    match source {
        "system" => "volume-2",
        "mic" => "mic",
        _ => "music",
    }
}

pub fn drawing_icon(kind: &str) -> &'static str {
    match kind {
        "highlight" => "highlighter",
        "rectangle" => "square",
        "circle" => "circle",
        "line" => "minus",
        "arrow" => "arrow-up-right",
        "text" => "type",
        "number" => "hash",
        "redact" => "eraser",
        _ => "pen-line",
    }
}

pub fn drawing_label(kind: &str) -> &'static str {
    match kind {
        "highlight" => "Highlight",
        "rectangle" => "Rectangle",
        "circle" => "Circle",
        "line" => "Line",
        "arrow" => "Arrow",
        "text" => "Text",
        "number" => "Number",
        "redact" => "Redact",
        _ => "Pen",
    }
}

pub fn drawing_gradient(kind: &str) -> (&'static str, &'static str) {
    match kind {
        "highlight" => ("#facc15", "#a16207"),
        "rectangle" => ("#38bdf8", "#0369a1"),
        "circle" => ("#22d3ee", "#0e7490"),
        "line" => ("#a3e635", "#4d7c0f"),
        "arrow" => ("#34d399", "#047857"),
        "text" => ("#f472b6", "#be185d"),
        "number" => ("#fb923c", "#c2410c"),
        "redact" => ("#fb7185", "#9f1239"),
        _ => ("#2dd4bf", "#0f766e"),
    }
}

struct LabelRule {
    icon_always: bool,
    text_at: f32,
    detail_at: Option<f32>,
}

pub struct Clip {
    pub id: SharedString,
    pub label: SharedString,
    pub detail: Option<SharedString>,
    pub badge: Option<SharedString>,
    pub icon: &'static str,
    pub start: f64,
    pub duration: f64,
    pub selected: bool,
    pub gradient: (&'static str, &'static str),
    pub zoom_level: Option<f64>,
}

impl Clip {
    pub fn new(id: String, selected: bool, start: f64, duration: f64) -> Self {
        Self {
            id: SharedString::from(id),
            label: SharedString::default(),
            detail: None,
            badge: None,
            icon: "film",
            start,
            duration: duration.max(0.0),
            selected,
            gradient: TrackKind::Video.gradient(),
            zoom_level: None,
        }
    }

    fn end(&self) -> f64 {
        self.start + self.duration
    }
}

pub struct MusicLane {
    pub group_id: SharedString,
    pub speed: f64,
    pub removable: bool,
}

pub struct Track {
    pub kind: TrackKind,
    pub lane_id: SharedString,
    pub icon: &'static str,
    pub empty_text: Option<&'static str>,
    pub clips: Vec<Clip>,
    pub music: Option<MusicLane>,
}

impl Track {
    pub fn new(kind: TrackKind, clips: Vec<Clip>) -> Self {
        Self {
            kind,
            lane_id: SharedString::from(kind.id()),
            icon: kind.icon(),
            empty_text: kind.empty_text(),
            clips,
            music: None,
        }
    }

    pub fn lane(mut self, suffix: &str) -> Self {
        self.lane_id = SharedString::from(format!("{}-{suffix}", self.kind.id()));
        self
    }

    pub fn icon(mut self, icon: &'static str) -> Self {
        self.icon = icon;
        self
    }

    pub fn music(mut self, music: MusicLane) -> Self {
        self.music = Some(music);
        self
    }
}

pub struct Timeline<'a> {
    pub tracks: &'a [Track],
    pub display_duration: f64,
    pub total_duration: f64,
    pub pixels_per_second: f32,
    pub playhead: f64,
    pub is_cut_tool_active: bool,
    pub clip_drag: Option<&'a ClipDrag>,
    pub reorder: Option<&'a ReorderDrag>,
    pub draw: Option<&'a DrawDrag>,
    pub menu: &'a MenuHandle,
    pub scroll: &'a ScrollHandle,
}

pub fn handle_seconds(duration: f64) -> f64 {
    (duration * EDGE_RESIZE_RATIO).min(MAX_EDGE_THRESHOLD)
}

pub fn handle_width(duration: f64, pixels_per_second: f32) -> f32 {
    ((handle_seconds(duration) * pixels_per_second as f64) as f32).clamp(1.0, RESIZE_HANDLE)
}

pub fn clip_geometry(
    start: f64,
    duration: f64,
    previous_end: Option<f64>,
    pixels_per_second: f32,
) -> (f32, f32) {
    let raw_width = duration as f32 * pixels_per_second;
    let adjacent = previous_end.is_some_and(|end| (start - end).abs() < 0.01);
    let gap = if adjacent && raw_width > CLIP_GAP {
        CLIP_GAP
    } else {
        0.0
    };
    (
        start as f32 * pixels_per_second + gap,
        (raw_width - gap).max(2.0),
    )
}

fn gradient_fill(from: &str, to: &str, alpha: f32) -> gpui::Background {
    linear_gradient(
        180.0,
        linear_color_stop(Srgba::parse(from).to_hsla().opacity(alpha), 0.0),
        linear_color_stop(Srgba::parse(to).to_hsla().opacity(alpha), 1.0),
    )
}

fn glow(color: Hsla, blur: f32) -> Vec<gpui::BoxShadow> {
    vec![gpui::BoxShadow {
        color: color.opacity(0.6),
        offset: point(px(0.0), px(0.0)),
        blur_radius: px(blur),
        spread_radius: px(0.0),
        inset: false,
    }]
}

fn playhead(position_pixels: f32) -> AnyElement {
    let color: Hsla = Srgba::parse(PLAYHEAD_COLOR).to_hsla();
    div()
        .absolute()
        .top_0()
        .bottom_0()
        .left(px(position_pixels))
        .w(px(2.0))
        .bg(color)
        .shadow(glow(color, 8.0))
        .child(
            div()
                .absolute()
                .top(px(-12.0))
                .left(px(-5.0))
                .size(px(12.0))
                .rounded_full()
                .bg(color)
                .shadow(glow(color, 8.0)),
        )
        .into_any_element()
}

fn cut_marker(left_pixels: f32) -> AnyElement {
    let amber: Hsla = Srgba::parse(CUT_MARKER_COLOR).to_hsla();
    div()
        .absolute()
        .top_0()
        .left(px(left_pixels - 12.0))
        .w(px(24.0))
        .h_full()
        .flex()
        .flex_col()
        .items_center()
        .child(
            div()
                .mt(px(-10.0))
                .size(px(20.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                .bg(amber)
                .text_color(crate::ui::colors::white(1.0))
                .child(icon_element("scissors", px(12.0))),
        )
        .child(div().w(px(2.0)).flex_1().bg(amber.opacity(0.5)))
        .into_any_element()
}

fn drop_indicator(left_pixels: f32) -> AnyElement {
    let white = crate::ui::colors::white(1.0);
    div()
        .absolute()
        .top_0()
        .left(px(left_pixels))
        .w(px(2.0))
        .h_full()
        .bg(white)
        .shadow(glow(white, 6.0))
        .into_any_element()
}

fn draw_preview(drag: &DrawDrag, pixels_per_second: f32) -> AnyElement {
    let from = drag.start.min(drag.end) as f32 * pixels_per_second;
    let width = ((drag.end - drag.start).abs() as f32 * pixels_per_second).max(2.0);
    let gradient = drag.kind.gradient();
    div()
        .absolute()
        .top_0()
        .left(px(from))
        .w(px(width))
        .h_full()
        .border_2()
        .border_dashed()
        .border_color(Srgba::parse(gradient.0).to_hsla())
        .bg(gradient_fill(gradient.0, gradient.1, 0.4))
        .into_any_element()
}

fn label_element(clip: &Clip, kind: TrackKind, width: f32, theme: &ThemeVars) -> AnyElement {
    let rule = kind.label_rule();
    let mut row = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .px(px(6.0))
        .overflow_hidden();
    if rule.icon_always || width < rule.text_at {
        row = row.child(icon_element(clip.icon, px(12.0)));
    }
    if width >= rule.text_at && !clip.label.is_empty() {
        row = row.child(div().truncate().child(clip.label.clone()));
    }
    if let Some(detail) = clip.detail.clone() {
        if rule.detail_at.is_some_and(|at| width >= at) {
            row = row.child(
                div()
                    .text_color(theme.foreground.opacity(0.7))
                    .child(detail),
            );
        }
    }
    if let Some(badge) = clip.badge.clone() {
        if width >= rule.text_at {
            row = row.child(
                div()
                    .rounded(px(4.0))
                    .bg(crate::ui::colors::white(0.2))
                    .px(px(6.0))
                    .text_size(px(11.0))
                    .child(badge),
            );
        }
    }
    row.into_any_element()
}

fn resize_handle(
    clip: &Clip,
    kind: TrackKind,
    start_edge: bool,
    width: f32,
    cx: &mut Context<VideoEditorWindow>,
) -> AnyElement {
    let id = clip.id.clone();
    div()
        .id(SharedString::from(format!(
            "clip-handle-{}-{}",
            clip.id,
            if start_edge { "start" } else { "end" }
        )))
        .absolute()
        .top_0()
        .h_full()
        .w(px(width))
        .when(start_edge, |el| el.left_0())
        .when(!start_edge, |el| el.right_0())
        .flex()
        .items_center()
        .when(start_edge, |el| el.justify_start())
        .when(!start_edge, |el| el.justify_end())
        .cursor_ew_resize()
        .hover(|el| el.bg(crate::ui::colors::white(0.2)))
        .child(
            div()
                .mx(px(2.0))
                .h(px(16.0))
                .w(px(4.0))
                .rounded_full()
                .bg(crate::ui::colors::white(0.4)),
        )
        .on_mouse_down(
            gpui::MouseButton::Left,
            cx.listener(move |this, _event: &MouseDownEvent, _window, cx| {
                cx.stop_propagation();
                this.select_clip(id.clone(), cx);
                this.begin_clip_drag(
                    ClipDrag {
                        kind,
                        id: id.clone(),
                        mode: if start_edge {
                            DragMode::ResizeStart
                        } else {
                            DragMode::ResizeEnd
                        },
                        grab_offset: 0.0,
                    },
                    cx,
                );
            }),
        )
        .into_any_element()
}

#[derive(Clone, Copy)]
struct ClipContext<'a> {
    kind: TrackKind,
    pixels_per_second: f32,
    total_duration: f64,
    is_cut_tool_active: bool,
    is_moving: bool,
    is_reordering: bool,
    siblings: usize,
    music: Option<&'a MusicLane>,
    menu: &'a MenuHandle,
    scroll: &'a ScrollHandle,
}

fn clip_element(
    clip: &Clip,
    previous_end: Option<f64>,
    context: &ClipContext<'_>,
    theme: &ThemeVars,
    cx: &mut Context<VideoEditorWindow>,
) -> AnyElement {
    let kind = context.kind;
    let pixels_per_second = context.pixels_per_second;
    let total_duration = context.total_duration;
    let is_cut_tool_active = context.is_cut_tool_active;
    let (left, width) = clip_geometry(clip.start, clip.duration, previous_end, pixels_per_second);
    let background = if clip.selected {
        linear_gradient(
            180.0,
            linear_color_stop(theme.primary, 0.0),
            linear_color_stop(theme.accent_hover, 1.0),
        )
    } else {
        gradient_fill(clip.gradient.0, clip.gradient.1, 1.0)
    };
    let id = clip.id.clone();
    let handle = handle_width(clip.duration, pixels_per_second);

    let mut element = div()
        .id(SharedString::from(format!("clip-{}", clip.id)))
        .absolute()
        .top_0()
        .left(px(left))
        .w(px(width))
        .h_full()
        .overflow_hidden()
        .bg(background)
        .when(context.is_reordering, |el| el.opacity(0.4))
        .when(context.is_moving, |el| el.cursor_grabbing())
        .when(is_cut_tool_active, |el| el.cursor_crosshair())
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(CLIP_LABEL_TEXT))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(crate::ui::colors::white(1.0))
        .child(label_element(clip, kind, width, theme))
        .on_mouse_down(
            gpui::MouseButton::Left,
            cx.listener({
                let id = id.clone();
                let scroll = context.scroll.clone();
                let start = clip.start;
                let duration = clip.duration;
                move |this, event: &MouseDownEvent, _window, cx| {
                    cx.stop_propagation();
                    let time = time_at_position(
                        event.position.x,
                        &scroll,
                        pixels_per_second,
                        total_duration,
                    );
                    if is_cut_tool_active {
                        let only = event.modifiers.shift.then_some(kind);
                        this.cut_at(time, only, cx);
                        return;
                    }
                    this.select_clip(id.clone(), cx);
                    if kind == TrackKind::Video {
                        this.begin_reorder(id.clone(), f32::from(event.position.x), cx);
                        return;
                    }
                    this.begin_clip_drag(
                        ClipDrag {
                            kind,
                            id: id.clone(),
                            mode: DragMode::Move,
                            grab_offset: (time - start).clamp(0.0, duration),
                        },
                        cx,
                    );
                }
            }),
        );

    if !is_cut_tool_active {
        element = element
            .child(resize_handle(clip, kind, true, handle, cx))
            .child(resize_handle(clip, kind, false, handle, cx));
    }

    if !menu::has_menu(kind) {
        return element.into_any_element();
    }

    let entries = menu::clip_menu(
        kind,
        clip,
        context.music,
        context.siblings,
        cx.entity().downgrade(),
    );
    element
        .on_mouse_down(
            gpui::MouseButton::Right,
            cx.listener({
                let menu = context.menu.clone();
                let id = id.clone();
                move |this, event: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    this.select_clip(id.clone(), cx);
                    menu.open_at(event.position, entries.clone(), window, cx);
                }
            }),
        )
        .into_any_element()
}

fn gutter(tracks: &[Track], theme: &ThemeVars) -> AnyElement {
    let mut gutter = div()
        .flex()
        .flex_col()
        .w(px(TRACK_GUTTER_WIDTH))
        .flex_shrink_0()
        .border_r_1()
        .border_color(theme.border);
    for track in tracks {
        gutter = gutter.child(
            div()
                .h(px(TRACK_HEIGHT))
                .flex_shrink_0()
                .flex()
                .items_center()
                .justify_center()
                .border_b_1()
                .border_color(theme.border)
                .text_color(theme.muted_foreground)
                .child(icon_element(track.icon, px(12.0))),
        );
    }
    gutter.into_any_element()
}

fn lane(
    track: &Track,
    timeline: &Timeline<'_>,
    theme: &ThemeVars,
    cx: &mut Context<VideoEditorWindow>,
) -> AnyElement {
    let kind = track.kind;
    let pixels_per_second = timeline.pixels_per_second;
    let total_duration = timeline.total_duration;
    let is_cut_tool_active = timeline.is_cut_tool_active;
    let drawable = can_add_to(kind);
    let moving_id = timeline
        .clip_drag
        .filter(|drag| drag.mode == DragMode::Move)
        .map(|drag| drag.id.clone());
    let reordering_id = timeline
        .reorder
        .filter(|drag| drag.dragging)
        .map(|drag| drag.id.clone());

    let mut lane = div()
        .id(track.lane_id.clone())
        .relative()
        .h(px(TRACK_HEIGHT))
        .flex_shrink_0()
        .w_full()
        .border_b_1()
        .border_color(theme.border)
        .when(is_cut_tool_active, |el| el.cursor_crosshair())
        .when(!is_cut_tool_active && drawable, |el| el.cursor_pointer())
        .on_mouse_down(
            gpui::MouseButton::Left,
            cx.listener({
                let scroll = timeline.scroll.clone();
                move |this, event: &MouseDownEvent, _window, cx| {
                    let time = time_at_position(
                        event.position.x,
                        &scroll,
                        pixels_per_second,
                        total_duration,
                    );
                    if is_cut_tool_active {
                        cx.stop_propagation();
                        let only = event.modifiers.shift.then_some(kind);
                        this.cut_at(time, only, cx);
                        return;
                    }
                    if drawable {
                        cx.stop_propagation();
                        this.begin_draw(kind, time, cx);
                        return;
                    }
                    this.set_playhead(time, cx);
                }
            }),
        );

    let context = ClipContext {
        kind,
        pixels_per_second,
        total_duration,
        is_cut_tool_active,
        is_moving: false,
        is_reordering: false,
        siblings: track.clips.len(),
        music: track.music.as_ref(),
        menu: timeline.menu,
        scroll: timeline.scroll,
    };
    let mut previous_end = None;
    for clip in &track.clips {
        let context = ClipContext {
            is_moving: moving_id.as_ref() == Some(&clip.id),
            is_reordering: reordering_id.as_ref() == Some(&clip.id),
            ..context
        };
        lane = lane.child(clip_element(clip, previous_end, &context, theme, cx));
        previous_end = Some(clip.end());
    }

    if kind == TrackKind::Video && reordering_id.is_none() {
        for clip in track.clips.iter().take(track.clips.len().saturating_sub(1)) {
            lane = lane.child(cut_marker(clip.end() as f32 * pixels_per_second));
        }
    }

    if let Some(drag) = timeline.draw.filter(|drag| drag.kind == kind) {
        lane = lane.child(draw_preview(drag, pixels_per_second));
    }

    if kind == TrackKind::Video {
        if let Some(drag) = timeline.reorder.filter(|drag| drag.dragging) {
            let durations: Vec<f64> = track.clips.iter().map(|clip| clip.duration).collect();
            let dragged = track
                .clips
                .iter()
                .position(|clip| clip.id == drag.id)
                .unwrap_or_default();
            if let Some(time) = reorder::drop_indicator_time(&durations, dragged, drag.drop_index) {
                lane = lane.child(drop_indicator(time as f32 * pixels_per_second));
            }
        }
    }

    if let Some(text) = track.empty_text.filter(|_| track.clips.is_empty()) {
        if timeline.draw.is_none() {
            lane = lane.child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(crate::ui::chrome::TEXT_XS))
                    .text_color(theme.muted_foreground)
                    .child(text),
            );
        }
    }

    lane.into_any_element()
}

pub fn render(
    timeline: &Timeline<'_>,
    theme: &ThemeVars,
    cx: &mut Context<VideoEditorWindow>,
) -> AnyElement {
    let pixels_per_second = timeline.pixels_per_second;
    let total_duration = timeline.total_duration;
    let total_width = (timeline.display_duration as f32 * pixels_per_second).max(1.0);

    let mut lanes = div().relative().flex().flex_col().w(px(total_width));
    for track in timeline.tracks {
        lanes = lanes.child(lane(track, timeline, theme, cx));
    }
    lanes = lanes.child(playhead(timeline.playhead as f32 * pixels_per_second));

    div()
        .flex()
        .flex_row()
        .flex_shrink_0()
        .child(gutter(timeline.tracks, theme))
        .child(
            div()
                .relative()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .id("timeline-tracks-scroll")
                        .track_scroll(timeline.scroll)
                        .relative()
                        .size_full()
                        .overflow_x_scroll()
                        .on_scroll_wheel(cx.listener({
                            let scroll = timeline.scroll.clone();
                            move |this, event: &ScrollWheelEvent, _window, cx| {
                                let delta = event.delta.pixel_delta(px(TRACK_HEIGHT));
                                if event.modifiers.platform || event.modifiers.control {
                                    let pointer =
                                        f32::from(event.position.x - scroll.bounds().left())
                                            .max(0.0);
                                    this.zoom_timeline_at(f32::from(delta.y), pointer, cx);
                                    return;
                                }
                                if event.modifiers.shift && f32::from(delta.x) == 0.0 {
                                    this.scroll_timeline_by(f32::from(delta.y), total_width, cx);
                                }
                            }
                        }))
                        .on_mouse_down(
                            gpui::MouseButton::Left,
                            cx.listener({
                                let scroll = timeline.scroll.clone();
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
                            let scroll = timeline.scroll.clone();
                            move |this, event: &MouseMoveEvent, _window, cx| {
                                let time = time_at_position(
                                    event.position.x,
                                    &scroll,
                                    pixels_per_second,
                                    total_duration,
                                );
                                if !event.dragging() {
                                    this.preview_seek(Some(quantize_scrub(time)), cx);
                                    return;
                                }
                                if this.update_timeline_drag(time, f32::from(event.position.x), cx)
                                {
                                    return;
                                }
                                this.set_playhead(time, cx);
                            }
                        }))
                        .on_mouse_exit(cx.listener(
                            |this, _event: &gpui::MouseExitEvent, _window, cx| {
                                this.preview_seek(None, cx);
                            },
                        ))
                        .child(lanes),
                )
                .child(crate::windows::scrollbars::overlay_horizontal(
                    "timeline-tracks-scrollbar",
                    timeline.scroll,
                    theme.muted_foreground,
                )),
        )
        .into_any_element()
}

pub fn video_clips(
    segments: &[crate::windows::video_editor::model::Segment],
    selected: Option<&str>,
) -> Vec<Clip> {
    let mut clips = Vec::with_capacity(segments.len());
    let mut start = 0.0;
    for segment in segments {
        let duration = segment.timeline_duration();
        let speed = segment.speed.unwrap_or(1.0);
        let mut clip = Clip::new(
            segment.id.clone(),
            selected == Some(segment.id.as_str()),
            start,
            duration,
        );
        clip.label = SharedString::from(crate::util::format::format_duration(duration));
        clip.gradient = TrackKind::Video.gradient();
        if (speed - 1.0).abs() > f64::EPSILON {
            clip.badge = Some(SharedString::from(
                crate::windows::video_editor::timeline::format_speed(speed),
            ));
        }
        clips.push(clip);
        start += duration;
    }
    clips
}

pub fn range_clips<T>(
    items: &[T],
    selected: Option<&str>,
    id: impl Fn(&T) -> String,
    range: impl Fn(&T) -> (f64, f64),
    decorate: impl Fn(&T, &mut Clip),
) -> Vec<Clip> {
    items
        .iter()
        .map(|item| {
            let (start, end) = range(item);
            let identifier = id(item);
            let mut clip = Clip::new(
                identifier.clone(),
                selected == Some(identifier.as_str()),
                start,
                end - start,
            );
            decorate(item, &mut clip);
            clip
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::windows::video_editor::model::Segment;

    fn segment(id: &str, start: f64, end: f64, speed: Option<f64>) -> Segment {
        Segment {
            id: id.to_string(),
            original_start: start,
            original_end: end,
            trim_min_start: start,
            trim_max_end: end,
            speed,
        }
    }

    #[test]
    fn lays_video_clips_end_to_end_on_the_timeline() {
        let clips = video_clips(
            &[
                segment("a", 0.0, 4.0, None),
                segment("b", 4.0, 12.0, Some(2.0)),
            ],
            Some("b"),
        );
        assert_eq!(clips.len(), 2);
        assert_eq!(clips[0].start, 0.0);
        assert_eq!(clips[0].duration, 4.0);
        assert!(!clips[0].selected);
        assert_eq!(clips[0].badge, None);
        assert_eq!(clips[1].start, 4.0);
        assert_eq!(clips[1].duration, 4.0);
        assert!(clips[1].selected);
        assert_eq!(clips[1].badge.as_deref(), Some("2x"));
    }

    #[test]
    fn maps_drawing_kinds_to_their_renderer_gradients() {
        assert_eq!(drawing_gradient("highlight"), ("#facc15", "#a16207"));
        assert_eq!(drawing_gradient("unknown"), ("#2dd4bf", "#0f766e"));
        assert_eq!(drawing_icon("arrow"), "arrow-up-right");
        assert_eq!(drawing_icon("unknown"), "pen-line");
        assert_eq!(drawing_label("redact"), "Redact");
        assert_eq!(drawing_label("unknown"), "Pen");
    }

    #[test]
    fn camera_clips_are_pink_and_music_clips_are_purple() {
        assert_eq!(TrackKind::Camera.gradient(), ("#f472b6", "#be185d"));
        assert_eq!(TrackKind::Music.gradient(), ("#c084fc", "#7e22ce"));
    }

    #[test]
    fn a_music_group_takes_its_gutter_icon_from_its_source() {
        assert_eq!(source_icon("system"), "volume-2");
        assert_eq!(source_icon("mic"), "mic");
        assert_eq!(source_icon("music"), "music");
    }

    #[test]
    fn adjacent_clips_open_a_two_pixel_seam() {
        assert_eq!(clip_geometry(0.0, 2.0, None, 100.0), (0.0, 200.0));
        assert_eq!(clip_geometry(2.0, 2.0, Some(2.0), 100.0), (202.0, 198.0));
        assert_eq!(clip_geometry(2.0, 0.01, Some(2.0), 100.0), (200.0, 2.0));
        assert_eq!(clip_geometry(4.0, 2.0, Some(2.0), 100.0), (400.0, 200.0));
    }

    #[test]
    fn the_grab_zone_shrinks_with_the_clip_but_never_past_the_reference_width() {
        assert_eq!(handle_seconds(10.0), 0.3);
        assert_eq!(handle_seconds(1.0), 0.15);
        assert_eq!(handle_width(10.0, 100.0), RESIZE_HANDLE);
        assert_eq!(handle_width(0.5, 100.0), 7.5);
        assert_eq!(handle_width(0.0, 100.0), 1.0);
    }
}
