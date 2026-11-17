use crate::video::decoder::DecodedFrame;
use crate::windows::video_editor::drawing_overlay::Rect;

pub(super) struct Comparison {
    pub mean: f64,
    pub above_twelve: u64,
    pub above_thirty_two: u64,
    pub maximum: u8,
}

pub(super) fn assert_distinct_exports(output: &std::path::Path) {
    let frames: Vec<_> = ["area", "window", "screen"]
        .into_iter()
        .map(|target| {
            let path = output.join(format!("recording-{target}-edited.mp4"));
            crate::video::decoder::VideoDecoder::open(&path)
                .unwrap()
                .frame_at(1.)
                .unwrap()
        })
        .collect();
    for first in 0..frames.len() {
        for second in first + 1..frames.len() {
            assert!(
                frames[first].bgra != frames[second].bgra,
                "different targets must retain their source pixels"
            );
        }
    }
}

pub(super) fn compare(
    screenshot: &image::RgbaImage,
    bounds: Rect,
    scale: f32,
    frame: &DecodedFrame,
) -> Comparison {
    let crop = preview_crop(screenshot, bounds, scale);
    let source = decoded_rgba(frame);
    let resized = image::imageops::resize(
        &crop,
        frame.width,
        frame.height,
        image::imageops::FilterType::Triangle,
    );
    compare_rgb(&resized, &source)
}

pub(super) fn compare_at_display_size(
    screenshot: &image::RgbaImage,
    bounds: Rect,
    scale: f32,
    frame: &DecodedFrame,
) -> Comparison {
    let crop = preview_crop(screenshot, bounds, scale);
    let source = decoded_rgba(frame);
    let resized = image::imageops::resize(
        &source,
        crop.width(),
        crop.height(),
        image::imageops::FilterType::Triangle,
    );
    compare_rgb(&crop, &resized)
}

fn decoded_rgba(frame: &DecodedFrame) -> image::RgbaImage {
    assert!(frame.width > 0 && frame.height > 0);
    let pixels = u64::from(frame.width) * u64::from(frame.height);
    assert_eq!(frame.bgra.len() as u64, pixels * 4);
    let mut rgba = frame.bgra.to_vec();
    for pixel in rgba.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    image::RgbaImage::from_raw(frame.width, frame.height, rgba).unwrap()
}

fn preview_crop(screenshot: &image::RgbaImage, bounds: Rect, scale: f32) -> image::RgbaImage {
    assert!(scale > 0. && bounds.x >= 0. && bounds.y >= 0.);
    let left = (bounds.x * scale).round() as u32;
    let top = (bounds.y * scale).round() as u32;
    let right = ((bounds.x + bounds.width) * scale).round() as u32;
    let bottom = ((bounds.y + bounds.height) * scale).round() as u32;
    assert!(right > left && bottom > top);
    assert!(right <= screenshot.width() && bottom <= screenshot.height());
    image::imageops::crop_imm(screenshot, left, top, right - left, bottom - top).to_image()
}

fn compare_rgb(preview: &image::RgbaImage, reference: &image::RgbaImage) -> Comparison {
    assert_eq!(preview.dimensions(), reference.dimensions());
    let pixels = u64::from(preview.width()) * u64::from(preview.height());
    let mut total = 0_u64;
    let mut above_twelve = 0_u64;
    let mut above_thirty_two = 0_u64;
    let mut maximum = 0;
    for (preview, reference) in preview.pixels().zip(reference.pixels()) {
        for (actual, expected) in preview.0[..3].iter().zip(&reference.0[..3]) {
            let difference = actual.abs_diff(*expected);
            total += u64::from(difference);
            above_twelve += u64::from(difference > 12);
            above_thirty_two += u64::from(difference > 32);
            maximum = maximum.max(difference);
        }
    }
    Comparison {
        mean: total as f64 / (pixels * 3) as f64,
        above_twelve,
        above_thirty_two,
        maximum,
    }
}
