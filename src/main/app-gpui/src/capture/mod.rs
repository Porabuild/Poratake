//! Screenshot capture pipeline — path generation (port of
//! `capture/screenshot/utils.ts`) and the daemon `screenshot capture-area`
//! call (same contract as `native-capture.ts`).

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use parking_lot::{Condvar, Mutex};
use poratake_daemon_common::geometry::{CaptureAreaRequest, CaptureWindowRequest};

pub use poratake_daemon_common::geometry::DisplayCaptureContext as DisplayCapture;

use crate::config::store::ConfigStore;
use crate::daemon::DaemonHandle;

#[derive(Clone)]
pub struct CaptureService {
    pub daemon: DaemonHandle,
    pub config: Arc<ConfigStore>,
    freeze: Arc<FreezeState>,
}

#[derive(Default)]
struct FreezeState {
    state: Mutex<FreezeProgress>,
    settled: Condvar,
    operation: Mutex<()>,
}

#[derive(Default)]
struct FreezeProgress {
    captures: BTreeMap<u64, usize>,
    next_generation: u64,
    completed_generation: u64,
}

pub(crate) struct CachedCaptureReservation {
    freeze: Arc<FreezeState>,
    generation: u64,
}

impl Drop for CachedCaptureReservation {
    fn drop(&mut self) {
        let mut state = self.freeze.state.lock();
        let captures = state
            .captures
            .get_mut(&self.generation)
            .expect("reserved capture generation");
        *captures -= 1;
        if *captures == 0 {
            state.captures.remove(&self.generation);
        }
        self.freeze.settled.notify_all();
    }
}

impl CachedCaptureReservation {
    pub(crate) fn wait_for_freeze(&self) {
        let mut state = self.freeze.state.lock();
        while state.completed_generation < self.generation {
            self.freeze.settled.wait(&mut state);
        }
    }
}

impl CaptureService {
    pub fn new(daemon: DaemonHandle, config: Arc<ConfigStore>) -> Self {
        Self {
            daemon,
            config,
            freeze: Arc::default(),
        }
    }

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
    pub fn capture_area_to_file_with_options(
        &self,
        capture: DisplayCapture,
        path: &std::path::Path,
        cached: bool,
    ) -> Result<()> {
        self.daemon.screenshot().capture_area(&CaptureAreaRequest {
            capture,
            path: path.to_path_buf(),
            cached,
            window_id: None,
        })
    }

    /// `freezeScreen()` in `capture/freeze-screen`: the daemon paints a still
    /// snapshot of every display and retains those frames, so the selection
    /// happens over a frozen screen and `capture-area { cached: true }` crops
    /// the moment the overlay opened rather than the live desktop.
    ///
    /// Without this the "Freeze screen" setting has no effect: nothing else
    /// populates the daemon's frozen frames, so `cached` finds none and falls
    /// back to a live capture.
    fn begin_freeze(&self) -> u64 {
        let mut state = self.freeze.state.lock();
        state.next_generation += 1;
        state.next_generation
    }

    fn wait_for_freeze_turn(&self, generation: u64) {
        let mut state = self.freeze.state.lock();
        while state.completed_generation + 1 != generation
            || state.captures.range(..generation).next().is_some()
        {
            self.freeze.settled.wait(&mut state);
        }
    }

    fn freeze_screen_started(&self) -> Result<()> {
        self.daemon.freeze_screen().freeze()
    }

    fn finish_freeze(&self, generation: u64, result: Result<()>) -> Result<()> {
        let mut state = self.freeze.state.lock();
        state.completed_generation = generation;
        self.freeze.settled.notify_all();
        result
    }

    pub(crate) fn reserve_cached_capture(&self) -> CachedCaptureReservation {
        let mut state = self.freeze.state.lock();
        let generation = state.next_generation;
        *state.captures.entry(generation).or_default() += 1;
        CachedCaptureReservation {
            freeze: self.freeze.clone(),
            generation,
        }
    }

    fn wait_for_release(&self, generation: u64) -> bool {
        let mut state = self.freeze.state.lock();
        while state.completed_generation < generation
            || state.captures.range(..=generation).next().is_some()
        {
            self.freeze.settled.wait(&mut state);
        }
        state.next_generation <= generation
    }

    /// Captures through the frozen-frame cache when the setting asks for it:
    /// reserves the current freeze generation so a replacement freeze cannot
    /// release the daemon's frames mid-capture, waits for that freeze to
    /// settle, and only then reads the pixels.
    pub(crate) fn capture_area_cached(
        &self,
        capture: DisplayCapture,
        path: &std::path::Path,
        reservation: Option<CachedCaptureReservation>,
    ) -> Result<()> {
        let cached = reservation.is_some();
        if let Some(reservation) = reservation.as_ref() {
            reservation.wait_for_freeze();
        }
        let result = self.capture_area_to_file_with_options(capture, path, cached);
        drop(reservation);
        result
    }

    /// `releaseScreen()`. Safe to call when nothing is frozen.
    pub fn release_screen(&self, generation: u64) -> bool {
        let _operation = self.freeze.operation.lock();
        if generation == 0 || !self.wait_for_release(generation) {
            return true;
        }
        self.release_screen_started()
    }

    fn release_screen_started(&self) -> bool {
        if !self.daemon.is_running() {
            return true;
        }
        if let Err(error) = self.daemon.freeze_screen().release() {
            eprintln!("[freeze] failed to release the frozen displays: {error}");
            return false;
        }
        true
    }

    /// `prewarm` warms the capture pipeline so the freeze itself is not the
    /// first thing to pay for initialising it.
    pub fn prewarm_freeze(&self) {
        if !self.daemon.is_running() {
            return;
        }
        if let Err(error) = self.daemon.freeze_screen().prewarm() {
            eprintln!("[freeze] prewarm failed: {error}");
        }
    }

    pub fn capture_window_to_file(&self, window_id: i64, path: &std::path::Path) -> Result<()> {
        self.daemon
            .screenshot()
            .capture_window(&CaptureWindowRequest {
                window_id,
                path: path.to_path_buf(),
            })
    }
}

pub mod all_in_one;
pub mod all_in_one_toolbar;
pub mod analysis;
pub mod color_picker;
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
) -> bool {
    let service = crate::state::state(cx);
    let primary = cx.primary_display().map(|display| display.id());
    let mut displays = cx.displays();
    if displays.is_empty() {
        crate::windows::toast::Toast::show(cx, "Capture failed", "No display available");
        return false;
    }
    displays.sort_by_key(|display| Some(display.id()) != primary);
    let mut opened = false;
    for display in displays {
        let focus = !opened;
        let bounds = crate::system::work_area::display_bounds(display.as_ref());
        if open(service.clone(), display.id(), bounds, focus, cx) {
            opened = true;
        }
    }
    if !opened {
        crate::windows::toast::Toast::show(cx, "Capture failed", "Could not open the overlay");
    }
    opened
}

/// `prewarmFreezeScreen()`: warms the daemon's capture pipeline off the main
/// thread at startup.
pub fn prewarm_freeze_screen(cx: &mut gpui::App) {
    if !crate::system::capabilities::is_supported(
        crate::system::capabilities::Feature::FreezeScreen,
    ) {
        return;
    }
    let service = crate::state::state(cx);
    cx.background_executor()
        .spawn(async move { service.prewarm_freeze() })
        .detach();
}

fn with_frozen_screen(
    cx: &mut gpui::App,
    release_first: Option<u64>,
    open: impl FnOnce(&mut gpui::App, bool, u64) -> bool + 'static,
) {
    let service = crate::state::state(cx);
    if !crate::system::capabilities::is_supported(
        crate::system::capabilities::Feature::FreezeScreen,
    ) || !service.config.get().screenshot.freeze_screen
    {
        if let Some(generation) = release_first {
            let releasing = service.clone();
            cx.background_executor()
                .spawn(async move { releasing.release_screen(generation) })
                .detach();
        }
        let _ = open(cx, false, 0);
        return;
    }

    let deferred_show = cfg!(windows);
    let generation = service.begin_freeze();
    #[cfg(windows)]
    let opened = open(cx, deferred_show, generation);
    let freezing = service.clone();
    cx.spawn(async move |cx| {
        let freezer = freezing.clone();
        let frozen = cx
            .background_executor()
            .spawn(async move {
                freezer.wait_for_freeze_turn(generation);
                let _operation = freezer.freeze.operation.lock();
                if release_first.is_some() {
                    freezer.release_screen_started();
                }
                let result = freezer.freeze_screen_started();
                freezer.finish_freeze(generation, result)
            })
            .await;
        if let Err(error) = frozen {
            // A failed freeze must not cost the user the capture; the overlay
            // opens over the live screen instead.
            eprintln!("[freeze] {error}");
        }
        #[cfg(windows)]
        let opened = {
            let _ = cx.update(|cx| overlay::raise_all(generation, cx));
            opened
        };
        #[cfg(not(windows))]
        let opened = cx
            .update(|cx| open(cx, deferred_show, generation))
            .unwrap_or(false);
        if !opened {
            cx.background_executor()
                .spawn(async move { freezing.release_screen(generation) })
                .detach();
        }
    })
    .detach();
}

/// Opens the shared area overlay for one of the selection-driven flows.
pub fn start_area_selection(intent: intent::CaptureIntent, cx: &mut gpui::App) {
    if !capture_topology_supported(cx) {
        return;
    }
    let release_first = overlay::replace_all(cx);
    with_frozen_screen(cx, release_first, move |cx, deferred_show, generation| {
        each_display(cx, |service, id, bounds, focus, cx| {
            overlay::AreaOverlay::open(
                service,
                id,
                bounds,
                intent,
                overlay::OverlayLaunch {
                    focus,
                    deferred_show,
                    generation,
                },
                cx,
            )
            .is_some()
        })
    });
}

/// Opens the all-in-one overlay: one surface that switches between the
/// screenshot, recording and OCR flows over area, window or screen.
pub fn start_all_in_one(cx: &mut gpui::App) {
    if !capture_topology_supported(cx) {
        return;
    }
    let choices = all_in_one::restore(&crate::state::state(cx).config);
    let release_first = overlay::replace_all(cx);
    with_frozen_screen(cx, release_first, move |cx, deferred_show, generation| {
        each_display(cx, |service, id, bounds, focus, cx| {
            overlay::AreaOverlay::open_all_in_one(
                service,
                id,
                bounds,
                choices,
                overlay::OverlayLaunch {
                    focus,
                    deferred_show,
                    generation,
                },
                cx,
            )
            .is_some()
        })
    });
}

fn capture_topology_supported(_cx: &mut gpui::App) -> bool {
    #[cfg(target_os = "linux")]
    if crate::system::linux_session::current()
        == crate::system::linux_session::LinuxSession::Wayland
        && _cx.displays().len() != 1
    {
        crate::windows::toast::Toast::show(
            _cx,
            "Capture unavailable",
            "Wayland capture currently requires a single active display",
        );
        return false;
    }
    true
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
    let scale = overlay::display_scale_factor(display.as_ref(), cx);
    #[cfg(target_os = "macos")]
    let display_id = Some(u32::from(display.id()));
    #[cfg(not(target_os = "macos"))]
    let display_id = None;
    crate::windows::recording_control::RecordingControl::open(
        cx,
        crate::video::recorder::RecordingTarget::Screen,
        overlay::physical_rect(
            crate::system::work_area::display_bounds(display.as_ref()),
            scale,
        ),
        display_id,
        None,
        None,
    );
}

fn start_window_picker(intent: intent::CaptureIntent, cx: &mut gpui::App) {
    let service = crate::state::state(cx);
    if let Some(generation) = overlay::replace_all(cx) {
        let releasing = service.clone();
        cx.background_executor()
            .spawn(async move { releasing.release_screen(generation) })
            .detach();
    }
    let mut pending = Vec::new();
    each_display(cx, |service, id, bounds, focus, cx| {
        let Some(handle) = overlay::AreaOverlay::open_with_windows(
            service,
            id,
            bounds,
            intent,
            Vec::new(),
            focus,
            cx,
        ) else {
            return false;
        };
        let request_generation = handle
            .update(cx, |overlay, _, _| overlay.window_list_generation())
            .unwrap_or(0);
        pending.push((handle, request_generation));
        true
    });
    if pending.is_empty() {
        return;
    }
    let daemon = service.daemon.clone();
    cx.spawn(async move |cx| {
        let windows = cx
            .background_executor()
            .spawn(async move { windows_list::list(&daemon) })
            .await;
        let _ = cx.update(|cx| {
            let mut applied = false;
            for (handle, request_generation) in pending {
                let window_list = windows.clone();
                applied |= handle
                    .update(cx, |overlay, _, cx| {
                        overlay.apply_window_list(request_generation, window_list, cx)
                    })
                    .unwrap_or(false);
            }
            if applied && windows.is_empty() {
                overlay::close_all(cx);
                crate::windows::toast::Toast::show(
                    cx,
                    "No windows found",
                    "The daemon did not report any capturable windows",
                );
            }
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

#[derive(Clone)]
struct ScreenTarget {
    display_id: gpui::DisplayId,
    bounds: gpui::Bounds<gpui::Pixels>,
    scale: f32,
    capture_display_id: Option<u32>,
    primary: bool,
}

#[cfg(any(target_os = "linux", test))]
fn x11_screen_bounds(
    rect: poratake_daemon_common::geometry::CaptureRect,
    scale: f32,
) -> gpui::Bounds<gpui::Pixels> {
    gpui::Bounds {
        origin: gpui::point(
            gpui::px(rect.x as f32 / scale),
            gpui::px(rect.y as f32 / scale),
        ),
        size: gpui::size(
            gpui::px(rect.width as f32 / scale),
            gpui::px(rect.height as f32 / scale),
        ),
    }
}

fn screen_targets(cx: &mut gpui::App) -> Result<Vec<ScreenTarget>> {
    #[cfg(target_os = "linux")]
    if crate::system::linux_session::current() == crate::system::linux_session::LinuxSession::X11 {
        let display =
            selected_display(cx).ok_or_else(|| anyhow::anyhow!("No display available"))?;
        let physical = crate::state::state(cx)
            .daemon
            .screenshot()
            .list_displays()?;
        let scale = crate::system::work_area::x11_capture_scale_factor(display.as_ref(), &physical)
            .unwrap_or(1.0);
        let mut targets: Vec<_> = physical
            .into_iter()
            .map(|physical| ScreenTarget {
                display_id: display.id(),
                bounds: x11_screen_bounds(physical.rect, scale),
                scale,
                capture_display_id: None,
                primary: physical.primary,
            })
            .collect();
        targets.sort_by_key(|target| !target.primary);
        return Ok(targets);
    }

    let primary = cx.primary_display().map(|display| display.id());
    let mut targets = Vec::new();
    for display in cx.displays() {
        let scale = overlay::display_scale_factor(display.as_ref(), cx);
        #[cfg(target_os = "macos")]
        let capture_display_id = Some(u32::from(display.id()));
        #[cfg(not(target_os = "macos"))]
        let capture_display_id = None;
        targets.push(ScreenTarget {
            display_id: display.id(),
            bounds: crate::system::work_area::display_bounds(display.as_ref()),
            scale,
            capture_display_id,
            primary: primary == Some(display.id()),
        });
    }
    targets.sort_by_key(|target| !target.primary);
    Ok(targets)
}

fn capture_screen_target(target: ScreenTarget, cx: &mut gpui::App) {
    let capture = overlay::display_capture(target.bounds, target.scale, target.capture_display_id);
    let coordinator = crate::state::coordinator(cx);
    coordinator.update(cx, |coordinator, cx| {
        coordinator.capture_area_for(capture, intent::CaptureIntent::Screenshot, cx);
    });
}

fn fallback_screen_target(target: ScreenTarget, close_overlays: bool, cx: &mut gpui::App) {
    let service = crate::state::state(cx);
    let reservation = (service.config.get().screenshot.freeze_screen
        && crate::system::capabilities::is_supported(
            crate::system::capabilities::Feature::FreezeScreen,
        ))
    .then(|| service.reserve_cached_capture());
    let capture = overlay::display_capture(target.bounds, target.scale, target.capture_display_id);
    let coordinator = crate::state::coordinator(cx);
    if close_overlays {
        let _ = overlay::replace_all(cx);
    }
    cx.defer(move |cx| {
        coordinator.update(cx, |coordinator, cx| {
            coordinator.capture_area_reserved(
                capture,
                intent::CaptureIntent::Screenshot,
                reservation,
                cx,
            );
        });
    });
}

fn all_screen_targets_opened(opened: usize, target_count: usize) -> bool {
    opened == target_count
}

pub fn start_screen_capture(cx: &mut gpui::App) {
    if !capture_topology_supported(cx) {
        return;
    }

    let targets = match screen_targets(cx) {
        Ok(targets) if !targets.is_empty() => targets,
        Ok(_) => {
            crate::windows::toast::Toast::show(cx, "Capture failed", "No display available");
            return;
        }
        Err(error) => {
            crate::windows::toast::Toast::show(cx, "Capture failed", error.to_string());
            return;
        }
    };

    if targets.len() > 1
        && crate::system::capabilities::is_supported(
            crate::system::capabilities::Feature::DisplaySelector,
        )
    {
        let fallback = targets[0].clone();
        let release_first = overlay::replace_all(cx);
        with_frozen_screen(cx, release_first, move |cx, deferred_show, generation| {
            let service = crate::state::state(cx);
            let target_count = targets.len();
            let mut opened = 0;
            for target in targets {
                if overlay::AreaOverlay::open_screen_picker(
                    service.clone(),
                    target.display_id,
                    target.bounds,
                    overlay::OverlayLaunch {
                        focus: opened == 0,
                        deferred_show,
                        generation,
                    },
                    cx,
                )
                .is_some()
                {
                    opened += 1;
                }
            }
            if all_screen_targets_opened(opened, target_count) {
                return true;
            }
            fallback_screen_target(fallback.clone(), opened > 0, cx);
            false
        });
        return;
    }

    capture_screen_target(targets[0].clone(), cx);
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use super::CaptureService;
    use crate::config::store::ConfigStore;
    use crate::daemon::DaemonHandle;

    #[test]
    fn x11_screen_target_keeps_the_randr_monitor_geometry() {
        let bounds = super::x11_screen_bounds(
            poratake_daemon_common::geometry::CaptureRect {
                x: -1920,
                y: 120,
                width: 1920,
                height: 1080,
            },
            2.0,
        );

        assert_eq!(f32::from(bounds.origin.x), -960.0);
        assert_eq!(f32::from(bounds.origin.y), 60.0);
        assert_eq!(f32::from(bounds.size.width), 960.0);
        assert_eq!(f32::from(bounds.size.height), 540.0);
    }

    #[test]
    fn partial_screen_picker_returns_control_to_the_freeze_driver() {
        assert!(super::all_screen_targets_opened(2, 2));
        assert!(!super::all_screen_targets_opened(1, 2));
        assert!(!super::all_screen_targets_opened(0, 2));
    }

    #[cfg(windows)]
    #[gpui::test]
    fn frozen_overlay_opens_before_the_freeze_task_runs(cx: &mut gpui::TestAppContext) {
        use std::sync::atomic::{AtomicBool, Ordering};

        let dir = tempfile::tempdir().expect("temp dir");
        let config =
            Arc::new(ConfigStore::load_at(dir.path().join("config.json")).expect("load config"));
        config.update(|config| config.screenshot.freeze_screen = true);
        cx.update(|cx| crate::state::set_test_state(cx, config));
        let opened = Arc::new(AtomicBool::new(false));
        let observed = opened.clone();

        cx.update(|cx| {
            super::with_frozen_screen(cx, None, move |_, deferred_show, generation| {
                assert!(deferred_show);
                assert_eq!(generation, 1);
                observed.store(true, Ordering::SeqCst);
                true
            });
        });

        assert!(opened.load(Ordering::SeqCst));
    }

    #[test]
    fn cached_capture_waits_for_pending_freeze() {
        let dir = tempfile::tempdir().expect("temp dir");
        let config =
            Arc::new(ConfigStore::load_at(dir.path().join("config.json")).expect("load config"));
        let service = CaptureService::new(DaemonHandle::new(), config);
        let generation = service.begin_freeze();
        let reservation = service.reserve_cached_capture();

        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            reservation.wait_for_freeze();
            sender.send(()).expect("send completion");
        });

        assert!(receiver.recv_timeout(Duration::from_millis(20)).is_err());
        service
            .finish_freeze(generation, Ok(()))
            .expect("finish freeze");
        receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("capture released");
    }

    #[test]
    fn release_waits_for_reserved_capture() {
        let dir = tempfile::tempdir().expect("temp dir");
        let config =
            Arc::new(ConfigStore::load_at(dir.path().join("config.json")).expect("load config"));
        let service = CaptureService::new(DaemonHandle::new(), config);
        let generation = service.begin_freeze();
        service
            .finish_freeze(generation, Ok(()))
            .expect("finish freeze");
        let reservation = service.reserve_cached_capture();

        let waiting = service.clone();
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            sender
                .send(waiting.wait_for_release(generation))
                .expect("send completion");
        });

        assert!(receiver.recv_timeout(Duration::from_millis(20)).is_err());
        drop(reservation);
        assert!(receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("release continued"));
    }

    #[test]
    fn replacement_freezes_run_in_request_order() {
        let dir = tempfile::tempdir().expect("temp dir");
        let config =
            Arc::new(ConfigStore::load_at(dir.path().join("config.json")).expect("load config"));
        let service = CaptureService::new(DaemonHandle::new(), config);
        let first = service.begin_freeze();
        let second = service.begin_freeze();

        let waiting = service.clone();
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            waiting.wait_for_freeze_turn(second);
            sender.send(()).expect("send completion");
        });

        assert!(receiver.recv_timeout(Duration::from_millis(20)).is_err());
        service.finish_freeze(first, Ok(())).expect("finish first");
        receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("second freeze started");
        service
            .finish_freeze(second, Ok(()))
            .expect("finish second");
    }

    #[test]
    fn replacement_freeze_waits_for_prior_capture_only() {
        let dir = tempfile::tempdir().expect("temp dir");
        let config =
            Arc::new(ConfigStore::load_at(dir.path().join("config.json")).expect("load config"));
        let service = CaptureService::new(DaemonHandle::new(), config);
        let first = service.begin_freeze();
        service.finish_freeze(first, Ok(())).expect("finish first");
        let first_capture = service.reserve_cached_capture();
        let second = service.begin_freeze();

        let (capture_sender, capture_receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            first_capture.wait_for_freeze();
            capture_sender
                .send(first_capture)
                .expect("send reservation");
        });
        let first_capture = capture_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("first capture started");

        let waiting = service.clone();
        let (freeze_sender, freeze_receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            waiting.wait_for_freeze_turn(second);
            freeze_sender.send(()).expect("send completion");
        });
        let second_capture = service.reserve_cached_capture();

        assert!(freeze_receiver
            .recv_timeout(Duration::from_millis(20))
            .is_err());
        drop(first_capture);
        freeze_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("replacement freeze started");
        drop(second_capture);
        service
            .finish_freeze(second, Ok(()))
            .expect("finish second");
    }

    #[test]
    fn stale_release_does_not_clear_replacement_freeze() {
        let dir = tempfile::tempdir().expect("temp dir");
        let config =
            Arc::new(ConfigStore::load_at(dir.path().join("config.json")).expect("load config"));
        let service = CaptureService::new(DaemonHandle::new(), config);
        let first = service.begin_freeze();
        service.finish_freeze(first, Ok(())).expect("finish first");
        let first_capture = service.reserve_cached_capture();

        let waiting = service.clone();
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            sender
                .send(waiting.wait_for_release(first))
                .expect("send release result");
        });

        let second = service.begin_freeze();
        drop(first_capture);
        assert!(!receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("stale release completed"));
        service
            .finish_freeze(second, Ok(()))
            .expect("finish second");
    }
}
