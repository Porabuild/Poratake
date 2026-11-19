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

#[derive(Clone, Copy)]
struct Samples<'a> {
    bytes: &'a [u8],
    width: u32,
    height: u32,
    straight_alpha: bool,
}

impl<'a> Samples<'a> {
    fn rgba(image: &'a image::RgbaImage) -> Self {
        Self {
            bytes: image.as_raw(),
            width: image.width(),
            height: image.height(),
            straight_alpha: true,
        }
    }

    fn pixmap(image: PixmapRef<'a>) -> Self {
        Self {
            bytes: image.data(),
            width: image.width(),
            height: image.height(),
            straight_alpha: false,
        }
    }

    fn pixel_at(self, x: u32, y: u32) -> Option<[u8; 4]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let index = (y as usize * self.width as usize + x as usize) * 4;
        let [r, g, b, a] = self.bytes[index..index + 4].try_into().ok()?;
        match a {
            0 => Some([0, 0, 0, 0]),
            255 => Some([r, g, b, 255]),
            _ => {
                let alpha = u32::from(a);
                let sample = |value: u8| {
                    let value = u32::from(value);
                    let premultiplied = if self.straight_alpha {
                        value * alpha / 255
                    } else {
                        value
                    };
                    ((premultiplied * 255 + alpha / 2) / alpha).min(255) as u8
                };
                Some([sample(r), sample(g), sample(b), a])
            }
        }
    }
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
    image: Samples<'_>,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    counts: &mut std::collections::HashMap<String, (u32, usize)>,
) {
    for row in y..y.saturating_add(height) {
        for column in x..x.saturating_add(width) {
            let Some(pixel) = image.pixel_at(column, row) else {
                continue;
            };
            if pixel[3] == 0 {
                continue;
            }
            let index = counts.len();
            counts.entry(to_hex(pixel)).or_insert((0, index)).0 += 1;
        }
    }
}

fn dominant(counts: std::collections::HashMap<String, (u32, usize)>) -> Option<String> {
    counts
        .into_iter()
        .max_by_key(|(_, (count, index))| (*count, std::cmp::Reverse(*index)))
        .map(|(color, _)| color)
}

/// `detectDominantEdgeColor` — the most common colour around the border.
fn dominant_edge_color(image: Samples<'_>) -> Option<String> {
    let (width, height) = (image.width, image.height);
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
fn content_bounds(image: Samples<'_>, background: &str) -> Option<ContentBounds> {
    let (width, height) = (image.width, image.height);
    if width == 0 || height == 0 {
        return None;
    }
    let parsed = crate::render::color::parse(background)?.to_color_u8();
    let background = (parsed.red(), parsed.green(), parsed.blue());

    let differs = |x: u32, y: u32| -> bool {
        let Some(pixel) = image.pixel_at(x, y) else {
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

pub fn balance_rgba(image: &image::RgbaImage) -> Option<ContentBounds> {
    balance_bounds(Samples::rgba(image))
}

pub fn balance_pixmap(image: PixmapRef<'_>) -> Option<ContentBounds> {
    balance_bounds(Samples::pixmap(image))
}

fn balance_bounds(image: Samples<'_>) -> Option<ContentBounds> {
    let edge = dominant_edge_color(image)?;
    let bounds = content_bounds(image, &edge)?;
    (bounds != ContentBounds::default()).then_some(bounds)
}

/// `sampleDominantInsetColor` — the colour the inset band is filled with,
/// sampled from the (possibly cropped) image's own border.
pub fn dominant_inset_color(image: PixmapRef<'_>, crop: BalanceCrop) -> Option<String> {
    inset_color(Samples::pixmap(image), crop)
}

pub fn dominant_inset_color_rgba(image: &image::RgbaImage, crop: BalanceCrop) -> Option<String> {
    inset_color(Samples::rgba(image), crop)
}

fn inset_color(image: Samples<'_>, crop: BalanceCrop) -> Option<String> {
    let width = image.width.checked_sub(crop.left + crop.right)?;
    let height = image.height.checked_sub(crop.top + crop.bottom)?;
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
            dominant_edge_color(Samples::pixmap(pixmap.as_ref())).as_deref(),
            Some("#141414")
        );
    }

    #[test]
    fn tied_edge_colours_choose_the_first_sample_consistently() {
        let mut pixmap = Pixmap::new(2, 2).unwrap();
        pixmap.data_mut().copy_from_slice(&[
            240, 80, 64, 255, 64, 112, 192, 255, 64, 112, 192, 255, 240, 80, 64, 255,
        ]);
        for _ in 0..128 {
            assert_eq!(
                dominant_edge_color(Samples::pixmap(pixmap.as_ref())).as_deref(),
                Some("#f05040")
            );
            assert_eq!(
                dominant_inset_color(pixmap.as_ref(), ContentBounds::default()).as_deref(),
                Some("#f05040")
            );
        }
    }

    #[test]
    fn content_bounds_report_the_uniform_margin() {
        let pixmap = image_with_content();
        let bounds = content_bounds(Samples::pixmap(pixmap.as_ref()), "#141414").expect("bounds");
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
            content_bounds(Samples::pixmap(pixmap.as_ref()), "#050505"),
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
