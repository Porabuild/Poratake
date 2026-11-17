use std::sync::Arc;

use crate::editor::export::PreviewEdgeImage;
use herogpui::gpui;

const MAX_IMAGE_SIDE: u32 = 16_384;
const TILE_SIDE: u32 = 8_192;
#[cfg(target_os = "macos")]
pub(super) const TILE_OVERLAP: u32 = (0.5 / crate::editor::options::MIN_ZOOM) as u32 + 1;
#[cfg(not(target_os = "macos"))]
pub(super) const TILE_OVERLAP: u32 = 1;

pub fn create(image: &gpui::RenderImage) -> Vec<PreviewEdgeImage> {
    let size = image.size(0);
    let (width, height) = (u32::from(size.width), u32::from(size.height));
    if width <= MAX_IMAGE_SIDE && height <= MAX_IMAGE_SIDE {
        return Vec::new();
    }
    let Some(source) = image.as_bytes(0) else {
        return Vec::new();
    };
    let mut tiles = Vec::new();
    for y in (0..height).step_by(TILE_SIDE as usize) {
        for x in (0..width).step_by(TILE_SIDE as usize) {
            let right = x.saturating_add(TILE_SIDE).min(width);
            let bottom = y.saturating_add(TILE_SIDE).min(height);
            let left = x.saturating_sub(TILE_OVERLAP);
            let top = y.saturating_sub(TILE_OVERLAP);
            let source_right = right.saturating_add(TILE_OVERLAP).min(width);
            let source_bottom = bottom.saturating_add(TILE_OVERLAP).min(height);
            let tile_width = source_right - left;
            let tile_height = source_bottom - top;
            let mut pixels = image::RgbaImage::new(tile_width, tile_height);
            let target = pixels.as_mut();
            let row_len = tile_width as usize * 4;
            for row in 0..tile_height as usize {
                let source_start = ((top as usize + row) * width as usize + left as usize) * 4;
                let target_start = row * row_len;
                target[target_start..target_start + row_len]
                    .copy_from_slice(&source[source_start..source_start + row_len]);
            }
            tiles.push(PreviewEdgeImage {
                x,
                y,
                width: right - x,
                height: bottom - y,
                inset_x: x - left,
                inset_y: y - top,
                image: Arc::new(gpui::RenderImage::new(smallvec::smallvec![
                    image::Frame::new(pixels)
                ])),
            });
        }
    }
    tiles
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oversized_images_keep_boundary_pixels_and_alpha() {
        for horizontal in [true, false] {
            let (width, height) = if horizontal { (17_000, 3) } else { (3, 17_000) };
            let source = image::RgbaImage::from_fn(width, height, |x, y| {
                let position = if horizontal { x } else { y };
                image::Rgba([(position % 251) as u8, 40, 90, (position % 255) as u8])
            });
            let render =
                gpui::RenderImage::new(smallvec::smallvec![image::Frame::new(source.clone())]);
            let tiles = create(&render);
            assert_eq!(tiles.len(), 3);
            for tile in tiles {
                let size = tile.image.size(0);
                let stride = u32::from(size.width);
                let data = tile.image.as_bytes(0).unwrap();
                for y in 0..u32::from(size.height) {
                    for x in 0..u32::from(size.width) {
                        let index = ((y * stride + x) * 4) as usize;
                        let source_x = tile.x + x - tile.inset_x;
                        let source_y = tile.y + y - tile.inset_y;
                        assert_eq!(
                            &data[index..index + 4],
                            &source.get_pixel(source_x, source_y).0
                        );
                    }
                }
            }
        }
        let small = gpui::RenderImage::new(smallvec::smallvec![image::Frame::new(
            image::RgbaImage::new(64, 64)
        )]);
        assert!(create(&small).is_empty());
    }
}
