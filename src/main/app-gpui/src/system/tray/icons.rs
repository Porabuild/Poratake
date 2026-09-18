const MONOCHROME_TOLERANCE: u8 = 24;

const TRAY_ICON_BYTES: &[u8] = include_bytes!("../../../../../../public/tray-icon.png");

#[cfg(not(windows))]
const RECORDING_ICON_BYTES: &[u8] =
    include_bytes!("../../../../../../src/main/menu/recording/iconTemplate@2x.png");

fn adapt_monochrome_pixels(image: &mut image::RgbaImage, dark_mode: bool) {
    let value = if dark_mode { 255 } else { 0 };
    for pixel in image.pixels_mut() {
        let [red, green, blue, _] = pixel.0;
        let spread = red.max(green).max(blue) - red.min(green).min(blue);
        if spread <= MONOCHROME_TOLERANCE {
            pixel.0[0] = value;
            pixel.0[1] = value;
            pixel.0[2] = value;
        }
    }
}

pub fn tray_icon(dark_mode: bool, recording: bool) -> Option<tray_icon::Icon> {
    #[cfg(windows)]
    let _ = recording;
    #[cfg(not(windows))]
    if recording {
        return recording_icon();
    }
    let decoded =
        image::load_from_memory_with_format(TRAY_ICON_BYTES, image::ImageFormat::Png).ok()?;
    let mut rgba = decoded.to_rgba8();
    adapt_monochrome_pixels(&mut rgba, dark_mode);
    let (width, height) = rgba.dimensions();
    tray_icon::Icon::from_rgba(rgba.into_raw(), width, height).ok()
}

#[cfg(not(windows))]
fn recording_icon() -> Option<tray_icon::Icon> {
    let decoded =
        image::load_from_memory_with_format(RECORDING_ICON_BYTES, image::ImageFormat::Png).ok()?;
    let rgba = decoded.to_rgba8();
    let (width, height) = rgba.dimensions();
    tray_icon::Icon::from_rgba(rgba.into_raw(), width, height).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_monochrome_pixels_follow_the_resolved_theme() {
        let mut light = image::RgbaImage::from_pixel(1, 1, image::Rgba([245, 240, 242, 128]));
        adapt_monochrome_pixels(&mut light, false);
        assert_eq!(light.get_pixel(0, 0).0, [0, 0, 0, 128]);

        let mut dark = image::RgbaImage::from_pixel(1, 1, image::Rgba([10, 15, 12, 128]));
        adapt_monochrome_pixels(&mut dark, true);
        assert_eq!(dark.get_pixel(0, 0).0, [255, 255, 255, 128]);
    }

    #[test]
    fn tray_colored_pixels_keep_their_color() {
        let mut image = image::RgbaImage::from_pixel(1, 1, image::Rgba([20, 100, 220, 200]));
        adapt_monochrome_pixels(&mut image, false);
        assert_eq!(image.get_pixel(0, 0).0, [20, 100, 220, 200]);
    }

    #[cfg(not(windows))]
    #[test]
    fn the_recording_icon_is_the_camera_glyph() {
        let decoded =
            image::load_from_memory_with_format(RECORDING_ICON_BYTES, image::ImageFormat::Png)
                .expect("decode the recording glyph");
        assert_eq!(decoded.to_rgba8().dimensions(), (44, 44));
        assert!(tray_icon(false, true).is_some());
    }
}
