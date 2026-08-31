//! Text drawing on a `Canvas`. Glyphs are rasterized at the surface's device
//! scale with the platform font (see `editor::text_render`) and blitted, so a
//! label reads the same in the editor preview and in the exported file.

use tiny_skia::{Color, Pixmap, Transform};

use crate::editor::text_render;
use crate::render::canvas::Canvas;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Align {
    Left,
    Center,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Baseline {
    Alphabetic,
    Middle,
}

pub struct Metrics {
    pub width: f32,
    pub ascent: f32,
    pub descent: f32,
}

impl Metrics {
    pub fn height(&self) -> f32 {
        self.ascent + self.descent
    }
}

/// `ctx.measureText` for the family the renderer would have resolved.
pub fn measure(text: &str, family: &str, size: f32) -> Metrics {
    match text_render::measure(text, family, size) {
        Some(metrics) => Metrics {
            width: metrics.width,
            ascent: metrics.ascent,
            descent: metrics.descent,
        },
        // Without a usable system font, fall back to a proportional estimate so
        // layout still centres and wraps at roughly the right places.
        None => Metrics {
            width: text.chars().count() as f32 * size * 0.55,
            ascent: size * 0.8,
            descent: size * 0.2,
        },
    }
}

/// `ctx.fillText`. `x`/`y` are in user space and interpreted per `align` and
/// `baseline`, matching the renderer's `textAlign`/`textBaseline` settings.
pub fn fill_text(
    canvas: &mut Canvas,
    text: &str,
    family: &str,
    size: f32,
    x: f32,
    y: f32,
    color: Color,
    align: Align,
    baseline: Baseline,
) {
    if text.is_empty() || size <= 0.0 {
        return;
    }
    let metrics = measure(text, family, size);
    let origin_x = match align {
        Align::Left => x,
        Align::Center => x - metrics.width / 2.0,
    };
    let baseline_y = match baseline {
        Baseline::Alphabetic => y,
        Baseline::Middle => y + (metrics.ascent - metrics.descent) / 2.0,
    };

    let scale = canvas.device_scale();
    let raster_size = (size * scale).clamp(1.0, 800.0);
    let effective_scale = raster_size / size;

    let pad = (raster_size * 0.6).ceil();
    let width = (metrics.width * effective_scale + pad * 2.0).ceil() as u32;
    let height = (metrics.height() * effective_scale + pad * 2.0).ceil() as u32;
    let Some(mut glyphs) = Pixmap::new(width.max(1), height.max(1)) else {
        return;
    };

    let color = color.to_color_u8();
    let (r, g, b, a) = (
        color.red() as u32,
        color.green() as u32,
        color.blue() as u32,
        color.alpha() as f32 / 255.0,
    );
    let stride = width as i64;
    let data = glyphs.data_mut();
    let drawn = text_render::rasterize(
        text,
        family,
        raster_size,
        pad,
        pad + metrics.ascent * effective_scale,
        |px, py, coverage| {
            if px < 0 || py < 0 || px >= stride || py >= height as i64 {
                return;
            }
            let index = ((py * stride + px) * 4) as usize;
            let alpha = (coverage * a * 255.0).round().clamp(0.0, 255.0) as u32;
            if alpha == 0 {
                return;
            }
            // Premultiplied, and taking the maximum keeps overlapping glyph
            // boxes from double-darkening their shared pixels.
            let existing = data[index + 3] as u32;
            if alpha <= existing {
                return;
            }
            data[index] = (r * alpha / 255) as u8;
            data[index + 1] = (g * alpha / 255) as u8;
            data[index + 2] = (b * alpha / 255) as u8;
            data[index + 3] = alpha as u8;
        },
    );
    if !drawn {
        return;
    }

    let placement = Transform::from_translate(
        origin_x - pad / effective_scale,
        baseline_y - (pad + metrics.ascent * effective_scale) / effective_scale,
    )
    .pre_scale(1.0 / effective_scale, 1.0 / effective_scale);
    canvas.draw_pixmap_transformed(glyphs.as_ref(), placement);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::canvas::Canvas;

    #[test]
    fn measurement_grows_with_the_string() {
        let short = measure("i", "sans", 20.0);
        let long = measure("iiiiiiii", "sans", 20.0);
        assert!(long.width > short.width);
        assert!(short.height() > 0.0);
    }

    #[test]
    fn drawing_marks_pixels_when_a_font_is_available() {
        if !text_render::is_available() {
            return;
        }
        let mut canvas = Canvas::new(120, 60).expect("canvas");
        fill_text(
            &mut canvas,
            "Hello",
            "sans",
            28.0,
            10.0,
            40.0,
            Color::from_rgba8(255, 255, 255, 255),
            Align::Left,
            Baseline::Alphabetic,
        );
        let covered = canvas
            .pixmap()
            .data()
            .chunks_exact(4)
            .filter(|pixel| pixel[3] > 0)
            .count();
        assert!(covered > 0);
    }

    #[test]
    fn centring_shifts_the_run_left_by_half_its_width() {
        if !text_render::is_available() {
            return;
        }
        let text = "MMM";
        let metrics = measure(text, "sans", 24.0);
        let mut left = Canvas::new(200, 60).expect("canvas");
        let mut centred = Canvas::new(200, 60).expect("canvas");
        fill_text(
            &mut left,
            text,
            "sans",
            24.0,
            100.0 - metrics.width / 2.0,
            40.0,
            Color::from_rgba8(255, 255, 255, 255),
            Align::Left,
            Baseline::Alphabetic,
        );
        fill_text(
            &mut centred,
            text,
            "sans",
            24.0,
            100.0,
            40.0,
            Color::from_rgba8(255, 255, 255, 255),
            Align::Center,
            Baseline::Alphabetic,
        );
        assert_eq!(left.pixmap().data(), centred.pixmap().data());
    }
}
