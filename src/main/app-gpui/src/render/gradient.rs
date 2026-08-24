//! Port of `renderGradientToCanvas` and `renderBackgroundImageToCanvas` in
//! `renderer/utils/wallpaper-render.ts` — the wallpaper backgrounds behind a
//! screenshot or a recording.

use tiny_skia::{
    FillRule, GradientStop, LinearGradient, Pixmap, PixmapRef, Point, Rect, SpreadMode, Transform,
};

use crate::render::canvas::Canvas;
use crate::render::color;

/// A `GradientOption` from `types/editor.ts` as it is persisted.
#[derive(Clone, Debug, Default, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct GradientOption {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub colors: Vec<String>,
    #[serde(default)]
    pub angle: f64,
}

impl GradientOption {
    pub fn is_renderable(&self) -> bool {
        self.colors.len() >= 2
    }
}

/// The gradient's endpoints for a `width` x `height` surface, following the
/// renderer's `angle - 90` convention (0 degrees points up).
pub fn endpoints(angle: f64, width: f32, height: f32) -> ((f32, f32), (f32, f32)) {
    let angle = if angle.is_finite() { angle } else { 0.0 };
    let radians = ((angle - 90.0) * std::f64::consts::PI / 180.0) as f32;
    let (cos, sin) = (radians.cos(), radians.sin());
    let half_width = width / 2.0;
    let half_height = height / 2.0;
    let length = (width * width + height * height).sqrt() / 2.0;
    (
        (half_width - cos * length, half_height - sin * length),
        (half_width + cos * length, half_height + sin * length),
    )
}

pub fn fill(canvas: &mut Canvas, gradient: &GradientOption, width: f32, height: f32) {
    if !gradient.is_renderable() || width <= 0.0 || height <= 0.0 {
        return;
    }
    let (start, end) = endpoints(gradient.angle, width, height);
    let last = (gradient.colors.len() - 1) as f32;
    let stops: Vec<GradientStop> = gradient
        .colors
        .iter()
        .enumerate()
        .map(|(index, value)| {
            GradientStop::new(
                index as f32 / last,
                color::parse_or(value, tiny_skia::Color::TRANSPARENT),
            )
        })
        .collect();

    let Some(shader) = LinearGradient::new(
        Point::from_xy(start.0, start.1),
        Point::from_xy(end.0, end.1),
        stops,
        SpreadMode::Pad,
        Transform::identity(),
    ) else {
        return;
    };
    let Some(rect) = Rect::from_xywh(0.0, 0.0, width, height) else {
        return;
    };
    let mut builder = tiny_skia::PathBuilder::new();
    builder.push_rect(rect);
    if let Some(path) = builder.finish() {
        canvas.fill_path_shader(&path, shader, FillRule::Winding);
    }
}

/// Covers the surface with `image`, cropping the overflowing axis — the
/// `object-fit: cover` behaviour of `renderBackgroundImageToCanvas`.
pub fn fill_image(canvas: &mut Canvas, image: PixmapRef<'_>, width: f32, height: f32) {
    if image.width() == 0 || image.height() == 0 || width <= 0.0 || height <= 0.0 {
        return;
    }
    let image_aspect = image.width() as f32 / image.height() as f32;
    let surface_aspect = width / height;

    let (draw_width, draw_height) = if image_aspect > surface_aspect {
        (height * image_aspect, height)
    } else {
        (width, width / image_aspect)
    };
    let x = (width - draw_width) / 2.0;
    let y = (height - draw_height) / 2.0;
    canvas.draw_pixmap(image, x, y, draw_width, draw_height);
}

/// Port of `renderNoise` in `renderer/utils/noise.ts` — a grain overlay over
/// the background. `seed` keeps the grain stable between renders, so a preview
/// and its export get the same pixels.
pub fn overlay_noise(canvas: &mut Canvas, width: f32, height: f32, noise: f64, seed: u64) {
    if noise <= 0.0 || width <= 0.0 || height <= 0.0 {
        return;
    }
    let opacity = ((noise / 100.0) * 0.3).clamp(0.0, 1.0) as f32;
    let (columns, rows) = (width.ceil() as u32, height.ceil() as u32);
    let Some(mut grain) = Pixmap::new(columns.max(1), rows.max(1)) else {
        return;
    };

    // A xorshift keeps the grain reproducible without pulling in a random
    // number generator; the renderer's `Math.random()` only has to look random.
    let mut state = seed | 1;
    for pixel in grain.data_mut().chunks_exact_mut(4) {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let value = (state >> 24) as u8;
        pixel[0] = value;
        pixel[1] = value;
        pixel[2] = value;
        pixel[3] = 255;
    }

    canvas.save();
    canvas.set_shadow(None);
    canvas.set_global_alpha(opacity);
    canvas.draw_pixmap_blended(
        grain.as_ref(),
        0.0,
        0.0,
        width,
        height,
        tiny_skia::BlendMode::Overlay,
    );
    canvas.restore();
}

/// Decodes a `data:` URL or a file path into a pixmap, which is how both the
/// wallpaper background and the first-frame image arrive from `state.json`.
pub fn load_image(source: &str) -> Option<Pixmap> {
    let bytes = if let Some(encoded) = source.split_once("base64,").map(|(_, tail)| tail) {
        use base64::Engine as _;
        base64::engine::general_purpose::STANDARD
            .decode(encoded.trim())
            .ok()?
    } else {
        std::fs::read(source).ok()?
    };
    decode_image(&bytes)
}

pub fn decode_image(bytes: &[u8]) -> Option<Pixmap> {
    let decoded = image::load_from_memory(bytes).ok()?.to_rgba8();
    let mut pixmap = Pixmap::new(decoded.width(), decoded.height())?;
    let target = pixmap.data_mut();
    for (index, pixel) in decoded.pixels().enumerate() {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_zero_degree_gradient_runs_bottom_to_top() {
        let (start, end) = endpoints(0.0, 100.0, 100.0);
        assert!(start.1 > end.1, "{start:?} -> {end:?}");
        assert!((start.0 - 50.0).abs() < 0.001);
    }

    #[test]
    fn a_ninety_degree_gradient_runs_left_to_right() {
        let (start, end) = endpoints(90.0, 100.0, 100.0);
        assert!(start.0 < end.0, "{start:?} -> {end:?}");
        assert!((start.1 - 50.0).abs() < 0.001);
    }

    #[test]
    fn a_single_colour_gradient_is_not_renderable() {
        let gradient = GradientOption {
            colors: vec!["#fff".into()],
            ..GradientOption::default()
        };
        assert!(!gradient.is_renderable());
    }

    #[test]
    fn fills_the_whole_surface() {
        let mut canvas = Canvas::new(16, 16).expect("canvas");
        let gradient = GradientOption {
            colors: vec!["#ff0000".into(), "#0000ff".into()],
            angle: 90.0,
            ..GradientOption::default()
        };
        fill(&mut canvas, &gradient, 16.0, 16.0);
        assert!(canvas
            .pixmap()
            .data()
            .chunks_exact(4)
            .all(|pixel| pixel[3] == 255));
    }

    #[test]
    fn noise_is_reproducible_for_a_seed() {
        let render = |seed: u64| {
            let mut canvas = Canvas::new(16, 16).expect("canvas");
            canvas.fill_all(tiny_skia::Color::from_rgba8(128, 128, 128, 255));
            overlay_noise(&mut canvas, 16.0, 16.0, 100.0, seed);
            canvas.into_pixmap().data().to_vec()
        };
        assert_eq!(render(7), render(7));
        assert_ne!(render(7), render(9));
    }

    #[test]
    fn zero_noise_leaves_the_surface_alone() {
        let mut canvas = Canvas::new(8, 8).expect("canvas");
        canvas.fill_all(tiny_skia::Color::from_rgba8(10, 20, 30, 255));
        let before = canvas.pixmap().data().to_vec();
        overlay_noise(&mut canvas, 8.0, 8.0, 0.0, 1);
        assert_eq!(canvas.pixmap().data(), &before[..]);
    }

    #[test]
    fn covers_with_an_image_without_letterboxing() {
        let mut source = Pixmap::new(4, 2).expect("pixmap");
        source.fill(tiny_skia::Color::from_rgba8(0, 255, 0, 255));
        let mut canvas = Canvas::new(10, 10).expect("canvas");
        fill_image(&mut canvas, source.as_ref(), 10.0, 10.0);
        assert!(canvas
            .pixmap()
            .data()
            .chunks_exact(4)
            .all(|pixel| pixel[3] == 255));
    }
}
