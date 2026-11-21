use tiny_skia::Pixmap;

pub fn rgba(pixmap: &Pixmap) -> Vec<u8> {
    let mut bytes = pixmap.data().to_vec();
    for pixel in bytes.as_chunks_mut::<4>().0 {
        pixel[3] = 255;
    }
    bytes
}

pub fn bgra(pixmap: &Pixmap) -> Vec<u8> {
    let mut out = Vec::new();
    bgra_into(pixmap, &mut out);
    out
}

pub fn bgra_into(pixmap: &Pixmap, out: &mut Vec<u8>) {
    let source = pixmap.data();
    out.resize(source.len(), 0);
    copy_rgba_to_opaque_bgra(source, out);
}

pub(crate) fn copy_bgra_to_rgba(source: &[u8], target: &mut [u8]) {
    swap_red_and_blue(source, target, false);
}

pub(crate) fn copy_rgba_to_opaque_bgra(source: &[u8], target: &mut [u8]) {
    swap_red_and_blue(source, target, true);
}

fn swap_red_and_blue(source: &[u8], target: &mut [u8], opaque: bool) {
    let count = source.len().min(target.len()) / 4;
    for (out, pixel) in target[..count * 4]
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(source[..count * 4].as_chunks::<4>().0)
    {
        let packed = u32::from_le_bytes(*pixel);
        let swapped = (packed & 0xFF00_FF00)
            | ((packed & 0x00FF_0000) >> 16)
            | ((packed & 0x0000_00FF) << 16);
        let packed = if opaque {
            (swapped & 0x00FF_FFFF) | 0xFF00_0000
        } else {
            swapped
        };
        *out = packed.to_le_bytes();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transparent_pixels_are_composited_over_black() {
        let mut pixmap = Pixmap::new(3, 1).unwrap();
        pixmap
            .data_mut()
            .copy_from_slice(&[64, 32, 16, 128, 1, 1, 1, 1, 0, 0, 0, 0]);
        assert_eq!(rgba(&pixmap), [64, 32, 16, 255, 1, 1, 1, 255, 0, 0, 0, 255]);
        assert_eq!(bgra(&pixmap), [16, 32, 64, 255, 1, 1, 1, 255, 0, 0, 0, 255]);
        let mut round = [0u8; 12];
        copy_bgra_to_rgba(&[16, 32, 64, 255, 1, 1, 1, 128, 0, 0, 0, 7], &mut round);
        assert_eq!(round, [64, 32, 16, 255, 1, 1, 1, 128, 0, 0, 0, 7]);
        let mut reused = Vec::new();
        bgra_into(&pixmap, &mut reused);
        assert_eq!(reused, [16, 32, 64, 255, 1, 1, 1, 255, 0, 0, 0, 255]);
    }
}
