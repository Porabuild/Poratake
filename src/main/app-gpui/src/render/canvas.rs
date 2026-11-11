//! A Canvas2D-shaped surface over `tiny_skia`. The video composition and the
//! image editor's export are direct ports of renderer code written against
//! `CanvasRenderingContext2D`, so they need the same primitives: a transform
//! and clip stack, `globalAlpha`, and shadowed fills. Keeping that shape here
//! means both ports read like the TypeScript they mirror and share one
//! rasterizer, which is what makes "preview equals export" hold.

use std::sync::Arc;

use tiny_skia::{
    BlendMode, Color, FillRule, FilterQuality, Mask, Paint, Path, PathBuilder, Pixmap, PixmapPaint,
    PixmapRef, Rect, Shader, Stroke, Transform,
};

use crate::render::blur;

pub const MAX_SHADOW_SURFACE_SIZE: u32 = 16_384;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shadow {
    pub color: Color,
    pub blur: f32,
    pub offset_x: f32,
    pub offset_y: f32,
}

#[derive(Clone)]
struct State {
    transform: Transform,
    clip: Option<Arc<Mask>>,
    alpha: f32,
    shadow: Option<Shadow>,
}

impl State {
    fn root() -> Self {
        Self {
            transform: Transform::identity(),
            clip: None,
            alpha: 1.0,
            shadow: None,
        }
    }
}

pub struct Canvas {
    pixmap: Pixmap,
    state: State,
    stack: Vec<State>,
    shadow_scale: f32,
}

impl Canvas {
    pub fn new(width: u32, height: u32) -> Option<Self> {
        Some(Self::from_pixmap(Pixmap::new(width.max(1), height.max(1))?))
    }

    pub fn from_pixmap(pixmap: Pixmap) -> Self {
        Self {
            pixmap,
            state: State::root(),
            stack: Vec::new(),
            shadow_scale: 1.0,
        }
    }

    pub fn width(&self) -> u32 {
        self.pixmap.width()
    }

    pub fn height(&self) -> u32 {
        self.pixmap.height()
    }

    pub fn pixmap(&self) -> &Pixmap {
        &self.pixmap
    }

    pub fn into_pixmap(self) -> Pixmap {
        self.pixmap
    }

    pub fn transform(&self) -> Transform {
        self.state.transform
    }

    pub fn global_alpha(&self) -> f32 {
        self.state.alpha
    }

    pub fn set_global_alpha(&mut self, alpha: f32) {
        self.state.alpha = alpha.clamp(0.0, 1.0);
    }

    pub fn set_shadow(&mut self, shadow: Option<Shadow>) {
        self.state.shadow = shadow;
    }

    pub fn set_shadow_scale(&mut self, scale: f32) {
        self.shadow_scale = scale.max(f32::EPSILON);
    }

    pub fn save(&mut self) {
        self.stack.push(self.state.clone());
    }

    pub fn restore(&mut self) {
        if let Some(state) = self.stack.pop() {
            self.state = state;
        }
    }

    pub fn translate(&mut self, x: f32, y: f32) {
        self.state.transform = self.state.transform.pre_translate(x, y);
    }

    pub fn scale(&mut self, x: f32, y: f32) {
        self.state.transform = self.state.transform.pre_scale(x, y);
    }

    pub fn rotate(&mut self, radians: f32) {
        self.state.transform = self
            .state
            .transform
            .pre_concat(Transform::from_rotate(radians.to_degrees()));
    }

    /// The average of the transform's axis scales, used to pick a raster size
    /// for glyphs and to convert stroke widths into device units.
    pub fn device_scale(&self) -> f32 {
        let transform = self.state.transform;
        let sx = (transform.sx * transform.sx + transform.ky * transform.ky).sqrt();
        let sy = (transform.kx * transform.kx + transform.sy * transform.sy).sqrt();
        ((sx + sy) / 2.0).max(f32::EPSILON)
    }

    pub fn clear(&mut self) {
        self.pixmap.fill(Color::TRANSPARENT);
    }

    pub fn fill_all(&mut self, color: Color) {
        self.pixmap.fill(color);
    }

    /// Intersects the clip with `path`, in current user space.
    pub fn clip_path(&mut self, path: &Path, fill_rule: FillRule) {
        let Some(mut mask) = Mask::new(self.pixmap.width(), self.pixmap.height()) else {
            return;
        };
        match &self.state.clip {
            Some(existing) => {
                mask.data_mut().copy_from_slice(existing.data());
                mask.intersect_path(path, fill_rule, true, self.state.transform);
            }
            None => mask.fill_path(path, fill_rule, true, self.state.transform),
        }
        self.state.clip = Some(Arc::new(mask));
    }

    pub fn clip_rect(&mut self, rect: Rect) {
        let mut builder = PathBuilder::new();
        builder.push_rect(rect);
        if let Some(path) = builder.finish() {
            self.clip_path(&path, FillRule::Winding);
        }
    }

    pub fn fill_path(&mut self, path: &Path, color: Color, fill_rule: FillRule) {
        self.fill_path_blended(path, color, fill_rule, BlendMode::SourceOver);
    }

    pub fn fill_path_blended(
        &mut self,
        path: &Path,
        color: Color,
        fill_rule: FillRule,
        blend: BlendMode,
    ) {
        if let Some(shadow) = self.state.shadow {
            self.draw_shadow_of(
                path_device_bounds(path, self.state.transform),
                shadow,
                |canvas| {
                    canvas.fill_path_raw(
                        path,
                        Color::from_rgba8(0, 0, 0, 255),
                        fill_rule,
                        BlendMode::SourceOver,
                    );
                },
            );
        }
        self.fill_path_raw(path, color, fill_rule, blend);
    }

    fn fill_path_raw(&mut self, path: &Path, color: Color, fill_rule: FillRule, blend: BlendMode) {
        let color = crate::render::color::with_alpha(color, self.state.alpha);
        if color.alpha() <= 0.0 {
            return;
        }
        let paint = paint_for(Shader::SolidColor(color), blend);
        self.pixmap.fill_path(
            path,
            &paint,
            fill_rule,
            self.state.transform,
            self.state.clip.as_deref(),
        );
    }

    pub fn fill_path_shader(&mut self, path: &Path, shader: Shader<'_>, fill_rule: FillRule) {
        let paint = paint_for(shader, BlendMode::SourceOver);
        self.pixmap.fill_path(
            path,
            &paint,
            fill_rule,
            self.state.transform,
            self.state.clip.as_deref(),
        );
    }

    pub fn fill_rect(&mut self, rect: Rect, color: Color) {
        let mut builder = PathBuilder::new();
        builder.push_rect(rect);
        if let Some(path) = builder.finish() {
            self.fill_path(&path, color, FillRule::Winding);
        }
    }

    pub fn stroke_path(&mut self, path: &Path, color: Color, stroke: &Stroke) {
        if let Some(shadow) = self.state.shadow {
            let bounds = grow(
                path_device_bounds(path, self.state.transform),
                stroke.width * self.device_scale(),
            );
            self.draw_shadow_of(bounds, shadow, |canvas| {
                canvas.stroke_path_raw(path, Color::from_rgba8(0, 0, 0, 255), stroke);
            });
        }
        self.stroke_path_raw(path, color, stroke);
    }

    fn stroke_path_raw(&mut self, path: &Path, color: Color, stroke: &Stroke) {
        let color = crate::render::color::with_alpha(color, self.state.alpha);
        if color.alpha() <= 0.0 || stroke.width <= 0.0 {
            return;
        }
        let paint = paint_for(Shader::SolidColor(color), BlendMode::SourceOver);
        self.pixmap.stroke_path(
            path,
            &paint,
            stroke,
            self.state.transform,
            self.state.clip.as_deref(),
        );
    }

    /// Draws `source` with its top-left at the user-space origin, scaled to
    /// `width` x `height` — the `drawImage(image, x, y, w, h)` overload after
    /// the caller has translated to `x, y`.
    pub fn draw_pixmap(&mut self, source: PixmapRef<'_>, x: f32, y: f32, width: f32, height: f32) {
        if source.width() == 0 || source.height() == 0 || width <= 0.0 || height <= 0.0 {
            return;
        }
        let placement = Transform::from_translate(x, y).pre_scale(
            width / source.width() as f32,
            height / source.height() as f32,
        );
        self.draw_pixmap_transformed(source, placement);
    }

    /// Draws `source` under `placement`, which maps source pixels into the
    /// current user space.
    pub fn draw_pixmap_transformed(&mut self, source: PixmapRef<'_>, placement: Transform) {
        let combined = self.state.transform.pre_concat(placement);
        if let Some(shadow) = self.state.shadow {
            let bounds = rect_device_bounds(
                Rect::from_xywh(0.0, 0.0, source.width() as f32, source.height() as f32),
                combined,
            );
            self.draw_shadow_of(bounds, shadow, |canvas| {
                canvas.draw_pixmap_raw(source, placement, 1.0);
            });
        }
        self.draw_pixmap_raw(source, placement, self.state.alpha);
    }

    fn draw_pixmap_raw(&mut self, source: PixmapRef<'_>, placement: Transform, alpha: f32) {
        if alpha <= 0.0 {
            return;
        }
        let paint = PixmapPaint {
            opacity: alpha,
            blend_mode: BlendMode::SourceOver,
            quality: FilterQuality::Bilinear,
        };
        self.pixmap.draw_pixmap(
            0,
            0,
            source,
            &paint,
            self.state.transform.pre_concat(placement),
            self.state.clip.as_deref(),
        );
    }

    /// Draws `source` scaled into the given rect with an explicit blend mode,
    /// which the noise overlay needs.
    pub fn draw_pixmap_blended(
        &mut self,
        source: PixmapRef<'_>,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        blend: BlendMode,
    ) {
        if source.width() == 0 || source.height() == 0 || width <= 0.0 || height <= 0.0 {
            return;
        }
        let placement = Transform::from_translate(x, y).pre_scale(
            width / source.width() as f32,
            height / source.height() as f32,
        );
        let paint = PixmapPaint {
            opacity: self.state.alpha,
            blend_mode: blend,
            quality: FilterQuality::Bilinear,
        };
        self.pixmap.draw_pixmap(
            0,
            0,
            source,
            &paint,
            self.state.transform.pre_concat(placement),
            self.state.clip.as_deref(),
        );
    }

    /// Draws `source` with nearest-neighbour sampling — the
    /// `imageSmoothingEnabled = false` path the pixelate redaction uses.
    pub fn draw_pixmap_nearest(
        &mut self,
        source: PixmapRef<'_>,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    ) {
        if source.width() == 0 || source.height() == 0 || width <= 0.0 || height <= 0.0 {
            return;
        }
        let placement = Transform::from_translate(x, y).pre_scale(
            width / source.width() as f32,
            height / source.height() as f32,
        );
        let paint = PixmapPaint {
            opacity: self.state.alpha,
            blend_mode: BlendMode::SourceOver,
            quality: FilterQuality::Nearest,
        };
        self.pixmap.draw_pixmap(
            0,
            0,
            source,
            &paint,
            self.state.transform.pre_concat(placement),
            self.state.clip.as_deref(),
        );
    }

    /// Copies a device-space region out of the surface, which the blur and
    /// pixelate redactions read back the way the renderer reads its canvas.
    pub fn read_region(&self, x: i32, y: i32, width: u32, height: u32) -> Option<Pixmap> {
        if width == 0 || height == 0 {
            return None;
        }
        let mut region = Pixmap::new(width, height)?;
        let source_width = self.pixmap.width() as i64;
        let source_height = self.pixmap.height() as i64;
        let source = self.pixmap.data();
        let target = region.data_mut();
        for row in 0..height as i64 {
            let source_y = y as i64 + row;
            if source_y < 0 || source_y >= source_height {
                continue;
            }
            for column in 0..width as i64 {
                let source_x = x as i64 + column;
                if source_x < 0 || source_x >= source_width {
                    continue;
                }
                let from = ((source_y * source_width + source_x) * 4) as usize;
                let to = ((row * width as i64 + column) * 4) as usize;
                target[to..to + 4].copy_from_slice(&source[from..from + 4]);
            }
        }
        Some(region)
    }

    /// Renders `draw` into an offscreen the size of `bounds` plus blur spread,
    /// blurs its alpha, tints it and composites it at the shadow offset. The
    /// offscreen is bounded by the shape, so a shadow costs the shape's area
    /// rather than the whole frame.
    fn draw_shadow_of(
        &mut self,
        bounds: Option<Rect>,
        shadow: Shadow,
        draw: impl FnOnce(&mut Canvas),
    ) {
        let Some(bounds) = bounds else {
            return;
        };
        if shadow.color.alpha() <= 0.0 {
            return;
        }
        let sigma = blur::sigma_for_shadow_blur(shadow.blur * self.shadow_scale);
        let pad = (blur::box_radius(sigma) as f32 * 3.0).ceil() + 2.0;
        let left = (bounds.left() - pad).floor();
        let top = (bounds.top() - pad).floor();
        let width = (bounds.width() + pad * 2.0).ceil() as u32;
        let height = (bounds.height() + pad * 2.0).ceil() as u32;
        if width == 0
            || height == 0
            || width > MAX_SHADOW_SURFACE_SIZE
            || height > MAX_SHADOW_SURFACE_SIZE
        {
            return;
        }

        let Some(mut offscreen) = Canvas::new(width, height) else {
            return;
        };
        offscreen.state.transform = self.state.transform.post_translate(-left, -top);
        draw(&mut offscreen);

        let mut mask = offscreen.into_pixmap();
        blur::blur_alpha(&mut mask, sigma);
        tint(&mut mask, shadow.color);

        let paint = PixmapPaint {
            opacity: self.state.alpha,
            blend_mode: BlendMode::SourceOver,
            quality: FilterQuality::Nearest,
        };
        self.pixmap.draw_pixmap(
            0,
            0,
            mask.as_ref(),
            &paint,
            Transform::from_translate(
                left + shadow.offset_x * self.shadow_scale,
                top + shadow.offset_y * self.shadow_scale,
            ),
            self.state.clip.as_deref(),
        );
    }
}

fn paint_for(shader: Shader<'_>, blend: BlendMode) -> Paint<'_> {
    Paint {
        shader,
        blend_mode: blend,
        anti_alias: true,
        force_hq_pipeline: false,
    }
}

/// Replaces every pixel's colour with `color`, keeping the blurred coverage.
fn tint(pixmap: &mut Pixmap, color: tiny_skia::Color) {
    let color = color.to_color_u8();
    let (r, g, b, a) = (
        color.red() as u32,
        color.green() as u32,
        color.blue() as u32,
        color.alpha() as u32,
    );
    for pixel in pixmap.data_mut().as_chunks_mut::<4>().0 {
        let coverage = pixel[3] as u32 * a / 255;
        pixel[0] = (r * coverage / 255) as u8;
        pixel[1] = (g * coverage / 255) as u8;
        pixel[2] = (b * coverage / 255) as u8;
        pixel[3] = coverage as u8;
    }
}

fn path_device_bounds(path: &Path, transform: Transform) -> Option<Rect> {
    rect_device_bounds(Some(path.bounds()), transform)
}

fn rect_device_bounds(rect: Option<Rect>, transform: Transform) -> Option<Rect> {
    let rect = rect?;
    let corners = [
        (rect.left(), rect.top()),
        (rect.right(), rect.top()),
        (rect.right(), rect.bottom()),
        (rect.left(), rect.bottom()),
    ];
    let mut points: Vec<tiny_skia::Point> = corners
        .iter()
        .map(|(x, y)| tiny_skia::Point::from_xy(*x, *y))
        .collect();
    transform.map_points(&mut points);
    let mut left = f32::MAX;
    let mut top = f32::MAX;
    let mut right = f32::MIN;
    let mut bottom = f32::MIN;
    for point in points {
        left = left.min(point.x);
        top = top.min(point.y);
        right = right.max(point.x);
        bottom = bottom.max(point.y);
    }
    Rect::from_ltrb(left, top, right, bottom)
}

fn grow(rect: Option<Rect>, amount: f32) -> Option<Rect> {
    let rect = rect?;
    Rect::from_ltrb(
        rect.left() - amount,
        rect.top() - amount,
        rect.right() + amount,
        rect.bottom() + amount,
    )
}

/// `ctx.roundRect` — a rectangle with one radius on every corner, clamped the
/// way the spec clamps radii that would overlap.
pub fn rounded_rect_path(x: f32, y: f32, width: f32, height: f32, radius: f32) -> Option<Path> {
    if width <= 0.0 || height <= 0.0 {
        return None;
    }
    let radius = radius.min(width / 2.0).min(height / 2.0).max(0.0);
    let mut builder = PathBuilder::new();
    if radius <= 0.0 {
        builder.push_rect(Rect::from_xywh(x, y, width, height)?);
        return builder.finish();
    }

    // A circular arc's Bezier control offset.
    let control = radius * (1.0 - 0.552_284_8);
    let (right, bottom) = (x + width, y + height);
    builder.move_to(x + radius, y);
    builder.line_to(right - radius, y);
    builder.cubic_to(right - control, y, right, y + control, right, y + radius);
    builder.line_to(right, bottom - radius);
    builder.cubic_to(
        right,
        bottom - control,
        right - control,
        bottom,
        right - radius,
        bottom,
    );
    builder.line_to(x + radius, bottom);
    builder.cubic_to(x + control, bottom, x, bottom - control, x, bottom - radius);
    builder.line_to(x, y + radius);
    builder.cubic_to(x, y + control, x + control, y, x + radius, y);
    builder.close();
    builder.finish()
}

pub fn paint_rounded_shadow(
    canvas: &mut Canvas,
    rect: (f32, f32, f32, f32),
    radius: f32,
    shadow: Shadow,
    fill: Color,
) -> Option<()> {
    let (x, y, width, height) = rect;
    let path = rounded_rect_path(x, y, width, height, radius)?;
    let spread = blur::box_radius(blur::sigma_for_shadow_blur(shadow.blur)) as u32 * 3 + 2;
    let full_shadow_fits = (path.bounds().width() + spread as f32 * 2.0).ceil()
        <= MAX_SHADOW_SURFACE_SIZE as f32
        && (path.bounds().height() + spread as f32 * 2.0).ceil() <= MAX_SHADOW_SURFACE_SIZE as f32;
    let outer = spread + shadow.offset_x.abs().max(shadow.offset_y.abs()).ceil() as u32 + 2;
    let inner = radius.min(width / 2.0).min(height / 2.0).max(0.0).ceil() as u32 + outer;
    let max_content = (inner * 2 + 1) as f32;
    let content_width = width.min(max_content + width.fract());
    let content_height = height.min(max_content + height.fract());
    let fraction_x = x - x.floor();
    let fraction_y = y - y.floor();
    let patch_width = (fraction_x + content_width).ceil() as u32 + outer * 2;
    let patch_height = (fraction_y + content_height).ceil() as u32 + outer * 2;
    let target_width = (fraction_x + width).ceil() as u32 + outer * 2;
    let target_height = (fraction_y + height).ceil() as u32 + outer * 2;
    if canvas.transform() != Transform::identity()
        || canvas.shadow_scale != 1.0
        || (full_shadow_fits
            && u64::from(patch_width) * u64::from(patch_height) * 2
                >= u64::from(target_width) * u64::from(target_height))
    {
        canvas.save();
        canvas.set_shadow(Some(shadow));
        canvas.fill_path(&path, fill, FillRule::Winding);
        canvas.restore();
        return Some(());
    }

    let mut patch = Canvas::new(patch_width, patch_height)?;
    let patch_path = rounded_rect_path(
        outer as f32 + fraction_x,
        outer as f32 + fraction_y,
        content_width,
        content_height,
        radius,
    )?;
    patch.set_shadow(Some(shadow));
    patch.fill_path(&patch_path, Color::TRANSPARENT, FillRule::Winding);
    let patch = patch.into_pixmap();
    let cap_x = (outer + inner).min(patch_width / 2);
    let cap_y = (outer + inner).min(patch_height / 2);
    let source_x = [0, cap_x, patch_width - cap_x, patch_width];
    let source_y = [0, cap_y, patch_height - cap_y, patch_height];
    let target_x = [0, cap_x, target_width - cap_x, target_width];
    let target_y = [0, cap_y, target_height - cap_y, target_height];
    let mut tiles = Vec::with_capacity(9);
    for row in 0..3 {
        for column in 0..3 {
            let tile_width = source_x[column + 1] - source_x[column];
            let tile_height = source_y[row + 1] - source_y[row];
            if tile_width == 0 || tile_height == 0 {
                continue;
            }
            let tile = patch.clone_rect(tiny_skia::IntRect::from_xywh(
                source_x[column] as i32,
                source_y[row] as i32,
                tile_width,
                tile_height,
            )?)?;
            tiles.push((tile, column, row));
        }
    }
    canvas.save();
    canvas.set_shadow(None);
    for (tile, column, row) in tiles {
        canvas.draw_pixmap_nearest(
            tile.as_ref(),
            x.floor() - outer as f32 + target_x[column] as f32,
            y.floor() - outer as f32 + target_y[row] as f32,
            (target_x[column + 1] - target_x[column]) as f32,
            (target_y[row + 1] - target_y[row]) as f32,
        );
    }
    canvas.fill_path(&path, fill, FillRule::Winding);
    canvas.restore();
    Some(())
}

pub struct PreviewEdge {
    pub x: u32,
    pub y: u32,
    pub pixels: Pixmap,
}

#[allow(clippy::too_many_arguments)]
pub fn preview_clip_edges(
    backdrop: &Pixmap,
    x: f64,
    y: f64,
    content_width: f64,
    content_height: f64,
    title_height: f64,
    radius: f64,
    border_style: Option<(f64, Color)>,
) -> Option<Vec<PreviewEdge>> {
    let stroke_width = border_style.map_or(0.0, |(width, _)| width);
    let stroke = stroke_width / 2.0;
    let left = (x - stroke).floor().max(0.0) as u32;
    let top = (y - stroke).floor().max(0.0) as u32;
    let right = (x + content_width + stroke)
        .ceil()
        .min(backdrop.width() as f64) as u32;
    let bottom = (y + title_height + content_height + stroke)
        .ceil()
        .min(backdrop.height() as f64) as u32;
    let width = right.checked_sub(left)?;
    let height = bottom.checked_sub(top)?;
    let edge = (radius + stroke_width + 2.0).ceil().max(1.0) as u32;
    let top_height = edge.min(height);
    let bottom_height = edge.min(height - top_height);
    let middle_height = height - top_height - bottom_height;
    let left_width = edge.min(width);
    let right_width = edge.min(width - left_width);
    let tiles = if border_style.is_some() {
        [
            (left, top, width, top_height),
            (left, bottom - bottom_height, width, bottom_height),
            (left, top + top_height, left_width, middle_height),
            (
                right - right_width,
                top + top_height,
                right_width,
                middle_height,
            ),
        ]
    } else {
        [
            (left, top, left_width, top_height),
            (right - right_width, top, right_width, top_height),
            (left, bottom - bottom_height, left_width, bottom_height),
            (
                right - right_width,
                bottom - bottom_height,
                right_width,
                bottom_height,
            ),
        ]
    };
    let path = rounded_rect_path(
        x as f32,
        y as f32,
        content_width as f32,
        (content_height + title_height) as f32,
        radius as f32,
    )?;
    let mut clip = Mask::new(backdrop.width(), backdrop.height())?;
    clip.fill_path(&path, FillRule::Winding, true, Transform::identity());
    let (border, border_color) = if let Some((stroke_width, color)) = border_style {
        let mut border = Canvas::new(backdrop.width(), backdrop.height())?;
        border.stroke_path(
            &path,
            Color::from_rgba8(255, 255, 255, 255),
            &Stroke {
                width: stroke_width as f32,
                ..Stroke::default()
            },
        );
        let color = color.to_color_u8();
        (Some(border), [color.red(), color.green(), color.blue()])
    } else {
        (None, [0, 0, 0])
    };
    let mut edges = Vec::new();
    for (tile_x, tile_y, tile_width, tile_height) in tiles {
        if tile_width == 0 || tile_height == 0 {
            continue;
        }
        let mut overlay = Pixmap::new(tile_width, tile_height)?;
        for row in 0..tile_height as usize {
            for column in 0..tile_width as usize {
                let source = ((tile_y as usize + row) * backdrop.width() as usize
                    + tile_x as usize
                    + column)
                    * 4;
                let target = (row * tile_width as usize + column) * 4;
                if backdrop.data()[source + 3] != 255 {
                    return None;
                }
                let pixel_x = tile_x as usize + column;
                let pixel_y = tile_y as usize + row;
                let in_content = (pixel_x as f64) >= x
                    && (pixel_x as f64) < x + content_width
                    && (pixel_y as f64) >= y + title_height
                    && (pixel_y as f64) < y + title_height + content_height;
                let clip_alpha = if in_content {
                    u32::from(clip.data()[source / 4])
                } else {
                    0
                };
                let border_alpha = border
                    .as_ref()
                    .map_or(0, |border| u32::from(border.pixmap().data()[source + 3]));
                let background_alpha = ((255 - border_alpha) * (255 - clip_alpha) + 127) / 255;
                let alpha = (border_alpha + background_alpha).min(255) as u8;
                let mut pixel = [0, 0, 0, alpha];
                for channel in 0..3 {
                    pixel[channel] = ((u32::from(border_color[channel]) * border_alpha
                        + u32::from(backdrop.data()[source + channel]) * background_alpha
                        + 127)
                        / 255)
                        .min(u32::from(alpha)) as u8;
                }
                overlay.data_mut()[target..target + 4].copy_from_slice(&pixel);
            }
        }
        edges.push(PreviewEdge {
            x: tile_x,
            y: tile_y,
            pixels: overlay,
        });
    }
    Some(edges)
}

pub fn circle_path(x: f32, y: f32, radius: f32) -> Option<Path> {
    if radius <= 0.0 {
        return None;
    }
    let mut builder = PathBuilder::new();
    builder.push_circle(x, y, radius);
    builder.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiled_rounded_shadows_match_fractional_full_rasterization() {
        for (x, y, width, height, radius) in [
            (40.25, 39.75, 700.75, 100.25, 23.5),
            (-5.25, 30.5, 720.5, 130.75, 16.5),
            (20.5, -3.75, 130.25, 700.75, 70.0),
            (12.75, 9.25, 770.5, 160.5, 0.0),
        ] {
            for background in [Color::TRANSPARENT, Color::from_rgba8(36, 104, 172, 255)] {
                let shadow = crate::editor::export::image_shadow(50.0).unwrap();
                let fill = Color::from_rgba8(18, 32, 46, 255);
                let mut expected = Canvas::new(800, 800).unwrap();
                let mut actual = Canvas::new(800, 800).unwrap();
                expected.fill_all(background);
                actual.fill_all(background);
                let path = rounded_rect_path(x, y, width, height, radius).unwrap();
                expected.set_shadow(Some(shadow));
                expected.fill_path(&path, fill, FillRule::Winding);
                paint_rounded_shadow(&mut actual, (x, y, width, height), radius, shadow, fill)
                    .unwrap();
                assert!(
                    expected
                        .pixmap()
                        .data()
                        .iter()
                        .zip(actual.pixmap().data())
                        .all(|(expected, actual)| expected.abs_diff(*actual) <= 2),
                    "{x},{y} {width}x{height}, radius {radius}, background {background:?}"
                );
            }
        }
    }

    #[test]
    fn transformed_rounded_shadows_use_the_full_rasterizer() {
        for (transformed, shadow_scale) in [(true, 1.0), (false, 1.5), (true, 1.5)] {
            let shadow = crate::editor::export::image_shadow(50.0).unwrap();
            let mut expected = Canvas::new(800, 400).unwrap();
            let mut actual = Canvas::new(800, 400).unwrap();
            for canvas in [&mut expected, &mut actual] {
                if transformed {
                    canvas.translate(6.0, 4.0);
                    canvas.scale(1.1, 1.2);
                }
                canvas.set_shadow_scale(shadow_scale);
            }
            let rect = (40.0, 40.0, 600.0, 200.0);
            let path = rounded_rect_path(rect.0, rect.1, rect.2, rect.3, 24.0).unwrap();
            expected.set_shadow(Some(shadow));
            expected.fill_path(&path, Color::WHITE, FillRule::Winding);
            paint_rounded_shadow(&mut actual, rect, 24.0, shadow, Color::WHITE).unwrap();
            assert_eq!(actual.pixmap().data(), expected.pixmap().data());
        }
    }

    #[test]
    fn preview_edges_match_rounded_image_clipping() {
        for (width, height, radius) in [(64, 40, 18.0), (10, 10, 20.0), (1, 1, 5.0)] {
            let source = image::RgbaImage::from_fn(width, height, |x, y| {
                image::Rgba([
                    (x * 3) as u8,
                    (y * 5) as u8,
                    160,
                    if (x + y) % 3 == 0 { 128 } else { 255 },
                ])
            });
            let image = crate::editor::export::from_rgba(&source).unwrap();
            let mut expected = Canvas::new(96, 80).unwrap();
            expected.fill_all(Color::from_rgba8(36, 104, 172, 255));
            expected.clip_path(
                &rounded_rect_path(15.0, 10.0, width as f32, height as f32, radius).unwrap(),
                FillRule::Winding,
            );
            expected.draw_pixmap(image.as_ref(), 15.0, 10.0, width as f32, height as f32);
            let mut actual = Canvas::new(96, 80).unwrap();
            actual.fill_all(Color::from_rgba8(36, 104, 172, 255));
            let edges = preview_clip_edges(
                actual.pixmap(),
                15.0,
                10.0,
                f64::from(width),
                f64::from(height),
                0.0,
                f64::from(radius),
                None,
            )
            .unwrap();
            actual.draw_pixmap(image.as_ref(), 15.0, 10.0, width as f32, height as f32);
            for edge in edges {
                actual.draw_pixmap(
                    edge.pixels.as_ref(),
                    edge.x as f32,
                    edge.y as f32,
                    edge.pixels.width() as f32,
                    edge.pixels.height() as f32,
                );
            }
            assert!(
                actual
                    .pixmap()
                    .data()
                    .iter()
                    .zip(expected.pixmap().data())
                    .all(|(actual, expected)| actual.abs_diff(*expected) <= 2),
                "{width}x{height}, radius {radius}"
            );
        }
    }

    fn alpha_at(canvas: &Canvas, x: u32, y: u32) -> u8 {
        let index = ((y * canvas.width() + x) * 4 + 3) as usize;
        canvas.pixmap().data()[index]
    }

    #[test]
    fn a_clip_confines_later_fills() {
        let mut canvas = Canvas::new(20, 20).expect("canvas");
        canvas.save();
        canvas.clip_rect(Rect::from_xywh(0.0, 0.0, 10.0, 10.0).expect("rect"));
        canvas.fill_rect(
            Rect::from_xywh(0.0, 0.0, 20.0, 20.0).expect("rect"),
            Color::from_rgba8(255, 0, 0, 255),
        );
        canvas.restore();

        assert_eq!(alpha_at(&canvas, 5, 5), 255);
        assert_eq!(alpha_at(&canvas, 15, 15), 0);

        canvas.fill_rect(
            Rect::from_xywh(0.0, 0.0, 20.0, 20.0).expect("rect"),
            Color::from_rgba8(255, 0, 0, 255),
        );
        assert_eq!(alpha_at(&canvas, 15, 15), 255);
    }

    #[test]
    fn global_alpha_scales_a_fill() {
        let mut canvas = Canvas::new(4, 4).expect("canvas");
        canvas.set_global_alpha(0.5);
        canvas.fill_rect(
            Rect::from_xywh(0.0, 0.0, 4.0, 4.0).expect("rect"),
            Color::from_rgba8(0, 0, 0, 255),
        );
        let alpha = alpha_at(&canvas, 1, 1);
        assert!((126..=129).contains(&alpha), "{alpha}");
    }

    #[test]
    fn a_shadow_paints_outside_the_shape() {
        let mut canvas = Canvas::new(60, 60).expect("canvas");
        canvas.set_shadow(Some(Shadow {
            color: Color::from_rgba8(0, 0, 0, 128),
            blur: 12.0,
            offset_x: 0.0,
            offset_y: 0.0,
        }));
        canvas.fill_rect(
            Rect::from_xywh(20.0, 20.0, 20.0, 20.0).expect("rect"),
            Color::from_rgba8(255, 255, 255, 255),
        );
        assert!(alpha_at(&canvas, 14, 30) > 0);
        assert_eq!(alpha_at(&canvas, 30, 30), 255);
    }

    #[test]
    fn transforms_stack_and_unwind() {
        let mut canvas = Canvas::new(20, 20).expect("canvas");
        canvas.save();
        canvas.translate(10.0, 10.0);
        canvas.fill_rect(
            Rect::from_xywh(0.0, 0.0, 4.0, 4.0).expect("rect"),
            Color::from_rgba8(0, 0, 0, 255),
        );
        canvas.restore();
        assert_eq!(alpha_at(&canvas, 12, 12), 255);
        assert_eq!(alpha_at(&canvas, 2, 2), 0);

        canvas.fill_rect(
            Rect::from_xywh(0.0, 0.0, 4.0, 4.0).expect("rect"),
            Color::from_rgba8(0, 0, 0, 255),
        );
        assert_eq!(alpha_at(&canvas, 2, 2), 255);
    }

    #[test]
    fn rounded_rect_radii_clamp_to_the_shorter_side() {
        let path = rounded_rect_path(0.0, 0.0, 10.0, 4.0, 40.0).expect("path");
        let bounds = path.bounds();
        assert_eq!(bounds.width(), 10.0);
        assert_eq!(bounds.height(), 4.0);
        assert!(rounded_rect_path(0.0, 0.0, 0.0, 4.0, 2.0).is_none());
    }
}
