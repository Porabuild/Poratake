//! Application-wide services exposed as a GPUI global: capture pipeline,
//! config store, daemon handle and the native shell bridge.

use std::sync::Arc;

use gpui::{prelude::*, Entity};
use herogpui::gpui;

use crate::capture::coordinator::{Coordinator, CoordinatorHandle};
use crate::capture::CaptureService;
use crate::config::store::ConfigStore;
use crate::daemon::DaemonHandle;
use crate::system::native::NativeBridge;

pub struct AppState {
    pub service: CaptureService,
    #[cfg(target_os = "macos")]
    _recording_target_close: crate::daemon::EventSubscription,
    #[cfg(all(target_os = "macos", not(test)))]
    _tooltip_window_close: gpui::Subscription,
}

impl gpui::Global for AppState {}

/// The shared updater cell — Electron's module-level `updateState`. About
/// renders it, the tray maps it, and the auto-check publishes into it.
pub struct UpdateState(pub crate::update::Shared);

impl Default for UpdateState {
    fn default() -> Self {
        Self(std::sync::Arc::new(std::sync::Mutex::new(
            crate::update::UpdateCell::default(),
        )))
    }
}

impl gpui::Global for UpdateState {}

struct NativeShell(Arc<NativeBridge>);

impl gpui::Global for NativeShell {}

fn start_native_daemon(daemon: &DaemonHandle) -> bool {
    match daemon.start() {
        Ok(()) => true,
        Err(error) => {
            eprintln!("[daemon] failed to start: {error}");
            false
        }
    }
}

pub fn init(cx: &mut gpui::App, defer_daemon: bool) -> Arc<ConfigStore> {
    let config = match ConfigStore::load() {
        Ok(store) => Arc::new(store),
        Err(error) => {
            eprintln!("[settings] failed to load config: {error}");
            std::process::exit(1);
        }
    };

    let service = CaptureService::new(DaemonHandle::new(), config.clone());

    if crate::system::capabilities::has_native_daemon() && !cfg!(windows) {
        if defer_daemon {
            let daemon = service.daemon.clone();
            cx.spawn(async move |cx| {
                let started = cx
                    .background_executor()
                    .spawn(async move { start_native_daemon(&daemon) })
                    .await;
                if started {
                    cx.update(crate::capture::prewarm_freeze_screen);
                }
            })
            .detach();
        } else {
            start_native_daemon(&service.daemon);
        }
    }

    let coordinator = cx.new(|_| Coordinator::new(service.clone()));
    cx.set_global(CoordinatorHandle(coordinator));
    #[cfg(all(target_os = "macos", not(test)))]
    let tooltip_window_close =
        crate::system::macos_tooltip::init(cx, service.daemon.clone(), config.clone());
    #[cfg(target_os = "macos")]
    let recording_target_close =
        crate::windows::recording_control::RecordingControl::watch_window_target_closure(
            &service.daemon,
            cx,
        );
    cx.set_global(AppState {
        service,
        #[cfg(target_os = "macos")]
        _recording_target_close: recording_target_close,
        #[cfg(all(target_os = "macos", not(test)))]
        _tooltip_window_close: tooltip_window_close,
    });
    cx.set_global(UpdateState::default());
    config
}

/// Installs the globals the windows read, without starting a daemon or
/// touching the real configuration. Used by the headless render tests.
#[cfg(any(test, feature = "parity"))]
pub fn set_test_state(cx: &mut gpui::App, config: Arc<ConfigStore>) {
    // HeroGPUI components read their tokens from the `ThemeProvider` global and
    // panic without it, so the harness installs the same provider `main` does
    // and publishes the app's resolved tokens into it.
    herogpui::init(cx);
    let settings = config.get();
    let mode = crate::theme::presets::resolve_theme_mode(crate::theme::presets::ThemeMode::parse(
        &settings.appearance.mode,
    ));
    crate::theme::vars::init_theme(cx, mode, &settings.appearance.theme);
    #[cfg(test)]
    let daemon = DaemonHandle::with_binary(std::path::PathBuf::new());
    #[cfg(not(test))]
    let daemon = DaemonHandle::new();
    let service = CaptureService::new(daemon, config.clone());
    let coordinator = cx.new(|_| Coordinator::new(service.clone()));
    cx.set_global(CoordinatorHandle(coordinator));
    #[cfg(all(target_os = "macos", not(test)))]
    let tooltip_window_close =
        crate::system::macos_tooltip::init(cx, service.daemon.clone(), config);
    #[cfg(target_os = "macos")]
    let recording_target_close =
        crate::windows::recording_control::RecordingControl::watch_window_target_closure(
            &service.daemon,
            cx,
        );
    cx.set_global(AppState {
        service,
        #[cfg(target_os = "macos")]
        _recording_target_close: recording_target_close,
        #[cfg(all(target_os = "macos", not(test)))]
        _tooltip_window_close: tooltip_window_close,
    });
    cx.set_global(UpdateState::default());
}

pub fn set_native(cx: &mut gpui::App, bridge: NativeBridge) {
    // The bridge lives only on the main thread (gpui globals are never touched
    // from other threads), but macOS native objects keep it !Send.
    #[allow(clippy::arc_with_non_send_sync)]
    cx.set_global(NativeShell(Arc::new(bridge)));
}

pub fn native(cx: &gpui::App) -> Arc<NativeBridge> {
    cx.global::<NativeShell>().0.clone()
}

pub fn try_native(cx: &gpui::App) -> Option<Arc<NativeBridge>> {
    cx.try_global::<NativeShell>().map(|shell| shell.0.clone())
}

pub fn state(cx: &gpui::App) -> CaptureService {
    cx.global::<AppState>().service.clone()
}

pub fn update_cell(cx: &gpui::App) -> crate::update::Shared {
    cx.global::<UpdateState>().0.clone()
}
pub fn try_state(cx: &gpui::App) -> Option<CaptureService> {
    cx.try_global::<AppState>()
        .map(|state| state.service.clone())
}

pub fn coordinator(cx: &gpui::App) -> Entity<Coordinator> {
    cx.global::<CoordinatorHandle>().0.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[herogpui::test]
    fn render_state_never_starts_the_ambient_daemon(cx: &mut gpui::TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let config = Arc::new(ConfigStore::load_at(directory.path().join("config.json")).unwrap());
        cx.update(|cx| set_test_state(cx, config));
        let daemon = cx.read(|cx| state(cx).daemon);
        let result = daemon.window_selector().list();
        let running = daemon.is_running();
        daemon.stop();
        assert!(
            result.is_err(),
            "render fixtures must not connect to the ambient daemon"
        );
        assert!(!running);
    }
}
