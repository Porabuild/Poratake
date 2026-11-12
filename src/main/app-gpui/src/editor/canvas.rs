//! Editor canvas — displays the screenshot and hosts annotation drawing.
//! Port of `editor/editor-canvas.tsx` (image + overlay) with the pointer
//! state machine from the editor window entity.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    canvas, div, img, prelude::*, px, App, Hsla, PathBuilder, Pixels, RenderOnce, ScrollHandle,
    Styled,
};
use herogpui::gpui;

use crate::editor::annotations::{
    arrow_curve_control, arrow_head_size, normalize_rect, number_size_config, Annotation, Offset,
    Point, ResizeHandle, TextBox,
};
use crate::editor::preview_raster;
use crate::theme::color::Srgba;
use crate::theme::vars::active_theme;
use crate::ui::colors::Tool;

/// Everything the canvas needs to draw one frame; owned by the window entity
/// and read at render time through the shared cell.
pub type SnapshotCell = Rc<RefCell<CanvasSnapshot>>;

#[derive(Clone)]
pub struct RedactPatch {
    pub image: std::sync::Arc<gpui::RenderImage>,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

pub struct CanvasSnapshot {
    pub image: Option<std::sync::Arc<gpui::RenderImage>>,
    /// Redacted pixels for committed redactions, rendered from the same code
    /// the export uses so the preview shows exactly what is written out.
    pub redact_patches: std::collections::HashMap<String, RedactPatch>,
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
    pub frame_scale: f64,
    /// The wallpaper backdrop, rendered by the same rasterizer the export
    /// uses. A gradient could be drawn with GPUI, but a background image with
    /// blur and grain could not, and preview and export have to agree.
    pub backdrop: Option<std::sync::Arc<gpui::RenderImage>>,
    pub backdrop_composite: bool,
    pub preview_edges: Vec<crate::editor::export::PreviewEdgeImage>,
    /// The annotation the select tool has picked, outlined on the canvas.
    pub selected: Vec<String>,
    /// The text annotation being re-edited, hidden while its draft shows.
    pub editing_text: Option<String>,
    /// The in-progress marquee selection in image coordinates.
    pub marquee: Option<(f64, f64, f64, f64)>,
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
        let frame_scale = snap.frame_scale;
        let has_preview_edges = !snap.preview_edges.is_empty();
        let layered_composite = snap.backdrop_composite;
        let composited_primary =
            crate::editor::export::composites_primary_backdrop(&wallpaper, &snap.layers);
        let stage_annotations = layered_composite || (snap.image.is_some() && composited_primary);
        let overlay_above = has_preview_edges || stage_annotations;
        let window_frame =
            crate::render::window_frame::theme(&wallpaper.window_frame.style).is_some();
        let balanced_unframed = wallpaper.balance
            && !window_frame
            && snap.layers.is_empty()
            && wallpaper.corners <= 0.0
            && wallpaper.shadow <= 0.0;
        let inset_unframed = !window_frame
            && snap.layers.is_empty()
            && wallpaper.corners <= 0.0
            && wallpaper.shadow <= 0.0;
        let transparent_backdrop = wallpaper.is_active_with_layers(!snap.layers.is_empty())
            && (snap.layers.is_empty() || overlay_above)
            && (!wallpaper.balance || balanced_unframed || layered_composite)
            && (wallpaper.inset <= 0.0 || inset_unframed)
            && (wallpaper.corners <= 0.0 || overlay_above)
            && (window_frame || wallpaper.shadow <= 0.0 || overlay_above);
        let stage_bounds = Rc::new(RefCell::new(None));
        let mut frame = div()
            .relative()
            .w(content_width)
            .h(content_height)
            .when(
                !wallpaper.is_active(!snap.layers.is_empty()) && !layered_composite,
                |el| el.shadow_md(),
            )
            .when(wallpaper.corners > 0.0 && !overlay_above, |el| {
                el.rounded(px(wallpaper.corners as f32 * snap.zoom))
                    .overflow_hidden()
            })
            .when(wallpaper.shadow > 0.0 && !transparent_backdrop, |el| {
                el.shadow_2xl()
            })
            .when(!transparent_backdrop, |el| el.bg(theme.surface));

        if !layered_composite {
            if let Some(image) = snap.image.clone() {
                frame = frame.child(
                    canvas(
                        |_, _, _| (),
                        move |bounds, (), window, _| {
                            let _ = window.paint_image(
                                bounds,
                                bounds,
                                gpui::Corners::default(),
                                image,
                                0,
                                false,
                            );
                        },
                    )
                    .absolute()
                    .inset_0(),
                );
            }
        }

        drop(snap);

        let mut decorations = div().absolute().inset_0();
        for outline in selection_outlines(&self.snapshot.borrow(), &theme) {
            decorations = decorations.child(outline);
        }

        if let Some((x, y, width, height)) = self.snapshot.borrow().marquee {
            let zoom = self.snapshot.borrow().zoom;
            decorations = decorations.child(
                div()
                    .absolute()
                    .left(px(x as f32 * zoom))
                    .top(px(y as f32 * zoom))
                    .w(px(width as f32 * zoom))
                    .h(px(height as f32 * zoom))
                    .bg(theme.primary.opacity(0.1))
                    .border_1()
                    .border_color(theme.primary),
            );
        }

        if let Some((x, y, width, height)) = self.snapshot.borrow().crop {
            let zoom = self.snapshot.borrow().zoom;
            decorations = decorations.child(crop_overlay(x, y, width, height, zoom, &theme));
        }

        let mut decorations = Some(decorations);
        if !overlay_above {
            frame = frame.child(decorations.take().unwrap());
        }

        // Annotation overlay paints every committed annotation plus the draft.
        let overlay_snapshot = self.snapshot.clone();
        let recorder = self.bounds_cell.clone();
        let frame_stage_bounds = stage_bounds.clone();
        let (primary, primary_fg) = (theme.primary, theme.primary_foreground);
        let background = gpui::Rgba::from(theme.background);
        let stage_background = [background.r, background.g, background.b]
            .map(|channel| (channel * 255.).round() as u8);
        let surface = gpui::Rgba::from(
            if transparent_backdrop && self.snapshot.borrow().backdrop.is_none() {
                theme.surface_secondary
            } else {
                theme.surface
            },
        );
        let stage_surface =
            [surface.r, surface.g, surface.b].map(|channel| (channel * 255.).round() as u8);
        frame = frame.child(
            canvas(
                move |bounds, _window, _cx| {
                    *recorder.borrow_mut() = Some(bounds);
                },
                move |bounds, (), window, _cx| {
                    set_paint_origin(bounds.origin);
                    let snapshot = overlay_snapshot.borrow();
                    if !stage_annotations {
                        paint_annotations(
                            window,
                            bounds,
                            *frame_stage_bounds.borrow(),
                            &snapshot,
                            primary.opacity(HALO_OPACITY),
                            stage_background,
                            stage_surface,
                            transparent_backdrop,
                        );
                    }
                    if !overlay_above {
                        paint_selection_handles(window, &snapshot, primary, primary_fg);
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
        let (layout_width, layout_height) = crate::editor::wallpaper::balanced_size(
            image_width,
            image_height,
            self.snapshot.borrow().balance_crop,
            !layers.is_empty() && !wallpaper.supports_edge_preview(true),
        );
        let group = crate::editor::layers::compute(layout_width, layout_height, &layers, spacing);
        let framed = wallpaper.is_active_with_layers(!layers.is_empty());

        let (stage_width, stage_height) = if framed {
            let ((canvas_width, canvas_height), _) = crate::editor::wallpaper::layout(
                &wallpaper,
                group.width,
                group.height,
                frame_scale,
                !layers.is_empty(),
            );
            let (canvas_width, canvas_height) = if composited_primary {
                (canvas_width.round(), canvas_height.round())
            } else {
                (canvas_width, canvas_height)
            };
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
                crate::editor::wallpaper::layout(
                    &wallpaper,
                    group.width,
                    group.height,
                    frame_scale,
                    !layers.is_empty(),
                );
            let (canvas_width, canvas_height, offset_x, offset_y) = if composited_primary {
                (
                    canvas_width.round(),
                    canvas_height.round(),
                    offset_x.round(),
                    offset_y.round(),
                )
            } else {
                (canvas_width, canvas_height, offset_x, offset_y)
            };
            // The capture sits wherever the group's layout put it.
            let (offset_x, offset_y) = (offset_x + group.primary.x, offset_y + group.primary.y);
            let measured_stage = stage_bounds.clone();
            let mut backdrop = div()
                .relative()
                .w(px(canvas_width as f32 * zoom))
                .h(px(canvas_height as f32 * zoom))
                .shadow_md()
                .when(layered_composite, |el| el.bg(theme.background))
                .overflow_hidden()
                .child(
                    canvas(
                        move |bounds, _, _| *measured_stage.borrow_mut() = Some(bounds),
                        |_, (), _, _| (),
                    )
                    .absolute()
                    .inset_0(),
                );
            let rendered = self.snapshot.borrow().backdrop.clone();
            backdrop = match rendered {
                Some(_) if layered_composite && has_preview_edges => backdrop,
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
            let title_bar = crate::render::window_frame::title_bar_height(
                &wallpaper.window_frame.style,
                frame_scale,
            );
            let crop = self.snapshot.borrow().balance_crop;
            let (crop_left, crop_top, crop_right, crop_bottom) =
                crop.unwrap_or((0.0, 0.0, 0.0, 0.0));
            backdrop = backdrop.child(
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
            );
            for edge in self.snapshot.borrow().preview_edges.iter() {
                if layered_composite {
                    let size = edge.image.size(0);
                    backdrop = backdrop.child(
                        div()
                            .absolute()
                            .left(px(edge.x as f32 * zoom))
                            .top(px(edge.y as f32 * zoom))
                            .w(px(edge.width as f32 * zoom))
                            .h(px(edge.height as f32 * zoom))
                            .overflow_hidden()
                            .child(
                                img(edge.image.clone())
                                    .absolute()
                                    .left(px(-(edge.inset_x as f32) * zoom))
                                    .top(px(-(edge.inset_y as f32) * zoom))
                                    .w(px(u32::from(size.width) as f32 * zoom))
                                    .h(px(u32::from(size.height) as f32 * zoom)),
                            ),
                    );
                } else {
                    backdrop = backdrop.child(
                        img(edge.image.clone())
                            .absolute()
                            .left(px(edge.x as f32 * zoom))
                            .top(px(edge.y as f32 * zoom))
                            .w(px(edge.width as f32 * zoom))
                            .h(px(edge.height as f32 * zoom)),
                    );
                }
            }
            let mut stage = div()
                .relative()
                .w(px(canvas_width as f32 * zoom))
                .h(px(canvas_height as f32 * zoom))
                .child(backdrop);
            if let Some(decorations) = decorations {
                let handle_snapshot = self.snapshot.clone();
                stage = stage.child(
                    div()
                        .absolute()
                        .left(px((offset_x as f32 - crop_left) * zoom))
                        .top(px((offset_y as f32 + title_bar as f32 - crop_top) * zoom))
                        .w(content_width)
                        .h(content_height)
                        .child(decorations)
                        .child(
                            canvas(
                                |_, _, _| (),
                                move |bounds, (), window, _cx| {
                                    set_paint_origin(bounds.origin);
                                    if stage_annotations {
                                        let mask = gpui::ContentMask {
                                            bounds: gpui::Bounds {
                                                origin: gpui::point(
                                                    bounds.origin.x
                                                        - px((offset_x as f32 - crop_left) * zoom),
                                                    bounds.origin.y
                                                        - px((offset_y as f32 + title_bar as f32
                                                            - crop_top)
                                                            * zoom),
                                                ),
                                                size: gpui::size(
                                                    px(canvas_width as f32 * zoom),
                                                    px(canvas_height as f32 * zoom),
                                                ),
                                            },
                                        };
                                        window.with_content_mask(Some(mask), |window| {
                                            paint_annotations(
                                                window,
                                                bounds,
                                                *stage_bounds.borrow(),
                                                &handle_snapshot.borrow(),
                                                primary.opacity(HALO_OPACITY),
                                                stage_background,
                                                stage_surface,
                                                transparent_backdrop,
                                            );
                                        });
                                    }
                                    paint_selection_handles(
                                        window,
                                        &handle_snapshot.borrow(),
                                        primary,
                                        primary_fg,
                                    );
                                    set_paint_origin(gpui::point(px(0.0), px(0.0)));
                                },
                            )
                            .absolute()
                            .inset_0(),
                        ),
                );
            }
            stage.into_any_element()
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

fn paint_selection_handles(
    window: &mut gpui::Window,
    snapshot: &CanvasSnapshot,
    primary: Hsla,
    primary_fg: Hsla,
) {
    let [selected] = snapshot.selected.as_slice() else {
        return;
    };
    let Some(annotation) = snapshot
        .annotations
        .iter()
        .find(|annotation| annotation.id() == selected)
    else {
        return;
    };
    draw_selection_handles(window, annotation, snapshot.zoom, primary, primary_fg);
}

fn paint_annotations(
    window: &mut gpui::Window,
    bounds: gpui::Bounds<Pixels>,
    stage_bounds: Option<gpui::Bounds<Pixels>>,
    snapshot: &CanvasSnapshot,
    halo: Hsla,
    stage_background: [u8; 3],
    stage_surface: [u8; 3],
    transparent_backdrop: bool,
) {
    let origin = bounds.origin;
    let zoom = snapshot.zoom;
    let haloed = match snapshot.selected.as_slice() {
        [single] => Some(single.as_str()),
        _ => None,
    };
    let visible: Vec<&Annotation> = snapshot
        .annotations
        .iter()
        .chain(snapshot.draft.iter())
        .filter(|annotation| snapshot.editing_text.as_deref() != Some(annotation.id()))
        .collect();
    let (surface_image, offset_x, offset_y) = if snapshot.backdrop_composite {
        let (width, height) = crate::editor::wallpaper::balanced_size(
            f64::from(snapshot.image_width),
            f64::from(snapshot.image_height),
            snapshot.balance_crop,
            false,
        );
        let layout = crate::editor::layers::compute(
            width,
            height,
            &snapshot.layers,
            snapshot.spacing.max(0.0),
        );
        let (_, (x, y, _, _)) = crate::editor::wallpaper::layout(
            &snapshot.wallpaper,
            layout.width,
            layout.height,
            snapshot.frame_scale,
            !snapshot.layers.is_empty(),
        );
        let (crop_left, crop_top, _, _) = snapshot.balance_crop.unwrap_or_default();
        (
            snapshot.backdrop.as_ref(),
            x.round() + layout.primary.x - f64::from(crop_left),
            y.round() + layout.primary.y - f64::from(crop_top),
        )
    } else {
        (snapshot.image.as_ref(), 0.0, 0.0)
    };
    let surface = preview_raster::Surface {
        image_id: surface_image.map(|image| image.id),
        width: surface_image.map_or(snapshot.image_width.max(0.0) as u32, |image| {
            image.size(0).width.into()
        }),
        height: surface_image.map_or(snapshot.image_height.max(0.0) as u32, |image| {
            image.size(0).height.into()
        }),
        offset_x,
        offset_y,
        native_scale: snapshot.frame_scale,
        pixels: surface_image.and_then(|image| image.as_bytes(0)),
        matte: snapshot.backdrop_composite.then_some(stage_background),
        matte_image: None,
        composite_backdrop: false,
    };
    let matte_image = if transparent_backdrop && !snapshot.backdrop_composite {
        let (width, height) = crate::editor::wallpaper::balanced_size(
            f64::from(snapshot.image_width),
            f64::from(snapshot.image_height),
            snapshot.balance_crop,
            !snapshot.layers.is_empty() && !snapshot.wallpaper.supports_edge_preview(true),
        );
        let group = crate::editor::layers::compute(
            width,
            height,
            &snapshot.layers,
            snapshot.spacing.max(0.0),
        );
        let (_, (x, y, _, _)) = crate::editor::wallpaper::layout(
            &snapshot.wallpaper,
            group.width,
            group.height,
            snapshot.frame_scale,
            !snapshot.layers.is_empty(),
        );
        let (crop_left, crop_top, _, _) = snapshot.balance_crop.unwrap_or_default();
        let title_bar = crate::render::window_frame::title_bar_height(
            &snapshot.wallpaper.window_frame.style,
            snapshot.frame_scale,
        );
        let (x, y) = if crate::editor::export::composites_primary_backdrop(
            &snapshot.wallpaper,
            &snapshot.layers,
        ) {
            (x.round(), y.round())
        } else {
            (x, y)
        };
        snapshot.backdrop.as_deref().map(|image| {
            (
                image,
                x + group.primary.x - f64::from(crop_left),
                y + group.primary.y + title_bar - f64::from(crop_top),
            )
        })
    } else {
        None
    };
    let redaction_surface = preview_raster::Surface {
        matte: Some(if matte_image.is_some() {
            stage_background
        } else {
            stage_surface
        }),
        matte_image,
        composite_backdrop: matte_image.is_some()
            && crate::editor::export::composites_primary_backdrop(
                &snapshot.wallpaper,
                &snapshot.layers,
            ),
        ..surface
    };
    let scale = f64::from(zoom) * f64::from(window.scale_factor());

    let keys = preview_raster::begin_frame(&visible);
    for (index, annotation) in visible.iter().enumerate() {
        if haloed == Some(annotation.id()) {
            draw_selection_halo(window, annotation, zoom, halo);
        }
        let is_draft = snapshot
            .draft
            .as_ref()
            .is_some_and(|draft| std::ptr::eq(draft, *annotation));
        let committed_pixelation = matches!(
            annotation,
            Annotation::Redact { style, .. }
                if style == "pixelate" && !is_draft
        );
        let committed_blur = matches!(
            annotation,
            Annotation::Redact { style, .. }
                if style == "blur" && !is_draft
        );
        let draft_redaction = matches!(annotation, Annotation::Redact { .. }) && is_draft;
        let aligned = (scale - 1.).abs() > 0.0001
            && !draft_redaction
            && (committed_blur
                || committed_pixelation
                || preview_raster::overlaps_filtered_redaction(
                    annotation,
                    &visible[..index],
                    &surface,
                ));
        if !snapshot.backdrop_composite
            && !redaction_surface.composite_backdrop
            && !committed_pixelation
            && !committed_blur
            && !aligned
        {
            if let Annotation::Redact { id, .. } = annotation {
                if let Some(patch) = snapshot.redact_patches.get(id) {
                    paint_patch(
                        window,
                        origin,
                        zoom,
                        patch.image.clone(),
                        patch.x,
                        patch.y,
                        patch.width,
                        patch.height,
                        None,
                    );
                    continue;
                }
            }
        }
        if preview_raster::is_rasterized(annotation)
            || ((snapshot.backdrop_composite || redaction_surface.composite_backdrop)
                && matches!(annotation, Annotation::Redact { .. })
                && !draft_redaction)
            || committed_pixelation
            || committed_blur
            || aligned
        {
            let clear_plain_pixelation = committed_pixelation
                && !snapshot
                    .wallpaper
                    .is_active_with_layers(!snapshot.layers.is_empty());
            let surface = if clear_plain_pixelation {
                &surface
            } else if !snapshot.backdrop_composite {
                &redaction_surface
            } else {
                &surface
            };
            let patch = if aligned {
                let (stage_size, offset) = if let Some((image, x, y)) = matte_image {
                    let size = image.size(0);
                    (
                        (
                            f64::from(u32::from(size.width)),
                            f64::from(u32::from(size.height)),
                        ),
                        (x, y),
                    )
                } else {
                    (
                        (f64::from(surface.width), f64::from(surface.height)),
                        (surface.offset_x, surface.offset_y),
                    )
                };
                let transparent_only = !snapshot.wallpaper.has_background()
                    && snapshot.wallpaper.inset <= 0.
                    && snapshot.wallpaper.corners <= 0.
                    && snapshot.wallpaper.shadow <= 0.
                    && crate::render::window_frame::theme(&snapshot.wallpaper.window_frame.style)
                        .is_none()
                    && snapshot.layers.is_empty();
                let aligned_surface = preview_raster::Surface {
                    matte_image: if transparent_only && !surface.composite_backdrop {
                        None
                    } else {
                        surface.matte_image
                    },
                    ..*surface
                };
                preview_raster::aligned_image(
                    &keys,
                    index,
                    annotation,
                    &visible[..index],
                    &aligned_surface,
                    preview_raster::SamplingGrid {
                        origin: (f32::from(origin.x), f32::from(origin.y)),
                        stage_bounds: {
                            let bounds = if matte_image.is_some() || snapshot.backdrop_composite {
                                stage_bounds.unwrap_or(bounds)
                            } else {
                                bounds
                            };
                            (
                                f32::from(bounds.origin.x),
                                f32::from(bounds.origin.y),
                                f32::from(bounds.size.width),
                                f32::from(bounds.size.height),
                            )
                        },
                        stage_size,
                        offset,
                        zoom,
                        dpi: window.scale_factor(),
                    },
                )
            } else {
                None
            }
            .or_else(|| {
                preview_raster::image(
                    &keys,
                    index,
                    annotation,
                    &visible[..index],
                    surface,
                    if committed_blur { 1.0 } else { scale },
                )
            });
            if let Some(patch) = patch {
                paint_patch(
                    window,
                    origin,
                    zoom,
                    patch.image,
                    patch.x,
                    patch.y,
                    patch.width,
                    patch.height,
                    clear_plain_pixelation.then_some(stage_surface),
                );
            }
            continue;
        }
        draw_annotation(window, annotation, zoom);
    }
    preview_raster::end_frame(window);
}

fn paint_patch(
    window: &mut gpui::Window,
    origin: gpui::Point<Pixels>,
    zoom: f32,
    image: std::sync::Arc<gpui::RenderImage>,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    background: Option<[u8; 3]>,
) {
    let region = gpui::Bounds {
        origin: gpui::point(
            origin.x + px(x as f32 * zoom),
            origin.y + px(y as f32 * zoom),
        ),
        size: gpui::size(px(width as f32 * zoom), px(height as f32 * zoom)),
    };
    if let Some([red, green, blue]) = background {
        let color = (u32::from(red) << 16) | (u32::from(green) << 8) | u32::from(blue);
        window.paint_quad(gpui::fill(region, gpui::rgb(color)));
    }
    let _ = window.paint_image(region, region, gpui::Corners::default(), image, 0, false);
}

pub fn draw_annotation(window: &mut gpui::Window, annotation: &Annotation, scale: f32) {
    match annotation {
        Annotation::Pen { .. }
        | Annotation::Highlight { .. }
        | Annotation::Number { .. }
        | Annotation::Text { .. } => {}
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
        Annotation::Line {
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

            finish(builder, window, stroke);
        }
        Annotation::Arrow {
            points,
            stroke,
            stroke_width,
            arrow_style,
            bend_offset,
            ..
        } => {
            let geometry =
                arrow_geometry(points, arrow_style.as_deref(), *bend_offset, *stroke_width);
            let width = *stroke_width as f32;
            let color = Srgba::parse(stroke).to_hsla();
            paint_arrow(window, &geometry, width, width, scale, color);
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

struct ArrowGeometry {
    start: Point,
    end: Point,
    control: Option<Point>,
    head_length: f32,
    double: bool,
}

fn arrow_geometry(
    points: &[f64; 4],
    arrow_style: Option<&str>,
    bend_offset: Option<Offset>,
    stroke_width: f64,
) -> ArrowGeometry {
    ArrowGeometry {
        start: Point {
            x: points[0] as f32,
            y: points[1] as f32,
        },
        end: Point {
            x: points[2] as f32,
            y: points[3] as f32,
        },
        control: arrow_curve_control(points, arrow_style, bend_offset).map(|(x, y)| Point {
            x: x as f32,
            y: y as f32,
        }),
        head_length: arrow_head_size(stroke_width) as f32,
        double: matches!(arrow_style, Some("double" | "double-curved")),
    }
}

fn paint_arrow(
    window: &mut gpui::Window,
    geometry: &ArrowGeometry,
    line_width: f32,
    head_width: f32,
    scale: f32,
    color: Hsla,
) {
    let ArrowGeometry {
        start,
        end,
        control,
        head_length,
        double,
    } = geometry;

    let mut line = stroked(line_width * scale);
    line.move_to(at_point(start, scale));
    match control {
        Some(control) => line.curve_to(at_point(end, scale), at_point(control, scale)),
        None => line.line_to(at_point(end, scale)),
    }
    if let Ok(path) = line.build() {
        window.paint_path(path, color);
    }

    let mut head = stroked(head_width * scale);
    let end_angle = match control {
        Some(control) => (end.y - control.y).atan2(end.x - control.x),
        None => (end.y - start.y).atan2(end.x - start.x),
    };
    push_arrow_head(&mut head, end, end_angle, *head_length, scale);
    if *double {
        let start_angle = match control {
            Some(control) => (start.y - control.y).atan2(start.x - control.x),
            None => (start.y - end.y).atan2(start.x - end.x),
        };
        push_arrow_head(&mut head, start, start_angle, *head_length, scale);
    }
    if let Ok(path) = head.build() {
        window.paint_path(path, color);
    }
}

const HALO_STROKE: f32 = 6.0;
const HALO_OPACITY: f32 = 0.8;
const HALO_ARROW_HEAD_STROKE: f32 = 4.0;
const HALO_NUMBER_OFFSET: f32 = 3.0;
const HALO_PEN_STROKE: f32 = 2.0;
const HALO_HIGHLIGHT_STROKE: f32 = 3.0;
const HALO_TEXT_STROKE: f32 = 4.0;

fn halo_stroke(stroke_width: f64) -> f32 {
    stroke_width as f32 + HALO_STROKE
}

fn number_halo_radius(size: &str) -> f32 {
    number_size_config(size).0 as f32 + HALO_NUMBER_OFFSET
}

fn text_halo_rect(text_box: &TextBox) -> (f64, f64, f64, f64) {
    let inset = f64::from(HALO_TEXT_STROKE) / 2.0;
    (
        text_box.x - inset,
        text_box.y - inset,
        text_box.width + inset * 2.0,
        text_box.height + inset * 2.0,
    )
}

fn draw_selection_halo(
    window: &mut gpui::Window,
    annotation: &Annotation,
    scale: f32,
    color: Hsla,
) {
    match annotation {
        Annotation::Rectangle {
            x,
            y,
            width,
            height,
            stroke_width,
            ..
        } => halo_rect(
            window,
            *x,
            *y,
            *width,
            *height,
            halo_stroke(*stroke_width),
            scale,
            color,
        ),
        Annotation::Redact {
            x,
            y,
            width,
            height,
            ..
        } => halo_rect(window, *x, *y, *width, *height, HALO_STROKE, scale, color),
        Annotation::Circle {
            x,
            y,
            radius,
            stroke_width,
            ..
        } => halo_ellipse(
            window,
            *x as f32,
            *y as f32,
            *radius as f32,
            halo_stroke(*stroke_width),
            scale,
            color,
        ),
        Annotation::Number { x, y, size, .. } => halo_ellipse(
            window,
            *x as f32,
            *y as f32,
            number_halo_radius(size),
            HALO_STROKE,
            scale,
            color,
        ),
        Annotation::Line {
            points,
            stroke_width,
            ..
        } => {
            let mut builder = stroked(halo_stroke(*stroke_width) * scale);
            builder.move_to(at_point(
                &Point {
                    x: points[0] as f32,
                    y: points[1] as f32,
                },
                scale,
            ));
            builder.line_to(at_point(
                &Point {
                    x: points[2] as f32,
                    y: points[3] as f32,
                },
                scale,
            ));
            if let Ok(path) = builder.build() {
                window.paint_path(path, color);
            }
        }
        Annotation::Arrow {
            points,
            stroke_width,
            arrow_style,
            bend_offset,
            ..
        } => {
            let geometry =
                arrow_geometry(points, arrow_style.as_deref(), *bend_offset, *stroke_width);
            paint_arrow(
                window,
                &geometry,
                halo_stroke(*stroke_width),
                *stroke_width as f32 + HALO_ARROW_HEAD_STROKE,
                scale,
                color,
            );
        }
        Annotation::Text { .. } => {
            let Some(text_box) = annotation.text_box() else {
                return;
            };
            let (left, top, width, height) = text_halo_rect(&text_box);
            let radians = text_box.rotation.to_radians() as f32;
            let (sin, cos) = radians.sin_cos();
            let (center_x, center_y) = ((left + width / 2.0) as f32, (top + height / 2.0) as f32);
            let (half_w, half_h) = ((width / 2.0) as f32, (height / 2.0) as f32);
            let corners = [
                (-half_w, -half_h),
                (half_w, -half_h),
                (half_w, half_h),
                (-half_w, half_h),
            ]
            .map(|(dx, dy)| Point {
                x: center_x + dx * cos - dy * sin,
                y: center_y + dx * sin + dy * cos,
            });
            halo_polygon(window, &corners, HALO_TEXT_STROKE, scale, color);
        }
        Annotation::Pen {
            points,
            stroke_width,
            ..
        } => {
            let outline = crate::render::annotations::pen_outline(points, *stroke_width);
            let Some(first) = outline.first() else {
                return;
            };
            let mut builder = stroked(HALO_PEN_STROKE * scale);
            builder.move_to(at_outline_point(*first, scale));
            for (control, end) in crate::render::annotations::freehand_segments(&outline) {
                builder.curve_to(
                    at_outline_point(end, scale),
                    at_outline_point(control, scale),
                );
            }
            if let Ok(path) = builder.build() {
                window.paint_path(path, color);
            }
        }
        Annotation::Highlight {
            points,
            stroke_width,
            ..
        } => {
            let outline = crate::render::annotations::highlighter_outline(points, *stroke_width);
            let corners: Vec<Point> = outline
                .iter()
                .map(|(x, y)| Point {
                    x: *x as f32,
                    y: *y as f32,
                })
                .collect();
            halo_polygon(window, &corners, HALO_HIGHLIGHT_STROKE, scale, color);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn halo_rect(
    window: &mut gpui::Window,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    stroke_width: f32,
    scale: f32,
    color: Hsla,
) {
    let (left, top, width, height) = normalize_rect(x, y, width, height);
    let corners = [
        Point {
            x: left as f32,
            y: top as f32,
        },
        Point {
            x: (left + width) as f32,
            y: top as f32,
        },
        Point {
            x: (left + width) as f32,
            y: (top + height) as f32,
        },
        Point {
            x: left as f32,
            y: (top + height) as f32,
        },
    ];
    halo_polygon(window, &corners, stroke_width, scale, color);
}

fn halo_ellipse(
    window: &mut gpui::Window,
    cx: f32,
    cy: f32,
    radius: f32,
    stroke_width: f32,
    scale: f32,
    color: Hsla,
) {
    if let Some(path) = ellipse_stroke_path(cx, cy, radius, radius, scale, stroke_width) {
        window.paint_path(path, color);
    }
}

fn halo_polygon(
    window: &mut gpui::Window,
    corners: &[Point],
    stroke_width: f32,
    scale: f32,
    color: Hsla,
) {
    let Some(first) = corners.first() else {
        return;
    };
    let mut builder = stroked(stroke_width * scale);
    builder.move_to(at_point(first, scale));
    for corner in corners.iter().skip(1).chain(std::iter::once(first)) {
        builder.line_to(at_point(corner, scale));
    }
    if let Ok(path) = builder.build() {
        window.paint_path(path, color);
    }
}

fn at_outline_point(point: (f64, f64), scale: f32) -> gpui::Point<Pixels> {
    at_point(
        &Point {
            x: point.0 as f32,
            y: point.1 as f32,
        },
        scale,
    )
}

fn draw_selection_handles(
    window: &mut gpui::Window,
    annotation: &Annotation,
    scale: f32,
    primary: Hsla,
    primary_fg: Hsla,
) {
    const HANDLE_SIZE: f32 = 12.0;
    const HANDLE_STROKE: f32 = 2.0;
    if scale <= 0.0 {
        return;
    }
    let half = HANDLE_SIZE / 2.0 / scale;
    let stroke = HANDLE_STROKE / scale;
    for (handle, x, y) in annotation.handles() {
        let (x, y) = (x as f32, y as f32);
        match handle {
            ResizeHandle::TopLeft
            | ResizeHandle::TopRight
            | ResizeHandle::BottomLeft
            | ResizeHandle::BottomRight => {
                let rotation = match annotation {
                    Annotation::Text { .. } => annotation
                        .text_box()
                        .map(|text_box| text_box.rotation)
                        .unwrap_or(0.0) as f32,
                    _ => 0.0,
                };
                handle_square(
                    window, x, y, half, rotation, scale, primary_fg, primary, stroke,
                );
            }
            ResizeHandle::Start | ResizeHandle::End => {
                handle_circle(window, x, y, half, scale, primary_fg, primary, stroke);
            }
            ResizeHandle::Bend => {
                if let Annotation::Arrow { points, .. } = annotation {
                    let mid = Point {
                        x: ((points[0] + points[2]) / 2.0) as f32,
                        y: ((points[1] + points[3]) / 2.0) as f32,
                    };
                    connector_line(window, &mid, &Point { x, y }, scale, primary);
                }
                handle_circle(
                    window,
                    x,
                    y,
                    half - 1.0 / scale,
                    scale,
                    primary,
                    primary_fg,
                    stroke,
                );
            }
            ResizeHandle::Rotate => {
                if let Some(text_box) = annotation.text_box() {
                    let rotation = text_box.rotation.to_radians();
                    let (sin, cos) = rotation.sin_cos();
                    let top = Point {
                        x: (text_box.center_x + sin * text_box.height / 2.0) as f32,
                        y: (text_box.center_y - cos * text_box.height / 2.0) as f32,
                    };
                    connector_line(window, &top, &Point { x, y }, scale, primary);
                }
                handle_circle(window, x, y, half, scale, primary, primary_fg, stroke);
            }
        }
    }
}

fn handle_square(
    window: &mut gpui::Window,
    x: f32,
    y: f32,
    half: f32,
    rotation_deg: f32,
    scale: f32,
    fill: Hsla,
    stroke: Hsla,
    stroke_width: f32,
) {
    let rotation = rotation_deg.to_radians();
    let (sin, cos) = rotation.sin_cos();
    let corners =
        [(-half, -half), (half, -half), (half, half), (-half, half)].map(|(dx, dy)| Point {
            x: x + dx * cos - dy * sin,
            y: y + dx * sin + dy * cos,
        });
    let mut filled = PathBuilder::fill();
    for (index, corner) in corners.iter().enumerate() {
        if index == 0 {
            filled.move_to(at_point(corner, scale));
        } else {
            filled.line_to(at_point(corner, scale));
        }
    }
    filled.close();
    if let Ok(path) = filled.build() {
        window.paint_path(path, fill);
    }
    let mut outlined = PathBuilder::stroke(px(stroke_width * scale));
    for (index, corner) in corners.iter().chain(corners.first()).enumerate() {
        if index == 0 {
            outlined.move_to(at_point(corner, scale));
        } else {
            outlined.line_to(at_point(corner, scale));
        }
    }
    if let Ok(path) = outlined.build() {
        window.paint_path(path, stroke);
    }
}

fn handle_circle(
    window: &mut gpui::Window,
    x: f32,
    y: f32,
    radius: f32,
    scale: f32,
    fill: Hsla,
    stroke: Hsla,
    stroke_width: f32,
) {
    if let Some(path) = ellipse_path(x, y, radius, radius, scale) {
        window.paint_path(path, fill);
    }
    if let Some(path) = ellipse_stroke_path(x, y, radius, radius, scale, stroke_width) {
        window.paint_path(path, stroke);
    }
}

fn connector_line(window: &mut gpui::Window, from: &Point, to: &Point, scale: f32, color: Hsla) {
    let mut builder = PathBuilder::stroke(px(1.0));
    builder.move_to(at_point(from, scale));
    builder.line_to(at_point(to, scale));
    if let Ok(path) = builder.build() {
        window.paint_path(path, color);
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
    tip_image: &Point,
    angle: f32,
    head_length: f32,
    scale: f32,
) {
    let spread = 0.5_f32; // ~28.6°, matches lucide-style arrow heads
    let tip = at_point(tip_image, scale);
    for delta in [
        angle + std::f32::consts::PI - spread,
        angle + std::f32::consts::PI + spread,
    ] {
        let wing = Point {
            x: tip_image.x + head_length * delta.cos(),
            y: tip_image.y + head_length * delta.sin(),
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
        let keyboard_handler = on_edge.clone();
        let focus_color = theme.accent;
        let group = gpui::SharedString::from(format!("capture-edge-{name}"));
        let indicator = div()
            .absolute()
            .bg(theme.accent)
            .opacity(0.0)
            .group_hover(group.clone(), |style| style.opacity(1.0));
        let indicator = match edge {
            Edge::Top => indicator.top_0().left_0().right_0().h(px(1.0)),
            Edge::Bottom => indicator.bottom_0().left_0().right_0().h(px(1.0)),
            Edge::Left => indicator.left_0().top_0().bottom_0().w(px(1.0)),
            Edge::Right => indicator.right_0().top_0().bottom_0().w(px(1.0)),
            Edge::Primary => indicator,
        };
        let mut strip = div()
            .id(group.clone())
            .role(gpui::accesskit::Role::Button)
            .aria_label(gpui::SharedString::from(format!(
                "Capture and attach at the {name} edge"
            )))
            .focusable()
            .tab_stop(true)
            .focus_visible(move |style| style.border_2().border_color(focus_color))
            .group(group.clone())
            .absolute()
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .on_any_mouse_down(move |event, window, cx: &mut App| {
                if !matches!(
                    event.button,
                    gpui::MouseButton::Left | gpui::MouseButton::Right
                ) {
                    return;
                }
                cx.stop_propagation();
                handler(edge, window, cx);
            })
            .on_click(move |event, window, cx| {
                if event.is_keyboard() {
                    cx.stop_propagation();
                    keyboard_handler(edge, window, cx);
                }
            })
            .child(indicator)
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
                    .group_hover(group, |style| style.opacity(1.0))
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
    #[cfg(feature = "parity")]
    let overlay = overlay.on_children_prepainted(|bounds, _, _| {
        crate::ui::parity::record("capture-edges", bounds);
    });
    overlay.into_any_element()
}

/// Port of `drop-zone-overlay.tsx` — the dashed frame shown while an image
/// drag hovers the stage, with the hovered edge's quarter tinted.
pub fn drop_zone_overlay(
    edge: Option<crate::editor::layers::Edge>,
    theme: &crate::theme::vars::ThemeVars,
) -> gpui::AnyElement {
    use crate::editor::layers::Edge;

    let overlay = div().absolute().inset_0().child(
        div()
            .absolute()
            .inset_0()
            .rounded(px(8.0))
            .border_2()
            .border_color(theme.primary.opacity(0.5))
            .bg(theme.primary.opacity(0.05)),
    );
    let Some(edge) = edge else {
        return overlay.into_any_element();
    };
    let zone = match edge {
        Edge::Top => div().absolute().top_0().left_0().right_0().h_1_4(),
        Edge::Bottom => div().absolute().bottom_0().left_0().right_0().h_1_4(),
        Edge::Left => div().absolute().left_0().top_0().bottom_0().w_1_4(),
        Edge::Right => div().absolute().right_0().top_0().bottom_0().w_1_4(),
        Edge::Primary => div().absolute().inset_0(),
    }
    .bg(theme.primary.opacity(0.15));
    let indicator = match edge {
        Edge::Top => div()
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .h(px(4.0))
            .rounded_b(px(4.0)),
        Edge::Bottom => div()
            .absolute()
            .bottom_0()
            .left_0()
            .right_0()
            .h(px(4.0))
            .rounded_t(px(4.0)),
        Edge::Left => div()
            .absolute()
            .left_0()
            .top_0()
            .bottom_0()
            .w(px(4.0))
            .rounded_r(px(4.0)),
        Edge::Right => div()
            .absolute()
            .right_0()
            .top_0()
            .bottom_0()
            .w(px(4.0))
            .rounded_l(px(4.0)),
        Edge::Primary => div().absolute().inset_0(),
    }
    .bg(theme.primary);
    overlay.child(zone).child(indicator).into_any_element()
}

fn selection_outlines(
    snapshot: &CanvasSnapshot,
    theme: &crate::theme::vars::ThemeVars,
) -> Vec<gpui::AnyElement> {
    const PADDING: f32 = 4.0;
    if snapshot.selected.len() < 2 {
        return Vec::new();
    }
    let zoom = snapshot.zoom;
    snapshot
        .selected
        .iter()
        .filter_map(|id| {
            snapshot
                .annotations
                .iter()
                .find(|annotation| annotation.id() == id)
        })
        .map(|annotation| {
            let (left, top, right, bottom) = annotation.bounds();
            div()
                .absolute()
                .left(px(left as f32 * zoom - PADDING))
                .top(px(top as f32 * zoom - PADDING))
                .w(px((right - left) as f32 * zoom + PADDING * 2.0))
                .h(px((bottom - top) as f32 * zoom + PADDING * 2.0))
                .rounded(px(3.0))
                .border_1()
                .border_color(theme.primary)
                .into_any_element()
        })
        .collect()
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
    let (nx, ny, nw, nh) = (
        if width < 0.0 { x + width } else { x },
        if height < 0.0 { y + height } else { y },
        width.abs(),
        height.abs(),
    );
    let (left, top) = (px(nx as f32 * zoom), px(ny as f32 * zoom));
    let (w, h) = (px(nw as f32 * zoom), px(nh as f32 * zoom));
    let handle = px(12.0);
    let half_handle = px(6.0);
    let corners = [
        (left - half_handle, top - half_handle),
        (left + w - half_handle, top - half_handle),
        (left - half_handle, top + h - half_handle),
        (left + w - half_handle, top + h - half_handle),
    ];

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
                .rounded(px(1.0))
                .border_2()
                .border_color(theme.primary),
        )
        .children(corners.into_iter().map(|(hx, hy)| {
            div()
                .absolute()
                .left(hx)
                .top(hy)
                .w(handle)
                .h(handle)
                .rounded(px(2.0))
                .bg(theme.primary_foreground)
                .border_2()
                .border_color(theme.primary)
        }))
        .child(crop_hint(left, top, w, h, theme))
        .into_any_element()
}

const CROP_HINT_GAP: f32 = 24.0;
const CROP_HINT_FONT_SIZE: f32 = 14.0;

fn crop_hint_layout(left: f32, top: f32, width: f32, height: f32) -> (f32, f32, f32) {
    (left, top + height + CROP_HINT_GAP, width)
}

fn crop_hint(
    left: Pixels,
    top: Pixels,
    width: Pixels,
    height: Pixels,
    theme: &crate::theme::vars::ThemeVars,
) -> gpui::Div {
    let (hint_left, hint_top, hint_width) = crop_hint_layout(
        f32::from(left),
        f32::from(top),
        f32::from(width),
        f32::from(height),
    );
    div()
        .absolute()
        .left(px(hint_left))
        .top(px(hint_top))
        .w(px(hint_width))
        .flex()
        .justify_center()
        .text_size(px(CROP_HINT_FONT_SIZE))
        .text_color(theme.primary)
        .child(div().whitespace_nowrap().child("Press Enter to crop"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{point, Context, Modifiers, MouseButton, Render, Window};
    use herogpui::test::TestWindowExt;

    struct CaptureEdgeSurface {
        edges: Rc<RefCell<Vec<crate::editor::layers::Edge>>>,
        bubbled: Rc<std::cell::Cell<usize>>,
    }

    impl Render for CaptureEdgeSurface {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let edges = self.edges.clone();
            let bubbled = self.bubbled.clone();
            let surface = div()
                .relative()
                .w(px(400.0))
                .h(px(300.0))
                .overflow_hidden()
                .on_any_mouse_down(move |_, _, _| bubbled.set(bubbled.get() + 1))
                .child(capture_edge_overlay(
                    &crate::theme::vars::ThemeVars::default(),
                    Rc::new(move |edge, _, _| edges.borrow_mut().push(edge)),
                ));
            crate::ui::window_root::focus_root(surface, window, cx)
        }
    }

    #[herogpui::test]
    fn capture_edges_accept_both_mouse_buttons_without_bubbling(cx: &mut gpui::TestAppContext) {
        use crate::editor::layers::Edge;

        let edges = Rc::new(RefCell::new(Vec::new()));
        let bubbled = Rc::new(std::cell::Cell::new(0));
        let (_view, cx) = herogpui::test::open_window(cx, {
            let edges = edges.clone();
            let bubbled = bubbled.clone();
            move |_, _| CaptureEdgeSurface { edges, bubbled }
        });
        cx.settle();
        for (edge, x, y) in [
            (Edge::Top, 200.0, 16.0),
            (Edge::Bottom, 200.0, 284.0),
            (Edge::Left, 16.0, 150.0),
            (Edge::Right, 384.0, 150.0),
        ] {
            let at = point(px(x), px(y));
            for button in [MouseButton::Left, MouseButton::Right] {
                cx.simulate_mouse_move(at, None, Modifiers::none());
                cx.simulate_mouse_down(at, button, Modifiers::none());
                cx.simulate_mouse_up(at, button, Modifiers::none());
                assert_eq!(edges.borrow_mut().pop(), Some(edge));
                assert_eq!(bubbled.get(), 0);
            }
        }
        let at = point(px(200.0), px(16.0));
        cx.simulate_mouse_move(at, None, Modifiers::none());
        cx.simulate_mouse_down(at, MouseButton::Middle, Modifiers::none());
        assert!(edges.borrow().is_empty());
        assert_eq!(bubbled.get(), 1);
    }

    #[herogpui::test]
    fn capture_edges_can_be_activated_from_the_keyboard(cx: &mut gpui::TestAppContext) {
        use crate::editor::layers::Edge;

        let edges = Rc::new(RefCell::new(Vec::new()));
        let (_view, cx) = herogpui::test::open_window(cx, {
            let edges = edges.clone();
            move |_, _| CaptureEdgeSurface {
                edges,
                bubbled: Rc::new(std::cell::Cell::new(0)),
            }
        });
        cx.settle();
        for (edge, key, width, height) in [
            (Edge::Top, "enter", 400.0, 32.0),
            (Edge::Bottom, "space", 400.0, 32.0),
            (Edge::Left, "enter", 32.0, 300.0),
            (Edge::Right, "space", 32.0, 300.0),
        ] {
            cx.press("tab");
            cx.settle();
            cx.update(|window, _| {
                let scale = window.scale_factor();
                let quads = window.painted_quads();
                let border: Vec<_> = quads
                    .iter()
                    .filter(|quad| {
                        (quad.bounds.size.width.0 / scale - width).abs() < 0.01
                            && (quad.bounds.size.height.0 / scale - height).abs() < 0.01
                            && quad.border_widths.top.0 / scale >= 2.0
                            && quad.border_widths.right.0 / scale >= 2.0
                            && quad.border_widths.bottom.0 / scale >= 2.0
                            && quad.border_widths.left.0 / scale >= 2.0
                    })
                    .collect();
                let bounds = border
                    .first()
                    .expect("the focused strip must have a border")
                    .bounds;
                let (left, top, right, bottom) = (
                    bounds.left().0,
                    bounds.top().0,
                    bounds.right().0,
                    bounds.bottom().0,
                );
                for (x, y) in [
                    ((left + right) / 2.0, top + scale),
                    ((left + right) / 2.0, bottom - scale),
                    (left + scale, (top + bottom) / 2.0),
                    (right - scale, (top + bottom) / 2.0),
                ] {
                    assert!(
                        border.iter().any(|quad| {
                            let mask = quad.content_mask.bounds;
                            x >= mask.left().0
                                && x < mask.right().0
                                && y >= mask.top().0
                                && y < mask.bottom().0
                        }),
                        "the focused {edge:?} strip border must not be clipped"
                    );
                }
            });
            cx.press(key);
            assert_eq!(edges.borrow_mut().pop(), Some(edge));
            assert!(edges.borrow().is_empty());
        }
    }

    #[test]
    fn shape_halo_restrokes_six_pixels_wider() {
        assert_eq!(halo_stroke(2.0), 8.0);
        assert_eq!(halo_stroke(12.5), 18.5);
    }

    #[test]
    fn number_halo_is_three_pixels_outside_the_badge() {
        assert_eq!(number_halo_radius("small"), 17.0);
        assert_eq!(number_halo_radius("medium"), 21.0);
        assert_eq!(number_halo_radius("large"), 27.0);
    }

    #[test]
    fn text_halo_grows_by_half_its_stroke_on_every_side() {
        let text_box = TextBox {
            x: 10.0,
            y: 20.0,
            width: 100.0,
            height: 40.0,
            center_x: 60.0,
            center_y: 40.0,
            rotation: 0.0,
        };
        assert_eq!(text_halo_rect(&text_box), (8.0, 18.0, 104.0, 44.0));
    }

    #[test]
    fn circle_halo_follows_the_circle() {
        let circle = Annotation::Circle {
            id: "c".into(),
            x: 50.0,
            y: 60.0,
            radius: 30.0,
            stroke: "#ff0000".into(),
            stroke_width: 4.0,
            fill: None,
        };
        let Annotation::Circle {
            radius,
            stroke_width,
            ..
        } = &circle
        else {
            unreachable!();
        };
        assert!(ellipse_stroke_path(
            50.0,
            60.0,
            *radius as f32,
            *radius as f32,
            1.0,
            halo_stroke(*stroke_width),
        )
        .is_some());
        assert_eq!(halo_stroke(*stroke_width), 10.0);
    }

    #[test]
    fn arrow_halo_reuses_the_arrow_path() {
        let points = [0.0, 0.0, 100.0, 0.0];
        let geometry = arrow_geometry(&points, Some("double"), None, 4.0);
        assert!(geometry.double);
        assert!(geometry.control.is_none());
        assert_eq!(geometry.head_length, 20.0);
        assert_eq!(geometry.start.x, 0.0);
        assert_eq!(geometry.end.x, 100.0);
    }

    #[test]
    fn crop_hint_sits_centred_below_the_box() {
        let (left, top, width) = crop_hint_layout(40.0, 60.0, 200.0, 120.0);
        assert_eq!(left, 40.0);
        assert_eq!(top, 204.0);
        assert_eq!(width, 200.0);
    }
}
