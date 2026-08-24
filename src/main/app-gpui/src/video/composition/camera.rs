//! Port of `composition/camera-canvas-renderer.ts` — the camera bubble, its
//! zoom-aware shrink and the cursor-dodge fade.

use tiny_skia::{Color, FillRule, PixmapRef, Rect};

use crate::render::canvas::{rounded_rect_path, Canvas};
use crate::video::composition::segments::{map_timeline_to_video_time, VideoSegment};
use crate::video::composition::wallpaper::shadow_config;
use crate::video::composition::zoom::Position;
use crate::video::sidecars::{CursorData, CursorEvent};
use crate::windows::video_editor::model::CameraSegment;
use crate::windows::video_editor::styles::CameraStyle;

const BOUNDS_PADDING: f64 = 0.05;
const LOOK_AHEAD_MS: f64 = 150.0;
const FADE_IN_MS: f64 = 200.0;

const CAMERA_ZOOM_SHRINK_FACTOR: f64 = 0.35;
const MIN_CAMERA_SCALE: f64 = 0.5;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ZoomInfo {
    pub scale: f64,
    pub viewport: Option<Position>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Layout {
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
    pub border_radius: f64,
}

/// `CAMERA_OVERLAY_SIZE_PERCENT`.
fn size_percent(size: &str) -> f64 {
    match size {
        "small" => 15.0,
        "large" => 25.0,
        _ => 20.0,
    }
}

/// `CAMERA_OVERLAY_ASPECT_RATIO`.
fn shape_aspect_ratio(shape: &str) -> f64 {
    match shape {
        "rectangle" => 0.75,
        "vertical" => 4.0 / 3.0,
        _ => 1.0,
    }
}

/// `getCameraOverlayDimensions`.
pub fn overlay_dimensions(
    video_width: f64,
    video_height: f64,
    size: &str,
    shape: &str,
) -> (f64, f64) {
    let reference = video_width.max(video_height);
    let width = (reference * size_percent(size) / 100.0).round();
    (width, (width * shape_aspect_ratio(shape)).round())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AnchorX {
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AnchorY {
    Top,
    Center,
    Bottom,
}

/// `getCameraPositionCoords`, reduced to the anchors the layout uses.
fn anchors(position: &str) -> (AnchorX, AnchorY) {
    let (vertical, horizontal) = position.split_once('-').unwrap_or(("bottom", "right"));
    let anchor_x = match horizontal {
        "left" => AnchorX::Left,
        "center" => AnchorX::Center,
        _ => AnchorX::Right,
    };
    let anchor_y = match vertical {
        "top" => AnchorY::Top,
        "middle" => AnchorY::Center,
        _ => AnchorY::Bottom,
    };
    (anchor_x, anchor_y)
}

/// `getCameraZoomScale` — the bubble shrinks while the frame is zoomed in so it
/// keeps roughly the same on-screen size.
fn zoom_scale(zoom: Option<ZoomInfo>) -> f64 {
    let Some(zoom) = zoom.filter(|zoom| zoom.scale > 1.0) else {
        return 1.0;
    };
    (1.0 - (zoom.scale - 1.0) * CAMERA_ZOOM_SHRINK_FACTOR).max(MIN_CAMERA_SCALE)
}

/// `calculateCameraLayout`.
pub fn calculate_layout(
    style: &CameraStyle,
    video_width: f64,
    video_height: f64,
    zoom: Option<ZoomInfo>,
) -> Layout {
    let (base_width, base_height) =
        overlay_dimensions(video_width, video_height, &style.size, &style.shape);
    let scale = zoom_scale(zoom);
    let width = base_width * scale;
    let height = base_height * scale;

    let (anchor_x, anchor_y) = anchors(&style.position);
    let padding = (style.padding / 100.0) * video_width.min(video_height);

    let left = match anchor_x {
        AnchorX::Left => padding,
        AnchorX::Center => video_width / 2.0 - width / 2.0,
        AnchorX::Right => video_width - width - padding,
    };
    let top = match anchor_y {
        AnchorY::Top => padding,
        AnchorY::Center => video_height / 2.0 - height / 2.0,
        AnchorY::Bottom => video_height - height - padding,
    };

    let max_border_radius = width.min(height) / 2.0;
    Layout {
        left,
        top,
        width,
        height,
        border_radius: (style.border_radius / 100.0) * max_border_radius,
    }
}

fn binary_search_events(events: &[CursorEvent], timestamp: f64) -> Option<usize> {
    let mut low = 0i64;
    let mut high = events.len() as i64 - 1;
    let mut result = None;
    while low <= high {
        let middle = ((low + high) / 2) as usize;
        if events[middle].timestamp <= timestamp {
            result = Some(middle);
            low = middle as i64 + 1;
        } else {
            high = middle as i64 - 1;
        }
    }
    result
}

/// `getCursorPositionAtTime` — the camera dodge samples without smoothing.
fn cursor_position_at(events: &[CursorEvent], timestamp: f64) -> Option<Position> {
    if events.is_empty() {
        return None;
    }
    let Some(index) = binary_search_events(events, timestamp) else {
        return Some(Position {
            x: events[0].x,
            y: events[0].y,
        });
    };
    if index == events.len() - 1 {
        return Some(Position {
            x: events[index].x,
            y: events[index].y,
        });
    }
    let before = &events[index];
    let after = &events[index + 1];
    if after.timestamp == before.timestamp {
        return Some(Position {
            x: before.x,
            y: before.y,
        });
    }
    let t = (timestamp - before.timestamp) / (after.timestamp - before.timestamp);
    Some(Position {
        x: before.x + (after.x - before.x) * t,
        y: before.y + (after.y - before.y) * t,
    })
}

/// `transformCursorToScreenSpace` — while zoomed, the pointer's on-screen
/// position is not its position in the source frame.
fn to_screen_space(cursor: Position, zoom: Option<ZoomInfo>) -> Position {
    let Some(zoom) = zoom.filter(|zoom| zoom.scale != 1.0) else {
        return cursor;
    };
    let Some(viewport) = zoom.viewport else {
        return cursor;
    };
    let viewport_size = 1.0 / zoom.scale;
    Position {
        x: (cursor.x - viewport.x) / viewport_size,
        y: (cursor.y - viewport.y) / viewport_size,
    }
}

fn is_cursor_in_camera_bounds(
    cursor: Position,
    layout: Layout,
    video_width: f64,
    video_height: f64,
) -> bool {
    let left = layout.left / video_width - BOUNDS_PADDING;
    let top = layout.top / video_height - BOUNDS_PADDING;
    let right = (layout.left + layout.width) / video_width + BOUNDS_PADDING;
    let bottom = (layout.top + layout.height) / video_height + BOUNDS_PADDING;
    cursor.x >= left && cursor.x <= right && cursor.y >= top && cursor.y <= bottom
}

fn ease_out_cubic(t: f64) -> f64 {
    1.0 - (1.0 - t).powi(3)
}

fn ease_in_cubic(t: f64) -> f64 {
    t * t * t
}

fn time_to_entry(
    events: &[CursorEvent],
    video_time: f64,
    layout: Layout,
    video_width: f64,
    video_height: f64,
    zoom: Option<ZoomInfo>,
) -> Option<f64> {
    const STEP_MS: f64 = 5.0;
    let mut offset = 0.0;
    while offset <= LOOK_AHEAD_MS {
        if let Some(position) = cursor_position_at(events, video_time + offset / 1000.0) {
            let screen = to_screen_space(position, zoom);
            if is_cursor_in_camera_bounds(screen, layout, video_width, video_height) {
                return Some(offset);
            }
        }
        offset += STEP_MS;
    }
    None
}

fn last_exit_time(
    events: &[CursorEvent],
    video_time: f64,
    layout: Layout,
    video_width: f64,
    video_height: f64,
    zoom: Option<ZoomInfo>,
) -> Option<f64> {
    const STEP_MS: f64 = 10.0;
    let start = (video_time - FADE_IN_MS / 1000.0).max(0.0);
    let mut last_exit = None;
    let mut was_in_bounds = false;
    let mut time = start;
    while time <= video_time {
        if let Some(position) = cursor_position_at(events, time) {
            let screen = to_screen_space(position, zoom);
            let in_bounds = is_cursor_in_camera_bounds(screen, layout, video_width, video_height);
            if !in_bounds && was_in_bounds {
                last_exit = Some(time);
            }
            was_in_bounds = in_bounds;
        }
        time += STEP_MS / 1000.0;
    }
    last_exit
}

/// `calculateCameraOpacity` — the bubble gets out of the pointer's way.
pub fn opacity(
    cursor_data: Option<&CursorData>,
    segments: &[VideoSegment],
    timeline_time: f64,
    layout: Layout,
    video_width: f64,
    video_height: f64,
    zoom: Option<ZoomInfo>,
) -> f64 {
    let Some(cursor_data) = cursor_data.filter(|data| !data.is_empty()) else {
        return 1.0;
    };
    let Some(video_time) = map_timeline_to_video_time(timeline_time, segments) else {
        return 1.0;
    };
    let Some(cursor) = cursor_position_at(&cursor_data.events, video_time) else {
        return 1.0;
    };
    let screen = to_screen_space(cursor, zoom);

    if is_cursor_in_camera_bounds(screen, layout, video_width, video_height) {
        return 0.0;
    }

    if let Some(entry) = time_to_entry(
        &cursor_data.events,
        video_time,
        layout,
        video_width,
        video_height,
        zoom,
    ) {
        let progress = 1.0 - entry / LOOK_AHEAD_MS;
        return 1.0 - ease_in_cubic((progress * 2.0).min(1.0));
    }

    if let Some(exit) = last_exit_time(
        &cursor_data.events,
        video_time,
        layout,
        video_width,
        video_height,
        zoom,
    ) {
        let since_exit = (video_time - exit) * 1000.0;
        if since_exit < FADE_IN_MS {
            return ease_out_cubic(since_exit / FADE_IN_MS);
        }
    }

    1.0
}

pub struct RenderConfig<'a> {
    pub camera_style: &'a CameraStyle,
    pub camera_visible_ranges: Option<&'a [CameraSegment]>,
    pub cursor_data: Option<&'a CursorData>,
    pub segments: &'a [VideoSegment],
    pub video_width: f64,
    pub video_height: f64,
    pub offset_x: f64,
    pub offset_y: f64,
    pub zoom: Option<ZoomInfo>,
}

/// Port of `renderCamera`.
pub fn render(
    canvas: &mut Canvas,
    timeline_time: f64,
    source: PixmapRef<'_>,
    config: &RenderConfig<'_>,
) {
    if !config.camera_style.visible {
        return;
    }
    if config.camera_visible_ranges.is_some()
        && !crate::video::sidecars::is_camera_visible_at(
            config.camera_visible_ranges,
            timeline_time,
        )
    {
        return;
    }
    if source.width() == 0 || source.height() == 0 {
        return;
    }

    let layout = calculate_layout(
        config.camera_style,
        config.video_width,
        config.video_height,
        config.zoom,
    );
    let alpha = opacity(
        config.cursor_data,
        config.segments,
        timeline_time,
        layout,
        config.video_width,
        config.video_height,
        config.zoom,
    );
    if alpha <= 0.0 {
        return;
    }

    let left = config.offset_x + layout.left;
    let top = config.offset_y + layout.top;

    canvas.save();
    canvas.set_global_alpha(alpha as f32);

    render_shadow(canvas, left, top, layout, config.camera_style.shadow);

    let clip = if layout.border_radius > 0.0 {
        rounded_rect_path(
            left as f32,
            top as f32,
            layout.width as f32,
            layout.height as f32,
            layout.border_radius as f32,
        )
    } else {
        Rect::from_xywh(
            left as f32,
            top as f32,
            layout.width as f32,
            layout.height as f32,
        )
        .and_then(|rect| {
            let mut builder = tiny_skia::PathBuilder::new();
            builder.push_rect(rect);
            builder.finish()
        })
    };
    if let Some(clip) = clip {
        canvas.clip_path(&clip, FillRule::Winding);
    }

    if config.camera_style.mirrored {
        let center_x = left + layout.width / 2.0;
        let center_y = top + layout.height / 2.0;
        canvas.translate(center_x as f32, center_y as f32);
        canvas.scale(-1.0, 1.0);
        canvas.translate(-center_x as f32, -center_y as f32);
    }

    // Cover: the bubble is filled and the overflowing axis is cropped.
    let source_aspect = source.width() as f64 / source.height() as f64;
    let target_aspect = layout.width / layout.height;
    let (draw_width, draw_height, draw_x, draw_y) = if source_aspect > target_aspect {
        let draw_width = layout.height * source_aspect;
        (
            draw_width,
            layout.height,
            left - (draw_width - layout.width) / 2.0,
            top,
        )
    } else {
        let draw_height = layout.width / source_aspect;
        (
            layout.width,
            draw_height,
            left,
            top - (draw_height - layout.height) / 2.0,
        )
    };
    canvas.draw_pixmap(
        source,
        draw_x as f32,
        draw_y as f32,
        draw_width as f32,
        draw_height as f32,
    );
    canvas.restore();
}

/// `renderShadow` — the bubble's own drop shadow, drawn behind it.
fn render_shadow(canvas: &mut Canvas, left: f64, top: f64, layout: Layout, shadow: f64) {
    let Some(shadow) = shadow_config(shadow) else {
        return;
    };
    let Some(path) = rounded_rect_path(
        left as f32,
        top as f32,
        layout.width as f32,
        layout.height as f32,
        layout.border_radius as f32,
    ) else {
        return;
    };
    canvas.save();
    canvas.set_shadow(Some(shadow));
    canvas.fill_path(&path, Color::from_rgba8(0, 0, 0, 255), FillRule::Winding);
    canvas.restore();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn style() -> CameraStyle {
        CameraStyle::default()
    }

    #[test]
    fn overlay_dimensions_follow_the_size_and_shape_tables() {
        assert_eq!(
            overlay_dimensions(1920.0, 1080.0, "medium", "square"),
            (384.0, 384.0)
        );
        assert_eq!(
            overlay_dimensions(1920.0, 1080.0, "small", "rectangle"),
            (288.0, 216.0)
        );
        assert_eq!(
            overlay_dimensions(1920.0, 1080.0, "large", "vertical"),
            (480.0, 640.0)
        );
    }

    #[test]
    fn the_bubble_is_anchored_by_its_position() {
        let mut settings = style();
        settings.padding = 0.0;
        settings.position = "bottom-right".into();
        let bottom_right = calculate_layout(&settings, 1000.0, 1000.0, None);
        assert_eq!(bottom_right.left, 800.0);
        assert_eq!(bottom_right.top, 800.0);

        settings.position = "top-left".into();
        let top_left = calculate_layout(&settings, 1000.0, 1000.0, None);
        assert_eq!((top_left.left, top_left.top), (0.0, 0.0));

        settings.position = "middle-center".into();
        let middle = calculate_layout(&settings, 1000.0, 1000.0, None);
        assert_eq!((middle.left, middle.top), (400.0, 400.0));
    }

    #[test]
    fn padding_is_a_percentage_of_the_shorter_side() {
        let mut settings = style();
        settings.padding = 10.0;
        settings.position = "top-left".into();
        let layout = calculate_layout(&settings, 2000.0, 1000.0, None);
        assert_eq!(layout.left, 100.0);
        assert_eq!(layout.top, 100.0);
    }

    #[test]
    fn zooming_in_shrinks_the_bubble_to_a_floor() {
        let settings = style();
        let plain = calculate_layout(&settings, 1000.0, 1000.0, None);
        let zoomed = calculate_layout(
            &settings,
            1000.0,
            1000.0,
            Some(ZoomInfo {
                scale: 2.0,
                viewport: None,
            }),
        );
        assert!(zoomed.width < plain.width);

        let extreme = calculate_layout(
            &settings,
            1000.0,
            1000.0,
            Some(ZoomInfo {
                scale: 10.0,
                viewport: None,
            }),
        );
        assert!((extreme.width - plain.width * MIN_CAMERA_SCALE).abs() < 1e-9);
    }

    #[test]
    fn the_border_radius_is_a_percentage_of_half_the_short_side() {
        let mut settings = style();
        settings.border_radius = 100.0;
        let layout = calculate_layout(&settings, 1000.0, 1000.0, None);
        assert_eq!(layout.border_radius, layout.height / 2.0);
    }

    #[test]
    fn the_bubble_hides_while_the_pointer_is_over_it() {
        let settings = style();
        let layout = calculate_layout(&settings, 1000.0, 1000.0, None);
        let cursor_data = CursorData {
            events: vec![crate::video::sidecars::CursorEvent {
                timestamp: 0.0,
                x: (layout.left + layout.width / 2.0) / 1000.0,
                y: (layout.top + layout.height / 2.0) / 1000.0,
                kind: "move".into(),
                ..Default::default()
            }],
            ..CursorData::default()
        };
        let segments = [VideoSegment {
            start_time: 0.0,
            end_time: 5.0,
            timeline_start: 0.0,
            speed: 1.0,
        }];
        let alpha = opacity(
            Some(&cursor_data),
            &segments,
            0.0,
            layout,
            1000.0,
            1000.0,
            None,
        );
        assert_eq!(alpha, 0.0);
    }

    #[test]
    fn without_cursor_data_the_bubble_stays_opaque() {
        let layout = calculate_layout(&style(), 1000.0, 1000.0, None);
        assert_eq!(opacity(None, &[], 0.0, layout, 1000.0, 1000.0, None), 1.0);
    }

    #[test]
    fn a_zoomed_viewport_moves_the_pointer_into_screen_space() {
        let cursor = Position { x: 0.5, y: 0.5 };
        let mapped = to_screen_space(
            cursor,
            Some(ZoomInfo {
                scale: 2.0,
                viewport: Some(Position { x: 0.25, y: 0.25 }),
            }),
        );
        assert_eq!(mapped, Position { x: 0.5, y: 0.5 });

        let offset = to_screen_space(
            Position { x: 0.3, y: 0.3 },
            Some(ZoomInfo {
                scale: 2.0,
                viewport: Some(Position { x: 0.25, y: 0.25 }),
            }),
        );
        assert!((offset.x - 0.1).abs() < 1e-9);
    }

    #[test]
    fn drawing_paints_the_bubble_into_the_frame() {
        let mut canvas = Canvas::new(400, 400).expect("canvas");
        let mut source = tiny_skia::Pixmap::new(64, 48).expect("source");
        source.fill(Color::from_rgba8(0, 200, 0, 255));
        let settings = style();
        render(
            &mut canvas,
            0.0,
            source.as_ref(),
            &RenderConfig {
                camera_style: &settings,
                camera_visible_ranges: None,
                cursor_data: None,
                segments: &[],
                video_width: 400.0,
                video_height: 400.0,
                offset_x: 0.0,
                offset_y: 0.0,
                zoom: None,
            },
        );
        let layout = calculate_layout(&settings, 400.0, 400.0, None);
        let x = (layout.left + layout.width / 2.0) as u32;
        let y = (layout.top + layout.height / 2.0) as u32;
        let index = ((y * 400 + x) * 4) as usize;
        assert!(canvas.pixmap().data()[index + 1] > 100);
    }
}
