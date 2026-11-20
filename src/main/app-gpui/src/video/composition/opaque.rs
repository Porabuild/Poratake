use tiny_skia::Pixmap;

fn pixels(pixmap: &Pixmap, red: usize, blue: usize) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(pixmap.data().len());
    for pixel in pixmap.data().as_chunks::<4>().0 {
        bytes.extend_from_slice(&[pixel[red], pixel[1], pixel[blue], 255]);
    }
    bytes
}

pub fn rgba(pixmap: &Pixmap) -> Vec<u8> {
    pixels(pixmap, 0, 2)
}

pub fn bgra(pixmap: &Pixmap) -> Vec<u8> {
    let mut out = Vec::new();
    bgra_into(pixmap, &mut out);
    out
}

pub fn bgra_into(pixmap: &Pixmap, out: &mut Vec<u8>) {
    let source = pixmap.data();
    out.resize(source.len(), 0);
    for (target, pixel) in out.as_chunks_mut::<4>().0.iter_mut().zip(source.as_chunks::<4>().0) {
        target[0] = pixel[2];
        target[1] = pixel[1];
        target[2] = pixel[0];
        target[3] = 255;
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
        let mut reused = Vec::new();
        bgra_into(&pixmap, &mut reused);
        assert_eq!(reused, [16, 32, 64, 255, 1, 1, 1, 255, 0, 0, 0, 255]);
    }
}
