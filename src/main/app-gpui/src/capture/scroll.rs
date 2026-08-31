//! Scroll capture — port of `capture/scroll-capture/index.ts`. On Windows the
//! daemon owns the on-screen control panel (`daemon-win/src/panel.rs`) and
//! reports `scroll-capture:done` / `:cancelled`, so the app only picks the
//! area, starts the session and finalizes the stitched image.

use std::sync::atomic::{AtomicBool, Ordering};

use poratake_daemon_common::contract::{ScrollCaptureFinishRequest, ScrollCaptureStartRequest};

use crate::daemon::DaemonHandle;

static ACTIVE: AtomicBool = AtomicBool::new(false);

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
    ACTIVE.store(false, Ordering::SeqCst);
    let response = daemon
        .scroll_capture()
        .finish(&ScrollCaptureFinishRequest {
            output_path: output_path.to_path_buf(),
        })
        .map_err(|error| eprintln!("[scroll-capture] finish failed: {error}"))
        .ok()?;

    if !response.success {
        return None;
    }
    Some(response.output_path)
}

pub fn cancel(daemon: &DaemonHandle) {
    if !ACTIVE.swap(false, Ordering::SeqCst) {
        return;
    }
    if let Err(error) = daemon.scroll_capture().cancel() {
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
}
