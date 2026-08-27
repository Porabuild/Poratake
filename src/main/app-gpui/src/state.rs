//! Application-wide services exposed as a GPUI global: capture pipeline,
//! config store, daemon handle and the native shell bridge.

use std::sync::Arc;

use gpui::{prelude::*, Entity};

use crate::capture::coordinator::{Coordinator, CoordinatorHandle};
use crate::capture::CaptureService;
use crate::config::store::ConfigStore;
use crate::daemon::DaemonHandle;
use crate::system::native::NativeBridge;

pub struct AppState {
    pub service: CaptureService,
}

impl gpui::Global for AppState {}

struct NativeShell(Arc<NativeBridge>);

impl gpui::Global for NativeShell {}

pub fn init(cx: &mut gpui::App) -> Arc<ConfigStore> {
    let config = match ConfigStore::load() {
        Ok(store) => Arc::new(store),
        Err(error) => {
            eprintln!("[settings] failed to load config: {error}");
            std::process::exit(1);
        }
    };

    let service = CaptureService::new(DaemonHandle::new(), config.clone());

    if let Err(error) = service.daemon.start() {
        // Capture features degrade gracefully; the editor still works for
        // files opened from disk.
        eprintln!("[daemon] failed to start: {error}");
    }

    let coordinator = cx.new(|_| Coordinator::new(service.clone()));
    cx.set_global(CoordinatorHandle(coordinator));
    cx.set_global(AppState { service });
    config
}

/// Installs the globals the windows read, without starting a daemon or
/// touching the real configuration. Used by the headless render tests.
#[cfg(test)]
pub fn set_test_state(cx: &mut gpui::App, config: Arc<ConfigStore>) {
    let service = CaptureService::new(DaemonHandle::new(), config);
    let coordinator = cx.new(|_| Coordinator::new(service.clone()));
    cx.set_global(CoordinatorHandle(coordinator));
    cx.set_global(AppState { service });
}

pub fn set_native(cx: &mut gpui::App, bridge: NativeBridge) {
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

pub fn coordinator(cx: &gpui::App) -> Entity<Coordinator> {
    cx.global::<CoordinatorHandle>().0.clone()
}
