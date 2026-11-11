//! Port of `renderWindowFrame` in `renderer/utils/wallpaper-render.ts` and the
//! themes in `renderer/utils/window-frame.ts` — the macOS and Windows window
//! chrome the editor can wrap a screenshot in.

use tiny_skia::{Color, FillRule, LineCap, PixmapRef, Rect, Stroke};

use crate::render::canvas::{circle_path, paint_rounded_shadow, rounded_rect_path, Canvas, Shadow};
use crate::render::color;

pub const TITLE_BAR_HEIGHT: f64 = 28.0;

const TRAFFIC_LIGHT_SIZE: f64 = 12.0;
const TRAFFIC_LIGHT_SPACING: f64 = 8.0;
const TRAFFIC_LIGHT_OFFSET_X: f64 = 13.0;
const TRAFFIC_LIGHT_COLORS: [&str; 3] = ["#FF5F57", "#FFBD2E", "#28C840"];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    pub title_bar: &'static str,
    pub title_bar_border: &'static str,
    pub content: &'static str,
    pub frame_border: &'static str,
    pub control: &'static str,
}

/// `WINDOW_FRAME_THEMES`. `None` is the unframed style.
pub fn theme(style: &str) -> Option<Theme> {
    match style {
        "macos-light" => Some(Theme {
            title_bar: "#E8E8E8",
            title_bar_border: "#D1D1D1",
            content: "#FFFFFF",
            frame_border: "#A8A8A8",
            control: "#262626",
        }),
        "macos-dark" => Some(Theme {
            title_bar: "#3A3A3C",
            title_bar_border: "#2A2A2C",
            content: "#1C1C1E",
            frame_border: "#606064",
            control: "#F5F5F5",
        }),
        "windows-light" => Some(Theme {
            title_bar: "#F3F3F3",
            title_bar_border: "#D6D6D6",
            content: "#FFFFFF",
            frame_border: "#8A8A8A",
            control: "#1A1A1A",
        }),
        "windows-dark" => Some(Theme {
            title_bar: "#202020",
            title_bar_border: "#3A3A3A",
            content: "#121212",
            frame_border: "#707070",
            control: "#FFFFFF",
        }),
        _ => None,
    }
}

pub fn is_windows_frame(style: &str) -> bool {
    style == "windows-light" || style == "windows-dark"
}

/// `getWindowFrameCornerRadius`.
pub fn corner_radius(style: &str) -> f64 {
    if is_windows_frame(style) {
        8.0
    } else {
        10.0
    }
}

/// The extra height a frame adds above the image.
pub fn title_bar_height(style: &str, scale: f64) -> f64 {
    if theme(style).is_some() {
        (TITLE_BAR_HEIGHT * scale).round()
    } else {
        0.0
    }
}

/// Draws the frame and the image inside it, at `x, y`. `scale` matches the
/// renderer's `nativeScale`, so the chrome grows with the exported size.
#[allow(clippy::too_many_arguments)]
pub fn render(
    canvas: &mut Canvas,
    image: PixmapRef<'_>,
    x: f64,
    y: f64,
    content_width: f64,
    content_height: f64,
    style: &str,
    shadow: Option<Shadow>,
    scale: f64,
) {
    render_chrome(
        canvas,
        x,
        y,
        content_width,
        content_height,
        style,
        shadow,
        scale,
        Some(image),
        true,
    );
}

/// The frame without its image, which the editor preview draws behind the
/// live canvas so the chrome is visible while editing.
#[allow(clippy::too_many_arguments)]
pub fn render_chrome(
    canvas: &mut Canvas,
    x: f64,
    y: f64,
    content_width: f64,
    content_height: f64,
    style: &str,
    shadow: Option<Shadow>,
    scale: f64,
    image: Option<PixmapRef<'_>>,
    paint_outline: bool,
) {
    let Some(theme) = theme(style) else {
        return;
    };
    let bar_height = (TITLE_BAR_HEIGHT * scale).round();
    let radius = corner_radius(style) * scale;
    let frame_width = content_width;
    let frame_height = content_height + bar_height;

    let Some(outline) = rounded_rect_path(
        x as f32,
        y as f32,
        frame_width as f32,
        frame_height as f32,
        radius as f32,
    ) else {
        return;
    };

    canvas.save();
    // The shadow needs something opaque to cast from, which is the frame's own
    // content colour.
    if let Some(shadow) = shadow {
        let fill = color::parse_or(theme.content, Color::from_rgba8(255, 255, 255, 255));
        if paint_rounded_shadow(
            canvas,
            (x as f32, y as f32, frame_width as f32, frame_height as f32),
            radius as f32,
            shadow,
            fill,
        )
        .is_none()
        {
            canvas.set_shadow(Some(shadow));
            canvas.fill_path(&outline, fill, FillRule::Winding);
        }
    }
    canvas.set_shadow(None);
    canvas.clip_path(&outline, FillRule::Winding);

    if let Some(bar) = Rect::from_xywh(x as f32, y as f32, frame_width as f32, bar_height as f32) {
        canvas.fill_rect(bar, color::parse_or(theme.title_bar, Color::TRANSPARENT));
    }

    let mut border = tiny_skia::PathBuilder::new();
    border.move_to(x as f32, (y + bar_height) as f32);
    border.line_to((x + frame_width) as f32, (y + bar_height) as f32);
    if let Some(path) = border.finish() {
        canvas.stroke_path(
            &path,
            color::parse_or(theme.title_bar_border, Color::TRANSPARENT),
            &Stroke {
                width: (0.5 * scale) as f32,
                ..Stroke::default()
            },
        );
    }

    if is_windows_frame(style) {
        draw_windows_controls(canvas, x, y, frame_width, theme.control, scale);
    } else {
        draw_traffic_lights(
            canvas,
            x + TRAFFIC_LIGHT_OFFSET_X * scale,
            y + bar_height / 2.0,
            scale,
        );
    }

    if let Some(image) = image {
        canvas.draw_pixmap(
            image,
            x as f32,
            (y + bar_height) as f32,
            content_width as f32,
            content_height as f32,
        );
    }
    canvas.restore();

    if paint_outline {
        stroke_outline(canvas, &outline, theme, scale);
    }
}

fn stroke_outline(canvas: &mut Canvas, outline: &tiny_skia::Path, theme: Theme, scale: f64) {
    canvas.save();
    canvas.stroke_path(
        outline,
        color::parse_or(theme.frame_border, Color::TRANSPARENT),
        &Stroke {
            width: scale as f32,
            ..Stroke::default()
        },
    );
    canvas.restore();
}

pub fn render_outline(
    canvas: &mut Canvas,
    x: f64,
    y: f64,
    content_width: f64,
    content_height: f64,
    style: &str,
    scale: f64,
) {
    let Some(theme) = theme(style) else {
        return;
    };
    let Some(outline) = rounded_rect_path(
        x as f32,
        y as f32,
        content_width as f32,
        (content_height + title_bar_height(style, scale)) as f32,
        (corner_radius(style) * scale) as f32,
    ) else {
        return;
    };
    stroke_outline(canvas, &outline, theme, scale);
}

pub fn preview_edges(
    backdrop: &tiny_skia::Pixmap,
    x: f64,
    y: f64,
    content_width: f64,
    content_height: f64,
    style: &str,
    scale: f64,
) -> Option<Vec<crate::render::canvas::PreviewEdge>> {
    let theme = theme(style)?;
    crate::render::canvas::preview_clip_edges(
        backdrop,
        x,
        y,
        content_width,
        content_height,
        title_bar_height(style, scale),
        corner_radius(style) * scale,
        Some((
            scale,
            color::parse_or(theme.frame_border, Color::TRANSPARENT),
        )),
    )
}

fn draw_traffic_lights(canvas: &mut Canvas, x: f64, y: f64, scale: f64) {
    let size = TRAFFIC_LIGHT_SIZE * scale;
    let spacing = TRAFFIC_LIGHT_SPACING * scale;
    for (index, value) in TRAFFIC_LIGHT_COLORS.iter().enumerate() {
        let center_x = x + index as f64 * (size + spacing);
        if let Some(path) = circle_path(center_x as f32, y as f32, (size / 2.0) as f32) {
            canvas.fill_path(
                &path,
                color::parse_or(value, Color::TRANSPARENT),
                FillRule::Winding,
            );
        }
    }
}

fn draw_windows_controls(
    canvas: &mut Canvas,
    x: f64,
    y: f64,
    width: f64,
    control: &str,
    scale: f64,
) {
    let control_width = 34.0 * scale;
    let center_y = y + (TITLE_BAR_HEIGHT * scale) / 2.0;
    let minimize_x = x + width - control_width * 2.5;
    let maximize_x = x + width - control_width * 1.5;
    let close_x = x + width - control_width * 0.5;
    let radius = 4.0 * scale;
    let paint = color::parse_or(control, Color::TRANSPARENT);
    let pen = Stroke {
        width: scale as f32,
        line_cap: LineCap::Butt,
        ..Stroke::default()
    };

    let mut builder = tiny_skia::PathBuilder::new();
    builder.move_to(
        (minimize_x - radius) as f32,
        (center_y + radius * 0.75) as f32,
    );
    builder.line_to(
        (minimize_x + radius) as f32,
        (center_y + radius * 0.75) as f32,
    );
    if let Some(path) = builder.finish() {
        canvas.stroke_path(&path, paint, &pen);
    }

    if let Some(rect) = Rect::from_xywh(
        (maximize_x - radius) as f32,
        (center_y - radius) as f32,
        (radius * 2.0) as f32,
        (radius * 2.0) as f32,
    ) {
        let mut builder = tiny_skia::PathBuilder::new();
        builder.push_rect(rect);
        if let Some(path) = builder.finish() {
            canvas.stroke_path(&path, paint, &pen);
        }
    }

    let mut builder = tiny_skia::PathBuilder::new();
    builder.move_to((close_x - radius) as f32, (center_y - radius) as f32);
    builder.line_to((close_x + radius) as f32, (center_y + radius) as f32);
    builder.move_to((close_x + radius) as f32, (center_y - radius) as f32);
    builder.line_to((close_x - radius) as f32, (center_y + radius) as f32);
    if let Some(path) = builder.finish() {
        canvas.stroke_path(&path, paint, &pen);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tiny_skia::Pixmap;

    #[test]
    fn only_the_four_framed_styles_have_a_theme() {
        for style in ["macos-light", "macos-dark", "windows-light", "windows-dark"] {
            assert!(theme(style).is_some(), "{style}");
        }
        assert!(theme("none").is_none());
        assert!(theme("").is_none());
    }

    #[test]
    fn windows_frames_have_the_tighter_corner() {
        assert_eq!(corner_radius("windows-light"), 8.0);
        assert_eq!(corner_radius("macos-dark"), 10.0);
    }

    #[test]
    fn an_unframed_style_adds_no_title_bar() {
        assert_eq!(title_bar_height("none", 2.0), 0.0);
        assert_eq!(title_bar_height("macos-light", 2.0), 56.0);
    }

    #[test]
    fn the_frame_paints_a_title_bar_above_the_image() {
        let mut canvas = Canvas::new(200, 160).expect("canvas");
        let mut image = Pixmap::new(180, 100).expect("image");
        image.fill(Color::from_rgba8(255, 0, 0, 255));

        render(
            &mut canvas,
            image.as_ref(),
            10.0,
            10.0,
            180.0,
            100.0,
            "macos-light",
            None,
            1.0,
        );

        let pixel = |x: u32, y: u32| {
            let index = ((y * canvas.width() + x) * 4) as usize;
            let data = canvas.pixmap().data();
            [data[index], data[index + 1], data[index + 2]]
        };
        // The title bar is the theme's grey, the content below it is the image.
        assert_ne!(pixel(100, 20), [255, 0, 0]);
        assert_eq!(pixel(100, 60), [255, 0, 0]);
    }

    #[test]
    fn translucent_content_uses_the_backdrop_without_a_shadow() {
        let mut image = Pixmap::new(60, 60).expect("image");
        image.fill(Color::from_rgba8(240, 80, 64, 128));
        let shadow = Shadow {
            color: Color::from_rgba8(0, 0, 0, 128),
            blur: 4.0,
            offset_x: 0.0,
            offset_y: 0.0,
        };
        for (shadow, background) in [(None, [36_u8, 104, 172]), (Some(shadow), [255, 255, 255])] {
            let mut canvas = Canvas::new(100, 120).expect("canvas");
            canvas.fill_all(Color::from_rgba8(36, 104, 172, 255));
            render(
                &mut canvas,
                image.as_ref(),
                10.0,
                10.0,
                60.0,
                60.0,
                "windows-light",
                shadow,
                1.0,
            );
            let index = ((68 * canvas.width() + 40) * 4) as usize;
            let pixel = &canvas.pixmap().data()[index..index + 4];
            for (actual, (foreground, background)) in pixel[..3]
                .iter()
                .zip([240_u8, 80, 64].into_iter().zip(background))
            {
                let expected = (u16::from(foreground) * 128 + u16::from(background) * 127) / 255;
                assert!(actual.abs_diff(expected as u8) <= 2, "{pixel:?}");
            }
            assert_eq!(pixel[3], 255);
        }

        let mut canvas = Canvas::new(100, 120).expect("canvas");
        render(
            &mut canvas,
            image.as_ref(),
            10.0,
            10.0,
            60.0,
            60.0,
            "windows-light",
            None,
            1.0,
        );
        let index = ((68 * canvas.width() + 40) * 4) as usize;
        assert_eq!(canvas.pixmap().data()[index + 3], 128);
    }

    #[test]
    fn preview_edge_tiles_composite_like_export() {
        let shadow = Shadow {
            color: Color::from_rgba8(0, 0, 0, 128),
            blur: 4.0,
            offset_x: 0.0,
            offset_y: 0.0,
        };
        for (width, height) in [(64, 40), (10, 10), (1, 1)] {
            let source = image::RgbaImage::from_fn(width, height, |x, y| {
                image::Rgba([
                    (x * 3) as u8,
                    (y * 5) as u8,
                    160,
                    if (x + y) % 3 == 0 { 128 } else { 255 },
                ])
            });
            let image = crate::editor::export::from_rgba(&source).unwrap();
            for style in ["macos-light", "macos-dark", "windows-light", "windows-dark"] {
                for scale in [1.0, 1.25, 2.0] {
                    for shadow in [None, Some(shadow)] {
                        let mut expected = Canvas::new(96, 140).unwrap();
                        expected.fill_all(Color::from_rgba8(36, 104, 172, 255));
                        render(
                            &mut expected,
                            image.as_ref(),
                            15.0,
                            10.0,
                            f64::from(width),
                            f64::from(height),
                            style,
                            shadow,
                            scale,
                        );
                        let mut backdrop = Canvas::new(96, 140).unwrap();
                        backdrop.fill_all(Color::from_rgba8(36, 104, 172, 255));
                        render_chrome(
                            &mut backdrop,
                            15.0,
                            10.0,
                            f64::from(width),
                            f64::from(height),
                            style,
                            shadow,
                            scale,
                            None,
                            false,
                        );
                        let edges = preview_edges(
                            backdrop.pixmap(),
                            15.0,
                            10.0,
                            f64::from(width),
                            f64::from(height),
                            style,
                            scale,
                        )
                        .unwrap();
                        backdrop.draw_pixmap(
                            image.as_ref(),
                            15.0,
                            (10.0 + title_bar_height(style, scale)) as f32,
                            width as f32,
                            height as f32,
                        );
                        for edge in edges {
                            backdrop.draw_pixmap(
                                edge.pixels.as_ref(),
                                edge.x as f32,
                                edge.y as f32,
                                edge.pixels.width() as f32,
                                edge.pixels.height() as f32,
                            );
                        }
                        let different = backdrop
                            .pixmap()
                            .data()
                            .iter()
                            .zip(expected.pixmap().data())
                            .filter(|(actual, expected)| actual.abs_diff(**expected) > 2)
                            .count();
                        assert_eq!(
                            different, 0,
                            "{width}x{height}, {style}, {scale}, shadow={shadow:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn transparent_backdrop_falls_back_to_the_original_outline() {
        let mut fallback = Canvas::new(96, 140).unwrap();
        render_chrome(
            &mut fallback,
            15.0,
            10.0,
            64.0,
            40.0,
            "windows-dark",
            None,
            1.25,
            None,
            false,
        );
        assert!(preview_edges(
            fallback.pixmap(),
            15.0,
            10.0,
            64.0,
            40.0,
            "windows-dark",
            1.25
        )
        .is_none());
        render_outline(&mut fallback, 15.0, 10.0, 64.0, 40.0, "windows-dark", 1.25);
        let mut original = Canvas::new(96, 140).unwrap();
        render_chrome(
            &mut original,
            15.0,
            10.0,
            64.0,
            40.0,
            "windows-dark",
            None,
            1.25,
            None,
            true,
        );
        assert_eq!(fallback.pixmap().data(), original.pixmap().data());
    }

    #[test]
    fn an_unframed_style_draws_nothing() {
        let mut canvas = Canvas::new(40, 40).expect("canvas");
        let image = Pixmap::new(20, 20).expect("image");
        render(
            &mut canvas,
            image.as_ref(),
            0.0,
            0.0,
            20.0,
            20.0,
            "none",
            None,
            1.0,
        );
        assert!(canvas
            .pixmap()
            .data()
            .as_chunks::<4>()
            .0
            .iter()
            .all(|pixel| pixel[3] == 0));
    }
}
