//! Screenshot capture pipeline — path generation (port of
//! `capture/screenshot/utils.ts`) and the daemon `screenshot capture-area`
//! call (same contract as `native-capture.ts`).

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use serde_json::json;

use crate::config::store::ConfigStore;
use crate::daemon::DaemonHandle;

#[derive(Clone)]
pub struct CaptureService {
    pub daemon: DaemonHandle,
    pub config: Arc<ConfigStore>,
}

impl CaptureService {
    /// Port of `getScreenshotsDir`.
    pub fn screenshots_dir(&self) -> PathBuf {
        let config = self.config.get();
        let custom = config.storage.screenshots_path.clone();
        if !custom.is_empty() && std::path::Path::new(&custom).is_dir() {
            return PathBuf::from(custom);
        }
        let pictures = dirs::picture_dir().unwrap_or_else(|| PathBuf::from("."));
        pictures.join("Poratake")
    }

    /// Port of `generateScreenshotPath`.
    pub fn generate_screenshot_path(&self) -> PathBuf {
        let config = self.config.get();
        let pattern = if config.storage.naming_pattern.is_empty() {
            "%type %Y-%m-%d at %H.%M.%S"
        } else {
            &config.storage.naming_pattern
        };
        let filename = crate::editor::filename::generate_filename(
            pattern,
            "Screenshot",
            "png",
            chrono::Local::now(),
        );
        self.screenshots_dir().join(filename)
    }

    /// Same daemon contract as `captureRegionToFile`: screen-space rect in
    /// physical pixels plus a destination file path.
    pub fn capture_area_to_file(
        &self,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        path: &std::path::Path,
    ) -> Result<()> {
        self.capture_area_to_file_with_options(x, y, width, height, path, false)
    }

    pub fn capture_area_to_file_with_options(
        &self,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        path: &std::path::Path,
        cached: bool,
    ) -> Result<()> {
        if !self.daemon.is_running() {
            self.daemon.start()?;
        }
        self.daemon
            .call(
                "screenshot",
                "capture-area",
                Some(json!({
                    "x": x,
                    "y": y,
                    "width": width,
                    "height": height,
                    "path": path.to_string_lossy(),
                    "cached": cached,
                })),
            )
            .map_err(|error| anyhow!("capture-area failed: {error}"))?;
        Ok(())
    }

    /// `freezeScreen()` in `capture/freeze-screen`: the daemon paints a still
    /// snapshot of every display and retains those frames, so the selection
    /// happens over a frozen screen and `capture-area { cached: true }` crops
    /// the moment the overlay opened rather than the live desktop.
    ///
    /// Without this the "Freeze screen" setting has no effect: nothing else
    /// populates the daemon's frozen frames, so `cached` finds none and falls
    /// back to a live capture.
    pub fn freeze_screen(&self) -> Result<()> {
        if !self.daemon.is_running() {
            self.daemon.start()?;
        }
        self.daemon
            .call("freeze-screen", "freeze", Some(json!({})))
            .map_err(|error| anyhow!("freeze failed: {error}"))?;
        Ok(())
    }

    /// `releaseScreen()`. Safe to call when nothing is frozen.
    pub fn release_screen(&self) {
        if !self.daemon.is_running() {
            return;
        }
        if let Err(error) = self
            .daemon
            .call("freeze-screen", "release", Some(json!({})))
        {
            eprintln!("[freeze] failed to release the frozen displays: {error}");
        }
    }

    /// `prewarm` warms the capture pipeline so the freeze itself is not the
    /// first thing to pay for initialising it.
    pub fn prewarm_freeze(&self) {
        if !self.daemon.is_running() {
            return;
        }
        if let Err(error) = self
            .daemon
            .call("freeze-screen", "prewarm", Some(json!({})))
        {
            eprintln!("[freeze] prewarm failed: {error}");
        }
    }

    pub fn capture_window_to_file(&self, window_id: i64, path: &std::path::Path) -> Result<()> {
        if !self.daemon.is_running() {
            self.daemon.start()?;
        }
        self.daemon
            .call(
                "screenshot",
                "capture-window",
                Some(json!({
                    "windowId": window_id,
                    "path": path.to_string_lossy(),
                })),
            )
            .map_err(|error| anyhow!("capture-window failed: {error}"))?;
        Ok(())
    }
}

pub mod all_in_one;
pub mod all_in_one_toolbar;
pub mod analysis;
pub mod coordinator;
pub mod desktop_icons;
pub mod intent;
pub mod overlay;
pub mod scroll;
pub mod selection;
pub mod timer;
pub mod windows_list;

pub fn start_area_capture(cx: &mut gpui::App) {
    start_area_selection(intent::CaptureIntent::Screenshot, cx);
}

fn each_display(
    cx: &mut gpui::App,
    mut open: impl FnMut(
        CaptureService,
        gpui::DisplayId,
        gpui::Bounds<gpui::Pixels>,
        bool,
        &mut gpui::App,
    ) -> bool,
) {
    overlay::close_all(cx);
    let service = crate::state::state(cx);
    let primary = cx.primary_display().map(|display| display.id());
    let displays = cx.displays();
    if displays.is_empty() {
        crate::windows::toast::Toast::show(cx, "Capture failed", "No display available");
        return;
    }
    let mut opened = false;
    for display in displays {
        let focus = primary.map(|id| id == display.id()).unwrap_or(!opened);
        if open(service.clone(), display.id(), display.bounds(), focus, cx) {
            opened = true;
        }
    }
    if !opened {
        crate::windows::toast::Toast::show(cx, "Capture failed", "Could not open the overlay");
    }
}

/// `prewarmFreezeScreen()`: warms the daemon's capture pipeline off the main
/// thread at startup.
pub fn prewarm_freeze_screen(cx: &mut gpui::App) {
    let service = crate::state::state(cx);
    cx.background_executor()
        .spawn(async move { service.prewarm_freeze() })
        .detach();
}

/// Freezes the screen if the setting asks for it, then opens the overlay.
///
/// `session.ts` awaits `freezeScreen()` before showing its windows, so the
/// still snapshot is already up when the user starts dragging; the freeze runs
/// on the background executor because the daemon answers it from its own UI
/// thread.
fn with_frozen_screen(cx: &mut gpui::App, open: impl FnOnce(&mut gpui::App) + 'static) {
    let service = crate::state::state(cx);
    if !service.config.get().screenshot.freeze_screen {
        open(cx);
        return;
    }

    cx.spawn(async move |cx| {
        let frozen = cx
            .background_executor()
            .spawn(async move { service.freeze_screen() })
            .await;
        if let Err(error) = frozen {
            // A failed freeze must not cost the user the capture; the overlay
            // opens over the live screen instead.
            eprintln!("[freeze] {error}");
        }
        let _ = cx.update(|cx| open(cx));
    })
    .detach();
}

/// Opens the shared area overlay for one of the selection-driven flows.
pub fn start_area_selection(intent: intent::CaptureIntent, cx: &mut gpui::App) {
    with_frozen_screen(cx, move |cx| {
        each_display(cx, |service, id, bounds, focus, cx| {
            overlay::AreaOverlay::open(service, id, bounds, intent, focus, cx).is_some()
        });
    });
}

/// Opens the all-in-one overlay: one surface that switches between the
/// screenshot, recording and OCR flows over area, window or screen.
pub fn start_all_in_one(cx: &mut gpui::App) {
    let choices = all_in_one::restore(&crate::state::state(cx).config);
    with_frozen_screen(cx, move |cx| {
        each_display(cx, |service, id, bounds, focus, cx| {
            overlay::AreaOverlay::open_all_in_one(service, id, bounds, choices, focus, cx).is_some()
        });
    });
}

fn selected_display(cx: &mut gpui::App) -> Option<std::rc::Rc<dyn gpui::PlatformDisplay>> {
    cx.primary_display()
        .or_else(|| cx.displays().into_iter().next())
}

/// Starts a recording of the primary display.
pub fn start_screen_recording(cx: &mut gpui::App) {
    let Some(display) = selected_display(cx) else {
        return;
    };
    let scale = overlay::app_scale_factor(cx);
    crate::windows::recording_control::RecordingControl::open(
        cx,
        crate::video::recorder::RecordingTarget::Screen,
        overlay::physical_rect(display.bounds(), scale),
        None,
        None,
    );
}

fn start_window_picker(intent: intent::CaptureIntent, cx: &mut gpui::App) {
    let service = crate::state::state(cx);
    let daemon = service.daemon.clone();
    cx.spawn(async move |cx| {
        let windows = cx
            .background_executor()
            .spawn(async move { windows_list::list(&daemon) })
            .await;
        let _ = cx.update(|cx| {
            if windows.is_empty() {
                crate::windows::toast::Toast::show(
                    cx,
                    "No windows found",
                    "The daemon did not report any capturable windows",
                );
                return;
            }
            each_display(cx, |service, id, bounds, focus, cx| {
                overlay::AreaOverlay::open_with_windows(
                    service,
                    id,
                    bounds,
                    intent,
                    windows.clone(),
                    focus,
                    cx,
                )
                .is_some()
            });
        });
    })
    .detach();
}

/// Picks a window to record through the same overlay the window screenshot
/// flow uses.
pub fn start_window_recording(cx: &mut gpui::App) {
    start_window_picker(intent::CaptureIntent::Recording, cx);
}

/// Opens the same overlay in window-pick mode: hovering highlights a window
/// and clicking captures it through the daemon.
pub fn start_window_capture(cx: &mut gpui::App) {
    start_window_picker(intent::CaptureIntent::Screenshot, cx);
}

pub fn start_screen_capture(cx: &mut gpui::App) {
    let Some(display) = selected_display(cx) else {
        return;
    };
    let scale = overlay::app_scale_factor(cx);
    let rect = overlay::physical_rect(display.bounds(), scale);
    let coordinator = crate::state::coordinator(cx);
    coordinator.update(cx, |coord, cx| {
        coord.capture_area(rect, cx);
    });
}
