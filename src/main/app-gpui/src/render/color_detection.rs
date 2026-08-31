//! Port of `renderer/utils/color-detection.ts` — the edge-colour sampling the
//! wallpaper's balance crop and inset band are derived from.

use tiny_skia::PixmapRef;

/// How far apart two colours may be and still count as the same background.
const COLOR_SIMILARITY_THRESHOLD: i32 = 30;
/// Content bounds are relaxed by this much so the crop never shaves content.
const BALANCE_CROP_BUFFER: u32 = 10;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ContentBounds {
    pub top: u32,
    pub right: u32,
    pub bottom: u32,
    pub left: u32,
}

pub type BalanceCrop = ContentBounds;

fn pixel_at(image: PixmapRef<'_>, x: u32, y: u32) -> Option<[u8; 4]> {
    if x >= image.width() || y >= image.height() {
        return None;
    }
    let index = ((y * image.width() + x) * 4) as usize;
    let data = image.data();
    let alpha = data[index + 3];
    if alpha == 0 {
        return Some([0, 0, 0, 0]);
    }
    // Pixmap data is premultiplied; the sampled colours are compared as
    // straight alpha, the way `getImageData` returns them.
    let unpremultiply = |value: u8| -> u8 {
        ((value as u32 * 255 + alpha as u32 / 2) / alpha as u32).min(255) as u8
    };
    Some([
        unpremultiply(data[index]),
        unpremultiply(data[index + 1]),
        unpremultiply(data[index + 2]),
        alpha,
    ])
}

fn colors_are_similar(a: [u8; 4], b: (u8, u8, u8)) -> bool {
    (a[0] as i32 - b.0 as i32).abs() <= COLOR_SIMILARITY_THRESHOLD
        && (a[1] as i32 - b.1 as i32).abs() <= COLOR_SIMILARITY_THRESHOLD
        && (a[2] as i32 - b.2 as i32).abs() <= COLOR_SIMILARITY_THRESHOLD
}

fn to_hex(pixel: [u8; 4]) -> String {
    format!("#{:02x}{:02x}{:02x}", pixel[0], pixel[1], pixel[2])
}

/// Counts the colours along a span, skipping fully transparent pixels.
fn tally(
    image: PixmapRef<'_>,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    counts: &mut std::collections::HashMap<String, u32>,
) {
    for row in y..y.saturating_add(height) {
        for column in x..x.saturating_add(width) {
            let Some(pixel) = pixel_at(image, column, row) else {
                continue;
            };
            if pixel[3] == 0 {
                continue;
            }
            *counts.entry(to_hex(pixel)).or_insert(0) += 1;
        }
    }
}

fn dominant(counts: std::collections::HashMap<String, u32>) -> Option<String> {
    counts
        .into_iter()
        .max_by_key(|(_, count)| *count)
        .map(|(color, _)| color)
}

/// `detectDominantEdgeColor` — the most common colour around the border.
pub fn dominant_edge_color(image: PixmapRef<'_>) -> Option<String> {
    let (width, height) = (image.width(), image.height());
    if width == 0 || height == 0 {
        return None;
    }
    let mut counts = std::collections::HashMap::new();
    tally(image, 0, 0, width, 1, &mut counts);
    tally(image, 0, height - 1, width, 1, &mut counts);
    if height > 2 {
        tally(image, 0, 1, 1, height - 2, &mut counts);
        tally(image, width - 1, 1, 1, height - 2, &mut counts);
    }
    dominant(counts)
}

/// `detectContentBounds` — how much uniform background sits around the
/// content, which the balance option crops away.
pub fn content_bounds(image: PixmapRef<'_>, background: &str) -> Option<ContentBounds> {
    let (width, height) = (image.width(), image.height());
    if width == 0 || height == 0 {
        return None;
    }
    let parsed = crate::render::color::parse(background)?.to_color_u8();
    let background = (parsed.red(), parsed.green(), parsed.blue());

    let differs = |x: u32, y: u32| -> bool {
        let Some(pixel) = pixel_at(image, x, y) else {
            return false;
        };
        if pixel[3] < 128 {
            return false;
        }
        !colors_are_similar(pixel, background)
    };

    let step = if width > 1000 || height > 1000 { 2 } else { 1 };
    let row_has_content = |y: u32| -> bool { (0..width).step_by(step).any(|x| differs(x, y)) };
    let column_has_content = |x: u32| -> bool { (0..height).step_by(step).any(|y| differs(x, y)) };

    let top = (0..height).find(|y| row_has_content(*y));
    let bottom = (0..height)
        .rev()
        .find(|y| row_has_content(*y))
        .map(|y| height - 1 - y);
    let left = (0..width).find(|x| column_has_content(*x));
    let right = (0..width)
        .rev()
        .find(|x| column_has_content(*x))
        .map(|x| width - 1 - x);

    if top.is_none() && bottom.is_none() && left.is_none() && right.is_none() {
        return Some(ContentBounds::default());
    }

    Some(ContentBounds {
        top: top.unwrap_or(0).saturating_sub(BALANCE_CROP_BUFFER),
        right: right.unwrap_or(0).saturating_sub(BALANCE_CROP_BUFFER),
        bottom: bottom.unwrap_or(0).saturating_sub(BALANCE_CROP_BUFFER),
        left: left.unwrap_or(0).saturating_sub(BALANCE_CROP_BUFFER),
    })
}

/// `sampleDominantInsetColor` — the colour the inset band is filled with,
/// sampled from the (possibly cropped) image's own border.
pub fn dominant_inset_color(image: PixmapRef<'_>, crop: BalanceCrop) -> Option<String> {
    let width = image.width().checked_sub(crop.left + crop.right)?;
    let height = image.height().checked_sub(crop.top + crop.bottom)?;
    if width == 0 || height == 0 {
        return None;
    }

    let mut counts = std::collections::HashMap::new();
    tally(image, crop.left, crop.top, width, 1, &mut counts);
    tally(
        image,
        crop.left,
        crop.top + height - 1,
        width,
        1,
        &mut counts,
    );
    if height > 2 {
        tally(image, crop.left, crop.top + 1, 1, height - 2, &mut counts);
        tally(
            image,
            crop.left + width - 1,
            crop.top + 1,
            1,
            height - 2,
            &mut counts,
        );
    }
    dominant(counts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tiny_skia::{Color, Paint, Pixmap, Rect, Transform};

    fn image_with_content() -> Pixmap {
        let mut pixmap = Pixmap::new(100, 100).expect("pixmap");
        pixmap.fill(Color::from_rgba8(20, 20, 20, 255));
        let mut paint = Paint::default();
        paint.set_color(Color::from_rgba8(240, 10, 10, 255));
        pixmap.fill_rect(
            Rect::from_xywh(30.0, 40.0, 20.0, 20.0).expect("rect"),
            &paint,
            Transform::identity(),
            None,
        );
        pixmap
    }

    #[test]
    fn the_dominant_edge_colour_is_the_border() {
        let pixmap = image_with_content();
        assert_eq!(
            dominant_edge_color(pixmap.as_ref()).as_deref(),
            Some("#141414")
        );
    }

    #[test]
    fn content_bounds_report_the_uniform_margin() {
        let pixmap = image_with_content();
        let bounds = content_bounds(pixmap.as_ref(), "#141414").expect("bounds");
        // The content sits at 30..50 x 40..60; the buffer relaxes each edge.
        assert_eq!(bounds.left, 20);
        assert_eq!(bounds.top, 30);
        assert_eq!(bounds.right, 40);
        assert_eq!(bounds.bottom, 30);
    }

    #[test]
    fn a_uniform_image_has_no_content_bounds() {
        let mut pixmap = Pixmap::new(20, 20).expect("pixmap");
        pixmap.fill(Color::from_rgba8(5, 5, 5, 255));
        assert_eq!(
            content_bounds(pixmap.as_ref(), "#050505"),
            Some(ContentBounds::default())
        );
    }

    #[test]
    fn the_inset_colour_comes_from_the_cropped_border() {
        let pixmap = image_with_content();
        let color = dominant_inset_color(
            pixmap.as_ref(),
            ContentBounds {
                top: 35,
                left: 25,
                right: 45,
                bottom: 35,
            },
        );
        assert_eq!(color.as_deref(), Some("#141414"));
    }

    #[test]
    fn an_over_cropped_image_samples_nothing() {
        let pixmap = image_with_content();
        assert!(dominant_inset_color(
            pixmap.as_ref(),
            ContentBounds {
                top: 200,
                left: 200,
                right: 200,
                bottom: 200,
            }
        )
        .is_none());
    }
}
