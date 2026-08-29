use gpui::{
    div, linear_color_stop, linear_gradient, prelude::*, px, AnyElement, Context, Hsla,
    MouseDownEvent, MouseMoveEvent, ScrollHandle, SharedString, Styled,
};

use crate::theme::color::Srgba;
use crate::theme::vars::ThemeVars;
use crate::ui::icon::icon_element;
use crate::ui::menu::{MenuBuilder, MenuEntry, MenuHandle, MenuItem};
use crate::windows::video_editor::model::format_duration;
use crate::windows::video_editor::timeline::{time_at_position, TRACK_GUTTER_WIDTH, TRACK_HEIGHT};
use crate::windows::video_editor::{ClipDrag, DragMode, VideoEditorWindow};

/// `ZOOM_LEVELS` in `types/zoom.ts`.
const ZOOM_LEVELS: [(f64, &str); 8] = [
    (1.25, "1.25x"),
    (1.5, "1.5x"),
    (1.75, "1.75x"),
    (2.0, "2x"),
    (2.25, "2.25x"),
    (2.5, "2.5x"),
    (2.75, "2.75x"),
    (3.0, "3x"),
];

/// How wide the grab zone at each end of a clip is. Narrower and a resize is
/// hard to hit; wider and a short clip has no body left to move.
const RESIZE_HANDLE: f32 = 6.0;

/// The length a clip added by clicking an empty lane gets.
const ADDED_CLIP_DURATION: f64 = 1.5;

/// Zoom, camera and drawing clips are created by clicking their empty lane;
/// video and music clips come from the recording and its imported files.
fn can_add_to(kind: TrackKind) -> bool {
    matches!(
        kind,
        TrackKind::Zoom | TrackKind::Camera | TrackKind::Drawing
    )
}

const PLAYHEAD_COLOR: &str = "#ef4444";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TrackKind {
    Video,
    Zoom,
    Camera,
    Drawing,
    Music,
}

impl TrackKind {
    /// The stable slug the track's generated clip ids are prefixed with.
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
            // `<Film>` in `timeline-track.tsx`, not a video camera.
            Self::Video => "film",
            Self::Zoom => "zoom-in",
            Self::Camera => "camera",
            Self::Drawing => "pen-line",
            Self::Music => "volume-2",
        }
    }

    fn gradient(self) -> (&'static str, &'static str) {
        match self {
            Self::Video => ("#d97706", "#b45309"),
            Self::Zoom => ("#818cf8", "#4f46e5"),
            Self::Camera => ("#c084fc", "#7e22ce"),
            Self::Drawing => ("#2dd4bf", "#0f766e"),
            Self::Music => ("#f472b6", "#be185d"),
        }
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

pub struct Clip {
    pub id: SharedString,
    pub label: SharedString,
    pub start: f64,
    pub duration: f64,
    pub selected: bool,
    pub gradient: (&'static str, &'static str),
    /// Set on zoom clips so their context menu can mark the current level.
    pub zoom_level: Option<f64>,
}

pub struct Track {
    pub kind: TrackKind,
    pub clips: Vec<Clip>,
}

fn gradient_fill(from: &str, to: &str) -> gpui::Background {
    linear_gradient(
        180.0,
        linear_color_stop(Srgba::parse(from).to_hsla(), 0.0),
        linear_color_stop(Srgba::parse(to).to_hsla(), 1.0),
    )
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
        .child(
            div()
                .absolute()
                .top(px(-6.0))
                .left(px(-5.0))
                .size(px(12.0))
                .rounded_full()
                .bg(color),
        )
        .into_any_element()
}

struct ClipContext<'a> {
    kind: TrackKind,
    pixels_per_second: f32,
    total_duration: f64,
    is_cut_tool_active: bool,
    siblings: usize,
    menu: &'a MenuHandle,
    scroll: &'a ScrollHandle,
}

#[allow(clippy::too_many_arguments)]
fn clip_element(
    clip: &Clip,
    context: &ClipContext<'_>,
    theme: &ThemeVars,
    cx: &mut Context<VideoEditorWindow>,
) -> AnyElement {
    let kind = context.kind;
    let pixels_per_second = context.pixels_per_second;
    let total_duration = context.total_duration;
    let is_cut_tool_active = context.is_cut_tool_active;
    let background = if clip.selected {
        linear_gradient(
            180.0,
            linear_color_stop(theme.primary, 0.0),
            linear_color_stop(theme.accent_hover, 1.0),
        )
    } else {
        gradient_fill(clip.gradient.0, clip.gradient.1)
    };
    let id = clip.id.clone();
    div()
        .id(SharedString::from(format!("clip-{}", clip.id)))
        .absolute()
        .top(px(2.0))
        .left(px(clip.start as f32 * pixels_per_second))
        .w(px((clip.duration as f32 * pixels_per_second).max(2.0)))
        .h(px(TRACK_HEIGHT - 4.0))
        .rounded(px(4.0))
        .overflow_hidden()
        .bg(background)
        .when(clip.selected, |el| {
            el.border_1().border_color(theme.foreground)
        })
        .flex()
        .items_center()
        // `renderLabel` sits in an `absolute inset-0 flex items-center
        // justify-center` box as `text-sm font-medium text-white
        // drop-shadow-md`, and collapses to a `Film` glyph once the clip is
        // narrower than 100px. This shell had a left-aligned 10px label with no
        // weight and no narrow fallback.
        .justify_center()
        .text_size(px(CLIP_LABEL_TEXT))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(crate::ui::colors::white(1.0))
        .child(if clip_is_narrow(clip.duration, pixels_per_second) {
            icon_element("film", px(14.0))
        } else {
            div()
                .truncate()
                .px(px(6.0))
                .child(clip.label.clone())
                .into_any_element()
        })
        .on_mouse_down(
            gpui::MouseButton::Left,
            cx.listener({
                let id = id.clone();
                let scroll = context.scroll.clone();
                let start = clip.start;
                let duration = clip.duration;
                move |this, event: &MouseDownEvent, _window, cx| {
                    cx.stop_propagation();
                    this.select_clip(id.clone(), cx);
                    if is_cut_tool_active {
                        let time = time_at_position(
                            event.position.x,
                            &scroll,
                            pixels_per_second,
                            total_duration,
                        );
                        let only = event.modifiers.shift.then_some(kind);
                        this.cut_at(time, only, cx);
                        return;
                    }

                    // Where the pointer grabbed the clip decides whether this is
                    // a resize at one end or a move of the whole clip.
                    let time = time_at_position(
                        event.position.x,
                        &scroll,
                        pixels_per_second,
                        total_duration,
                    );
                    let grab_offset = (time - start).clamp(0.0, duration);
                    let grab_pixels = grab_offset as f32 * pixels_per_second;
                    let width = (duration as f32 * pixels_per_second).max(2.0);
                    let mode = if grab_pixels <= RESIZE_HANDLE {
                        DragMode::ResizeStart
                    } else if grab_pixels >= width - RESIZE_HANDLE {
                        DragMode::ResizeEnd
                    } else {
                        DragMode::Move
                    };
                    this.begin_clip_drag(
                        ClipDrag {
                            kind,
                            id: id.clone(),
                            mode,
                            grab_offset: if mode == DragMode::Move {
                                grab_offset
                            } else {
                                0.0
                            },
                        },
                        cx,
                    );
                }
            }),
        )
        .on_mouse_down(
            gpui::MouseButton::Right,
            cx.listener({
                let menu = context.menu.clone();
                let id = id.clone();
                let zoom_level = clip.zoom_level;
                let siblings = context.siblings;
                move |this, event: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    this.select_clip(id.clone(), cx);
                    let entries =
                        clip_menu(kind, &id, zoom_level, siblings, cx.entity().downgrade());
                    menu.open_at(event.position, entries, window, cx);
                }
            }),
        )
        .into_any_element()
}

/// The context menu a clip opens, mirroring the per-track menus in
/// `timeline/zoom-track.tsx` and its siblings.
fn clip_menu(
    kind: TrackKind,
    id: &SharedString,
    zoom_level: Option<f64>,
    siblings: usize,
    editor: gpui::WeakEntity<VideoEditorWindow>,
) -> Vec<MenuEntry> {
    let mut builder = MenuBuilder::new();

    if let Some(current) = zoom_level.filter(|_| kind == TrackKind::Zoom) {
        let mut levels = MenuBuilder::new();
        for (value, label) in ZOOM_LEVELS {
            let editor = editor.clone();
            let id = id.clone();
            levels = levels.item(
                MenuItem::new(label)
                    .radio((current - value).abs() < f64::EPSILON)
                    .on_select(move |_window, cx| {
                        if let Some(editor) = editor.upgrade() {
                            editor.update(cx, |this, cx| {
                                this.set_zoom_segment_level(id.clone(), value, cx)
                            });
                        }
                    }),
            );
        }
        builder = builder
            .item(
                MenuItem::new("Zoom Level")
                    .icon("zoom-in")
                    .submenu(levels.build()),
            )
            .separator()
            .item({
                let editor = editor.clone();
                let id = id.clone();
                MenuItem::new("Apply zoom to All")
                    .icon("copy")
                    .disabled(siblings <= 1)
                    .on_select(move |_window, cx| {
                        if let Some(editor) = editor.upgrade() {
                            editor.update(cx, |this, cx| {
                                this.apply_zoom_level_to_all(id.clone(), cx)
                            });
                        }
                    })
            })
            .separator();
    }

    builder = builder
        .item({
            let editor = editor.clone();
            MenuItem::new("Cut at Playhead")
                .icon("scissors")
                .on_select(move |_window, cx| {
                    if let Some(editor) = editor.upgrade() {
                        editor.update(cx, |this, cx| {
                            let time = this.playhead();
                            this.cut_at(time, Some(kind), cx);
                        });
                    }
                })
        })
        .item({
            let editor = editor.clone();
            MenuItem::new("Cut All Tracks")
                .icon("scissors")
                .on_select(move |_window, cx| {
                    if let Some(editor) = editor.upgrade() {
                        editor.update(cx, |this, cx| {
                            let time = this.playhead();
                            this.cut_at(time, None, cx);
                        });
                    }
                })
        })
        .separator()
        .item({
            let editor = editor.clone();
            let id = id.clone();
            MenuItem::new("Delete")
                .icon("trash-2")
                .danger()
                .on_select(move |_window, cx| {
                    if let Some(editor) = editor.upgrade() {
                        editor.update(cx, |this, cx| this.delete_clip(kind, id.clone(), cx));
                    }
                })
        })
        .item({
            let id = id.clone();
            MenuItem::new("Delete Others")
                .icon("trash-2")
                .disabled(siblings <= 1)
                .on_select(move |_window, cx| {
                    if let Some(editor) = editor.upgrade() {
                        editor.update(cx, |this, cx| this.delete_other_clips(kind, id.clone(), cx));
                    }
                })
        });

    builder.build()
}

#[allow(clippy::too_many_arguments)]
/// `text-sm`.
const CLIP_LABEL_TEXT: f32 = 14.0;
/// `widthPixels < 100` in `renderLabel`.
const CLIP_LABEL_MIN_WIDTH: f32 = 100.0;

/// Whether a clip is too narrow for its duration label, in which case the
/// reference shows a film glyph instead.
pub fn clip_is_narrow(duration: f64, pixels_per_second: f32) -> bool {
    (duration as f32 * pixels_per_second) < CLIP_LABEL_MIN_WIDTH
}

pub fn render(
    tracks: &[Track],
    total_duration: f64,
    pixels_per_second: f32,
    playhead_seconds: f64,
    is_cut_tool_active: bool,
    menu: &MenuHandle,
    scroll: &ScrollHandle,
    theme: &ThemeVars,
    cx: &mut Context<VideoEditorWindow>,
) -> AnyElement {
    let total_width = (total_duration as f32 * pixels_per_second).max(1.0);

    let mut gutter = div()
        .flex()
        .flex_col()
        .w(px(TRACK_GUTTER_WIDTH))
        .flex_shrink_0()
        .border_r_1()
        .border_color(theme.border);
    let mut lanes = div().relative().flex().flex_col().w(px(total_width));

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
                .child(icon_element(track.kind.icon(), px(12.0))),
        );

        let kind = track.kind;
        let mut lane = div()
            .id(SharedString::from(format!("timeline-lane-{}", kind.id())))
            .relative()
            .h(px(TRACK_HEIGHT))
            .flex_shrink_0()
            .w_full()
            .border_b_1()
            .border_color(theme.border)
            .when(is_cut_tool_active || can_add_to(kind), |el| {
                el.cursor_pointer()
            })
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener({
                    let scroll = scroll.clone();
                    move |this, event: &MouseDownEvent, _window, cx| {
                        let time = time_at_position(
                            event.position.x,
                            &scroll,
                            pixels_per_second,
                            total_duration,
                        );
                        if is_cut_tool_active {
                            // Shift cuts only this track, matching the
                            // renderer's `onTrackClick`.
                            let only = event.modifiers.shift.then_some(kind);
                            this.cut_at(time, only, cx);
                            return;
                        }
                        if can_add_to(kind) {
                            this.add_clip(kind, time, time + ADDED_CLIP_DURATION, cx);
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
            siblings: track.clips.len(),
            menu,
            scroll,
        };
        for clip in &track.clips {
            lane = lane.child(clip_element(clip, &context, theme, cx));
        }
        lanes = lanes.child(lane);
    }

    lanes = lanes.child(playhead(playhead_seconds as f32 * pixels_per_second));

    div()
        .flex()
        .flex_row()
        .flex_1()
        .min_h_0()
        .child(gutter)
        .child(
            div()
                .id("timeline-tracks-scroll")
                .track_scroll(scroll)
                .relative()
                .flex_1()
                .overflow_x_scroll()
                .on_mouse_down(
                    gpui::MouseButton::Left,
                    cx.listener({
                        let scroll = scroll.clone();
                        move |this, event: &MouseDownEvent, _window, cx| {
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
                        if this.is_dragging_clip() {
                            this.update_clip_drag(time, cx);
                            return;
                        }
                        this.set_playhead(time, cx);
                    }
                }))
                .child(lanes),
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
        clips.push(Clip {
            id: SharedString::from(segment.id.clone()),
            label: SharedString::from(format_duration(duration)),
            start,
            duration,
            selected: selected == Some(segment.id.as_str()),
            gradient: TrackKind::Video.gradient(),
            zoom_level: None,
        });
        start += duration;
    }
    clips
}

#[allow(clippy::too_many_arguments)]
pub fn range_clips<T>(
    items: &[T],
    selected: Option<&str>,
    id: impl Fn(&T) -> String,
    range: impl Fn(&T) -> (f64, f64),
    label: impl Fn(&T) -> String,
    gradient: impl Fn(&T) -> (&'static str, &'static str),
    zoom_level: impl Fn(&T) -> Option<f64>,
) -> Vec<Clip> {
    items
        .iter()
        .map(|item| {
            let (start, end) = range(item);
            let identifier = id(item);
            Clip {
                selected: selected == Some(identifier.as_str()),
                id: SharedString::from(identifier),
                label: SharedString::from(label(item)),
                start,
                duration: (end - start).max(0.0),
                gradient: gradient(item),
                zoom_level: zoom_level(item),
            }
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
        assert_eq!(clips[1].start, 4.0);
        assert_eq!(clips[1].duration, 4.0);
        assert!(clips[1].selected);
    }

    #[test]
    fn maps_drawing_kinds_to_their_renderer_gradients() {
        assert_eq!(drawing_gradient("highlight"), ("#facc15", "#a16207"));
        assert_eq!(drawing_gradient("unknown"), ("#2dd4bf", "#0f766e"));
    }
}

#[cfg(test)]
mod label_tests {
    /// `widthPixels < 100` -- the threshold at which `renderLabel` gives up on
    /// the duration text and shows a film glyph instead.
    #[test]
    fn a_clip_narrower_than_a_hundred_pixels_shows_a_glyph_instead() {
        // 3s at 30px/s is 90px: too narrow.
        assert!(super::clip_is_narrow(3.0, 30.0));
        // 3s at 40px/s is 120px: room for the label.
        assert!(!super::clip_is_narrow(3.0, 40.0));
        // Exactly 100px is not narrower than 100.
        assert!(!super::clip_is_narrow(2.0, 50.0));
        assert!(super::clip_is_narrow(0.0, 100.0));
    }

    /// `text-sm`, which is 14px -- not the 10px this shell used to draw.
    #[test]
    fn the_label_is_the_reference_text_size() {
        assert_eq!(super::CLIP_LABEL_TEXT, 14.0);
        assert_eq!(super::CLIP_LABEL_MIN_WIDTH, 100.0);
    }
}
