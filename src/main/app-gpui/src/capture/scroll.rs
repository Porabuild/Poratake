//! Scroll capture — port of `capture/scroll-capture/index.ts`. On Windows the
//! daemon owns the on-screen control panel (`daemon-win/src/panel.rs`) and
//! reports `scroll-capture:done` / `:cancelled`, so the app only picks the
//! area, starts the session and finalizes the stitched image.

use std::sync::atomic::{AtomicBool, Ordering};

use poratake_daemon_common::contract::{ScrollCaptureFinishRequest, ScrollCaptureStartRequest};

use crate::daemon::DaemonHandle;

static ACTIVE: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Debug)]
pub enum ScrollSessionSignal {
    Finish,
    Cancel,
    Stopped(&'static str),
    #[cfg(any(target_os = "macos", test, all(windows, feature = "parity")))]
    Frame {
        preview: Option<String>,
    },
    #[cfg(any(target_os = "macos", test, all(windows, feature = "parity")))]
    AutoScrolling(bool),
    #[cfg(any(target_os = "macos", test, all(windows, feature = "parity")))]
    CursorOutside(bool),
}

impl ScrollSessionSignal {
    pub fn from_daemon_event(event: &str, payload: &serde_json::Value) -> Option<Self> {
        #[cfg(target_os = "macos")]
        use poratake_daemon_common::contract::{
            SCROLL_CAPTURE_AUTO_SCROLL_EVENT, SCROLL_CAPTURE_CURSOR_EVENT,
            SCROLL_CAPTURE_FRAME_PREVIEW_EVENT,
        };
        use poratake_daemon_common::contract::{
            SCROLL_CAPTURE_CANCELLED_EVENT, SCROLL_CAPTURE_DONE_EVENT,
            SCROLL_CAPTURE_SCROLL_ENDED_EVENT, SYSTEM_EXIT_EVENT,
        };

        match event {
            SCROLL_CAPTURE_DONE_EVENT => Some(Self::Finish),
            SCROLL_CAPTURE_CANCELLED_EVENT | SYSTEM_EXIT_EVENT => Some(Self::Cancel),
            SCROLL_CAPTURE_SCROLL_ENDED_EVENT => {
                match payload.get("reason").and_then(serde_json::Value::as_str) {
                    Some("no-overlap") => Some(Self::Stopped(
                        "The latest frame could not be aligned. You can save the captured portion.",
                    )),
                    Some("capture-error") => Some(Self::Stopped(
                        "A frame could not be captured. You can save the captured portion.",
                    )),
                    Some("input-error") => Some(Self::Stopped(
                        "Scrolling could not be continued. You can save the captured portion.",
                    )),
                    _ => None,
                }
            }
            #[cfg(target_os = "macos")]
            SCROLL_CAPTURE_FRAME_PREVIEW_EVENT => Some(Self::Frame {
                preview: payload
                    .get("preview")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string),
            }),
            #[cfg(target_os = "macos")]
            SCROLL_CAPTURE_AUTO_SCROLL_EVENT => Some(Self::AutoScrolling(
                payload
                    .get("scrolling")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false),
            )),
            #[cfg(target_os = "macos")]
            SCROLL_CAPTURE_CURSOR_EVENT => Some(Self::CursorOutside(
                payload
                    .get("outside")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false),
            )),
            _ => None,
        }
    }
}

pub fn is_active() -> bool {
    ACTIVE.load(Ordering::SeqCst)
}

pub fn start(daemon: &DaemonHandle, request: &ScrollCaptureStartRequest) -> bool {
    if ACTIVE.swap(true, Ordering::SeqCst) {
        return false;
    }
    let called = daemon.scroll_capture().start(request);
    if let Err(error) = called {
        eprintln!("[scroll-capture] start failed: {error}");
        ACTIVE.store(false, Ordering::SeqCst);
        return false;
    }
    true
}

/// Stitches and writes the capture, returning the file the daemon produced.
pub fn finish(daemon: &DaemonHandle, output_path: &std::path::Path) -> Option<std::path::PathBuf> {
    let response = daemon.scroll_capture().finish(&ScrollCaptureFinishRequest {
        output_path: output_path.to_path_buf(),
    });
    ACTIVE.store(false, Ordering::SeqCst);
    let response = response
        .map_err(|error| eprintln!("[scroll-capture] finish failed: {error}"))
        .ok()?;

    if !response.success {
        return None;
    }
    Some(response.output_path)
}

pub fn cancel(daemon: &DaemonHandle) {
    if !is_active() {
        return;
    }
    let result = daemon.scroll_capture().cancel();
    ACTIVE.store(false, Ordering::SeqCst);
    if let Err(error) = result {
        eprintln!("[scroll-capture] cancel failed: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interruption_events_distinguish_errors_from_normal_stops() {
        use poratake_daemon_common::contract::{
            SCROLL_CAPTURE_CANCELLED_EVENT, SCROLL_CAPTURE_DONE_EVENT,
            SCROLL_CAPTURE_SCROLL_ENDED_EVENT, SYSTEM_EXIT_EVENT,
        };

        for (reason, expected) in [
            (
                "no-overlap",
                "The latest frame could not be aligned. You can save the captured portion.",
            ),
            (
                "capture-error",
                "A frame could not be captured. You can save the captured portion.",
            ),
            (
                "input-error",
                "Scrolling could not be continued. You can save the captured portion.",
            ),
        ] {
            let event = ScrollSessionSignal::from_daemon_event(
                SCROLL_CAPTURE_SCROLL_ENDED_EVENT,
                &serde_json::json!({ "reason": reason, "frameCount": 2 }),
            );
            assert!(
                matches!(event, Some(ScrollSessionSignal::Stopped(message)) if message == expected)
            );
        }
        for reason in ["duplicate", "max-height", "unknown"] {
            assert!(ScrollSessionSignal::from_daemon_event(
                SCROLL_CAPTURE_SCROLL_ENDED_EVENT,
                &serde_json::json!({ "reason": reason }),
            )
            .is_none());
        }
        assert!(ScrollSessionSignal::from_daemon_event(
            SCROLL_CAPTURE_SCROLL_ENDED_EVENT,
            &serde_json::Value::Null,
        )
        .is_none());
        assert!(matches!(
            ScrollSessionSignal::from_daemon_event(
                SCROLL_CAPTURE_DONE_EVENT,
                &serde_json::Value::Null
            ),
            Some(ScrollSessionSignal::Finish)
        ));
        for event in [SCROLL_CAPTURE_CANCELLED_EVENT, SYSTEM_EXIT_EVENT] {
            assert!(matches!(
                ScrollSessionSignal::from_daemon_event(event, &serde_json::Value::Null),
                Some(ScrollSessionSignal::Cancel)
            ));
        }
        assert!(
            ScrollSessionSignal::from_daemon_event("unknown", &serde_json::Value::Null).is_none()
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_progress_events_keep_their_payloads() {
        use poratake_daemon_common::contract::{
            SCROLL_CAPTURE_AUTO_SCROLL_EVENT, SCROLL_CAPTURE_CURSOR_EVENT,
            SCROLL_CAPTURE_FRAME_PREVIEW_EVENT,
        };

        assert!(matches!(
            ScrollSessionSignal::from_daemon_event(
                SCROLL_CAPTURE_FRAME_PREVIEW_EVENT,
                &serde_json::json!({ "preview": "image" }),
            ),
            Some(ScrollSessionSignal::Frame { preview: Some(preview) }) if preview == "image"
        ));
        assert!(matches!(
            ScrollSessionSignal::from_daemon_event(
                SCROLL_CAPTURE_AUTO_SCROLL_EVENT,
                &serde_json::json!({ "scrolling": true }),
            ),
            Some(ScrollSessionSignal::AutoScrolling(true))
        ));
        assert!(matches!(
            ScrollSessionSignal::from_daemon_event(
                SCROLL_CAPTURE_CURSOR_EVENT,
                &serde_json::json!({ "outside": true }),
            ),
            Some(ScrollSessionSignal::CursorOutside(true))
        ));
    }

    #[test]
    fn tracks_a_single_active_session() {
        assert!(!is_active());
        ACTIVE.store(true, Ordering::SeqCst);
        assert!(is_active());
        ACTIVE.store(false, Ordering::SeqCst);
        assert!(!is_active());
    }

    #[test]
    fn stitched_scroll_frames_reopen_with_identical_pixels() {
        use poratake_daemon_common::scroll::{write_png, CaptureOutcome, FrameAccumulator};

        for (width, height, overlap, expected, repeating) in [
            (96, 160, 64, 64, false),
            (1280, 320, 97, 97, false),
            (800, 720, 217, 217, false),
            (1000, 160, 101, 100, true),
        ] {
            let source = image::RgbaImage::from_fn(width, height * 2 - overlap, |x, y| {
                if repeating {
                    return image::Rgba(if x == 0 {
                        [y as u8, (y >> 8) as u8, y.wrapping_mul(17) as u8, 255]
                    } else {
                        [100 + (y % 2) as u8, 0, 0, 255]
                    });
                }
                let seed = y.wrapping_mul(0x45d9f3b) ^ x.wrapping_mul(0x119de1f3);
                image::Rgba([
                    seed as u8,
                    seed.rotate_left(9) as u8,
                    seed.rotate_left(17) as u8,
                    255,
                ])
            });
            let first = image::imageops::crop_imm(&source, 0, 0, width, height).to_image();
            let second =
                image::imageops::crop_imm(&source, 0, height - overlap, width, height).to_image();
            let mut accumulator = FrameAccumulator::default();
            for frame in [first, second] {
                assert_eq!(
                    accumulator.submit(
                        frame.into_raw(),
                        width as usize,
                        height as usize,
                        expected as usize,
                    ),
                    CaptureOutcome::Added
                );
            }
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("scroll.png");
            write_png(&path, &accumulator.take_frames()).unwrap();
            let stitched = image::open(path).unwrap().to_rgba8();
            assert_eq!(stitched.dimensions(), source.dimensions());
            assert_eq!(
                stitched
                    .pixels()
                    .zip(source.pixels())
                    .filter(|(actual, expected)| actual != expected)
                    .count(),
                0
            );
        }
    }

    #[test]
    fn stitched_scroll_frames_keep_one_fixed_footer() {
        use poratake_daemon_common::scroll::{write_png, CaptureOutcome, FrameAccumulator};

        for (footer, step) in [(40, 128), (80, 32), (80, 128), (80, 192)] {
            let (width, height) = (1280, 320);
            let source = image::RgbaImage::from_fn(width, height + step * 2, |x, y| {
                let seed = y.wrapping_mul(0x45d9f3b) ^ x.wrapping_mul(0x119de1f3);
                image::Rgba([
                    seed as u8,
                    seed.rotate_left(9) as u8,
                    seed.rotate_left(17) as u8,
                    255,
                ])
            });
            let footer_pixel = |x: u32, y: u32| image::Rgba([x as u8, y as u8, 64, 255]);
            let mut accumulator = FrameAccumulator::default();
            for offset in [0, step, step * 2] {
                let frame = image::RgbaImage::from_fn(width, height, |x, y| {
                    if y >= height - footer {
                        footer_pixel(x, y - (height - footer))
                    } else {
                        *source.get_pixel(x, y + offset)
                    }
                });
                assert_eq!(
                    accumulator.submit(
                        frame.into_raw(),
                        width as usize,
                        height as usize,
                        (height - step) as usize,
                    ),
                    CaptureOutcome::Added,
                    "footer={footer}, step={step}, offset={offset}"
                );
            }
            let expected_height = height + step * 2;
            let expected = image::RgbaImage::from_fn(width, expected_height, |x, y| {
                if y >= expected_height - footer {
                    footer_pixel(x, y - (expected_height - footer))
                } else {
                    *source.get_pixel(x, y)
                }
            });
            assert_eq!(accumulator.estimated_height(), expected_height as usize);
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("scroll-footer.png");
            write_png(&path, &accumulator.take_frames()).unwrap();
            let stitched = image::open(path).unwrap().to_rgba8();
            assert_eq!(stitched.dimensions(), expected.dimensions());
            assert_eq!(
                stitched
                    .pixels()
                    .zip(expected.pixels())
                    .filter(|(actual, expected)| actual != expected)
                    .count(),
                0,
                "footer={footer}, step={step}"
            );
        }
    }

    #[test]
    fn stitched_scroll_baselines_keep_one_fixed_footer() {
        use poratake_daemon_common::scroll::{write_png, CaptureOutcome, FrameAccumulator};

        for theme in ["light", "dark"] {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
                "../../../scripts/parity/baselines/win32/{theme}-1280-1/hardware-nvidia-geforce-rtx-5080/gpui-attachment-edges-default.png"
            ));
            let source = image::open(path).unwrap().to_rgba8();
            let (width, height) = (source.width(), 320);
            for footer in [40, 80] {
                for step in [32, 64, 96, 128, 192] {
                    let mut accumulator = FrameAccumulator::default();
                    for offset in [0, step, step * 2] {
                        let mut frame =
                            image::imageops::crop_imm(&source, 0, offset, width, height).to_image();
                        for y in height - footer..height {
                            for x in 0..width {
                                frame.put_pixel(x, y, image::Rgba([64, 64, 64, 255]));
                            }
                        }
                        assert_eq!(
                            accumulator.submit(
                                frame.into_raw(),
                                width as usize,
                                height as usize,
                                (height - step) as usize,
                            ),
                            CaptureOutcome::Added,
                            "theme={theme}, footer={footer}, step={step}, offset={offset}"
                        );
                    }
                    let expected_height = height + step * 2;
                    let mut expected =
                        image::imageops::crop_imm(&source, 0, 0, width, expected_height).to_image();
                    for y in expected_height - footer..expected_height {
                        for x in 0..width {
                            expected.put_pixel(x, y, image::Rgba([64, 64, 64, 255]));
                        }
                    }
                    let dir = tempfile::tempdir().unwrap();
                    let path = dir.path().join("scroll-baseline-footer.png");
                    write_png(&path, &accumulator.take_frames()).unwrap();
                    let stitched = image::open(path).unwrap().to_rgba8();
                    assert_eq!(stitched.dimensions(), expected.dimensions());
                    assert_eq!(
                        stitched
                            .pixels()
                            .zip(expected.pixels())
                            .filter(|(actual, expected)| actual != expected)
                            .count(),
                        0,
                        "theme={theme}, footer={footer}, step={step}"
                    );
                }
            }
        }
    }

    #[test]
    fn stitched_scroll_keeps_sparse_text_and_footer() {
        use poratake_daemon_common::scroll::{write_png, CaptureOutcome, FrameAccumulator};

        let (width, height, footer, step) = (96, 160, 40, 80);
        let source = image::RgbaImage::from_fn(width, height + step, |_, y| {
            if y == 100 {
                image::Rgba([0, 0, 0, 255])
            } else {
                image::Rgba([255, 255, 255, 255])
            }
        });
        let mut accumulator = FrameAccumulator::default();
        for offset in [0, step] {
            let mut frame = image::imageops::crop_imm(&source, 0, offset, width, height).to_image();
            for y in height - footer..height {
                for x in 0..width {
                    frame.put_pixel(x, y, image::Rgba([64, 64, 64, 255]));
                }
            }
            assert_eq!(
                accumulator.submit(
                    frame.into_raw(),
                    width as usize,
                    height as usize,
                    (height - step) as usize,
                ),
                CaptureOutcome::Added
            );
        }
        let mut expected = source;
        for y in height + step - footer..height + step {
            for x in 0..width {
                expected.put_pixel(x, y, image::Rgba([64, 64, 64, 255]));
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("scroll-sparse-footer.png");
        write_png(&path, &accumulator.take_frames()).unwrap();
        let stitched = image::open(path).unwrap().to_rgba8();
        assert_eq!(stitched.dimensions(), expected.dimensions());
        assert_eq!(
            stitched
                .pixels()
                .zip(expected.pixels())
                .filter(|(actual, expected)| actual != expected)
                .count(),
            0
        );
    }

    #[test]
    fn stitched_scroll_tolerates_one_changed_pixel_at_the_overlap_edge() {
        use poratake_daemon_common::scroll::{write_png, CaptureOutcome, FrameAccumulator};

        let (width, height, footer, step) = (96, 160, 40, 80);
        let source = image::RgbaImage::from_fn(width, height + step * 2, |x, y| {
            let seed = y.wrapping_mul(0x45d9f3b) ^ x.wrapping_mul(0x119de1f3);
            image::Rgba([
                seed as u8,
                seed.rotate_left(9) as u8,
                seed.rotate_left(17) as u8,
                255,
            ])
        });
        let mut accumulator = FrameAccumulator::default();
        for index in 0..3 {
            let mut frame =
                image::imageops::crop_imm(&source, 0, index * step, width, height).to_image();
            for y in height - footer..height {
                for x in 0..width {
                    frame.put_pixel(x, y, image::Rgba([64, 64, 64, 255]));
                }
            }
            if index == 2 {
                let pixel = frame.get_pixel_mut(7, height - footer - step - 1);
                pixel[0] ^= 255;
                pixel[1] ^= 255;
            }
            assert_eq!(
                accumulator.submit(
                    frame.into_raw(),
                    width as usize,
                    height as usize,
                    (height - step) as usize,
                ),
                CaptureOutcome::Added
            );
        }
        let mut expected = source;
        for y in height + step * 2 - footer..height + step * 2 {
            for x in 0..width {
                expected.put_pixel(x, y, image::Rgba([64, 64, 64, 255]));
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("scroll-overlap-noise.png");
        write_png(&path, &accumulator.take_frames()).unwrap();
        let stitched = image::open(path).unwrap().to_rgba8();
        assert_eq!(stitched.dimensions(), expected.dimensions());
        assert_eq!(
            stitched
                .pixels()
                .zip(expected.pixels())
                .filter(|(actual, expected)| actual != expected)
                .count(),
            0
        );
    }

    #[test]
    fn stitched_scroll_keeps_a_changed_or_removed_footer_once() {
        use poratake_daemon_common::scroll::{write_png, CaptureOutcome, FrameAccumulator};

        let (width, height, step) = (96, 160, 80);
        for (footers, colors, accepted, matching_body) in [
            ([40, 40, 40, 40], [64, 64, 96, 128], 4, false),
            ([40, 40, 40, 0], [64, 64, 96, 0], 4, false),
            ([40, 40, 0, 0], [64, 64, 0, 0], 4, false),
            ([40, 40, 60, 60], [64, 64, 96, 96], 4, false),
            ([40, 40, 60, 60], [64, 64, 96, 96], 4, true),
            ([40, 40, 20, 20], [64, 64, 96, 96], 4, false),
            ([40, 40, 60, 60], [64, 64, 96, 128], 3, false),
            ([40, 40, 60, 40], [64, 64, 96, 96], 3, false),
        ] {
            let source = image::RgbaImage::from_fn(width, height + step * 3, |x, y| {
                if matching_body && (step * 3..step * 3 + 20).contains(&y) {
                    return image::Rgba([96, (y - step * 3) as u8, 96, 255]);
                }
                let seed = y.wrapping_mul(0x45d9f3b) ^ x.wrapping_mul(0x119de1f3);
                image::Rgba([
                    seed as u8,
                    seed.rotate_left(9) as u8,
                    seed.rotate_left(17) as u8,
                    255,
                ])
            });
            let mut accumulator = FrameAccumulator::default();
            for index in 0..4_usize {
                let mut frame =
                    image::imageops::crop_imm(&source, 0, index as u32 * step, width, height)
                        .to_image();
                let color = colors[index];
                for y in height - footers[index]..height {
                    for x in 0..width {
                        let pixel = if matching_body && footers[index] == 60 {
                            [color, (y - (height - footers[index])) as u8, color, 255]
                        } else {
                            [color, color, color, 255]
                        };
                        frame.put_pixel(x, y, image::Rgba(pixel));
                    }
                }
                let outcome = if index < accepted {
                    CaptureOutcome::Added
                } else {
                    CaptureOutcome::NoOverlap
                };
                assert_eq!(
                    accumulator.submit(
                        frame.into_raw(),
                        width as usize,
                        height as usize,
                        (height - step) as usize,
                    ),
                    outcome,
                    "frame={index}, footers={footers:?}, colors={colors:?}"
                );
            }
            let expected_height = height + step * (accepted as u32 - 1);
            let mut expected =
                image::imageops::crop_imm(&source, 0, 0, width, expected_height).to_image();
            let footer = footers[accepted - 1];
            let color = colors[accepted - 1];
            for y in expected_height - footer..expected_height {
                for x in 0..width {
                    let pixel = if matching_body && footer == 60 {
                        [color, (y - (expected_height - footer)) as u8, color, 255]
                    } else {
                        [color, color, color, 255]
                    };
                    expected.put_pixel(x, y, image::Rgba(pixel));
                }
            }
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("scroll-changed-footer.png");
            write_png(&path, &accumulator.take_frames()).unwrap();
            let stitched = image::open(path).unwrap().to_rgba8();
            assert_eq!(stitched.dimensions(), expected.dimensions());
            assert_eq!(
                stitched
                    .pixels()
                    .zip(expected.pixels())
                    .filter(|(actual, expected)| actual != expected)
                    .count(),
                0,
                "footers={footers:?}, colors={colors:?}"
            );
        }
    }
}
