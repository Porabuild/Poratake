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
    #[cfg(target_os = "macos")]
    Frame {
        preview: Option<String>,
    },
    #[cfg(target_os = "macos")]
    AutoScrolling(bool),
    #[cfg(target_os = "macos")]
    CursorOutside(bool),
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

        let (width, height, overlap) = (96, 160, 64);
        let source = image::RgbaImage::from_fn(width, height * 2 - overlap, |x, y| {
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
                    overlap as usize,
                ),
                CaptureOutcome::Added
            );
        }
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("scroll.png");
        write_png(&path, &accumulator.take_frames()).unwrap();
        assert_eq!(image::open(path).unwrap().to_rgba8(), source);
    }
}
