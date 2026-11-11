//! Screenshot capture pipeline — path generation (port of
//! `capture/screenshot/utils.ts`) and the daemon `screenshot capture-area`
//! call (same contract as `native-capture.ts`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use parking_lot::{Condvar, Mutex};
use poratake_daemon_common::geometry::{CaptureAreaRequest, CaptureWindowRequest};

pub use poratake_daemon_common::geometry::DisplayCaptureContext as DisplayCapture;

use crate::config::store::ConfigStore;
use crate::daemon::DaemonHandle;

fn uuid_simple() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{nanos:x}-{count:x}-{}", std::process::id())
}

pub(crate) fn prepare_screenshot(output: &Path) -> std::io::Result<PathBuf> {
    let directory = output.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(directory)?;
    loop {
        let temporary = directory.join(format!(".poratake-{}.tmp.png", uuid_simple()));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(_) => return Ok(temporary),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
}

pub(crate) fn publish_screenshot(temporary: &Path, output: &Path) -> std::io::Result<PathBuf> {
    let mut suffix = 0;
    let mut copy = false;
    loop {
        let destination = if suffix == 0 {
            output.to_path_buf()
        } else {
            let mut name = output.file_stem().unwrap_or_default().to_os_string();
            name.push(format!(" ({suffix})"));
            if let Some(extension) = output.extension() {
                name.push(".");
                name.push(extension);
            }
            output.with_file_name(name)
        };
        if !copy {
            match std::fs::hard_link(temporary, &destination) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    suffix += 1;
                    continue;
                }
                Err(_) => {
                    copy = true;
                    continue;
                }
            }
        } else {
            let mut source = std::fs::File::open(temporary)?;
            let mut target = match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&destination)
            {
                Ok(file) => file,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    suffix += 1;
                    continue;
                }
                Err(error) => return Err(error),
            };
            if let Err(error) = std::io::copy(&mut source, &mut target) {
                drop(target);
                let _ = std::fs::remove_file(&destination);
                return Err(error);
            }
        }
        if let Err(error) = std::fs::remove_file(temporary) {
            eprintln!(
                "[capture] failed to remove {}: {error}",
                temporary.display()
            );
        }
        return Ok(destination);
    }
}

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
    #[cfg(test)]
    release_waiting: Mutex<Option<std::sync::mpsc::SyncSender<()>>>,
    #[cfg(all(test, windows))]
    defer_open_for_test: std::sync::atomic::AtomicBool,
    #[cfg(all(test, windows))]
    scheduled_for_test: std::sync::atomic::AtomicU64,
    #[cfg(all(test, windows))]
    release_fails_for_test: std::sync::atomic::AtomicBool,
    #[cfg(all(test, windows))]
    freeze_succeeds_for_test: std::sync::atomic::AtomicBool,
}

#[derive(Default)]
struct FreezeProgress {
    captures: BTreeMap<u64, usize>,
    next_generation: u64,
    completed_generation: u64,
    frozen_generation: Option<u64>,
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
    pub(crate) fn wait_for_freeze(&self) -> bool {
        let mut state = self.freeze.state.lock();
        while state.completed_generation < self.generation {
            self.freeze.settled.wait(&mut state);
        }
        state.frozen_generation == Some(self.generation)
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
        #[cfg(all(test, windows))]
        if self
            .freeze
            .freeze_succeeds_for_test
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            return Ok(());
        }
        self.daemon.freeze_screen().freeze()
    }

    fn finish_freeze(&self, generation: u64, result: Result<()>) -> Result<()> {
        let mut state = self.freeze.state.lock();
        state.completed_generation = generation;
        state.frozen_generation = result.is_ok().then_some(generation);
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
        #[cfg(test)]
        if let Some(waiting) = self.freeze.release_waiting.lock().take() {
            let _ = waiting.send(());
        }
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
        release_after: Option<u64>,
    ) -> Result<()> {
        let cached = reservation
            .as_ref()
            .is_some_and(CachedCaptureReservation::wait_for_freeze);
        let result = self.capture_area_to_file_with_options(capture, path, cached);
        drop(reservation);
        if let Some(generation) = release_after {
            self.release_screen(generation);
        }
        result
    }

    /// `releaseScreen()`. Safe to call when nothing is frozen.
    pub fn release_screen(&self, generation: u64) -> bool {
        if generation == 0 {
            return true;
        }
        loop {
            if !self.wait_for_release(generation) {
                return true;
            }
            let _operation = self.freeze.operation.lock();
            let state = self.freeze.state.lock();
            if state.next_generation > generation {
                return true;
            }
            if state.captures.range(..=generation).next().is_some() {
                continue;
            }
            drop(state);
            return self.release_screen_started();
        }
    }

    fn release_screen_started(&self) -> bool {
        #[cfg(all(test, windows))]
        if self
            .freeze
            .release_fails_for_test
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            return false;
        }
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

    pub(crate) fn capture_window_to_file(
        &self,
        window_id: i64,
        capture: DisplayCapture,
        path: &std::path::Path,
        reservation: Option<CachedCaptureReservation>,
    ) -> Result<()> {
        if reservation
            .as_ref()
            .is_some_and(CachedCaptureReservation::wait_for_freeze)
        {
            return self.daemon.screenshot().capture_area(&CaptureAreaRequest {
                capture,
                path: path.to_path_buf(),
                cached: true,
                window_id: Some(window_id),
            });
        }
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
    freeze: bool,
    failure_title: &'static str,
    open: impl FnOnce(&mut gpui::App, bool, u64) -> bool + 'static,
) {
    let service = crate::state::state(cx);
    let capture_hide = overlay::capture_hide(cx);
    let owner = overlay::revision(cx);
    if !freeze
        || !crate::system::capabilities::is_supported(
            crate::system::capabilities::Feature::FreezeScreen,
        )
        || !service.config.get().screenshot.freeze_screen
    {
        if let Some(generation) = release_first.filter(|generation| *generation != 0) {
            overlay::set_pending_freeze(generation, cx);
            cx.spawn(async move |cx| {
                let released = cx
                    .background_executor()
                    .spawn(async move { service.release_screen(generation) })
                    .await;
                cx.update(|cx| {
                    if overlay::revision(cx) != owner {
                        return;
                    }
                    if !released {
                        overlay::release_capture_hide(capture_hide, cx);
                        let coordinator = crate::state::coordinator(cx);
                        coordinator
                            .update(cx, |coordinator, cx| coordinator.cancel_editor_attach(cx));
                        crate::windows::toast::Toast::show(
                            cx,
                            failure_title,
                            "Could not release the frozen screen",
                        );
                        return;
                    }
                    overlay::clear_pending_freeze(generation, owner, cx);
                    if !open(cx, false, 0) {
                        overlay::release_capture_hide(capture_hide, cx);
                    }
                });
            })
            .detach();
            return;
        }
        if !open(cx, false, 0) {
            overlay::release_capture_hide(capture_hide, cx);
        }
        return;
    }

    let deferred_show = cfg!(windows);
    #[cfg(all(test, windows))]
    let deferred_show = deferred_show
        && !service
            .freeze
            .defer_open_for_test
            .load(std::sync::atomic::Ordering::SeqCst);
    let generation = service.begin_freeze();
    overlay::set_pending_freeze(generation, cx);
    let mut open = Some(open);
    let opened_early = deferred_show.then(|| open.take().unwrap()(cx, true, generation));
    let freezing = service.clone();
    cx.spawn(async move |cx| {
        let freezer = freezing.clone();
        #[cfg(all(test, windows))]
        freezer
            .freeze
            .scheduled_for_test
            .store(generation, std::sync::atomic::Ordering::SeqCst);
        let (frozen, released) = cx
            .background_executor()
            .spawn(async move {
                freezer.wait_for_freeze_turn(generation);
                let _operation = freezer.freeze.operation.lock();
                let released = release_first
                    .filter(|generation| *generation != 0)
                    .is_none()
                    || freezer.release_screen_started();
                let result = if released {
                    freezer.freeze_screen_started()
                } else {
                    Err(anyhow::anyhow!("Could not release the frozen screen"))
                };
                let released = released && (result.is_ok() || freezer.release_screen_started());
                (freezer.finish_freeze(generation, result), released)
            })
            .await;
        if !released {
            cx.update(|cx| {
                if overlay::revision(cx) != owner {
                    return;
                }
                let pending = overlay::replace_all(cx).unwrap_or(generation);
                overlay::set_pending_freeze(pending, cx);
                let coordinator = crate::state::coordinator(cx);
                coordinator.update(cx, |coordinator, cx| coordinator.cancel_editor_attach(cx));
                crate::windows::toast::Toast::show(
                    cx,
                    failure_title,
                    "Could not release the frozen screen",
                );
            });
            return;
        }
        if let Err(error) = frozen {
            // A failed freeze must not cost the user the capture; the overlay
            // opens over the live screen instead.
            eprintln!("[freeze] {error}");
        }
        let opened = if let Some(open) = open {
            cx.update(|cx| {
                if overlay::revision(cx) != owner {
                    return false;
                }
                let opened = open(cx, false, generation);
                if opened {
                    overlay::clear_pending_freeze(generation, owner, cx);
                }
                opened
            })
        } else {
            #[cfg(windows)]
            cx.update(|cx| {
                if opened_early == Some(true) {
                    overlay::clear_pending_freeze(generation, owner, cx);
                }
                overlay::raise_all(generation, cx);
            });
            opened_early.unwrap()
        };
        if !opened {
            cx.update(|cx| overlay::release_capture_hide(capture_hide, cx));
            let released = cx
                .background_executor()
                .spawn(async move { freezing.release_screen(generation) })
                .await;
            if released {
                cx.update(|cx| overlay::clear_pending_freeze(generation, owner, cx));
            }
        }
    })
    .detach();
}

fn hides_icons_before_selection(intent: intent::CaptureIntent) -> bool {
    matches!(
        intent,
        intent::CaptureIntent::Timer | intent::CaptureIntent::ScrollCapture
    )
}

/// Opens the shared area overlay for one of the selection-driven flows.
pub fn start_area_selection(intent: intent::CaptureIntent, cx: &mut gpui::App) {
    if intent != intent::CaptureIntent::EditorAttach
        && crate::state::coordinator(cx)
            .read(cx)
            .has_pending_editor_attach()
    {
        return;
    }
    if !capture_topology_supported(cx) {
        if intent == intent::CaptureIntent::EditorAttach {
            let coordinator = crate::state::coordinator(cx);
            cx.defer(move |cx| {
                coordinator.update(cx, |coordinator, cx| coordinator.cancel_editor_attach(cx));
            });
        }
        return;
    }
    let release_first = overlay::replace_all(cx);
    if hides_icons_before_selection(intent) {
        let service = crate::state::state(cx);
        let hide = desktop_icons::hide_for_capture(&service.daemon, &service.config);
        overlay::set_capture_hide(hide, cx);
    }
    let freeze = intent.allows_freeze();
    with_frozen_screen(
        cx,
        release_first,
        freeze,
        if intent == intent::CaptureIntent::Recording {
            "Recording failed"
        } else {
            "Capture Failed"
        },
        move |cx, deferred_show, generation| {
            let opened = each_display(cx, |service, id, bounds, focus, cx| {
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
            });
            if !opened && intent == intent::CaptureIntent::EditorAttach {
                let coordinator = crate::state::coordinator(cx);
                cx.defer(move |cx| {
                    coordinator.update(cx, |coordinator, cx| coordinator.cancel_editor_attach(cx));
                });
            }
            opened
        },
    );
}

/// Opens the all-in-one overlay: one surface that switches between the
/// screenshot, recording and OCR flows over area, window or screen.
pub fn start_all_in_one(cx: &mut gpui::App) {
    if crate::state::coordinator(cx)
        .read(cx)
        .has_pending_editor_attach()
    {
        return;
    }
    if !capture_topology_supported(cx) {
        return;
    }
    let choices = all_in_one::restore(&crate::state::state(cx).config);
    let release_first = overlay::replace_all(cx);
    with_frozen_screen(
        cx,
        release_first,
        true,
        if choices.mode == all_in_one::Mode::Record {
            "Recording failed"
        } else {
            "Capture Failed"
        },
        move |cx, deferred_show, generation| {
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
        },
    );
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

#[cfg(target_os = "linux")]
fn selected_display(cx: &mut gpui::App) -> Option<std::rc::Rc<dyn gpui::PlatformDisplay>> {
    cx.primary_display()
        .or_else(|| cx.displays().into_iter().next())
}

/// Starts a screen recording: with several displays the user picks one
/// through the same click-to-pick overlay the screen screenshot flow uses
/// (`recordScreen` picks through the overlay too); a click opens the
/// pre-recording bar for that display.
pub fn start_screen_recording(cx: &mut gpui::App) {
    if crate::state::coordinator(cx)
        .read(cx)
        .has_pending_editor_attach()
    {
        return;
    }
    let targets = match screen_targets(cx) {
        Ok(targets) if !targets.is_empty() => targets,
        Ok(_) => {
            crate::windows::toast::Toast::show(cx, "Recording failed", "No display available");
            return;
        }
        Err(error) => {
            crate::windows::toast::Toast::show(cx, "Recording failed", error.to_string());
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
        with_frozen_screen(
            cx,
            release_first,
            false,
            "Recording failed",
            move |cx, deferred_show, generation| {
                let service = crate::state::state(cx);
                let target_count = targets.len();
                let mut opened = 0;
                for target in targets {
                    if overlay::AreaOverlay::open_screen_picker(
                        service.clone(),
                        target.display_id,
                        target.bounds,
                        intent::CaptureIntent::Recording,
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
                let _ = overlay::replace_all(cx);
                let fallback = fallback.clone();
                cx.defer(move |cx| open_screen_recording_bar(&fallback, cx));
                false
            },
        );
        return;
    }

    let target = targets[0].clone();
    let release_first = overlay::replace_all(cx);
    with_frozen_screen(
        cx,
        release_first,
        false,
        "Recording failed",
        move |cx, _, _| {
            open_screen_recording_bar(&target, cx);
            true
        },
    );
}

fn open_screen_recording_bar(target: &ScreenTarget, cx: &mut gpui::App) {
    crate::windows::recording_control::RecordingControl::open(
        cx,
        crate::video::recorder::RecordingTarget::Screen,
        overlay::physical_rect(target.bounds, target.scale),
        target.capture_display_id,
        None,
        None,
        crate::windows::recording_control::PreferenceScope::Global,
    );
}

fn start_window_picker(intent: intent::CaptureIntent, cx: &mut gpui::App) {
    if crate::state::coordinator(cx)
        .read(cx)
        .has_pending_editor_attach()
    {
        return;
    }
    let service = crate::state::state(cx);
    let release_first = overlay::replace_all(cx);
    with_frozen_screen(
        cx,
        release_first,
        intent == intent::CaptureIntent::Screenshot,
        if intent == intent::CaptureIntent::Recording {
            "Recording failed"
        } else {
            "Capture Failed"
        },
        move |cx, deferred_show, generation| {
            let mut pending = Vec::new();
            each_display(cx, |service, id, bounds, focus, cx| {
                let Some(handle) = overlay::AreaOverlay::open_with_windows(
                    service,
                    id,
                    bounds,
                    intent,
                    Vec::new(),
                    overlay::OverlayLaunch {
                        focus,
                        deferred_show,
                        generation,
                    },
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
                return false;
            }
            let daemon = service.daemon.clone();
            cx.spawn(async move |cx| {
                let windows = cx
                    .background_executor()
                    .spawn(async move { windows_list::list(&daemon) })
                    .await;
                cx.update(|cx| {
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
            true
        },
    );
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
        let capture_display_id = Some(u64::from(display.id()) as u32);
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
    let service = crate::state::state(cx);
    let hide = desktop_icons::hide_for_capture(&service.daemon, &service.config);
    let capture = overlay::display_capture(target.bounds, target.scale, target.capture_display_id);
    let coordinator = crate::state::coordinator(cx);
    coordinator.update(cx, |coordinator, cx| {
        coordinator.capture_area_reserved(
            capture,
            intent::CaptureIntent::Screenshot,
            None,
            None,
            hide,
            None,
            cx,
        );
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
        if let Some(generation) = overlay::replace_all(cx).filter(|generation| *generation != 0) {
            overlay::set_pending_freeze(generation, cx);
            overlay::release_frozen_screen(generation, cx);
        }
    }
    let hide = desktop_icons::hide_for_capture(&service.daemon, &service.config);
    cx.defer(move |cx| {
        coordinator.update(cx, |coordinator, cx| {
            coordinator.capture_area_reserved(
                capture,
                intent::CaptureIntent::Screenshot,
                reservation,
                None,
                hide,
                None,
                cx,
            );
        });
    });
}

fn all_screen_targets_opened(opened: usize, target_count: usize) -> bool {
    opened == target_count
}

pub fn start_screen_capture(cx: &mut gpui::App) {
    if crate::state::coordinator(cx)
        .read(cx)
        .has_pending_editor_attach()
    {
        return;
    }
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

    let release_first = overlay::replace_all(cx);

    if targets.len() > 1
        && crate::system::capabilities::is_supported(
            crate::system::capabilities::Feature::DisplaySelector,
        )
    {
        let fallback = targets[0].clone();
        with_frozen_screen(
            cx,
            release_first,
            true,
            "Capture Failed",
            move |cx, deferred_show, generation| {
                let service = crate::state::state(cx);
                let target_count = targets.len();
                let mut opened = 0;
                for target in targets {
                    if overlay::AreaOverlay::open_screen_picker(
                        service.clone(),
                        target.display_id,
                        target.bounds,
                        intent::CaptureIntent::Screenshot,
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
            },
        );
        return;
    }

    let target = targets[0].clone();
    let owner = overlay::revision(cx);
    let Some(generation) = release_first.filter(|generation| *generation != 0) else {
        cx.defer(move |cx| {
            if overlay::revision(cx) == owner {
                capture_screen_target(target, cx);
            }
        });
        return;
    };
    let service = crate::state::state(cx);
    overlay::set_pending_freeze(generation, cx);
    cx.spawn(async move |cx| {
        let released = cx
            .background_executor()
            .spawn(async move { service.release_screen(generation) })
            .await;
        cx.update(|cx| {
            if overlay::revision(cx) != owner {
                return;
            }
            if !released {
                crate::windows::toast::Toast::show(
                    cx,
                    "Capture Failed",
                    "Could not release the frozen screen",
                );
                return;
            }
            overlay::clear_pending_freeze(generation, owner, cx);
            capture_screen_target(target, cx);
        });
    })
    .detach();
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use super::CaptureService;
    use crate::config::store::ConfigStore;
    use crate::daemon::DaemonHandle;

    #[cfg(windows)]
    fn queue_pending_freeze(cx: &mut gpui::TestAppContext, service: &CaptureService) -> u64 {
        cx.update(|cx| super::start_area_selection(super::intent::CaptureIntent::Screenshot, cx));
        let generation = service.freeze.state.lock().next_generation;
        while service
            .freeze
            .scheduled_for_test
            .load(std::sync::atomic::Ordering::SeqCst)
            < generation
        {
            assert!(cx.dispatcher.tick(false));
        }
        assert!(!cx.update(super::overlay::parity_is_active));
        generation
    }

    #[cfg(windows)]
    fn finish_queued_freeze(cx: &gpui::TestAppContext, service: &CaptureService, generation: u64) {
        while service.freeze.state.lock().completed_generation < generation {
            assert!(cx.dispatcher.tick(true));
        }
    }

    #[test]
    fn concurrent_captures_keep_existing_files_and_each_others_pixels() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("screenshots").join("Capture.png");
        let stages: Vec<_> = (0..12)
            .map(|index| {
                let stage = super::prepare_screenshot(&output).unwrap();
                std::fs::write(&stage, format!("capture {index}")).unwrap();
                stage
            })
            .collect();
        std::fs::write(&output, "original").unwrap();
        let barrier = Arc::new(std::sync::Barrier::new(stages.len()));
        let workers: Vec<_> = stages
            .into_iter()
            .map(|stage| {
                let barrier = barrier.clone();
                let output = output.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    super::publish_screenshot(&stage, &output).unwrap()
                })
            })
            .collect();
        let paths: std::collections::HashSet<_> = workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect();
        assert_eq!(paths.len(), 12);
        assert_eq!(std::fs::read_to_string(&output).unwrap(), "original");
        let contents: std::collections::HashSet<_> = paths
            .iter()
            .map(|path| std::fs::read_to_string(path).unwrap())
            .collect();
        assert_eq!(
            contents,
            (0..12).map(|index| format!("capture {index}")).collect()
        );
        assert_eq!(
            std::fs::read_dir(output.parent().unwrap()).unwrap().count(),
            13
        );
    }

    #[test]
    fn failed_screenshot_publication_retains_the_temporary_capture() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("Capture.png");
        let stage = super::prepare_screenshot(&output).unwrap();
        std::fs::write(&stage, "capture").unwrap();
        assert!(
            super::publish_screenshot(&stage, &directory.path().join("missing/Capture.png"))
                .is_err()
        );
        assert_eq!(std::fs::read_to_string(stage).unwrap(), "capture");
    }

    #[test]
    fn only_the_timer_and_scroll_flows_hide_icons_before_the_selection() {
        use crate::capture::intent::CaptureIntent;

        assert!(super::hides_icons_before_selection(CaptureIntent::Timer));
        assert!(super::hides_icons_before_selection(
            CaptureIntent::ScrollCapture
        ));
        assert!(!super::hides_icons_before_selection(
            CaptureIntent::Screenshot
        ));
        assert!(!super::hides_icons_before_selection(
            CaptureIntent::Recording
        ));
    }

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
    #[herogpui::test]
    fn screen_capture_closes_an_existing_area_selector(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let config = Arc::new(ConfigStore::load_at(dir.path().join("config.json")).unwrap());
        config.update(|settings| {
            settings.screenshot.hide_desktop_icons = false;
            settings.storage.screenshots_path = dir.path().to_string_lossy().into_owned();
        });
        let notifications = crate::system::notification::capture_for_test();
        cx.update(|cx| {
            crate::state::set_test_state(cx, config.clone());
            super::overlay::init_bindings(cx);
        });
        assert_eq!(cx.read(|cx| cx.displays().len()), 1);
        for freeze in [false, true] {
            config.update(|settings| settings.screenshot.freeze_screen = freeze);
            cx.update(|cx| {
                super::start_area_selection(super::intent::CaptureIntent::Screenshot, cx);
            });
            cx.run_until_parked();
            assert!(cx.update(super::overlay::parity_is_active));
            cx.update(super::start_screen_capture);
            cx.run_until_parked();
            assert!(!cx.update(super::overlay::parity_is_active));
        }
        assert_eq!(
            *notifications.borrow(),
            ["Capture Failed", "Capture Failed"]
        );
        notifications.borrow_mut().clear();
        for freeze in [false, true] {
            config.update(|settings| settings.screenshot.freeze_screen = freeze);
            cx.update(|cx| {
                super::start_area_selection(super::intent::CaptureIntent::Screenshot, cx);
            });
            cx.run_until_parked();
            cx.update(|cx| {
                super::start_screen_capture(cx);
                super::start_area_selection(super::intent::CaptureIntent::Screenshot, cx);
            });
            cx.run_until_parked();
            assert!(cx.update(super::overlay::parity_is_active));
            assert!(notifications.borrow().is_empty());
            cx.update(super::overlay::close_all);
            cx.run_until_parked();
        }
    }

    #[cfg(windows)]
    #[herogpui::test]
    fn pending_freeze_does_not_reopen_a_superseded_selector(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let config = Arc::new(ConfigStore::load_at(dir.path().join("config.json")).unwrap());
        config.update(|settings| {
            settings.screenshot.freeze_screen = true;
            settings.screenshot.hide_desktop_icons = false;
            settings.storage.screenshots_path = dir.path().to_string_lossy().into_owned();
        });
        let notifications = crate::system::notification::capture_for_test();
        let service = cx.update(|cx| {
            crate::state::set_test_state(cx, config);
            super::overlay::init_bindings(cx);
            let service = crate::state::state(cx);
            service
                .freeze
                .defer_open_for_test
                .store(true, std::sync::atomic::Ordering::SeqCst);
            service
                .freeze
                .freeze_succeeds_for_test
                .store(true, std::sync::atomic::Ordering::SeqCst);
            service
        });
        let generation = queue_pending_freeze(cx, &service);
        cx.update(super::start_screen_capture);
        finish_queued_freeze(cx, &service, generation);
        cx.run_until_parked();
        assert!(!cx.update(super::overlay::parity_is_active));
        assert_eq!(*notifications.borrow(), ["Capture Failed"]);
    }

    #[cfg(windows)]
    #[herogpui::test]
    fn replacement_selectors_wait_for_a_pending_freeze(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let config = Arc::new(ConfigStore::load_at(dir.path().join("config.json")).unwrap());
        config.update(|settings| {
            settings.screenshot.freeze_screen = true;
            settings.screenshot.hide_desktop_icons = false;
        });
        let service = cx.update(|cx| {
            crate::state::set_test_state(cx, config);
            super::overlay::init_bindings(cx);
            let service = crate::state::state(cx);
            service
                .freeze
                .defer_open_for_test
                .store(true, std::sync::atomic::Ordering::SeqCst);
            service
        });
        for replacement in [
            super::intent::CaptureIntent::ScrollCapture,
            super::intent::CaptureIntent::Ocr,
        ] {
            let generation = queue_pending_freeze(cx, &service);
            cx.update(|cx| {
                super::start_area_selection(replacement, cx);
                assert!(!super::overlay::parity_is_active(cx));
            });
            finish_queued_freeze(cx, &service, generation);
            cx.run_until_parked();
            assert_eq!(
                cx.update(super::overlay::active_intents_for_test),
                [replacement]
            );
            cx.update(super::overlay::close_all);
            cx.run_until_parked();
        }
    }

    #[cfg(windows)]
    #[herogpui::test]
    fn pending_freeze_ownership_transfers_across_multiple_replacements(
        cx: &mut gpui::TestAppContext,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let config = Arc::new(ConfigStore::load_at(dir.path().join("config.json")).unwrap());
        config.update(|settings| {
            settings.screenshot.freeze_screen = true;
            settings.screenshot.hide_desktop_icons = false;
        });
        let notifications = crate::system::notification::capture_for_test();
        let service = cx.update(|cx| {
            crate::state::set_test_state(cx, config);
            super::overlay::init_bindings(cx);
            let service = crate::state::state(cx);
            service
                .freeze
                .defer_open_for_test
                .store(true, std::sync::atomic::Ordering::SeqCst);
            service
                .freeze
                .freeze_succeeds_for_test
                .store(true, std::sync::atomic::Ordering::SeqCst);
            service
        });
        for screen in [false, true] {
            let generation = queue_pending_freeze(cx, &service);
            cx.update(|cx| {
                if screen {
                    super::start_screen_capture(cx);
                } else {
                    super::start_area_selection(super::intent::CaptureIntent::ScrollCapture, cx);
                }
                super::start_area_selection(super::intent::CaptureIntent::Ocr, cx);
                assert!(!super::overlay::parity_is_active(cx));
            });
            finish_queued_freeze(cx, &service, generation);
            cx.run_until_parked();
            assert_eq!(
                cx.update(super::overlay::active_intents_for_test),
                [super::intent::CaptureIntent::Ocr]
            );
            assert!(service.reserve_cached_capture().wait_for_freeze());
            assert!(notifications.borrow().is_empty());
            cx.update(super::overlay::close_all);
            cx.run_until_parked();
        }
        let generation = queue_pending_freeze(cx, &service);
        cx.update(super::overlay::close_all);
        finish_queued_freeze(cx, &service, generation);
        cx.run_until_parked();
        assert!(!cx.update(super::overlay::parity_is_active));
    }

    #[cfg(windows)]
    #[herogpui::test]
    fn failed_freeze_release_is_retried_by_the_next_capture(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let config = Arc::new(ConfigStore::load_at(dir.path().join("config.json")).unwrap());
        config.update(|settings| {
            settings.screenshot.freeze_screen = false;
            settings.screenshot.hide_desktop_icons = false;
            settings.storage.screenshots_path = dir.path().to_string_lossy().into_owned();
        });
        let notifications = crate::system::notification::capture_for_test();
        let service = cx.update(|cx| {
            crate::state::set_test_state(cx, config);
            super::overlay::init_bindings(cx);
            crate::state::state(cx)
        });
        for screen in [false, true] {
            let generation = service.begin_freeze();
            service.finish_freeze(generation, Ok(())).unwrap();
            service
                .freeze
                .release_fails_for_test
                .store(true, std::sync::atomic::Ordering::SeqCst);
            cx.update(|cx| {
                super::overlay::set_pending_freeze(generation, cx);
                if screen {
                    super::start_screen_capture(cx);
                } else {
                    super::start_area_selection(super::intent::CaptureIntent::ScrollCapture, cx);
                }
            });
            cx.run_until_parked();
            assert!(!cx.update(super::overlay::parity_is_active));
            assert_eq!(*notifications.borrow(), ["Capture Failed"]);
            notifications.borrow_mut().clear();
            cx.update(|cx| {
                assert_eq!(super::overlay::replace_all(cx), Some(generation));
                super::overlay::set_pending_freeze(generation, cx);
            });
            service
                .freeze
                .release_fails_for_test
                .store(false, std::sync::atomic::Ordering::SeqCst);
            cx.update(|cx| {
                if screen {
                    super::start_screen_capture(cx);
                } else {
                    super::start_area_selection(super::intent::CaptureIntent::ScrollCapture, cx);
                }
            });
            cx.run_until_parked();
            if screen {
                assert_eq!(*notifications.borrow(), ["Capture Failed"]);
                notifications.borrow_mut().clear();
                assert_eq!(cx.update(super::overlay::replace_all), None);
            } else {
                assert_eq!(
                    cx.update(super::overlay::active_intents_for_test),
                    [super::intent::CaptureIntent::ScrollCapture]
                );
                cx.update(super::overlay::close_all);
                cx.run_until_parked();
            }
        }
        let generation = service.begin_freeze();
        service.finish_freeze(generation, Ok(())).unwrap();
        service
            .freeze
            .release_fails_for_test
            .store(true, std::sync::atomic::Ordering::SeqCst);
        cx.update(|cx| {
            super::overlay::set_pending_freeze(generation, cx);
            super::overlay::close_all(cx);
        });
        cx.run_until_parked();
        cx.update(|cx| {
            assert_eq!(super::overlay::replace_all(cx), Some(generation));
            super::overlay::set_pending_freeze(generation, cx);
        });
        service
            .freeze
            .release_fails_for_test
            .store(false, std::sync::atomic::Ordering::SeqCst);
        cx.update(|cx| {
            super::start_area_selection(super::intent::CaptureIntent::ScrollCapture, cx);
        });
        cx.run_until_parked();
        assert_eq!(
            cx.update(super::overlay::active_intents_for_test),
            [super::intent::CaptureIntent::ScrollCapture]
        );
        cx.update(super::overlay::close_all);
        cx.run_until_parked();
    }

    #[cfg(windows)]
    #[herogpui::test]
    fn failed_release_blocks_a_replacement_freeze_until_retry(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let config = Arc::new(ConfigStore::load_at(dir.path().join("config.json")).unwrap());
        config.update(|settings| {
            settings.screenshot.freeze_screen = true;
            settings.screenshot.hide_desktop_icons = false;
        });
        let notifications = crate::system::notification::capture_for_test();
        let service = cx.update(|cx| {
            crate::state::set_test_state(cx, config);
            super::overlay::init_bindings(cx);
            let service = crate::state::state(cx);
            service
                .freeze
                .freeze_succeeds_for_test
                .store(true, std::sync::atomic::Ordering::SeqCst);
            service
        });
        for deferred in [false, true] {
            service
                .freeze
                .defer_open_for_test
                .store(deferred, std::sync::atomic::Ordering::SeqCst);
            let original = service.begin_freeze();
            service.finish_freeze(original, Ok(())).unwrap();
            service
                .freeze
                .release_fails_for_test
                .store(true, std::sync::atomic::Ordering::SeqCst);
            cx.update(|cx| {
                super::overlay::set_pending_freeze(original, cx);
                super::start_area_selection(super::intent::CaptureIntent::Ocr, cx);
                assert_eq!(super::overlay::parity_is_active(cx), !deferred);
            });
            cx.run_until_parked();
            assert!(!cx.update(super::overlay::parity_is_active));
            assert_eq!(*notifications.borrow(), ["Capture Failed"]);
            notifications.borrow_mut().clear();
            assert_eq!(service.freeze.state.lock().frozen_generation, None);
            cx.update(|cx| {
                assert_eq!(super::overlay::replace_all(cx), Some(original + 1));
                super::overlay::set_pending_freeze(original + 1, cx);
            });
            service
                .freeze
                .release_fails_for_test
                .store(false, std::sync::atomic::Ordering::SeqCst);
            cx.update(|cx| super::start_area_selection(super::intent::CaptureIntent::Ocr, cx));
            cx.run_until_parked();
            assert_eq!(
                cx.update(super::overlay::active_intents_for_test),
                [super::intent::CaptureIntent::Ocr]
            );
            assert!(service.reserve_cached_capture().wait_for_freeze());
            cx.update(super::overlay::close_all);
            cx.run_until_parked();
        }
    }

    #[cfg(windows)]
    #[herogpui::test]
    fn failed_freeze_is_released_before_live_fallback(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let config = Arc::new(ConfigStore::load_at(dir.path().join("config.json")).unwrap());
        config.update(|settings| {
            settings.screenshot.freeze_screen = true;
            settings.screenshot.hide_desktop_icons = false;
        });
        let notifications = crate::system::notification::capture_for_test();
        let service = cx.update(|cx| {
            crate::state::set_test_state(cx, config);
            super::overlay::init_bindings(cx);
            crate::state::state(cx)
        });
        for deferred in [false, true] {
            service
                .freeze
                .defer_open_for_test
                .store(deferred, std::sync::atomic::Ordering::SeqCst);
            service
                .freeze
                .release_fails_for_test
                .store(true, std::sync::atomic::Ordering::SeqCst);
            cx.update(|cx| {
                super::start_area_selection(super::intent::CaptureIntent::Screenshot, cx)
            });
            cx.run_until_parked();
            assert!(!cx.update(super::overlay::parity_is_active));
            assert_eq!(*notifications.borrow(), ["Capture Failed"]);
            notifications.borrow_mut().clear();
            service
                .freeze
                .release_fails_for_test
                .store(false, std::sync::atomic::Ordering::SeqCst);
            cx.update(|cx| {
                super::start_area_selection(super::intent::CaptureIntent::Screenshot, cx)
            });
            cx.run_until_parked();
            assert!(cx.update(super::overlay::parity_is_active));
            assert!(!service.reserve_cached_capture().wait_for_freeze());
            cx.update(super::overlay::close_all);
            cx.run_until_parked();
        }
    }

    #[cfg(windows)]
    #[herogpui::test]
    fn failed_release_after_overlay_open_failure_remains_pending(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let config = Arc::new(ConfigStore::load_at(dir.path().join("config.json")).unwrap());
        config.update(|settings| settings.screenshot.freeze_screen = true);
        let service = cx.update(|cx| {
            crate::state::set_test_state(cx, config);
            let service = crate::state::state(cx);
            service
                .freeze
                .defer_open_for_test
                .store(true, std::sync::atomic::Ordering::SeqCst);
            service
                .freeze
                .freeze_succeeds_for_test
                .store(true, std::sync::atomic::Ordering::SeqCst);
            service
                .freeze
                .release_fails_for_test
                .store(true, std::sync::atomic::Ordering::SeqCst);
            super::with_frozen_screen(cx, None, true, "Capture Failed", |_, _, _| false);
            service
        });
        cx.run_until_parked();
        assert_eq!(cx.update(super::overlay::replace_all), Some(1));
        service
            .freeze
            .release_fails_for_test
            .store(false, std::sync::atomic::Ordering::SeqCst);
        assert!(service.release_screen(1));
    }

    #[cfg(windows)]
    #[herogpui::test]
    fn screen_recording_replaces_an_existing_selector(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let config = Arc::new(ConfigStore::load_at(dir.path().join("config.json")).unwrap());
        config.update(|settings| {
            settings.screenshot.freeze_screen = false;
            settings.screenshot.hide_desktop_icons = false;
            settings.recording.camera.enabled = false;
        });
        cx.update(|cx| {
            crate::state::set_test_state(cx, config);
            super::overlay::init_bindings(cx);
            super::start_area_selection(super::intent::CaptureIntent::Screenshot, cx);
            assert!(super::overlay::parity_is_active(cx));
            super::start_screen_recording(cx);
            assert!(!super::overlay::parity_is_active(cx));
            assert!(crate::windows::registry::is_open(
                crate::windows::registry::WindowKind::RecordingControl,
                cx
            ));
            crate::windows::registry::close(
                crate::windows::registry::WindowKind::RecordingControl,
                cx,
            );
        });
        cx.run_until_parked();
    }

    #[cfg(windows)]
    #[herogpui::test]
    fn screen_recording_waits_for_pending_freeze_and_respects_replacement(
        cx: &mut gpui::TestAppContext,
    ) {
        use crate::windows::registry::{self, WindowKind};

        let dir = tempfile::tempdir().unwrap();
        let config = Arc::new(ConfigStore::load_at(dir.path().join("config.json")).unwrap());
        config.update(|settings| {
            settings.screenshot.freeze_screen = true;
            settings.screenshot.hide_desktop_icons = false;
            settings.recording.camera.enabled = false;
        });
        let notifications = crate::system::notification::capture_for_test();
        let service = cx.update(|cx| {
            crate::state::set_test_state(cx, config.clone());
            super::overlay::init_bindings(cx);
            let service = crate::state::state(cx);
            service
                .freeze
                .defer_open_for_test
                .store(true, std::sync::atomic::Ordering::SeqCst);
            service
                .freeze
                .freeze_succeeds_for_test
                .store(true, std::sync::atomic::Ordering::SeqCst);
            service
        });
        for replace in [false, true] {
            let generation = queue_pending_freeze(cx, &service);
            cx.update(|cx| {
                super::start_screen_recording(cx);
                assert!(!registry::is_open(WindowKind::RecordingControl, cx));
                if replace {
                    super::start_area_selection(super::intent::CaptureIntent::Ocr, cx);
                }
            });
            finish_queued_freeze(cx, &service, generation);
            cx.run_until_parked();
            cx.update(|cx| {
                assert_eq!(
                    registry::is_open(WindowKind::RecordingControl, cx),
                    !replace
                );
                assert_eq!(super::overlay::parity_is_active(cx), replace);
                registry::close(WindowKind::RecordingControl, cx);
                super::overlay::close_all(cx);
            });
            cx.run_until_parked();
            assert!(notifications.borrow().is_empty());
        }
        config.update(|settings| settings.screenshot.freeze_screen = false);
        cx.update(|cx| {
            super::start_screen_capture(cx);
            super::start_screen_recording(cx);
            assert!(registry::is_open(WindowKind::RecordingControl, cx));
        });
        cx.run_until_parked();
        assert!(notifications.borrow().is_empty());
        cx.update(|cx| registry::close(WindowKind::RecordingControl, cx));
    }

    #[cfg(windows)]
    #[herogpui::test]
    fn screen_recording_retries_a_failed_freeze_release(cx: &mut gpui::TestAppContext) {
        use crate::windows::registry::{self, WindowKind};

        let dir = tempfile::tempdir().unwrap();
        let config = Arc::new(ConfigStore::load_at(dir.path().join("config.json")).unwrap());
        config.update(|settings| settings.recording.camera.enabled = false);
        let notifications = crate::system::notification::capture_for_test();
        let service = cx.update(|cx| {
            crate::state::set_test_state(cx, config);
            crate::state::state(cx)
        });
        let generation = service.begin_freeze();
        service.finish_freeze(generation, Ok(())).unwrap();
        service
            .freeze
            .release_fails_for_test
            .store(true, std::sync::atomic::Ordering::SeqCst);
        cx.update(|cx| {
            super::overlay::set_pending_freeze(generation, cx);
            super::start_screen_recording(cx);
        });
        cx.run_until_parked();
        assert!(!cx.update(|cx| registry::is_open(WindowKind::RecordingControl, cx)));
        assert_eq!(*notifications.borrow(), ["Recording failed"]);
        service
            .freeze
            .release_fails_for_test
            .store(false, std::sync::atomic::Ordering::SeqCst);
        cx.update(super::start_screen_recording);
        cx.run_until_parked();
        cx.update(|cx| {
            assert!(registry::is_open(WindowKind::RecordingControl, cx));
            registry::close(WindowKind::RecordingControl, cx);
        });
    }

    #[cfg(windows)]
    #[herogpui::test]
    fn all_in_one_release_failure_matches_the_restored_mode(cx: &mut gpui::TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let config = Arc::new(ConfigStore::load_at(dir.path().join("config.json")).unwrap());
        config.update(|settings| {
            settings.screenshot.freeze_screen = true;
            settings.all_in_one.remember_choices = true;
        });
        let notifications = crate::system::notification::capture_for_test();
        let service = cx.update(|cx| {
            crate::state::set_test_state(cx, config.clone());
            super::overlay::init_bindings(cx);
            crate::state::state(cx)
        });
        for (mode, title) in [
            ("screenshot", "Capture Failed"),
            ("record", "Recording failed"),
        ] {
            config.update(|settings| settings.all_in_one.last_mode = mode.into());
            let generation = service.begin_freeze();
            service.finish_freeze(generation, Ok(())).unwrap();
            service
                .freeze
                .release_fails_for_test
                .store(true, std::sync::atomic::Ordering::SeqCst);
            cx.update(|cx| {
                super::overlay::set_pending_freeze(generation, cx);
                super::start_all_in_one(cx);
            });
            cx.run_until_parked();
            assert!(!cx.update(super::overlay::parity_is_active));
            assert_eq!(*notifications.borrow(), [title]);
            notifications.borrow_mut().clear();
            service
                .freeze
                .release_fails_for_test
                .store(false, std::sync::atomic::Ordering::SeqCst);
            cx.update(super::overlay::close_all);
            cx.run_until_parked();
        }
    }

    #[herogpui::test]
    fn unavailable_attachment_selector_does_not_leave_the_editor_pending(
        cx: &mut gpui::TestAppContext,
    ) {
        use crate::editor::layers::Edge;
        use crate::editor::window::EditorWindow;

        let dir = tempfile::tempdir().expect("temp dir");
        let config =
            Arc::new(ConfigStore::load_at(dir.path().join("config.json")).expect("load config"));
        cx.update(|cx| crate::state::set_test_state(cx, config.clone()));
        let image = dir.path().join("capture.png");
        image::RgbaImage::from_pixel(64, 64, image::Rgba([64, 112, 192, 255]))
            .save(&image)
            .unwrap();
        let editors: Vec<_> = (0..2)
            .map(|_| {
                cx.add_window(|window, cx| {
                    EditorWindow::from_file(image.to_str().unwrap(), window, cx)
                })
            })
            .collect();
        let displays = cx.read(|cx| {
            cx.displays()
                .iter()
                .map(|display| display.id())
                .collect::<Vec<_>>()
        });
        for display in displays {
            cx.simulate_display_removed(display);
        }
        for (index, freeze) in [false, true].into_iter().enumerate() {
            let editor = editors[index];
            editors[1 - index]
                .update(cx, |_, window, _| window.activate_window())
                .unwrap();
            assert_ne!(cx.read(|cx| cx.active_window()), Some(editor.into()));
            config.update(|config| config.screenshot.freeze_screen = freeze);
            editor
                .update(cx, |editor, window, cx| {
                    editor.attach_layer(Edge::Right, window, cx)
                })
                .unwrap();
            cx.run_until_parked();
            assert!(!cx.read(|cx| crate::state::coordinator(cx)
                .read(cx)
                .has_pending_editor_attach()));
            assert_eq!(cx.read(|cx| cx.active_window()), Some(editor.into()));
        }
    }

    #[cfg(windows)]
    #[herogpui::test]
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
            super::with_frozen_screen(
                cx,
                None,
                true,
                "Capture Failed",
                move |_, deferred_show, generation| {
                    assert!(deferred_show);
                    assert_eq!(generation, 1);
                    observed.store(true, Ordering::SeqCst);
                    true
                },
            );
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
    fn release_does_not_block_a_pending_freeze() {
        let dir = tempfile::tempdir().unwrap();
        let config = Arc::new(ConfigStore::load_at(dir.path().join("config.json")).unwrap());
        let service =
            CaptureService::new(DaemonHandle::with_binary(std::path::PathBuf::new()), config);
        let generation = service.begin_freeze();
        let (waiting_tx, waiting_rx) = std::sync::mpsc::sync_channel(1);
        *service.freeze.release_waiting.lock() = Some(waiting_tx);
        let releasing = service.clone();
        let release = std::thread::spawn(move || releasing.release_screen(generation));
        waiting_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let freezing = service.clone();
        let (locked_tx, locked_rx) = std::sync::mpsc::channel();
        let freeze = std::thread::spawn(move || {
            let _operation = freezing.freeze.operation.lock();
            locked_tx.send(()).unwrap();
            freezing.finish_freeze(generation, Ok(())).unwrap();
        });
        let freeze_started = locked_rx.recv_timeout(Duration::from_secs(1)).is_ok();
        if !freeze_started {
            service.finish_freeze(generation, Ok(())).unwrap();
        }
        assert!(release.join().unwrap());
        freeze.join().unwrap();
        assert!(freeze_started, "release blocked the pending freeze");
    }

    #[test]
    fn release_rechecks_reservations_without_holding_the_operation_lock() {
        let dir = tempfile::tempdir().unwrap();
        let config = Arc::new(ConfigStore::load_at(dir.path().join("config.json")).unwrap());
        let service =
            CaptureService::new(DaemonHandle::with_binary(std::path::PathBuf::new()), config);
        let generation = service.begin_freeze();
        service.finish_freeze(generation, Ok(())).unwrap();
        let operation = service.freeze.operation.lock();
        let (waiting_tx, waiting_rx) = std::sync::mpsc::sync_channel(1);
        *service.freeze.release_waiting.lock() = Some(waiting_tx);
        let releasing = service.clone();
        let release = std::thread::spawn(move || releasing.release_screen(generation));
        waiting_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let reservation = service.reserve_cached_capture();
        let (retry_tx, retry_rx) = std::sync::mpsc::sync_channel(1);
        *service.freeze.release_waiting.lock() = Some(retry_tx);
        drop(operation);
        retry_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(service.freeze.operation.try_lock().is_some());
        drop(reservation);
        assert!(release.join().unwrap());
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
