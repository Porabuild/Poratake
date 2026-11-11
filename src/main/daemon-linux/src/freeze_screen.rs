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

fn begin_current<T>(current: &FreezeEpoch, state: &Mutex<Option<T>>) -> u64 {
    let _state = state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    current.advance()
}

fn replace_current<T, U, E>(
    current: &FreezeEpoch,
    epoch: u64,
    state: &Mutex<Option<T>>,
    create: impl FnOnce() -> Result<(T, U), E>,
    install: impl FnOnce(U),
) -> Result<bool, E> {
    let mut state = state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if !current.is_current(epoch) {
        return Ok(false);
    }
    let (value, payload) = create()?;
    let previous = state.replace(value);
    install(payload);
    drop(previous);
    Ok(true)
}

fn release_current<T>(current: &FreezeEpoch, state: &Mutex<Option<T>>, clear: impl FnOnce()) {
    let mut state = state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    current.advance();
    let released = state.take();
    clear();
    drop(released);
    drop(state);
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
        let epoch = begin_current(&current, &overlay);
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
                match replace_current(
                    &current,
                    epoch,
                    &overlay,
                    || {
                        crate::capture::present_frozen_frames(backend, &frames)
                            .map(|presented| (presented, frames))
                    },
                    |frames| frozen.replace(frames),
                ) {
                    Ok(true) => {
                        send_response(Response::success(&id, Some(json!({ "frozen": true }))));
                    }
                    Ok(false) => {
                        send_response(Response::success(&id, Some(json!({ "frozen": false }))));
                    }
                    Err(error) => {
                        send_response(Response::error(&id, "UI_ERROR", &error.to_string()));
                    }
                }
            }) {
            Ok(_) => Reply::Deferred,
            Err(error) => Reply::Now(Err(("CAPTURE_FAILED".into(), error.to_string()))),
        }
    }

    fn release(&self) -> Reply {
        release_current(&self.current, &self.overlay, || self.frozen.clear());
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
        let state = Mutex::new(Some(1));
        let freeze = begin_current(&current, &state);
        begin_current(&current, &state);
        let installed = AtomicU64::new(0);

        assert_eq!(
            replace_current(
                &current,
                freeze,
                &state,
                || Ok::<_, ()>((2, ())),
                |()| { installed.store(1, Ordering::SeqCst) }
            ),
            Ok(false)
        );
        assert_eq!(installed.load(Ordering::SeqCst), 0);
        assert_eq!(
            *state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
            Some(1)
        );
    }

    #[test]
    fn release_and_presentation_use_the_same_lock() {
        let current = FreezeEpoch::default();
        let state = Mutex::new(None);
        let epoch = begin_current(&current, &state);
        assert_eq!(
            replace_current(
                &current,
                epoch,
                &state,
                || {
                    assert!(matches!(
                        state.try_lock(),
                        Err(std::sync::TryLockError::WouldBlock)
                    ));
                    Ok::<_, ()>((2, ()))
                },
                |()| {},
            ),
            Ok(true)
        );
        assert_eq!(*state.lock().unwrap(), Some(2));
        release_current(&current, &state, || {
            assert!(matches!(
                state.try_lock(),
                Err(std::sync::TryLockError::WouldBlock)
            ));
        });
        assert!(!current.is_current(epoch));
        assert_eq!(*state.lock().unwrap(), None);
    }

    #[test]
    fn newer_freeze_invalidates_capture_in_progress() {
        let current = FreezeEpoch::default();
        let state = Arc::new(Mutex::new(Some(1)));
        let epoch = begin_current(&current, &state);
        let installed = Arc::new(AtomicU64::new(0));
        let (capturing_tx, capturing_rx) = std::sync::mpsc::sync_channel(1);
        let (resume_tx, resume_rx) = std::sync::mpsc::sync_channel(1);
        let worker_epoch = current.clone();
        let worker_state = state.clone();
        let worker_installed = installed.clone();
        let worker = std::thread::spawn(move || {
            capturing_tx.send(()).unwrap();
            resume_rx.recv().unwrap();
            replace_current(
                &worker_epoch,
                epoch,
                &worker_state,
                || Ok::<_, ()>((2, ())),
                |()| worker_installed.store(1, Ordering::SeqCst),
            )
        });
        capturing_rx.recv().unwrap();
        begin_current(&current, &state);
        resume_tx.send(()).unwrap();
        assert_eq!(worker.join().unwrap(), Ok(false));
        assert_eq!(installed.load(Ordering::SeqCst), 0);
        assert_eq!(*state.lock().unwrap(), Some(1));
    }

    #[test]
    fn replaced_freeze_window_closes_under_the_lock() {
        struct Overlay {
            state: Option<std::sync::Weak<Mutex<Option<Overlay>>>>,
            closed_under_lock: Arc<std::sync::atomic::AtomicBool>,
        }
        impl Drop for Overlay {
            fn drop(&mut self) {
                if let Some(state) = self.state.as_ref().and_then(std::sync::Weak::upgrade) {
                    self.closed_under_lock.store(
                        matches!(state.try_lock(), Err(std::sync::TryLockError::WouldBlock)),
                        Ordering::SeqCst,
                    );
                }
            }
        }

        let current = FreezeEpoch::default();
        let state = Arc::new(Mutex::new(None));
        let closed_under_lock = Arc::new(std::sync::atomic::AtomicBool::new(false));
        *state.lock().unwrap() = Some(Overlay {
            state: Some(Arc::downgrade(&state)),
            closed_under_lock: closed_under_lock.clone(),
        });
        let epoch = begin_current(&current, &state);
        assert_eq!(
            replace_current(
                &current,
                epoch,
                &state,
                || {
                    Ok::<_, ()>((
                        Overlay {
                            state: None,
                            closed_under_lock: closed_under_lock.clone(),
                        },
                        (),
                    ))
                },
                |()| {},
            ),
            Ok(true)
        );
        assert!(closed_under_lock.load(Ordering::SeqCst));
    }
}
