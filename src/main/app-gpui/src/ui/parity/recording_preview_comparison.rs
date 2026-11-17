use crate::video::decoder::DecodedFrame;
use crate::windows::video_editor::drawing_overlay::Rect;

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
) -> (f64, u64) {
    assert!(scale > 0. && bounds.x >= 0. && bounds.y >= 0.);
    assert!(frame.width > 0 && frame.height > 0);
    let pixels = u64::from(frame.width) * u64::from(frame.height);
    assert_eq!(frame.bgra.len() as u64, pixels * 4);
    let left = (bounds.x * scale).round() as u32;
    let top = (bounds.y * scale).round() as u32;
    let right = ((bounds.x + bounds.width) * scale).round() as u32;
    let bottom = ((bounds.y + bounds.height) * scale).round() as u32;
    assert!(right > left && bottom > top);
    assert!(right <= screenshot.width() && bottom <= screenshot.height());
    let crop =
        image::imageops::crop_imm(screenshot, left, top, right - left, bottom - top).to_image();
    let resized = image::imageops::resize(
        &crop,
        frame.width,
        frame.height,
        image::imageops::FilterType::Triangle,
    );
    let mut total = 0_u64;
    let mut above_twelve = 0_u64;
    for (preview, decoded) in resized.pixels().zip(frame.bgra.as_chunks::<4>().0) {
        for (actual, expected) in preview.0[..3]
            .iter()
            .zip([decoded[2], decoded[1], decoded[0]])
        {
            let difference = actual.abs_diff(expected);
            total += u64::from(difference);
            above_twelve += u64::from(difference > 12);
        }
    }
    (total as f64 / (pixels * 3) as f64, above_twelve)
}
