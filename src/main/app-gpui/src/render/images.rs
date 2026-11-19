use std::io::{BufRead, Cursor, Seek};
use std::path::Path;

use base64::Engine as _;
use image::{
    ColorType, DynamicImage, ImageBuffer, ImageDecoder, ImageReader, ImageResult, Limits, RgbaImage,
};
use moxcms::{ColorProfile, DataColorSpace, Layout, TransformExecutor};

pub fn open(path: impl AsRef<Path>) -> ImageResult<DynamicImage> {
    decode_reader(ImageReader::open(path)?.with_guessed_format()?, false)
}

pub fn open_rgba8(path: impl AsRef<Path>) -> ImageResult<RgbaImage> {
    decode_reader(ImageReader::open(path)?.with_guessed_format()?, true)
        .map(DynamicImage::into_rgba8)
}

pub fn decode(bytes: &[u8]) -> ImageResult<DynamicImage> {
    decode_reader(
        ImageReader::new(Cursor::new(bytes)).with_guessed_format()?,
        false,
    )
}

pub fn load_source(source: &str) -> Option<DynamicImage> {
    if let Some((_, encoded)) = source.split_once("base64,") {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded.trim())
            .ok()?;
        decode(&bytes).ok()
    } else {
        open(source).ok()
    }
}

pub fn encode_srgb_png(image: &RgbaImage) -> anyhow::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, image.width(), image.height());
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::Fast);
    encoder.set_filter(png::Filter::Adaptive);
    encoder.set_source_srgb(png::SrgbRenderingIntent::RelativeColorimetric);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(image.as_raw())?;
    writer.finish()?;
    Ok(bytes)
}

fn decode_reader(
    reader: ImageReader<impl BufRead + Seek>,
    rgba8: bool,
) -> ImageResult<DynamicImage> {
    let png = reader.format() == Some(image::ImageFormat::Png);
    let mut decoder = reader.into_decoder()?;
    let mut limits = Limits::default();
    let decoded_bytes = decoder.total_bytes();
    limits.reserve(decoded_bytes)?;
    let expand_rgb = rgba8
        && png
        && decoder.color_type() == ColorType::Rgb8
        && limits.reserve(decoded_bytes / 3).is_ok();
    decoder.set_limits(limits)?;
    let profile = decoder
        .icc_profile()
        .ok()
        .flatten()
        .and_then(|bytes| ColorProfile::new_from_slice(&bytes).ok());
    let decoded = if expand_rgb {
        let (width, height) = decoder.dimensions();
        let count = decoded_bytes as usize / 3;
        let mut pixels = vec![0; count * 4];
        decoder.read_image(&mut pixels[..count * 3])?;
        for index in (0..count).rev() {
            let src = index * 3;
            let dst = index * 4;
            let rgba = [pixels[src], pixels[src + 1], pixels[src + 2], 255];
            pixels[dst..dst + 4].copy_from_slice(&rgba);
        }
        DynamicImage::ImageRgba8(RgbaImage::from_raw(width, height, pixels).unwrap())
    } else {
        DynamicImage::from_decoder(decoder)?
    };
    Ok(match profile {
        Some(profile)
            if profile.color_space == DataColorSpace::Rgb && decoded.color().has_color() =>
        {
            convert_rgb_profile(decoded, &profile)
        }
        _ => decoded,
    })
}

fn convert_rgb_profile(decoded: DynamicImage, profile: &ColorProfile) -> DynamicImage {
    let srgb = ColorProfile::new_srgb();
    if matches!(decoded.color(), ColorType::Rgb16 | ColorType::Rgba16) {
        let Ok(transform) =
            profile.create_transform_16bit(Layout::Rgba, &srgb, Layout::Rgba, Default::default())
        else {
            return decoded;
        };
        let rgba = decoded.into_rgba16();
        let (width, height) = rgba.dimensions();
        let pixels = convert_channels(rgba.into_raw(), transform.as_ref());
        return DynamicImage::ImageRgba16(ImageBuffer::from_raw(width, height, pixels).unwrap());
    }

    let Ok(transform) =
        profile.create_transform_8bit(Layout::Rgba, &srgb, Layout::Rgba, Default::default())
    else {
        return decoded;
    };
    let rgba = decoded.into_rgba8();
    let (width, height) = rgba.dimensions();
    let pixels = convert_channels(rgba.into_raw(), transform.as_ref());
    DynamicImage::ImageRgba8(ImageBuffer::from_raw(width, height, pixels).unwrap())
}

fn convert_channels<T: Copy + Default>(
    source: Vec<T>,
    transform: &dyn TransformExecutor<T>,
) -> Vec<T> {
    let mut converted = vec![T::default(); source.len()];
    match transform.transform(&source, &mut converted) {
        Ok(()) => converted,
        Err(_) => source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ExtendedColorType, ImageEncoder};

    fn tagged_pixel(profile: Option<ColorProfile>, pixel: [u8; 4]) -> Vec<u8> {
        let mut bytes = Vec::new();
        let mut encoder = image::codecs::png::PngEncoder::new(&mut bytes);
        if let Some(profile) = profile {
            encoder.set_icc_profile(profile.encode().unwrap()).unwrap();
        }
        encoder
            .write_image(&pixel, 1, 1, ExtendedColorType::Rgba8)
            .unwrap();
        bytes
    }

    #[test]
    fn converts_display_p3_pixels_to_srgb_and_keeps_straight_alpha() {
        let bytes = tagged_pixel(Some(ColorProfile::new_display_p3()), [100, 204, 101, 128]);
        let pixel = decode(&bytes).unwrap().into_rgba8().get_pixel(0, 0).0;
        for (actual, expected) in pixel[..3].iter().zip([39, 207, 86]) {
            assert!(actual.abs_diff(expected) <= 2, "{pixel:?}");
        }
        assert_eq!(pixel[3], 128);
    }

    #[test]
    fn keeps_untagged_and_srgb_pixel_values() {
        let pixel = [35, 108, 207, 127];
        for profile in [None, Some(ColorProfile::new_srgb())] {
            let bytes = tagged_pixel(profile, pixel);
            assert_eq!(
                decode(&bytes).unwrap().into_rgba8().get_pixel(0, 0).0,
                pixel
            );
        }
    }

    #[test]
    fn expands_rgb_to_rgba_without_changing_screenshot_pixels() {
        let pixels = [35, 108, 207, 10, 81, 190, 100, 204, 101, 199, 72, 3];
        for profile in [None, Some(ColorProfile::new_display_p3())] {
            let mut bytes = Vec::new();
            let mut encoder = image::codecs::png::PngEncoder::new(&mut bytes);
            if let Some(profile) = profile {
                encoder.set_icc_profile(profile.encode().unwrap()).unwrap();
            }
            encoder
                .write_image(&pixels, 2, 2, ExtendedColorType::Rgb8)
                .unwrap();
            let reader = ImageReader::new(Cursor::new(&bytes))
                .with_guessed_format()
                .unwrap();
            let expanded = decode_reader(reader, true).unwrap();
            assert!(expanded.as_rgba8().is_some());
            assert_eq!(expanded.into_rgba8(), decode(&bytes).unwrap().into_rgba8());
        }
    }

    #[test]
    fn writes_the_srgb_profile_with_the_original_alpha_and_pixels() {
        let pixel = [39, 207, 86, 128];
        let image = RgbaImage::from_pixel(1, 1, image::Rgba(pixel));
        let bytes = encode_srgb_png(&image).unwrap();
        let decoder = png::Decoder::new(Cursor::new(&bytes)).read_info().unwrap();
        assert_eq!(
            decoder.info().srgb,
            Some(png::SrgbRenderingIntent::RelativeColorimetric)
        );
        assert_eq!(bytes, encode_srgb_png(&image).unwrap());
        assert_eq!(decode(&bytes).unwrap().into_rgba8(), image);
    }

    #[test]
    fn keeps_sixteen_bit_precision_and_alpha() {
        let pixel: [u16; 4] = [25_700, 52_428, 25_957, 32_769];
        let bytes: Vec<u8> = pixel.into_iter().flat_map(u16::to_ne_bytes).collect();
        let mut png = Vec::new();
        let mut encoder = image::codecs::png::PngEncoder::new(&mut png);
        encoder
            .set_icc_profile(ColorProfile::new_display_p3().encode().unwrap())
            .unwrap();
        encoder
            .write_image(&bytes, 1, 1, ExtendedColorType::Rgba16)
            .unwrap();
        let DynamicImage::ImageRgba16(converted) = decode(&png).unwrap() else {
            panic!("The converted image must still hold 16-bit samples");
        };
        let converted = converted.get_pixel(0, 0).0;
        assert!(converted[0] < pixel[0]);
        assert_eq!(converted[3], pixel[3]);
    }
}
