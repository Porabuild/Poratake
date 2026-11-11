//! Separable box blur, the standard three-pass approximation of the Gaussian
//! that `ctx.filter = blur()` and `ctx.shadowBlur` apply in the renderer.

use tiny_skia::Pixmap;

/// Canvas maps `shadowBlur` to a Gaussian with `sigma = blur / 2`.
pub fn sigma_for_shadow_blur(blur: f32) -> f32 {
    blur / 2.0
}

/// The box width that best approximates a Gaussian of `sigma` over three
/// passes, following the same derivation browsers use.
pub fn box_radius(sigma: f32) -> usize {
    if sigma <= 0.0 {
        return 0;
    }
    ((sigma * 3.0 * (2.0 * std::f32::consts::PI).sqrt() / 4.0) + 0.5).floor() as usize
}

/// Blurs `pixmap` in place. The buffer is premultiplied BGRA, which box blur
/// handles correctly because premultiplied channels are linear in coverage.
pub fn blur(pixmap: &mut Pixmap, sigma: f32) {
    let radius = box_radius(sigma);
    if radius == 0 {
        return;
    }
    let width = pixmap.width() as usize;
    let height = pixmap.height() as usize;
    if width == 0 || height == 0 {
        return;
    }

    let mut data: Vec<u8> = pixmap.data().to_vec();
    let mut scratch = vec![0u8; data.len()];
    for _ in 0..3 {
        box_blur_horizontal(&data, &mut scratch, width, height, radius);
        box_blur_vertical(&scratch, &mut data, width, height, radius);
    }
    pixmap.data_mut().copy_from_slice(&data);
}

pub fn blur_alpha(pixmap: &mut Pixmap, sigma: f32) {
    let radius = box_radius(sigma);
    if radius == 0 {
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
    let window = (radius * 2 + 1) as u32;
    for _ in 0..3 {
        for y in 0..height {
            let row = y * width;
            let mut sum = u32::from(data[row]) * radius as u32;
            for x in 0..=radius.min(width - 1) {
                sum += u32::from(data[row + x]);
            }
            for x in 0..width {
                scratch[row + x] = (sum / window) as u8;
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
        for y in 0..height {
            let row = y * width;
            let leaving = y.saturating_sub(radius) * width;
            let entering = (y + radius + 1).min(height - 1) * width;
            for x in 0..width {
                data[row + x] = (sums[x] / window) as u8;
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
    radius: usize,
) {
    let window = (radius * 2 + 1) as u32;
    for y in 0..height {
        let row = y * width * 4;
        let mut sums = [0u32; 4];
        for x in 0..=radius.min(width - 1) {
            accumulate(&mut sums, source, row + x * 4);
        }
        // Pixels off the left edge repeat the first pixel, matching the clamp
        // the canvas filter uses at the source bounds.
        for _ in 0..radius {
            accumulate(&mut sums, source, row);
        }
        for x in 0..width {
            for channel in 0..4 {
                target[row + x * 4 + channel] = (sums[channel] / window) as u8;
            }
            let leaving = row + x.saturating_sub(radius) * 4;
            let entering = row + (x + radius + 1).min(width - 1) * 4;
            deaccumulate(&mut sums, source, leaving);
            accumulate(&mut sums, source, entering);
        }
    }
}

fn box_blur_vertical(source: &[u8], target: &mut [u8], width: usize, height: usize, radius: usize) {
    let window = (radius * 2 + 1) as u32;
    let stride = width * 4;
    for x in 0..width {
        let column = x * 4;
        let mut sums = [0u32; 4];
        for y in 0..=radius.min(height - 1) {
            accumulate(&mut sums, source, column + y * stride);
        }
        for _ in 0..radius {
            accumulate(&mut sums, source, column);
        }
        for y in 0..height {
            for channel in 0..4 {
                target[column + y * stride + channel] = (sums[channel] / window) as u8;
            }
            let leaving = column + y.saturating_sub(radius) * stride;
            let entering = column + (y + radius + 1).min(height - 1) * stride;
            deaccumulate(&mut sums, source, leaving);
            accumulate(&mut sums, source, entering);
        }
    }
}

fn accumulate(sums: &mut [u32; 4], source: &[u8], offset: usize) {
    for channel in 0..4 {
        sums[channel] += source[offset + channel] as u32;
    }
}

fn deaccumulate(sums: &mut [u32; 4], source: &[u8], offset: usize) {
    for channel in 0..4 {
        sums[channel] = sums[channel].saturating_sub(source[offset + channel] as u32);
    }
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
