//! Export composition for the image editor. Annotations go through the shared
//! rasterizer in `crate::render`, the same one the video editor composites
//! with, so a saved or copied image matches the editor preview.

use image::RgbaImage;
use tiny_skia::{Color, FillRule, Pixmap};

use crate::editor::annotations::Annotation;
use crate::editor::wallpaper::WallpaperSettings;
use crate::render::canvas::{paint_rounded_shadow, rounded_rect_path, Canvas, Shadow};
use crate::render::{annotations as annotation_renderer, color, gradient};

#[derive(Clone)]
pub struct PreviewEdgeImage {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub inset_x: u32,
    pub inset_y: u32,
    pub image: std::sync::Arc<gpui::RenderImage>,
}

pub fn composites_wallpaper_effects(
    wallpaper: &WallpaperSettings,
    layers: &[crate::editor::layers::ImageLayer],
) -> bool {
    layers.is_empty()
        && !wallpaper.balance
        && (wallpaper.shadow > 0.0
            || crate::render::window_frame::theme(&wallpaper.window_frame.style).is_some()
            || (wallpaper.inset > 0.0 && wallpaper.corners > 0.0))
}

pub fn composites_primary_backdrop(
    wallpaper: &WallpaperSettings,
    layers: &[crate::editor::layers::ImageLayer],
) -> bool {
    layers.is_empty()
        && !wallpaper.balance
        && (composites_wallpaper_effects(wallpaper, layers)
            || (wallpaper.corners <= 0.0
                && wallpaper.shadow <= 0.0
                && crate::render::window_frame::theme(&wallpaper.window_frame.style).is_none()))
        && wallpaper.is_active(false)
}

/// Renders the base image plus every annotation into a fresh RGBA buffer at
/// natural size, on its wallpaper backdrop when one is set.
#[allow(dead_code)]
pub fn compose(
    base: Option<&image::DynamicImage>,
    width: u32,
    height: u32,
    annotations: &[Annotation],
    wallpaper: &WallpaperSettings,
    frame_scale: f64,
) -> RgbaImage {
    compose_with_layers(
        base,
        width,
        height,
        annotations,
        wallpaper,
        &[],
        frame_scale,
    )
}

/// The full composition, including any images attached to the capture's edges.
pub fn compose_with_layers(
    base: Option<&image::DynamicImage>,
    width: u32,
    height: u32,
    annotations: &[Annotation],
    wallpaper: &WallpaperSettings,
    layers: &[crate::editor::layers::ImageLayer],
    frame_scale: f64,
) -> RgbaImage {
    let (width, height) = match base {
        Some(image) => (image.width().max(1), image.height().max(1)),
        None => (width.max(1), height.max(1)),
    };

    let mut canvas = match Canvas::new(width, height) {
        Some(canvas) => canvas,
        None => return RgbaImage::new(width, height),
    };
    if let Some(base) = base {
        if let Some(pixmap) = from_rgba(&base.to_rgba8()) {
            canvas.draw_pixmap(pixmap.as_ref(), 0.0, 0.0, width as f32, height as f32);
        }
    }
    let original_balance = wallpaper
        .balance
        .then(|| balance_bounds_pixmap(canvas.pixmap()))
        .flatten();
    let original_inset_color = layers.is_empty().then(|| {
        (wallpaper.inset > 0.0)
            .then(|| {
                crate::render::color_detection::dominant_inset_color(
                    canvas.pixmap().as_ref(),
                    original_balance.unwrap_or_default(),
                )
            })
            .flatten()
    });
    let layered_wallpaper = if !layers.is_empty() && wallpaper.supports_edge_preview(true) {
        let cropped = original_balance.and_then(|bounds| balance_crop(canvas.pixmap(), bounds));
        let primary = cropped.as_ref().unwrap_or(canvas.pixmap());
        let offset = original_balance
            .map(|bounds| (bounds.left as f32, bounds.top as f32))
            .unwrap_or_default();
        compose_layered_wallpaper(primary, wallpaper, layers, annotations, frame_scale, offset)
    } else {
        None
    };
    if let Some(composed) = layered_wallpaper {
        return to_rgba(&composed);
    }

    let composited_primary = base.is_some() && composites_primary_backdrop(wallpaper, layers);
    if !composited_primary {
        annotation_renderer::draw_all_screenshot(&mut canvas, annotations, frame_scale);
    }
    let mut annotated = canvas.into_pixmap();

    // Attached images sit around the annotated capture, before the wallpaper
    // frames the whole group.
    if !layers.is_empty() {
        if let Some(combined) = compose_layers(&annotated, layers, wallpaper.spacing.max(0.0)) {
            annotated = combined;
        }
    }

    if !wallpaper.is_active(!layers.is_empty()) {
        return to_rgba(&annotated);
    }

    // `balance` trims the uniform border away before the image is placed, so
    // the content sits centred inside the wallpaper rather than the artwork.
    if wallpaper.balance {
        let bounds = if layers.is_empty() {
            original_balance
        } else {
            balance_bounds_pixmap(&annotated)
        };
        if let Some(cropped) = bounds.and_then(|bounds| balance_crop(&annotated, bounds)) {
            annotated = cropped;
        }
    }
    let composed = compose_wallpaper(
        &annotated,
        wallpaper,
        frame_scale,
        original_inset_color,
        !layers.is_empty(),
    );
    if !composited_primary {
        return to_rgba(&composed);
    }
    let (_, (offset_x, offset_y, _, _)) = crate::editor::wallpaper::layout(
        wallpaper,
        annotated.width() as f64,
        annotated.height() as f64,
        frame_scale,
        false,
    );
    let mut canvas = Canvas::from_pixmap(composed);
    let title_bar =
        crate::render::window_frame::title_bar_height(&wallpaper.window_frame.style, frame_scale);
    canvas.translate(
        offset_x.round() as f32,
        (offset_y.round() + title_bar) as f32,
    );
    annotation_renderer::draw_all_screenshot(&mut canvas, annotations, frame_scale);
    to_rgba(canvas.pixmap())
}

/// Lays the primary image and its attached layers out on one surface.
fn compose_layers(
    primary: &Pixmap,
    layers: &[crate::editor::layers::ImageLayer],
    spacing: f64,
) -> Option<Pixmap> {
    use crate::editor::layers;

    let layout = layers::compute(
        primary.width() as f64,
        primary.height() as f64,
        layers,
        spacing,
    );
    let mut canvas = Canvas::new(
        layout.width.round().max(1.0) as u32,
        layout.height.round().max(1.0) as u32,
    )?;

    canvas.draw_pixmap(
        primary.as_ref(),
        layout.primary.x as f32,
        layout.primary.y as f32,
        layout.primary.width as f32,
        layout.primary.height as f32,
    );

    for (id, rect) in &layout.layers {
        let Some(source) = layers
            .iter()
            .find(|layer| &layer.id == id)
            .and_then(|layer| gradient::load_image(&layer.image_url))
        else {
            continue;
        };
        canvas.draw_pixmap(
            source.as_ref(),
            rect.x as f32,
            rect.y as f32,
            rect.width as f32,
            rect.height as f32,
        );
    }

    Some(canvas.into_pixmap())
}

fn draw_unframed_image(
    canvas: &mut Canvas,
    image: tiny_skia::PixmapRef<'_>,
    rect: (f32, f32, f32, f32),
    radius: f32,
    shadow: Option<Shadow>,
) {
    let (x, y, width, height) = rect;
    let Some(path) = rounded_rect_path(x, y, width, height, radius) else {
        return;
    };
    canvas.save();
    if shadow.is_some_and(|shadow| {
        paint_rounded_shadow(canvas, rect, radius, shadow, Color::WHITE).is_none()
    }) {
        canvas.set_shadow(shadow);
        canvas.fill_path(&path, Color::WHITE, FillRule::Winding);
        canvas.set_shadow(None);
    }
    canvas.clip_path(&path, FillRule::Winding);
    canvas.draw_pixmap(image, x, y, width, height);
    canvas.restore();
}

fn draw_inset_layer(
    canvas: &mut Canvas,
    image: tiny_skia::PixmapRef<'_>,
    rect: (f32, f32, f32, f32),
    radius: f32,
    shadow: Option<Shadow>,
    inset: f32,
) {
    if inset <= 0.0 {
        draw_unframed_image(canvas, image, rect, radius, shadow);
        return;
    }
    let (x, y, width, height) = rect;
    let Some(path) = rounded_rect_path(x, y, width, height, radius) else {
        return;
    };
    let fill = crate::render::color_detection::dominant_inset_color(
        image,
        crate::render::color_detection::ContentBounds::default(),
    )
    .map(|value| color::parse_or(&value, Color::WHITE))
    .or_else(|| shadow.map(|_| Color::WHITE));
    canvas.save();
    if let Some(fill) = fill {
        if shadow
            .is_none_or(|shadow| paint_rounded_shadow(canvas, rect, radius, shadow, fill).is_none())
        {
            canvas.set_shadow(shadow);
            canvas.fill_path(&path, fill, FillRule::Winding);
            canvas.set_shadow(None);
        }
    }
    canvas.clip_path(&path, FillRule::Winding);
    canvas.draw_pixmap(
        image,
        x + inset,
        y + inset,
        (width - inset * 2.0).max(0.0),
        (height - inset * 2.0).max(0.0),
    );
    canvas.restore();
}

fn compose_layered_wallpaper(
    primary: &Pixmap,
    wallpaper: &WallpaperSettings,
    layers: &[crate::editor::layers::ImageLayer],
    annotations: &[Annotation],
    frame_scale: f64,
    crop_offset: (f32, f32),
) -> Option<Pixmap> {
    let inset = wallpaper.layer_inset(true);
    let layout = crate::editor::layers::compute_with_inset(
        f64::from(primary.width()),
        f64::from(primary.height()),
        layers,
        wallpaper.spacing.max(0.0),
        inset,
    );
    let ((width, height), (offset_x, offset_y, _, _)) =
        crate::editor::wallpaper::layout(wallpaper, layout.width, layout.height, frame_scale, true);
    let (width, height) = (
        width.round().max(1.0) as u32,
        height.round().max(1.0) as u32,
    );
    let mut canvas = Canvas::new(width, height)?;
    paint_backdrop(&mut canvas, wallpaper, width as f32, height as f32);
    let radius = wallpaper.corners.max(0.0) as f32;
    let shadow = image_shadow(wallpaper.shadow);
    let rect = |rect: crate::editor::layers::LayerRect| {
        (
            (offset_x.round() + rect.x) as f32,
            (offset_y.round() + rect.y) as f32,
            rect.width as f32,
            rect.height as f32,
        )
    };
    let primary_rect = rect(layout.primary);
    draw_inset_layer(
        &mut canvas,
        primary.as_ref(),
        primary_rect,
        radius,
        shadow,
        inset as f32,
    );
    for (id, layer_rect) in layout.layers {
        let Some(source) = layers
            .iter()
            .find(|layer| layer.id == id)
            .and_then(|layer| gradient::load_image(&layer.image_url))
        else {
            continue;
        };
        draw_inset_layer(
            &mut canvas,
            source.as_ref(),
            rect(layer_rect),
            radius,
            shadow,
            inset as f32,
        );
    }
    canvas.translate(
        primary_rect.0 + inset as f32 - crop_offset.0,
        primary_rect.1 + inset as f32 - crop_offset.1,
    );
    annotation_renderer::draw_all_screenshot(&mut canvas, annotations, frame_scale);
    Some(canvas.into_pixmap())
}

/// The margin the balance option trims, in image pixels. The preview shifts
/// and clips by the same amounts so it shows what the export writes.
pub fn balance_bounds(
    image: &image::RgbaImage,
) -> Option<crate::render::color_detection::ContentBounds> {
    let pixmap = from_rgba(image)?;
    balance_bounds_pixmap(&pixmap)
}

fn balance_bounds_pixmap(pixmap: &Pixmap) -> Option<crate::render::color_detection::ContentBounds> {
    use crate::render::color_detection;

    let edge = color_detection::dominant_edge_color(pixmap.as_ref())?;
    let bounds = color_detection::content_bounds(pixmap.as_ref(), &edge)?;
    (bounds != color_detection::ContentBounds::default()).then_some(bounds)
}

/// Crops the uniform margin the balance option removes, or `None` when there
/// is nothing to trim.
fn balance_crop(
    image: &Pixmap,
    bounds: crate::render::color_detection::ContentBounds,
) -> Option<Pixmap> {
    let width = image.width().checked_sub(bounds.left + bounds.right)?;
    let height = image.height().checked_sub(bounds.top + bounds.bottom)?;
    if width == 0 || height == 0 {
        return None;
    }

    let mut cropped = Pixmap::new(width, height)?;
    let source = image.data();
    let target = cropped.data_mut();
    for row in 0..height {
        let from = (((row + bounds.top) * image.width() + bounds.left) * 4) as usize;
        let to = (row * width * 4) as usize;
        let span = (width * 4) as usize;
        target[to..to + span].copy_from_slice(&source[from..from + span]);
    }
    Some(cropped)
}

/// Draws the annotated image onto the wallpaper backdrop at export size.
fn compose_wallpaper(
    image: &Pixmap,
    wallpaper: &WallpaperSettings,
    frame_scale: f64,
    original_inset_color: Option<Option<String>>,
    has_layers: bool,
) -> Pixmap {
    let (image_width, image_height) = (image.width() as f64, image.height() as f64);
    let ((canvas_width, canvas_height), (offset_x, offset_y, _, _)) =
        crate::editor::wallpaper::layout(
            wallpaper,
            image_width,
            image_height,
            frame_scale,
            has_layers,
        );
    let (canvas_width, canvas_height) = (
        canvas_width.round().max(1.0) as u32,
        canvas_height.round().max(1.0) as u32,
    );

    let Some(mut canvas) = Canvas::new(canvas_width, canvas_height) else {
        return image.clone();
    };
    paint_backdrop(
        &mut canvas,
        wallpaper,
        canvas_width as f32,
        canvas_height as f32,
    );

    let radius = wallpaper.corners.max(0.0) as f32;
    let (offset_x, offset_y) = (offset_x.round() as f32, offset_y.round() as f32);

    // An inset lays the image on a band of its own dominant border colour, so
    // a screenshot with a light background does not float on the wallpaper.
    let inset = wallpaper.inset.max(0.0) as f32;
    let inset_color = (inset > 0.0)
        .then(|| {
            original_inset_color.unwrap_or_else(|| {
                crate::render::color_detection::dominant_inset_color(
                    image.as_ref(),
                    crate::render::color_detection::ContentBounds::default(),
                )
            })
        })
        .flatten();

    // A window frame replaces the plain rounded rect with its own chrome.
    if crate::render::window_frame::theme(&wallpaper.window_frame.style).is_some() {
        let frame_inset = if !has_layers && !wallpaper.balance {
            inset
        } else {
            0.0
        };
        crate::render::window_frame::render(
            &mut canvas,
            image.as_ref(),
            f64::from(offset_x - frame_inset),
            f64::from(offset_y - frame_inset),
            image_width,
            image_height,
            &wallpaper.window_frame.style,
            image_shadow_at_scale(wallpaper.shadow, frame_scale),
            frame_scale,
            f64::from(frame_inset),
            inset_color.as_deref(),
        );
        return canvas.into_pixmap();
    }

    if inset_color.is_none() {
        draw_unframed_image(
            &mut canvas,
            image.as_ref(),
            (offset_x, offset_y, image_width as f32, image_height as f32),
            radius,
            image_shadow(wallpaper.shadow),
        );
        return canvas.into_pixmap();
    }

    let (frame_x, frame_y) = match &inset_color {
        Some(_) => (offset_x - inset, offset_y - inset),
        None => (offset_x, offset_y),
    };
    let (frame_width, frame_height) = match &inset_color {
        Some(_) => (
            image_width as f32 + inset * 2.0,
            image_height as f32 + inset * 2.0,
        ),
        None => (image_width as f32, image_height as f32),
    };

    canvas.save();
    canvas.set_shadow(image_shadow(wallpaper.shadow));
    if let Some(path) = rounded_rect_path(frame_x, frame_y, frame_width, frame_height, radius) {
        // A shadow needs an opaque shape behind the image; with an inset that
        // shape is the band itself, and the image is clipped inside it.
        let fill = match &inset_color {
            Some(value) => Some(color::parse_or(
                value,
                Color::from_rgba8(255, 255, 255, 255),
            )),
            None if wallpaper.shadow > 0.0 => Some(Color::from_rgba8(255, 255, 255, 255)),
            None => None,
        };
        if let Some(fill) = fill {
            match image_shadow(wallpaper.shadow) {
                Some(shadow) => {
                    canvas.set_shadow(None);
                    if paint_rounded_shadow(
                        &mut canvas,
                        (frame_x, frame_y, frame_width, frame_height),
                        radius,
                        shadow,
                        fill,
                    )
                    .is_none()
                    {
                        canvas.set_shadow(Some(shadow));
                        canvas.fill_path(&path, fill, FillRule::Winding);
                    }
                }
                None => canvas.fill_path(&path, fill, FillRule::Winding),
            }
        }
        canvas.set_shadow(None);
        canvas.clip_path(&path, FillRule::Winding);
    }
    canvas.draw_pixmap(
        image.as_ref(),
        offset_x,
        offset_y,
        image_width as f32,
        image_height as f32,
    );
    canvas.restore();

    canvas.into_pixmap()
}

/// Port of `applyImageShadow` in `renderer/utils/wallpaper-render.ts`.
pub fn image_shadow(shadow: f64) -> Option<Shadow> {
    if shadow <= 0.0 {
        return None;
    }
    let alpha = (0.2 + (shadow / 100.0) * 0.3).clamp(0.0, 1.0) as f32;
    Some(Shadow {
        color: color::with_alpha(Color::from_rgba8(0, 0, 0, 255), alpha),
        blur: ((shadow / 100.0) * 50.0) as f32,
        offset_x: 0.0,
        offset_y: ((shadow / 100.0) * 15.0) as f32,
    })
}

fn image_shadow_at_scale(shadow: f64, scale: f64) -> Option<Shadow> {
    image_shadow(shadow).map(|mut shadow| {
        shadow.blur *= scale as f32;
        shadow.offset_y *= scale as f32;
        shadow
    })
}

fn paint_backdrop(canvas: &mut Canvas, wallpaper: &WallpaperSettings, width: f32, height: f32) {
    paint_background_source(canvas, wallpaper, width, height);

    // `backgroundBlur` is a percentage of a 50px radius, as in
    // `renderWallpaperComposite`.
    if wallpaper.has_background() && wallpaper.background_blur > 0.0 {
        let sigma = ((wallpaper.background_blur / 100.0) * 50.0) as f32;
        let mut blurred = canvas.pixmap().clone();
        crate::render::blur::blur(&mut blurred, sigma);
        canvas.draw_pixmap(blurred.as_ref(), 0.0, 0.0, width, height);
    }

    if wallpaper.has_background() {
        gradient::overlay_noise(canvas, width, height, wallpaper.noise, NOISE_SEED);
    }
}

/// The grain is deterministic so the preview and the exported file agree.
const NOISE_SEED: u64 = 0x9E37_79B9_7F4A_7C15;

fn paint_background_source(
    canvas: &mut Canvas,
    wallpaper: &WallpaperSettings,
    width: f32,
    height: f32,
) {
    if let Some(source) = wallpaper.background_image.as_deref() {
        if let Some(image) = gradient::load_image(source) {
            gradient::fill_image(canvas, image.as_ref(), width, height);
            return;
        }
    }
    let Some(preset) = wallpaper.gradient.as_ref() else {
        if wallpaper.background_image.is_some() {
            canvas.fill_all(Color::from_rgba8(24, 24, 27, 255));
        }
        return;
    };
    let option = gradient::GradientOption {
        id: preset.id.clone(),
        name: String::new(),
        colors: preset.colors.clone(),
        angle: preset.angle,
    };
    if option.is_renderable() {
        gradient::fill(canvas, &option, width, height);
        return;
    }
    let single = preset
        .colors
        .first()
        .map(|value| color::parse_or(value, Color::from_rgba8(24, 24, 27, 255)))
        .unwrap_or(Color::from_rgba8(24, 24, 27, 255));
    canvas.fill_all(single);
}

fn render_image(pixmap: &Pixmap) -> std::sync::Arc<gpui::RenderImage> {
    let mut buffer = to_rgba(pixmap);
    for pixel in buffer.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    std::sync::Arc::new(gpui::RenderImage::new(smallvec::smallvec![
        image::Frame::new(buffer)
    ]))
}

fn preview_edge_image(edge: crate::render::canvas::PreviewEdge) -> PreviewEdgeImage {
    PreviewEdgeImage {
        x: edge.x,
        y: edge.y,
        width: edge.pixels.width(),
        height: edge.pixels.height(),
        inset_x: 0,
        inset_y: 0,
        image: render_image(&edge.pixels),
    }
}

fn composite_tiles(
    composite: &Pixmap,
    stage_background: [u8; 3],
    side: u32,
) -> Option<Vec<PreviewEdgeImage>> {
    let mut tiles = Vec::new();
    for y in (0..composite.height()).step_by(side as usize) {
        for x in (0..composite.width()).step_by(side as usize) {
            let right = (x + side).min(composite.width());
            let bottom = (y + side).min(composite.height());
            let source_x = x.saturating_sub(1);
            let source_y = y.saturating_sub(1);
            let source_right = (right + 1).min(composite.width());
            let source_bottom = (bottom + 1).min(composite.height());
            let mut pixels = composite.clone_rect(tiny_skia::IntRect::from_xywh(
                source_x as i32,
                source_y as i32,
                source_right - source_x,
                source_bottom - source_y,
            )?)?;
            let mut paint = tiny_skia::Paint::default();
            paint.set_color_rgba8(
                stage_background[0],
                stage_background[1],
                stage_background[2],
                255,
            );
            paint.blend_mode = tiny_skia::BlendMode::DestinationOver;
            pixels.fill_rect(
                tiny_skia::Rect::from_xywh(0., 0., pixels.width() as f32, pixels.height() as f32)?,
                &paint,
                tiny_skia::Transform::identity(),
                None,
            );
            tiles.push(PreviewEdgeImage {
                x,
                y,
                width: right - x,
                height: bottom - y,
                inset_x: x - source_x,
                inset_y: y - source_y,
                image: render_image(&pixels),
            });
        }
    }
    Some(tiles)
}

/// Renders just the wallpaper backdrop at `width` x `height`, for the editor
/// preview to show behind the image.
#[allow(clippy::too_many_arguments)]
pub fn render_backdrop(
    wallpaper: &WallpaperSettings,
    width: u32,
    height: u32,
    image_width: f64,
    image_height: f64,
    inset_color: Option<String>,
    layers: &[crate::editor::layers::ImageLayer],
    base: Option<&image::DynamicImage>,
    frame_scale: f64,
    stage_background: [u8; 3],
) -> Option<(
    std::sync::Arc<gpui::RenderImage>,
    Vec<PreviewEdgeImage>,
    bool,
)> {
    if composites_wallpaper_effects(wallpaper, layers) {
        let primary = from_rgba(&base?.to_rgba8())?;
        let composite =
            compose_wallpaper(&primary, wallpaper, frame_scale, Some(inset_color), false);
        let tiles = if composite.width() > 16384 || composite.height() > 16384 {
            composite_tiles(&composite, stage_background, 8192)?
        } else {
            Vec::new()
        };
        return Some((render_image(&composite), tiles, true));
    }
    if wallpaper.supports_edge_preview(!layers.is_empty()) && !layers.is_empty() {
        let primary = from_rgba(&base?.to_rgba8())?;
        let cropped = wallpaper
            .balance
            .then(|| balance_bounds_pixmap(&primary))
            .flatten()
            .and_then(|bounds| balance_crop(&primary, bounds));
        let primary = cropped.as_ref().unwrap_or(&primary);
        let composite =
            compose_layered_wallpaper(primary, wallpaper, layers, &[], frame_scale, (0.0, 0.0))?;
        let tiles = if composite.width() > 16384 || composite.height() > 16384 {
            composite_tiles(&composite, stage_background, 8192)?
        } else {
            Vec::new()
        };
        return Some((render_image(&composite), tiles, true));
    }
    let mut canvas = Canvas::new(width.max(1), height.max(1))?;
    if !wallpaper.has_background() && !composites_primary_backdrop(wallpaper, layers) {
        canvas.fill_all(Color::from_rgba8(
            stage_background[0],
            stage_background[1],
            stage_background[2],
            255,
        ));
    }
    paint_backdrop(&mut canvas, wallpaper, width as f32, height as f32);
    let edge_preview = wallpaper.supports_edge_preview(!layers.is_empty());
    if edge_preview && wallpaper.has_background() {
        let mut builder = tiny_skia::PathBuilder::new();
        builder.push_rect(tiny_skia::Rect::from_xywh(
            0.0,
            0.0,
            width as f32,
            height as f32,
        )?);
        canvas.fill_path_blended(
            &builder.finish()?,
            Color::from_rgba8(
                stage_background[0],
                stage_background[1],
                stage_background[2],
                255,
            ),
            FillRule::Winding,
            tiny_skia::BlendMode::DestinationOver,
        );
    }

    if edge_preview && crate::render::window_frame::theme(&wallpaper.window_frame.style).is_none() {
        if let Some(shadow) = image_shadow(wallpaper.shadow) {
            let (_, (offset_x, offset_y, _, _)) = crate::editor::wallpaper::layout(
                wallpaper,
                image_width,
                image_height,
                frame_scale,
                !layers.is_empty(),
            );
            paint_rounded_shadow(
                &mut canvas,
                (
                    offset_x.round() as f32,
                    offset_y.round() as f32,
                    image_width as f32,
                    image_height as f32,
                ),
                wallpaper.corners.max(0.0) as f32,
                shadow,
                Color::WHITE,
            )?;
        }
    }

    // Attached images are static, so they belong to the backdrop; only the
    // primary capture and its annotations stay live.
    if !layers.is_empty() {
        let (_, (offset_x, offset_y, _, _)) = crate::editor::wallpaper::layout(
            wallpaper,
            image_width,
            image_height,
            frame_scale,
            !layers.is_empty(),
        );
        let layout = crate::editor::layers::compute(
            image_width,
            image_height,
            layers,
            wallpaper.spacing.max(0.0),
        );
        for (id, rect) in &layout.layers {
            let Some(source) = layers
                .iter()
                .find(|layer| &layer.id == id)
                .and_then(|layer| gradient::load_image(&layer.image_url))
            else {
                continue;
            };
            canvas.draw_pixmap(
                source.as_ref(),
                (offset_x + rect.x) as f32,
                (offset_y + rect.y) as f32,
                rect.width as f32,
                rect.height as f32,
            );
        }
    }

    // The inset band belongs to the backdrop too; the live canvas draws the
    // image on top of it.
    if let Some(color_value) = inset_color.filter(|_| wallpaper.inset > 0.0) {
        let (_, (offset_x, offset_y, _, _)) = crate::editor::wallpaper::layout(
            wallpaper,
            image_width,
            image_height,
            frame_scale,
            !layers.is_empty(),
        );
        let inset = wallpaper.inset as f32;
        if let Some(path) = rounded_rect_path(
            offset_x.round() as f32 - inset,
            offset_y.round() as f32 - inset,
            image_width as f32 + inset * 2.0,
            image_height as f32 + inset * 2.0,
            wallpaper.corners.max(0.0) as f32,
        ) {
            canvas.save();
            let fill = color::parse_or(&color_value, Color::from_rgba8(255, 255, 255, 255));
            if let Some(shadow) = image_shadow(wallpaper.shadow) {
                if paint_rounded_shadow(
                    &mut canvas,
                    (
                        offset_x.round() as f32 - inset,
                        offset_y.round() as f32 - inset,
                        image_width as f32 + inset * 2.0,
                        image_height as f32 + inset * 2.0,
                    ),
                    wallpaper.corners.max(0.0) as f32,
                    shadow,
                    fill,
                )
                .is_none()
                {
                    canvas.set_shadow(Some(shadow));
                    canvas.fill_path(&path, fill, FillRule::Winding);
                }
            } else {
                canvas.fill_path(&path, fill, FillRule::Winding);
            }
            canvas.restore();
        }
    }

    // The frame's chrome belongs to the backdrop; the live canvas draws the
    // image and its annotations inside it.
    let mut edge_tiles = None;
    if crate::render::window_frame::theme(&wallpaper.window_frame.style).is_some() {
        let (_, (offset_x, offset_y, _, _)) = crate::editor::wallpaper::layout(
            wallpaper,
            image_width,
            image_height,
            frame_scale,
            !layers.is_empty(),
        );
        crate::render::window_frame::render_chrome(
            &mut canvas,
            offset_x.round(),
            offset_y.round(),
            image_width,
            image_height,
            &wallpaper.window_frame.style,
            image_shadow_at_scale(wallpaper.shadow, frame_scale),
            frame_scale,
            0.0,
            None,
            None,
            !edge_preview,
        );
        if edge_preview {
            edge_tiles = crate::render::window_frame::preview_edges(
                canvas.pixmap(),
                offset_x.round(),
                offset_y.round(),
                image_width,
                image_height,
                &wallpaper.window_frame.style,
                frame_scale,
            );
            if edge_tiles.is_none() {
                crate::render::window_frame::render_outline(
                    &mut canvas,
                    offset_x.round(),
                    offset_y.round(),
                    image_width,
                    image_height,
                    &wallpaper.window_frame.style,
                    frame_scale,
                );
            }
        }
    } else if edge_preview {
        let (_, (offset_x, offset_y, _, _)) = crate::editor::wallpaper::layout(
            wallpaper,
            image_width,
            image_height,
            frame_scale,
            !layers.is_empty(),
        );
        edge_tiles = crate::render::canvas::preview_clip_edges(
            canvas.pixmap(),
            offset_x.round(),
            offset_y.round(),
            image_width,
            image_height,
            0.0,
            wallpaper.corners,
            None,
        );
    }
    Some((
        render_image(canvas.pixmap()),
        edge_tiles
            .into_iter()
            .flatten()
            .map(preview_edge_image)
            .collect(),
        false,
    ))
}

/// Converts an `image` RGBA buffer into a premultiplied pixmap.
pub fn from_rgba(source: &RgbaImage) -> Option<Pixmap> {
    let mut pixmap = Pixmap::new(source.width().max(1), source.height().max(1))?;
    let target = pixmap.data_mut();
    for (index, pixel) in source.pixels().enumerate() {
        let [r, g, b, a] = pixel.0;
        let alpha = a as u32;
        let offset = index * 4;
        target[offset] = (r as u32 * alpha / 255) as u8;
        target[offset + 1] = (g as u32 * alpha / 255) as u8;
        target[offset + 2] = (b as u32 * alpha / 255) as u8;
        target[offset + 3] = a;
    }
    Some(pixmap)
}

/// Converts a premultiplied pixmap back into straight-alpha RGBA.
pub fn to_rgba(source: &Pixmap) -> RgbaImage {
    let mut target = RgbaImage::new(source.width(), source.height());
    for (pixel, output) in source
        .data()
        .as_chunks::<4>()
        .0
        .iter()
        .zip(target.as_chunks_mut::<4>().0.iter_mut())
    {
        let alpha = pixel[3];
        *output = if alpha == 0 {
            [0, 0, 0, 0]
        } else if alpha == 255 {
            *pixel
        } else {
            let unpremultiply = |value: u8| -> u8 {
                ((value as u32 * 255 + alpha as u32 / 2) / alpha as u32).min(255) as u8
            };
            [
                unpremultiply(pixel[0]),
                unpremultiply(pixel[1]),
                unpremultiply(pixel[2]),
                alpha,
            ]
        };
    }
    target
}

/// Encodes an RGBA buffer as PNG bytes.
pub fn encode_png(canvas: &RgbaImage) -> anyhow::Result<Vec<u8>> {
    let mut bytes = std::io::Cursor::new(Vec::new());
    canvas.write_to(&mut bytes, image::ImageFormat::Png)?;
    Ok(bytes.into_inner())
}

/// Applies a redaction to a standalone RGBA region. The editor uses this to
/// rasterize the patch it shows under a committed redaction, so the preview
/// pixels are the ones the export writes.
pub fn redact_region(
    canvas: &mut RgbaImage,
    rect: (i64, i64, i64, i64),
    style: &str,
    intensity: f64,
    native_scale: f64,
) {
    let (x, y, width, height) = rect;
    let Some(pixmap) = from_rgba(canvas) else {
        return;
    };
    let mut surface = Canvas::from_pixmap(pixmap);
    annotation_renderer::draw_screenshot(
        &mut surface,
        &Annotation::Redact {
            id: String::new(),
            x: x as f64,
            y: y as f64,
            width: width as f64,
            height: height as f64,
            style: style.to_string(),
            intensity,
        },
        native_scale,
    );
    *canvas = to_rgba(surface.pixmap());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::wallpaper::preset;

    #[test]
    fn tiled_preview_shadow_matches_the_full_rasterizer() {
        let cases = (0..40u32).map(|index| {
            (
                1 + (index * 103) % 900,
                1 + (index * 137) % 600,
                ((index * 47) % 401) as f32,
                [1.0, 10.0, 50.0, 100.0, 200.0, 300.0][index as usize % 6],
                (index * 11) % 60,
            )
        });
        for (width, height, radius, strength, padding) in [
            (640, 360, 24.0, 50.0, 40),
            (1170, 452, 279.0, 100.0, 13),
            (15_534, 10, 0.0, 300.0, 13),
        ]
        .into_iter()
        .chain(cases)
        {
            let shadow = image_shadow(strength).unwrap();
            let mut expected = Canvas::new(width + padding * 2, height + padding * 2).unwrap();
            let mut actual = Canvas::new(width + padding * 2, height + padding * 2).unwrap();
            let background = Color::from_rgba8(36, 104, 172, 255);
            expected.fill_all(background);
            actual.fill_all(background);
            let path = rounded_rect_path(
                padding as f32,
                padding as f32,
                width as f32,
                height as f32,
                radius,
            )
            .unwrap();
            expected.set_shadow(Some(shadow));
            expected.fill_path(&path, Color::WHITE, FillRule::Winding);
            paint_rounded_shadow(
                &mut actual,
                (padding as f32, padding as f32, width as f32, height as f32),
                radius,
                shadow,
                Color::WHITE,
            )
            .unwrap();
            assert!(
                expected
                    .pixmap()
                    .data()
                    .iter()
                    .zip(actual.pixmap().data())
                    .all(|(expected, actual)| expected.abs_diff(*actual) <= 2),
                "{width}x{height}, radius {radius}, shadow {strength}, padding {padding}"
            );
        }
    }

    #[test]
    fn large_screenshot_shadows_survive_export_and_png_reopen() {
        for (horizontal, radius) in [(true, 0.0), (false, 0.0), (true, 24.0), (false, 24.0)] {
            let settings = WallpaperSettings {
                padding: 40.0,
                corners: radius,
                shadow: 50.0,
                ..WallpaperSettings::default()
            };
            let (width, height) = if horizontal {
                (17_000, 100)
            } else {
                (100, 17_000)
            };
            let (reference_width, reference_height) = if horizontal {
                (1_200, 100)
            } else {
                (100, 1_200)
            };
            let make_image = |width, height| {
                image::DynamicImage::ImageRgba8(RgbaImage::from_fn(width, height, |x, y| {
                    image::Rgba([
                        200,
                        30,
                        60,
                        if (x / 20 + y / 20) % 3 == 0 { 128 } else { 255 },
                    ])
                }))
            };
            let mut reference = Canvas::new(reference_width + 80, reference_height + 80).unwrap();
            let path = rounded_rect_path(
                40.0,
                40.0,
                reference_width as f32,
                reference_height as f32,
                radius as f32,
            )
            .unwrap();
            reference.set_shadow(image_shadow(settings.shadow));
            reference.fill_path(&path, Color::WHITE, FillRule::Winding);
            let reference = to_rgba(reference.pixmap());
            let base = make_image(width, height);
            let output = compose(Some(&base), width, height, &[], &settings, 1.0);
            let mut png = std::io::Cursor::new(Vec::new());
            image::DynamicImage::ImageRgba8(output.clone())
                .write_to(&mut png, image::ImageFormat::Png)
                .unwrap();
            let reopened = image::load_from_memory(png.get_ref()).unwrap().to_rgba8();
            assert_eq!(reopened, output);
            let (_, tiles, composite) = render_backdrop(
                &settings,
                output.width(),
                output.height(),
                f64::from(width),
                f64::from(height),
                None,
                &[],
                Some(&base),
                1.0,
                [24, 24, 27],
            )
            .unwrap();
            assert!(composite);
            assert!(!tiles.is_empty());
            let mut preview = RgbaImage::new(output.width(), output.height());
            for tile in tiles {
                let mut pixels = tile.image.as_bytes(0).unwrap().to_vec();
                for pixel in pixels.as_chunks_mut::<4>().0 {
                    pixel.swap(0, 2);
                }
                let size = tile.image.size(0);
                let pixels =
                    RgbaImage::from_raw(size.width.into(), size.height.into(), pixels).unwrap();
                let interior = image::imageops::crop_imm(
                    &pixels,
                    tile.inset_x,
                    tile.inset_y,
                    tile.width,
                    tile.height,
                );
                image::imageops::replace(
                    &mut preview,
                    &interior.to_image(),
                    tile.x.into(),
                    tile.y.into(),
                );
            }
            let mut expected = RgbaImage::from_pixel(
                output.width(),
                output.height(),
                image::Rgba([24, 24, 27, 255]),
            );
            image::imageops::overlay(&mut expected, &reopened, 0, 0);
            for y in 0..output.height() {
                for x in 0..output.width() {
                    let near_left = (40..40 + radius as u32 + 2).contains(&x);
                    let near_right = (40 + width - radius as u32 - 2..40 + width).contains(&x);
                    let near_top = (40..40 + radius as u32 + 2).contains(&y);
                    let near_bottom = (40 + height - radius as u32 - 2..40 + height).contains(&y);
                    let tolerance =
                        if radius > 0.0 && (near_left || near_right) && (near_top || near_bottom) {
                            3
                        } else {
                            2
                        };
                    assert!(
                        preview
                            .get_pixel(x, y)
                            .0
                            .iter()
                            .zip(expected.get_pixel(x, y).0)
                            .all(|(actual, expected)| actual.abs_diff(expected) <= tolerance),
                        "preview differs at {x},{y}, horizontal {horizontal}, radius {radius}: {:?} vs {:?}",
                        preview.get_pixel(x, y).0,
                        expected.get_pixel(x, y).0
                    );
                }
            }
            let center = if horizontal {
                (output.width() / 2, 35)
            } else {
                (35, output.height() / 2)
            };
            assert!(output.get_pixel(center.0, center.1).0[3] > 0);
            for cross in 0..40 {
                let (actual, expected) = if horizontal {
                    ((output.width() / 2, cross), (reference.width() / 2, cross))
                } else {
                    (
                        (cross, output.height() / 2),
                        (cross, reference.height() / 2),
                    )
                };
                assert!(
                    output
                        .get_pixel(actual.0, actual.1)
                        .0
                        .iter()
                        .zip(reference.get_pixel(expected.0, expected.1).0)
                        .all(|(actual, expected)| actual.abs_diff(expected) <= 2),
                    "middle shadow differs, horizontal {horizontal}, radius {radius}, {cross}"
                );
            }
            for cross in 0..180 {
                for distance in 0..200 {
                    let points = if horizontal {
                        [
                            ((distance, cross), (distance, cross)),
                            (
                                (output.width() - 1 - distance, cross),
                                (reference.width() - 1 - distance, cross),
                            ),
                        ]
                    } else {
                        [
                            ((cross, distance), (cross, distance)),
                            (
                                (cross, output.height() - 1 - distance),
                                (cross, reference.height() - 1 - distance),
                            ),
                        ]
                    };
                    for (actual, expected) in points {
                        if (40..40 + width).contains(&actual.0)
                            && (40..40 + height).contains(&actual.1)
                        {
                            continue;
                        }
                        assert!(
                            output
                                .get_pixel(actual.0, actual.1)
                                .0
                                .iter()
                                .zip(reference.get_pixel(expected.0, expected.1).0)
                                .all(|(actual, expected)| actual.abs_diff(expected) <= 2),
                            "horizontal {horizontal}, {actual:?}, {expected:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn large_inset_and_framed_shadows_match_smaller_exports() {
        for (style, inset, corners, shadow, horizontal) in [
            ("none", 6.0, 0.0, 50.0, true),
            ("none", 6.0, 0.0, 50.0, false),
            ("none", 6.0, 24.0, 50.0, true),
            ("none", 6.0, 24.0, 50.0, false),
            ("windows-dark", 0.0, 0.0, 50.0, true),
            ("windows-dark", 0.0, 0.0, 50.0, false),
            ("macos-light", 0.0, 0.0, 50.0, true),
            ("macos-light", 0.0, 0.0, 50.0, false),
            ("windows-dark", 0.0, 0.0, 0.0, true),
            ("windows-dark", 0.0, 0.0, 0.0, false),
        ] {
            let mut settings = WallpaperSettings {
                padding: 40.0,
                shadow,
                inset,
                corners,
                ..WallpaperSettings::default()
            };
            settings.window_frame.style = style.into();
            let (width, height) = if horizontal {
                (17_000, 100)
            } else {
                (100, 17_000)
            };
            let (reference_width, reference_height) = if horizontal {
                (1_200, 100)
            } else {
                (100, 1_200)
            };
            let make_image = |width, height| {
                image::DynamicImage::ImageRgba8(RgbaImage::from_pixel(
                    width,
                    height,
                    image::Rgba([200, 30, 60, 255]),
                ))
            };
            let source = make_image(width, height);
            let output = compose(Some(&source), width, height, &[], &settings, 1.0);
            let title_height = crate::render::window_frame::title_bar_height(style, 1.0) as u32;
            let mut reference = Canvas::new(
                reference_width + 80 + inset as u32 * 2,
                reference_height + title_height + 80 + inset as u32 * 2,
            )
            .unwrap();
            let radius = if style == "none" {
                corners as f32
            } else {
                crate::render::window_frame::corner_radius(style) as f32
            };
            let path = rounded_rect_path(
                40.0,
                40.0,
                reference_width as f32 + inset as f32 * 2.0,
                (reference_height + title_height) as f32 + inset as f32 * 2.0,
                radius,
            )
            .unwrap();
            reference.set_shadow(image_shadow(settings.shadow));
            reference.fill_path(&path, Color::WHITE, FillRule::Winding);
            let reference = to_rgba(reference.pixmap());
            let shadow_y = 35;
            if shadow > 0.0 {
                assert!(reference.get_pixel(reference.width() / 2, shadow_y).0[3] > 0);
                assert!(
                    output.get_pixel(output.width() / 2, shadow_y).0[3] > 0,
                    "missing shadow for {style}, inset {inset}, horizontal {horizontal}"
                );
            }
            for y in 0..shadow_y + 1 {
                assert!(
                    output
                        .get_pixel(output.width() / 2, y)
                        .0
                        .iter()
                        .zip(reference.get_pixel(reference.width() / 2, y).0)
                        .all(|(actual, expected)| actual.abs_diff(expected) <= 2),
                    "shadow differs for {style}, inset {inset}, horizontal {horizontal}, y {y}"
                );
            }
            let mut png = std::io::Cursor::new(Vec::new());
            image::DynamicImage::ImageRgba8(output.clone())
                .write_to(&mut png, image::ImageFormat::Png)
                .unwrap();
            assert_eq!(
                image::load_from_memory(png.get_ref()).unwrap().to_rgba8(),
                output
            );
            let inset_color = if inset > 0.0 {
                let pixels = from_rgba(&source.to_rgba8()).unwrap();
                crate::render::color_detection::dominant_inset_color(
                    pixels.as_ref(),
                    crate::render::color_detection::ContentBounds::default(),
                )
            } else {
                None
            };
            assert_eq!(inset_color.is_some(), inset > 0.0);
            let (preview, tiles, composite) = render_backdrop(
                &settings,
                output.width(),
                output.height(),
                f64::from(width),
                f64::from(height),
                inset_color,
                &[],
                Some(&source),
                1.0,
                [24, 24, 27],
            )
            .unwrap();
            let mut pixels = preview.as_bytes(0).unwrap().to_vec();
            for pixel in pixels.as_chunks_mut::<4>().0 {
                pixel.swap(0, 2);
            }
            let pixels = RgbaImage::from_raw(output.width(), output.height(), pixels).unwrap();
            let mut actual = RgbaImage::from_pixel(
                output.width(),
                output.height(),
                image::Rgba([24, 24, 27, 255]),
            );
            image::imageops::overlay(&mut actual, &pixels, 0, 0);
            let mut expected = RgbaImage::from_pixel(
                output.width(),
                output.height(),
                image::Rgba([24, 24, 27, 255]),
            );
            image::imageops::overlay(&mut expected, &output, 0, 0);
            assert!(composite);
            assert!(!tiles.is_empty());
            for (x, y) in [
                (40, 40),
                (45, 45),
                (55, 42),
                (42, 55),
                (output.width() - 41, 40),
                (40, output.height() - 41),
                (output.width() - 41, output.height() - 41),
            ] {
                let tile = tiles
                    .iter()
                    .find(|tile| {
                        (tile.x..tile.x + tile.width).contains(&x)
                            && (tile.y..tile.y + tile.height).contains(&y)
                    })
                    .unwrap();
                let bytes = tile.image.as_bytes(0).unwrap();
                let stride = u32::from(tile.image.size(0).width);
                let offset = (((y - tile.y + tile.inset_y) * stride + x - tile.x + tile.inset_x)
                    * 4) as usize;
                let pixel = [
                    bytes[offset + 2],
                    bytes[offset + 1],
                    bytes[offset],
                    bytes[offset + 3],
                ];
                assert!(pixel
                    .iter()
                    .zip(expected.get_pixel(x, y).0)
                    .all(|(actual, expected)| actual.abs_diff(expected) <= 2));
            }
            for y in 0..shadow_y + 1 {
                let x = output.width() / 2;
                assert!(
                    actual
                        .get_pixel(x, y)
                        .0
                        .iter()
                        .zip(expected.get_pixel(x, y).0)
                        .all(|(actual, expected)| actual.abs_diff(expected) <= 2),
                    "preview shadow differs for {style}, inset {inset}, horizontal {horizontal}, y {y}"
                );
            }
        }
    }

    #[test]
    fn converts_premultiplied_pixels_across_all_alpha_values() {
        let source = RgbaImage::from_fn(256, 256, |x, y| {
            image::Rgba([x as u8, (255 - x) as u8, (x ^ y) as u8, y as u8])
        });
        let pixmap = from_rgba(&source).unwrap();
        let restored = to_rgba(&pixmap);
        for (original, actual) in source.pixels().zip(restored.pixels()) {
            let alpha = u32::from(original.0[3]);
            let mut expected = [0, 0, 0, original.0[3]];
            if alpha != 0 {
                for (channel, value) in original.0[..3].iter().enumerate() {
                    let premultiplied = u32::from(*value) * alpha / 255;
                    expected[channel] = ((premultiplied * 255 + alpha / 2) / alpha).min(255) as u8;
                }
            }
            assert_eq!(actual.0, expected);
        }
    }

    #[test]
    fn round_trips_rgba_through_a_pixmap() {
        let mut source = RgbaImage::new(2, 1);
        source.put_pixel(0, 0, image::Rgba([200, 100, 50, 255]));
        source.put_pixel(1, 0, image::Rgba([10, 20, 30, 128]));
        let pixmap = from_rgba(&source).expect("pixmap");
        let restored = to_rgba(&pixmap);
        assert_eq!(restored.get_pixel(0, 0).0, [200, 100, 50, 255]);
        let second = restored.get_pixel(1, 0).0;
        assert_eq!(second[3], 128);
        assert!((second[0] as i32 - 10).abs() <= 2, "{second:?}");
    }

    #[test]
    fn wallpaper_grows_the_export_canvas() {
        let mut settings = WallpaperSettings::default();
        settings.set_gradient(preset("ocean"));
        settings.padding = 20.0;

        let base = image::DynamicImage::ImageRgba8(RgbaImage::from_pixel(
            40,
            30,
            image::Rgba([255, 0, 0, 255]),
        ));
        let composed = compose(Some(&base), 40, 30, &[], &settings, 1.0);
        assert_eq!(composed.dimensions(), (80, 70));
        assert_eq!(composed.get_pixel(40, 35).0, [255, 0, 0, 255]);
        assert_ne!(composed.get_pixel(2, 2).0, [255, 0, 0, 255]);
    }

    #[test]
    fn translucent_wallpaper_keeps_its_alpha_in_the_saved_image() {
        use base64::Engine as _;

        let background = image::DynamicImage::ImageRgba8(RgbaImage::from_pixel(
            64,
            64,
            image::Rgba([32, 96, 160, 128]),
        ));
        let mut bytes = std::io::Cursor::new(Vec::new());
        background
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        let source = format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(bytes.get_ref())
        );
        let base = image::DynamicImage::ImageRgba8(RgbaImage::from_pixel(
            100,
            80,
            image::Rgba([240, 80, 64, 255]),
        ));
        for scale in [1.25, 2.] {
            for image_background in [false, true] {
                let mut settings = WallpaperSettings::default();
                if image_background {
                    settings.set_background_image(Some(source.clone()));
                } else {
                    settings.set_gradient(Some(crate::editor::wallpaper::GradientOption {
                        id: "alpha-background".into(),
                        colors: vec!["#2060a080".into(), "#2060a080".into()],
                        angle: 0.,
                    }));
                }
                settings.padding = 20.;
                let saved = compose(Some(&base), 100, 80, &[], &settings.for_scale(scale), scale);
                assert_eq!(saved.get_pixel(0, 0).0[3], 128);
            }
        }
    }

    #[test]
    fn an_inactive_wallpaper_leaves_the_image_alone() {
        let base = image::DynamicImage::ImageRgba8(RgbaImage::from_pixel(
            8,
            8,
            image::Rgba([1, 2, 3, 255]),
        ));
        let composed = compose(Some(&base), 8, 8, &[], &WallpaperSettings::default(), 1.0);
        assert_eq!(composed.dimensions(), (8, 8));
        assert_eq!(composed.get_pixel(4, 4).0, [1, 2, 3, 255]);
    }

    #[test]
    fn rounded_corners_clip_the_image() {
        let mut settings = WallpaperSettings::default();
        settings.set_gradient(preset("slate"));
        settings.padding = 10.0;
        settings.corners = 30.0;

        let base = image::DynamicImage::ImageRgba8(RgbaImage::from_pixel(
            60,
            60,
            image::Rgba([255, 0, 0, 255]),
        ));
        let composed = compose(Some(&base), 60, 60, &[], &settings, 1.0);
        // The image's own corner is cut away, its centre is not.
        assert_ne!(composed.get_pixel(11, 11).0[0], 255);
        assert_eq!(composed.get_pixel(40, 40).0, [255, 0, 0, 255]);
    }

    #[test]
    fn an_attached_layer_widens_the_composition() {
        use crate::editor::layers::{Edge, ImageLayer};

        let attached = crate::util::test_paths::unique_temp("poratake-layer-test.png");
        let side = image::DynamicImage::ImageRgba8(RgbaImage::from_pixel(
            50,
            50,
            image::Rgba([0, 0, 255, 255]),
        ));
        side.save(&attached).expect("write");

        let base = image::DynamicImage::ImageRgba8(RgbaImage::from_pixel(
            100,
            100,
            image::Rgba([255, 0, 0, 255]),
        ));
        let composed = compose_with_layers(
            Some(&base),
            100,
            100,
            &[],
            &WallpaperSettings::default(),
            &[ImageLayer {
                id: "a".into(),
                image_url: attached.to_string_lossy().to_string(),
                natural_width: 50.0,
                natural_height: 50.0,
                edge: Edge::Right,
            }],
            1.0,
        );

        // The side image is scaled to the capture's height and placed beside it.
        assert_eq!(composed.dimensions(), (200, 100));
        assert_eq!(composed.get_pixel(50, 50).0, [255, 0, 0, 255]);
        assert_eq!(composed.get_pixel(150, 50).0, [0, 0, 255, 255]);
        let _ = std::fs::remove_file(&attached);
    }

    #[test]
    fn attached_images_each_keep_their_own_inset_without_padding() {
        use crate::editor::layers::{Edge, ImageLayer};

        let attached = crate::util::test_paths::unique_temp("poratake-inset-layer.png");
        let mut side = RgbaImage::from_pixel(60, 60, image::Rgba([20, 40, 200, 255]));
        side.put_pixel(30, 30, image::Rgba([240, 220, 20, 255]));
        side.put_pixel(0, 40, image::Rgba([240, 20, 40, 0]));
        side.save(&attached).unwrap();
        let mut base = RgbaImage::from_pixel(60, 60, image::Rgba([20, 180, 40, 255]));
        base.put_pixel(30, 30, image::Rgba([240, 20, 40, 255]));
        let base = image::DynamicImage::ImageRgba8(base);
        let settings = WallpaperSettings {
            inset: 6.0,
            ..WallpaperSettings::default()
        };
        let annotations = [
            Annotation::Rectangle {
                id: "inset-annotation".into(),
                x: 5.0,
                y: 5.0,
                width: 10.0,
                height: 10.0,
                stroke: "#ff00ff".into(),
                stroke_width: 1.0,
                fill: Some("#ff00ff".into()),
            },
            Annotation::Rectangle {
                id: "inset-edge-annotation".into(),
                x: 55.0,
                y: 20.0,
                width: 25.0,
                height: 10.0,
                stroke: "#ff00ff".into(),
                stroke_width: 1.0,
                fill: Some("#ff00ff".into()),
            },
        ];
        let output = compose_with_layers(
            Some(&base),
            60,
            60,
            &annotations,
            &settings,
            &[ImageLayer {
                id: "right".into(),
                image_url: attached.to_string_lossy().into_owned(),
                natural_width: 60.0,
                natural_height: 60.0,
                edge: Edge::Right,
            }],
            1.0,
        );
        assert_eq!(output.dimensions(), (144, 72));
        assert_eq!(output.get_pixel(2, 36).0, [20, 180, 40, 255]);
        assert_eq!(output.get_pixel(74, 50).0, [20, 40, 200, 255]);
        assert_eq!(output.get_pixel(36, 36).0, [240, 20, 40, 255]);
        assert_eq!(output.get_pixel(108, 36).0, [240, 220, 20, 255]);
        assert_eq!(output.get_pixel(78, 46).0, [20, 40, 200, 255]);
        assert_eq!(output.get_pixel(16, 16).0, [255, 0, 255, 255]);
        assert_eq!(output.get_pixel(8, 8).0, [20, 180, 40, 255]);
        assert_eq!(output.get_pixel(80, 31).0, [255, 0, 255, 255]);
        for (edge, dimensions, primary, layer) in [
            (Edge::Left, (144, 72), (74, 36), (2, 36)),
            (Edge::Top, (72, 144), (36, 74), (36, 2)),
            (Edge::Bottom, (72, 144), (36, 2), (36, 74)),
        ] {
            let output = compose_with_layers(
                Some(&base),
                60,
                60,
                &[],
                &settings,
                &[ImageLayer {
                    id: "edge".into(),
                    image_url: attached.to_string_lossy().into_owned(),
                    natural_width: 60.0,
                    natural_height: 60.0,
                    edge,
                }],
                1.0,
            );
            assert_eq!(output.dimensions(), dimensions);
            assert_eq!(output.get_pixel(primary.0, primary.1).0, [20, 180, 40, 255]);
            assert_eq!(output.get_pixel(layer.0, layer.1).0, [20, 40, 200, 255]);
        }
        let _ = std::fs::remove_file(&attached);
    }

    #[test]
    fn attached_images_each_keep_their_own_rounded_corners() {
        use crate::editor::layers::{Edge, ImageLayer};
        use crate::editor::wallpaper::GradientOption;

        let attached = crate::util::test_paths::unique_temp("poratake-rounded-layer.png");
        RgbaImage::from_pixel(60, 60, image::Rgba([0, 0, 255, 255]))
            .save(&attached)
            .unwrap();
        let base = image::DynamicImage::ImageRgba8(RgbaImage::from_pixel(
            60,
            60,
            image::Rgba([255, 0, 0, 255]),
        ));
        let settings = WallpaperSettings {
            gradient: Some(GradientOption {
                id: "solid".into(),
                colors: vec!["#2468ac".into()],
                angle: 0.0,
            }),
            padding: 10.0,
            spacing: 20.0,
            corners: 20.0,
            ..WallpaperSettings::default()
        };
        let output = compose_with_layers(
            Some(&base),
            60,
            60,
            &[],
            &settings,
            &[ImageLayer {
                id: "right".into(),
                image_url: attached.to_string_lossy().to_string(),
                natural_width: 60.0,
                natural_height: 60.0,
                edge: Edge::Right,
            }],
            1.0,
        );
        let _ = std::fs::remove_file(&attached);
        assert_eq!(output.dimensions(), (160, 80));
        assert_eq!(output.get_pixel(40, 40).0, [255, 0, 0, 255]);
        assert_eq!(output.get_pixel(120, 40).0, [0, 0, 255, 255]);
        assert_eq!(output.get_pixel(69, 10).0, [36, 104, 172, 255]);
        assert_eq!(output.get_pixel(90, 10).0, [36, 104, 172, 255]);
    }

    #[test]
    fn annotations_remain_above_attached_images_and_shadows() {
        use crate::editor::layers::{Edge, ImageLayer};
        use crate::editor::wallpaper::GradientOption;

        let attached = crate::util::test_paths::unique_temp("poratake-annotation-layer.png");
        RgbaImage::from_pixel(60, 60, image::Rgba([0, 0, 255, 255]))
            .save(&attached)
            .unwrap();
        let base = image::DynamicImage::ImageRgba8(RgbaImage::from_pixel(
            60,
            60,
            image::Rgba([255, 0, 0, 255]),
        ));
        let settings = WallpaperSettings {
            gradient: Some(GradientOption {
                id: "solid".into(),
                colors: vec!["#2468ac".into()],
                angle: 0.0,
            }),
            padding: 20.0,
            shadow: 50.0,
            ..WallpaperSettings::default()
        };
        let annotations = [Annotation::Rectangle {
            id: "edge".into(),
            x: 50.0,
            y: 20.0,
            width: 20.0,
            height: 20.0,
            stroke: "#ff00ff".into(),
            stroke_width: 1.0,
            fill: Some("#ff00ff".into()),
        }];
        let layer = ImageLayer {
            id: "right".into(),
            image_url: attached.to_string_lossy().to_string(),
            natural_width: 60.0,
            natural_height: 60.0,
            edge: Edge::Right,
        };
        let output = compose_with_layers(
            Some(&base),
            60,
            60,
            &annotations,
            &settings,
            std::slice::from_ref(&layer),
            1.0,
        );
        let plain = compose_with_layers(
            Some(&base),
            60,
            60,
            &annotations,
            &WallpaperSettings::default(),
            std::slice::from_ref(&layer),
            1.0,
        );
        let _ = std::fs::remove_file(&attached);
        assert_eq!(output.get_pixel(78, 50).0, [255, 0, 255, 255]);
        assert_eq!(output.get_pixel(85, 50).0, [255, 0, 255, 255]);
        assert_eq!(output.get_pixel(95, 50).0, [0, 0, 255, 255]);
        assert_eq!(plain.get_pixel(58, 30).0, [255, 0, 255, 255]);
        assert_eq!(plain.get_pixel(65, 30).0, [255, 0, 255, 255]);
        assert_eq!(plain.get_pixel(75, 30).0, [0, 0, 255, 255]);
    }

    #[test]
    fn composite_tiles_reconstruct_a_two_dimensional_translucent_image() {
        let source = RgbaImage::from_fn(19, 19, |x, y| {
            image::Rgba([
                (x * 11) as u8,
                (y * 11) as u8,
                96,
                if (x + y) % 2 == 0 { 128 } else { 255 },
            ])
        });
        let composite = from_rgba(&source).unwrap();
        let tiles = composite_tiles(&composite, [240, 240, 240], 8).unwrap();
        assert_eq!(tiles.len(), 9);
        assert_eq!(
            (
                tiles[4].x,
                tiles[4].y,
                tiles[4].width,
                tiles[4].height,
                tiles[4].inset_x,
                tiles[4].inset_y
            ),
            (8, 8, 8, 8, 1, 1)
        );
        let mut expected = composite.clone();
        let mut paint = tiny_skia::Paint::default();
        paint.set_color_rgba8(240, 240, 240, 255);
        paint.blend_mode = tiny_skia::BlendMode::DestinationOver;
        expected.fill_rect(
            tiny_skia::Rect::from_xywh(0., 0., 19., 19.).unwrap(),
            &paint,
            tiny_skia::Transform::identity(),
            None,
        );
        let expected = to_rgba(&expected);
        let mut assembled = RgbaImage::new(19, 19);
        for tile in tiles {
            let bytes = tile.image.as_bytes(0).unwrap();
            let stride = u32::from(tile.image.size(0).width) as usize * 4;
            for y in 0..tile.height {
                for x in 0..tile.width {
                    let index =
                        (y + tile.inset_y) as usize * stride + (x + tile.inset_x) as usize * 4;
                    assembled.put_pixel(
                        tile.x + x,
                        tile.y + y,
                        image::Rgba([
                            bytes[index + 2],
                            bytes[index + 1],
                            bytes[index],
                            bytes[index + 3],
                        ]),
                    );
                }
            }
        }
        assert_eq!(assembled, expected);
    }

    #[test]
    fn zero_padding_keeps_corner_and_shadow_effects() {
        let base = image::DynamicImage::ImageRgba8(RgbaImage::from_pixel(
            60,
            60,
            image::Rgba([255, 0, 0, 255]),
        ));
        let settings = WallpaperSettings {
            corners: 20.0,
            ..WallpaperSettings::default()
        };
        let rounded = compose(Some(&base), 60, 60, &[], &settings, 1.0);
        assert_eq!(rounded.dimensions(), (60, 60));
        assert_eq!(rounded.get_pixel(0, 0).0, [0, 0, 0, 0]);
        assert_eq!(rounded.get_pixel(30, 30).0, [255, 0, 0, 255]);

        let attached = crate::util::test_paths::unique_temp("poratake-zero-padding-layer.png");
        RgbaImage::from_pixel(60, 60, image::Rgba([0, 0, 255, 255]))
            .save(&attached)
            .unwrap();
        let layered = compose_with_layers(
            Some(&base),
            60,
            60,
            &[],
            &settings,
            &[crate::editor::layers::ImageLayer {
                id: "right".into(),
                image_url: attached.to_string_lossy().into_owned(),
                natural_width: 60.0,
                natural_height: 60.0,
                edge: crate::editor::layers::Edge::Right,
            }],
            1.0,
        );
        let _ = std::fs::remove_file(&attached);
        assert_eq!(layered.dimensions(), (120, 60));
        assert_eq!(layered.get_pixel(59, 0).0, [0, 0, 0, 0]);
        assert_eq!(layered.get_pixel(60, 0).0, [0, 0, 0, 0]);
        assert_eq!(layered.get_pixel(90, 30).0, [0, 0, 255, 255]);

        let transparent = image::DynamicImage::ImageRgba8(RgbaImage::from_pixel(
            60,
            60,
            image::Rgba([0, 0, 0, 0]),
        ));
        let settings = WallpaperSettings {
            shadow: 50.0,
            ..WallpaperSettings::default()
        };
        let shadowed = compose(Some(&transparent), 60, 60, &[], &settings, 1.0);
        assert_eq!(shadowed.dimensions(), (60, 60));
        assert_eq!(shadowed.get_pixel(30, 30).0, [255, 255, 255, 255]);
    }

    #[test]
    fn an_inset_lays_the_image_on_a_band_of_its_own_edge_colour() {
        let mut base = RgbaImage::from_pixel(60, 60, image::Rgba([10, 200, 10, 255]));
        // A distinct centre keeps the edge colour unambiguous.
        base.put_pixel(30, 30, image::Rgba([255, 255, 255, 255]));
        let base = image::DynamicImage::ImageRgba8(base);
        for (padding, background) in [(0, false), (20, true)] {
            let mut settings = WallpaperSettings::default();
            if background {
                settings.set_gradient(preset("slate"));
            }
            settings.padding = padding as f64;
            settings.inset = 6.0;
            let composed = compose(Some(&base), 60, 60, &[], &settings, 1.0);
            assert_eq!(composed.dimensions(), (72 + padding * 2, 72 + padding * 2));
            assert_eq!(
                composed.get_pixel(padding + 3, padding + 30).0,
                [10, 200, 10, 255]
            );
            assert_eq!(
                composed.get_pixel(padding + 36, padding + 36).0,
                [255, 255, 255, 255]
            );
        }
    }

    #[test]
    fn rounded_inset_surrounds_the_image_with_or_without_padding() {
        let base = image::DynamicImage::ImageRgba8(RgbaImage::from_fn(60, 60, |x, y| {
            if (20..40).contains(&x) && (20..40).contains(&y) {
                image::Rgba([255, 255, 255, 255])
            } else {
                image::Rgba([10, 200, 10, 255])
            }
        }));
        for (padding, shadow) in [(0, 0.0), (20, 50.0)] {
            let settings = WallpaperSettings {
                padding: f64::from(padding),
                inset: 6.0,
                corners: 20.0,
                shadow,
                ..WallpaperSettings::default()
            };
            let composed = compose(Some(&base), 60, 60, &[], &settings, 1.0);
            assert_eq!(composed.dimensions(), (72 + padding * 2, 72 + padding * 2));
            assert_eq!(
                composed.get_pixel(padding + 3, padding + 36).0,
                [10, 200, 10, 255]
            );
            assert_eq!(
                composed.get_pixel(padding + 36, padding + 36).0,
                [255, 255, 255, 255]
            );
            if padding == 0 {
                assert_eq!(composed.get_pixel(0, 0).0[3], 0);
            } else {
                let shadow_alpha = composed.get_pixel(padding + 36, padding - 5).0[3];
                assert!((1..255).contains(&shadow_alpha));
            }
        }
    }

    #[test]
    fn balanced_inset_uses_the_source_border_even_with_annotations() {
        let base = image::DynamicImage::ImageRgba8(RgbaImage::from_fn(200, 160, |x, y| {
            if (40..160).contains(&x) && (30..130).contains(&y) {
                image::Rgba([64, 112, 192, 255])
            } else if (5..195).contains(&x) && (5..155).contains(&y) {
                image::Rgba([230, 230, 230, 255])
            } else {
                image::Rgba([255, 255, 255, 255])
            }
        }));
        let settings = WallpaperSettings {
            balance: true,
            padding: 20.0,
            inset: 8.0,
            ..WallpaperSettings::default()
        };
        let annotation = Annotation::Rectangle {
            id: "border".into(),
            x: 30.0,
            y: 20.0,
            width: 140.0,
            height: 120.0,
            stroke: "#ff00ff".into(),
            stroke_width: 3.0,
            fill: None,
        };
        let composed = compose(Some(&base), 200, 160, &[annotation], &settings, 1.0);
        assert_eq!(composed.dimensions(), (196, 176));
        assert_eq!(composed.get_pixel(23, 88).0, [230, 230, 230, 255]);
    }

    #[test]
    fn a_window_frame_preserves_the_inset_around_the_image() {
        let base = image::DynamicImage::ImageRgba8(RgbaImage::from_fn(100, 60, |x, y| {
            if (30..70).contains(&x) && (20..40).contains(&y) {
                image::Rgba([240, 60, 80, 255])
            } else {
                image::Rgba([40, 120, 180, 255])
            }
        }));
        for (padding, style) in [(0, "macos-light"), (10, "windows-dark")] {
            let mut settings = WallpaperSettings {
                padding: f64::from(padding),
                inset: 6.0,
                ..WallpaperSettings::default()
            };
            settings.window_frame.style = style.into();
            let composed = compose(Some(&base), 100, 60, &[], &settings, 1.0);
            assert_eq!(
                composed.dimensions(),
                (112 + padding * 2, 100 + padding * 2)
            );
            assert_eq!(
                composed.get_pixel(padding + 3, padding + 58).0,
                [40, 120, 180, 255]
            );
            assert_eq!(
                composed.get_pixel(padding + 56, padding + 64).0,
                [240, 60, 80, 255]
            );
        }
        let transparent = image::DynamicImage::ImageRgba8(RgbaImage::from_fn(100, 60, |x, y| {
            if (30..70).contains(&x) && (20..40).contains(&y) {
                image::Rgba([240, 60, 80, 255])
            } else {
                image::Rgba([0, 0, 0, 0])
            }
        }));
        for (shadow, band) in [(0.0, [0, 0, 0, 0]), (50.0, [18, 18, 18, 255])] {
            let mut settings = WallpaperSettings {
                padding: 10.0,
                inset: 6.0,
                shadow,
                ..WallpaperSettings::default()
            };
            settings.window_frame.style = "windows-dark".into();
            let composed = compose(Some(&transparent), 100, 60, &[], &settings, 1.0);
            assert_eq!(composed.dimensions(), (132, 120));
            assert_eq!(composed.get_pixel(13, 68).0, band);
            assert_eq!(composed.get_pixel(66, 74).0, [240, 60, 80, 255]);
        }
    }

    #[test]
    fn a_window_frame_grows_the_export_and_paints_its_chrome() {
        let mut settings = WallpaperSettings::default();
        settings.set_gradient(preset("slate"));
        settings.padding = 10.0;
        settings.window_frame.style = "windows-dark".into();

        let base = image::DynamicImage::ImageRgba8(RgbaImage::from_pixel(
            100,
            60,
            image::Rgba([255, 0, 0, 255]),
        ));
        let composed = compose(Some(&base), 100, 60, &[], &settings, 1.0);
        // 28px of title bar sits between the padding and the image.
        assert_eq!(composed.dimensions(), (120, 108));
        assert_ne!(composed.get_pixel(60, 20).0, [255, 0, 0, 255]);
        assert_eq!(composed.get_pixel(60, 60).0, [255, 0, 0, 255]);
    }

    #[test]
    fn a_blackout_redaction_paints_the_whole_region() {
        let mut canvas = RgbaImage::from_pixel(8, 8, image::Rgba([255, 255, 255, 255]));
        redact_region(&mut canvas, (2, 2, 4, 4), "blackout", 5.0, 1.0);
        assert_eq!(canvas.get_pixel(3, 3).0, [0, 0, 0, 255]);
        assert_eq!(canvas.get_pixel(0, 0).0, [255, 255, 255, 255]);
    }

    #[test]
    fn overlapping_annotations_follow_electron_export_order() {
        let base = image::DynamicImage::ImageRgba8(RgbaImage::from_pixel(
            100,
            80,
            image::Rgba([64, 128, 192, 255]),
        ));
        let shape = Annotation::Rectangle {
            id: "shape".into(),
            x: 30.,
            y: 20.,
            width: 24.,
            height: 24.,
            stroke: "#ff00ff".into(),
            stroke_width: 1.,
            fill: Some("#ff00ff".into()),
        };
        let blur = Annotation::Redact {
            id: "blur".into(),
            x: 20.,
            y: 15.,
            width: 40.,
            height: 40.,
            style: "blur".into(),
            intensity: 2.,
        };
        let blackout = Annotation::Redact {
            id: "blackout".into(),
            x: 32.,
            y: 22.,
            width: 20.,
            height: 20.,
            style: "blackout".into(),
            intensity: 2.,
        };
        let highlight = Annotation::Highlight {
            id: "highlight".into(),
            points: vec![25., 30., 55., 30.],
            fill: "#ffff00".into(),
            opacity: 0.5,
            stroke_width: 10.,
        };
        for (annotations, expected, corner) in [
            (vec![shape.clone(), blur.clone()], [255, 0, 255, 255], None),
            (vec![blur, shape.clone()], [255, 0, 255, 255], None),
            (
                vec![shape.clone(), blackout.clone()],
                [0, 0, 0, 255],
                Some([169, 0, 169, 255]),
            ),
            (
                vec![blackout.clone(), shape.clone()],
                [255, 0, 255, 255],
                Some([255, 0, 255, 255]),
            ),
            (
                vec![highlight.clone(), shape.clone()],
                [255, 0, 127, 255],
                None,
            ),
            (vec![shape, highlight.clone()], [255, 0, 127, 255], None),
            (
                vec![highlight.clone(), blackout.clone()],
                [0, 0, 0, 255],
                None,
            ),
            (vec![blackout, highlight], [0, 0, 0, 255], None),
        ] {
            let output = compose(
                Some(&base),
                100,
                80,
                &annotations,
                &WallpaperSettings::default(),
                1.,
            );
            let actual = output.get_pixel(40, 30).0;
            assert!(
                actual.iter().zip(expected).all(|(a, e)| a.abs_diff(e) <= 2),
                "{annotations:?}: {actual:?} vs {expected:?}"
            );
            if let Some(expected) = corner {
                let actual = output.get_pixel(32, 22).0;
                assert!(
                    actual
                        .iter()
                        .zip(expected)
                        .all(|(a, e)| a.abs_diff(e) <= 24),
                    "{annotations:?}: corner {actual:?} vs {expected:?}"
                );
            }
        }
    }

    #[test]
    fn redactions_sample_the_composited_wallpaper_and_inset() {
        let base = image::DynamicImage::ImageRgba8(RgbaImage::from_fn(100, 80, |x, y| {
            if (30..70).contains(&x) && (20..60).contains(&y) {
                image::Rgba([220, 70, 90, 128])
            } else {
                image::Rgba([40, 120, 180, 255])
            }
        }));
        for (background, inset, x) in [(true, 0., 32.), (false, 8., 32.), (true, 8., -5.)] {
            let mut settings = WallpaperSettings::default();
            settings.set_gradient(background.then(|| preset("ocean").unwrap()));
            settings.padding = 12.;
            settings.inset = inset;
            let annotations = [
                Annotation::Rectangle {
                    id: "before".into(),
                    x: 37.,
                    y: 26.,
                    width: 16.,
                    height: 16.,
                    stroke: "#ff00ff".into(),
                    stroke_width: 1.,
                    fill: Some("#ff00ff".into()),
                },
                Annotation::Redact {
                    id: "blur".into(),
                    x,
                    y: 22.,
                    width: 40.,
                    height: 30.,
                    style: "blur".into(),
                    intensity: 2.,
                },
            ];
            let unannotated = compose(Some(&base), 100, 80, &[], &settings, 1.);
            let mut expected = Canvas::from_pixmap(from_rgba(&unannotated).unwrap());
            let (_, (offset_x, offset_y, _, _)) =
                crate::editor::wallpaper::layout(&settings, 100., 80., 1., false);
            expected.translate(offset_x.round() as f32, offset_y.round() as f32);
            annotation_renderer::draw_all_screenshot(&mut expected, &annotations, 1.);
            let expected = to_rgba(expected.pixmap());
            let actual = compose(Some(&base), 100, 80, &annotations, &settings, 1.);
            let changed = actual
                .pixels()
                .zip(expected.pixels())
                .filter(|(actual, expected)| actual != expected)
                .count();
            assert_eq!(changed, 0, "background {background}, inset {inset}, x {x}");
        }
    }

    #[test]
    fn screenshot_blur_matches_electron_over_wallpaper_and_inset() {
        use crate::editor::wallpaper::GradientOption;

        let base = image::DynamicImage::ImageRgba8(RgbaImage::from_fn(100, 80, |x, y| {
            if (30..70).contains(&x) && (20..60).contains(&y) {
                image::Rgba([220, 70, 90, 128])
            } else {
                image::Rgba([40, 120, 180, 255])
            }
        }));
        let cases = [
            (
                true,
                0.,
                32.,
                [
                    (32, [96, 99, 150, 255]),
                    (40, [121, 90, 135, 255]),
                    (50, [127, 87, 131, 255]),
                    (71, [77, 106, 159, 255]),
                ],
            ),
            (
                false,
                8.,
                32.,
                [
                    (32, [98, 104, 152, 255]),
                    (40, [122, 97, 139, 255]),
                    (50, [129, 95, 135, 255]),
                    (71, [78, 109, 161, 255]),
                ],
            ),
            (
                true,
                8.,
                -5.,
                [
                    (-5, [38, 115, 178, 255]),
                    (0, [39, 118, 179, 255]),
                    (26, [70, 112, 165, 255]),
                    (34, [105, 103, 148, 255]),
                ],
            ),
        ];
        for (background, inset, x, samples) in cases {
            let mut settings = WallpaperSettings::default();
            settings.set_gradient(background.then(|| GradientOption {
                id: "solid".into(),
                colors: vec!["#2468ac".into()],
                angle: 0.,
            }));
            settings.padding = 12.;
            settings.inset = inset;
            let annotation = Annotation::Redact {
                id: "blur".into(),
                x,
                y: 22.,
                width: 40.,
                height: 30.,
                style: "blur".into(),
                intensity: 2.,
            };
            let actual = compose(Some(&base), 100, 80, &[annotation], &settings, 1.);
            let offset = (12. + inset) as i32;
            for (sample_x, expected) in samples {
                let pixel = actual
                    .get_pixel((offset + sample_x) as u32, (offset + 37) as u32)
                    .0;
                assert!(
                    pixel.iter().zip(expected).all(|(a, e)| a.abs_diff(e) <= 3),
                    "background {background}, inset {inset}, x {sample_x}: {pixel:?} vs {expected:?}"
                );
            }
        }
    }

    #[test]
    fn removing_a_redaction_does_not_clip_other_annotations() {
        let base = image::DynamicImage::ImageRgba8(RgbaImage::from_pixel(
            100,
            80,
            image::Rgba([40, 120, 180, 255]),
        ));
        let mut settings = WallpaperSettings::default();
        settings.set_gradient(preset("ocean"));
        settings.padding = 12.;
        let shape = Annotation::Rectangle {
            id: "edge".into(),
            x: -6.,
            y: 24.,
            width: 12.,
            height: 12.,
            stroke: "#ff00ff".into(),
            stroke_width: 1.,
            fill: Some("#ff00ff".into()),
        };
        let redact = Annotation::Redact {
            id: "redact".into(),
            x: 60.,
            y: 20.,
            width: 10.,
            height: 10.,
            style: "blackout".into(),
            intensity: 2.,
        };
        let without = compose(
            Some(&base),
            100,
            80,
            std::slice::from_ref(&shape),
            &settings,
            1.,
        );
        let with = compose(Some(&base), 100, 80, &[shape, redact], &settings, 1.);
        assert_eq!(without.get_pixel(9, 42).0, [255, 0, 255, 255]);
        assert_eq!(with.get_pixel(9, 42), without.get_pixel(9, 42));
    }

    #[test]
    fn screenshot_pixelation_averages_fixed_size_blocks() {
        let base = image::DynamicImage::ImageRgba8(RgbaImage::from_fn(30, 1, |x, _| {
            let red = match x {
                0..=7 => 10,
                8..=14 => 20,
                15..=22 => 30,
                _ => 40,
            };
            image::Rgba([red, 0, 0, 255])
        }));
        let annotation = Annotation::Redact {
            id: "pixelate".into(),
            x: 0.,
            y: 0.,
            width: 30.,
            height: 1.,
            style: "pixelate".into(),
            intensity: 5.,
        };
        let actual = compose(
            Some(&base),
            30,
            1,
            &[annotation],
            &WallpaperSettings::default(),
            1.,
        );
        for x in 0..30 {
            let expected = match x {
                0..=7 => 10,
                8..=15 => 21,
                16..=23 => 31,
                _ => 40,
            };
            assert_eq!(actual.get_pixel(x, 0).0, [expected, 0, 0, 255], "x={x}");
        }
    }

    #[test]
    fn screenshot_pixelation_matches_electron_with_alpha_clipping_and_scale() {
        let base = image::DynamicImage::ImageRgba8(RgbaImage::from_fn(30, 2, |x, y| {
            let red = match x {
                0..=7 => 10,
                8..=14 => 20,
                15..=22 => 30,
                _ => 40,
            };
            image::Rgba([
                red,
                if y == 0 { 100 } else { 200 },
                60,
                if x % 3 == 0 { 128 } else { 255 },
            ])
        }));
        for (start, scale, samples) in [
            (
                0.,
                1.,
                vec![
                    (0, [10, 150, 60, 207]),
                    (8, [21, 150, 60, 207]),
                    (16, [31, 150, 59, 223]),
                    (24, [40, 150, 60, 213]),
                ],
            ),
            (
                -3.,
                1.,
                vec![
                    (0, [10, 150, 60, 207]),
                    (8, [21, 150, 60, 207]),
                    (16, [31, 150, 59, 223]),
                    (24, [40, 150, 60, 213]),
                    (27, [40, 100, 60, 128]),
                    (29, [40, 100, 60, 255]),
                ],
            ),
            (
                0.,
                2.,
                vec![(0, [16, 150, 60, 207]), (16, [35, 150, 61, 219])],
            ),
        ] {
            let annotation = Annotation::Redact {
                id: "pixelate".into(),
                x: start,
                y: 0.,
                width: 30.,
                height: 2.,
                style: "pixelate".into(),
                intensity: 5.,
            };
            let actual = compose(
                Some(&base),
                30,
                2,
                &[annotation],
                &WallpaperSettings::default(),
                scale,
            );
            for (x, expected) in samples {
                let pixel = actual.get_pixel(x, 0).0;
                assert!(
                    pixel.iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 2),
                    "x={x}, start={start}, scale={scale}: {pixel:?} vs {expected:?}"
                );
            }
        }
    }

    #[test]
    fn shadow_strength_follows_the_renderer_curve() {
        assert!(image_shadow(0.0).is_none());
        let shadow = image_shadow(100.0).expect("shadow");
        assert_eq!(shadow.blur, 50.0);
        assert_eq!(shadow.offset_y, 15.0);
        assert!((shadow.color.alpha() - 0.5).abs() < 0.01);
    }
}
