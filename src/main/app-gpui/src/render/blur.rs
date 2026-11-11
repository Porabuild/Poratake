//! Separable box blur, the standard three-pass approximation of the Gaussian
//! that `ctx.filter = blur()` and `ctx.shadowBlur` apply in the renderer.

use tiny_skia::Pixmap;

/// Canvas maps `shadowBlur` to a Gaussian with `sigma = blur / 2`.
pub fn sigma_for_shadow_blur(blur: f32) -> f32 {
    blur / 2.0
}

pub fn box_radius(sigma: f32) -> usize {
    if sigma <= 0.0 {
        return 0;
    }
    let kernel = box_kernel(sigma);
    kernel.radius + usize::from(kernel.edge > 0)
}

#[derive(Clone, Copy)]
struct BoxKernel {
    radius: usize,
    edge: u32,
    denominator: u32,
}

fn box_kernel(sigma: f32) -> BoxKernel {
    let variance = sigma * sigma;
    let radius = (((1.0 + 4.0 * variance).sqrt() - 1.0) * 0.5).floor() as usize;
    let r = radius as f32;
    let edge = ((2.0 * r + 1.0) * (variance - r * (r + 1.0))
        / (2.0 * (3.0 * (r + 1.0).powi(2) - variance)))
        .clamp(0.0, 1.0);
    let edge = (edge * 1024.0).round() as u32;
    let denominator = (radius as u32 * 2 + 1) * 1024 + 2 * edge;
    BoxKernel {
        radius,
        edge,
        denominator,
    }
}

/// Blurs `pixmap` in place. The buffer is premultiplied BGRA, which box blur
/// handles correctly because premultiplied channels are linear in coverage.
pub fn blur(pixmap: &mut Pixmap, sigma: f32) {
    if sigma <= 0.0 {
        return;
    }
    let width = pixmap.width() as usize;
    let height = pixmap.height() as usize;
    if width == 0 || height == 0 {
        return;
    }

    let mut data: Vec<u8> = pixmap.data().to_vec();
    let mut scratch = vec![0u8; data.len()];
    let kernel = box_kernel(sigma);
    for _ in 0..3 {
        box_blur_horizontal(&data, &mut scratch, width, height, kernel);
        box_blur_vertical(&scratch, &mut data, width, height, kernel);
    }
    pixmap.data_mut().copy_from_slice(&data);
}

pub fn blur_transparent(pixmap: &mut Pixmap, sigma: f32) {
    if sigma <= 0.0 {
        return;
    }
    let width = pixmap.width() as usize;
    let height = pixmap.height() as usize;
    let kernel = box_kernel(sigma);
    let guard = box_radius(sigma) * 2;
    let max_line = (width.max(height) + guard * 2) * 4;
    let mut first = vec![0u8; max_line];
    let mut second = vec![0u8; max_line];
    let mut horizontal = vec![0u8; width * height * 4];
    let row_width = width + guard * 2;
    let row_bytes = width * 4;
    for y in 0..height {
        first[..row_width * 4].fill(0);
        let offset = y * row_bytes;
        first[guard * 4..guard * 4 + row_bytes]
            .copy_from_slice(&pixmap.data()[offset..offset + row_bytes]);
        for _ in 0..3 {
            box_blur_horizontal(&first, &mut second, row_width, 1, kernel);
            std::mem::swap(&mut first, &mut second);
        }
        horizontal[offset..offset + row_bytes]
            .copy_from_slice(&first[guard * 4..guard * 4 + row_bytes]);
    }
    let column_width = height + guard * 2;
    for x in 0..width {
        first[..column_width * 4].fill(0);
        for y in 0..height {
            let from = (y * width + x) * 4;
            let to = (y + guard) * 4;
            first[to..to + 4].copy_from_slice(&horizontal[from..from + 4]);
        }
        for _ in 0..3 {
            box_blur_horizontal(&first, &mut second, column_width, 1, kernel);
            std::mem::swap(&mut first, &mut second);
        }
        for y in 0..height {
            let from = (y + guard) * 4;
            let to = (y * width + x) * 4;
            pixmap.data_mut()[to..to + 4].copy_from_slice(&first[from..from + 4]);
        }
    }
}

pub fn blur_alpha(pixmap: &mut Pixmap, sigma: f32) {
    if sigma <= 0.0 {
        return;
    }
    let width = pixmap.width() as usize;
    let height = pixmap.height() as usize;
    let mut data: Vec<u8> = pixmap
        .data()
        .as_chunks::<4>()
        .0
        .iter()
        .map(|pixel| pixel[3])
        .collect();
    let mut scratch = vec![0u8; data.len()];
    let mut sums = vec![0u32; width];
    let kernel = box_kernel(sigma);
    let radius = kernel.radius;
    for _ in 0..3 {
        for y in 0..height {
            let row = y * width;
            let mut sum = u32::from(data[row]) * radius as u32;
            for x in 0..=radius.min(width - 1) {
                sum += u32::from(data[row + x]);
            }
            sum += u32::from(data[row + width - 1]) * radius.saturating_sub(width - 1) as u32;
            for x in 0..width {
                scratch[row + x] = filtered(
                    sum,
                    data[row + x.saturating_sub(radius + 1)],
                    data[row + (x + radius + 1).min(width - 1)],
                    kernel,
                );
                sum = sum.saturating_sub(u32::from(data[row + x.saturating_sub(radius)]))
                    + u32::from(data[row + (x + radius + 1).min(width - 1)]);
            }
        }
        for x in 0..width {
            sums[x] = u32::from(scratch[x]) * radius as u32;
        }
        for y in 0..=radius.min(height - 1) {
            for x in 0..width {
                sums[x] += u32::from(scratch[y * width + x]);
            }
        }
        for x in 0..width {
            sums[x] += u32::from(scratch[(height - 1) * width + x])
                * radius.saturating_sub(height - 1) as u32;
        }
        for y in 0..height {
            let row = y * width;
            let leaving = y.saturating_sub(radius) * width;
            let entering = (y + radius + 1).min(height - 1) * width;
            let outer_leaving = y.saturating_sub(radius + 1) * width;
            for x in 0..width {
                data[row + x] = filtered(
                    sums[x],
                    scratch[outer_leaving + x],
                    scratch[entering + x],
                    kernel,
                );
                sums[x] = sums[x].saturating_sub(u32::from(scratch[leaving + x]))
                    + u32::from(scratch[entering + x]);
            }
        }
    }
    for (pixel, alpha) in pixmap
        .data_mut()
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(data)
    {
        pixel[3] = alpha;
    }
}

fn box_blur_horizontal(
    source: &[u8],
    target: &mut [u8],
    width: usize,
    height: usize,
    kernel: BoxKernel,
) {
    let radius = kernel.radius;
    let source = source.as_chunks::<4>().0;
    let target = target.as_chunks_mut::<4>().0;
    for y in 0..height {
        let row = y * width;
        let mut sums = [0u32; 4];
        for x in 0..=radius.min(width - 1) {
            for channel in 0..4 {
                sums[channel] += u32::from(source[row + x][channel]);
            }
        }
        for channel in 0..4 {
            sums[channel] += u32::from(source[row][channel]) * radius as u32
                + u32::from(source[row + width - 1][channel])
                    * radius.saturating_sub(width - 1) as u32;
        }
        for x in 0..width {
            let first = &source[row + x.saturating_sub(radius + 1)];
            let last = &source[row + (x + radius + 1).min(width - 1)];
            let leaving = &source[row + x.saturating_sub(radius)];
            for channel in 0..4 {
                target[row + x][channel] =
                    filtered(sums[channel], first[channel], last[channel], kernel);
                sums[channel] = sums[channel].saturating_sub(u32::from(leaving[channel]))
                    + u32::from(last[channel]);
            }
        }
    }
}

fn box_blur_vertical(
    source: &[u8],
    target: &mut [u8],
    width: usize,
    height: usize,
    kernel: BoxKernel,
) {
    let radius = kernel.radius;
    let source = source.as_chunks::<4>().0;
    let target = target.as_chunks_mut::<4>().0;
    for x in 0..width {
        let mut sums = [0u32; 4];
        for y in 0..=radius.min(height - 1) {
            for channel in 0..4 {
                sums[channel] += u32::from(source[y * width + x][channel]);
            }
        }
        for channel in 0..4 {
            sums[channel] += u32::from(source[x][channel]) * radius as u32
                + u32::from(source[(height - 1) * width + x][channel])
                    * radius.saturating_sub(height - 1) as u32;
        }
        for y in 0..height {
            let first = &source[y.saturating_sub(radius + 1) * width + x];
            let last = &source[(y + radius + 1).min(height - 1) * width + x];
            let leaving = &source[y.saturating_sub(radius) * width + x];
            for channel in 0..4 {
                target[y * width + x][channel] =
                    filtered(sums[channel], first[channel], last[channel], kernel);
                sums[channel] = sums[channel].saturating_sub(u32::from(leaving[channel]))
                    + u32::from(last[channel]);
            }
        }
    }
}

#[inline]
fn filtered(sum: u32, first: u8, last: u8, kernel: BoxKernel) -> u8 {
    let value = sum * 1024 + (u32::from(first) + u32::from(last)) * kernel.edge;
    ((value + kernel.denominator / 2) / kernel.denominator) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use tiny_skia::{Color, Paint, Rect, Transform};

    #[test]
    fn a_zero_sigma_is_a_no_op() {
        let mut pixmap = Pixmap::new(8, 8).expect("pixmap");
        pixmap.fill(Color::from_rgba8(255, 0, 0, 255));
        let before = pixmap.data().to_vec();
        blur(&mut pixmap, 0.0);
        assert_eq!(pixmap.data(), &before[..]);
    }

    #[test]
    fn a_uniform_image_stays_uniform_when_blur_exceeds_its_dimensions() {
        for (width, height) in [(1, 1), (2, 3), (3, 1), (1, 30), (30, 1)] {
            let mut image = Pixmap::new(width, height).unwrap();
            image.fill(Color::from_rgba8(160, 80, 40, 255));
            let before = image.data().to_vec();
            blur(&mut image, 20.0);
            assert_eq!(image.data(), &before, "{width}x{height}");
            blur_alpha(&mut image, 20.0);
            assert_eq!(image.data(), &before, "alpha {width}x{height}");
        }
    }

    #[test]
    fn small_images_repeat_both_edges_during_blur() {
        for (width, height) in [(2, 1), (1, 2)] {
            let source = [0, 0, 0, 255, 255, 255, 255, 255];
            let mut result = [0; 8];
            let kernel = BoxKernel {
                radius: 2,
                edge: 0,
                denominator: 5120,
            };
            if width == 2 {
                box_blur_horizontal(&source, &mut result, width, height, kernel);
            } else {
                box_blur_vertical(&source, &mut result, width, height, kernel);
            }
            assert_eq!(result, [102, 102, 102, 255, 153, 153, 153, 255]);
        }
    }

    #[test]
    fn blur_approximates_electron_at_small_sigmas() {
        for (sigma, expected) in [
            (0.5, [0, 0, 0, 27, 228, 255, 255, 255, 255]),
            (1.0, [0, 1, 15, 76, 178, 240, 254, 255, 255]),
            (1.5, [0, 11, 39, 94, 161, 216, 244, 253, 255]),
            (2.0, [3, 26, 57, 102, 153, 198, 229, 245, 254]),
            (3.0, [17, 51, 78, 110, 145, 177, 204, 224, 247]),
        ] {
            let mut image = Pixmap::new(256, 128).unwrap();
            image.fill(Color::BLACK);
            for y in 0..128 {
                for x in 128..256 {
                    let offset = (y * 256 + x) as usize * 4;
                    image.data_mut()[offset..offset + 4].copy_from_slice(&[255; 4]);
                }
            }
            blur(&mut image, sigma);
            for (offset, expected) in [-5, -3, -2, -1, 0, 1, 2, 3, 5].into_iter().zip(expected) {
                let actual = image.pixel((128 + offset) as u32, 64).unwrap().red();
                assert!(
                    actual.abs_diff(expected) <= 3,
                    "sigma={sigma}, offset={offset}: {actual} vs {expected}"
                );
            }
        }
    }

    #[test]
    fn transparent_blur_matches_a_padded_reference() {
        for (width, height, sigma) in [
            (1, 1, 0.5),
            (2, 3, 1.0),
            (19, 7, 4.0),
            (31, 17, 8.0),
            (200, 1, 10.0),
        ] {
            let mut original = Pixmap::new(width, height).unwrap();
            for (index, pixel) in original
                .data_mut()
                .as_chunks_mut::<4>()
                .0
                .iter_mut()
                .enumerate()
            {
                let alpha = (index.wrapping_mul(77).wrapping_add(31) % 256) as u8;
                pixel.copy_from_slice(&[alpha / 2, alpha / 3, alpha / 4, alpha]);
            }
            let guard = (box_radius(sigma) * 3 + 1) as u32;
            let mut expected = Pixmap::new(width + guard * 2, height + guard * 2).unwrap();
            expected.draw_pixmap(
                guard as i32,
                guard as i32,
                original.as_ref(),
                &tiny_skia::PixmapPaint::default(),
                Transform::identity(),
                None,
            );
            blur(&mut expected, sigma);
            blur_transparent(&mut original, sigma);
            for y in 0..height {
                for x in 0..width {
                    let actual = original.pixel(x, y).unwrap();
                    let expected = expected.pixel(x + guard, y + guard).unwrap();
                    for (actual, expected) in [
                        (actual.red(), expected.red()),
                        (actual.green(), expected.green()),
                        (actual.blue(), expected.blue()),
                        (actual.alpha(), expected.alpha()),
                    ] {
                        assert!(
                            actual.abs_diff(expected) <= 2,
                            "{width}x{height}, sigma={sigma}, {x},{y}: {actual} vs {expected}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn alpha_blur_matches_the_full_color_blur() {
        for width in [1, 2, 7, 31] {
            for height in [1, 3, 10, 33] {
                for sigma in [0.0, 0.5, 2.0, 7.0, 20.0] {
                    let mut original = Pixmap::new(width, height).unwrap();
                    for (index, pixel) in original
                        .data_mut()
                        .as_chunks_mut::<4>()
                        .0
                        .iter_mut()
                        .enumerate()
                    {
                        let alpha = (index.wrapping_mul(77).wrapping_add(31) % 256) as u8;
                        pixel.copy_from_slice(&[alpha / 2, alpha / 3, alpha / 4, alpha]);
                    }
                    let mut full = original.clone();
                    let mut alpha = original;
                    blur(&mut full, sigma);
                    blur_alpha(&mut alpha, sigma);
                    assert!(
                        full.data()
                            .as_chunks::<4>()
                            .0
                            .iter()
                            .zip(alpha.data().as_chunks::<4>().0.iter())
                            .all(|(full, alpha)| full[3] == alpha[3]),
                        "{width}x{height}, sigma {sigma}"
                    );
                }
            }
        }
    }

    #[test]
    fn spreads_coverage_beyond_the_source_rect() {
        let mut pixmap = Pixmap::new(32, 32).expect("pixmap");
        let mut paint = Paint::default();
        paint.set_color(Color::from_rgba8(0, 0, 0, 255));
        pixmap.fill_rect(
            Rect::from_xywh(12.0, 12.0, 8.0, 8.0).expect("rect"),
            &paint,
            Transform::identity(),
            None,
        );

        let outside = |pixmap: &Pixmap| pixmap.data()[(6 * 32 + 6) * 4 + 3];
        assert_eq!(outside(&pixmap), 0);
        blur(&mut pixmap, 4.0);
        assert!(outside(&pixmap) > 0);
    }

    #[test]
    fn derives_the_browser_box_radius() {
        assert_eq!(box_radius(0.0), 0);
        assert!(box_radius(4.0) >= 3);
        assert_eq!(sigma_for_shadow_blur(10.0), 5.0);
    }
}
