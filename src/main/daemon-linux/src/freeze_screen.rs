use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use poratake_daemon_common::contract::{FREEZE_SCREEN_MODULE, FreezeScreenMethod};
use poratake_daemon_common::protocol::{Request, Response, send_response};
use poratake_daemon_common::router::{Module, Reply, method_not_found};
use serde_json::json;

use crate::Backend;
use crate::capture::{FrozenFrames, X11FreezeOverlay};

#[derive(Clone, Default)]
struct FreezeEpoch(Arc<AtomicU64>);

impl FreezeEpoch {
    fn advance(&self) -> u64 {
        self.0.fetch_add(1, Ordering::SeqCst) + 1
    }

    fn is_current(&self, epoch: u64) -> bool {
        self.0.load(Ordering::SeqCst) == epoch
    }
}

fn replace_current<T>(
    current: &FreezeEpoch,
    epoch: u64,
    state: &Mutex<Option<T>>,
    value: T,
    install: impl FnOnce(),
) -> Result<Option<T>, T> {
    let mut state = state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if !current.is_current(epoch) {
        return Err(value);
    }
    let previous = state.replace(value);
    install();
    Ok(previous)
}

pub struct FreezeScreenModule {
    backend: Backend,
    frozen: FrozenFrames,
    current: FreezeEpoch,
    overlay: Arc<Mutex<Option<X11FreezeOverlay>>>,
}

impl FreezeScreenModule {
    pub fn new(backend: Backend, frozen: FrozenFrames) -> Self {
        Self {
            backend,
            frozen,
            current: FreezeEpoch::default(),
            overlay: Arc::new(Mutex::new(None)),
        }
    }

    fn freeze(&self, request: &Request) -> Reply {
        let backend = self.backend;
        let frozen = self.frozen.clone();
        let current = self.current.clone();
        let overlay = self.overlay.clone();
        let epoch = current.advance();
        let id = request.id.clone();
        match std::thread::Builder::new()
            .name("linux-freeze-capture".into())
            .spawn(move || {
                let frames = match crate::capture::capture_display_frames(backend) {
                    Ok(frames) => frames,
                    Err(error) => {
                        if current.is_current(epoch) {
                            send_response(Response::error(
                                &id,
                                "CAPTURE_FAILED",
                                &error.to_string(),
                            ));
                        } else {
                            send_response(Response::success(&id, Some(json!({ "frozen": false }))));
                        }
                        return;
                    }
                };
                if !current.is_current(epoch) {
                    send_response(Response::success(&id, Some(json!({ "frozen": false }))));
                    return;
                }
                let presented = match crate::capture::present_frozen_frames(backend, &frames) {
                    Ok(presented) => presented,
                    Err(error) => {
                        send_response(Response::error(&id, "UI_ERROR", &error.to_string()));
                        return;
                    }
                };
                match replace_current(&current, epoch, &overlay, presented, || {
                    frozen.replace(frames)
                }) {
                    Ok(previous) => {
                        drop(previous);
                        send_response(Response::success(&id, Some(json!({ "frozen": true }))));
                    }
                    Err(stale) => {
                        drop(stale);
                        send_response(Response::success(&id, Some(json!({ "frozen": false }))));
                    }
                }
            }) {
            Ok(_) => Reply::Deferred,
            Err(error) => Reply::Now(Err(("CAPTURE_FAILED".into(), error.to_string()))),
        }
    }

    fn release(&self) -> Reply {
        self.current.advance();
        let overlay = self
            .overlay
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut overlay = overlay;
        let released = overlay.take();
        self.frozen.clear();
        drop(overlay);
        drop(released);
        Reply::Now(Ok(Some(json!({ "frozen": false }))))
    }

    fn prewarm(&self, request: &Request) -> Reply {
        let backend = self.backend;
        let id = request.id.clone();
        match std::thread::Builder::new()
            .name("linux-freeze-prewarm".into())
            .spawn(move || {
                let response = match crate::capture::capture_display_frames(backend) {
                    Ok(_) => Response::success(&id, Some(json!({ "prewarmed": true }))),
                    Err(error) => Response::error(&id, "CAPTURE_FAILED", &error.to_string()),
                };
                send_response(response);
            }) {
            Ok(_) => Reply::Deferred,
            Err(error) => Reply::Now(Err(("CAPTURE_FAILED".into(), error.to_string()))),
        }
    }
}

impl Module for FreezeScreenModule {
    fn name(&self) -> &'static str {
        FREEZE_SCREEN_MODULE
    }

    fn handle(&mut self, request: &Request) -> Reply {
        match FreezeScreenMethod::parse(&request.method) {
            Some(FreezeScreenMethod::Freeze) => self.freeze(request),
            Some(FreezeScreenMethod::Release) => self.release(),
            Some(FreezeScreenMethod::Prewarm) => self.prewarm(request),
            None => method_not_found(&request.method),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_before_freeze_is_idempotent() {
        let mut module = FreezeScreenModule::new(Backend::Headless, FrozenFrames::default());
        let request = Request {
            id: "release".into(),
            module: FREEZE_SCREEN_MODULE.into(),
            method: FreezeScreenMethod::Release.id().into(),
            params: None,
        };
        let Reply::Now(result) = module.handle(&request) else {
            panic!("release should complete immediately");
        };
        assert_eq!(result.expect("release"), Some(json!({ "frozen": false })));
    }

    #[test]
    fn stale_freezes_cannot_replace_released_state() {
        let current = FreezeEpoch::default();
        let freeze = current.advance();
        current.advance();
        let state = Mutex::new(Some(1));
        let installed = AtomicU64::new(0);

        assert_eq!(
            replace_current(&current, freeze, &state, 2, || {
                installed.store(1, Ordering::SeqCst)
            }),
            Err(2)
        );
        assert_eq!(installed.load(Ordering::SeqCst), 0);
        assert_eq!(
            *state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
            Some(1)
        );
    }
}
