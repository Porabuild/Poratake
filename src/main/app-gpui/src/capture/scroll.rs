//! Scroll capture — port of `capture/scroll-capture/index.ts`. On Windows the
//! daemon owns the on-screen control panel (`daemon-win/src/panel.rs`) and
//! reports `scroll-capture:done` / `:cancelled`, so the app only picks the
//! area, starts the session and finalizes the stitched image.

use std::sync::atomic::{AtomicBool, Ordering};

use serde_json::json;

use crate::capture::overlay::ScreenRect;
use crate::daemon::DaemonHandle;

static ACTIVE: AtomicBool = AtomicBool::new(false);

pub fn is_active() -> bool {
    ACTIVE.load(Ordering::SeqCst)
}

pub struct StartParams {
    pub rect: ScreenRect,
    pub auto_scroll_speed: String,
    pub max_height: f64,
    pub scale_factor: f32,
}

pub fn start(daemon: &DaemonHandle, params: &StartParams) -> bool {
    if ACTIVE.swap(true, Ordering::SeqCst) {
        return false;
    }
    if !daemon.is_running() && daemon.start().is_err() {
        ACTIVE.store(false, Ordering::SeqCst);
        return false;
    }
    let called = daemon.call(
        "scroll-capture",
        "start",
        Some(json!({
            "x": params.rect.x,
            "y": params.rect.y,
            "width": params.rect.width,
            "height": params.rect.height,
            "scaleFactor": params.scale_factor,
            "autoScrollSpeed": params.auto_scroll_speed,
            "maxHeight": params.max_height,
        })),
    );
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
        .call(
            "scroll-capture",
            "finish",
            Some(json!({ "outputPath": output_path.to_string_lossy() })),
        )
        .map_err(|error| eprintln!("[scroll-capture] finish failed: {error}"))
        .ok()?;

    if !response
        .get("success")
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
    {
        return None;
    }
    Some(
        response
            .get("outputPath")
            .and_then(|value| value.as_str())
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| output_path.to_path_buf()),
    )
}

pub fn cancel(daemon: &DaemonHandle) {
    if !ACTIVE.swap(false, Ordering::SeqCst) {
        return;
    }
    if let Err(error) = daemon.call("scroll-capture", "cancel", None) {
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
