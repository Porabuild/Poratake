//! Editor canvas — displays the screenshot and hosts annotation drawing.
//! Port of `editor/editor-canvas.tsx` (image + overlay) with the pointer
//! state machine from the editor window entity.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    canvas, div, img, prelude::*, px, App, PathBuilder, Pixels, RenderOnce, ScrollHandle, Styled,
};

use crate::editor::annotations::{
    arrow_head_size, normalize_rect, points_to_coordinates, Annotation, Point, DEFAULT_TEXT_FONT,
};
use crate::theme::color::Srgba;
use crate::theme::vars::active_theme;
use crate::ui::colors::Tool;

/// Everything the canvas needs to draw one frame; owned by the window entity
/// and read at render time through the shared cell.
pub type SnapshotCell = Rc<RefCell<CanvasSnapshot>>;

pub struct CanvasSnapshot {
    pub image: Option<std::sync::Arc<gpui::RenderImage>>,
    /// Redacted pixels for committed redactions, rendered from the same code
    /// the export uses so the preview shows exactly what is written out.
    pub redact_patches: std::collections::HashMap<String, std::sync::Arc<gpui::RenderImage>>,
    pub image_width: f32,
    pub image_height: f32,
    pub zoom: f32,
    pub annotations: Vec<Annotation>,
    /// Annotation currently being drawn, in image coordinates.
    pub draft: Option<Annotation>,
    #[allow(dead_code)]
    pub tool: Tool,
    #[allow(dead_code)]
    pub color_hex: String,
    #[allow(dead_code)]
    pub stroke_width: f64,
    /// The pending crop rectangle in image coordinates.
    pub crop: Option<(f64, f64, f64, f64)>,
    pub wallpaper: crate::editor::wallpaper::WallpaperSettings,
    /// The wallpaper backdrop, rendered by the same rasterizer the export
    /// uses. A gradient could be drawn with GPUI, but a background image with
    /// blur and grain could not, and preview and export have to agree.
    pub backdrop: Option<std::sync::Arc<gpui::RenderImage>>,
    /// The annotation the select tool has picked, outlined on the canvas.
    pub selected: Option<String>,
    /// `(left, top, right, bottom)` the balance option trims from the image.
    /// The frame is shifted and clipped by it so the preview matches the file.
    pub balance_crop: Option<(f32, f32, f32, f32)>,
    /// Images attached to the capture's edges. They are painted into the
    /// backdrop; the snapshot carries them so the layout can place the capture
    /// inside the group they form.
    pub layers: Vec<crate::editor::layers::ImageLayer>,
    pub spacing: f64,
}

#[derive(IntoElement)]
pub struct EditorCanvas {
    pub snapshot: SnapshotCell,
    pub bounds_cell: Rc<RefCell<Option<gpui::Bounds<Pixels>>>>,
    /// The stage scrolls when the zoomed image is larger than the window, the
    /// way `overflow-auto` does in the renderer.
    pub scroll: ScrollHandle,
}

impl EditorCanvas {
    pub fn new(
        snapshot: SnapshotCell,
        bounds_cell: Rc<RefCell<Option<gpui::Bounds<Pixels>>>>,
        scroll: ScrollHandle,
    ) -> Self {
        Self {
            snapshot,
            bounds_cell,
            scroll,
        }
    }
}

impl RenderOnce for EditorCanvas {
    fn render(self, _window: &mut gpui::Window, cx: &mut App) -> impl IntoElement {
        let theme = active_theme(cx);
        let snap = self.snapshot.borrow();

        let content_width = px(snap.image_width * snap.zoom);
        let content_height = px(snap.image_height * snap.zoom);

        let wallpaper = snap.wallpaper.clone();
        let mut frame = div()
            .relative()
            .w(content_width)
            .h(content_height)
            .when(!wallpaper.is_active(), |el| el.shadow_md())
            .when(wallpaper.corners > 0.0, |el| {
                el.rounded(px(wallpaper.corners as f32 * snap.zoom))
                    .overflow_hidden()
            })
            .when(wallpaper.shadow > 0.0, |el| el.shadow_2xl())
            .bg(theme.surface);

        if let Some(image) = snap.image.clone() {
            frame = frame.child(img(image).w(content_width).h(content_height));
        }

        drop(snap);

        for annotation in self
            .snapshot
            .borrow()
            .annotations
            .iter()
            .chain(self.snapshot.borrow().draft.iter())
        {
            if let Annotation::Text {
                id,
                x,
                y,
                text,
                fill,
                font_size,
                font_family,
                background_color,
                ..
            } = annotation
            {
                frame = frame.child(text_overlay(
                    id,
                    *x,
                    *y,
                    text,
                    fill,
                    *font_size,
                    font_family.as_deref().unwrap_or(DEFAULT_TEXT_FONT),
                    background_color.as_deref(),
                    self.snapshot.borrow().zoom,
                ));
            }
        }

        if let Some(outline) = selection_outline(&self.snapshot.borrow(), &theme) {
            frame = frame.child(outline);
        }

        if let Some((x, y, width, height)) = self.snapshot.borrow().crop {
            let zoom = self.snapshot.borrow().zoom;
            frame = frame.child(crop_overlay(x, y, width, height, zoom, &theme));
        }

        // Annotation overlay paints every committed annotation plus the draft.
        let overlay_snapshot = self.snapshot.clone();
        let recorder = self.bounds_cell.clone();
        frame = frame.child(
            canvas(
                move |bounds, _window, _cx| {
                    *recorder.borrow_mut() = Some(bounds);
                },
                move |bounds, (), window, _cx| {
                    set_paint_origin(bounds.origin);
                    let snapshot = overlay_snapshot.borrow();
                    for annotation in snapshot.annotations.iter().chain(snapshot.draft.iter()) {
                        if let Annotation::Redact {
                            id,
                            x,
                            y,
                            width,
                            height,
                            ..
                        } = annotation
                        {
                            if let Some(patch) = snapshot.redact_patches.get(id) {
                                let scale = snapshot.zoom;
                                let region = gpui::Bounds {
                                    origin: gpui::point(
                                        bounds.origin.x + px(*x as f32 * scale),
                                        bounds.origin.y + px(*y as f32 * scale),
                                    ),
                                    size: gpui::size(
                                        px(*width as f32 * scale),
                                        px(*height as f32 * scale),
                                    ),
                                };
                                let _ = window.paint_image(
                                    region,
                                    gpui::Corners::default(),
                                    patch.clone(),
                                    0,
                                    false,
                                );
                                continue;
                            }
                        }
                        draw_annotation(window, annotation, snapshot.zoom);
                    }
                    set_paint_origin(gpui::point(px(0.0), px(0.0)));
                },
            )
            .absolute()
            .inset_0(),
        );

        let snap = self.snapshot.borrow();
        let (zoom, image_width, image_height) =
            (snap.zoom, snap.image_width as f64, snap.image_height as f64);
        drop(snap);

        let layers = self.snapshot.borrow().layers.clone();
        let spacing = self.snapshot.borrow().spacing.max(0.0);
        let group = crate::editor::layers::compute(image_width, image_height, &layers, spacing);
        let framed = wallpaper.is_active_with_layers(!layers.is_empty());

        let (stage_width, stage_height) = if framed {
            let ((canvas_width, canvas_height), _) =
                crate::editor::wallpaper::layout(&wallpaper, group.width, group.height);
            (
                px(canvas_width as f32 * zoom),
                px(canvas_height as f32 * zoom),
            )
        } else {
            (
                px(image_width as f32 * zoom),
                px(image_height as f32 * zoom),
            )
        };

        let stage_child: gpui::AnyElement = if framed {
            let ((canvas_width, canvas_height), (offset_x, offset_y, _, _)) =
                crate::editor::wallpaper::layout(&wallpaper, group.width, group.height);
            // The capture sits wherever the group's layout put it.
            let (offset_x, offset_y) = (offset_x + group.primary.x, offset_y + group.primary.y);
            let mut backdrop = div()
                .relative()
                .w(px(canvas_width as f32 * zoom))
                .h(px(canvas_height as f32 * zoom))
                .shadow_md()
                .overflow_hidden();
            let rendered = self.snapshot.borrow().backdrop.clone();
            backdrop = match rendered {
                Some(image) => backdrop.child(
                    img(image)
                        .absolute()
                        .inset_0()
                        .w(px(canvas_width as f32 * zoom))
                        .h(px(canvas_height as f32 * zoom)),
                ),
                None => backdrop.bg(theme.surface_secondary),
            };
            // A window frame's title bar sits between the padding and the
            // image, so the image starts below it.
            let title_bar =
                crate::render::window_frame::title_bar_height(&wallpaper.window_frame.style, 1.0);
            let crop = self.snapshot.borrow().balance_crop;
            let (crop_left, crop_top, crop_right, crop_bottom) =
                crop.unwrap_or((0.0, 0.0, 0.0, 0.0));
            backdrop
                .child(
                    div()
                        .absolute()
                        .left(px(offset_x as f32 * zoom))
                        .top(px((offset_y + title_bar) as f32 * zoom))
                        .w(px((image_width as f32 - crop_left - crop_right) * zoom))
                        .h(px((image_height as f32 - crop_top - crop_bottom) * zoom))
                        .overflow_hidden()
                        .child(
                            div()
                                .absolute()
                                .left(px(-crop_left * zoom))
                                .top(px(-crop_top * zoom))
                                .child(frame),
                        ),
                )
                .into_any_element()
        } else {
            frame.into_any_element()
        };

        // The scroll container is the window-sized viewport; the content box
        // inside it is at least as large as the viewport so a small image stays
        // centred, and grows with the zoom so a large one can be scrolled.
        div()
            .id("editor-stage")
            .track_scroll(&self.scroll)
            .size_full()
            .overflow_scroll()
            .bg(theme.background)
            .child(
                div()
                    .min_w_full()
                    .min_h_full()
                    .w(stage_width)
                    .h(stage_height)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(stage_child),
            )
    }
}

pub fn draw_annotation(window: &mut gpui::Window, annotation: &Annotation, scale: f32) {
    match annotation {
        Annotation::Highlight {
            points,
            fill,
            opacity,
            stroke_width,
            ..
        } => {
            let coordinates = points_to_coordinates(points);
            if coordinates.len() < 2 {
                return;
            }
            let mut builder = stroked(*stroke_width as f32 * scale);
            for (index, (x, y)) in coordinates.iter().enumerate() {
                let at = at_point(
                    &Point {
                        x: *x as f32,
                        y: *y as f32,
                    },
                    scale,
                );
                if index == 0 {
                    builder.move_to(at);
                } else {
                    builder.line_to(at);
                }
            }
            if let Ok(path) = builder.build() {
                window.paint_path(path, Srgba::parse(fill).to_hsla().opacity(*opacity as f32));
            }
        }
        Annotation::Number {
            x,
            y,
            display_value,
            fill,
            size,
            ..
        } => {
            let (radius, _) = crate::editor::annotations::number_size_config(size);
            let center = Point {
                x: *x as f32,
                y: *y as f32,
            };
            if let Some(path) =
                ellipse_path(center.x, center.y, radius as f32, radius as f32, scale)
            {
                window.paint_path(path, Srgba::parse(fill).to_hsla());
            }
            let (_, font_size) = crate::editor::annotations::number_size_config(size);
            let cell = font_size as f32 / crate::editor::glyphs::GLYPH_ROWS as f32;
            let text_color = contrast_color(fill);
            for (column, row) in crate::editor::glyphs::cells(display_value) {
                let a = Point {
                    x: center.x + column * cell,
                    y: center.y + row * cell,
                };
                let b = Point {
                    x: a.x + cell,
                    y: a.y + cell,
                };
                let mut builder = PathBuilder::fill();
                for (index, point) in rect_points(a, b).iter().enumerate() {
                    if index == 0 {
                        builder.move_to(at_point(point, scale));
                    } else {
                        builder.line_to(at_point(point, scale));
                    }
                }
                builder.close();
                if let Ok(path) = builder.build() {
                    window.paint_path(path, text_color);
                }
            }
        }
        Annotation::Redact {
            x,
            y,
            width,
            height,
            style,
            ..
        } => {
            let (left, top, width, height) = normalize_rect(*x, *y, *width, *height);
            let a = Point {
                x: left as f32,
                y: top as f32,
            };
            let b = Point {
                x: (left + width) as f32,
                y: (top + height) as f32,
            };
            let mut builder = PathBuilder::fill();
            for (index, point) in rect_points(a, b).iter().enumerate() {
                if index == 0 {
                    builder.move_to(at_point(point, scale));
                } else {
                    builder.line_to(at_point(point, scale));
                }
            }
            builder.close();
            // Committed redactions paint their rasterized patch above; this
            // is the drag preview, before the pixels have been resolved.
            let preview = if style == "blackout" { 1.0 } else { 0.85 };
            if let Ok(path) = builder.build() {
                window.paint_path(path, Srgba::parse("#000000").to_hsla().opacity(preview));
            }
        }
        Annotation::Text { .. } => {
            // Text is drawn by GPUI's text system as a positioned overlay so
            // it uses the same platform font the export rasterizes with.
        }
        Annotation::Pen {
            points,
            stroke,
            stroke_width,
            ..
        } => {
            let coordinates = points_to_coordinates(points);
            if coordinates.len() < 2 {
                return;
            }
            let mut builder = stroked(*stroke_width as f32 * scale);
            let mut first = true;
            for (x, y) in coordinates {
                let at = at_point(
                    &Point {
                        x: x as f32,
                        y: y as f32,
                    },
                    scale,
                );
                if first {
                    builder.move_to(at);
                    first = false;
                } else {
                    builder.line_to(at);
                }
            }
            finish(builder, window, stroke);
        }
        Annotation::Line {
            points,
            stroke,
            stroke_width,
            ..
        }
        | Annotation::Arrow {
            points,
            stroke,
            stroke_width,
            ..
        } => {
            let start = Point {
                x: points[0] as f32,
                y: points[1] as f32,
            };
            let end = Point {
                x: points[2] as f32,
                y: points[3] as f32,
            };

            let mut builder = stroked(*stroke_width as f32 * scale);
            builder.move_to(at_point(&start, scale));
            builder.line_to(at_point(&end, scale));

            if matches!(annotation, Annotation::Arrow { .. }) {
                push_arrow_head(
                    &mut builder,
                    &start,
                    &end,
                    arrow_head_size(*stroke_width) as f32,
                    scale,
                );
            }

            finish(builder, window, stroke);
        }
        Annotation::Rectangle {
            x,
            y,
            width,
            height,
            stroke,
            fill,
            stroke_width,
            ..
        } => {
            let (left, top, width, height) = normalize_rect(*x, *y, *width, *height);
            let a = Point {
                x: left as f32,
                y: top as f32,
            };
            let b = Point {
                x: (left + width) as f32,
                y: (top + height) as f32,
            };
            if let Some(fill_color) = fill {
                let mut builder = PathBuilder::fill();
                for (index, point) in rect_points(a, b).iter().enumerate() {
                    if index == 0 {
                        builder.move_to(at_point(point, scale));
                    } else {
                        builder.line_to(at_point(point, scale));
                    }
                }
                builder.close();
                finish_fill(builder, window, fill_color);
            }
            outline_rect(window, a, b, stroke, *stroke_width, scale);
        }
        Annotation::Circle {
            x,
            y,
            radius,
            stroke,
            fill,
            stroke_width,
            ..
        } => {
            let rx = *radius as f32;
            let ry = rx;
            let cx = *x as f32;
            let cy = *y as f32;

            if let Some(fill_color) = fill {
                if let Some(path) = ellipse_path(cx, cy, rx, ry, scale) {
                    window.paint_path(path, Srgba::parse(fill_color).to_hsla());
                }
            }
            if let Some(path) = ellipse_stroke_path(cx, cy, rx, ry, scale, *stroke_width as f32) {
                window.paint_path(path, Srgba::parse(stroke).to_hsla());
            }
        }
    }
}

fn outline_rect(
    window: &mut gpui::Window,
    a: Point,
    b: Point,
    stroke: &str,
    stroke_width: f64,
    scale: f32,
) {
    let mut builder = stroked(stroke_width as f32 * scale);
    let corners = [
        Point { x: a.x, y: a.y },
        Point { x: b.x, y: a.y },
        Point { x: b.x, y: b.y },
        Point { x: a.x, y: b.y },
        Point { x: a.x, y: a.y },
    ];
    for (index, point) in corners.iter().enumerate() {
        if index == 0 {
            builder.move_to(at_point(point, scale));
        } else {
            builder.line_to(at_point(point, scale));
        }
    }
    builder.close();
    finish(builder, window, stroke);
}

fn rect_points(a: Point, b: Point) -> [Point; 5] {
    [a, Point { x: b.x, y: a.y }, b, Point { x: a.x, y: b.y }, a]
}

fn ellipse_path(cx: f32, cy: f32, rx: f32, ry: f32, scale: f32) -> Option<gpui::Path<Pixels>> {
    if rx <= 0.0 || ry <= 0.0 {
        return None;
    }
    let (origin_x, origin_y) = paint_origin();
    let center = gpui::point(px(origin_x + cx * scale), px(origin_y + cy * scale));
    let radii = gpui::point(px(rx * scale), px(ry * scale));
    let mut builder = PathBuilder::fill();
    builder.move_to(gpui::point(center.x - radii.x, center.y));
    builder.arc_to(
        radii,
        px(0.0),
        true,
        true,
        gpui::point(center.x + radii.x, center.y),
    );
    builder.arc_to(
        radii,
        px(0.0),
        true,
        true,
        gpui::point(center.x - radii.x, center.y),
    );
    builder.close();
    builder.build().ok()
}

fn ellipse_stroke_path(
    cx: f32,
    cy: f32,
    rx: f32,
    ry: f32,
    scale: f32,
    stroke_width: f32,
) -> Option<gpui::Path<Pixels>> {
    if rx <= 0.0 || ry <= 0.0 {
        return None;
    }
    let (origin_x, origin_y) = paint_origin();
    let center = gpui::point(px(origin_x + cx * scale), px(origin_y + cy * scale));
    let radii = gpui::point(px(rx * scale), px(ry * scale));
    let mut builder = PathBuilder::stroke(px(stroke_width * scale));
    set_round(&mut builder);
    builder.move_to(gpui::point(center.x - radii.x, center.y));
    builder.arc_to(
        radii,
        px(0.0),
        true,
        true,
        gpui::point(center.x + radii.x, center.y),
    );
    builder.arc_to(
        radii,
        px(0.0),
        true,
        true,
        gpui::point(center.x - radii.x, center.y),
    );
    builder.build().ok()
}

fn push_arrow_head(
    builder: &mut PathBuilder,
    start: &Point,
    end: &Point,
    head_length: f32,
    scale: f32,
) {
    let angle = (end.y - start.y).atan2(end.x - start.x);
    let spread = 0.5_f32; // ~28.6°, matches lucide-style arrow heads
    let tip = at_point(end, scale);
    for delta in [
        angle + std::f32::consts::PI - spread,
        angle + std::f32::consts::PI + spread,
    ] {
        let wing = Point {
            x: end.x + head_length * delta.cos(),
            y: end.y + head_length * delta.sin(),
        };
        builder.move_to(tip);
        builder.line_to(at_point(&wing, scale));
    }
}

fn stroked(width: f32) -> PathBuilder {
    let mut builder = PathBuilder::stroke(px(width.max(1.0)));
    set_round(&mut builder);
    builder
}

fn set_round(builder: &mut PathBuilder) {
    if let gpui::PathStyle::Stroke(options) = &mut builder.style {
        let width = options.line_width;
        *options = gpui::StrokeOptions::default()
            .with_line_width(width)
            .with_line_cap(lyon::path::LineCap::Round)
            .with_line_join(lyon::path::LineJoin::Round);
    }
}

fn at_point(point: &Point, scale: f32) -> gpui::Point<Pixels> {
    let (origin_x, origin_y) = paint_origin();
    gpui::point(
        px(origin_x + point.x * scale),
        px(origin_y + point.y * scale),
    )
}

thread_local! {
    /// The overlay's window-space origin for the frame being painted; the
    /// path builders below work in window coordinates, so every point is
    /// shifted by it.
    static PAINT_ORIGIN: std::cell::Cell<(f32, f32)> = const { std::cell::Cell::new((0.0, 0.0)) };
}

fn set_paint_origin(origin: gpui::Point<Pixels>) {
    PAINT_ORIGIN.with(|cell| cell.set((f32::from(origin.x), f32::from(origin.y))));
}

fn paint_origin() -> (f32, f32) {
    PAINT_ORIGIN.with(|cell| cell.get())
}

fn finish(builder: PathBuilder, window: &mut gpui::Window, stroke_hex: &str) {
    if let Ok(path) = builder.build() {
        window.paint_path(path, Srgba::parse(stroke_hex).to_hsla());
    }
}

fn finish_fill(builder: PathBuilder, window: &mut gpui::Window, fill_hex: &str) {
    if let Ok(path) = builder.build() {
        window.paint_path(path, Srgba::parse(fill_hex).to_hsla());
    }
}

/// Port of `capture-edge-overlay.tsx` — four hover strips that attach an
/// image to the corresponding edge of the capture.
pub fn capture_edge_overlay(
    theme: &crate::theme::vars::ThemeVars,
    on_edge: std::rc::Rc<dyn Fn(crate::editor::layers::Edge, &mut gpui::Window, &mut App)>,
) -> gpui::AnyElement {
    use crate::editor::layers::Edge;

    const STRIP: f32 = 32.0;
    let edges: [(Edge, &str); 4] = [
        (Edge::Top, "top"),
        (Edge::Bottom, "bottom"),
        (Edge::Left, "left"),
        (Edge::Right, "right"),
    ];

    let mut overlay = div().absolute().inset_0();
    for (edge, name) in edges {
        let handler = on_edge.clone();
        let mut strip = div()
            .id(gpui::SharedString::from(format!("capture-edge-{name}")))
            .absolute()
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .on_mouse_down(
                gpui::MouseButton::Left,
                move |_event, window, cx: &mut App| handler(edge, window, cx),
            )
            .child(
                div()
                    .size(px(24.0))
                    .rounded_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(theme.accent)
                    .text_color(theme.accent_foreground)
                    .opacity(0.4)
                    .child(crate::ui::icon::icon_element("camera", px(12.0))),
            );

        strip = match edge {
            Edge::Top => strip.top_0().left_0().right_0().h(px(STRIP)),
            Edge::Bottom => strip.bottom_0().left_0().right_0().h(px(STRIP)),
            Edge::Left => strip.left_0().top_0().bottom_0().w(px(STRIP)),
            Edge::Right => strip.right_0().top_0().bottom_0().w(px(STRIP)),
            Edge::Primary => strip,
        };
        overlay = overlay.child(strip);
    }
    overlay.into_any_element()
}

/// A dashed box around the selected annotation, matching `SELECTION_STROKE`
/// in the renderer's annotation layer.
fn selection_outline(
    snapshot: &CanvasSnapshot,
    theme: &crate::theme::vars::ThemeVars,
) -> Option<gpui::AnyElement> {
    let id = snapshot.selected.as_ref()?;
    let annotation = snapshot
        .annotations
        .iter()
        .find(|annotation| annotation.id() == id)?;
    let (left, top, right, bottom) = annotation.bounds();
    let zoom = snapshot.zoom;
    const PADDING: f32 = 4.0;

    let _ = theme;
    Some(
        div()
            .absolute()
            .left(px(left as f32 * zoom - PADDING))
            .top(px(top as f32 * zoom - PADDING))
            .w(px((right - left) as f32 * zoom + PADDING * 2.0))
            .h(px((bottom - top) as f32 * zoom + PADDING * 2.0))
            .rounded(px(3.0))
            .border_1()
            .border_color(crate::ui::colors::selection_ring())
            .into_any_element(),
    )
}

/// Dims everything outside the pending crop and outlines the kept region,
/// matching `svg-crop-overlay.tsx`.
fn crop_overlay(
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    zoom: f32,
    theme: &crate::theme::vars::ThemeVars,
) -> gpui::AnyElement {
    let dim = crate::ui::colors::black(0.5);
    let (left, top) = (px(x as f32 * zoom), px(y as f32 * zoom));
    let (w, h) = (px(width as f32 * zoom), px(height as f32 * zoom));

    div()
        .absolute()
        .inset_0()
        .child(div().absolute().left_0().top_0().w(left).h_full().bg(dim))
        .child(
            div()
                .absolute()
                .left(left + w)
                .top_0()
                .right_0()
                .h_full()
                .bg(dim),
        )
        .child(div().absolute().left(left).top_0().w(w).h(top).bg(dim))
        .child(
            div()
                .absolute()
                .left(left)
                .top(top + h)
                .w(w)
                .bottom_0()
                .bg(dim),
        )
        .child(
            div()
                .absolute()
                .left(left)
                .top(top)
                .w(w)
                .h(h)
                .border_1()
                .border_color(theme.accent),
        )
        .child(
            div()
                .absolute()
                .left(left)
                .top(top + h + px(6.0))
                .rounded(px(4.0))
                .bg(theme.popover)
                .border_1()
                .border_color(theme.border)
                .px(px(8.0))
                .py(px(3.0))
                .text_size(px(11.0))
                .text_color(theme.popover_foreground)
                .child("Enter to crop \u{00b7} Esc to cancel"),
        )
        .into_any_element()
}

#[allow(clippy::too_many_arguments)]
fn text_overlay(
    id: &str,
    x: f64,
    y: f64,
    text: &str,
    fill: &str,
    font_size: f64,
    font_family: &str,
    background_color: Option<&str>,
    zoom: f32,
) -> gpui::AnyElement {
    use crate::editor::annotations::{TEXT_BG_PADDING_X, TEXT_BG_PADDING_Y, TEXT_BG_RADIUS};

    let mut element = div()
        .absolute()
        .left(px(x as f32 * zoom))
        .top(px(y as f32 * zoom))
        .text_size(px(font_size as f32 * zoom))
        .text_color(Srgba::parse(fill).to_hsla())
        .child(gpui::SharedString::from(text.to_string()))
        .id(gpui::SharedString::from(format!("text-annotation-{id}")));

    element = match font_family {
        "serif" => element.font_family("Georgia"),
        "mono" => element.font_family("Consolas"),
        "comic" => element.font_family("Comic Sans MS"),
        _ => element,
    };

    if let Some(background) = background_color {
        element = element
            .px(px(TEXT_BG_PADDING_X as f32 * zoom))
            .py(px(TEXT_BG_PADDING_Y as f32 * zoom))
            .rounded(px(TEXT_BG_RADIUS as f32 * zoom))
            .bg(Srgba::parse(background).to_hsla());
    }

    element.into_any_element()
}

/// Port of `getContrastColor` in `renderer/utils/color.ts`.
fn contrast_color(hex: &str) -> gpui::Hsla {
    let parsed = Srgba::parse(hex);
    let luminance = 0.299 * parsed.r + 0.587 * parsed.g + 0.114 * parsed.b;
    if luminance > 0.5 {
        gpui::hsla(0.0, 0.0, 0.0, 1.0)
    } else {
        gpui::hsla(0.0, 0.0, 1.0, 1.0)
    }
}
